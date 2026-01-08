#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use anyhow::Result;
use clap::Parser;
use winit::event_loop::EventLoop;

use token::cli::CliArgs;

#[cfg(debug_assertions)]
mod debug_dump;
mod runtime;
mod view;

use runtime::App;

// ============================================================================
// MAIN - Entry point
// ============================================================================

fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    token::tracing::init();

    // Parse command-line arguments
    let args = CliArgs::parse();
    let startup_config = args.into_config().map_err(|e| anyhow::anyhow!(e))?;

    let event_loop = EventLoop::new()?;
    let mut app = App::new(800, 600, startup_config);
    event_loop.run_app(&mut app)?;

    Ok(())
}

// ============================================================================
// TESTS - Keyboard handling tests that require handle_key()
// TODO: Find a way to move it into test module instead of main.rs
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::runtime::input::handle_key;
    use token::config::EditorConfig;
    use token::messages::{DocumentMsg, EditorMsg, Msg};
    use token::model::{
        AppModel, Cursor, Document, EditorArea, EditorState, Position, RectangleSelectionState,
        Selection, UiState, ViewMode, Viewport,
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
            view_mode: ViewMode::default(),
        };
        let editor_area = EditorArea::single_document(document, editor);
        AppModel {
            editor_area,
            ui: UiState::new(),
            theme: Theme::default(),
            config: EditorConfig::default(),
            window_size: (800, 600),
            line_height: 20,
            char_width: 10.0,
            metrics: token::model::ScaledMetrics::default(),
            workspace: None,
            #[cfg(debug_assertions)]
            debug_overlay: None,
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
        use token::messages::Direction;
        let mut model = test_model_with_selection("hello world\n", 0, 2, 0, 8);
        // Selection: anchor at col 2, head/cursor at col 8

        // MoveCursor(Left) via keymap dispatches this message
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
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
        use token::messages::Direction;
        let mut model = test_model_with_selection("hello world\n", 0, 2, 0, 8);

        // MoveCursor(Right) via keymap dispatches this message
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
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
        // MoveCursor(Right) via keymap dispatches this message
        use token::messages::Direction;
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
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
    // Undo/Redo Tests
    // These test the Undo/Redo commands via the message system.
    // The keymap handles Cmd+Z/Ctrl+Z → Undo and Cmd+Shift+Z/Ctrl+Y → Redo.
    // ========================================================================

    #[test]
    fn test_undo_command() {
        // Test that Undo command works correctly
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        // Make a change: insert 'X'
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");
        assert_eq!(model.editor().active_cursor().column, 6);

        // Undo via message (what keymap dispatches for Cmd+Z/Ctrl+Z)
        update(&mut model, Msg::Document(DocumentMsg::Undo));

        // Undo should have run
        assert_eq!(
            model.document().buffer.to_string(),
            "hello",
            "Undo should revert the insert"
        );
        assert_eq!(model.editor().active_cursor().column, 5);
    }

    #[test]
    fn test_redo_command() {
        // Test that Redo command works correctly
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        // Make a change and undo it
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");

        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "hello");

        // Redo via message (what keymap dispatches for Cmd+Shift+Z/Ctrl+Y)
        update(&mut model, Msg::Document(DocumentMsg::Redo));

        // Redo should have run
        assert_eq!(
            model.document().buffer.to_string(),
            "helloX",
            "Redo should re-apply the insert"
        );
        assert_eq!(model.editor().active_cursor().column, 6);
    }

    // ========================================================================
    // Modal Isolation Tests
    // Verify that when a modal is active, key presses don't affect the editor
    // ========================================================================

    use token::model::{CommandPaletteState, ModalState};

    /// Helper to create a model with command palette open and no selection
    fn test_model_with_modal(text: &str) -> AppModel {
        let cursor = Cursor {
            line: 0,
            column: 0,
            desired_column: None,
        };
        let document = Document::with_text(text);
        let editor = EditorState {
            id: None,
            document_id: None,
            cursors: vec![cursor],
            selections: vec![Selection::new(Position::new(0, 0))],
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
            view_mode: ViewMode::default(),
        };
        let editor_area = EditorArea::single_document(document, editor);
        let mut model = AppModel {
            editor_area,
            ui: UiState::new(),
            theme: Theme::default(),
            config: EditorConfig::default(),
            window_size: (800, 600),
            line_height: 20,
            char_width: 10.0,
            metrics: token::model::ScaledMetrics::default(),
            workspace: None,
            #[cfg(debug_assertions)]
            debug_overlay: None,
        };

        // Open command palette
        model
            .ui
            .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));
        assert!(model.ui.has_modal(), "Modal should be open");
        model
    }

    #[test]
    fn test_modal_arrow_keys_dont_move_editor_cursor() {
        let mut model = test_model_with_modal("hello\nworld\nfoo\nbar\n");

        // Position editor cursor at line 2 (must also update selection to match)
        model.editor_mut().primary_cursor_mut().line = 2;
        model.editor_mut().primary_cursor_mut().column = 1;
        let pos = Position::new(2, 1);
        model.editor_mut().selections[0] = Selection::new(pos);
        let initial_line = model.editor().active_cursor().line;
        let initial_col = model.editor().active_cursor().column;

        // Press Down arrow while modal is open
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

        // Editor cursor should NOT have moved
        assert_eq!(
            model.editor().active_cursor().line,
            initial_line,
            "Arrow Down with modal open should not move editor cursor"
        );

        // Press Up arrow
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

        assert_eq!(
            model.editor().active_cursor().line,
            initial_line,
            "Arrow Up with modal open should not move editor cursor"
        );

        // Press Left/Right arrows
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

        assert_eq!(
            model.editor().active_cursor().column,
            initial_col,
            "Arrow Left/Right with modal open should not move editor cursor"
        );
    }

    #[test]
    fn test_modal_typing_doesnt_insert_in_editor() {
        let mut model = test_model_with_modal("hello world");
        let initial_text = model.document().buffer.to_string();

        // Type characters while modal is open
        handle_key(
            &mut model,
            Key::Character("a".into()),
            PhysicalKey::Code(KeyCode::KeyA),
            false,
            false,
            false,
            false,
            false,
        );
        handle_key(
            &mut model,
            Key::Character("b".into()),
            PhysicalKey::Code(KeyCode::KeyB),
            false,
            false,
            false,
            false,
            false,
        );
        handle_key(
            &mut model,
            Key::Character("c".into()),
            PhysicalKey::Code(KeyCode::KeyC),
            false,
            false,
            false,
            false,
            false,
        );

        // Editor text should NOT have changed
        assert_eq!(
            model.document().buffer.to_string(),
            initial_text,
            "Typing with modal open should not insert text in editor"
        );
    }

    #[test]
    fn test_modal_backspace_doesnt_delete_in_editor() {
        let mut model = test_model_with_modal("hello world");
        // Position cursor in middle of editor text (must also update selection)
        model.editor_mut().primary_cursor_mut().column = 5;
        let pos = Position::new(0, 5);
        model.editor_mut().selections[0] = Selection::new(pos);
        let initial_text = model.document().buffer.to_string();

        // Press backspace while modal is open
        handle_key(
            &mut model,
            Key::Named(NamedKey::Backspace),
            PhysicalKey::Code(KeyCode::Backspace),
            false,
            false,
            false,
            false,
            false,
        );

        // Editor text should NOT have changed
        assert_eq!(
            model.document().buffer.to_string(),
            initial_text,
            "Backspace with modal open should not delete text in editor"
        );
    }

    #[test]
    fn test_modal_delete_doesnt_delete_in_editor() {
        let mut model = test_model_with_modal("hello world");
        model.editor_mut().primary_cursor_mut().column = 5;
        let pos = Position::new(0, 5);
        model.editor_mut().selections[0] = Selection::new(pos);
        let initial_text = model.document().buffer.to_string();

        // Press delete while modal is open
        handle_key(
            &mut model,
            Key::Named(NamedKey::Delete),
            PhysicalKey::Code(KeyCode::Delete),
            false,
            false,
            false,
            false,
            false,
        );

        // Editor text should NOT have changed
        assert_eq!(
            model.document().buffer.to_string(),
            initial_text,
            "Delete with modal open should not delete text in editor"
        );
    }

    #[test]
    fn test_modal_enter_doesnt_insert_newline() {
        let mut model = test_model_with_modal("hello world");
        let initial_line_count = model.document().line_count();

        // Press Enter while modal is open
        handle_key(
            &mut model,
            Key::Named(NamedKey::Enter),
            PhysicalKey::Code(KeyCode::Enter),
            false,
            false,
            false,
            false,
            false,
        );

        // Modal should be closed (Enter confirms), but no newline inserted
        // Note: Enter in modal confirms the action - modal closes
        // Check that no newline was inserted in editor
        assert_eq!(
            model.document().line_count(),
            initial_line_count,
            "Enter with modal open should not insert newline in editor"
        );
    }

    #[test]
    fn test_modal_escape_closes_modal_not_clear_editor_selection() {
        let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);
        // Open modal
        model
            .ui
            .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));
        assert!(model.ui.has_modal());

        // Editor has a selection
        assert!(!model.editor().active_selection().is_empty());

        // Press Escape while modal is open
        handle_key(
            &mut model,
            Key::Named(NamedKey::Escape),
            PhysicalKey::Code(KeyCode::Escape),
            false,
            false,
            false,
            false,
            false,
        );

        // Modal should be closed
        assert!(!model.ui.has_modal(), "Escape should close modal");

        // Editor selection should still be there (Escape didn't clear it)
        assert!(
            !model.editor().active_selection().is_empty(),
            "Escape with modal should close modal, not clear editor selection"
        );
    }

    #[test]
    fn test_modal_pageup_pagedown_dont_scroll_editor() {
        let text = (0..100)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_modal(&text);
        model.editor_mut().viewport.visible_lines = 20;
        model.editor_mut().viewport.top_line = 50;
        model.editor_mut().primary_cursor_mut().line = 55;

        let initial_viewport = model.editor().viewport.top_line;
        let initial_cursor_line = model.editor().active_cursor().line;

        // Press PageDown while modal is open
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

        // Editor viewport and cursor should NOT have changed
        assert_eq!(
            model.editor().viewport.top_line,
            initial_viewport,
            "PageDown with modal open should not scroll editor"
        );
        assert_eq!(
            model.editor().active_cursor().line,
            initial_cursor_line,
            "PageDown with modal open should not move editor cursor"
        );

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

        assert_eq!(
            model.editor().viewport.top_line,
            initial_viewport,
            "PageUp with modal open should not scroll editor"
        );
    }

    #[test]
    fn test_modal_home_end_dont_move_editor_cursor() {
        let mut model = test_model_with_modal("hello world\nfoo bar baz\n");
        model.editor_mut().primary_cursor_mut().line = 1;
        model.editor_mut().primary_cursor_mut().column = 5;
        let pos = Position::new(1, 5);
        model.editor_mut().selections[0] = Selection::new(pos);

        let initial_line = model.editor().active_cursor().line;
        let initial_col = model.editor().active_cursor().column;

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

        assert_eq!(
            model.editor().active_cursor().column,
            initial_col,
            "Home with modal open should not move editor cursor"
        );

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

        assert_eq!(
            model.editor().active_cursor().column,
            initial_col,
            "End with modal open should not move editor cursor"
        );
        assert_eq!(model.editor().active_cursor().line, initial_line);
    }

    #[test]
    fn test_modal_cmd_shortcuts_dont_affect_editor() {
        let mut model = test_model_with_modal("hello world");
        model.editor_mut().primary_cursor_mut().column = 5;
        let pos = Position::new(0, 5);
        model.editor_mut().selections[0] = Selection::new(pos);

        // Make a change first so we can test that Cmd+Z doesn't undo
        let initial_text = model.document().buffer.to_string();

        // Try Cmd+A (Select All) - should not select all in editor
        handle_key(
            &mut model,
            Key::Character("a".into()),
            PhysicalKey::Code(KeyCode::KeyA),
            false,
            false,
            false,
            true,
            false, // logo=true (Cmd on macOS)
        );

        // Selection in editor should remain empty
        assert!(
            model.editor().active_selection().is_empty(),
            "Cmd+A with modal open should not select all in editor"
        );

        // Try Cmd+D (Duplicate) - should not duplicate
        handle_key(
            &mut model,
            Key::Character("d".into()),
            PhysicalKey::Code(KeyCode::KeyD),
            false,
            false,
            false,
            true,
            false,
        );

        assert_eq!(
            model.document().buffer.to_string(),
            initial_text,
            "Cmd+D with modal open should not duplicate line"
        );
    }
}
