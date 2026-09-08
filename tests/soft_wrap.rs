//! Soft wrap must use the same visual rows for updates, selection and pixels.

mod common;

use token::messages::{Direction, DocumentMsg, EditorMsg, LayoutMsg, Msg};
use token::model::{Cursor, Document, EditorState, Position, SplitDirection};
use token::update::update;

fn wrapped(text: &str, rows: usize, columns: usize) -> (Document, EditorState) {
    let document = Document::with_text(text);
    let mut editor = EditorState::with_viewport(rows, columns);
    editor.toggle_soft_wrap(&document);
    (document, editor)
}

#[test]
fn soft_wrap_positions_pixels_and_tab_stops_agree() {
    let (doc, editor) = wrapped("hello world\n\tcat dog\n終わり🙂", 10, 6);
    let map = editor.viewport_map(&doc);
    for line in 0..doc.line_count() {
        for column in 0..=doc.line_length(line) {
            let (row, display_col) = map.display_position(&doc, line, column);
            let hit = map.position_for_pixel(
                &doc,
                display_col as f64 * 8.0,
                row as f64 * 20.0,
                8.0,
                20.0,
            );
            assert_eq!(
                hit,
                Position::new(line, column),
                "line {line}, column {column}"
            );
        }
    }
}

#[test]
fn soft_wrap_vertical_movement_preserves_display_column_through_short_rows() {
    let (doc, mut editor) = wrapped("abcd efgh ijkl\nx\n\tword", 10, 5);
    editor.cursors[0] = Cursor::at(0, 3);
    editor.move_cursor_down_at(&doc, 0);
    assert_eq!(editor.cursors[0].to_position(), Position::new(0, 8));
    editor.move_cursor_down_at(&doc, 0);
    assert_eq!(editor.cursors[0].to_position(), Position::new(0, 13));
    editor.move_cursor_down_at(&doc, 0);
    assert_eq!(editor.cursors[0].to_position(), Position::new(1, 1));
    editor.move_cursor_up_at(&doc, 0);
    assert_eq!(editor.cursors[0].to_position(), Position::new(0, 13));
    assert_eq!(editor.cursors[0].desired_column, Some(3));
}

#[test]
fn soft_wrap_clamps_to_requested_row_before_internal_boundary() {
    let (doc, mut editor) = wrapped("abcdefghijklmno", 5, 5);
    editor.cursors[0] = Cursor::at(0, 15);
    editor.move_cursor_up_at(&doc, 0);
    assert_eq!(editor.cursor_visual_line(&doc), 1);
    editor.move_cursor_up_at(&doc, 0);
    assert_eq!(editor.cursor_visual_line(&doc), 0);
    editor.move_cursor_down_at(&doc, 0);
    assert_eq!(editor.cursor_visual_line(&doc), 1);
}

#[test]
fn soft_wrap_partial_top_line_is_visible_and_scroll_uses_visual_rows() {
    let (doc, mut editor) = wrapped("abcdefghijklmnopqrst\nend", 2, 5);
    editor.set_top_line_clamped(&doc, 2);
    let map = editor.viewport_map(&doc);
    assert_eq!(map.visible_doc_lines(), 0..1);
    assert!(map.contains_doc_line(0));
    assert_eq!(map.doc_line_for_visible_row(0), Some(0));
    assert_eq!(map.visible_row_for_position(0, 12), Some(0));
    editor.scroll_vertical_by(&doc, 99);
    assert_eq!(editor.viewport.top_line, 3);
    assert!(!editor.scroll_horizontal_visible_window_by(&doc, 10));
    editor.cursors[0] = Cursor::at(0, 2);
    editor.ensure_cursor_visible(&doc);
    assert_eq!(editor.viewport.top_line, 0);
}

#[test]
fn soft_wrap_page_movement_and_home_end_remain_logical() {
    let (doc, mut editor) = wrapped("abcde fghij klmno pqrst uvwxy\n終🙂", 4, 6);
    editor.cursors[0] = Cursor::at(0, 2);
    editor.page_down_at(&doc, 2, 0);
    assert_eq!(editor.cursor_visual_line(&doc), 2);
    editor.page_up_at(&doc, 2, 0);
    assert_eq!(editor.cursors[0].to_position(), Position::new(0, 2));
    editor.move_cursor_line_end_at(&doc, 0);
    assert_eq!(editor.cursors[0].column, doc.line_length(0));
    editor.move_cursor_line_start_at(&doc, 0);
    assert_eq!(editor.cursors[0].column, 0);
    editor.cursors[0] = Cursor::at(1, 0);
    editor.move_cursor_line_end_at(&doc, 0);
    assert_eq!(editor.cursors[0].column, 2);
}

#[test]
fn soft_wrap_resize_and_toggle_preserve_logical_cursor() {
    let (doc, mut editor) = wrapped("abcde fghij klmno pqrst uvwxy\nend", 3, 6);
    editor.cursors[0] = Cursor::at(0, 20);
    editor.ensure_cursor_visible(&doc);
    let before = editor.wrap_cache.total_visual_lines();
    editor.resize_viewport(3, 12);
    editor.ensure_cursor_visible(&doc);
    assert!(editor.wrap_cache.total_visual_lines() < before);
    assert_eq!(editor.cursors[0].to_position(), Position::new(0, 20));
    editor.toggle_soft_wrap(&doc);
    assert!(!editor.soft_wrap);
    assert_eq!(editor.cursors[0].to_position(), Position::new(0, 20));
    assert_eq!(editor.viewport.top_line, 0);
}

#[test]
fn soft_wrap_embedded_keymap_binds_alt_z() {
    use token::keymap::{Command, KeyCode, Keystroke, Modifiers};
    let bindings =
        token::keymap::parse_keymap_yaml(token::keymap::get_default_keymap_yaml()).unwrap();
    assert!(bindings
        .iter()
        .any(|binding| binding.command == Command::ToggleSoftWrap
            && binding.keystrokes == vec![Keystroke::new(KeyCode::Char('z'), Modifiers::ALT)]));
}

#[test]
fn soft_wrap_toggle_does_not_change_special_tab_viewports() {
    let mut model = common::test_model("not plain text", 0, 0);
    model.editor_mut().tab_content = token::model::editor::TabContent::BinaryPlaceholder(
        token::model::editor::BinaryPlaceholderState {
            path: "binary.dat".into(),
            size_bytes: token::util::byte_size::ByteSize::kibibytes(1).as_u64(),
        },
    );
    model.editor_mut().viewport.left_column = 7;
    let cmd = update(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    assert!(cmd.is_none());
    assert!(!model.editor().soft_wrap);
    assert!(!model.editor().wrap_cache.is_valid());
    assert_eq!(model.editor().viewport.left_column, 7);
}

#[test]
fn soft_wrap_caret_and_group_hit_test_agree_on_a_continuation() {
    let mut model = common::test_model(&"words and\ttabs ".repeat(80), 0, 0);
    model
        .editor_area
        .compute_layout(token::model::Rect::new(0.0, 0.0, 800.0, 580.0));
    update(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    model.editor_mut().viewport.top_line = 1;
    let (line, column) = model.editor().wrap_cache.visual_to_logical(2, 3);
    let caret = token::view::caret::editor_text_rect_at(
        &model,
        line,
        column,
        model.char_width,
        model.line_height,
    )
    .unwrap();
    let group = model.editor_area.focused_group().unwrap();
    let hit = token::view::geometry::pixel_to_cursor_in_group(
        caret.x as f64,
        caret.y as f64,
        model.char_width,
        model.line_height as f64,
        &group.rect,
        &model,
        model.editor(),
        model.document(),
    );
    assert_eq!(hit, (line, column));
}

#[test]
fn soft_wrap_rectangle_creates_distinct_selections_on_visual_rows() {
    let mut model = common::test_model(&"a".repeat(250), 0, 0);
    model.editor_mut().soft_wrap = true;
    model.editor_mut().resize_viewport(8, 10);
    update(
        &mut model,
        Msg::Editor(EditorMsg::StartRectangleSelection {
            line: 1,
            visual_col: 2,
        }),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::UpdateRectangleSelection {
            line: 3,
            visual_col: 5,
        }),
    );
    assert_eq!(
        model.editor().rectangle_selection.preview_cursors,
        vec![
            Position::new(0, 15),
            Position::new(0, 25),
            Position::new(0, 35)
        ]
    );
    update(&mut model, Msg::Editor(EditorMsg::FinishRectangleSelection));
    assert_eq!(model.editor().selections.len(), 3);
    for (i, selection) in model.editor().selections.iter().enumerate() {
        assert_eq!(selection.start(), Position::new(0, (i + 1) * 10 + 2));
        assert_eq!(selection.end(), Position::new(0, (i + 1) * 10 + 5));
    }
}

#[test]
fn soft_wrap_word_and_line_selection_cross_visual_boundaries() {
    let text = "a".repeat(250);
    let mut model = common::test_model(&text, 0, 75);
    update(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    update(&mut model, Msg::Editor(EditorMsg::SelectWord));
    assert_eq!(
        model.editor().selections[0].get_text(model.document()),
        text
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition {
            line: 0,
            column: 95,
        }),
    );
    update(&mut model, Msg::Editor(EditorMsg::SelectLine));
    assert_eq!(
        model.editor().selections[0].get_text(model.document()),
        text
    );
}

#[test]
fn soft_wrap_edit_undo_selection_and_split_panes_refresh_all_caches() {
    let mut model = common::test_model(&"word ".repeat(100), 0, 2);
    update(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    let first_id = model.editor().id.unwrap();
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let second_id = model.editor().id.unwrap();
    assert_ne!(first_id, second_id);
    update(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    assert!(model.editor().soft_wrap);
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 0, column: 2 }),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Down)),
    );
    assert_eq!(model.editor().selections[0].anchor, Position::new(0, 2));
    assert_eq!(model.editor().cursors[0].line, 0);
    assert!(model.editor().cursors[0].column > 2);
    update(&mut model, Msg::Editor(EditorMsg::ClearSelection));
    update(
        &mut model,
        Msg::Document(DocumentMsg::InsertText("extra ".repeat(100))),
    );
    for editor in model.editor_area.editors.values() {
        assert!(!editor
            .wrap_cache
            .needs_rebuild(model.document().revision, editor.viewport.visible_columns));
    }
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "word ".repeat(100));
    update(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    assert!(!model.editor().soft_wrap);
    assert!(model.editor_area.editors[&first_id].soft_wrap);
}
