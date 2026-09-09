//! Modal overlay rendering (command palette, goto line, find/replace, etc.)
//!
//! Every modal renders through `overlay_surface::OverlaySurface` — chrome,
//! header, sectioned list, fields, and footer are drawn by that one
//! component; this module builds the per-context `OverlaySpec` (real row
//! content) and, for hit-testing/caret placement, a cheaper *shape-only*
//! spec (placeholder row content, real counts/titles/sections) via
//! `with_modal_overlay_layout` — both go through the same
//! `overlay_surface::layout()`, so geometry can't drift between paint and
//! hit-test (overlay-surface.md "Hit-testing": one layout, two consumers).

use crate::completion::menu::MenuItemKind;
use crate::model::ui::{LanguagePickerState, LspServersState, RecentFilesState, ThemePickerState};
use crate::model::AppModel;
use crate::theme::ThemeInfo;

use super::frame::{Frame, RoundedRectMaskCache, TextPainter};
use super::geometry::WidgetRect;
use super::overlay_surface::{
    self, Accessory, Anchor, Body, Field, FlatIndex, Footer, Header, OverlayLayout, OverlaySpec,
    Row, RowIcon, Section, TabBar, WidthRule, Zones,
};
use super::text_field::{TextFieldContent, TextFieldRenderer};

use crate::model::COMMAND_PALETTE_MAX_VISIBLE;

/// Modal dim background alpha (102/255 ≈ 40% opacity)
/// The prompt glyph before the palette query. Shared by the render spec
/// and the layout-only spec so `modal_header_input_rect` sees the same
/// glyph the painter draws.
const PALETTE_HEADER_GLYPH: char = '\u{276F}';
const MODAL_DIM_ALPHA: u8 = 0x66;

#[derive(Clone, Copy)]
struct ModalColors {
    fg: u32,
    dim: u32,
    input_bg: u32,
}

impl ModalColors {
    fn from_model(model: &AppModel) -> Self {
        Self {
            fg: model.theme.overlay.foreground.to_argb_u32(),
            dim: model.theme.overlay.foreground.with_alpha(128).to_argb_u32(),
            input_bg: model.theme.overlay.input_background.to_argb_u32(),
        }
    }
}

/// Constant rendering context shared by every modal for the duration of a
/// single `render_modals` call: window size, the shared line/char metrics,
/// and the resolved theme colors.
struct ModalRenderCtx {
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: ModalColors,
    scale_factor: f64,
}

// ============================================================================
// Width rules (Visual Language > Chrome)
// ============================================================================

const PALETTE_WIDTH: (f32, f32, f32) = (0.5, 480.0, 640.0);
const PICKER_WIDTH: (f32, f32, f32) = (0.7, 520.0, 900.0);
const SMALL_MODAL_WIDTH: (f32, f32, f32) = (0.5, 300.0, 500.0);

/// Primary selection of a single-line editable as an ordered, end-exclusive
/// char-column range, for `Header.selection`. `None` when collapsed.
fn editable_selection(
    editable: &crate::editable::EditableState<crate::editable::StringBuffer>,
) -> Option<(usize, usize)> {
    if !editable.has_selection() {
        return None;
    }
    let sel = editable.selection();
    let (start, end) = (sel.start().column, sel.end().column);
    (end > start).then_some((start, end))
}

fn width_rule((pct, min, max): (f32, f32, f32)) -> WidthRule {
    WidthRule { pct, min, max }
}

/// The All tab's `max_visible`, in `FlatIndex` display slots rather than
/// selectable rows: `flatten_rows` (overlay_surface.rs) emits one extra
/// slot per titled section header, and the All tab is deliberately
/// non-scrolling, so anything past `max_visible` is permanently
/// unreachable — undercounting silently clips the tail (e.g. the last file
/// row, or even the leading "Commands" header).
fn all_tab_max_visible(sections_spec: &[(Option<&'static str>, usize)]) -> usize {
    let rows: usize = sections_spec.iter().map(|&(_, len)| len).sum();
    let headers = sections_spec.iter().filter(|(t, _)| t.is_some()).count();
    (rows + headers).max(1)
}

// ============================================================================
// Command Palette
// ============================================================================

/// Per-row accessory decision: a keybinding renders as keycap chips unless
/// it has more than 4 chips total, in which case it falls back to dim text
/// (Visual Language > Keycaps).
enum PaletteAccessory {
    None,
    DimText(String),
    Keycaps(Vec<Vec<overlay_surface::Chip>>),
}

fn command_accessories(
    model: &AppModel,
    matches: &[crate::model::CommandMatch],
) -> Vec<PaletteAccessory> {
    let context = crate::keymap::KeyContext::for_command_hints(model);
    matches
        .iter()
        .map(|m| {
            palette_accessory(crate::commands::keybinding_for_command(
                m.def.id,
                &model.ui.keymap,
                &context,
            ))
        })
        .collect()
}

fn palette_accessory(keybinding: Option<String>) -> PaletteAccessory {
    use crate::view::overlay_surface::{binding_chips, chip_count};

    match keybinding {
        None => PaletteAccessory::None,
        Some(kb) => {
            let steps = binding_chips(&kb);
            if chip_count(&steps) > 4 {
                PaletteAccessory::DimText(kb)
            } else {
                PaletteAccessory::Keycaps(steps)
            }
        }
    }
}

/// Build the row set for the Commands tab's `matches` cache, plus the
/// accessory storage its `Row::accessory`s borrow into for the render call.
fn command_rows<'a>(
    matches: &'a [crate::model::CommandMatch],
    accessories: &'a [PaletteAccessory],
    icon_color: u32,
) -> Vec<Row<'a>> {
    matches
        .iter()
        .zip(accessories)
        .map(|(m, accessory)| Row {
            icon: RowIcon::Glyph {
                ch: m.def.category.glyph(),
                color: icon_color,
            },
            label: m.def.label,
            match_indices: &m.indices,
            detail: None,
            detail_style: None,
            accessory: match accessory {
                PaletteAccessory::None => Accessory::None,
                PaletteAccessory::DimText(kb) => Accessory::DimText(kb),
                PaletteAccessory::Keycaps(steps) => Accessory::Keycaps(steps),
            },
        })
        .collect()
}

fn file_rows(results: &[crate::model::FileMatch], icon_color: u32) -> Vec<Row<'_>> {
    results
        .iter()
        .map(|m| Row {
            icon: RowIcon::Glyph {
                ch: file_icon_char(&m.path),
                color: icon_color,
            },
            label: &m.filename,
            match_indices: &m.indices,
            detail: Some(&m.relative_path),
            detail_style: None,
            accessory: Accessory::None,
        })
        .collect()
}

/// Search Everywhere tab labels + match counts, in `SearchTab::ORDER`
/// (overlay-surface.md Phase 4 "Search Everywhere tabs"). Counts are
/// `Hidden` on an empty query — depth lives in the tabs, so this is also
/// what makes tab counts meaningful once a query is typed.
fn search_tab_bar<'a>(
    state: &crate::model::ui::CommandPaletteState,
) -> (Vec<(&'a str, overlay_surface::TabCount)>, usize) {
    use crate::model::SearchTab;
    use overlay_surface::TabCount;

    let query_empty = state.input().is_empty();
    let commands_total = state.matches.len() - state.recent_count;
    let file_total = if state.files_available {
        state.files.as_ref().map(|f| f.results.len()).unwrap_or(0)
    } else {
        0
    };
    let count_for = |n: usize| {
        if query_empty {
            TabCount::Hidden
        } else {
            TabCount::N(n)
        }
    };

    let tabs = vec![
        // "All" is a match count for the current query too — the union of
        // the Commands and Files counts (overlay-surface.md Phase 4: "Tab
        // counts are match counts for the current query").
        (
            "All",
            count_for(commands_total + file_total + state.symbols.results.items.len()),
        ),
        ("Commands", count_for(commands_total)),
        (
            "Files",
            if !state.files_available {
                TabCount::Unavailable
            } else {
                state
                    .files
                    .as_ref()
                    .map(|f| count_for(f.results.len()))
                    .unwrap_or(TabCount::Hidden)
            },
        ),
        (
            "Symbols",
            if state.symbols.available {
                count_for(state.symbols.results.items.len())
            } else {
                TabCount::Unavailable
            },
        ),
    ];
    let active = SearchTab::ORDER
        .iter()
        .position(|&t| t == state.active_tab)
        .unwrap_or(0);
    (tabs, active)
}

fn render_command_palette_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::CommandPaletteState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    use crate::model::SearchTab;
    use crate::update::search_everywhere_sections;

    let input_text = state.input();
    let icon_color = model.theme.overlay.text_dim.to_argb_u32();

    // Per-row keycap chip storage lives here so `Row::accessory` can borrow
    // into it for the duration of this render call (spec lifetime: built
    // and consumed in one scope — see overlay-surface.md "The spec").
    let accessories = command_accessories(model, &state.matches);
    let cmd_rows = command_rows(&state.matches, &accessories, icon_color);
    let file_rows_all = state
        .files
        .as_ref()
        .map(|f| file_rows(&f.results, icon_color))
        .unwrap_or_default();
    let symbol_rows: Vec<_> = state
        .symbols
        .results
        .items
        .iter()
        .map(|symbol| Row {
            icon: RowIcon::Glyph {
                ch: '◇',
                color: icon_color,
            },
            label: &symbol.name,
            match_indices: &[],
            detail: Some(&symbol.detail),
            detail_style: None,
            accessory: Accessory::None,
        })
        .collect();

    let (tab_labels, active_tab_idx) = search_tab_bar(state);
    let tabs = TabBar {
        tabs: &tab_labels,
        active: active_tab_idx,
    };

    // `search_everywhere_sections` (`update::ui`) is the ordering authority
    // for section boundaries — the same counts drive `Confirm`/
    // `SelectNext` and the shape-only spec `with_modal_overlay_layout`
    // builds below, so real rows, selection, and hit-testing can't drift
    // apart (overlay-surface.md "Hit-testing": one layout, two consumers).
    // Empty-state messages are drawn as decoration after `render()`, not as
    // rows, keeping `FlatIndex` space limited to real, actionable rows.
    let sections_spec = search_everywhere_sections(state);
    let (sections, selected_index, scroll, max_visible): (Vec<Section>, usize, usize, usize) =
        match state.active_tab {
            SearchTab::Commands => {
                let mut offset = 0;
                let sections = sections_spec
                    .iter()
                    .map(|&(title, len)| {
                        let rows = &cmd_rows[offset..offset + len];
                        offset += len;
                        Section { title, rows }
                    })
                    .collect();
                let sel = state.selected_index.min(cmd_rows.len().saturating_sub(1));
                (
                    sections,
                    sel,
                    state.scroll_offset,
                    COMMAND_PALETTE_MAX_VISIBLE,
                )
            }
            SearchTab::Files => {
                let sections = sections_spec
                    .iter()
                    .map(|&(title, len)| Section {
                        title,
                        rows: &file_rows_all[..len],
                    })
                    .collect();
                let files_selected = state.files.as_ref().map(|f| f.selected_index).unwrap_or(0);
                let files_scroll = state.files.as_ref().map(|f| f.scroll_offset).unwrap_or(0);
                (
                    sections,
                    files_selected,
                    files_scroll,
                    COMMAND_PALETTE_MAX_VISIBLE,
                )
            }
            SearchTab::All => {
                let mut cmd_offset = 0;
                let mut file_offset = 0;
                let sections = sections_spec
                    .iter()
                    .map(|&(title, len)| {
                        if title == Some("Files") {
                            let rows = &file_rows_all[file_offset..file_offset + len];
                            file_offset += len;
                            Section { title, rows }
                        } else if title == Some("Symbols") {
                            Section {
                                title,
                                rows: &symbol_rows[..len],
                            }
                        } else {
                            let rows = &cmd_rows[cmd_offset..cmd_offset + len];
                            cmd_offset += len;
                            Section { title, rows }
                        }
                    })
                    .collect();
                (
                    sections,
                    state.all_selected,
                    0,
                    all_tab_max_visible(&sections_spec),
                )
            }
            SearchTab::Symbols => (
                sections_spec
                    .iter()
                    .map(|&(title, len)| Section {
                        title,
                        rows: &symbol_rows[..len],
                    })
                    .collect(),
                state.symbols.selected_index,
                state.symbols.scroll_offset,
                COMMAND_PALETTE_MAX_VISIBLE,
            ),
        };

    let spec = OverlaySpec {
        tabs: Some(tabs),
        anchor: Anchor::Centered {
            width: width_rule(PALETTE_WIDTH),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: Some(PALETTE_HEADER_GLYPH),
            text: &input_text,
            placeholder: "Search commands, files, symbols\u{2026}",
            caret: Some(
                state
                    .editable
                    .cursors()
                    .first()
                    .map(|c| c.column)
                    .unwrap_or(0),
            ),
            selection: editable_selection(&state.editable),
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll,
            max_visible,
        },
        footer: Some(Footer {
            leading: if matches!(state.active_tab, SearchTab::All | SearchTab::Symbols)
                && state.symbols.available
            {
                state.symbols.status()
            } else {
                None
            }
            .unwrap_or("\u{2191}\u{2193} navigate \u{00b7} \u{21b5} run \u{00b7} \u{21e5} tab"),
            trailing: "esc dismiss",
        }),
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );

    // Empty-state messages (existing strings preserved) — decoration only,
    // drawn outside `Body::List` so they don't occupy `FlatIndex` space
    // (fixes reusing the "no workspace" message when a workspace *is* open
    // but the query matches nothing, overlay-surface.md Phase 4 tabs).
    let empty_message: Option<&str> = match state.active_tab {
        SearchTab::Files if !state.files_available => {
            Some("No workspace open \u{2014} use \u{2318}\u{21e7}O after Cmd+O")
        }
        SearchTab::Files if file_rows_all.is_empty() && !input_text.is_empty() => {
            Some("No files match your query")
        }
        SearchTab::All if sections.is_empty() => Some("No matches"),
        SearchTab::Symbols if symbol_rows.is_empty() => state.symbols.status(),
        _ => None,
    };
    if let Some(text) = empty_message {
        let l =
            overlay_surface::layout(&spec, ctx.window_width, ctx.window_height, ctx.scale_factor);
        if let Some(header) = l.header {
            let y = header.y + header.h + ctx.line_height / 4;
            painter.draw(frame, l.panel.x + 16, y, text, ctx.colors.dim);
        }
    }
}

// ============================================================================
// File Finder
// ============================================================================

fn render_file_finder_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::FileFinderState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let input_text = state.input();
    let icon_color = model.theme.overlay.text_dim.to_argb_u32();

    let rows: Vec<Row> = state
        .results
        .iter()
        .map(|m| Row {
            icon: RowIcon::Glyph {
                ch: file_icon_char(&m.path),
                color: icon_color,
            },
            label: &m.filename,
            match_indices: &m.indices,
            detail: Some(&m.relative_path),
            detail_style: None,
            accessory: Accessory::None,
        })
        .collect();
    let sections = [Section {
        title: None,
        rows: &rows,
    }];
    let selected_index = state.selected_index.min(rows.len().saturating_sub(1));

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: width_rule(PICKER_WIDTH),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: None,
            text: &input_text,
            placeholder: "Go to File...",
            caret: Some(
                state
                    .editable
                    .cursors()
                    .first()
                    .map(|c| c.column)
                    .unwrap_or(0),
            ),
            selection: editable_selection(&state.editable),
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: Some(Footer {
            leading: "\u{2191}\u{2193} navigate \u{00b7} \u{21b5} open",
            trailing: "esc dismiss",
        }),
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );

    if state.results.is_empty() && !input_text.is_empty() {
        // Empty-state message row (existing string preserved).
        let l =
            overlay_surface::layout(&spec, ctx.window_width, ctx.window_height, ctx.scale_factor);
        if let Some(header) = l.header {
            let y = header.y + header.h + ctx.line_height / 4;
            painter.draw(
                frame,
                l.panel.x + 16,
                y,
                "No files match your query",
                ctx.colors.dim,
            );
        }
    }
}

fn file_icon_char(path: &std::path::Path) -> char {
    crate::model::FileExtension::from_path(path)
        .icon()
        .chars()
        .next()
        .unwrap_or('?')
}

// ============================================================================
// Recent Files
// ============================================================================

/// Group `state.filtered_rows` into contiguous (title, entry-index) runs by
/// `RecentGroup` — the single source of truth for Recent Files section
/// boundaries, shared by the content spec (real rows) and the shape spec
/// (hit-testing/caret placement) so they can't drift.
pub(crate) fn recent_files_groups(state: &RecentFilesState) -> Vec<(&'static str, Vec<usize>)> {
    let mut groups: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for &entry_idx in &state.filtered_rows {
        let title = state.entries[entry_idx].group().title();
        match groups.last_mut() {
            Some((last_title, indices)) if *last_title == title => indices.push(entry_idx),
            _ => groups.push((title, vec![entry_idx])),
        }
    }
    groups
}

fn render_recent_files_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &RecentFilesState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let input_text = state.input();
    let icon_color = model.theme.overlay.text_dim.to_argb_u32();
    let groups = recent_files_groups(state);

    // Per-row time-ago strings live here so `Row::accessory` can borrow them
    // for the render call.
    let time_strings: Vec<Vec<String>> = groups
        .iter()
        .map(|(_, indices)| {
            indices
                .iter()
                .map(|&i| state.entries[i].time_ago())
                .collect()
        })
        .collect();
    let display_paths: Vec<Vec<String>> = groups
        .iter()
        .map(|(_, indices)| {
            indices
                .iter()
                .map(|&i| state.entries[i].display_path())
                .collect()
        })
        .collect();

    let row_groups: Vec<Vec<Row>> = groups
        .iter()
        .enumerate()
        .map(|(gi, (_, indices))| {
            indices
                .iter()
                .enumerate()
                .map(|(ri, &entry_idx)| Row {
                    icon: RowIcon::Glyph {
                        ch: file_icon_char(&state.entries[entry_idx].path),
                        color: icon_color,
                    },
                    label: &display_paths[gi][ri],
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::DimText(&time_strings[gi][ri]),
                })
                .collect()
        })
        .collect();
    let sections: Vec<Section> = groups
        .iter()
        .zip(&row_groups)
        .map(|((title, _), rows)| Section {
            title: Some(title),
            rows,
        })
        .collect();

    let total_rows = state.filtered_rows.len();
    let selected_index = state.selected_index.min(total_rows.saturating_sub(1));

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: width_rule(PICKER_WIDTH),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: None,
            text: &input_text,
            placeholder: "Recent Files...",
            caret: Some(
                state
                    .editable
                    .cursors()
                    .first()
                    .map(|c| c.column)
                    .unwrap_or(0),
            ),
            selection: editable_selection(&state.editable),
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: Some(Footer {
            leading: "\u{2191}\u{2193} navigate \u{00b7} \u{21b5} open \u{00b7} \u{2318}. pin",
            trailing: "esc dismiss",
        }),
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );

    if total_rows == 0 && !input_text.is_empty() {
        let l =
            overlay_surface::layout(&spec, ctx.window_width, ctx.window_height, ctx.scale_factor);
        if let Some(header) = l.header {
            let y = header.y + header.h + ctx.line_height / 4;
            painter.draw(
                frame,
                l.panel.x + 16,
                y,
                "No recent files match your query",
                ctx.colors.dim,
            );
        }
    }
}

// ============================================================================
// Theme Picker
// ============================================================================

/// Group `themes` into contiguous (title, theme-index) runs by source — the
/// single source of truth for Theme Picker section boundaries. `FlatIndex`
/// equals the theme's index in `themes` directly (the list is static per
/// modal-open, no filtering/reordering happens), so no separate
/// ordering-authority cache is needed the way Recent Files needs one.
pub(crate) fn theme_picker_groups(
    themes: &[ThemeInfo],
) -> Vec<(&'static str, std::ops::Range<usize>)> {
    use crate::theme::ThemeSource;

    let mut groups: Vec<(&'static str, std::ops::Range<usize>)> = Vec::new();
    for (i, theme_info) in themes.iter().enumerate() {
        let title = match theme_info.source {
            ThemeSource::User => "User Themes",
            ThemeSource::Builtin => "Built-in Themes",
        };
        match groups.last_mut() {
            Some((last_title, range)) if *last_title == title => range.end = i + 1,
            _ => groups.push((title, i..i + 1)),
        }
    }
    groups
}

fn render_theme_picker_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &ThemePickerState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let fallback_swatch = crate::theme::ThemeSwatch::fallback();
    let groups = theme_picker_groups(&state.themes);

    let row_groups: Vec<Vec<Row>> = groups
        .iter()
        .map(|(_, range)| {
            range
                .clone()
                .map(|idx| {
                    let theme_info = &state.themes[idx];
                    // `swatches` is parallel to `themes`; guard anyway so a
                    // stale state never panics rendering.
                    let swatch = state.swatches.get(idx).unwrap_or(&fallback_swatch);
                    // ✓ marks only the theme that is *set* (config), never
                    // a merely-loaded/previewed one.
                    let is_active = model.config.theme == theme_info.id;
                    Row {
                        icon: RowIcon::Glyph {
                            ch: '\u{25CF}',
                            color: swatch.accent,
                        },
                        label: &theme_info.name,
                        match_indices: &[],
                        detail: None,
                        detail_style: None,
                        accessory: Accessory::Swatches {
                            colors: &swatch.colors,
                            active: is_active,
                        },
                    }
                })
                .collect()
        })
        .collect();
    let sections: Vec<Section> = groups
        .iter()
        .zip(&row_groups)
        .map(|((title, _), rows)| Section {
            title: Some(title),
            rows,
        })
        .collect();

    let selected_index = state
        .selected_index
        .min(state.themes.len().saturating_sub(1));

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: WidthRule {
                pct: 0.0,
                min: 400.0,
                max: 400.0,
            },
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: None,
            text: "",
            placeholder: "Switch Theme",
            caret: None,
            selection: None,
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: None,
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );
}

// ============================================================================
// Settings
// ============================================================================

/// Both rendering and hit testing build the actual same settings spec, including
/// the choice count on each row. No placeholder can lose its chip hit targets.
pub(crate) fn with_settings_spec<R>(
    model: &AppModel,
    state: &crate::settings::SettingsState,
    f: impl FnOnce(&OverlaySpec) -> R,
) -> R {
    use crate::settings::{keymap::SettingsTab, RowKind};
    let keymap_tab = state.tab == SettingsTab::Keymap;
    let capturing = state.keymap.capture.is_some();
    let categories = crate::settings::categories();
    let tab_labels: Vec<_> = categories
        .iter()
        .map(|category| {
            (
                category.unwrap_or("All Settings"),
                overlay_surface::TabCount::Hidden,
            )
        })
        .collect();
    let tabs = TabBar {
        tabs: &tab_labels,
        active: if keymap_tab {
            categories.len() - 1
        } else {
            state.category
        },
    };
    let groups = state.sections();
    let details: Vec<_> = state
        .rows
        .iter()
        .map(|&id| {
            if matches!(state.entries[id].kind, RowKind::Preset(i) if crate::settings::DESCRIPTORS[i].setting == crate::settings::Setting::Theme) {
                std::borrow::Cow::Borrowed(model.config.theme.as_str())
            } else { state.entries[id].detail(&model.config) }
        })
        .collect();
    let statuses: Vec<_> = state
        .rows
        .iter()
        .map(|&id| state.entries[id].status(model))
        .collect();
    let bindings: Vec<_> = state
        .rows
        .iter()
        .map(|&id| match state.entries[id].kind {
            RowKind::KeymapBinding(index, _) => index
                .and_then(|index| state.keymap.snapshot.as_ref()?.bindings.get(index))
                .map(|binding| palette_accessory(Some(binding.display_string())))
                .unwrap_or_else(|| PaletteAccessory::DimText("Unassigned".into())),
            _ => PaletteAccessory::None,
        })
        .collect();
    let row_groups: Vec<Vec<Row>> = groups
        .iter()
        .map(|(_, range)| {
            state.rows[range.clone()]
                .iter()
                .enumerate()
                .map(|(offset, &id)| {
                    let index = range.start + offset;
                    let entry = &state.entries[id];
                    Row {
                        icon: RowIcon::None,
                        label: &entry.name,
                        detail: if matches!(entry.kind, RowKind::ServerCommand(_))
                            || matches!(entry.kind, RowKind::Preset(i) if crate::settings::DESCRIPTORS[i].setting == crate::settings::Setting::Theme) {
                            None
                        } else { Some(&details[index]) },
                        detail_style: None,
                        match_indices: &[],
                        accessory: if matches!(entry.kind, RowKind::Preset(i) if crate::settings::DESCRIPTORS[i].setting == crate::settings::Setting::Theme) {
                            Accessory::SettingValue { text: &model.config.theme, action: Some("Choose…") }
                        } else if matches!(entry.kind, RowKind::ServerCommand(_)) {
                            Accessory::SettingValue { text: &details[index], action: None }
                        } else if matches!(entry.kind, RowKind::KeymapBinding(_, _)) {
                            match &bindings[index] {
                                PaletteAccessory::None => Accessory::None,
                                PaletteAccessory::DimText(text) => Accessory::DimText(text),
                                PaletteAccessory::Keycaps(steps) => Accessory::Keycaps(steps),
                            }
                        } else if entry.choices().is_empty() {
                            Accessory::DimText(statuses[index].as_deref().unwrap_or("Read-only"))
                        } else {
                            Accessory::Choices {
                                labels: entry.choices(),
                                active: if matches!(entry.kind, RowKind::KeymapBase) {
                                    state.keymap.base_index()
                                } else {
                                    entry.active(&model.config)
                                },
                            }
                        },
                    }
                })
                .collect()
        })
        .collect();
    let sections: Vec<Section> = groups
        .iter()
        .zip(&row_groups)
        .map(|((title, _), rows)| Section {
            title: Some(title),
            rows,
        })
        .collect();
    let query = state
        .keymap
        .capture
        .as_ref()
        .map(|capture| {
            capture
                .strokes
                .iter()
                .map(crate::keymap::Keystroke::display_string)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| state.editable.text());
    let selected_detail = state.selected_index.min(details.len().saturating_sub(1));
    let spec = OverlaySpec {
        tabs: Some(tabs),
        anchor: Anchor::Settings {
            width: WidthRule {
                pct: 0.95,
                min: 0.0,
                max: 1160.0,
            },
        },
        header: Some(Header {
            glyph: None,
            text: &query,
            placeholder: if capturing {
                "Recording shortcut…"
            } else if keymap_tab {
                "Search keybindings…"
            } else {
                "Search settings…"
            },
            caret: (!capturing).then_some(state.editable.cursor().column),
            selection: if capturing {
                None
            } else {
                editable_selection(&state.editable)
            },
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(state.selected_index),
            scroll: state.scroll_offset_px,
            max_visible: overlay_surface::settings_visible_count(
                model.window_size.0 as usize,
                model.window_size.1 as usize,
                model.metrics.scale_factor,
            ),
        },
        footer: Some(Footer {
            leading: if keymap_tab {
                &state.keymap.status
            } else {
                details
                    .get(selected_detail)
                    .map_or("No matching settings", |detail| detail.as_ref())
            },
            trailing: if keymap_tab {
                ""
            } else {
                "←→ change · Tab category · Esc close"
            },
        }),
        docs: None,
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
    };
    f(&spec)
}

// ============================================================================
// Language Servers picker
// ============================================================================

/// `(icon color, state label)` for a server's live state, dimming anything
/// that isn't `Ready`/`Failed`/`Missing` — `Starting`/`Indexing` and
/// "never started"/`ShuttingDown`/`Restarting` all read as the same "not
/// actively serving" dim dot (overlay-surface.md's Row color table for this
/// modal only calls out the four states worth a distinct color).
fn lsp_server_state_visual(model: &AppModel, server_id: &str) -> (u32, &'static str) {
    use crate::lsp::ServerState;
    let overlay = &model.theme.overlay;
    match model
        .lsp
        .servers
        .get(&crate::lsp::LspServerId::from(server_id))
    {
        Some(ServerState::Ready) => (overlay.accent.to_argb_u32(), "Ready"),
        Some(ServerState::Starting) => (overlay.text_dim.to_argb_u32(), "Starting"),
        Some(ServerState::Indexing) => (overlay.text_dim.to_argb_u32(), "Indexing"),
        Some(ServerState::Failed) => (overlay.severity_error.to_argb_u32(), "Failed"),
        Some(ServerState::Missing) => (overlay.severity_warning.to_argb_u32(), "Missing"),
        _ => (overlay.text_dim.to_argb_u32(), "Off"),
    }
}

/// Whether `lsp.servers.<id>.enabled` allows this server to run (absent ==
/// enabled — same default `lsp::resolve_server` uses).
fn lsp_server_config_enabled(model: &AppModel, server_id: &str) -> bool {
    model
        .config
        .lsp
        .servers
        .get(server_id)
        .and_then(|o| o.enabled)
        .unwrap_or(true)
}

/// Row accessory for a server's config-enabled state — extracted so
/// "disabled renders `DimText`" is unit-testable without a renderer.
fn lsp_server_accessory(enabled: bool) -> Accessory<'static> {
    if enabled {
        Accessory::Check
    } else {
        Accessory::DimText("disabled")
    }
}

/// "Set Language..." picker: one row per registered language, the
/// focused document's current one marked with a check.
fn render_language_picker_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &LanguagePickerState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let current = model.editor_area.focused_document().map(|doc| doc.language);
    let rows: Vec<Row> = crate::syntax::LanguageId::all()
        .map(|language| Row {
            icon: RowIcon::None,
            label: language.display_name(),
            match_indices: &[],
            detail: None,
            detail_style: None,
            accessory: if Some(language) == current {
                Accessory::Check
            } else {
                Accessory::None
            },
        })
        .collect();
    let sections = [Section {
        title: None,
        rows: &rows,
    }];

    let selected_index = state.selected_index.min(rows.len().saturating_sub(1));

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: WidthRule {
                pct: 0.0,
                min: 420.0,
                max: 420.0,
            },
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: None,
            text: "",
            placeholder: "Set Language",
            caret: None,
            selection: None,
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: None,
        docs: None,
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );
}

/// Detail text for one server row: affected languages, then its live
/// state — e.g. `"TypeScript, JavaScript · Ready"`.
fn lsp_server_detail(model: &AppModel, server_id: &str) -> String {
    let (_, state_label) = lsp_server_state_visual(model, server_id);
    let languages: Vec<&str> = crate::lsp::languages_for_server(server_id)
        .iter()
        .map(|l| l.display_name())
        .collect();
    format!("{} \u{b7} {}", languages.join(", "), state_label)
}

fn render_lsp_servers_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &LspServersState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let defs = crate::lsp::all_server_defs();
    let details: Vec<String> = defs
        .iter()
        .map(|def| lsp_server_detail(model, def.id))
        .collect();
    let rows: Vec<Row> = defs
        .iter()
        .zip(&details)
        .map(|(def, detail)| {
            let (color, _) = lsp_server_state_visual(model, def.id);
            let enabled = lsp_server_config_enabled(model, def.id);
            Row {
                icon: RowIcon::Glyph {
                    ch: '\u{25CF}',
                    color,
                },
                label: def.id,
                match_indices: &[],
                detail: Some(detail.as_str()),
                detail_style: None,
                accessory: lsp_server_accessory(enabled),
            }
        })
        .collect();
    let sections = [Section {
        title: None,
        rows: &rows,
    }];

    let selected_index = state.selected_index.min(defs.len().saturating_sub(1));

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: WidthRule {
                pct: 0.0,
                min: 420.0,
                max: 420.0,
            },
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: None,
            text: "",
            placeholder: "Language Servers",
            caret: None,
            selection: None,
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: None,
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );
}

// ============================================================================
// Go to Line / Find & Replace (Body::Fields)
// ============================================================================

/// Single-field prompt shared by Go to Line and Rename Symbol.
fn render_single_field_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    label: &'static str,
    editable: &crate::editable::EditableState<crate::editable::StringBuffer>,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let fields = [Field::labeled(label)];
    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: width_rule(SMALL_MODAL_WIDTH),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: None,
        body: Body::Fields {
            fields: &fields,
            focused: 0,
        },
        footer: None,
        hover_row: None,
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );

    let l = overlay_surface::layout(&spec, ctx.window_width, ctx.window_height, ctx.scale_factor);
    if let Some(field) = l.fields.first() {
        TextFieldRenderer::render_modal_input(
            frame,
            painter,
            editable,
            &field.input,
            ctx.line_height,
            ctx.char_width,
            ctx.colors.input_bg,
            ctx.colors.fg,
            model.theme.overlay.highlight.to_argb_u32(),
            model.theme.overlay.selection_background.to_argb_u32(),
            model.ui.cursor_visible,
            ctx.scale_factor,
        );
    }
}

// ============================================================================
// Shape-only layouts (hit-testing, caret placement)
// ============================================================================

/// Build the shape-only `OverlaySpec` (placeholder row content, real
/// counts/titles/sections/scroll) for whichever modal is active, call
/// `overlay_surface::layout()` on it, and hand both to `f`. Used by
/// `hit_test::hit_test_modal` (row/tab/scrollbar hit-testing) and
/// `view::caret` (IME caret placement) so both consume the exact geometry
/// the renderer computes without re-deriving row content.
pub(crate) fn with_modal_overlay_layout<R>(
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    f: impl FnOnce(&OverlaySpec, &OverlayLayout) -> R,
) -> Option<R> {
    use crate::model::ModalState;

    let modal = model.ui.active_modal.as_ref()?;
    match modal {
        ModalState::FileConflict(state) => Some(with_file_conflict_spec(state, |spec| {
            let layout = overlay_surface::layout(spec, window_width, window_height, scale_factor);
            f(spec, &layout)
        })),
        ModalState::CommandPalette(state) => {
            use crate::model::SearchTab;
            use crate::update::search_everywhere_sections;

            // Mirrors `render_command_palette_modal` exactly: same tab bar
            // (`search_tab_bar`), same section boundaries
            // (`search_everywhere_sections`), same per-tab selection/scroll
            // — the one-layout-two-consumers invariant this function exists
            // for (overlay-surface.md "Hit-testing").
            let (tab_labels, active_tab_idx) = search_tab_bar(state);
            let tabs = TabBar {
                tabs: &tab_labels,
                active: active_tab_idx,
            };

            let sections_spec = search_everywhere_sections(state);
            let row_groups: Vec<Vec<Row>> = sections_spec
                .iter()
                .map(|&(_, len)| placeholder_rows(len))
                .collect();
            let sections: Vec<Section> = sections_spec
                .iter()
                .zip(&row_groups)
                .map(|(&(title, _), rows)| Section { title, rows })
                .collect();
            let total: usize = sections_spec.iter().map(|&(_, len)| len).sum();

            let (selected_index, scroll, max_visible) = match state.active_tab {
                SearchTab::Commands => (
                    state.selected_index.min(total.saturating_sub(1)),
                    state.scroll_offset,
                    COMMAND_PALETTE_MAX_VISIBLE,
                ),
                SearchTab::Files => (
                    state.files.as_ref().map(|f| f.selected_index).unwrap_or(0),
                    state.files.as_ref().map(|f| f.scroll_offset).unwrap_or(0),
                    COMMAND_PALETTE_MAX_VISIBLE,
                ),
                SearchTab::All => (state.all_selected, 0, all_tab_max_visible(&sections_spec)),
                SearchTab::Symbols => (
                    state.symbols.selected_index,
                    state.symbols.scroll_offset,
                    COMMAND_PALETTE_MAX_VISIBLE,
                ),
            };

            let spec = OverlaySpec {
                tabs: Some(tabs),
                anchor: Anchor::Centered {
                    width: width_rule(PALETTE_WIDTH),
                    dim_alpha: MODAL_DIM_ALPHA,
                },
                header: Some(Header {
                    glyph: Some(PALETTE_HEADER_GLYPH),
                    text: "",
                    placeholder: "",
                    caret: Some(state.input().chars().count()),
                    selection: None,
                    scope: None,
                }),
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(selected_index),
                    scroll,
                    max_visible,
                },
                footer: Some(Footer {
                    leading: "",
                    trailing: "",
                }),
                hover_row: None,
                docs: None,
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::FileFinder(state) => {
            let rows = placeholder_rows(state.results.len());
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = list_shape_spec(
                PICKER_WIDTH,
                Some(state.input().chars().count()),
                &sections,
                state.selected_index.min(rows.len().saturating_sub(1)),
                state.scroll_offset,
                true,
            );
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::RecentFiles(state) => {
            let groups = recent_files_groups(state);
            let row_groups: Vec<Vec<Row>> = groups
                .iter()
                .map(|(_, indices)| placeholder_rows(indices.len()))
                .collect();
            let sections: Vec<Section> = groups
                .iter()
                .zip(&row_groups)
                .map(|((title, _), rows)| Section {
                    title: Some(title),
                    rows,
                })
                .collect();
            let total = state.filtered_rows.len();
            let spec = list_shape_spec(
                PICKER_WIDTH,
                Some(state.input().chars().count()),
                &sections,
                state.selected_index.min(total.saturating_sub(1)),
                state.scroll_offset,
                true,
            );
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::Settings(state) => Some(with_settings_spec(model, state, |spec| {
            let layout = overlay_surface::layout(spec, window_width, window_height, scale_factor);
            f(spec, &layout)
        })),
        ModalState::ThemePicker(state) => {
            let groups = theme_picker_groups(&state.themes);
            let row_groups: Vec<Vec<Row>> = groups
                .iter()
                .map(|(_, range)| placeholder_rows(range.len()))
                .collect();
            let sections: Vec<Section> = groups
                .iter()
                .zip(&row_groups)
                .map(|((title, _), rows)| Section {
                    title: Some(title),
                    rows,
                })
                .collect();
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Centered {
                    width: WidthRule {
                        pct: 0.0,
                        min: 400.0,
                        max: 400.0,
                    },
                    dim_alpha: MODAL_DIM_ALPHA,
                },
                header: Some(Header {
                    glyph: None,
                    text: "",
                    placeholder: "",
                    caret: None,
                    selection: None,
                    scope: None,
                }),
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(
                        state
                            .selected_index
                            .min(state.themes.len().saturating_sub(1)),
                    ),
                    scroll: state.scroll_offset,
                    max_visible: COMMAND_PALETTE_MAX_VISIBLE,
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::LspServers(state) => {
            let rows = placeholder_rows(crate::lsp::all_server_defs().len());
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = list_shape_spec(
                (0.0, 420.0, 420.0),
                None,
                &sections,
                state.selected_index.min(rows.len().saturating_sub(1)),
                state.scroll_offset,
                false,
            );
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::LanguagePicker(state) => {
            let rows = placeholder_rows(crate::syntax::LanguageId::all().count());
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = list_shape_spec(
                (0.0, 420.0, 420.0),
                None,
                &sections,
                state.selected_index.min(rows.len().saturating_sub(1)),
                state.scroll_offset,
                false,
            );
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::GotoLine(_) | ModalState::RenameSymbol(_) => {
            let fields = [Field::labeled("")];
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Centered {
                    width: width_rule(SMALL_MODAL_WIDTH),
                    dim_alpha: MODAL_DIM_ALPHA,
                },
                header: None,
                body: Body::Fields {
                    fields: &fields,
                    focused: 0,
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
    }
}

/// One conflict specification for painting, row hit testing and keyboard order.
fn with_file_conflict_spec<R>(
    state: &crate::model::FileConflictState,
    f: impl FnOnce(&OverlaySpec) -> R,
) -> R {
    let rows: Vec<_> = state
        .actions()
        .iter()
        .map(|&action| Row {
            icon: RowIcon::None,
            label: state.label(action),
            match_indices: &[],
            detail: None,
            detail_style: None,
            accessory: Accessory::None,
        })
        .collect();
    let title = match &state.observed.content {
        crate::model::DiskContent::Text(_) => "File changed outside Token",
        crate::model::DiskContent::Missing => "File deleted outside Token",
        crate::model::DiskContent::Unavailable(_) => {
            "File could not be checked — local version retained"
        }
    };
    let sections = [Section {
        title: Some(title),
        rows: &rows,
    }];
    let path = state.path.to_string_lossy();
    let mut spec = list_shape_spec(
        (0.6, 420.0, 640.0),
        None,
        &sections,
        state.selected_index,
        0,
        true,
    );
    if let Some(header) = &mut spec.header {
        header.text = &path;
    }
    spec.footer = Some(Footer {
        leading: "Esc: keep editing",
        trailing: "Enter: choose",
    });
    f(&spec)
}

/// Placeholder rows for a shape-only spec — layout only depends on row
/// count/height, not content.
fn placeholder_rows(count: usize) -> Vec<Row<'static>> {
    (0..count)
        .map(|_| Row {
            icon: RowIcon::None,
            label: "",
            match_indices: &[],
            detail: None,
            detail_style: None,
            accessory: Accessory::None,
        })
        .collect()
}

/// Shared shape for the input-as-header list contexts (palette, file
/// finder, recent files): same width rule shape, header-with-caret,
/// optional footer presence mirrored from the real render (footer
/// content doesn't matter for hit-testing/caret, only its Some/None-ness
/// affects panel height).
fn list_shape_spec<'a>(
    width: (f32, f32, f32),
    input_len: Option<usize>,
    sections: &'a [Section<'a>],
    selected_index: usize,
    scroll: usize,
    has_footer: bool,
) -> OverlaySpec<'a> {
    OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: width_rule(width),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: None,
            text: "",
            placeholder: "",
            caret: input_len,
            selection: None,
            scope: None,
        }),
        body: Body::List {
            sections,
            selected: FlatIndex(selected_index),
            scroll,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: has_footer.then_some(Footer {
            leading: "",
            trailing: "",
        }),
        hover_row: None,
        docs: None,
    }
}

/// The header text-input rect (inside the panel, minus header padding) for
/// whichever modal has an input-as-header, used by `view::caret` to place
/// the IME caret.
pub(crate) fn modal_header_input_rect(
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    char_width: f32,
) -> Option<WidgetRect> {
    with_modal_overlay_layout(
        model,
        window_width,
        window_height,
        scale_factor,
        |spec, l| {
            let header = l.header?;
            let pad = overlay_surface::header_pad_x(scale_factor);
            // Mirrors `render_header`'s text origin exactly: the header pad,
            // then the glyph's advance plus half a pad when one is drawn. The
            // returned rect *is* the text box — callers must not inset it
            // again, or the IME caret lands off the painted one.
            let glyph_w = spec
                .header
                .as_ref()
                .and_then(|h| h.glyph)
                .map_or(0, |_| char_width.ceil() as usize + pad / 2);
            let x = header.x + pad + glyph_w;
            Some(WidgetRect {
                x,
                y: header.y,
                w: (header.x + header.w).saturating_sub(x + pad),
                h: header.h,
            })
        },
    )
    .flatten()
}

pub(crate) fn modal_field_input_rect(
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    field_index: usize,
) -> Option<WidgetRect> {
    with_modal_overlay_layout(model, window_width, window_height, scale_factor, |_, l| {
        l.fields.get(field_index).map(|f| f.input)
    })
    .flatten()
}

// ============================================================================
// Top-level modal / drop-overlay rendering
// ============================================================================

/// Render the active modal overlay.
pub fn render_modals(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    overlay_mask_cache: &mut RoundedRectMaskCache,
) {
    use crate::model::ModalState;

    let Some(ref modal) = model.ui.active_modal else {
        return;
    };

    let ctx = ModalRenderCtx {
        window_width,
        window_height,
        line_height: painter.line_height(),
        char_width: painter.char_width(),
        colors: ModalColors::from_model(model),
        scale_factor: model.metrics.scale_factor,
    };

    match modal {
        ModalState::FileConflict(state) => with_file_conflict_spec(state, |spec| {
            overlay_surface::render(
                frame,
                painter,
                overlay_mask_cache,
                &model.theme,
                spec,
                window_width,
                window_height,
                ctx.scale_factor,
                false,
            );
        }),
        ModalState::ThemePicker(state) => {
            render_theme_picker_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::Settings(state) => with_settings_spec(model, state, |spec| {
            overlay_surface::render(
                frame,
                painter,
                overlay_mask_cache,
                &model.theme,
                spec,
                window_width,
                window_height,
                ctx.scale_factor,
                model.ui.cursor_visible,
            );
        }),
        ModalState::CommandPalette(state) => {
            render_command_palette_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::GotoLine(state) => render_single_field_modal(
            frame,
            painter,
            model,
            "Line:",
            &state.editable,
            &ctx,
            overlay_mask_cache,
        ),
        ModalState::RenameSymbol(state) => render_single_field_modal(
            frame,
            painter,
            model,
            "Rename to:",
            &state.editable,
            &ctx,
            overlay_mask_cache,
        ),
        ModalState::FileFinder(state) => {
            render_file_finder_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::RecentFiles(state) => {
            render_recent_files_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::LspServers(state) => {
            render_lsp_servers_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::LanguagePicker(state) => {
            render_language_picker_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
    }
}

/// Render the file drop overlay when files are being dragged over the window.
pub fn render_drop_overlay(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let text = model.ui.drop_state.display_text();

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: width_rule(SMALL_MODAL_WIDTH),
            // The drop overlay dims darker than regular modals (0x80 vs
            // 0x66) — preserved from the pre-migration behavior.
            dim_alpha: 0x80,
        },
        header: None,
        body: Body::Zones(overlay_surface::Zones {
            center_text: true,
            banner: None,
            code: None,
            text: Some(&text),
            ..Default::default()
        }),
        footer: None,
        hover_row: None,
        docs: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme,
        &spec,
        window_width,
        window_height,
        model.metrics.scale_factor,
        model.ui.cursor_visible,
    );
}

// ============================================================================
// Cursor-anchored popups (overlay-surface.md Phase 5)
// ============================================================================

/// The (physical-px) anchor rect for a cursor-anchored popup: the text
/// caret's top-left corner and line height, per overlay-surface.md Phase 5
/// ("pixel rect from view geometry ... `active_text_input_rect`"). Carrying
/// the height lets `Anchor::Cursor` flip above the caret's own *top* edge
/// instead of just its bottom, so a flipped popup never covers the line
/// being edited. `None` when there's no live text caret to anchor to (no
/// modal is open, so this reduces to the editor caret).
fn cursor_overlay_anchor(model: &AppModel) -> Option<(usize, usize, usize)> {
    let rect = super::caret::active_text_input_rect(model, model.char_width, model.line_height)?;
    Some((rect.x, rect.y, rect.h))
}

/// Build the real Completion popup's rows from `UiState::completion_menu`,
/// in filtered/sorted order.
fn completion_rows(state: &crate::completion::CompletionMenuState) -> Vec<Row<'_>> {
    state
        .filtered
        .iter()
        .filter_map(|(_, idx, indices)| state.items.get(*idx).map(|item| (item, indices)))
        .map(|(item, indices)| Row {
            icon: RowIcon::KindBadge(item.kind),
            label: &item.label,
            match_indices: indices,
            detail: item.detail.as_deref(),
            // A server's `detail` is a type signature: chip it. Offline
            // sources ("snippet") keep the dim meta text.
            detail_style: (item.source == crate::completion::menu::MenuSourceId::Lsp)
                .then_some(crate::model::SpanStyle::Code),
            accessory: Accessory::None,
        })
        .collect()
}

/// Dummy rows exercising the Completion list shell (kind badges, dim
/// signature accessory) — manual-testing content only; the real completion
/// source is [autocomplete.md](autocomplete.md) Phase 1.
fn debug_completion_rows() -> Vec<Row<'static>> {
    const ITEMS: &[(MenuItemKind, &str, &str)] = &[
        (MenuItemKind::Method, "to_string", "fn() -> String"),
        (MenuItemKind::Method, "trim", "fn() -> &str"),
        (MenuItemKind::Variable, "value", "i32"),
        (MenuItemKind::Type, "String", "struct"),
        (MenuItemKind::Keyword, "match", "keyword"),
        (MenuItemKind::Field, "len", "usize"),
        (MenuItemKind::Module, "std::fmt", "module"),
        (MenuItemKind::Constant, "MAX", "usize"),
    ];
    ITEMS
        .iter()
        .map(|&(kind, label, detail)| Row {
            icon: RowIcon::KindBadge(kind),
            label,
            match_indices: &[],
            detail: Some(detail),
            detail_style: None,
            accessory: Accessory::None,
        })
        .collect()
}

/// Row count of the debug Completion demo — `runtime::input`'s cursor-
/// overlay key branch needs this to wrap Up/Down without duplicating (or
/// importing) the row content itself.
pub fn debug_completion_row_count() -> usize {
    debug_completion_rows().len()
}

/// Maps an LSP diagnostic severity onto the overlay surface's severity
/// palette. `None` (server left severity to the client) and `ERROR` both
/// resolve to `Error` — matches `model::decorations::diagnostic_mark`'s
/// same "more visible when unstated" rule.
fn diagnostic_severity_to_overlay(
    severity: Option<lsp_types::DiagnosticSeverity>,
) -> overlay_surface::Severity {
    use lsp_types::DiagnosticSeverity as S;
    match severity {
        Some(S::WARNING) => overlay_surface::Severity::Warning,
        Some(S::INFORMATION) => overlay_surface::Severity::Info,
        Some(S::HINT) => overlay_surface::Severity::Hint,
        _ => overlay_surface::Severity::Error,
    }
}

/// Flattens every `relatedInformation` entry across `diagnostics` into
/// "note: <message> (<file>:<line>)" lines — rust-analyzer's "first borrow
/// occurs here" is half the value of the error (lsp-integration.md Phase
/// 4). `None` when nothing has related info.
pub fn related_information_text(diagnostics: &[&lsp_types::Diagnostic]) -> Option<String> {
    let lines: Vec<String> = diagnostics
        .iter()
        .flat_map(|d| d.related_information.iter().flatten())
        .map(|info| {
            let file = crate::lsp::uri_to_path(&info.location.uri)
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| info.location.uri.as_str().to_owned());
            format!(
                "note: {} ({file}:{})",
                info.message,
                info.location.range.start.line + 1
            )
        })
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Dummy content exercising the hover `Zones` card (banner/code/text,
/// severity conventions) — manual-testing content only; the real hover
/// source is [lsp-integration.md](lsp-integration.md) Phase 4.
fn debug_hover_zones() -> overlay_surface::Zones<'static> {
    overlay_surface::Zones {
        banner: Some((
            overlay_surface::Severity::Warning,
            "unused variable: `x`",
            "rustc",
        )),
        code: Some("fn foo(x: i32) -> i32"),
        text: Some("This value is never read.\nConsider prefixing with an underscore: `_x`."),
        ..Default::default()
    }
}

/// Build the cursor-overlay `OverlaySpec` for whichever kind is open and
/// hand it to `f`. The spec borrows per-branch temporaries (row buffers,
/// section arrays), which is why it is delivered through a closure rather
/// than returned.
///
/// Laying the spec out is the caller's job: rendering does it inside
/// `overlay_surface::render` (measuring through the glyph cache) and
/// hit-testing does it with the same measure, so the two still cannot
/// disagree — but a caller that only needs the spec no longer pays for a
/// layout it throws away.
pub fn with_cursor_overlay_spec<R>(
    model: &AppModel,
    f: impl FnOnce(&OverlaySpec) -> R,
) -> Option<R> {
    let state = model.ui.cursor_overlay?;

    if state.kind == crate::model::CursorOverlayKind::Completion {
        let menu = model.ui.completion_menu.as_ref()?;
        let rect = super::caret::editor_text_rect_at(
            model,
            menu.query_start.line,
            menu.query_start.column,
            model.char_width,
            model.line_height,
        )?;
        let rows = completion_rows(menu);
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let docs = menu.selected_documentation(state.selected);
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: rect.x,
                y: rect.y,
                h: rect.h,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 240.0,
                    max: 320.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(state.selected.min(rows.len().saturating_sub(1))),
                scroll: state.scroll,
                max_visible: overlay_surface::MAX_VISIBLE_COMPLETION,
            },
            footer: None,
            hover_row: state.hover_row.map(FlatIndex),
            docs: docs.map(|text| overlay_surface::Documentation {
                text,
                scroll: state.docs_scroll,
                expanded: state.docs_expanded,
            }),
        };
        return Some(f(&spec));
    }

    // The context menu carries its own open-time anchor (`ui.context_menu.
    // anchor` — the click point, or the caret rect for Shift+F10), never
    // derived from the live caret/hover state the block below reads —
    // early-return like Completion, before that block's `focused_editor`/
    // `focused_document` requirements (a right-click on a file-tree item
    // has neither).
    if state.kind == crate::model::CursorOverlayKind::ContextMenu {
        let menu = model.ui.context_menu.as_ref()?;
        let chip_steps = context_menu_chip_steps(&menu.items);
        let rows = context_menu_rows(&menu.items, &chip_steps);
        let sections = context_menu_sections(&menu.items, &rows);
        let (x, y, h) = menu.anchor;
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Menu {
                x,
                y,
                h,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 200.0,
                    max: 520.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(state.selected.min(rows.len().saturating_sub(1))),
                scroll: state.scroll,
                // V1 menus top out at ~8 items including separators — no
                // scroll behavior needed (context-menu.md "Body::List and
                // row anatomy").
                max_visible: usize::MAX,
            },
            footer: None,
            hover_row: state.hover_row.map(FlatIndex),
            docs: None,
        };
        return Some(f(&spec));
    }

    // A mouse-dwell hover anchors to the hovered text cell instead of the
    // caret (`HoverCardState::anchor`, set only for `ShowHoverAt` replies);
    // every other kind — including a keyboard-invoked hover — keeps
    // anchoring to the live caret rect.
    let dwell_anchor = (state.kind == crate::model::CursorOverlayKind::Hover)
        .then(|| model.ui.hover_card.as_ref().and_then(|c| c.anchor))
        .flatten();
    let (x, y, h) = match dwell_anchor {
        Some((line, col)) => {
            let rect = super::caret::editor_text_rect_at(
                model,
                line,
                col,
                model.char_width,
                model.line_height,
            )?;
            (rect.x, rect.y, rect.h)
        }
        None => cursor_overlay_anchor(model)?,
    };

    match state.kind {
        crate::model::CursorOverlayKind::Completion => unreachable!("handled above"),
        crate::model::CursorOverlayKind::ContextMenu => unreachable!("handled above"),
        crate::model::CursorOverlayKind::DebugCompletion => {
            let rows = debug_completion_rows();
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x,
                    y,
                    h,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: 240.0,
                        max: 320.0,
                    },
                },
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(state.selected.min(rows.len().saturating_sub(1))),
                    scroll: state.scroll,
                    max_visible: overlay_surface::MAX_VISIBLE_COMPLETION,
                },
                footer: None,
                hover_row: state.hover_row.map(FlatIndex),
                docs: None,
            };
            Some(f(&spec))
        }
        crate::model::CursorOverlayKind::DebugHover => {
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x,
                    y,
                    h,
                    // Hover is "Cursor, above-preferred" (Contexts table),
                    // unlike Completion.
                    prefer_below: false,
                    width: WidthRule {
                        pct: 0.42,
                        min: 360.0,
                        max: 560.0,
                    },
                },
                header: None,
                body: Body::Zones(debug_hover_zones()),
                footer: None,
                hover_row: None,
                docs: None,
            };
            Some(f(&spec))
        }
        crate::model::CursorOverlayKind::Hover => {
            let doc = model.try_document()?;
            // Diagnostics-under-hover reads at the hovered cell for a
            // dwell card, or the caret for a keyboard-invoked one — same
            // position `HoverResolved` resolved content against.
            let cursor = dwell_anchor
                .map(|(line, col)| crate::model::editor::Position::new(line, col))
                .unwrap_or_else(|| model.editor().active_cursor().to_position());
            let diagnostics = crate::model::decorations::diagnostics_at_position(doc, cursor);
            // Diagnostic messages quote identifiers in backticks: chip
            // them, but treat nothing else as markdown.
            let banner_text = diagnostics
                .first()
                .map(|d| crate::lsp::markdown::code_spans_only(&d.message));
            let banner = diagnostics
                .first()
                .zip(banner_text.as_ref())
                .map(|(d, msg)| {
                    (
                        diagnostic_severity_to_overlay(d.severity),
                        msg.text.as_str(),
                        d.source.as_deref().unwrap_or(""),
                    )
                });
            let banner_spans: &[crate::model::Span] =
                banner_text.as_ref().map_or(&[], |t| t.spans.as_slice());
            let hover_text = model
                .ui
                .hover_card
                .as_ref()
                .and_then(|s| s.content.as_ref());
            let related = related_information_text(&diagnostics);
            // A hover that opens with a code fence (rust-analyzer's
            // signature block) puts that block in the code zone; the
            // prose after it is the text zone.
            let (code, prose) = hover_text
                .map(|h| h.split_leading_code())
                .unwrap_or((None, crate::model::StyledText::default()));
            let mut text = prose;
            if let Some(r) = related.as_deref() {
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text.push_str(r);
            }
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x,
                    y,
                    h,
                    // "Cursor, above-preferred" per the Contexts table.
                    prefer_below: false,
                    width: WidthRule {
                        pct: 0.42,
                        min: 360.0,
                        max: 560.0,
                    },
                },
                header: None,
                body: Body::Zones(Zones {
                    banner,
                    banner_spans,
                    code: code.as_ref().map(|c| c.text.as_str()),
                    code_spans: code.as_ref().map_or(&[], |c| c.spans.as_slice()),
                    text: (!text.is_empty()).then_some(text.text.as_str()),
                    text_spans: &text.spans,
                    ..Default::default()
                }),
                footer: None,
                hover_row: None,
                docs: None,
            };
            Some(f(&spec))
        }
        crate::model::CursorOverlayKind::CodeActions => {
            let items = model.ui.code_action_list.as_deref().unwrap_or(&[]);
            let rows: Vec<Row> = items
                .iter()
                .map(|item| Row {
                    icon: RowIcon::None,
                    label: item.title.as_str(),
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: item
                        .kind
                        .as_deref()
                        .map_or(Accessory::None, Accessory::DimText),
                })
                .collect();
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x,
                    y,
                    h,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: 320.0,
                        max: 520.0,
                    },
                },
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(state.selected.min(rows.len().saturating_sub(1))),
                    scroll: state.scroll,
                    max_visible: overlay_surface::MAX_VISIBLE_COMPLETION,
                },
                footer: None,
                hover_row: state.hover_row.map(FlatIndex),
                docs: None,
            };
            Some(f(&spec))
        }
        crate::model::CursorOverlayKind::References => {
            let items = model.ui.reference_list.as_deref().unwrap_or(&[]);
            let (details, accessories) = reference_row_text(model, items);
            let rows: Vec<Row> = items
                .iter()
                .zip(details.iter())
                .zip(accessories.iter())
                .map(|((item, detail), accessory)| Row {
                    icon: RowIcon::None,
                    label: if item.preview.is_empty() {
                        detail.as_str()
                    } else {
                        item.preview.as_str()
                    },
                    match_indices: &[],
                    detail: Some(detail.as_str()),
                    detail_style: None,
                    accessory: Accessory::DimText(accessory.as_str()),
                })
                .collect();
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x,
                    y,
                    h,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: 320.0,
                        max: 520.0,
                    },
                },
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(state.selected.min(rows.len().saturating_sub(1))),
                    scroll: state.scroll,
                    max_visible: overlay_surface::MAX_VISIBLE_COMPLETION,
                },
                footer: None,
                hover_row: state.hover_row.map(FlatIndex),
                docs: None,
            };
            Some(f(&spec))
        }
    }
}

/// Per-row keycap chips for `context_menu_rows` — `None` for an unbound
/// item, `Some(steps)` otherwise; owned by the caller so `Row::accessory`'s
/// `Keycaps(&'a [Vec<Chip>])` borrow outlives the render call, the same
/// "owned buffer computed first, rows borrow into it" split
/// `reference_row_text` uses.
fn context_menu_chip_steps(
    items: &[crate::context_menu::MenuItem],
) -> Vec<Option<Vec<Vec<overlay_surface::Chip>>>> {
    crate::context_menu::selectable_items(items)
        .map(|item| {
            item.shortcut_hint
                .as_deref()
                .map(overlay_surface::binding_chips)
        })
        .collect()
}

/// One `Row` per non-separator `MenuItem`, in `context_menu::
/// selectable_items` order — the same "flat rows, addressed by position"
/// space `FlatIndex`, keyboard nav, and `ContextMenuMsg::ActivateItem`'s
/// `index` all share (context-menu.md "Separators"). Keycap chips reuse
/// the palette's own >4-chip -> `DimText` fallback (`chip_count`); an
/// unbound item gets `Accessory::None` (no dim filler — "Overlay Context"
/// > Accessory).
fn context_menu_rows<'a>(
    items: &'a [crate::context_menu::MenuItem],
    chip_steps: &'a [Option<Vec<Vec<overlay_surface::Chip>>>],
) -> Vec<Row<'a>> {
    crate::context_menu::selectable_items(items)
        .zip(chip_steps)
        .map(|(item, steps)| {
            let accessory = match steps {
                None => Accessory::None,
                Some(steps) if overlay_surface::chip_count(steps) > 4 => {
                    Accessory::DimText(item.shortcut_hint.as_deref().unwrap_or(""))
                }
                Some(steps) => Accessory::Keycaps(steps),
            };
            Row {
                icon: RowIcon::None,
                label: &item.label,
                match_indices: &[],
                detail: None,
                detail_style: None,
                accessory,
            }
        })
        .collect()
}

/// Maps a flat `MenuItem` list (including separators) onto `OverlaySurface`
/// `Section` boundaries: each run of non-separator items between
/// separators becomes its own untitled `Section` (context-menu.md
/// "Separators" — a centered hairline row). `rows` must be
/// `context_menu_rows(items)` — the non-separator subset, one-to-one with
/// `context_menu::selectable_items(items)` in the same order.
fn context_menu_sections<'a>(
    items: &[crate::context_menu::MenuItem],
    rows: &'a [Row<'a>],
) -> Vec<Section<'a>> {
    let mut sections = Vec::new();
    let mut run_start = 0;
    let mut row_index = 0;
    for item in items {
        if item.is_separator {
            if row_index > run_start {
                sections.push(Section {
                    title: None,
                    rows: &rows[run_start..row_index],
                });
            }
            run_start = row_index;
        } else {
            row_index += 1;
        }
    }
    if row_index > run_start {
        sections.push(Section {
            title: None,
            rows: &rows[run_start..row_index],
        });
    }
    sections
}

/// Per-row `detail`/`accessory` text for the References/multi-def popup —
/// `file name  workspace-relative-dir` and `line:col` (1-based, matching
/// the status bar's display convention), storage owned by the caller so
/// `Row`'s borrowed fields outlive this call.
fn reference_row_text(
    model: &AppModel,
    items: &[crate::update::navigation::LocationItem],
) -> (Vec<String>, Vec<String>) {
    let workspace_root = model.workspace_root();
    let details = items
        .iter()
        .map(|item| {
            let name = item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| item.path.display().to_string());
            let dir = item.path.parent().map(|p| {
                workspace_root
                    .and_then(|root| p.strip_prefix(root).ok())
                    .unwrap_or(p)
                    .display()
                    .to_string()
            });
            match dir.filter(|d| !d.is_empty()) {
                Some(dir) => format!("{name}  {dir}"),
                None => name,
            }
        })
        .collect();
    let accessories = items
        .iter()
        .map(|item| {
            let (line, col) = item.display_position(model);
            format!("{}:{}", line + 1, col + 1)
        })
        .collect();
    (details, accessories)
}

/// Signature help's caret-anchored float, a sibling of
/// `with_cursor_overlay_spec` rather than a `CursorOverlayKind`: it never
/// routes keys and may show together with the completion menu (menu
/// below the caret, this above). Inert to hit-testing.
pub fn with_signature_help_spec<R>(
    model: &AppModel,
    f: impl FnOnce(&OverlaySpec) -> R,
) -> Option<R> {
    // A completion's documentation or explicit quick docs owns the reading
    // surface. Retain signature state so it can return when the menu closes.
    if model.ui.has_modal() || model.ui.cursor_overlay.is_some() {
        return None;
    }
    let help = model.ui.signature_help.as_ref()?;
    let sig = help.signatures.get(help.active)?;
    let (x, y, h) = cursor_overlay_anchor(model)?;
    // The active parameter is an `Accent` run in the signature; the
    // parameter doc keeps its markdown spans; the "(n of m)" counter is dim.
    let mut code = crate::model::StyledText::plain(sig.label.clone());
    if let Some((start, end)) = sig.active_parameter_range {
        code.style_chars(start, end, crate::model::SpanStyle::Accent);
    }
    // Parameter doc first (it's what the caret is on), then the
    // signature's own doc, then the counter.
    let mut text = crate::model::StyledText::default();
    for part in [sig.parameter_doc.as_ref(), sig.doc.as_ref()]
        .into_iter()
        .flatten()
    {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.extend(part);
    }
    if help.signatures.len() > 1 {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_styled(
            &format!("({} of {})", help.active + 1, help.signatures.len()),
            crate::model::SpanStyle::Dim,
        );
    }
    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Cursor {
            x,
            y,
            h,
            prefer_below: false,
            width: WidthRule {
                pct: 0.0,
                min: 280.0,
                max: 480.0,
            },
        },
        header: None,
        body: Body::Zones(Zones {
            banner: None,
            banner_spans: &[],
            code: Some(code.text.as_str()),
            code_spans: &code.spans,
            text: (!text.is_empty()).then_some(text.text.as_str()),
            text_spans: &text.spans,
            ..Default::default()
        }),
        footer: None,
        hover_row: None,
        docs: None,
    };
    Some(f(&spec))
}

/// Render the active cursor-anchored popup(s): signature help first so
/// a simultaneously open completion menu paints over it if they ever
/// collide.
pub fn render_cursor_overlay(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let scale_factor = model.metrics.scale_factor;
    with_signature_help_spec(model, |spec| {
        overlay_surface::render(
            frame,
            painter,
            mask_cache,
            &model.theme,
            spec,
            window_width,
            window_height,
            scale_factor,
            model.ui.cursor_visible,
        );
    });
    with_cursor_overlay_spec(model, |spec| {
        overlay_surface::render(
            frame,
            painter,
            mask_cache,
            &model.theme,
            spec,
            window_width,
            window_height,
            scale_factor,
            model.ui.cursor_visible,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{ModalMsg, UiMsg};
    use crate::model::ModalId;

    #[test]
    fn settings_keymap_tabs_keycaps_and_capture_share_narrow_geometry() {
        use crate::settings::keymap::{Capture, SettingsTab};
        let model = AppModel::new(800, 600, 1.0);
        let mut state = crate::settings::SettingsState {
            tab: SettingsTab::Keymap,
            ..Default::default()
        };
        state.keymap.snapshot =
            Some(crate::keymap::preferences::KeymapSnapshot::parse(None).unwrap());
        state.refresh_entries();
        with_settings_spec(&model, &state, |spec| {
            assert_eq!(
                spec.tabs.as_ref().unwrap().active,
                crate::settings::categories().len() - 1
            );
            let Body::List { sections, .. } = &spec.body else {
                panic!("list")
            };
            assert!(sections
                .iter()
                .flat_map(|s| s.rows)
                .any(|r| matches!(r.accessory, Accessory::Keycaps(_))));
            assert_eq!(
                sections
                    .iter()
                    .flat_map(|s| s.rows)
                    .map(|r| r.label)
                    .collect::<Vec<_>>(),
                state
                    .filtered_rows()
                    .map(|(label, _)| label)
                    .collect::<Vec<_>>()
            );
            for width in [360, 800] {
                let layout = overlay_surface::layout(spec, width, 600, 1.0);
                assert!(layout.panel.x + layout.panel.w <= width);
                for (index, rect) in layout.tab_rects.iter().enumerate() {
                    assert_eq!(
                        overlay_surface::hit_test(
                            spec,
                            &layout,
                            rect.x + rect.w / 2,
                            rect.y + rect.h / 2
                        ),
                        overlay_surface::OverlayHit::Tab(index)
                    );
                }
            }
        });
        state.keymap.capture = Some(Capture {
            original: None,
            command: crate::keymap::Command::SaveFile,
            strokes: crate::keymap::preferences::parse_sequence("ctrl+k ctrl+s").unwrap(),
            literal_next: false,
        });
        state.refresh_entries();
        with_settings_spec(&model, &state, |spec| {
            assert!(spec.header.as_ref().unwrap().caret.is_none());
            let recorded = state
                .keymap
                .capture
                .as_ref()
                .unwrap()
                .strokes
                .iter()
                .map(crate::keymap::Keystroke::display_string)
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(spec.header.as_ref().unwrap().text, recorded);
            let layout = overlay_surface::layout(spec, 360, 600, 1.0);
            // Find all three shared chip hit targets; no feature-local rectangles.
            let mut choices = std::collections::BTreeSet::new();
            for y in layout.panel.y..layout.panel.y + layout.panel.h {
                for x in layout.panel.x..layout.panel.x + layout.panel.w {
                    if let overlay_surface::OverlayHit::Choice { choice, .. } =
                        overlay_surface::hit_test(spec, &layout, x, y)
                    {
                        choices.insert(choice);
                    }
                }
            }
            assert_eq!(choices, [0, 1, 2].into_iter().collect());
        });
    }

    #[test]
    fn lsp_settings_status_updates_redraw_open_modal_without_reordering_rows() {
        use crate::lsp::{LspServerId, ServerState};
        use crate::messages::{LspMsg, Msg};
        let mut model = AppModel::new(1000, 800, 1.0);
        update(&mut model, Msg::Ui(UiMsg::ToggleModal(ModalId::Settings)));
        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("rust-analyzer".into()))),
        );
        let Some(crate::model::ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!("settings");
        };
        let before: Vec<_> = state
            .filtered_rows()
            .map(|(label, _)| label.to_owned())
            .collect();
        let status_row = before
            .iter()
            .position(|label| label == "rust-analyzer status")
            .unwrap();
        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::ActivateRow(status_row))),
        );
        for (state, expected) in [
            (ServerState::Starting, "Starting"),
            (ServerState::Indexing, "Indexing"),
            (ServerState::Ready, "Ready"),
            (ServerState::Restarting { attempt: 2 }, "Restarting (2)"),
            (ServerState::Failed, "Failed"),
            (ServerState::Missing, "Missing"),
            (ServerState::ShuttingDown, "Shutting down"),
        ] {
            let cmd = update(
                &mut model,
                Msg::Lsp(LspMsg::ServerStateChanged {
                    server_id: LspServerId::from("rust-analyzer"),
                    root: "/ws".into(),
                    state,
                }),
            )
            .unwrap();
            assert!(matches!(cmd.damage(), crate::commands::Damage::Full));
            let Some(crate::model::ModalState::Settings(settings)) = &model.ui.active_modal else {
                panic!("settings");
            };
            assert_eq!(settings.selected_index(), status_row);
            assert_eq!(
                settings
                    .filtered_rows()
                    .map(|(label, _)| label.to_owned())
                    .collect::<Vec<_>>(),
                before
            );
            with_settings_spec(&model, settings, |spec| {
                let Body::List { sections, .. } = &spec.body else {
                    panic!("list");
                };
                let row = sections
                    .iter()
                    .flat_map(|s| s.rows)
                    .find(|r| r.label == "rust-analyzer status")
                    .unwrap();
                assert!(matches!(row.accessory, Accessory::DimText(value) if value == expected));
            });
        }
    }

    #[test]
    fn lsp_settings_command_values_use_clipped_detail_not_unbounded_accessory() {
        let mut model = AppModel::new(1000, 800, 1.0);
        let command = format!("/{}server", "long-directory/".repeat(100));
        model
            .config
            .lsp
            .servers
            .entry("rust-analyzer".into())
            .or_default()
            .command = Some(command.clone());
        let state = crate::settings::SettingsState::default();
        with_settings_spec(&model, &state, |spec| {
            let Body::List { sections, .. } = &spec.body else {
                panic!("list");
            };
            let row = sections
                .iter()
                .flat_map(|s| s.rows)
                .find(|r| r.label == "lsp.servers.rust-analyzer.command")
                .unwrap();
            assert!(row.detail.is_none());
            assert!(
                matches!(row.accessory, Accessory::SettingValue { text, action: None } if text == command)
            );
        });
    }

    #[test]
    fn settings_view_and_input_share_filtered_section_order() {
        let mut model = AppModel::new(1200, 800, 1.0);
        update(
            &mut model,
            crate::messages::Msg::Ui(UiMsg::ToggleModal(ModalId::Settings)),
        );
        update(
            &mut model,
            crate::messages::Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("bar".into()))),
        );
        let Some(crate::model::ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!("settings");
        };
        let expected: Vec<_> = state.filtered_rows().collect();
        with_settings_spec(&model, state, |spec| {
            let Body::List { sections, .. } = &spec.body else {
                panic!("list");
            };
            assert!(sections.len() >= 2, "fixture must filter multiple sections");
            let actual: Vec<_> = sections
                .iter()
                .flat_map(|s| s.rows.iter().map(move |r| (r.label, s.title.unwrap())))
                .collect();
            assert_eq!(actual, expected);
        });
        let font_row = expected
            .iter()
            .position(|(label, _)| *label == "Status bar font")
            .unwrap();
        update(
            &mut model,
            crate::messages::Msg::Ui(UiMsg::Modal(ModalMsg::ChooseSetting {
                row: font_row,
                choice: 2,
            })),
        );
        assert_eq!(model.config.status_bar_font_size, 13.0);
    }

    #[test]
    fn shortcut_hints_in_palette_rows_follow_rebinding_and_unbinding() {
        use crate::keymap::{
            Command, Condition, KeyCode, Keybinding, Keymap, Keystroke, Modifiers,
        };
        let mut model = AppModel::new(800, 600, 1.0);
        let chord = [
            Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL),
            Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL),
        ];
        model.ui.keymap =
            Keymap::with_bindings(vec![Keybinding::chord(chord.to_vec(), Command::SaveFile)
                .when_single(Condition::EditorFocused)]);
        let state = crate::model::CommandPaletteState::default();
        model
            .ui
            .open_modal(crate::model::ModalState::CommandPalette(state.clone()));
        let index = state
            .matches
            .iter()
            .position(|m| m.def.id == crate::commands::CommandId::SaveFile)
            .unwrap();
        let accessories = command_accessories(&model, &state.matches);
        let rows = command_rows(&state.matches, &accessories, 0);
        assert!(matches!(rows[index].accessory, Accessory::Keycaps(_)));
        match &accessories[index] {
            PaletteAccessory::Keycaps(steps) => assert_eq!(steps.len(), 2),
            _ => panic!("A two-step chord should produce keycaps"),
        }
        model.ui.keymap = Keymap::new();
        let accessories = command_accessories(&model, &state.matches);
        assert!(matches!(accessories[index], PaletteAccessory::None));
    }

    use crate::update::update;

    use crate::view::hit_test::{hit_test_modal, HitTarget, Point};

    /// A Search Everywhere modal open on the Commands tab with a non-empty
    /// query — real rows exist, so row-rect assertions below aren't testing
    /// against an empty list. Window size/scale mirror the drift reported
    /// against overlay-surface.md's Hit-testing invariant.
    fn opened_palette_model() -> AppModel {
        let mut model = AppModel::new(1200, 800, 1.0);
        update(
            &mut model,
            crate::messages::Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)),
        );
        update(
            &mut model,
            crate::messages::Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('o'))),
        );
        model
    }

    #[test]
    fn all_tab_max_visible_counts_headers_as_display_slots() {
        assert_eq!(
            all_tab_max_visible(&[(Some("Commands"), 5), (Some("Files"), 5)]),
            12,
            "2 headers + 10 rows"
        );
        assert_eq!(all_tab_max_visible(&[(None, 5)]), 5);
        assert_eq!(
            all_tab_max_visible(&[]),
            1,
            "never 0 — layout divides by it"
        );
    }

    /// All tab with 5 commands + 5 files (2 titled sections, 10 rows = 12
    /// display slots): every row must be laid out and hit-testable,
    /// including the last file row and the leading "Commands" header —
    /// `max_visible` undercounting used to truncate/scroll them off even
    /// though the All tab is deliberately non-scrolling.
    #[test]
    fn all_tab_lays_out_every_row_across_two_titled_sections() {
        use crate::model::ui::{CommandMatch, FileFinderState, FileMatch};
        use crate::model::{CommandPaletteState, ModalState, SearchTab};

        let mut state = CommandPaletteState {
            matches: crate::commands::all_commands()
                .take(5)
                .map(|def| CommandMatch {
                    def,
                    indices: Vec::new(),
                })
                .collect(),
            active_tab: SearchTab::All,
            files_available: true,
            ..CommandPaletteState::default()
        };
        let root = std::path::PathBuf::from("/ws");
        let mut files = FileFinderState::new(Vec::new(), root.clone());
        files.results = (0..5)
            .map(|i| FileMatch::from_path(&root.join(format!("f{i}.rs")), &root, 0, Vec::new()))
            .collect();
        state.files = Some(files);

        let mut model = AppModel::new(1200, 800, 1.0);
        model.ui.open_modal(ModalState::CommandPalette(state));

        let (max_visible, rows_laid_out) =
            with_modal_overlay_layout(&model, 1200, 800, 1.0, |spec, layout| {
                let Body::List { max_visible, .. } = spec.body else {
                    panic!("expected a list body");
                };
                (max_visible, layout.rows.len())
            })
            .expect("expected an active modal");

        assert_eq!(max_visible, 12);
        assert_eq!(
            rows_laid_out, 12,
            "all 12 display slots (2 headers + 10 rows) must be laid out"
        );
    }

    #[test]
    fn workspace_symbols_rows_share_flat_indices_with_hit_testing() {
        use crate::lsp::workspace_symbols::{SymbolItem, SymbolProvider};
        use crate::model::{CommandPaletteState, ModalState, SearchTab};
        for tab in [SearchTab::All, SearchTab::Symbols] {
            let mut state = CommandPaletteState {
                active_tab: tab,
                ..Default::default()
            };
            state.matches.truncate(5);
            state.symbols.available = true;
            state.symbols.results.items = (0..20)
                .map(|i| SymbolItem {
                    name: format!("symbol{i}"),
                    detail: "source.rs".into(),
                    kind: lsp_types::SymbolKind::FUNCTION,
                    location: lsp_types::Location {
                        uri: "file:///ws/source.rs".parse().unwrap(),
                        range: Default::default(),
                    },
                    provider: SymbolProvider {
                        server_id: "fake".into(),
                        root: "/ws".into(),
                        generation: 1,
                    },
                })
                .collect();
            if tab == SearchTab::Symbols {
                state.symbols.scroll_offset = 5;
            }
            let mut model = AppModel::new(1200, 800, 1.0);
            model.ui.open_modal(ModalState::CommandPalette(state));
            let (row, expected) = with_modal_overlay_layout(&model, 1200, 800, 1.0, |_, layout| {
                if tab == SearchTab::All {
                    assert_eq!(
                        layout.rows.len(),
                        12,
                        "two headings and capped command/symbol groups"
                    );
                    (layout.rows[11], 9)
                } else {
                    (layout.rows[0], 5)
                }
            })
            .unwrap();
            let point = Point::new((row.x + row.w / 2) as f64, (row.y + row.h / 2) as f64);
            assert!(
                matches!(hit_test_modal(&model, point), Some(HitTarget::ModalRow {flat_index}) if flat_index == expected)
            );
        }
    }

    /// Regression for the reported blocker: the shape-only spec
    /// `with_modal_overlay_layout` builds for hit-testing/caret placement
    /// used to hardcode `tabs: None`, silently dropping the tab bar the
    /// renderer draws — this is exactly what overlay-surface.md's
    /// Hit-testing section forbids ("one layout, two consumers").
    #[test]
    fn shape_spec_includes_the_tab_bar_the_renderer_draws() {
        let model = opened_palette_model();
        let result = with_modal_overlay_layout(&model, 1200, 800, 1.0, |spec, layout| {
            assert!(spec.tabs.is_some(), "shape spec dropped the tab bar");
            assert_eq!(layout.tab_rects.len(), 4, "All/Commands/Files/Symbols");
        });
        assert!(result.is_some(), "expected an active modal");
    }

    /// A click inside the tab bar must resolve to `HitTarget::ModalTab`,
    /// not fall through to `Modal { inside: true }` — the failure mode the
    /// tab-bar-less shape spec produced (click-to-switch unreachable).
    #[test]
    fn tab_bar_click_resolves_to_a_modal_tab_not_a_generic_inside_hit() {
        let model = opened_palette_model();
        let commands_tab_rect =
            with_modal_overlay_layout(&model, 1200, 800, 1.0, |_, layout| layout.tab_rects[1])
                .expect("expected an active modal");
        let pt = Point::new(
            (commands_tab_rect.x + commands_tab_rect.w / 2) as f64,
            (commands_tab_rect.y + commands_tab_rect.h / 2) as f64,
        );
        assert!(
            matches!(
                hit_test_modal(&model, pt),
                Some(HitTarget::ModalTab { index: 1 })
            ),
            "expected a click on the Commands tab to hit ModalTab {{ index: 1 }}"
        );
    }

    /// A click on the first rendered *data* row must activate flat index 0,
    /// not a different row — reproduces the reported drift where the
    /// renderer's row 0 and the hit-test layout's row 0 disagreed once the
    /// tab bar was missing from one of the two consumers. `rows[0]` here is
    /// the "Commands" section header (`opened_palette_model` lands on the
    /// All tab, which always titles its single section) — a display slot,
    /// but not a `ModalRow`.
    #[test]
    fn row_click_activates_the_row_actually_rendered_there() {
        let model = opened_palette_model();
        let (header_rect, row_rect) =
            with_modal_overlay_layout(&model, 1200, 800, 1.0, |_, layout| {
                (layout.rows[0], layout.rows[1])
            })
            .expect("expected an active modal");

        let header_pt = Point::new(
            (header_rect.x + header_rect.w / 2) as f64,
            (header_rect.y + header_rect.h / 2) as f64,
        );
        assert!(
            !matches!(
                hit_test_modal(&model, header_pt),
                Some(HitTarget::ModalRow { .. })
            ),
            "a click on the section header must not activate a row"
        );

        let pt = Point::new(
            (row_rect.x + row_rect.w / 2) as f64,
            (row_rect.y + row_rect.h / 2) as f64,
        );
        assert!(
            matches!(
                hit_test_modal(&model, pt),
                Some(HitTarget::ModalRow { flat_index: 0 })
            ),
            "expected a click on the first rendered data row to hit ModalRow {{ flat_index: 0 }}"
        );
    }

    /// The header input rect (used to place the IME caret) must sit
    /// *below* the tab bar, not overlap it — the caret-drift symptom of the
    /// same shape-spec bug.
    #[test]
    fn header_input_rect_sits_below_the_tab_bar() {
        let model = opened_palette_model();
        let (tab_bottom, header_top) = with_modal_overlay_layout(&model, 1200, 800, 1.0, |_, l| {
            let tab_bar = l.tab_bar.expect("shape spec should lay out a tab bar");
            let header = l.header.expect("palette always has a header");
            (tab_bar.y + tab_bar.h, header.y)
        })
        .expect("expected an active modal");
        assert!(
            header_top >= tab_bottom,
            "header (y={header_top}) overlaps the tab bar (bottom={tab_bottom})"
        );
    }

    /// Regression: a cursor overlay flipped above the caret must clear the
    /// caret's own *line* (top edge), not just its bottom edge — the caret
    /// rect passed to `Anchor::Cursor` has real height, and an early
    /// version anchored both directions off the caret's bottom, so a
    /// flipped popup's bottom edge sat just above the caret bottom while
    /// still covering the line being edited.
    #[test]
    fn cursor_overlay_flip_above_clears_the_caret_line_not_just_its_bottom() {
        use crate::model::editor::Cursor;
        use crate::model::CursorOverlayKind;
        let mut model = AppModel::new(400, 800, 1.0);
        model.line_height = 20;
        model.document_mut().buffer = ropey::Rope::from("\n".repeat(50));
        model.editor_mut().cursors = vec![Cursor::at(36, 0)];
        model.editor_mut().viewport.top_line = 0;
        model.editor_mut().viewport.visible_lines = 40;
        model
            .editor_area
            .focused_group_mut()
            .expect("focused group")
            .rect = crate::model::editor_area::Rect::new(0.0, 0.0, 400.0, 800.0);
        // Line 36 lands near the bottom of the window: not enough room
        // below for an 8-row completion popup, plenty of room above.
        let caret =
            crate::view::caret::active_text_input_rect(&model, model.char_width, model.line_height)
                .expect("text caret");
        assert!(
            800 - (caret.y + caret.h) < 192,
            "test setup: caret (y={} h={}) should leave less than one popup's worth of room below",
            caret.y,
            caret.h
        );

        model.ui.cursor_overlay = Some(crate::model::CursorOverlayState::new(
            CursorOverlayKind::DebugCompletion,
        ));
        let l = with_cursor_overlay_spec(&model, |spec| {
            overlay_surface::layout(spec, 400, 800, 1.0).panel
        })
        .expect("cursor overlay open");
        assert!(
            l.y + l.h <= caret.y,
            "flipped popup (y={} h={}) overlaps the caret's own line (top={})",
            l.y,
            l.h,
            caret.y
        );
    }

    #[test]
    fn opening_the_lsp_servers_modal_lists_a_row_per_registered_server() {
        let mut model = AppModel::new(1200, 800, 1.0);
        update(
            &mut model,
            crate::messages::Msg::Ui(UiMsg::ToggleModal(ModalId::LspServers)),
        );

        let row_count =
            with_modal_overlay_layout(&model, 1200, 800, 1.0, |_, layout| layout.rows.len())
                .expect("expected an active modal");
        assert_eq!(row_count, crate::lsp::all_server_defs().len());
    }

    #[test]
    fn lsp_server_config_enabled_defaults_true_and_reflects_a_disabled_override() {
        let mut model = AppModel::new(1200, 800, 1.0);
        let server_id = crate::lsp::all_server_defs()[0].id;
        assert!(lsp_server_config_enabled(&model, server_id));

        model.config.lsp.servers.insert(
            server_id.to_owned(),
            crate::config::LspServerOverride {
                enabled: Some(false),
                ..Default::default()
            },
        );
        assert!(!lsp_server_config_enabled(&model, server_id));
    }

    #[test]
    fn disabled_server_gets_the_dim_text_accessory_enabled_gets_the_check() {
        assert!(matches!(lsp_server_accessory(true), Accessory::Check));
        assert!(matches!(
            lsp_server_accessory(false),
            Accessory::DimText("disabled")
        ));
    }

    // ========================================================================
    // Context menu (context-menu.md)
    // ========================================================================

    #[test]
    fn context_menu_sections_maps_separators_to_section_boundaries() {
        use crate::context_menu::MenuItem;

        let items = vec![
            MenuItem::custom("Cut", true, vec![]),
            MenuItem::custom("Copy", false, vec![]),
            MenuItem::separator(),
            MenuItem::custom("Go to Definition", true, vec![]),
            MenuItem::separator(),
            MenuItem::separator(), // adjacent separators: no empty section
            MenuItem::custom("Reveal in File Explorer", true, vec![]),
        ];
        let chip_steps = context_menu_chip_steps(&items);
        let rows = context_menu_rows(&items, &chip_steps);
        assert_eq!(rows.len(), 4, "4 non-separator items");

        let sections = context_menu_sections(&items, &rows);
        let lens: Vec<usize> = sections.iter().map(|s| s.rows.len()).collect();
        assert_eq!(
            lens,
            vec![2, 1, 1],
            "3 runs: {{Cut,Copy}}, {{GotoDef}}, {{Reveal}}"
        );
        assert!(sections.iter().all(|s| s.title.is_none()));
        assert_eq!(sections[0].rows[0].label, "Cut");
        assert_eq!(sections[0].rows[1].label, "Copy");
        assert_eq!(sections[1].rows[0].label, "Go to Definition");
        assert_eq!(sections[2].rows[0].label, "Reveal in File Explorer");
    }

    #[test]
    fn context_menu_sections_drops_leading_and_trailing_separators() {
        use crate::context_menu::MenuItem;

        let items = vec![
            MenuItem::separator(),
            MenuItem::custom("Only Item", true, vec![]),
            MenuItem::separator(),
        ];
        let chip_steps = context_menu_chip_steps(&items);
        let rows = context_menu_rows(&items, &chip_steps);
        let sections = context_menu_sections(&items, &rows);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].rows.len(), 1);
    }

    #[test]
    fn context_menu_layout_clamps_to_window_and_keeps_separators_unselectable() {
        use crate::context_menu::{ContextMenuRegion, MenuItem};
        use crate::model::{ContextMenuState, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(400, 300, 1.0);
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::ContextMenu));
        model.ui.context_menu = Some(ContextMenuState {
            items: vec![
                MenuItem::custom("First", true, vec![]),
                MenuItem::separator(),
                MenuItem::custom("Second", true, vec![]),
            ],
            anchor: (399, 299, 0),
            region: ContextMenuRegion::Editor,
        });
        model.ui.cursor_overlay.as_mut().unwrap().hover_row = Some(1);

        with_cursor_overlay_spec(&model, |spec| {
            assert_eq!(spec.hover_row, Some(FlatIndex(1)));
            let layout = &overlay_surface::layout(spec, 400, 300, 1.0);
            assert!(layout.panel.x + layout.panel.w <= 400);
            assert!(layout.panel.y + layout.panel.h <= 300);
            assert_eq!(layout.rows.len(), 3, "separator occupies one display row");

            let hit_at = |rect: WidgetRect| {
                overlay_surface::hit_test(spec, layout, rect.x + rect.w / 2, rect.y + rect.h / 2)
            };
            assert_eq!(
                hit_at(layout.rows[0]),
                overlay_surface::OverlayHit::Row(FlatIndex(0))
            );
            assert_eq!(hit_at(layout.rows[1]), overlay_surface::OverlayHit::Inside);
            assert_eq!(
                hit_at(layout.rows[2]),
                overlay_surface::OverlayHit::Row(FlatIndex(1))
            );
        })
        .expect("context menu should produce a layout");

        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = crate::view::GlyphCache::default();
        let mut sample_rows = |hover_row| {
            model.ui.cursor_overlay.as_mut().unwrap().hover_row = hover_row;
            with_cursor_overlay_spec(&model, |spec| {
                let mut buffer = vec![0; 400 * 300];
                let mut frame = Frame::new(&mut buffer, 400, 300);
                let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);
                let mut mask_cache = RoundedRectMaskCache::new();
                overlay_surface::render(
                    &mut frame,
                    &mut painter,
                    &mut mask_cache,
                    &model.theme,
                    spec,
                    400,
                    300,
                    1.0,
                    false,
                );
                let layout = overlay_surface::layout(spec, 400, 300, 1.0);
                layout
                    .rows
                    .iter()
                    .map(|rect| frame.get_pixel(rect.x + rect.w - 20, rect.y + rect.h / 2))
                    .collect::<Vec<_>>()
            })
            .expect("context menu should render")
        };
        let plain = sample_rows(None);
        let hovered = sample_rows(Some(1));
        assert_eq!(plain[0], hovered[0], "keyboard selection stays highlighted");
        assert_eq!(plain[1], hovered[1], "separator is never highlighted");
        assert_ne!(
            plain[2], hovered[2],
            "pointer row receives a visible hover wash"
        );
        assert_eq!(plain, sample_rows(None), "leaving clears the hover wash");

        let mut item = MenuItem::custom("Reveal in File Explorer", true, vec![]);
        item.shortcut_hint = Some("⌃⌥⇧⌘R".to_owned());
        model.ui.context_menu.as_mut().unwrap().items = vec![item];
        for scale in [1.0, 2.0] {
            with_cursor_overlay_spec(&model, |spec| {
                let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);
                let mut measure = crate::layout::PainterMeasure::new(&mut painter);
                let layout = overlay_surface::layout_measured(spec, 1200, 900, scale, &mut measure);
                assert!(
                    layout.panel.w > (200.0 * scale) as usize,
                    "long menu grows beyond its floor"
                );
                assert!(layout.panel.w <= (520.0 * scale) as usize);
                assert!(layout.panel.x + layout.panel.w <= 1200);
                let narrow = overlay_surface::layout_measured(spec, 150, 300, scale, &mut measure);
                assert!(narrow.panel.x + narrow.panel.w <= 150);
                assert_eq!(
                    overlay_surface::hit_test(
                        spec,
                        &layout,
                        layout.panel.x + layout.panel.w - 2,
                        layout.rows[0].y + 2
                    ),
                    overlay_surface::OverlayHit::Row(FlatIndex(0)),
                    "expanded area is interactive too",
                );
            })
            .unwrap();
        }
    }

    #[test]
    fn context_menu_rows_falls_back_to_dim_text_for_a_5_chip_binding() {
        use crate::context_menu::MenuItem;

        let mut item = MenuItem::custom("Everything", true, vec![]);
        item.shortcut_hint = Some("⌃⌥⇧⌘X".to_owned()); // 5 chips
        let items = vec![item];
        let chip_steps = context_menu_chip_steps(&items);
        let rows = context_menu_rows(&items, &chip_steps);
        assert!(matches!(rows[0].accessory, Accessory::DimText(_)));
    }

    /// The docs card only exists when the selected completion item carries
    /// documentation, and then sits to the right of the menu panel.
    /// Acceptance: the active parameter is an `Accent` span over the exact
    /// label range (no `‹›` brackets in the text), the parameter doc keeps
    /// its markdown spans, and the signature counter is dim.
    #[test]
    fn signature_help_spec_styles_the_active_parameter_doc_and_counter() {
        use crate::model::{SignatureHelpState, SignatureView, SpanStyle};

        let mut model = AppModel::new(800, 600, 1.0);
        let label = "fn f(a: i32, b: &str)";
        model.ui.signature_help = Some(SignatureHelpState {
            signatures: vec![
                SignatureView {
                    label: label.to_owned(),
                    active_parameter_range: Some((13, 20)), // "b: &str"
                    doc: Some(crate::lsp::markdown::markdown_to_styled("Does *f*.")),
                    parameter_doc: Some(crate::lsp::markdown::markdown_to_styled(
                        "the **second** one, see `foo`",
                    )),
                },
                SignatureView {
                    label: "fn f()".to_owned(),
                    active_parameter_range: None,
                    doc: None,
                    parameter_doc: None,
                },
            ],
            active: 0,
        });

        let (code, code_spans, text, text_spans) =
            with_signature_help_spec(&model, |spec| match &spec.body {
                Body::Zones(z) => (
                    z.code.unwrap().to_owned(),
                    z.code_spans.to_vec(),
                    z.text.unwrap().to_owned(),
                    z.text_spans.to_vec(),
                ),
                _ => panic!("signature help renders a Zones body"),
            })
            .expect("signature help open");

        assert_eq!(code, label, "no bracket markers in the signature text");
        assert_eq!(code_spans.len(), 1);
        assert_eq!(&code[code_spans[0].range.clone()], "b: &str");
        assert_eq!(code_spans[0].style, SpanStyle::Accent);

        assert_eq!(text, "the second one, see foo\n\nDoes f.\n\n(1 of 2)");
        let styled: Vec<(&str, SpanStyle)> = text_spans
            .iter()
            .map(|s| (&text[s.range.clone()], s.style))
            .collect();
        assert_eq!(
            styled,
            vec![
                ("second", SpanStyle::Strong),
                ("foo", SpanStyle::Code),
                ("f", SpanStyle::Strong),
                ("(1 of 2)", SpanStyle::Dim),
            ]
        );
        model.ui.cursor_overlay = Some(crate::model::CursorOverlayState::new(
            crate::model::CursorOverlayKind::Completion,
        ));
        assert!(with_signature_help_spec(&model, |_| ()).is_none());
        assert!(
            model.ui.signature_help.is_some(),
            "priority hides rather than discards signature state"
        );
        model.ui.cursor_overlay = None;
        assert!(with_signature_help_spec(&model, |_| ()).is_some());
    }

    /// Hover content keeps its markdown spans all the way to the card.
    #[test]
    fn hover_card_spec_carries_markdown_spans() {
        use crate::model::{CursorOverlayKind, CursorOverlayState, HoverCardState, SpanStyle};

        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.hover_card = Some(HoverCardState {
            content: Some(crate::lsp::markdown::markdown_to_styled(
                "```rust\nfn foo()\n```\nReturns **nothing**.",
            )),
            ..Default::default()
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Hover));

        let (code, text, spans) = with_cursor_overlay_spec(&model, |spec| match &spec.body {
            Body::Zones(z) => (
                z.code.map(str::to_owned),
                z.text.unwrap().to_owned(),
                z.text_spans.to_vec(),
            ),
            _ => panic!("hover renders a Zones body"),
        })
        .expect("hover open");
        // The leading fence lands in the code zone; the prose keeps its
        // emphasis span.
        assert_eq!(code.as_deref(), Some("fn foo()"));
        assert_eq!(text, "Returns nothing.");
        let styled: Vec<(&str, SpanStyle)> = spans
            .iter()
            .map(|s| (&text[s.range.clone()], s.style))
            .collect();
        assert_eq!(styled, vec![("nothing", SpanStyle::Strong)]);
    }

    #[test]
    fn completion_rows_show_server_method_metadata_without_altering_label() {
        let item = serde_json::from_value(serde_json::json!({
            "label": "compile", "kind": 2,
            "labelDetails": { "detail": "(output: &str)", "description": "()" }
        }))
        .unwrap();
        let items = crate::completion::lsp::items_to_menu_items(
            vec![item],
            &crate::lsp::LspServerId::from("rust-analyzer"),
            std::path::Path::new("/tmp/proj"),
            None,
        );
        let menu = crate::completion::CompletionMenuState {
            document_id: crate::model::DocumentId(1),
            revision: 0,
            query_start: crate::model::Cursor::at(0, 8),
            query: String::new(),
            items,
            filtered: vec![(0, 0, vec![])],
            is_incomplete: false,
            pending_resolve: None,
            context: crate::completion::context::CompletionContext::Member,
            selection_changed: false,
        };
        let rows = completion_rows(&menu);
        assert!(matches!(
            rows[0].icon,
            RowIcon::KindBadge(MenuItemKind::Method)
        ));
        assert_eq!(rows[0].detail, Some("(output: &str) ()"));
        assert_eq!(menu.selected_item(0).unwrap().label, "compile");
    }

    #[test]
    fn completion_docs_panel_follows_the_selected_items_documentation() {
        use crate::completion::menu::{
            CompletionMenuState, LspInsert, MenuInsert, MenuItem, MenuItemKind, MenuSourceId,
        };
        use crate::model::{CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str("va\n");
        let doc = model.document();
        let (document_id, revision) = (doc.id.unwrap(), doc.revision);
        let item = |label: &str, docs: Option<&str>| MenuItem {
            label: label.to_owned(),
            filter_text: label.to_owned(),
            insert: MenuInsert::Lsp(Box::new(LspInsert {
                text: label.to_owned(),
                server_id: crate::lsp::LspServerId::from("rust-analyzer"),
                root: std::path::PathBuf::from("/tmp/proj"),
                raw: std::sync::Arc::new(lsp_types::CompletionItem {
                    label: label.into(),
                    ..Default::default()
                }),
                can_resolve: true,
                resolved: true,
                text_edit: None,
                additional_text_edits: Vec::new(),
                commit_characters: std::sync::Arc::from([]),
                documentation: docs.map(crate::model::StyledText::from),
                caret_offset: None,
            })),
            kind: MenuItemKind::Function,
            source: MenuSourceId::Lsp,
            detail: None,
            sort_text: None,
            preselect: false,
        };
        model.ui.completion_menu = Some(CompletionMenuState {
            document_id,
            revision,
            query_start: crate::model::Cursor::at(0, 0),
            query: "va".to_owned(),
            items: vec![
                item("value_plain", None),
                item("value_documented", Some("Returns the value.")),
            ],
            filtered: vec![(0, 0, Vec::new()), (0, 1, Vec::new())],
            is_incomplete: false,
            pending_resolve: None,
            context: Default::default(),
            selection_changed: false,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        let layout_for = |model: &AppModel| {
            with_cursor_overlay_spec(model, |spec| overlay_surface::layout(spec, 800, 600, 1.0))
                .expect("completion overlay open")
        };

        let without = layout_for(&model);
        assert!(without.docs_panel.is_none(), "no docs -> no card");

        model.ui.cursor_overlay.as_mut().unwrap().selected = 1;
        let with = layout_for(&model);
        let docs = with.docs_panel.expect("documented item -> card");
        assert_eq!(
            docs.x,
            with.panel.x + with.panel.w,
            "card sits right of the panel"
        );
        assert_eq!(docs.y, with.panel.y);
        assert!(with.docs_text.is_some());
    }
}
