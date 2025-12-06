//! Shared test helpers for integration tests
//!
//! Note: Functions may appear unused because each test file compiles separately.

#![allow(dead_code)]

use token::model::{
    AppModel, Cursor, Document, EditorArea, EditorState, Position, RectangleSelectionState,
    Selection, UiState, Viewport,
};
use token::theme::Theme;

/// Create a test model with given text and cursor position
pub fn test_model(text: &str, line: usize, column: usize) -> AppModel {
    let cursor = Cursor {
        line,
        column,
        desired_column: None,
    };
    let selection = Selection::new(Position::new(line, column));

    let document = Document::with_text(text);
    let editor = EditorState {
        id: None,
        document_id: None,
        cursors: vec![cursor],
        selections: vec![selection],
        viewport: Viewport {
            top_line: 0,
            left_column: 0,
            visible_lines: 25,
            visible_columns: 80,
        },
        scroll_padding: 1, // Default padding for tests
        rectangle_selection: RectangleSelectionState::default(),
    };

    let editor_area = EditorArea::single_document(document, editor);

    AppModel {
        editor_area,
        ui: UiState::new(),
        theme: Theme::default(),
        window_size: (800, 600),
        line_height: 20,
        char_width: 10.0,
    }
}

/// Helper to get buffer content as string
pub fn buffer_to_string(model: &AppModel) -> String {
    model.document().buffer.to_string()
}

/// Create a test model with given text and a selection (anchor to head)
/// The cursor will be at the head position
pub fn test_model_with_selection(
    text: &str,
    anchor_line: usize,
    anchor_col: usize,
    head_line: usize,
    head_col: usize,
) -> AppModel {
    let cursor = Cursor {
        line: head_line,
        column: head_col,
        desired_column: None,
    };
    let selection = Selection {
        anchor: Position::new(anchor_line, anchor_col),
        head: Position::new(head_line, head_col),
    };

    let document = Document::with_text(text);
    let editor = EditorState {
        id: None,
        document_id: None,
        cursors: vec![cursor],
        selections: vec![selection],
        viewport: Viewport {
            top_line: 0,
            left_column: 0,
            visible_lines: 25,
            visible_columns: 80,
        },
        scroll_padding: 1,
        rectangle_selection: RectangleSelectionState::default(),
    };

    let editor_area = EditorArea::single_document(document, editor);

    AppModel {
        editor_area,
        ui: UiState::new(),
        theme: Theme::default(),
        window_size: (800, 600),
        line_height: 20,
        char_width: 10.0,
    }
}
