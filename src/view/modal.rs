//! Modal overlay rendering (command palette, goto line, find/replace, etc.)

use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::geometry;
use super::text_field::{TextFieldOptions, TextFieldRenderer};

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
    use crate::commands::filter_commands;
    use crate::model::ModalState;
    use crate::theme::ThemeSource;
    let char_width = painter.char_width();
    let line_height = painter.line_height();

    let Some(ref modal) = model.ui.active_modal else {
        return;
    };

    // 1. Dim background (40% black overlay)
    frame.dim(MODAL_DIM_ALPHA); // 102/255 ≈ 40% opacity

    // Theme colors
    let bg_color = model.theme.overlay.background.to_argb_u32();
    let fg_color = model.theme.overlay.foreground.to_argb_u32();
    let highlight_color = model.theme.overlay.highlight.to_argb_u32();
    let dim_color = model.theme.overlay.foreground.with_alpha(128).to_argb_u32();
    let selection_bg = model.theme.overlay.selection_background.to_argb_u32();
    let input_bg = model.theme.overlay.input_background.to_argb_u32();
    let border_color = model
        .theme
        .overlay
        .border
        .map(|c| c.to_argb_u32())
        .unwrap_or(0xFF444444);

    // Handle different modal types
    match modal {
        ModalState::ThemePicker(state) => {
            let themes = &state.themes;

            let has_user = themes.iter().any(|t| t.source == ThemeSource::User);
            let has_builtin = themes.iter().any(|t| t.source == ThemeSource::Builtin);
            let section_count = has_user as usize + has_builtin as usize;
            let total_rows = themes.len() + section_count;

            let (layout, w) =
                geometry::theme_picker_layout(window_width, window_height, line_height, total_rows);

            frame.draw_bordered_rect(
                layout.x,
                layout.y,
                layout.w,
                layout.h,
                bg_color,
                border_color,
            );

            // Title
            let title_r = layout.widget(w.title);
            painter.draw(frame, title_r.x, title_r.y, "Switch Theme", fg_color);

            // Theme list with sections
            let list_r = layout.widget(w.list);
            let clamped_selected = state.selected_index.min(themes.len().saturating_sub(1));

            let mut current_y = list_r.y;
            let mut current_source: Option<ThemeSource> = None;
            let dim_color = 0xFF666666;

            for (i, theme_info) in themes.iter().enumerate() {
                if current_source != Some(theme_info.source) {
                    current_source = Some(theme_info.source);
                    let header = match theme_info.source {
                        ThemeSource::User => "User Themes",
                        ThemeSource::Builtin => "Built-in Themes",
                    };
                    painter.draw(frame, layout.x + 12, current_y, header, dim_color);
                    current_y += line_height;
                }

                let is_selected = i == clamped_selected;

                if is_selected {
                    frame.fill_rect_px(
                        layout.x + 4,
                        current_y,
                        layout.w - 8,
                        line_height,
                        selection_bg,
                    );
                }

                let label_x = layout.x + 24;
                painter.draw(frame, label_x, current_y, &theme_info.name, fg_color);

                if model.theme.name == theme_info.name || model.config.theme == theme_info.id {
                    let check_x = layout.x + layout.w - 30;
                    painter.draw(frame, check_x, current_y, "✓", highlight_color);
                }

                current_y += line_height;
            }
        }

        ModalState::CommandPalette(state) => {
            let input_text = state.input();
            let filtered_commands = filter_commands(&input_text);
            let max_visible_items = 8;

            let (layout, w) = geometry::command_palette_layout(
                window_width,
                window_height,
                line_height,
                filtered_commands.len(),
            );

            frame.draw_bordered_rect(
                layout.x,
                layout.y,
                layout.w,
                layout.h,
                bg_color,
                border_color,
            );

            // Title
            let title_r = layout.widget(w.title);
            painter.draw(frame, title_r.x, title_r.y, "Command Palette", fg_color);

            // Input field
            let input_r = layout.widget(w.input);
            frame.fill_rect_px(input_r.x, input_r.y, input_r.w, input_r.h, input_bg);

            let padx = geometry::ModalSpacing::INPUT_PAD_X;
            let text_x = input_r.x + padx;
            let text_y = input_r.y + (input_r.h.saturating_sub(line_height)) / 2;
            let text_width = input_r.w.saturating_sub(padx * 2);
            let opts = TextFieldOptions {
                x: text_x,
                y: text_y,
                width: text_width,
                height: line_height,
                char_width,
                text_color: fg_color,
                cursor_color: highlight_color,
                selection_color: selection_bg,
                cursor_visible: model.ui.cursor_visible,
                scroll_x: 0,
            };
            TextFieldRenderer::render(frame, painter, &state.editable, &opts);

            // Command list
            if let Some(list_idx) = w.list {
                let list_r = layout.widget(list_idx);
                let total_items = filtered_commands.len();
                let clamped_selected = state.selected_index.min(total_items.saturating_sub(1));

                let scroll_offset = if clamped_selected >= max_visible_items {
                    clamped_selected + 1 - max_visible_items
                } else {
                    0
                };

                for (i, cmd) in filtered_commands
                    .iter()
                    .skip(scroll_offset)
                    .take(max_visible_items)
                    .enumerate()
                {
                    let actual_index = scroll_offset + i;
                    let item_y = list_r.y + i * line_height;
                    let is_selected = actual_index == clamped_selected;

                    if is_selected {
                        frame.fill_rect_px(
                            layout.x + 4,
                            item_y,
                            layout.w - 8,
                            line_height,
                            selection_bg,
                        );
                    }

                    painter.draw(frame, layout.x + 16, item_y, cmd.label, fg_color);

                    if let Some(kb) = cmd.keybinding {
                        let kb_width = (kb.chars().count() as f32 * char_width).round() as usize;
                        let kb_x = layout.x + layout.w - kb_width - 16;
                        painter.draw(frame, kb_x, item_y, kb, dim_color);
                    }
                }

                let items_after = total_items.saturating_sub(scroll_offset + max_visible_items);
                if items_after > 0 {
                    let more_y = list_r.y + max_visible_items * line_height;
                    let more_text = format!("... and {} more", items_after);
                    painter.draw(frame, layout.x + 16, more_y, &more_text, dim_color);
                }
            }
        }

        ModalState::GotoLine(state) => {
            let (layout, w) = geometry::goto_line_layout(window_width, window_height, line_height);

            frame.draw_bordered_rect(
                layout.x,
                layout.y,
                layout.w,
                layout.h,
                bg_color,
                border_color,
            );

            // Title
            let title_r = layout.widget(w.title);
            painter.draw(frame, title_r.x, title_r.y, "Go to Line", fg_color);

            // Input field
            let input_r = layout.widget(w.input);
            frame.fill_rect_px(input_r.x, input_r.y, input_r.w, input_r.h, input_bg);

            let padx = geometry::ModalSpacing::INPUT_PAD_X;
            let text_x = input_r.x + padx;
            let text_y = input_r.y + (input_r.h.saturating_sub(line_height)) / 2;
            let text_width = input_r.w.saturating_sub(padx * 2);
            let opts = TextFieldOptions {
                x: text_x,
                y: text_y,
                width: text_width,
                height: line_height,
                char_width,
                text_color: fg_color,
                cursor_color: highlight_color,
                selection_color: selection_bg,
                cursor_visible: model.ui.cursor_visible,
                scroll_x: 0,
            };
            TextFieldRenderer::render(frame, painter, &state.editable, &opts);
        }

        ModalState::FindReplace(state) => {
            let (layout, w) = geometry::find_replace_layout(
                window_width,
                window_height,
                line_height,
                state.replace_mode,
            );

            frame.draw_bordered_rect(
                layout.x,
                layout.y,
                layout.w,
                layout.h,
                bg_color,
                border_color,
            );

            // Title
            let title_r = layout.widget(w.title);
            let title = if state.replace_mode {
                "Find and Replace"
            } else {
                "Find"
            };
            painter.draw(frame, title_r.x, title_r.y, title, fg_color);

            let padx = geometry::ModalSpacing::INPUT_PAD_X;

            // Find label (only in replace mode)
            if let Some(label_idx) = w.find_label {
                let label_r = layout.widget(label_idx);
                let label_color = match state.focused_field {
                    crate::model::ui::FindReplaceField::Query => fg_color,
                    crate::model::ui::FindReplaceField::Replace => dim_color,
                };
                painter.draw(frame, label_r.x, label_r.y, "Find:", label_color);
            }

            // Find input
            let find_r = layout.widget(w.find_input);
            frame.fill_rect_px(find_r.x, find_r.y, find_r.w, find_r.h, input_bg);

            let find_cursor_visible = model.ui.cursor_visible
                && matches!(
                    state.focused_field,
                    crate::model::ui::FindReplaceField::Query
                );
            let find_opts = TextFieldOptions {
                x: find_r.x + padx,
                y: find_r.y + (find_r.h.saturating_sub(line_height)) / 2,
                width: find_r.w.saturating_sub(padx * 2),
                height: line_height,
                char_width,
                text_color: fg_color,
                cursor_color: highlight_color,
                selection_color: selection_bg,
                cursor_visible: find_cursor_visible,
                scroll_x: 0,
            };
            TextFieldRenderer::render(frame, painter, &state.query_editable, &find_opts);

            // Replace label + input (only in replace mode)
            if let Some(label_idx) = w.replace_label {
                let label_r = layout.widget(label_idx);
                let label_color = match state.focused_field {
                    crate::model::ui::FindReplaceField::Replace => fg_color,
                    crate::model::ui::FindReplaceField::Query => dim_color,
                };
                painter.draw(frame, label_r.x, label_r.y, "Replace:", label_color);
            }
            if let Some(input_idx) = w.replace_input {
                let repl_r = layout.widget(input_idx);
                frame.fill_rect_px(repl_r.x, repl_r.y, repl_r.w, repl_r.h, input_bg);

                let replace_cursor_visible = model.ui.cursor_visible
                    && matches!(
                        state.focused_field,
                        crate::model::ui::FindReplaceField::Replace
                    );
                let replace_opts = TextFieldOptions {
                    x: repl_r.x + padx,
                    y: repl_r.y + (repl_r.h.saturating_sub(line_height)) / 2,
                    width: repl_r.w.saturating_sub(padx * 2),
                    height: line_height,
                    char_width,
                    text_color: fg_color,
                    cursor_color: highlight_color,
                    selection_color: selection_bg,
                    cursor_visible: replace_cursor_visible,
                    scroll_x: 0,
                };
                TextFieldRenderer::render(frame, painter, &state.replace_editable, &replace_opts);
            }
        }

        ModalState::FileFinder(state) => {
            let results = &state.results;
            let max_visible_items = 10;

            let (layout, w) = geometry::file_finder_layout(
                window_width,
                window_height,
                line_height,
                results.len(),
                !state.input().is_empty(),
            );

            frame.draw_bordered_rect(
                layout.x,
                layout.y,
                layout.w,
                layout.h,
                bg_color,
                border_color,
            );

            // Title
            let title_r = layout.widget(w.title);
            painter.draw(frame, title_r.x, title_r.y, "Go to File", fg_color);

            // Input field
            let input_r = layout.widget(w.input);
            frame.fill_rect_px(input_r.x, input_r.y, input_r.w, input_r.h, input_bg);

            let padx = geometry::ModalSpacing::INPUT_PAD_X;
            let text_x = input_r.x + padx;
            let text_y = input_r.y + (input_r.h.saturating_sub(line_height)) / 2;
            let text_width = input_r.w.saturating_sub(padx * 2);
            let opts = TextFieldOptions {
                x: text_x,
                y: text_y,
                width: text_width,
                height: line_height,
                char_width,
                text_color: fg_color,
                cursor_color: highlight_color,
                selection_color: selection_bg,
                cursor_visible: model.ui.cursor_visible,
                scroll_x: 0,
            };
            TextFieldRenderer::render(frame, painter, &state.editable, &opts);

            // Results list
            let results_y = if let Some(list_idx) = w.list {
                layout.widget(list_idx).y
            } else {
                input_r.y + input_r.h + geometry::ModalSpacing::GAP_MD
            };
            let clamped_selected = state.selected_index.min(results.len().saturating_sub(1));
            let dim_color = 0xFF888888; // Dimmed color for relative path

            // Compute scroll offset to keep selected item visible
            let scroll_offset = if clamped_selected >= max_visible_items {
                clamped_selected + 1 - max_visible_items
            } else {
                0
            };

            for (i, file_match) in results
                .iter()
                .skip(scroll_offset)
                .take(max_visible_items)
                .enumerate()
            {
                let actual_index = scroll_offset + i;
                let item_y = results_y + i * line_height;
                let is_selected = actual_index == clamped_selected;

                // Selection highlight
                if is_selected {
                    frame.fill_rect_px(
                        layout.x + 4,
                        item_y,
                        layout.w - 8,
                        line_height,
                        selection_bg,
                    );
                }

                // File icon
                let icon = crate::model::FileExtension::from_path(&file_match.path).icon();
                let icon_x = layout.x + 12;
                painter.draw(frame, icon_x, item_y, icon, fg_color);

                // Filename
                let name_x = layout.x + 36;
                painter.draw(frame, name_x, item_y, &file_match.filename, fg_color);

                // Relative path (dimmed, after filename) - truncate if needed
                let filename_width = (file_match.filename.len() as f32 * char_width) as usize;
                let path_x = name_x + filename_width + (char_width as usize * 2);
                let available_width = (layout.x + layout.w).saturating_sub(path_x + 16);
                let max_path_chars = (available_width as f32 / char_width) as usize;

                if max_path_chars > 5 {
                    let path_display = if file_match.relative_path.chars().count() > max_path_chars
                    {
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
            }

            // Show "No matches" if results are empty and query is not empty
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

        ModalState::RecentFiles(state) => {
            let filtered = state.filtered_entries();
            let max_visible_items = 10;

            let (layout, w) = geometry::file_finder_layout(
                window_width,
                window_height,
                line_height,
                filtered.len(),
                !state.input().is_empty(),
            );

            frame.draw_bordered_rect(
                layout.x,
                layout.y,
                layout.w,
                layout.h,
                bg_color,
                border_color,
            );

            // Title
            let title_r = layout.widget(w.title);
            painter.draw(frame, title_r.x, title_r.y, "Recent Files", fg_color);

            // Input field
            let input_r = layout.widget(w.input);
            frame.fill_rect_px(input_r.x, input_r.y, input_r.w, input_r.h, input_bg);

            let padx = geometry::ModalSpacing::INPUT_PAD_X;
            let text_x = input_r.x + padx;
            let text_y = input_r.y + (input_r.h.saturating_sub(line_height)) / 2;
            let text_width = input_r.w.saturating_sub(padx * 2);
            let opts = TextFieldOptions {
                x: text_x,
                y: text_y,
                width: text_width,
                height: line_height,
                char_width,
                text_color: fg_color,
                cursor_color: highlight_color,
                selection_color: selection_bg,
                cursor_visible: model.ui.cursor_visible,
                scroll_x: 0,
            };
            TextFieldRenderer::render(frame, painter, &state.editable, &opts);

            // Results list
            let results_y = if let Some(list_idx) = w.list {
                layout.widget(list_idx).y
            } else {
                input_r.y + input_r.h + geometry::ModalSpacing::GAP_MD
            };
            let clamped_selected = state.selected_index.min(filtered.len().saturating_sub(1));
            let dim_color = 0xFF888888;

            let scroll_offset = if clamped_selected >= max_visible_items {
                clamped_selected + 1 - max_visible_items
            } else {
                0
            };

            for (i, entry) in filtered
                .iter()
                .skip(scroll_offset)
                .take(max_visible_items)
                .enumerate()
            {
                let actual_index = scroll_offset + i;
                let item_y = results_y + i * line_height;
                let is_selected = actual_index == clamped_selected;

                if is_selected {
                    frame.fill_rect_px(
                        layout.x + 4,
                        item_y,
                        layout.w - 8,
                        line_height,
                        selection_bg,
                    );
                }

                let icon = crate::model::FileExtension::from_path(&entry.path).icon();
                let icon_x = layout.x + 12;
                painter.draw(frame, icon_x, item_y, icon, fg_color);

                let display = entry.display_path();
                let name_x = layout.x + 36;
                painter.draw(frame, name_x, item_y, &display, fg_color);

                // Time ago (right-aligned, dimmed)
                let time_str = entry.time_ago();
                let time_width = (time_str.len() as f32 * char_width) as usize;
                let time_x = (layout.x + layout.w).saturating_sub(time_width + 12);
                painter.draw(frame, time_x, item_y, &time_str, dim_color);
            }

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
