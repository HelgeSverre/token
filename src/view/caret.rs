//! Active text-input caret geometry for platform text services.

use crate::csv::render::CsvRenderLayout;
use crate::model::editor::TextViewportMap;
use crate::model::ui::{FindReplaceField, ModalState};
use crate::model::{AppModel, FocusTarget};

use super::geometry::{self, char_col_to_visual_col, column_to_pixel_x, GroupLayout, WidgetRect};
use super::{TextFieldOptions, TextFieldRenderer};

const CARET_WIDTH: usize = 2;

/// Return the window-relative physical rectangle of the text caret that owns
/// keyboard input. Platform text services use this to position accessories and
/// IME candidate windows.
pub fn active_text_input_rect(
    model: &AppModel,
    char_width: f32,
    line_height: usize,
) -> Option<WidgetRect> {
    if let Some(modal) = &model.ui.active_modal {
        return modal_caret_rect(model, modal, char_width, line_height);
    }

    if model.ui.focus != FocusTarget::Editor {
        return None;
    }

    let group = model.editor_area.focused_group()?;
    let editor = model.editor_area.focused_editor()?;
    let layout = GroupLayout::new(group, model, char_width);

    if let Some(csv) = editor.view_mode.as_csv() {
        let edit = csv.editing.as_ref()?;
        return csv_caret_rect(csv, edit, &layout, char_width, line_height);
    }

    if !editor.is_plain_text_mode() {
        return None;
    }

    let document = editor
        .document_id
        .and_then(|id| model.editor_area.documents.get(&id))?;
    let cursor = editor.active_cursor();
    let viewport = TextViewportMap::new(&editor.viewport, document.line_count());
    let screen_row = viewport.visible_row_for_doc_line(cursor.line);
    let y = screen_row
        .map(|row| layout.content_y() + row * line_height)
        .unwrap_or_else(|| {
            if cursor.line < viewport.top_line() {
                layout.content_y()
            } else {
                layout
                    .content_y()
                    .saturating_add(layout.content_h().saturating_sub(line_height))
            }
        });

    let line = document.get_line_cow(cursor.line).unwrap_or_default();
    let visual_col = char_col_to_visual_col(&line, cursor.column);
    let x = column_to_pixel_x(
        visual_col,
        viewport.left_column(),
        layout.text_start_x,
        char_width,
    );
    let max_x = layout
        .rect_x()
        .saturating_add(layout.rect_w())
        .saturating_sub(CARET_WIDTH);

    Some(WidgetRect {
        x: x.clamp(layout.text_start_x, max_x.max(layout.text_start_x)),
        y,
        w: CARET_WIDTH,
        h: line_height.max(1),
    })
}

fn modal_caret_rect(
    model: &AppModel,
    modal: &ModalState,
    char_width: f32,
    line_height: usize,
) -> Option<WidgetRect> {
    let width = model.window_size.0 as usize;
    let height = model.window_size.1 as usize;
    let scale_factor = model.metrics.scale_factor;

    let (content, input_rect) = match modal {
        ModalState::CommandPalette(state) => {
            let input_rect = super::modal::command_palette_input_rect(width, height, scale_factor);
            (&state.editable, input_rect)
        }
        ModalState::GotoLine(state) => {
            let (layout, widgets) =
                geometry::goto_line_layout(width, height, line_height, scale_factor);
            (&state.editable, *layout.widget(widgets.input))
        }
        ModalState::FindReplace(state) => {
            let (layout, widgets) = geometry::find_replace_layout(
                width,
                height,
                line_height,
                state.replace_mode,
                scale_factor,
            );
            match state.focused_field {
                FindReplaceField::Query => {
                    (&state.query_editable, *layout.widget(widgets.find_input))
                }
                FindReplaceField::Replace => {
                    let input = widgets.replace_input?;
                    (&state.replace_editable, *layout.widget(input))
                }
            }
        }
        ModalState::FileFinder(state) => {
            let (layout, widgets) =
                geometry::file_finder_layout(width, height, line_height, 0, false, scale_factor);
            (&state.editable, *layout.widget(widgets.input))
        }
        ModalState::RecentFiles(state) => {
            let (layout, widgets) =
                geometry::file_finder_layout(width, height, line_height, 0, false, scale_factor);
            (&state.editable, *layout.widget(widgets.input))
        }
        ModalState::ThemePicker(_) => return None,
    };

    let options =
        TextFieldOptions::for_modal(content, &input_rect, line_height, char_width, scale_factor);
    TextFieldRenderer::caret_rect(content, &options)
}

fn csv_caret_rect(
    csv: &crate::csv::CsvState,
    edit: &crate::csv::CellEditState,
    group: &GroupLayout,
    char_width: f32,
    line_height: usize,
) -> Option<WidgetRect> {
    let layout = CsvRenderLayout::calculate(
        csv,
        group.rect_x(),
        group.rect_w(),
        group.content_y(),
        line_height,
        char_width,
    );
    let cell = layout.cell_editor_rect(csv, edit.position, line_height)?;
    let options = TextFieldOptions {
        x: cell.x as usize + 4,
        y: cell.y as usize + 1,
        width: (cell.width as usize).saturating_sub(8),
        height: line_height.saturating_sub(2),
        char_width,
        scroll_x: edit.scroll_x,
        ..TextFieldOptions::default()
    };

    TextFieldRenderer::caret_rect(&edit.editable, &options)
}

#[cfg(test)]
mod tests {
    use ropey::Rope;

    use crate::csv::{CsvData, CsvState, Delimiter};
    use crate::model::editor::ViewMode;
    use crate::model::editor::{Cursor, Position, Selection};
    use crate::model::editor_area::Rect;
    use crate::model::ui::{GotoLineState, ModalState};
    use crate::model::AppModel;

    use super::*;

    #[test]
    fn editor_caret_accounts_for_tabs_scroll_and_group_offset() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().buffer = Rope::from("a\tb\n");
        model.editor_mut().cursors = vec![Cursor::at(0, 2)];
        model.editor_mut().selections = vec![Selection::new(Position::new(0, 2))];
        model.editor_mut().viewport.left_column = 2;
        model.editor_mut().viewport.visible_lines = 20;
        model
            .editor_area
            .focused_group_mut()
            .expect("focused group")
            .rect = Rect::new(50.0, 20.0, 600.0, 400.0);

        let rect = active_text_input_rect(&model, 8.0, 20).expect("text caret");
        let group = GroupLayout::new(model.editor_area.focused_group().unwrap(), &model, 8.0);

        assert_eq!(rect.x, group.text_start_x + 16);
        assert_eq!(rect.y, group.content_y());
        assert_eq!((rect.w, rect.h), (2, 20));
    }

    #[test]
    fn modal_caret_uses_modal_input_and_scroll_geometry() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let mut state = GotoLineState::default();
        state.editable.set_content(&"1".repeat(100));
        model.ui.active_modal = Some(ModalState::GotoLine(state));

        let rect = active_text_input_rect(&model, 8.0, 20).expect("modal caret");
        let (layout, widgets) = geometry::goto_line_layout(800, 600, 20, 1.0);
        let input = layout.widget(widgets.input);

        assert!(rect.x >= input.x + geometry::ModalSpacing::input_pad_x(1.0));
        assert!(rect.x < input.x + input.w);
        assert!(rect.y >= input.y);
        assert_ne!((rect.x, rect.y), (0, 0));
    }

    #[test]
    fn csv_editing_caret_uses_cell_editor_geometry() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let mut csv = CsvState::new(
            CsvData::from_rows(vec![vec!["first".into(), "second".into()]]),
            Delimiter::Comma,
        );
        csv.viewport.visible_rows = 10;
        csv.select_cell(0, 1);
        csv.start_editing();
        model.editor_mut().view_mode = ViewMode::Csv(Box::new(csv));
        model
            .editor_area
            .focused_group_mut()
            .expect("focused group")
            .rect = Rect::new(40.0, 20.0, 600.0, 400.0);

        let rect = active_text_input_rect(&model, 8.0, 20).expect("CSV editing caret");
        let group = GroupLayout::new(model.editor_area.focused_group().unwrap(), &model, 8.0);

        assert!(rect.x > group.rect_x());
        assert!(rect.y > group.content_y());
        assert_ne!((rect.x, rect.y), (0, 0));
    }

    #[test]
    fn non_text_modal_has_no_caret_rect() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.active_modal = Some(ModalState::ThemePicker(
            crate::model::ui::ThemePickerState::new(model.config.theme.clone()),
        ));

        assert_eq!(active_text_input_rect(&model, 8.0, 20), None);
    }
}
