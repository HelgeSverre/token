//! Modal overlay rendering (command palette, goto line, find/replace, etc.)

use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::geometry;
use super::selectable_list::{render_selectable_list, SelectableListColors, SelectableListLayout};
use super::text_field::TextFieldRenderer;

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

fn render_theme_picker_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::ThemePickerState,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    colors: &ModalColors,
) {
    use crate::theme::ThemeSource;

    let themes = &state.themes;

    let has_user = themes.iter().any(|t| t.source == ThemeSource::User);
    let has_builtin = themes.iter().any(|t| t.source == ThemeSource::Builtin);
    let section_count = has_user as usize + has_builtin as usize;
    let total_rows = themes.len() + section_count;

    let (layout, w) =
        geometry::theme_picker_layout(window_width, window_height, line_height, total_rows);

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, "Switch Theme", colors.fg);

    let list_r = layout.widget(w.list);
    let clamped_selected = state.selected_index.min(themes.len().saturating_sub(1));

    let mut current_y = list_r.y;
    let mut current_source: Option<ThemeSource> = None;
    let section_color = 0xFF666666;

    for (i, theme_info) in themes.iter().enumerate() {
        if current_source != Some(theme_info.source) {
            current_source = Some(theme_info.source);
            let header = match theme_info.source {
                ThemeSource::User => "User Themes",
                ThemeSource::Builtin => "Built-in Themes",
            };
            painter.draw(frame, layout.x + 12, current_y, header, section_color);
            current_y += line_height;
        }

        let is_selected = i == clamped_selected;

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

        current_y += line_height;
    }
}

fn render_command_palette_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::CommandPaletteState,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: &ModalColors,
) {
    use crate::commands::filter_commands;

    let input_text = state.input();
    let filtered_commands = filter_commands(&input_text);
    let max_visible_items = 8;

    let (layout, w) = geometry::command_palette_layout(
        window_width,
        window_height,
        line_height,
        filtered_commands.len(),
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, "Command Palette", colors.fg);

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
    );

    if let Some(list_idx) = w.list {
        let list_r = layout.widget(list_idx);
        let list_layout = SelectableListLayout {
            x: layout.x,
            y: list_r.y,
            width: layout.w,
            row_height: line_height,
            max_visible_items,
            selection_inset: 4,
        };
        let list_colors = SelectableListColors {
            selection_bg: colors.selection_bg,
        };
        let viewport = render_selectable_list(
            frame,
            filtered_commands.as_slice(),
            state.selected_index,
            &list_layout,
            &list_colors,
            |frame, cmd, _actual_index, item_y, _is_selected| {
                painter.draw(frame, layout.x + 16, item_y, cmd.label, colors.fg);

                if let Some(kb) = cmd.keybinding {
                    let kb_width = (kb.chars().count() as f32 * char_width).round() as usize;
                    let kb_x = layout.x + layout.w - kb_width - 16;
                    painter.draw(frame, kb_x, item_y, kb, colors.dim);
                }
            },
        );

        if viewport.items_after > 0 {
            let more_y = list_r.y + max_visible_items * line_height;
            let more_text = format!("... and {} more", viewport.items_after);
            painter.draw(frame, layout.x + 16, more_y, &more_text, colors.dim);
        }
    }
}

fn render_goto_line_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::GotoLineState,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: &ModalColors,
) {
    let (layout, w) = geometry::goto_line_layout(window_width, window_height, line_height);

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
    );
}

fn render_find_replace_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::FindReplaceState,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: &ModalColors,
) {
    use crate::model::ui::FindReplaceField;

    let (layout, w) =
        geometry::find_replace_layout(window_width, window_height, line_height, state.replace_mode);

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
        );
    }
}

fn render_file_finder_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::FileFinderState,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: &ModalColors,
) {
    let results = &state.results;
    let max_visible_items = 10;

    let (layout, w) = geometry::file_finder_layout(
        window_width,
        window_height,
        line_height,
        results.len(),
        !state.input().is_empty(),
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, "Go to File", colors.fg);

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
    );

    let results_y = if let Some(list_idx) = w.list {
        layout.widget(list_idx).y
    } else {
        input_r.y + input_r.h + geometry::ModalSpacing::GAP_MD
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
        results.as_slice(),
        state.selected_index,
        &list_layout,
        &list_colors,
        |frame, file_match, _actual_index, item_y, _is_selected| {
            let icon = crate::model::FileExtension::from_path(&file_match.path).icon();
            let icon_x = layout.x + 12;
            painter.draw(frame, icon_x, item_y, icon, colors.fg);

            let name_x = layout.x + 36;
            painter.draw(frame, name_x, item_y, &file_match.filename, colors.fg);

            let filename_width = (file_match.filename.len() as f32 * char_width) as usize;
            let path_x = name_x + filename_width + (char_width as usize * 2);
            let available_width = (layout.x + layout.w).saturating_sub(path_x + 16);
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
                painter.draw(frame, path_x, item_y, &path_display, dim_color);
            }
        },
    );

    if results.is_empty() && !state.input().is_empty() {
        painter.draw(
            frame,
            layout.x + 12,
            results_y,
            "No files match your query",
            dim_color,
        );
    }
}

fn render_recent_files_modal(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &crate::model::ui::RecentFilesState,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    char_width: f32,
    colors: &ModalColors,
) {
    let filtered = state.filtered_entries();
    let max_visible_items = 10;

    let (layout, w) = geometry::file_finder_layout(
        window_width,
        window_height,
        line_height,
        filtered.len(),
        !state.input().is_empty(),
    );

    render_modal_shell(frame, &layout, colors);

    let title_r = layout.widget(w.title);
    painter.draw(frame, title_r.x, title_r.y, "Recent Files", colors.fg);

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
    );

    let results_y = if let Some(list_idx) = w.list {
        layout.widget(list_idx).y
    } else {
        input_r.y + input_r.h + geometry::ModalSpacing::GAP_MD
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
        filtered.as_slice(),
        state.selected_index,
        &list_layout,
        &list_colors,
        |frame, entry, _actual_index, item_y, _is_selected| {
            let icon = crate::model::FileExtension::from_path(&entry.path).icon();
            let icon_x = layout.x + 12;
            painter.draw(frame, icon_x, item_y, icon, colors.fg);

            let display = entry.display_path();
            let name_x = layout.x + 36;
            painter.draw(frame, name_x, item_y, &display, colors.fg);

            let time_str = entry.time_ago();
            let time_width = (time_str.len() as f32 * char_width) as usize;
            let time_x = (layout.x + layout.w).saturating_sub(time_width + 12);
            painter.draw(frame, time_x, item_y, &time_str, dim_color);
        },
    );

    if filtered.is_empty() && !state.input().is_empty() {
        painter.draw(
            frame,
            layout.x + 12,
            results_y,
            "No recent files match your query",
            dim_color,
        );
    }
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
) {
    use crate::model::ModalState;
    let char_width = painter.char_width();
    let line_height = painter.line_height();

    let Some(ref modal) = model.ui.active_modal else {
        return;
    };

    // 1. Dim background (40% black overlay)
    frame.dim(MODAL_DIM_ALPHA); // 102/255 ≈ 40% opacity

    let colors = ModalColors::from_model(model);

    // Handle different modal types
    match modal {
        ModalState::ThemePicker(state) => render_theme_picker_modal(
            frame,
            painter,
            model,
            state,
            window_width,
            window_height,
            line_height,
            &colors,
        ),
        ModalState::CommandPalette(state) => render_command_palette_modal(
            frame,
            painter,
            model,
            state,
            window_width,
            window_height,
            line_height,
            char_width,
            &colors,
        ),
        ModalState::GotoLine(state) => render_goto_line_modal(
            frame,
            painter,
            model,
            state,
            window_width,
            window_height,
            line_height,
            char_width,
            &colors,
        ),
        ModalState::FindReplace(state) => render_find_replace_modal(
            frame,
            painter,
            model,
            state,
            window_width,
            window_height,
            line_height,
            char_width,
            &colors,
        ),
        ModalState::FileFinder(state) => render_file_finder_modal(
            frame,
            painter,
            model,
            state,
            window_width,
            window_height,
            line_height,
            char_width,
            &colors,
        ),
        ModalState::RecentFiles(state) => render_recent_files_modal(
            frame,
            painter,
            model,
            state,
            window_width,
            window_height,
            line_height,
            char_width,
            &colors,
        ),
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
