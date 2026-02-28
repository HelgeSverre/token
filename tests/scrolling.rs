//! Scrolling tests - vertical, horizontal, page navigation

mod common;

use common::test_model;
use token::messages::{Direction, DocumentMsg, EditorMsg, Msg};
use token::update::update;

// ========================================================================
// Vertical Scrolling tests - JetBrains-Style Boundary Scrolling
// ========================================================================

#[test]
fn test_scroll_no_scroll_when_content_fits() {
    // Document with fewer lines than viewport
    let mut model = test_model("line1\nline2\nline3\n", 0, 0);
    model.editor_mut().viewport.visible_lines = 25;

    // Move down multiple times - should not scroll
    for _ in 0..3 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
        );
    }

    assert_eq!(model.editor().viewport.top_line, 0);
    assert_eq!(model.editor().primary_cursor().line, 3);
}

#[test]
fn test_scroll_down_boundary_crossing() {
    // Create 30 lines of text
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().scroll_padding = 1;

    // Initially at top
    assert_eq!(model.editor().viewport.top_line, 0);
    assert_eq!(model.editor().primary_cursor().line, 0);

    // Move to line 8 (bottom_boundary = top_line + visible_lines - padding - 1 = 0 + 10 - 1 - 1 = 8)
    for _ in 0..8 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
        );
    }

    // Should not have scrolled yet (cursor at boundary)
    assert_eq!(model.editor().viewport.top_line, 0);
    assert_eq!(model.editor().primary_cursor().line, 8);

    // Move one more line down - should trigger scroll
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    // Viewport should scroll to maintain padding
    // cursor is now at line 9, bottom_boundary was 8, so we need to scroll
    // desired_top = cursor.line + padding + 1 = 9 + 1 + 1 = 11
    // viewport.top_line = (11 - visible_lines) = (11 - 10) = 1
    assert_eq!(model.editor().primary_cursor().line, 9);
    assert_eq!(model.editor().viewport.top_line, 1);
}

#[test]
fn test_scroll_up_boundary_crossing() {
    // Create 30 lines of text
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 15, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 10; // Start scrolled down
    model.editor_mut().scroll_padding = 1;

    // cursor at line 15, top_line at 10
    // top_boundary = top_line + padding = 10 + 1 = 11

    // Move up to line 11 (the boundary)
    for _ in 0..4 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
        );
    }

    // Should not have scrolled yet (cursor at boundary)
    assert_eq!(model.editor().viewport.top_line, 10);
    assert_eq!(model.editor().primary_cursor().line, 11);

    // Move one more line up - should trigger scroll
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );

    // Viewport should scroll to maintain padding
    // cursor is now at line 10, should scroll up
    // viewport.top_line = cursor.line - padding = 10 - 1 = 9
    assert_eq!(model.editor().primary_cursor().line, 10);
    assert_eq!(model.editor().viewport.top_line, 9);
}

#[test]
fn test_scroll_mouse_wheel_independent() {
    // Create 50 lines
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 5, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0;

    // Cursor at line 5, viewport at top
    assert_eq!(model.editor().primary_cursor().line, 5);
    assert_eq!(model.editor().viewport.top_line, 0);

    // Scroll down 10 lines with mouse wheel
    update(&mut model, Msg::Editor(EditorMsg::Scroll(10)));

    // Viewport should move but cursor stays at line 5
    assert_eq!(model.editor().primary_cursor().line, 5);
    assert_eq!(model.editor().viewport.top_line, 10);
}

#[test]
fn test_scroll_snap_back_on_insert() {
    // Create 50 lines
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 5, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0;

    // Scroll viewport away from cursor using mouse wheel
    update(&mut model, Msg::Editor(EditorMsg::Scroll(20)));
    assert_eq!(model.editor().viewport.top_line, 20);
    assert_eq!(model.editor().primary_cursor().line, 5); // Cursor off-screen

    // Insert a character - should snap back
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // Viewport should snap to show cursor with padding
    // cursor at line 5, padding = 1
    // Should scroll to show cursor in visible range with padding
    assert_eq!(model.editor().primary_cursor().line, 5);
    assert!(model.editor().viewport.top_line <= 5 - model.editor().scroll_padding);
    assert!(
        model.editor().viewport.top_line + model.editor().viewport.visible_lines
            > 5 + model.editor().scroll_padding
    );
}

#[test]
fn test_scroll_snap_back_on_newline() {
    // Create 50 lines
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 5, 4);
    model.editor_mut().viewport.visible_lines = 10;

    // Scroll viewport away
    update(&mut model, Msg::Editor(EditorMsg::Scroll(20)));
    assert_eq!(model.editor().viewport.top_line, 20);

    // Insert newline - should snap back
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    // Cursor should be at line 6 now
    assert_eq!(model.editor().primary_cursor().line, 6);
    // Viewport should show cursor with padding
    assert!(model.editor().viewport.top_line <= 6 - model.editor().scroll_padding);
    assert!(
        model.editor().viewport.top_line + model.editor().viewport.visible_lines
            > 6 + model.editor().scroll_padding
    );
}

#[test]
fn test_scroll_padding_configurable() {
    // Test with different padding values
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    // Test with padding = 3
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().scroll_padding = 3;

    // bottom_boundary = 0 + 10 - 3 - 1 = 6
    // Move to line 6
    for _ in 0..6 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
        );
    }
    assert_eq!(model.editor().viewport.top_line, 0);

    // Move one more - should scroll
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert_eq!(model.editor().primary_cursor().line, 7);
    assert!(model.editor().viewport.top_line > 0);
}

#[test]
fn test_scroll_at_document_boundaries() {
    // Test at start of document
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_lines = 10;

    // Try to scroll up when already at top
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().viewport.top_line, 0);

    // Test at end of document
    // Text has 31 lines total (line0 through line29, plus empty line 30 from trailing \n)
    let last_line = model.document().buffer.len_lines().saturating_sub(1);
    model.editor_mut().primary_cursor_mut().line = last_line;
    model.editor_mut().viewport.top_line = 20;

    // Try to scroll down when at bottom
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert_eq!(model.editor().primary_cursor().line, last_line); // Should stay at last line
}

#[test]
fn test_scroll_wheel_boundaries() {
    // Test mouse wheel scrolling respects boundaries
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut model = test_model(&text, 15, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 5;

    // Scroll up past the top
    update(&mut model, Msg::Editor(EditorMsg::Scroll(-10)));
    assert_eq!(model.editor().viewport.top_line, 0);

    // Scroll down past the bottom
    model.editor_mut().viewport.top_line = 15;
    update(&mut model, Msg::Editor(EditorMsg::Scroll(10)));
    // Text has 31 lines (0-30), max_top = 31 - 10 = 21
    let max_top = model
        .document()
        .buffer
        .len_lines()
        .saturating_sub(model.editor().viewport.visible_lines);
    assert_eq!(model.editor().viewport.top_line, max_top);

    // Try to scroll further down
    update(&mut model, Msg::Editor(EditorMsg::Scroll(10)));
    assert_eq!(model.editor().viewport.top_line, max_top); // Should stay at max
}

// ========================================================================
// Cursor off-screen visibility tests - Arrow Key Snap-back
// ========================================================================

#[test]
fn test_arrow_up_snaps_viewport_when_cursor_above() {
    // Scenario: Cursor at line 5, viewport scrolled down to show lines 20-29
    // Pressing Up should move cursor to line 4 AND snap viewport back
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 20; // Scrolled far below cursor
    model.editor_mut().scroll_padding = 1;

    // Cursor is way above viewport
    assert!(model.editor().primary_cursor().line < model.editor().viewport.top_line);

    // Press Up - cursor moves to line 4
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );

    assert_eq!(model.editor().primary_cursor().line, 4);
    // Viewport MUST snap back to show cursor with padding
    // Cursor is at line 4, so viewport should show it
    assert!(
        model.editor().viewport.top_line <= 4,
        "Viewport top_line {} should be <= cursor line 4",
        model.editor().viewport.top_line
    );
    assert!(
        model.editor().viewport.top_line + model.editor().viewport.visible_lines > 4,
        "Cursor line 4 should be within visible range"
    );
}

#[test]
fn test_arrow_down_snaps_viewport_when_cursor_above() {
    // Scenario: Cursor at line 5, viewport scrolled down to show lines 20-29
    // Pressing Down should move cursor to line 6 AND snap viewport back
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 20; // Scrolled far below cursor
    model.editor_mut().scroll_padding = 1;

    // Cursor is above viewport
    assert!(model.editor().primary_cursor().line < model.editor().viewport.top_line);

    // Press Down - cursor moves to line 6
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    assert_eq!(model.editor().primary_cursor().line, 6);
    // Viewport MUST snap back to show cursor
    assert!(
        model.editor().viewport.top_line <= 6,
        "Viewport top_line {} should be <= cursor line 6",
        model.editor().viewport.top_line
    );
    assert!(
        model.editor().viewport.top_line + model.editor().viewport.visible_lines > 6,
        "Cursor line 6 should be within visible range"
    );
}

#[test]
fn test_arrow_down_snaps_viewport_when_cursor_below() {
    // Scenario: Cursor at line 40, viewport at top showing lines 0-9
    // Pressing Down should move cursor to line 41 AND snap viewport
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 40, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0; // Viewport at top
    model.editor_mut().scroll_padding = 1;

    // Cursor is below viewport
    assert!(
        model.editor().primary_cursor().line
            >= model.editor().viewport.top_line + model.editor().viewport.visible_lines
    );

    // Press Down - cursor moves to line 41
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    assert_eq!(model.editor().primary_cursor().line, 41);
    // Viewport MUST snap to show cursor
    assert!(
        model.editor().viewport.top_line + model.editor().viewport.visible_lines > 41,
        "Cursor line 41 should be within visible range"
    );
}

#[test]
fn test_arrow_up_snaps_viewport_when_cursor_below() {
    // Scenario: Cursor at line 40, viewport at top showing lines 0-9
    // Pressing Up should move cursor to line 39 AND snap viewport
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 40, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0; // Viewport at top
    model.editor_mut().scroll_padding = 1;

    // Cursor is below viewport
    assert!(
        model.editor().primary_cursor().line
            >= model.editor().viewport.top_line + model.editor().viewport.visible_lines
    );

    // Press Up - cursor moves to line 39
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );

    assert_eq!(model.editor().primary_cursor().line, 39);
    // Viewport MUST snap to show cursor
    assert!(
        model.editor().viewport.top_line + model.editor().viewport.visible_lines > 39,
        "Cursor line 39 should be within visible range"
    );
}

#[test]
fn test_arrow_left_snaps_viewport_when_cursor_offscreen() {
    // Scenario: Cursor at line 5 col 10, viewport scrolled down
    // Pressing Left should snap viewport back
    let text = (0..50)
        .map(|i| format!("line{} content here", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 10);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 20;
    model.editor_mut().scroll_padding = 1;

    // Press Left
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    assert_eq!(model.editor().primary_cursor().line, 5);
    assert_eq!(model.editor().primary_cursor().column, 9);
    // Viewport MUST snap back
    assert!(
        model.editor().viewport.top_line <= 5,
        "Viewport should snap back to show cursor"
    );
}

#[test]
fn test_arrow_right_snaps_viewport_when_cursor_offscreen() {
    // Scenario: Cursor at line 5 col 10, viewport scrolled down
    // Pressing Right should snap viewport back
    let text = (0..50)
        .map(|i| format!("line{} content here", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 10);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 20;
    model.editor_mut().scroll_padding = 1;

    // Press Right
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    assert_eq!(model.editor().primary_cursor().line, 5);
    assert_eq!(model.editor().primary_cursor().column, 11);
    // Viewport MUST snap back
    assert!(
        model.editor().viewport.top_line <= 5,
        "Viewport should snap back to show cursor"
    );
}

// ========================================================================
// Direction-aware scroll reveal tests
// ========================================================================

#[test]
fn test_page_up_reveals_cursor_at_top_of_safe_zone() {
    // When pressing PageUp with cursor off-screen below, cursor should
    // be revealed at TOP of safe zone (so you see what's above)
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 80, 0); // Cursor at line 80
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0; // Viewport way above cursor
    model.editor_mut().scroll_padding = 1;

    // Cursor is below viewport
    assert!(
        model.editor().primary_cursor().line
            >= model.editor().viewport.top_line + model.editor().viewport.visible_lines
    );

    // PageUp - cursor moves up and viewport should reveal with cursor at TOP
    update(&mut model, Msg::Editor(EditorMsg::PageUp));

    // Cursor should be near top of visible area (within top padding)
    let cursor_screen_pos = model.editor().primary_cursor().line - model.editor().viewport.top_line;
    assert!(
        cursor_screen_pos <= model.editor().scroll_padding + 1,
        "Cursor at screen position {} should be at top of safe zone (padding={})",
        cursor_screen_pos,
        model.editor().scroll_padding
    );
}

#[test]
fn test_page_down_reveals_cursor_at_bottom_of_safe_zone() {
    // When pressing PageDown with cursor off-screen above, cursor should
    // be revealed at BOTTOM of safe zone (so you see what's below)
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 0); // Cursor at line 5
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 50; // Viewport way below cursor
    model.editor_mut().scroll_padding = 1;

    // Cursor is above viewport
    assert!(model.editor().primary_cursor().line < model.editor().viewport.top_line);

    // PageDown - cursor moves down and viewport should reveal with cursor at BOTTOM
    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    // Cursor should be near bottom of visible area
    let cursor_screen_pos = model.editor().primary_cursor().line - model.editor().viewport.top_line;
    let bottom_safe = model.editor().viewport.visible_lines - model.editor().scroll_padding - 1;
    assert!(
        cursor_screen_pos >= bottom_safe.saturating_sub(1),
        "Cursor at screen position {} should be at bottom of safe zone (bottom_safe={})",
        cursor_screen_pos,
        bottom_safe
    );
}

#[test]
fn test_arrow_up_reveals_at_top_when_offscreen() {
    // When cursor is off-screen and we press Up, reveal at top of safe zone
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 30; // Viewport far below cursor
    model.editor_mut().scroll_padding = 1;

    // Press Up
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );

    // Cursor should be at top of safe zone
    let cursor_screen_pos = model.editor().primary_cursor().line - model.editor().viewport.top_line;
    assert!(
        cursor_screen_pos <= model.editor().scroll_padding,
        "Cursor at screen position {} should be at top (padding={})",
        cursor_screen_pos,
        model.editor().scroll_padding
    );
}

#[test]
fn test_arrow_down_reveals_at_bottom_when_offscreen() {
    // When cursor is off-screen and we press Down, reveal at bottom of safe zone
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 40, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0; // Viewport far above cursor
    model.editor_mut().scroll_padding = 1;

    // Press Down
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    // Cursor should be at bottom of safe zone
    let cursor_screen_pos = model.editor().primary_cursor().line - model.editor().viewport.top_line;
    let bottom_safe = model.editor().viewport.visible_lines - model.editor().scroll_padding - 1;
    assert!(
        cursor_screen_pos >= bottom_safe,
        "Cursor at screen position {} should be at bottom (bottom_safe={})",
        cursor_screen_pos,
        bottom_safe
    );
}

#[test]
fn test_arrow_within_safe_zone_no_scroll() {
    // When cursor is already in safe zone, arrow keys should NOT cause scroll
    let text = (0..50)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 15, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 12; // Cursor at 15 is visible (12-21)
    model.editor_mut().scroll_padding = 1;

    let initial_top = model.editor().viewport.top_line;

    // Press Up - cursor moves but viewport shouldn't change
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );
    assert_eq!(model.editor().primary_cursor().line, 14);
    assert_eq!(
        model.editor().viewport.top_line,
        initial_top,
        "Viewport should not change when cursor stays in safe zone"
    );

    // Press Down - same expectation
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert_eq!(model.editor().primary_cursor().line, 15);
    assert_eq!(model.editor().viewport.top_line, initial_top);
}

// ========================================================================
// Cursor off-screen visibility tests
// ========================================================================

#[test]
fn test_cursor_position_unchanged_during_scroll() {
    // Scrolling viewport should not change cursor position
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 2);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0;

    // Scroll down - cursor should stay at line 5, column 2
    update(&mut model, Msg::Editor(EditorMsg::Scroll(10)));
    assert_eq!(model.editor().primary_cursor().line, 5);
    assert_eq!(model.editor().primary_cursor().column, 2);
    assert!(model.editor().viewport.top_line > 5); // Viewport moved past cursor
}

#[test]
fn test_cursor_off_screen_above_viewport() {
    // When cursor is above viewport, it should be considered off-screen
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 5, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 10; // Viewport starts at line 10, cursor at line 5

    // Cursor is above viewport - verify positions
    assert!(model.editor().primary_cursor().line < model.editor().viewport.top_line);
}

#[test]
fn test_cursor_off_screen_below_viewport() {
    // When cursor is below viewport, it should be considered off-screen
    let text = (0..30)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 25, 0);
    model.editor_mut().viewport.visible_lines = 10;
    model.editor_mut().viewport.top_line = 0; // Viewport at top, cursor at line 25

    // Cursor is below viewport
    assert!(
        model.editor().primary_cursor().line
            >= model.editor().viewport.top_line + model.editor().viewport.visible_lines
    );
}

// ========================================================================
// Horizontal scroll tests
// ========================================================================

#[test]
fn test_horizontal_scroll_right() {
    let text = "a".repeat(200); // 200 character line
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_columns = 80;
    model.editor_mut().viewport.left_column = 0;

    update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(10)));
    assert_eq!(model.editor().viewport.left_column, 10);
}

#[test]
fn test_horizontal_scroll_left() {
    let text = "a".repeat(200);
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_columns = 80;
    model.editor_mut().viewport.left_column = 50;

    update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(-10)));
    assert_eq!(model.editor().viewport.left_column, 40);
}

#[test]
fn test_horizontal_scroll_left_boundary() {
    let text = "a".repeat(200);
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_columns = 80;
    model.editor_mut().viewport.left_column = 5;

    // Try to scroll left past 0
    update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(-10)));
    assert_eq!(model.editor().viewport.left_column, 0);
}

#[test]
fn test_horizontal_scroll_right_boundary() {
    let text = "a".repeat(100); // 100 char line
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_columns = 80;
    model.editor_mut().viewport.left_column = 0;

    // Try to scroll right past max (100 - 80 = 20)
    update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(50)));
    assert_eq!(model.editor().viewport.left_column, 20); // max_left = 100 - 80 = 20
}

#[test]
fn test_horizontal_scroll_no_scroll_when_content_fits() {
    let text = "short line";
    let mut model = test_model(text, 0, 0);
    model.editor_mut().viewport.visible_columns = 80;
    model.editor_mut().viewport.left_column = 0;

    // Content fits, no scroll should happen
    let result = update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(10)));
    assert!(result.is_none());
    assert_eq!(model.editor().viewport.left_column, 0);
}

#[test]
fn test_horizontal_scroll_cursor_position_unchanged() {
    let text = "a".repeat(200);
    let mut model = test_model(&text, 0, 50);
    model.editor_mut().viewport.visible_columns = 80;
    model.editor_mut().viewport.left_column = 0;

    // Scroll right - cursor should stay at column 50
    update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(100)));
    assert_eq!(model.editor().primary_cursor().column, 50);
}

// ========================================================================
// PageUp/PageDown tests - Column Preservation
// ========================================================================

#[test]
fn test_page_up_preserves_desired_column() {
    // Create text with lines of varying lengths
    let text = "short\nmedium line\nthis is a very long line\nshort\nmedium\n".to_string()
        + &(0..30)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");

    let mut model = test_model(&text, 20, 15); // Start at line 20, column 15
    model.editor_mut().viewport.visible_lines = 10;

    // PageUp should jump ~8 lines (visible_lines - 2)
    update(&mut model, Msg::Editor(EditorMsg::PageUp));

    // Should be at line 12 now (20 - 8)
    assert_eq!(model.editor().primary_cursor().line, 12);

    // desired_column should be preserved
    assert_eq!(model.editor().primary_cursor().desired_column, Some(15));

    // If line 12 is shorter than 15 chars, column should be clamped
    let line_len = model
        .document()
        .line_length(model.editor().primary_cursor().line);
    assert_eq!(model.editor().primary_cursor().column, 15.min(line_len));
}

#[test]
fn test_page_down_preserves_desired_column() {
    let text = (0..50)
        .map(|i| {
            if i % 3 == 0 {
                "short".to_string()
            } else {
                format!("this is line number {}", i)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut model = test_model(&text, 10, 18); // Start at line 10, column 18
    model.editor_mut().viewport.visible_lines = 10;

    // PageDown should jump ~8 lines
    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    // Should be at line 18 now (10 + 8)
    assert_eq!(model.editor().primary_cursor().line, 18);

    // desired_column should be preserved
    assert_eq!(model.editor().primary_cursor().desired_column, Some(18));

    // Column should be clamped if line is shorter
    let line_len = model
        .document()
        .line_length(model.editor().primary_cursor().line);
    assert_eq!(model.editor().primary_cursor().column, 18.min(line_len));
}

#[test]
fn test_multiple_page_jumps_preserve_column() {
    let text = (0..100)
        .map(|i| {
            if i % 5 == 0 {
                "x".to_string() // Very short lines
            } else {
                format!("this is a longer line number {}", i)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut model = test_model(&text, 51, 25); // Start at line 51 (NOT a multiple of 5)
    model.editor_mut().viewport.visible_lines = 10;

    // PageUp twice
    update(&mut model, Msg::Editor(EditorMsg::PageUp));
    update(&mut model, Msg::Editor(EditorMsg::PageUp));

    // PageDown twice (should return to original line)
    update(&mut model, Msg::Editor(EditorMsg::PageDown));
    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    // Should be back at line 51
    assert_eq!(model.editor().primary_cursor().line, 51);

    // Column should be restored to 25
    assert_eq!(model.editor().primary_cursor().column, 25);
    assert_eq!(model.editor().primary_cursor().desired_column, Some(25));
}

#[test]
fn test_page_up_to_short_line_clamps_column() {
    let text = "x\ny\nz\n".to_string()  // Lines 0-2 are 1 char
        + &(3..50).map(|i| format!("this is a very long line {}", i)).collect::<Vec<_>>().join("\n");

    let mut model = test_model(&text, 20, 30); // Start at line 20, column 30
    model.editor_mut().viewport.visible_lines = 10;

    // PageUp multiple times to reach short lines at top
    update(&mut model, Msg::Editor(EditorMsg::PageUp)); // Line 12
    update(&mut model, Msg::Editor(EditorMsg::PageUp)); // Line 4

    assert_eq!(model.editor().primary_cursor().line, 4);
    assert_eq!(model.editor().primary_cursor().desired_column, Some(30)); // Remembers 30

    // PageUp once more to line 0 (very short)
    update(&mut model, Msg::Editor(EditorMsg::PageUp));

    // Should be clamped to line length (1)
    assert!(model.editor().primary_cursor().line <= 2); // One of the short lines
    assert_eq!(model.editor().primary_cursor().column, 1); // Clamped to short line length
    assert_eq!(model.editor().primary_cursor().desired_column, Some(30)); // Still remembers 30

    // PageDown to long line
    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    // Column should restore toward 30
    let line_len = model
        .document()
        .line_length(model.editor().primary_cursor().line);
    assert_eq!(model.editor().primary_cursor().column, 30.min(line_len));
}

// ========================================================================
// Mouse click (SetCursorPosition) should not scroll viewport
// ========================================================================

#[test]
fn test_click_on_visible_line_does_not_scroll() {
    // Simulate clicking on a visible line in the lower half of the viewport.
    // This should NEVER cause the viewport to scroll - the clicked line is
    // already visible, so ensure_cursor_visible should be a no-op.
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_lines = 40;
    model.editor_mut().viewport.top_line = 0;
    model.editor_mut().scroll_padding = 1;

    // Click on line 35 (well within the viewport of 40 visible lines)
    // This is in the lower half but not the last line.
    let top_before = model.editor().viewport.top_line;
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition {
            line: 35,
            column: 0,
        }),
    );

    assert_eq!(
        model.editor().viewport.top_line,
        top_before,
        "Viewport should not scroll when clicking on a visible line (line 35 of 40 visible)"
    );
    assert_eq!(model.editor().primary_cursor().line, 35);
}

#[test]
fn test_click_on_last_visible_line_does_not_scroll() {
    // Clicking on the very last visible line should not scroll either.
    // Scroll padding should NOT apply to mouse clicks.
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_lines = 40;
    model.editor_mut().viewport.top_line = 0;
    model.editor_mut().scroll_padding = 1;

    // Click on line 39 (the very last visible line: top_line=0, visible_lines=40)
    let top_before = model.editor().viewport.top_line;
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition {
            line: 39,
            column: 0,
        }),
    );

    assert_eq!(
        model.editor().viewport.top_line,
        top_before,
        "Viewport should not scroll when clicking on the last visible line"
    );
    assert_eq!(model.editor().primary_cursor().line, 39);
}

#[test]
fn test_click_on_first_visible_line_does_not_scroll() {
    // Clicking on the first visible line (which is inside the scroll padding zone)
    // should not scroll when done via mouse click.
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 50, 0);
    model.editor_mut().viewport.visible_lines = 40;
    model.editor_mut().viewport.top_line = 40;
    model.editor_mut().scroll_padding = 3;

    // Click on line 40 (the very first visible line, within padding zone)
    let top_before = model.editor().viewport.top_line;
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition {
            line: 40,
            column: 0,
        }),
    );

    assert_eq!(
        model.editor().viewport.top_line,
        top_before,
        "Viewport should not scroll when clicking on the first visible line"
    );
    assert_eq!(model.editor().primary_cursor().line, 40);
}

#[test]
fn test_click_off_screen_below_does_scroll() {
    // If SetCursorPosition places cursor BELOW the viewport (e.g., programmatic
    // cursor movement, not a real mouse click on a visible line), it should scroll.
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 0, 0);
    model.editor_mut().viewport.visible_lines = 40;
    model.editor_mut().viewport.top_line = 0;
    model.editor_mut().scroll_padding = 1;

    // "Click" on line 50, which is completely outside the viewport (0..39)
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition {
            line: 50,
            column: 0,
        }),
    );

    // Should scroll to reveal line 50
    assert_eq!(model.editor().primary_cursor().line, 50);
    assert!(
        model.editor().viewport.top_line > 0,
        "Viewport should scroll when cursor is placed outside the visible area"
    );
    // Cursor should be within the visible range
    let top = model.editor().viewport.top_line;
    let vis = model.editor().viewport.visible_lines;
    assert!(
        50 >= top && 50 < top + vis,
        "Cursor should be within visible range after scroll"
    );
}

#[test]
fn test_click_off_screen_above_does_scroll() {
    // Cursor placed above the viewport should trigger scroll
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 50, 0);
    model.editor_mut().viewport.visible_lines = 40;
    model.editor_mut().viewport.top_line = 30;
    model.editor_mut().scroll_padding = 1;

    // "Click" on line 5, which is above the viewport (30..69)
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 5, column: 0 }),
    );

    assert_eq!(model.editor().primary_cursor().line, 5);
    assert!(
        model.editor().viewport.top_line <= 5,
        "Viewport should scroll up when cursor is placed above visible area"
    );
}

// ========================================================================
// sync_all_viewports correctness
// ========================================================================

#[test]
fn test_sync_all_viewports_subtracts_tab_bar_height() {
    // sync_all_viewports should compute visible_lines by subtracting
    // tab_bar_height from the group rect height, since the group rect
    // includes the tab bar area.
    let text = (0..100)
        .map(|i| format!("line{}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let mut model = test_model(&text, 0, 0);

    let line_height = model.line_height; // 20
    let tab_bar_height = model.metrics.tab_bar_height; // 28 at scale 1.0

    // Set group rect to simulate a 600px-tall group (includes tab bar)
    let group_id = model.editor_area.focused_group_id;
    model.editor_area.groups.get_mut(&group_id).unwrap().rect =
        token::model::Rect::new(0.0, 0.0, 800.0, 600.0);

    model
        .editor_area
        .sync_all_viewports(line_height, model.char_width, tab_bar_height);

    // Expected: (600 - 28) / 20 = 572 / 20 = 28 lines
    let expected_visible = (600 - tab_bar_height) / line_height;
    assert_eq!(
        model.editor().viewport.visible_lines,
        expected_visible,
        "sync_all_viewports should subtract tab_bar_height from group height"
    );
}

#[test]
fn test_new_editor_gets_correct_viewport_after_open() {
    // When opening a new file in a new tab, the new editor's viewport
    // should be sized correctly, not stuck at the default 25 lines.
    let text = "initial content";
    let mut model = test_model(text, 0, 0);
    model.editor_mut().viewport.visible_lines = 40;
    model.editor_mut().viewport.visible_columns = 100;

    // Set the group rect so sync_all_viewports can work
    let group_id = model.editor_area.focused_group_id;
    let line_height = model.line_height;
    let tab_bar_height = model.metrics.tab_bar_height;
    let group_height = (40 * line_height + tab_bar_height) as f32;
    model.editor_area.groups.get_mut(&group_id).unwrap().rect =
        token::model::Rect::new(0.0, 0.0, 800.0, group_height);

    // Open a new tab (creating a new editor)
    update(&mut model, Msg::Layout(token::messages::LayoutMsg::NewTab));

    // The new editor should have visible_lines matching the group, NOT the default 25
    assert!(
        model.editor().viewport.visible_lines > 25,
        "New editor should have correct viewport size (got {}), not default 25",
        model.editor().viewport.visible_lines
    );
    assert_eq!(model.editor().viewport.visible_lines, 40);
}
