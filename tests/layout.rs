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

// ============================================================================
// Splitter Hit Testing
// ============================================================================

#[test]
fn test_splitter_exact_position_horizontal() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split horizontally
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    let splitters = model.editor_area.compute_layout(available);

    assert_eq!(splitters.len(), 1);

    // Splitter is centered at x=400 with SPLITTER_WIDTH=6, so range is [397, 403)
    // Test at exact center
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 400.0, 300.0)
        .is_some());

    // Test at splitter edges
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 397.0, 300.0)
        .is_some());
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 402.0, 300.0)
        .is_some());

    // Test just outside splitter
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 396.0, 300.0)
        .is_none());
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 403.0, 300.0)
        .is_none());
}

#[test]
fn test_splitter_exact_position_vertical() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split vertically
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    let splitters = model.editor_area.compute_layout(available);

    assert_eq!(splitters.len(), 1);
    assert_eq!(splitters[0].direction, SplitDirection::Vertical);

    // Splitter is centered at y=300 with SPLITTER_WIDTH=6
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 400.0, 300.0)
        .is_some());
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 400.0, 297.0)
        .is_some());
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 400.0, 302.0)
        .is_some());

    // Outside splitter vertically
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 400.0, 100.0)
        .is_none());
    assert!(model
        .editor_area
        .splitter_at_point(&splitters, 400.0, 500.0)
        .is_none());
}

#[test]
fn test_multiple_splitters_hit_testing() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Create 3 groups with horizontal splits: [group1 | group2 | group3]
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Focus first group and split again
    let group_ids: Vec<_> = model.editor_area.groups.keys().copied().collect();
    let first_group = group_ids.iter().min_by_key(|g| g.0).unwrap();
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(*first_group)));
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    assert_eq!(model.editor_area.groups.len(), 3);

    let available = Rect::new(0.0, 0.0, 900.0, 600.0);
    let splitters = model.editor_area.compute_layout(available);

    // Should have multiple splitters in the layout
    assert!(splitters.len() >= 1);
}

// ============================================================================
// Complex Nested Layouts
// ============================================================================

#[test]
fn test_deeply_nested_layout() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Create a complex nested layout:
    // First horizontal split: [left | right]
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 2);

    // Split the right group vertically: [left | top/bottom]
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    assert_eq!(model.editor_area.groups.len(), 3);

    // Split the bottom-right group horizontally again
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 4);

    // Compute layout and verify all groups have valid rects
    let available = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let splitters = model.editor_area.compute_layout(available);

    // Should have 3 splitters for 4 groups in nested configuration
    assert!(splitters.len() >= 2);

    // All groups should have non-zero dimensions
    for group in model.editor_area.groups.values() {
        assert!(
            group.rect.width > 0.0,
            "Group {:?} has zero width",
            group.id
        );
        assert!(
            group.rect.height > 0.0,
            "Group {:?} has zero height",
            group.id
        );
    }
}

#[test]
fn test_mixed_split_directions() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Horizontal split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Vertical split on group2
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let group3 = model.editor_area.focused_group_id;

    assert_eq!(model.editor_area.groups.len(), 3);

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    model.editor_area.compute_layout(available);

    // Group1 should be on the left (full height)
    let g1_rect = model.editor_area.groups.get(&group1).unwrap().rect;
    assert_eq!(g1_rect.x, 0.0);
    assert_eq!(g1_rect.height, 600.0);

    // Group2 and Group3 should share the right side vertically
    let g2_rect = model.editor_area.groups.get(&group2).unwrap().rect;
    let g3_rect = model.editor_area.groups.get(&group3).unwrap().rect;

    // Both should be on the right side
    assert!(g2_rect.x > 0.0);
    assert!(g3_rect.x > 0.0);

    // They should be stacked vertically (one above the other)
    assert!(g2_rect.y != g3_rect.y);
}

// ============================================================================
// Move Tab Operations
// ============================================================================

#[test]
fn test_move_tab_between_groups() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split to create second group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Get the tab from group2
    let tab_id = model.editor_area.groups.get(&group2).unwrap().tabs[0].id;

    // Move tab from group2 to group1
    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id,
            to_group: group1,
        }),
    );

    // Group1 should now have 2 tabs
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 2);

    // Group2 should be closed since it became empty
    assert!(!model.editor_area.groups.contains_key(&group2));

    // Should now have only 1 group
    assert_eq!(model.editor_area.groups.len(), 1);
}

#[test]
fn test_move_tab_to_same_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    let tab_id = model.editor_area.groups.get(&group1).unwrap().tabs[0].id;

    // Move tab to the same group (should be a no-op or work gracefully)
    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id,
            to_group: group1,
        }),
    );

    // Should still have the tab
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 1);
}

#[test]
fn test_move_tab_to_invalid_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    let tab_id = model.editor_area.groups.get(&group1).unwrap().tabs[0].id;

    let invalid_group = GroupId(999);

    // Move tab to non-existent group should be a no-op
    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id,
            to_group: invalid_group,
        }),
    );

    // Tab should still be in the original group
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 1);
    assert_eq!(
        model.editor_area.groups.get(&group1).unwrap().tabs[0].id,
        tab_id
    );
}

// ============================================================================
// Close Tab Operations
// ============================================================================

#[test]
fn test_close_tab_removes_editor() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split to have a second group (so closing tab doesn't fail)
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    let initial_editor_count = model.editor_area.editors.len();
    let group2 = model.editor_area.focused_group_id;
    let tab_id = model.editor_area.groups.get(&group2).unwrap().tabs[0].id;

    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(tab_id)));

    // Editor should be removed
    assert_eq!(model.editor_area.editors.len(), initial_editor_count - 1);
}

#[test]
fn test_close_focused_tab() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split first
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    let initial_editor_count = model.editor_area.editors.len();
    assert_eq!(initial_editor_count, 2);

    // Close focused tab
    update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedTab));

    // Should have one less editor
    assert_eq!(model.editor_area.editors.len(), 1);
}

#[test]
fn test_close_only_tab_closes_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Split to have two groups
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    assert_eq!(model.editor_area.groups.len(), 2);

    let group2 = model.editor_area.focused_group_id;
    let tab_id = model.editor_area.groups.get(&group2).unwrap().tabs[0].id;

    // Close the only tab in group2
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(tab_id)));

    // Group should be closed since it had only one tab
    assert_eq!(model.editor_area.groups.len(), 1);
}

#[test]
fn test_close_tab_in_last_group_keeps_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    assert_eq!(model.editor_area.groups.len(), 1);

    let group1 = model.editor_area.focused_group_id;
    let tab_id = model.editor_area.groups.get(&group1).unwrap().tabs[0].id;

    // Try to close the only tab in the only group
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(tab_id)));

    // Group should still exist (can't close last tab in last group)
    assert_eq!(model.editor_area.groups.len(), 1);

    // Tab should still exist
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 1);

    // Editor should still exist
    assert_eq!(model.editor_area.editors.len(), 1);
}

#[test]
fn test_close_invalid_tab() {
    use token::model::TabId;

    let mut model = test_model("hello\nworld\n", 0, 0);
    let initial_state = model.editor_area.groups.len();

    // Try to close a non-existent tab
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(TabId(999))));

    // Nothing should change
    assert_eq!(model.editor_area.groups.len(), initial_state);
}

// ============================================================================
// Layout Tree Collapse Scenarios
// ============================================================================

#[test]
fn test_close_middle_group_collapses_correctly() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Create 3 groups: [1 | 2 | 3]
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group3 = model.editor_area.focused_group_id;

    assert_eq!(model.editor_area.groups.len(), 3);

    // Close the middle group (group2)
    update(&mut model, Msg::Layout(LayoutMsg::CloseGroup(group2)));

    assert_eq!(model.editor_area.groups.len(), 2);

    // Both remaining groups should still be in layout
    assert!(model.editor_area.groups.contains_key(&group1));
    assert!(model.editor_area.groups.contains_key(&group3));
}

#[test]
fn test_close_nested_group_collapses_parent_split() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Create nested layout: [left | (top / bottom)]
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );

    assert_eq!(model.editor_area.groups.len(), 3);

    // Close the focused group (one of the vertical splits)
    update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedGroup));

    assert_eq!(model.editor_area.groups.len(), 2);

    // Layout should have collapsed the empty vertical split
    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    let splitters = model.editor_area.compute_layout(available);

    // Should have exactly one splitter now (just horizontal)
    assert_eq!(splitters.len(), 1);
}

// ============================================================================
// Independent Viewport/Cursor per Editor
// ============================================================================

#[test]
fn test_independent_cursor_positions() {
    use token::messages::{Direction, EditorMsg};

    let mut model = test_model("line1\nline2\nline3\nline4\nline5\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Move cursor down in group2
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    // Get cursor position for group2's editor
    let editor2_cursor = model.editor().cursor().line;
    assert_eq!(editor2_cursor, 2);

    // Focus group1
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));

    // Cursor in group1 should still be at line 0
    let editor1_cursor = model.editor().cursor().line;
    assert_eq!(editor1_cursor, 0);

    // Move cursor in group1
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert_eq!(model.editor().cursor().line, 1);

    // Switch back to group2 - cursor should still be at line 2
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group2)));
    assert_eq!(model.editor().cursor().line, 2);
}

#[test]
fn test_independent_viewport_scroll() {
    use token::messages::EditorMsg;

    let mut model = test_model(
        &"line\n".repeat(100), // 100 lines
        0,
        0,
    );
    let group1 = model.editor_area.focused_group_id;

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Scroll down in group2
    update(&mut model, Msg::Editor(EditorMsg::Scroll(20)));

    let viewport2_top = model.editor().viewport.top_line;
    assert!(viewport2_top > 0);

    // Focus group1
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));

    // Viewport in group1 should still be at top
    assert_eq!(model.editor().viewport.top_line, 0);

    // Switch back to group2
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group2)));
    assert_eq!(model.editor().viewport.top_line, viewport2_top);
}

// ============================================================================
// SplitGroup with Specific Group ID
// ============================================================================

#[test]
fn test_split_specific_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split focused first to create group2
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // Now split group1 specifically (not focused)
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitGroup {
            group_id: group1,
            direction: SplitDirection::Vertical,
        }),
    );

    assert_eq!(model.editor_area.groups.len(), 3);

    // Focus should have moved to the new split from group1
    assert_ne!(model.editor_area.focused_group_id, group1);
}

#[test]
fn test_split_invalid_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let initial_count = model.editor_area.groups.len();

    // Try to split a non-existent group
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitGroup {
            group_id: GroupId(999),
            direction: SplitDirection::Horizontal,
        }),
    );

    // Nothing should change
    assert_eq!(model.editor_area.groups.len(), initial_count);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_focus_next_with_single_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Focus next on single group should not crash
    update(&mut model, Msg::Layout(LayoutMsg::FocusNextGroup));

    // Focus should remain on the same group
    assert_eq!(model.editor_area.focused_group_id, group1);
}

#[test]
fn test_focus_prev_with_single_group() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    update(&mut model, Msg::Layout(LayoutMsg::FocusPrevGroup));

    assert_eq!(model.editor_area.focused_group_id, group1);
}

#[test]
fn test_group_at_point_after_resize() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // First layout with one size
    let available1 = Rect::new(0.0, 0.0, 800.0, 600.0);
    model.editor_area.compute_layout(available1);

    // Get group at middle (before resize)
    let _group_at_400 = model.editor_area.group_at_point(400.0, 300.0);

    // Resize and recompute
    let available2 = Rect::new(0.0, 0.0, 1200.0, 600.0);
    model.editor_area.compute_layout(available2);

    // Now the midpoint is at 600, so 400 should be in the left group
    let group_at_400_after = model.editor_area.group_at_point(400.0, 300.0);

    // Point at 600 should be at or near the splitter for the new layout
    let group_at_700 = model.editor_area.group_at_point(700.0, 300.0);

    // Both should return valid groups
    assert!(group_at_400_after.is_some());
    assert!(group_at_700.is_some());
}

#[test]
fn test_empty_layout_does_not_panic() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Even with an unusual available rect, should not panic
    let zero_rect = Rect::new(0.0, 0.0, 0.0, 0.0);
    let splitters = model.editor_area.compute_layout(zero_rect);
    assert!(splitters.is_empty());
}

#[test]
fn test_very_small_split_dimensions() {
    use token::model::Rect;

    let mut model = test_model("hello\nworld\n", 0, 0);

    // Create multiple splits
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    assert_eq!(model.editor_area.groups.len(), 4);

    // Compute layout with very small dimensions
    let small_rect = Rect::new(0.0, 0.0, 100.0, 50.0);
    let _splitters = model.editor_area.compute_layout(small_rect);

    // Should handle gracefully without panicking
    // All groups should have some rect assigned
    for group in model.editor_area.groups.values() {
        // Width might be very small but should be non-negative
        assert!(group.rect.width >= 0.0);
        assert!(group.rect.height >= 0.0);
    }
}

// ============================================================================
// Tab Cycling with Multiple Tabs
// ============================================================================

#[test]
fn test_next_tab_with_multiple_tabs() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split to create a second tab, then move it to group1
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

    // Now group1 has 2 tabs
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 2);

    // Focus group1
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));

    // Active tab should be the second one (moved tab becomes active)
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        1
    );

    // Next tab should cycle to first
    update(&mut model, Msg::Layout(LayoutMsg::NextTab));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );

    // Next tab again should go to second
    update(&mut model, Msg::Layout(LayoutMsg::NextTab));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        1
    );
}

#[test]
fn test_prev_tab_with_multiple_tabs() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split to create a second tab, then move it to group1
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

    // Focus group1 and start at first tab
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));
    update(&mut model, Msg::Layout(LayoutMsg::SwitchToTab(0)));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );

    // Prev tab should wrap to last
    update(&mut model, Msg::Layout(LayoutMsg::PrevTab));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        1
    );

    // Prev tab again should go to first
    update(&mut model, Msg::Layout(LayoutMsg::PrevTab));
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );
}

#[test]
fn test_close_non_active_tab() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Split to create a second tab, then move it to group1
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

    // Focus group1, ensure we're on tab 1
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 2);

    // Get the first tab id (not active)
    let tab1_id = model.editor_area.groups.get(&group1).unwrap().tabs[0].id;

    // Close the first tab (not the active one)
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(tab1_id)));

    // Should have 1 tab left
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 1);

    // Active tab index should be adjusted
    assert_eq!(
        model.editor_area.focused_group().unwrap().active_tab_index,
        0
    );
}

#[test]
fn test_selections_preserved_per_editor() {
    use token::messages::EditorMsg;

    let mut model = test_model("hello world\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    // Create a selection in the first editor
    update(&mut model, Msg::Editor(EditorMsg::SelectAll));
    assert!(!model.editor().selection().is_empty());

    // Split
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );

    // New editor should have empty selection
    assert!(model.editor().selection().is_empty());

    // Go back to group1 - selection should still be there
    update(&mut model, Msg::Layout(LayoutMsg::FocusGroup(group1)));
    assert!(!model.editor().selection().is_empty());
}

#[test]
fn test_move_last_tab_from_last_group_prevented() {
    let mut model = test_model("hello\nworld\n", 0, 0);
    let group1 = model.editor_area.focused_group_id;

    let tab_id = model.editor_area.groups.get(&group1).unwrap().tabs[0].id;

    // Split to create second group, then close it
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let group2 = model.editor_area.focused_group_id;

    // Close group2 to get back to single group
    update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedGroup));
    assert_eq!(model.editor_area.groups.len(), 1);

    // Now try to move the only tab - should be prevented
    // (target is invalid since group2 is gone)
    update(
        &mut model,
        Msg::Layout(LayoutMsg::MoveTab {
            tab_id,
            to_group: group2,
        }),
    );

    // Tab should still be in group1
    assert_eq!(model.editor_area.groups.get(&group1).unwrap().tabs.len(), 1);
}

#[test]
fn test_rapid_split_and_close_operations() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Rapidly split and close multiple times
    for _ in 0..5 {
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        );
        update(&mut model, Msg::Layout(LayoutMsg::CloseFocusedGroup));
    }

    // Should still have exactly one group
    assert_eq!(model.editor_area.groups.len(), 1);

    // And the model should be valid
    assert!(model.editor_area.focused_document().is_some());
    assert!(model.editor_area.focused_editor().is_some());
}
