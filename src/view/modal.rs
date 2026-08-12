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

use crate::model::ui::{FindReplaceField, RecentFilesState, ThemePickerState};
use crate::model::AppModel;
use crate::theme::ThemeInfo;

use super::frame::{Frame, RoundedRectMaskCache, TextPainter};
use super::geometry::WidgetRect;
use super::overlay_surface::{
    self, Accessory, Anchor, Body, Field, FlatIndex, Footer, Header, OverlayLayout, OverlaySpec,
    Row, RowIcon, Section, TabBar, WidthRule,
};
use super::text_field::{TextFieldContent, TextFieldRenderer};

use crate::model::COMMAND_PALETTE_MAX_VISIBLE;

/// Modal dim background alpha (102/255 ≈ 40% opacity)
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

fn width_rule((pct, min, max): (f32, f32, f32)) -> WidthRule {
    WidthRule { pct, min, max }
}

// ============================================================================
// Command Palette
// ============================================================================

/// Per-row accessory decision: a keybinding renders as keycap chips unless
/// it has more than 4 chips total, in which case it falls back to dim text
/// (Visual Language > Keycaps).
enum PaletteAccessory {
    None,
    DimText(&'static str),
    Keycaps(Vec<Vec<overlay_surface::Chip>>),
}

fn palette_accessory(keybinding: Option<&'static str>) -> PaletteAccessory {
    use crate::view::overlay_surface::{binding_chips, chip_count};

    match keybinding {
        None => PaletteAccessory::None,
        Some(kb) => {
            let steps = binding_chips(kb);
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
        ("All", count_for(commands_total + file_total)),
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
        ("Symbols", TabCount::Unavailable),
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
    let accessories: Vec<PaletteAccessory> = state
        .matches
        .iter()
        .map(|m| palette_accessory(m.def.keybinding))
        .collect();
    let cmd_rows = command_rows(&state.matches, &accessories, icon_color);
    let file_rows_all = state
        .files
        .as_ref()
        .map(|f| file_rows(&f.results, icon_color))
        .unwrap_or_default();

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
                        } else {
                            let rows = &cmd_rows[cmd_offset..cmd_offset + len];
                            cmd_offset += len;
                            Section { title, rows }
                        }
                    })
                    .collect();
                let total: usize = sections_spec.iter().map(|&(_, len)| len).sum();
                (sections, state.all_selected, 0, total.max(1))
            }
            SearchTab::Symbols => (Vec::new(), 0, 0, COMMAND_PALETTE_MAX_VISIBLE),
        };

    let spec = OverlaySpec {
        tabs: Some(tabs),
        anchor: Anchor::Centered {
            width: width_rule(PALETTE_WIDTH),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Some(Header {
            glyph: Some('\u{276F}'),
            text: &input_text,
            placeholder: "Search commands, files\u{2026}",
            caret: Some(
                state
                    .editable
                    .cursors()
                    .first()
                    .map(|c| c.column)
                    .unwrap_or(0),
            ),
            scope: None,
        }),
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll,
            max_visible,
        },
        footer: Some(Footer {
            leading: "\u{2191}\u{2193} navigate \u{00b7} \u{21b5} run \u{00b7} \u{21e5} tab",
            trailing: "esc dismiss",
        }),
        hover_row: model.ui.modal_hover_row.map(FlatIndex),
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
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
        SearchTab::Symbols => Some("No language server for this file"),
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
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
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
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
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
    let icon_color = model.theme.overlay.text_dim.to_argb_u32();
    let groups = theme_picker_groups(&state.themes);

    let row_groups: Vec<Vec<Row>> = groups
        .iter()
        .map(|(_, range)| {
            state.themes[range.clone()]
                .iter()
                .map(|theme_info| {
                    let is_active =
                        model.theme.name == theme_info.name || model.config.theme == theme_info.id;
                    Row {
                        icon: RowIcon::Glyph {
                            ch: '\u{25CF}',
                            color: icon_color,
                        },
                        label: &theme_info.name,
                        match_indices: &[],
                        detail: None,
                        accessory: if is_active {
                            Accessory::Check
                        } else {
                            Accessory::None
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
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
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

fn render_goto_line_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::GotoLineState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let fields = [Field { label: "Line:" }];
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
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
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
            &state.editable,
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

fn render_find_replace_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::FindReplaceState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let fields = if state.replace_mode {
        vec![Field { label: "Find:" }, Field { label: "Replace:" }]
    } else {
        vec![Field { label: "Find:" }]
    };
    let focused = match state.focused_field {
        FindReplaceField::Query => 0,
        FindReplaceField::Replace => 1,
    };

    let spec = OverlaySpec {
        tabs: None,
        anchor: Anchor::Centered {
            width: width_rule(SMALL_MODAL_WIDTH),
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: None,
        body: Body::Fields {
            fields: &fields,
            focused,
        },
        footer: if state.replace_mode {
            Some(Footer {
                leading: "\u{21b5} find \u{00b7} \u{2318}\u{21b5} replace all",
                trailing: "esc dismiss",
            })
        } else {
            None
        },
        hover_row: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
        &spec,
        ctx.window_width,
        ctx.window_height,
        ctx.scale_factor,
        model.ui.cursor_visible,
    );

    let l = overlay_surface::layout(&spec, ctx.window_width, ctx.window_height, ctx.scale_factor);
    let highlight = model.theme.overlay.highlight.to_argb_u32();
    let selection_bg = model.theme.overlay.selection_background.to_argb_u32();

    if let Some(find_field) = l.fields.first() {
        let cursor_visible =
            model.ui.cursor_visible && matches!(state.focused_field, FindReplaceField::Query);
        TextFieldRenderer::render_modal_input(
            frame,
            painter,
            &state.query_editable,
            &find_field.input,
            ctx.line_height,
            ctx.char_width,
            ctx.colors.input_bg,
            ctx.colors.fg,
            highlight,
            selection_bg,
            cursor_visible,
            ctx.scale_factor,
        );
    }

    if state.replace_mode {
        if let Some(replace_field) = l.fields.get(1) {
            let cursor_visible =
                model.ui.cursor_visible && matches!(state.focused_field, FindReplaceField::Replace);
            TextFieldRenderer::render_modal_input(
                frame,
                painter,
                &state.replace_editable,
                &replace_field.input,
                ctx.line_height,
                ctx.char_width,
                ctx.colors.input_bg,
                ctx.colors.fg,
                highlight,
                selection_bg,
                cursor_visible,
                ctx.scale_factor,
            );
        }
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
                SearchTab::All => (state.all_selected, 0, total.max(1)),
                SearchTab::Symbols => (0, 0, COMMAND_PALETTE_MAX_VISIBLE),
            };

            let spec = OverlaySpec {
                tabs: Some(tabs),
                anchor: Anchor::Centered {
                    width: width_rule(PALETTE_WIDTH),
                    dim_alpha: MODAL_DIM_ALPHA,
                },
                header: Some(Header {
                    glyph: None,
                    text: "",
                    placeholder: "",
                    caret: Some(state.input().chars().count()),
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
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::GotoLine(_) => {
            let fields = [Field { label: "" }];
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
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
        ModalState::FindReplace(state) => {
            let fields = if state.replace_mode {
                vec![Field { label: "" }, Field { label: "" }]
            } else {
                vec![Field { label: "" }]
            };
            let focused = match state.focused_field {
                FindReplaceField::Query => 0,
                FindReplaceField::Replace => 1,
            };
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Centered {
                    width: width_rule(SMALL_MODAL_WIDTH),
                    dim_alpha: MODAL_DIM_ALPHA,
                },
                header: None,
                body: Body::Fields {
                    fields: &fields,
                    focused,
                },
                footer: if state.replace_mode {
                    Some(Footer {
                        leading: "",
                        trailing: "",
                    })
                } else {
                    None
                },
                hover_row: None,
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
    }
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
) -> Option<WidgetRect> {
    with_modal_overlay_layout(model, window_width, window_height, scale_factor, |_, l| {
        let header = l.header?;
        let pad = overlay_surface::header_pad_x(scale_factor);
        Some(WidgetRect {
            x: header.x + pad,
            y: header.y,
            w: header.w.saturating_sub(pad * 2),
            h: header.h,
        })
    })
    .flatten()
}

/// The input rect for field `field_index` of a `Body::Fields` modal (Go to
/// Line, Find/Replace), used by `view::caret` to place the IME caret.
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
        ModalState::ThemePicker(state) => {
            render_theme_picker_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::CommandPalette(state) => {
            render_command_palette_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::GotoLine(state) => {
            render_goto_line_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::FindReplace(state) => {
            render_find_replace_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::FileFinder(state) => {
            render_file_finder_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::RecentFiles(state) => {
            render_recent_files_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
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
            banner: None,
            code: None,
            text: Some(&text),
        }),
        footer: None,
        hover_row: None,
    };

    overlay_surface::render(
        frame,
        painter,
        mask_cache,
        &model.theme.overlay,
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

/// Dummy rows exercising the Completion list shell (kind badges, dim
/// signature accessory) — manual-testing content only; the real completion
/// source is [autocomplete.md](autocomplete.md) Phase 1.
fn debug_completion_rows() -> Vec<Row<'static>> {
    const ITEMS: &[(overlay_surface::CompletionKind, &str, &str)] = &[
        (
            overlay_surface::CompletionKind::Function,
            "to_string",
            "fn() -> String",
        ),
        (
            overlay_surface::CompletionKind::Function,
            "trim",
            "fn() -> &str",
        ),
        (overlay_surface::CompletionKind::Variable, "value", "i32"),
        (overlay_surface::CompletionKind::Type, "String", "struct"),
        (overlay_surface::CompletionKind::Keyword, "match", "keyword"),
        (overlay_surface::CompletionKind::Field, "len", "usize"),
        (
            overlay_surface::CompletionKind::Module,
            "std::fmt",
            "module",
        ),
        (overlay_surface::CompletionKind::Constant, "MAX", "usize"),
    ];
    ITEMS
        .iter()
        .map(|&(kind, label, detail)| Row {
            icon: RowIcon::KindBadge(kind),
            label,
            match_indices: &[],
            detail: Some(detail),
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
    }
}

/// Build the cursor-overlay `OverlaySpec` for whichever demo kind is open,
/// call `overlay_surface::layout()` on it, and hand both to `f` — mirrors
/// `with_modal_overlay_layout`'s one-layout-two-consumers shape so render
/// and hit-testing can't disagree on geometry. Content here is static (no
/// per-frame temporaries), so unlike the modal version there's no need for a
/// separate placeholder-content path.
pub(crate) fn with_cursor_overlay_layout<R>(
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    f: impl FnOnce(&OverlaySpec, &OverlayLayout) -> R,
) -> Option<R> {
    let state = model.ui.cursor_overlay?;
    let (x, y, h) = cursor_overlay_anchor(model)?;

    match state.kind {
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
                hover_row: None,
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
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
                        pct: 0.0,
                        min: 280.0,
                        max: 420.0,
                    },
                },
                header: None,
                body: Body::Zones(debug_hover_zones()),
                footer: None,
                hover_row: None,
            };
            let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
            Some(f(&spec, &l))
        }
    }
}

/// Render the active cursor-anchored popup (completion/hover shells;
/// currently debug-demo content only — see `with_cursor_overlay_layout`).
pub fn render_cursor_overlay(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    window_width: usize,
    window_height: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let scale_factor = model.metrics.scale_factor;
    with_cursor_overlay_layout(
        model,
        window_width,
        window_height,
        scale_factor,
        |spec, _l| {
            overlay_surface::render(
                frame,
                painter,
                mask_cache,
                &model.theme.overlay,
                spec,
                window_width,
                window_height,
                scale_factor,
                model.ui.cursor_visible,
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{ModalMsg, UiMsg};
    use crate::model::ModalId;
    use crate::update::update_ui;
    use crate::view::hit_test::{hit_test_modal, HitTarget, Point};

    /// A Search Everywhere modal open on the Commands tab with a non-empty
    /// query — real rows exist, so row-rect assertions below aren't testing
    /// against an empty list. Window size/scale mirror the drift reported
    /// against overlay-surface.md's Hit-testing invariant.
    fn opened_palette_model() -> AppModel {
        let mut model = AppModel::new(1200, 800, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(&mut model, UiMsg::Modal(ModalMsg::InsertChar('o')));
        model
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

    /// A click on the first rendered row must activate flat index 0, not a
    /// different row — reproduces the reported drift where the renderer's
    /// row 0 and the hit-test layout's row 0 disagreed once the tab bar
    /// was missing from one of the two consumers.
    #[test]
    fn row_click_activates_the_row_actually_rendered_there() {
        let model = opened_palette_model();
        let row_rect =
            with_modal_overlay_layout(&model, 1200, 800, 1.0, |_, layout| layout.rows[0])
                .expect("expected an active modal");
        let pt = Point::new(
            (row_rect.x + row_rect.w / 2) as f64,
            (row_rect.y + row_rect.h / 2) as f64,
        );
        assert!(
            matches!(
                hit_test_modal(&model, pt),
                Some(HitTarget::ModalRow { flat_index: 0 })
            ),
            "expected a click on the first rendered row to hit ModalRow {{ flat_index: 0 }}"
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
        let mut model = AppModel::new(400, 800, 1.0, vec![]);
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
        let l = with_cursor_overlay_layout(&model, 400, 800, 1.0, |_, l| l.panel)
            .expect("cursor overlay open");
        assert!(
            l.y + l.h <= caret.y,
            "flipped popup (y={} h={}) overlaps the caret's own line (top={})",
            l.y,
            l.h,
            caret.y
        );
    }
}
