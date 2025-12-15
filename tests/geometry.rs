//! Tests for geometry helpers and layout consistency
//!
//! These tests verify that geometry calculations are consistent across the codebase.

mod common;

use common::test_model;
use token::messages::{LayoutMsg, Msg};
use token::model::{gutter_border_x, text_start_x, SplitDirection};
use token::update::update;

// ============================================================================
// text_start_x Consistency Tests
// ============================================================================

#[test]
fn test_text_start_x_consistency() {
    // Verify text_start_x returns consistent values for various char widths
    let char_widths = [6.0, 8.0, 10.0, 12.0, 14.0, 16.0];

    for &char_width in &char_widths {
        let result1 = text_start_x(char_width);
        let result2 = text_start_x(char_width);

        // Same input should produce same output
        assert_eq!(
            result1, result2,
            "text_start_x({}) not consistent",
            char_width
        );

        // Result should be positive and reasonable
        assert!(
            result1 > 0.0,
            "text_start_x({}) should be positive",
            char_width
        );
        assert!(
            result1 < 200.0,
            "text_start_x({}) should be reasonable (< 200px)",
            char_width
        );

        // text_start_x should be > gutter_border_x (gutter + some padding)
        let border_x = gutter_border_x(char_width);
        assert!(
            result1 >= border_x,
            "text_start_x({}) should be >= gutter_border_x({})",
            char_width,
            char_width
        );
    }
}

#[test]
fn test_text_start_x_scales_with_char_width() {
    // Larger char_width should generally produce larger text_start_x
    // (since gutter shows line numbers which scale with font)
    let small = text_start_x(6.0);
    let medium = text_start_x(10.0);
    let large = text_start_x(16.0);

    assert!(
        small <= medium,
        "text_start_x should grow with char_width: {} <= {}",
        small,
        medium
    );
    assert!(
        medium <= large,
        "text_start_x should grow with char_width: {} <= {}",
        medium,
        large
    );
}

#[test]
fn test_text_start_x_zero_char_width() {
    // Edge case: zero char_width should not panic and return something reasonable
    let result = text_start_x(0.0);
    // Just verify it doesn't panic and returns a non-negative value
    assert!(result >= 0.0);
}

#[test]
fn test_gutter_border_x_consistency() {
    let char_widths = [6.0, 8.0, 10.0, 12.0];

    for &char_width in &char_widths {
        let result1 = gutter_border_x(char_width);
        let result2 = gutter_border_x(char_width);

        assert_eq!(
            result1, result2,
            "gutter_border_x({}) not consistent",
            char_width
        );
        assert!(
            result1 >= 0.0,
            "gutter_border_x({}) should be non-negative",
            char_width
        );
    }
}

// ============================================================================
// active_tab_index Bounds Tests
// ============================================================================

#[test]
fn test_active_tab_index_bounds_single_tab() {
    let model = test_model("hello\nworld\n", 0, 0);
    let group = model.editor_area.focused_group().unwrap();

    assert_eq!(group.tabs.len(), 1);
    assert!(
        group.active_tab_index < group.tabs.len(),
        "active_tab_index {} should be < tabs.len() {}",
        group.active_tab_index,
        group.tabs.len()
    );
}

#[test]
fn test_active_tab_index_after_split() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split creates new group with new tab
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Check all groups have valid active_tab_index
    for (group_id, group) in &model.editor_area.groups {
        assert!(
            group.active_tab_index < group.tabs.len(),
            "Group {:?} has invalid active_tab_index {} (tabs.len() = {})",
            group_id,
            group.active_tab_index,
            group.tabs.len()
        );
    }
}

#[test]
fn test_active_tab_index_after_close() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split to create second group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;
    let tab2_id = model.editor_area.groups.get(&group2).unwrap().tabs[0].id;

    // Move tab to group1 (now group1 has 2 tabs)
    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id: tab2_id,
            to_group: group1,
        }),
    );

    // Focus group1 and switch to last tab
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));
    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(1)));

    // Close the active tab
    let active_tab_id = model.editor_area.focused_group().unwrap().tabs[1].id;
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(active_tab_id)));

    // active_tab_index should be adjusted to remain valid
    let group = model.editor_area.focused_group().unwrap();
    assert!(
        group.active_tab_index < group.tabs.len(),
        "active_tab_index {} should be < tabs.len() {} after closing last tab",
        group.active_tab_index,
        group.tabs.len()
    );
}

#[test]
fn test_active_tab_index_switch_valid() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split and move tab to create 2 tabs in group1
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;
    let tab2_id = model.editor_area.groups.get(&group2).unwrap().tabs[0].id;

    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id: tab2_id,
            to_group: group1,
        }),
    );

    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));

    // Valid switch
    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(0)));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );

    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(1)));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        1
    );
}

#[test]
fn test_active_tab_index_switch_invalid_ignored() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Only 1 tab, try to switch to index 5
    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(5)));

    // Should still be 0
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );

    // Bounds check
    let group = model.editor_area.focused_group().unwrap();
    assert!(group.active_tab_index < group.tabs.len());
}

// ============================================================================
// Split View Cursor Bounds Tests
// ============================================================================

#[test]
fn test_split_view_cursor_stays_in_bounds() {
    use token::messages::{Direction, EditorMsg};

    let mut model = test_model("short\nvery long line here\n", 0, 0);

    // Split to create two views
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Move cursor to end of long line in group2
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));

    let cursor = model.editor().primary_cursor();
    let line_len = model.document().line_length(cursor.line);

    // Cursor column should never exceed line length
    assert!(
        cursor.column <= line_len,
        "Cursor column {} should be <= line length {} on line {}",
        cursor.column,
        line_len,
        cursor.line
    );
}

#[test]
fn test_split_view_cursor_at_document_end() {
    use token::messages::EditorMsg;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Move to document end
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorDocumentEnd));

    let cursor = model.editor().primary_cursor();
    let total_lines = model.document().buffer.len_lines();

    // Cursor line should be valid
    assert!(
        cursor.line < total_lines,
        "Cursor line {} should be < total lines {}",
        cursor.line,
        total_lines
    );
}

#[test]
fn test_split_view_cursor_after_edit_in_other_view() {
    use token::messages::{Direction, DocumentMsg, EditorMsg};

    let mut model = test_model("hello\nworld\ntest\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // In group2, move to last line using proper messages
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    // Focus group1 and delete a line
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    update(&mut model, Msg::Editor(EditorMsg::SelectLine));
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    // Focus group2 and verify cursor is still valid
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group2)));

    let cursor = model.editor().primary_cursor();
    let total_lines = model.document().buffer.len_lines();

    assert!(
        cursor.line < total_lines,
        "Cursor line {} should be < total lines {} after deletion in other view",
        cursor.line,
        total_lines
    );
}

// ============================================================================
// Empty Group Handling Tests
// ============================================================================

#[test]
fn test_empty_group_not_created() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // All groups should have at least one tab
    for (group_id, group) in &model.editor_area.groups {
        assert!(
            !group.tabs.is_empty(),
            "Group {:?} should not be empty",
            group_id
        );
    }
}

#[test]
fn test_close_last_tab_closes_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split to have 2 groups
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 2);

    // Close the focused group via CloseFocusedGroup
    update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedGroup));

    // Should be back to 1 group
    assert_eq!(model.editor_area.groups.len(), 1);

    // Remaining group should not be empty
    let group = model.editor_area.focused_group().unwrap();
    assert!(!group.tabs.is_empty());
}

#[test]
fn test_close_tab_in_multi_tab_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split and move tab back to create 2 tabs in one group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;
    let tab2_id = model.editor_area.groups.get(&group2).unwrap().tabs[0].id;

    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id: tab2_id,
            to_group: group1,
        }),
    );

    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));
    assert_eq!(model.editor_area.focused_group().unwrap().tabs.len(), 2);

    // Close one tab
    let tab_to_close = model.editor_area.focused_group().unwrap().tabs[0].id;
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(tab_to_close)));

    // Group should still exist with 1 tab
    assert!(model.editor_area.groups.contains_key(&group1));
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 1);
}

#[test]
fn test_focused_group_always_valid() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split multiple times and close groups
    for _ in 0..3 {
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        );
    }

    for _ in 0..2 {
        update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedGroup));
    }

    // focused_group_id should always point to a valid group
    assert!(model
        .editor_area
        .groups
        .contains_key(&model.editor_area.focused_group_id));

    // focused_group() should return Some
    assert!(model.editor_area.focused_group().is_some());
}
