//! Active text-input caret geometry for platform text services.

use crate::csv::render::CsvRenderLayout;
use crate::model::ui::{GotoLineState, ModalState, RenameSymbolState};
use crate::model::{AppModel, FocusTarget};

use super::geometry::{GroupLayout, WidgetRect};
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

    if model.ui.focus == FocusTarget::FindBar {
        let state = model.ui.find_bar.as_ref()?;
        let options = super::find_bar::field_options(model, state.focused_field)?;
        return TextFieldRenderer::caret_rect(state.focused_editable(), &options);
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

    let cursor = editor.active_cursor();
    editor_text_rect_at(model, cursor.line, cursor.column, char_width, line_height)
}

/// The physical rect of an arbitrary `(line, column)` position in the
/// focused plain-text editor — the same geometry `active_text_input_rect`
/// uses for the live caret, generalized to any column so callers (e.g. the
/// completion popup, anchored at the query start rather than the cursor)
/// don't need their own copy of the viewport/layout math.
pub fn editor_text_rect_at(
    model: &AppModel,
    line: usize,
    column: usize,
    char_width: f32,
    line_height: usize,
) -> Option<WidgetRect> {
    let group = model.editor_area.focused_group()?;
    let editor = model.editor_area.focused_editor()?;
    let layout = GroupLayout::new(group, model, char_width);

    let document = editor
        .document_id
        .and_then(|id| model.editor_area.documents.get(&id))?;
    let viewport = editor.viewport_map(document);
    let (visual_row, visual_col) = viewport.display_position(document, line, column);
    let screen_row = viewport.visible_row_for_position(line, column);
    let y = screen_row
        .map(|row| {
            (layout.content_y() as f64 + viewport.row_pixel_offset(row, line_height as f64))
                .round()
                .max(layout.content_y() as f64) as usize
        })
        .unwrap_or_else(|| {
            if visual_row < viewport.top_line() {
                layout.content_y()
            } else {
                layout
                    .content_y()
                    .saturating_add(layout.content_h().saturating_sub(line_height))
            }
        });

    let x = (layout.text_start_x as f64 + viewport.column_pixel_offset(visual_col, char_width))
        .round()
        .max(0.0) as usize;
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

    let header = |model: &AppModel| {
        super::modal::modal_header_input_rect(model, width, height, scale_factor, char_width)
    };
    // Header inputs get the exact painted text box (no further inset);
    // field inputs keep the modal field padding.
    let (content, options): (&dyn super::TextFieldContent, TextFieldOptions) = match modal {
        ModalState::Settings(state) => {
            let rect = header(model)?;
            (
                &state.editable,
                TextFieldOptions::for_text_box(&state.editable, &rect, line_height, char_width),
            )
        }
        ModalState::CommandPalette(state) => {
            let rect = header(model)?;
            (
                &state.editable,
                TextFieldOptions::for_text_box(&state.editable, &rect, line_height, char_width),
            )
        }
        ModalState::FileFinder(state) => {
            let rect = header(model)?;
            (
                &state.editable,
                TextFieldOptions::for_text_box(&state.editable, &rect, line_height, char_width),
            )
        }
        ModalState::RecentFiles(state) => {
            let rect = header(model)?;
            (
                &state.editable,
                TextFieldOptions::for_text_box(&state.editable, &rect, line_height, char_width),
            )
        }
        ModalState::GotoLine(GotoLineState { editable, .. })
        | ModalState::RenameSymbol(RenameSymbolState { editable, .. }) => {
            let rect = super::modal::modal_field_input_rect(model, width, height, scale_factor, 0)?;
            (
                editable,
                TextFieldOptions::for_modal(editable, &rect, line_height, char_width, scale_factor),
            )
        }
        ModalState::ThemePicker(_)
        | ModalState::LspServers(_)
        | ModalState::LanguagePicker(_)
        | ModalState::FileConflict(_)
        | ModalState::UnsavedChanges(_) => return None,
    };

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
    let options =
        crate::csv::render::cell_text_field_options(&cell, line_height, char_width, edit.scroll_x);
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
        let mut model = AppModel::new(800, 600, 1.0);
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
        let mut model = AppModel::new(800, 600, 1.0);
        let mut state = GotoLineState::default();
        state.editable.set_content(&"1".repeat(100));
        model.ui.active_modal = Some(ModalState::GotoLine(state));

        let rect = active_text_input_rect(&model, 8.0, 20).expect("modal caret");
        let input = super::super::modal::modal_field_input_rect(&model, 800, 600, 1.0, 0)
            .expect("field input rect");

        assert!(rect.x >= input.x);
        assert!(rect.x < input.x + input.w);
        assert!(rect.y >= input.y);
        assert_ne!((rect.x, rect.y), (0, 0));
    }

    #[test]
    fn header_caret_starts_where_the_painter_starts_the_query_text() {
        // The command palette header draws a glyph before the query, so the
        // painted caret sits at pad + glyph + pad/2 from the header edge.
        // The IME rect must land there too — not pad + input_pad (the old
        // double inset), and not before the glyph.
        use crate::model::ui::CommandPaletteState;
        use crate::view::overlay_surface::header_pad_x;

        let mut model = AppModel::new(800, 600, 1.0);
        model
            .ui
            .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));
        let char_width = 8.0;
        let rect = active_text_input_rect(&model, char_width, 20).expect("modal caret");
        let header =
            super::super::modal::with_modal_overlay_layout(&model, 800, 600, 1.0, |_, l| l.header)
                .flatten()
                .expect("header rect");
        let pad = header_pad_x(1.0);
        assert_eq!(rect.x, header.x + pad + char_width as usize + pad / 2);
    }

    #[test]
    fn csv_editing_caret_uses_cell_editor_geometry() {
        let mut model = AppModel::new(800, 600, 1.0);
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
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.active_modal = Some(ModalState::ThemePicker(
            crate::model::ui::ThemePickerState::new(model.config.theme.clone()),
        ));

        assert_eq!(active_text_input_rect(&model, 8.0, 20), None);
    }
}
