#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use anyhow::Result;
use std::path::PathBuf;
use winit::event_loop::EventLoop;

mod app;
#[cfg(debug_assertions)]
mod debug_dump;
mod input;
mod perf;
mod view;

use app::App;

// ============================================================================
// MAIN - Entry point
// ============================================================================

fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    env_logger::init();

    // TODO: accept multiple files and open tabs for each in the first editorgroup
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let file_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let event_loop = EventLoop::new()?;
    let mut app = App::new(800, 600, file_path);

    event_loop.run_app(&mut app)?;

    Ok(())
}

// ============================================================================
// TESTS - Keyboard handling tests that require handle_key()
// TODO: Find a way to move it into test module instead of main.rs
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::input::handle_key;
    use token::messages::{DocumentMsg, EditorMsg, Msg};
    use token::model::{
        AppModel, Cursor, Document, EditorArea, EditorState, Position, RectangleSelectionState,
        Selection, UiState, Viewport,
    };
    use token::theme::Theme;
    use token::update::update;
    use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

    /// Create a test model with given text and a selection (anchor to head)
    /// The cursor will be at the head position
    fn test_model_with_selection(
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
            active_cursor_index: 0,
            viewport: Viewport {
                top_line: 0,
                left_column: 0,
                visible_lines: 25,
                visible_columns: 80,
            },
            scroll_padding: 1,
            rectangle_selection: RectangleSelectionState::default(),
            occurrence_state: None,
            selection_history: Vec::new(),
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

    // ========================================================================
    // Arrow Keys with Selection Tests
    // These tests require handle_key() which is in the binary, not the library
    // ========================================================================

    #[test]
    fn test_left_arrow_with_selection_jumps_to_start() {
        // When text is selected and Left is pressed, cursor should go to selection START
        // Text: "hello world" with "llo wo" selected (columns 2-8)
        let mut model = test_model_with_selection("hello world\n", 0, 2, 0, 8);
        // Selection: anchor at col 2, head/cursor at col 8

        // Press Left (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowLeft),
            PhysicalKey::Code(KeyCode::ArrowLeft),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be at selection START (column 2), not moved left from 8
        assert_eq!(
            model.editor().active_cursor().column,
            2,
            "Cursor should jump to selection start (col 2), not stay at col 8 or move to col 7"
        );
    }

    #[test]
    fn test_right_arrow_with_selection_jumps_to_end() {
        // When text is selected and Right is pressed, cursor should go to selection END
        // Text: "hello world" with "llo wo" selected (columns 2-8)
        let mut model = test_model_with_selection("hello world\n", 0, 2, 0, 8);

        // Press Right (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowRight),
            PhysicalKey::Code(KeyCode::ArrowRight),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be at selection END (column 8), not moved right from 8
        assert_eq!(
            model.editor().active_cursor().column,
            8,
            "Cursor should jump to selection end (col 8), not move to col 9"
        );
    }

    #[test]
    fn test_up_arrow_with_selection_moves_from_start() {
        // When text is selected and Up is pressed, cursor should:
        // 1. Jump to selection START
        // 2. Move up one line from there
        // Selection spans line 1, cols 2-8
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 1, 2, 1, 8);
        // Cursor is at line 1, col 8 (head of selection)

        // Press Up (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowUp),
            PhysicalKey::Code(KeyCode::ArrowUp),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be on line 0 (moved up from line 1)
        assert_eq!(
            model.editor().active_cursor().line,
            0,
            "Cursor should move up to line 0"
        );
        // Cursor should be at column 2 (selection start column)
        assert_eq!(
            model.editor().active_cursor().column,
            2,
            "Cursor should be at column 2 (selection start column)"
        );
    }

    #[test]
    fn test_down_arrow_with_selection_moves_from_end() {
        // When text is selected and Down is pressed, cursor should:
        // 1. Jump to selection END
        // 2. Move down one line from there
        // Selection spans line 1, cols 2-8
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 1, 2, 1, 8);
        // Cursor is at line 1, col 8 (head of selection)

        // Press Down (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowDown),
            PhysicalKey::Code(KeyCode::ArrowDown),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be on line 2 (moved down from line 1)
        assert_eq!(
            model.editor().active_cursor().line,
            2,
            "Cursor should move down to line 2"
        );
        // Cursor should be at column 8 (selection end column)
        assert_eq!(
            model.editor().active_cursor().column,
            8,
            "Cursor should be at column 8 (selection end column)"
        );
    }

    // ========================================================================
    // Home/End with Selection Tests
    // ========================================================================

    #[test]
    fn test_home_with_selection_uses_head_line() {
        // Home should cancel selection and go to start of line where HEAD is
        // Selection: anchor at (0, 5), head at (1, 8)
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 0, 5, 1, 8);
        // Head is on line 1

        // Press Home
        handle_key(
            &mut model,
            Key::Named(NamedKey::Home),
            PhysicalKey::Code(KeyCode::Home),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should stay on line 1 (where head was)
        assert_eq!(
            model.editor().active_cursor().line,
            1,
            "Cursor should stay on line 1 (head line)"
        );
        // Cursor should be at start of line (smart home: first non-ws char, but for "foo" that's 0)
        assert_eq!(
            model.editor().active_cursor().column,
            0,
            "Cursor should be at start of line"
        );
    }

    #[test]
    fn test_end_with_selection_uses_head_line() {
        // End should cancel selection and go to end of line where HEAD is
        // Selection: anchor at (0, 5), head at (1, 2)
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 0, 5, 1, 2);
        // Head is on line 1

        // Press End
        handle_key(
            &mut model,
            Key::Named(NamedKey::End),
            PhysicalKey::Code(KeyCode::End),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should stay on line 1 (where head was)
        assert_eq!(
            model.editor().active_cursor().line,
            1,
            "Cursor should stay on line 1 (head line)"
        );
        // Cursor should be at end of line 1 ("foo bar baz" has length 11)
        assert_eq!(
            model.editor().active_cursor().column,
            11,
            "Cursor should be at end of line (col 11)"
        );
    }

    // ========================================================================
    // PageUp/PageDown with Selection Tests
    // ========================================================================

    #[test]
    fn test_pageup_with_selection_moves_from_start() {
        // PageUp should cancel selection and move up from selection START
        // Create text with many lines
        let text = (0..30).map(|i| format!("line {}\n", i)).collect::<String>();
        // Selection: anchor at (15, 2), head at (15, 5) - both on line 15
        let mut model = test_model_with_selection(&text, 15, 2, 15, 5);
        model.editor_mut().viewport.visible_lines = 10;

        // Press PageUp
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should have moved up from selection start (line 15, col 2)
        // PageUp moves ~8 lines (visible_lines - 2)
        assert!(
            model.editor().active_cursor().line < 15,
            "Cursor should have moved up from line 15"
        );
        // Column should be from selection start (col 2)
        assert_eq!(
            model.editor().active_cursor().column,
            2,
            "Cursor column should be at selection start col (2)"
        );
    }

    #[test]
    fn test_pagedown_with_selection_moves_from_end() {
        // PageDown should cancel selection and move down from selection END
        // Create text with many lines
        let text = (0..30).map(|i| format!("line {}\n", i)).collect::<String>();
        // Selection: anchor at (5, 2), head at (5, 5) - both on line 5
        let mut model = test_model_with_selection(&text, 5, 2, 5, 5);
        model.editor_mut().viewport.visible_lines = 10;

        // Press PageDown
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageDown),
            PhysicalKey::Code(KeyCode::PageDown),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should have moved down from selection end (line 5, col 5)
        // PageDown moves ~8 lines (visible_lines - 2)
        assert!(
            model.editor().active_cursor().line > 5,
            "Cursor should have moved down from line 5"
        );
        // Column should be from selection end (col 5)
        assert_eq!(
            model.editor().active_cursor().column,
            5,
            "Cursor column should be at selection end col (5)"
        );
    }

    // ========================================================================
    // Large Document Viewport Focus Tests
    // ========================================================================

    #[test]
    fn test_select_all_then_right_arrow_scrolls_to_end() {
        // Create a 500-line document (each line has newline, so 500 lines total, last is empty)
        let text = (0..500)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_selection(&text, 0, 0, 0, 0);
        model.editor_mut().viewport.visible_lines = 25;
        model.editor_mut().viewport.top_line = 0;

        let total_lines = model.document().line_count();

        // Select all (Cmd+A)
        update(&mut model, Msg::Editor(EditorMsg::SelectAll));

        // Verify selection spans entire document
        assert_eq!(
            model.editor().primary_selection().anchor,
            Position::new(0, 0)
        );
        let last_line = total_lines.saturating_sub(1);
        assert_eq!(
            model.editor().active_cursor().line,
            last_line,
            "Cursor should be at last line"
        );

        // Press Right arrow - should clear selection and position cursor at end
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowRight),
            PhysicalKey::Code(KeyCode::ArrowRight),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().active_selection().is_empty(),
            "Selection should be cleared"
        );

        // Cursor should be at end of document
        assert_eq!(
            model.editor().active_cursor().line,
            last_line,
            "Cursor should be at last line"
        );

        // Viewport should have scrolled to show the cursor
        // The cursor should be visible within the viewport
        let viewport_end = model.editor().viewport.top_line + model.editor().viewport.visible_lines;
        assert!(
            model.editor().active_cursor().line >= model.editor().viewport.top_line,
            "Cursor (line {}) should be >= viewport top (line {})",
            model.editor().active_cursor().line,
            model.editor().viewport.top_line
        );
        assert!(
            model.editor().active_cursor().line < viewport_end,
            "Cursor (line {}) should be < viewport end (line {})",
            model.editor().active_cursor().line,
            viewport_end
        );
    }

    #[test]
    fn test_pageup_scrolls_cursor_to_viewport_top() {
        // Create a 100-line document
        let text = (0..100)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_selection(&text, 0, 0, 0, 0);
        model.editor_mut().viewport.visible_lines = 20;
        model.editor_mut().scroll_padding = 1;

        // Position cursor at about half the viewport height (line 10)
        // and set viewport to start at line 0
        model.editor_mut().primary_cursor_mut().line = 10;
        model.editor_mut().viewport.top_line = 0;

        // Press PageUp - cursor should jump above the viewport,
        // and viewport should adjust to show cursor at top
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            false,
            false,
            false,
            false,
            false,
        );

        // PageUp moves visible_lines - 2 = 18 lines up
        // From line 10, that would be line 0 (clamped)
        assert_eq!(
            model.editor().active_cursor().line,
            0,
            "Cursor should be at line 0 after PageUp"
        );

        // Viewport should adjust to show cursor
        // With cursor at line 0, viewport.top_line should be 0
        assert_eq!(
            model.editor().viewport.top_line,
            0,
            "Viewport should scroll to top to show cursor"
        );

        // Cursor should be visible
        assert!(
            model.editor().active_cursor().line >= model.editor().viewport.top_line,
            "Cursor should be visible (>= viewport top)"
        );
    }

    #[test]
    fn test_pageup_from_middle_adjusts_viewport() {
        // Create a 100-line document
        let text = (0..100)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_selection(&text, 0, 0, 0, 0);
        model.editor_mut().viewport.visible_lines = 20;
        model.editor_mut().scroll_padding = 1;

        // Position cursor at line 50 with viewport showing lines 40-60
        model.editor_mut().primary_cursor_mut().line = 50;
        model.editor_mut().viewport.top_line = 40;

        // Press PageUp - cursor should move up 18 lines (20 - 2)
        // From line 50, cursor goes to line 32
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            false,
            false,
            false,
            false,
            false,
        );

        // Cursor should be at line 32 (50 - 18)
        assert_eq!(
            model.editor().active_cursor().line,
            32,
            "Cursor should be at line 32"
        );

        // Line 32 was above the viewport (which was at 40-60)
        // Viewport should have adjusted to show the cursor
        // Cursor should be visible and near the top of viewport
        assert!(
            model.editor().viewport.top_line <= model.editor().active_cursor().line,
            "Cursor (line {}) should be >= viewport top (line {})",
            model.editor().active_cursor().line,
            model.editor().viewport.top_line
        );

        // Cursor should be within visible range
        let viewport_end = model.editor().viewport.top_line + model.editor().viewport.visible_lines;
        assert!(
            model.editor().active_cursor().line < viewport_end,
            "Cursor should be visible within viewport"
        );
    }

    // ========================================================================
    // Cmd+Z / Cmd+Shift+Z Keybinding Tests (macOS)
    // ========================================================================

    #[test]
    fn test_cmd_z_triggers_undo_not_insert_z() {
        // Test that Cmd+Z (logo=true) triggers undo and doesn't insert 'z'
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        // Make a change: insert 'X'
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");
        assert_eq!(model.editor().active_cursor().column, 6);

        // Simulate Cmd+Z: logo=true, ctrl=false
        handle_key(
            &mut model,
            Key::Character("z".into()),
            PhysicalKey::Code(KeyCode::KeyZ),
            false, // ctrl
            false, // shift
            false, // alt
            true,  // logo (Cmd on macOS)
            false, // option_double_tapped
        );

        // Undo should have run, and no 'z' should be typed
        assert_eq!(
            model.document().buffer.to_string(),
            "hello",
            "Cmd+Z should undo the insert, not type 'z'"
        );
        assert_eq!(model.editor().active_cursor().column, 5);
    }

    #[test]
    fn test_cmd_shift_z_triggers_redo_not_insert_z() {
        // Test that Cmd+Shift+Z (logo=true, shift=true) triggers redo
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        // Make a change and undo it
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");

        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "hello");

        // Simulate Cmd+Shift+Z: logo=true, shift=true
        handle_key(
            &mut model,
            Key::Character("z".into()),
            PhysicalKey::Code(KeyCode::KeyZ),
            false, // ctrl
            true,  // shift
            false, // alt
            true,  // logo (Cmd on macOS)
            false, // option_double_tapped
        );

        // Redo should have run
        assert_eq!(
            model.document().buffer.to_string(),
            "helloX",
            "Cmd+Shift+Z should redo the insert"
        );
        assert_eq!(model.editor().active_cursor().column, 6);
    }

    #[test]
    fn test_ctrl_z_still_works_for_undo() {
        // Ensure Ctrl+Z still works (for non-macOS)
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");

        // Simulate Ctrl+Z
        handle_key(
            &mut model,
            Key::Character("z".into()),
            PhysicalKey::Code(KeyCode::KeyZ),
            true,  // ctrl
            false, // shift
            false, // alt
            false, // logo
            false,
        );

        assert_eq!(
            model.document().buffer.to_string(),
            "hello",
            "Ctrl+Z should undo"
        );
    }
}
