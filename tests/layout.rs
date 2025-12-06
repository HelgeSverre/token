//! Tests for split view and layout operations

mod common;

use common::test_model;
use token::messages::{LayoutMsg, Msg};
use token::model::{GroupId, LayoutNode, SplitDirection};
use token::update::update;

// ============================================================================
// Split Operations
// ============================================================================

#[test]
fn test_split_focused_horizontal() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let original_group_id = model.editor_area.focused_group_id;

    // Initially one group
    assert_eq!(model.editor_area.groups.len(), 1);

    // Split horizontally
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Now two groups
    assert_eq!(model.editor_area.groups.len(), 2);

    // Focus moved to new group
    assert_ne!(model.editor_area.focused_group_id, original_group_id);

    // Layout is now a split
    match &model.editor_area.layout {
        LayoutNode::Split(container) => {
            assert_eq!(container.direction, SplitDirection::Horizontal);
            assert_eq!(container.children.len(), 2);
            assert_eq!(container.ratios.len(), 2);
            assert_eq!(container.ratios[0], 0.5);
            assert_eq!(container.ratios[1], 0.5);
        }
        LayoutNode::Group(_) => panic!("Expected Split, got Group"),
    }
}

#[test]
fn test_split_focused_vertical() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );

    assert_eq!(model.editor_area.groups.len(), 2);

    match &model.editor_area.layout {
        LayoutNode::Split(container) => {
            assert_eq!(container.direction, SplitDirection::Vertical);
        }
        LayoutNode::Group(_) => panic!("Expected Split, got Group"),
    }
}

#[test]
fn test_split_creates_new_editor_for_same_document() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let original_doc_id = model.editor_area.focused_document_id().unwrap();
    let original_editor_count = model.editor_area.editors.len();

    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // New editor created
    assert_eq!(model.editor_area.editors.len(), original_editor_count + 1);

    // New editor references the same document
    let new_doc_id = model.editor_area.focused_document_id().unwrap();
    assert_eq!(new_doc_id, original_doc_id);

    // Document count unchanged (same document, not a copy)
    assert_eq!(model.editor_area.documents.len(), 1);
}

#[test]
fn test_multiple_splits() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // First split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 2);

    // Second split (splits the new group)
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    assert_eq!(model.editor_area.groups.len(), 3);

    // Third split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 4);
}

// ============================================================================
// Close Group Operations
// ============================================================================

#[test]
fn test_close_group_not_allowed_when_last() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group_id = model.editor_area.focused_group_id;

    // Try to close the only group
    update(&mut model, Msg::Layout(LayoutMsg::CloseGroup(group_id)));

    // Should still have one group
    assert_eq!(model.editor_area.groups.len(), 1);
}

#[test]
fn test_close_group_removes_from_layout() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let original_group_id = model.editor_area.focused_group_id;

    // Split first
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 2);

    let new_group_id = model.editor_area.focused_group_id;

    // Close the new group
    update(&mut model, Msg::Layout(LayoutMsg::CloseGroup(new_group_id)));

    // Back to one group
    assert_eq!(model.editor_area.groups.len(), 1);

    // Layout collapsed back to single group
    match &model.editor_area.layout {
        LayoutNode::Group(id) => {
            assert_eq!(*id, original_group_id);
        }
        LayoutNode::Split(_) => panic!("Expected Group, got Split"),
    }

    // Focus moved to remaining group
    assert_eq!(model.editor_area.focused_group_id, original_group_id);
}

#[test]
fn test_close_focused_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split first
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 2);

    // Close focused group
    update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedGroup));

    // Back to one group
    assert_eq!(model.editor_area.groups.len(), 1);
}

#[test]
fn test_close_group_cleans_up_editors() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split to create new editor
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.editors.len(), 2);

    let group_to_close = model.editor_area.focused_group_id;

    // Close the new group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::CloseGroup(group_to_close)),
    );

    // Editor removed
    assert_eq!(model.editor_area.editors.len(), 1);
}

// ============================================================================
// Focus Operations
// ============================================================================

#[test]
fn test_focus_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let original_group_id = model.editor_area.focused_group_id;

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let new_group_id = model.editor_area.focused_group_id;
    assert_ne!(new_group_id, original_group_id);

    // Focus original group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::FocusGroup(original_group_id)),
    );
    assert_eq!(model.editor_area.focused_group_id, original_group_id);

    // Focus new group
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(new_group_id)));
    assert_eq!(model.editor_area.focused_group_id, new_group_id);
}

#[test]
fn test_focus_invalid_group_ignored() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let original_group_id = model.editor_area.focused_group_id;

    // Try to focus a non-existent group
    let invalid_group_id = GroupId(999);
    update(
        &mut model,
        Msg::Layout(LayoutMsg::FocusGroup(invalid_group_id)),
    );

    // Focus unchanged
    assert_eq!(model.editor_area.focused_group_id, original_group_id);
}

#[test]
fn test_focus_next_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Focus next should cycle back to first
    update(&mut model, Msg::Layout(LayoutMsg::FocusNextGroup));
    assert_eq!(model.editor_area.focused_group_id, group1);

    // Focus next again goes to second
    update(&mut model, Msg::Layout(LayoutMsg::FocusNextGroup));
    assert_eq!(model.editor_area.focused_group_id, group2);
}

#[test]
fn test_focus_prev_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Focus prev from group2 goes to group1
    update(&mut model, Msg::Layout(LayoutMsg::FocusPrevGroup));
    assert_eq!(model.editor_area.focused_group_id, group1);

    // Focus prev from group1 wraps to group2
    update(&mut model, Msg::Layout(LayoutMsg::FocusPrevGroup));
    assert_eq!(model.editor_area.focused_group_id, group2);
}

#[test]
fn test_focus_group_by_index() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Create 3 groups
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    assert_eq!(model.editor_area.groups.len(), 3);

    // Focus by index (1-indexed)
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroupByIndex(1)));
    let focused1 = model.editor_area.focused_group_id;

    update(&mut model, Msg::Layout(LayoutMsg::FocusGroupByIndex(2)));
    let focused2 = model.editor_area.focused_group_id;
    assert_ne!(focused1, focused2);

    update(&mut model, Msg::Layout(LayoutMsg::FocusGroupByIndex(3)));
    let focused3 = model.editor_area.focused_group_id;
    assert_ne!(focused2, focused3);

    // Index 0 and out-of-bounds are ignored
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroupByIndex(0)));
    assert_eq!(model.editor_area.focused_group_id, focused3);

    update(&mut model, Msg::Layout(LayoutMsg::FocusGroupByIndex(99)));
    assert_eq!(model.editor_area.focused_group_id, focused3);
}

// ============================================================================
// Tab Operations
// ============================================================================

#[test]
fn test_next_tab_single_tab() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // With only one tab, next shouldn't crash
    update(&mut model, Msg::Layout(LayoutMsg::NextTab));

    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );
}

#[test]
fn test_prev_tab_single_tab() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // With only one tab, prev shouldn't crash
    update(&mut model, Msg::Layout(LayoutMsg::PrevTab));

    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );
}

#[test]
fn test_switch_to_tab() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Valid switch (to tab 0)
    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(0)));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );

    // Invalid switch (index out of bounds) - should be ignored
    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(99)));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_edit_in_split_view_affects_both() {
    use token::messages::DocumentMsg;

    let mut model = test_model("hello\nworld\n", 0, 0);
    let original_group = model.editor_area.focused_group_id;

    // Split to create second view of same document
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Insert character in the new view
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // Document content changed
    assert_eq!(model.document().buffer.to_string(), "Xhello\nworld\n");

    // Focus original group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::FocusGroup(original_group)),
    );

    // Same document content visible
    assert_eq!(model.document().buffer.to_string(), "Xhello\nworld\n");
}
