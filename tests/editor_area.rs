use token::model::{
    Document, EditorArea, EditorGroup, EditorState, LayoutNode, Rect, SplitContainer,
    SplitDirection, Tab,
};

fn create_test_editor_area() -> EditorArea {
    let document = Document::new();
    let editor = EditorState::new();
    EditorArea::single_document(document, editor)
}

#[test]
fn test_rect_contains() {
    let rect = Rect::new(10.0, 20.0, 100.0, 50.0);

    // Inside
    assert!(rect.contains(50.0, 40.0));
    assert!(rect.contains(10.0, 20.0)); // Top-left corner

    // Outside
    assert!(!rect.contains(5.0, 40.0)); // Left of rect
    assert!(!rect.contains(150.0, 40.0)); // Right of rect
    assert!(!rect.contains(50.0, 10.0)); // Above rect
    assert!(!rect.contains(50.0, 80.0)); // Below rect

    // Edge cases (exclusive upper bounds)
    assert!(!rect.contains(110.0, 40.0)); // At right edge
    assert!(!rect.contains(50.0, 70.0)); // At bottom edge
}

#[test]
fn test_single_group_layout() {
    let mut area = create_test_editor_area();
    let available = Rect::new(0.0, 0.0, 800.0, 600.0);

    let splitters = area.compute_layout(available);

    // Single group = no splitters
    assert!(splitters.is_empty());

    // Group should occupy entire area
    let group = area.focused_group().unwrap();
    assert_eq!(group.rect.x, 0.0);
    assert_eq!(group.rect.y, 0.0);
    assert_eq!(group.rect.width, 800.0);
    assert_eq!(group.rect.height, 600.0);
}

#[test]
fn test_group_at_point_single() {
    let mut area = create_test_editor_area();
    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    area.compute_layout(available);

    // Point inside should find the group
    let group_id = area.group_at_point(400.0, 300.0);
    assert!(group_id.is_some());
    assert_eq!(group_id.unwrap(), area.focused_group_id);

    // Point outside should find nothing
    let outside = area.group_at_point(900.0, 300.0);
    assert!(outside.is_none());
}

#[test]
fn test_horizontal_split_layout() {
    let mut area = create_test_editor_area();

    // Create a second group
    let group2_id = area.next_group_id();
    let doc_id = area.focused_document_id().unwrap();
    let editor2_id = area.next_editor_id();
    let tab2_id = area.next_tab_id();

    let mut editor2 = EditorState::new();
    editor2.id = Some(editor2_id);
    editor2.document_id = Some(doc_id);
    area.editors.insert(editor2_id, editor2);

    let tab2 = Tab {
        id: tab2_id,
        editor_id: editor2_id,
        is_pinned: false,
        is_preview: false,
    };

    area.groups.insert(
        group2_id,
        EditorGroup {
            id: group2_id,
            tabs: vec![tab2],
            active_tab_index: 0,
            rect: Rect::default(),
            attached_preview: None,
        },
    );

    // Create horizontal split
    let group1_id = area.focused_group_id;
    area.layout = LayoutNode::Split(SplitContainer {
        direction: SplitDirection::Horizontal,
        children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
        ratios: vec![0.5, 0.5],
        min_sizes: vec![100.0, 100.0],
    });

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    let splitters = area.compute_layout(available);

    // Should have one splitter
    assert_eq!(splitters.len(), 1);
    assert_eq!(splitters[0].direction, SplitDirection::Horizontal);

    // Groups should be side by side
    let group1 = area.groups.get(&group1_id).unwrap();
    let group2 = area.groups.get(&group2_id).unwrap();

    assert_eq!(group1.rect.x, 0.0);
    assert_eq!(group1.rect.width, 400.0);

    assert_eq!(group2.rect.x, 400.0);
    assert_eq!(group2.rect.width, 400.0);
}

#[test]
fn test_vertical_split_layout() {
    let mut area = create_test_editor_area();

    // Create a second group
    let group2_id = area.next_group_id();
    let doc_id = area.focused_document_id().unwrap();
    let editor2_id = area.next_editor_id();
    let tab2_id = area.next_tab_id();

    let mut editor2 = EditorState::new();
    editor2.id = Some(editor2_id);
    editor2.document_id = Some(doc_id);
    area.editors.insert(editor2_id, editor2);

    let tab2 = Tab {
        id: tab2_id,
        editor_id: editor2_id,
        is_pinned: false,
        is_preview: false,
    };

    area.groups.insert(
        group2_id,
        EditorGroup {
            id: group2_id,
            tabs: vec![tab2],
            active_tab_index: 0,
            rect: Rect::default(),
            attached_preview: None,
        },
    );

    // Create vertical split
    let group1_id = area.focused_group_id;
    area.layout = LayoutNode::Split(SplitContainer {
        direction: SplitDirection::Vertical,
        children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
        ratios: vec![0.5, 0.5],
        min_sizes: vec![100.0, 100.0],
    });

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    let splitters = area.compute_layout(available);

    // Should have one splitter
    assert_eq!(splitters.len(), 1);
    assert_eq!(splitters[0].direction, SplitDirection::Vertical);

    // Groups should be stacked
    let group1 = area.groups.get(&group1_id).unwrap();
    let group2 = area.groups.get(&group2_id).unwrap();

    assert_eq!(group1.rect.y, 0.0);
    assert_eq!(group1.rect.height, 300.0);

    assert_eq!(group2.rect.y, 300.0);
    assert_eq!(group2.rect.height, 300.0);
}

#[test]
fn test_group_at_point_split() {
    let mut area = create_test_editor_area();

    // Create a second group with horizontal split
    let group2_id = area.next_group_id();
    let doc_id = area.focused_document_id().unwrap();
    let editor2_id = area.next_editor_id();
    let tab2_id = area.next_tab_id();

    let mut editor2 = EditorState::new();
    editor2.id = Some(editor2_id);
    editor2.document_id = Some(doc_id);
    area.editors.insert(editor2_id, editor2);

    let tab2 = Tab {
        id: tab2_id,
        editor_id: editor2_id,
        is_pinned: false,
        is_preview: false,
    };

    area.groups.insert(
        group2_id,
        EditorGroup {
            id: group2_id,
            tabs: vec![tab2],
            active_tab_index: 0,
            rect: Rect::default(),
            attached_preview: None,
        },
    );

    let group1_id = area.focused_group_id;
    area.layout = LayoutNode::Split(SplitContainer {
        direction: SplitDirection::Horizontal,
        children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
        ratios: vec![0.5, 0.5],
        min_sizes: vec![100.0, 100.0],
    });

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    area.compute_layout(available);

    // Left side should be group1
    assert_eq!(area.group_at_point(100.0, 300.0), Some(group1_id));

    // Right side should be group2
    assert_eq!(area.group_at_point(600.0, 300.0), Some(group2_id));
}

#[test]
fn test_splitter_at_point() {
    let mut area = create_test_editor_area();

    // Create horizontal split
    let group2_id = area.next_group_id();
    let doc_id = area.focused_document_id().unwrap();
    let editor2_id = area.next_editor_id();
    let tab2_id = area.next_tab_id();

    let mut editor2 = EditorState::new();
    editor2.id = Some(editor2_id);
    editor2.document_id = Some(doc_id);
    area.editors.insert(editor2_id, editor2);

    let tab2 = Tab {
        id: tab2_id,
        editor_id: editor2_id,
        is_pinned: false,
        is_preview: false,
    };

    area.groups.insert(
        group2_id,
        EditorGroup {
            id: group2_id,
            tabs: vec![tab2],
            active_tab_index: 0,
            rect: Rect::default(),
            attached_preview: None,
        },
    );

    let group1_id = area.focused_group_id;
    area.layout = LayoutNode::Split(SplitContainer {
        direction: SplitDirection::Horizontal,
        children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
        ratios: vec![0.5, 0.5],
        min_sizes: vec![100.0, 100.0],
    });

    let available = Rect::new(0.0, 0.0, 800.0, 600.0);
    let splitters = area.compute_layout(available);

    // Splitter should be at x=400 (middle)
    assert!(area.splitter_at_point(&splitters, 400.0, 300.0).is_some());

    // Away from splitter should return None
    assert!(area.splitter_at_point(&splitters, 100.0, 300.0).is_none());
    assert!(area.splitter_at_point(&splitters, 600.0, 300.0).is_none());
}
