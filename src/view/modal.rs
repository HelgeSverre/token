//! Modal overlay rendering (command palette, goto line, find/replace, etc.)

use crate::model::AppModel;

use super::frame::{Frame, RoundedRectMaskCache, TextPainter};
use super::geometry;
use super::overlay_surface;
use super::selectable_list::{
    render_selectable_list, SelectableListColors, SelectableListLayout, SelectableListViewport,
};
use super::text_field::{TextFieldContent, TextFieldRenderer};

#[derive(Clone, Copy)]
struct ModalColors {
    bg: u32,
    fg: u32,
    highlight: u32,
    dim: u32,
    selection_bg: u32,
    input_bg: u32,
    border: u32,
}

impl ModalColors {
    fn from_model(model: &AppModel) -> Self {
        Self {
            bg: model.theme.overlay.background.to_argb_u32(),
            fg: model.theme.overlay.foreground.to_argb_u32(),
            highlight: model.theme.overlay.highlight.to_argb_u32(),
            dim: model.theme.overlay.foreground.with_alpha(128).to_argb_u32(),
            selection_bg: model.theme.overlay.selection_background.to_argb_u32(),
            input_bg: model.theme.overlay.input_background.to_argb_u32(),
            border: model
                .theme
                .overlay
                .border
                .map(|c| c.to_argb_u32())
                .unwrap_or(0xFF444444),
        }
    }
}

/// Constant rendering context shared by every modal for the duration of a
/// single `render_modals` call: window size, the shared line/char metrics,
/// and the resolved theme colors. Bundling these avoids threading the same
/// 5-6 parameters through every `render_*_modal` function individually.
struct ModalRenderCtx {
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: ModalColors,
    scale_factor: f64,
}

fn render_modal_shell(frame: &mut Frame, layout: &geometry::ModalLayout, colors: &ModalColors) {
    frame.draw_bordered_rect(
        layout.x,
        layout.y,
        layout.w,
        layout.h,
        colors.bg,
        colors.border,
    );
}

/// A single rendered row in the theme picker: either a section header or a
/// theme entry (indexing back into `state.themes`).
enum ThemeRow<'a> {
    Header(&'static str),
    Theme(usize, &'a crate::theme::ThemeInfo),
}

use geometry::THEME_PICKER_MAX_VISIBLE_ROWS;

fn render_theme_picker_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::ThemePickerState,
    ctx: &ModalRenderCtx,
) {
    use crate::theme::ThemeSource;

    let colors = &ctx.colors;
    let line_height = ctx.line_height;

    let themes = &state.themes;
    let clamped_selected = state.selected_index.min(themes.len().saturating_sub(1));

    // Flatten themes + section headers into a single row list so scrolling
    // and clipping can treat them uniformly.
    let mut rows: Vec<ThemeRow> = Vec::with_capacity(themes.len() + 2);
    let mut current_source: Option<ThemeSource> = None;
    let mut selected_row_index = 0;
    for (i, theme_info) in themes.iter().enumerate() {
        if current_source != Some(theme_info.source) {
            current_source = Some(theme_info.source);
            let header = match theme_info.source {
                ThemeSource::User => "User Themes",
                ThemeSource::Builtin => "Built-in Themes",
            };
            rows.push(ThemeRow::Header(header));
        }
        if i == clamped_selected {
            selected_row_index = rows.len();
        }
        rows.push(ThemeRow::Theme(i, theme_info));
    }

    let max_visible_rows = THEME_PICKER_MAX_VISIBLE_ROWS.min(rows.len().max(1));
    let viewport = SelectableListViewport::compute_from(
        rows.len(),
        selected_row_index,
        max_visible_rows,
        state.scroll_offset,
    );

    let (layout, w) = geometry::theme_picker_layout(
        ctx.window_width,
        ctx.window_height,
        line_height,
        rows.len().min(max_visible_rows),
        ctx.scale_factor,
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, "Switch Theme", colors.fg);

    let list_r = layout.widget(w.list);
    let section_color = 0xFF666666;

    frame.set_clip(crate::model::Rect::new(
        list_r.x as f32,
        list_r.y as f32,
        list_r.w as f32,
        list_r.h as f32,
    ));

    let mut current_y = list_r.y;
    for row in rows
        .iter()
        .skip(viewport.scroll_offset)
        .take(max_visible_rows)
    {
        match row {
            ThemeRow::Header(header) => {
                painter.draw(frame, layout.x + 12, current_y, header, section_color);
            }
            ThemeRow::Theme(i, theme_info) => {
                let is_selected = *i == clamped_selected;
                if is_selected {
                    frame.fill_rect_px(
                        layout.x + 4,
                        current_y,
                        layout.w - 8,
                        line_height,
                        colors.selection_bg,
                    );
                }

                let label_x = layout.x + 24;
                painter.draw(frame, label_x, current_y, &theme_info.name, colors.fg);

                if model.theme.name == theme_info.name || model.config.theme == theme_info.id {
                    let check_x = layout.x + layout.w - 30;
                    painter.draw(frame, check_x, current_y, "✓", colors.highlight);
                }
            }
        }
        current_y += line_height;
    }

    frame.clear_clip();
}

/// Palette width: 50% of window, clamped 480-640 logical px (Visual
/// Language > Chrome).
const COMMAND_PALETTE_WIDTH: (f32, f32, f32) = (0.5, 480.0, 640.0);

use crate::model::COMMAND_PALETTE_MAX_VISIBLE;

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

/// Panel geometry for the command palette, shared by
/// `render_command_palette_modal` and `hit_test::hit_test_modal` so a click
/// is tested against the geometry actually painted (one layout, two
/// consumers).
pub(crate) fn command_palette_overlay_panel(
    state: &crate::model::ui::CommandPaletteState,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
) -> super::geometry::WidgetRect {
    use crate::view::overlay_surface::{
        self, Accessory, Anchor, Body, FlatIndex, Header, Row, RowIcon, Section, WidthRule,
    };

    let count = state.matches.len();
    // Layout only depends on row count/height, not row content, so
    // placeholder rows are sufficient here.
    let rows: Vec<Row> = (0..count)
        .map(|_| Row {
            icon: RowIcon::None,
            label: "",
            match_indices: &[],
            detail: None,
            accessory: Accessory::None,
        })
        .collect();
    let sections = [Section {
        title: None,
        rows: &rows,
    }];
    let selected_index = state.selected_index.min(count.saturating_sub(1));
    let (pct, min, max) = COMMAND_PALETTE_WIDTH;
    let spec = overlay_surface::OverlaySpec {
        anchor: Anchor::Centered {
            width: WidthRule { pct, min, max },
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Header {
            glyph: None,
            text: "",
            placeholder: "",
            caret: None,
            scope: None,
        },
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: Some(overlay_surface::Footer {
            leading: "",
            trailing: "",
        }),
    };
    overlay_surface::layout(&spec, window_width, window_height, scale_factor).panel
}

/// The command palette's header text-input rect (inside the panel, minus
/// header padding), used by `view::caret` to place the IME caret. Content
/// doesn't affect this geometry, so an empty spec is enough.
pub(crate) fn command_palette_input_rect(
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
) -> super::geometry::WidgetRect {
    use crate::view::overlay_surface::{self, Anchor, Body, FlatIndex, Header, WidthRule};

    let sections: [overlay_surface::Section; 0] = [];
    let (pct, min, max) = COMMAND_PALETTE_WIDTH;
    let spec = overlay_surface::OverlaySpec {
        anchor: Anchor::Centered {
            width: WidthRule { pct, min, max },
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Header {
            glyph: None,
            text: "",
            placeholder: "",
            caret: None,
            scope: None,
        },
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(0),
            scroll: 0,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: None,
    };
    let l = overlay_surface::layout(&spec, window_width, window_height, scale_factor);
    let pad = overlay_surface::header_pad_x(scale_factor);
    super::geometry::WidgetRect {
        x: l.header.x + pad,
        y: l.header.y,
        w: l.header.w.saturating_sub(pad * 2),
        h: l.header.h,
    }
}

fn render_command_palette_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::CommandPaletteState,
    ctx: &ModalRenderCtx,
    mask_cache: &mut RoundedRectMaskCache,
) {
    use crate::view::overlay_surface::{
        self, Accessory, Anchor, Body, FlatIndex, Header, Row, RowIcon, Section, WidthRule,
    };

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

    let rows: Vec<Row> = state
        .matches
        .iter()
        .zip(&accessories)
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
        .collect();
    let sections = [Section {
        title: None,
        rows: &rows,
    }];

    let selected_index = state.selected_index.min(rows.len().saturating_sub(1));

    let (pct, min, max) = COMMAND_PALETTE_WIDTH;
    let spec = overlay_surface::OverlaySpec {
        anchor: Anchor::Centered {
            width: WidthRule { pct, min, max },
            dim_alpha: MODAL_DIM_ALPHA,
        },
        header: Header {
            glyph: Some('\u{276F}'),
            text: &input_text,
            placeholder: "Type a command...",
            caret: Some(
                state
                    .editable
                    .cursors()
                    .first()
                    .map(|c| c.column)
                    .unwrap_or(0),
            ),
            scope: None,
        },
        body: Body::List {
            sections: &sections,
            selected: FlatIndex(selected_index),
            scroll: state.scroll_offset,
            max_visible: COMMAND_PALETTE_MAX_VISIBLE,
        },
        footer: Some(overlay_surface::Footer {
            leading: "\u{2191}\u{2193} navigate \u{00b7} \u{21b5} run",
            trailing: "esc dismiss",
        }),
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

fn render_goto_line_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::GotoLineState,
    ctx: &ModalRenderCtx,
) {
    let colors = &ctx.colors;
    let line_height = ctx.line_height;
    let char_width = ctx.char_width;

    let (layout, w) = geometry::goto_line_layout(
        ctx.window_width,
        ctx.window_height,
        line_height,
        ctx.scale_factor,
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, "Go to Line", colors.fg);

    let input_r = layout.widget(w.input);
    TextFieldRenderer::render_modal_input(
        frame,
        painter,
        &state.editable,
        input_r,
        line_height,
        char_width,
        colors.input_bg,
        colors.fg,
        colors.highlight,
        colors.selection_bg,
        model.ui.cursor_visible,
        ctx.scale_factor,
    );
}

fn render_find_replace_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::FindReplaceState,
    ctx: &ModalRenderCtx,
) {
    use crate::model::ui::FindReplaceField;

    let colors = &ctx.colors;
    let line_height = ctx.line_height;
    let char_width = ctx.char_width;

    let (layout, w) = geometry::find_replace_layout(
        ctx.window_width,
        ctx.window_height,
        line_height,
        state.replace_mode,
        ctx.scale_factor,
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    let title = if state.replace_mode {
        "Find and Replace"
    } else {
        "Find"
    };
    painter.draw(frame, title_r.x, title_r.y, title, colors.fg);

    if let Some(label_idx) = w.find_label {
        let label_r = layout.widget(label_idx);
        let label_color = match state.focused_field {
            FindReplaceField::Query => colors.fg,
            FindReplaceField::Replace => colors.dim,
        };
        painter.draw(frame, label_r.x, label_r.y, "Find:", label_color);
    }

    let find_r = layout.widget(w.find_input);
    let find_cursor_visible =
        model.ui.cursor_visible && matches!(state.focused_field, FindReplaceField::Query);
    TextFieldRenderer::render_modal_input(
        frame,
        painter,
        &state.query_editable,
        find_r,
        line_height,
        char_width,
        colors.input_bg,
        colors.fg,
        colors.highlight,
        colors.selection_bg,
        find_cursor_visible,
        ctx.scale_factor,
    );

    if let Some(label_idx) = w.replace_label {
        let label_r = layout.widget(label_idx);
        let label_color = match state.focused_field {
            FindReplaceField::Replace => colors.fg,
            FindReplaceField::Query => colors.dim,
        };
        painter.draw(frame, label_r.x, label_r.y, "Replace:", label_color);
    }

    if let Some(input_idx) = w.replace_input {
        let repl_r = layout.widget(input_idx);
        let replace_cursor_visible =
            model.ui.cursor_visible && matches!(state.focused_field, FindReplaceField::Replace);
        TextFieldRenderer::render_modal_input(
            frame,
            painter,
            &state.replace_editable,
            repl_r,
            line_height,
            char_width,
            colors.input_bg,
            colors.fg,
            colors.highlight,
            colors.selection_bg,
            replace_cursor_visible,
            ctx.scale_factor,
        );
    }
}

/// Shared shell for the two "search a list, filtered by a text input" modals
/// (file finder and recent files): modal shell + title + filter input +
/// selectable list + "no results" fallback message.
///
/// The two modals differ only in title, item type, per-row rendering, and
/// empty-state copy, so those are the only things callers need to supply.
#[allow(clippy::too_many_arguments)]
fn render_search_list_modal<T>(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    ctx: &ModalRenderCtx,
    title: &str,
    input: &dyn TextFieldContent,
    input_is_empty: bool,
    empty_message: &str,
    items: &[T],
    selected_index: usize,
    max_visible_items: usize,
    mut render_row: impl FnMut(&mut Frame, &mut TextPainter, &T, usize, usize, usize, f32, u32, u32),
) {
    let colors = &ctx.colors;
    let line_height = ctx.line_height;
    let char_width = ctx.char_width;

    let (layout, w) = geometry::file_finder_layout(
        ctx.window_width,
        ctx.window_height,
        line_height,
        items.len(),
        !input_is_empty,
        ctx.scale_factor,
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, title, colors.fg);

    let input_r = layout.widget(w.input);
    TextFieldRenderer::render_modal_input(
        frame,
        painter,
        input,
        input_r,
        line_height,
        char_width,
        colors.input_bg,
        colors.fg,
        colors.highlight,
        colors.selection_bg,
        model.ui.cursor_visible,
        ctx.scale_factor,
    );

    let results_y = if let Some(list_idx) = w.list {
        layout.widget(list_idx).y
    } else {
        input_r.y + input_r.h + geometry::ModalSpacing::gap_md(ctx.scale_factor)
    };
    let dim_color = 0xFF888888;
    let list_layout = SelectableListLayout {
        x: layout.x,
        y: results_y,
        width: layout.w,
        row_height: line_height,
        max_visible_items,
        selection_inset: 4,
    };
    let list_colors = SelectableListColors {
        selection_bg: colors.selection_bg,
    };
    render_selectable_list(
        frame,
        items,
        selected_index,
        &list_layout,
        &list_colors,
        |frame, item, _actual_index, item_y, _is_selected| {
            render_row(
                frame, painter, item, item_y, layout.x, layout.w, char_width, colors.fg, dim_color,
            );
        },
    );

    if items.is_empty() && !input_is_empty {
        painter.draw(frame, layout.x + 12, results_y, empty_message, dim_color);
    }
}

fn render_file_finder_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::FileFinderState,
    ctx: &ModalRenderCtx,
) {
    let input_text = state.input();
    render_search_list_modal(
        frame,
        painter,
        model,
        ctx,
        "Go to File",
        &state.editable,
        input_text.is_empty(),
        "No files match your query",
        state.results.as_slice(),
        state.selected_index,
        10,
        |frame, painter, file_match, item_y, layout_x, layout_w, char_width, fg, dim| {
            let icon = crate::model::FileExtension::from_path(&file_match.path).icon();
            let icon_x = layout_x + 12;
            painter.draw(frame, icon_x, item_y, icon, fg);

            let name_x = layout_x + 36;
            painter.draw(frame, name_x, item_y, &file_match.filename, fg);

            let filename_width = (file_match.filename.len() as f32 * char_width) as usize;
            let path_x = name_x + filename_width + (char_width as usize * 2);
            let available_width = (layout_x + layout_w).saturating_sub(path_x + 16);
            let max_path_chars = (available_width as f32 / char_width) as usize;

            if max_path_chars > 5 {
                let path_display = if file_match.relative_path.chars().count() > max_path_chars {
                    let truncated: String = file_match
                        .relative_path
                        .chars()
                        .take(max_path_chars - 1)
                        .collect();
                    format!("{}…", truncated)
                } else {
                    file_match.relative_path.clone()
                };
                painter.draw(frame, path_x, item_y, &path_display, dim);
            }
        },
    );
}

fn render_recent_files_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::RecentFilesState,
    ctx: &ModalRenderCtx,
) {
    let filtered = state.filtered_entries();
    let input_text = state.input();
    render_search_list_modal(
        frame,
        painter,
        model,
        ctx,
        "Recent Files",
        &state.editable,
        input_text.is_empty(),
        "No recent files match your query",
        filtered.as_slice(),
        state.selected_index,
        10,
        |frame, painter, entry, item_y, layout_x, layout_w, char_width, fg, dim| {
            let icon = crate::model::FileExtension::from_path(&entry.path).icon();
            let icon_x = layout_x + 12;
            painter.draw(frame, icon_x, item_y, icon, fg);

            let display = entry.display_path();
            let name_x = layout_x + 36;
            painter.draw(frame, name_x, item_y, &display, fg);

            let time_str = entry.time_ago();
            let time_width = (time_str.len() as f32 * char_width) as usize;
            let time_x = (layout_x + layout_w).saturating_sub(time_width + 12);
            painter.draw(frame, time_x, item_y, &time_str, dim);
        },
    );
}

/// Render the active modal overlay.
///
/// Draws:
/// - Dimmed background over entire window
/// - Modal dialog box (centered)
/// - Modal content (title, input field, command list for palette)
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

    // 1. Dim background (40% black overlay). Skipped for the command
    // palette: it renders through `OverlaySurface`, which owns its own
    // backdrop dim as part of the component's chrome.
    if !matches!(modal, ModalState::CommandPalette(_)) {
        frame.dim(MODAL_DIM_ALPHA); // 102/255 ≈ 40% opacity
    }

    let ctx = ModalRenderCtx {
        window_width,
        window_height,
        line_height: painter.line_height(),
        char_width: painter.char_width(),
        colors: ModalColors::from_model(model),
        scale_factor: model.metrics.scale_factor,
    };

    // Handle different modal types
    match modal {
        ModalState::ThemePicker(state) => {
            render_theme_picker_modal(frame, painter, model, state, &ctx)
        }
        ModalState::CommandPalette(state) => {
            render_command_palette_modal(frame, painter, model, state, &ctx, overlay_mask_cache)
        }
        ModalState::GotoLine(state) => render_goto_line_modal(frame, painter, model, state, &ctx),
        ModalState::FindReplace(state) => {
            render_find_replace_modal(frame, painter, model, state, &ctx)
        }
        ModalState::FileFinder(state) => {
            render_file_finder_modal(frame, painter, model, state, &ctx)
        }
        ModalState::RecentFiles(state) => {
            render_recent_files_modal(frame, painter, model, state, &ctx)
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
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();

    // Semi-transparent overlay covering the entire window
    frame.dim(0x80); // 50% dim

    // Draw centered drop zone
    let text = model.ui.drop_state.display_text();
    let text_len = text.chars().count();

    let box_width = ((text_len as f32 + 4.0) * char_width).round() as usize;
    let box_height = line_height * 3;
    let box_x = (window_width.saturating_sub(box_width)) / 2;
    let box_y = (window_height.saturating_sub(box_height)) / 2;

    let bg_color = model.theme.overlay.background.to_argb_u32();
    let border_color = model.theme.overlay.highlight.to_argb_u32();
    let fg_color = model.theme.overlay.foreground.to_argb_u32();

    frame.draw_bordered_rect(box_x, box_y, box_width, box_height, bg_color, border_color);

    // Centered text
    let text_x = box_x + (box_width - (text_len as f32 * char_width).round() as usize) / 2;
    let text_y = box_y + line_height;

    painter.draw(frame, text_x, text_y, &text, fg_color);
}

/// Modal dim background alpha (102/255 ≈ 40% opacity)
const MODAL_DIM_ALPHA: u8 = 0x66;
