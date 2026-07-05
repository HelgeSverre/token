//! Regression tests for cursor-position clamping:
//! - EditorMsg::SetCursorPosition (src/update/editor.rs)
//! - OutlineMsg::JumpToSymbol / ClickRow (src/update/outline.rs)
//!
//! All three write cursor coordinates that could, in principle, be
//! out-of-range (SetCursorPosition from a caller bug, JumpToSymbol/ClickRow
//! from a stale outline built before the document was edited). They must
//! clamp to valid document bounds instead of producing an out-of-range
//! cursor.

mod common;

use common::test_model;
use token::messages::{EditorMsg, Msg, OutlineMsg};
use token::model::OutlinePanelState;
use token::outline::{OutlineData, OutlineNode, OutlineRange};
use token::update::update;

#[test]
fn set_cursor_position_clamps_wildly_out_of_range_coordinates() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition {
            line: 9999,
            column: 9999,
        }),
    );

    let cursor = *model.editor().primary_cursor();
    let last_line = model.document().line_count().saturating_sub(1);
    assert_eq!(cursor.line, last_line, "line must clamp to the last line");
    assert!(
        cursor.column <= model.document().line_length(cursor.line),
        "column must clamp to the clamped line's length"
    );
}

fn outline_with_out_of_range_symbol() -> OutlineData {
    OutlineData {
        revision: 0,
        roots: vec![OutlineNode {
            kind: token::outline::OutlineKind::Function,
            name: "stale_symbol".to_string(),
            range: OutlineRange {
                start_line: 9999,
                start_col: 9999,
                end_line: 9999,
                end_col: 9999,
            },
            children: Vec::new(),
        }],
    }
}

#[test]
fn jump_to_symbol_clamps_stale_out_of_range_position() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    update(
        &mut model,
        Msg::Outline(OutlineMsg::JumpToSymbol {
            line: 9999,
            col: 9999,
        }),
    );

    let cursor = model.editor().cursors[0];
    let last_line = model.document().line_count().saturating_sub(1);
    assert_eq!(cursor.line, last_line);
    assert!(cursor.column <= model.document().line_length(cursor.line));
}

#[test]
fn click_row_double_click_clamps_stale_out_of_range_position() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    model.document_mut().outline = Some(outline_with_out_of_range_symbol());
    model.outline_panel = OutlinePanelState::default();

    update(
        &mut model,
        Msg::Outline(OutlineMsg::ClickRow {
            index: 0,
            click_count: 2,
            on_chevron: false,
        }),
    );

    let cursor = model.editor().cursors[0];
    let last_line = model.document().line_count().saturating_sub(1);
    assert_eq!(cursor.line, last_line);
    assert!(cursor.column <= model.document().line_length(cursor.line));
}

// ========================================================================
// OutlineMsg::Scroll saturating-add regression test
// ========================================================================

#[test]
fn outline_scroll_near_usize_max_does_not_panic_on_overflow() {
    // Regression test: the scroll handler used to do plain `offset + lines
    // as usize`, which could overflow-panic in a debug build before the
    // later `.min(...)` clamp ever runs. It must saturate instead.
    let mut model = test_model("hello\nworld\n", 0, 0);
    model.outline_panel.scroll_offset = usize::MAX - 1;

    update(&mut model, Msg::Outline(OutlineMsg::Scroll { lines: 10 }));

    // No outline is attached to this document, so the handler's own
    // no-outline branch resets scroll_offset to 0 — the important part is
    // that reaching that point didn't panic on overflow first.
    assert_eq!(model.outline_panel.scroll_offset, 0);
}
