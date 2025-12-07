//! Layout message handlers (split views, tabs, groups)

use std::path::PathBuf;

use crate::commands::Cmd;
use crate::messages::LayoutMsg;
use crate::model::{
    AppModel, Document, EditorGroup, EditorState, GroupId, LayoutNode, SplitContainer,
    SplitDirection, Tab, TabId,
};

/// Handle layout messages (split views, tabs, groups)
pub fn update_layout(model: &mut AppModel, msg: LayoutMsg) -> Option<Cmd> {
    match msg {
        LayoutMsg::NewTab => {
            new_tab_in_focused_group(model);
            Some(Cmd::Redraw)
        }

        LayoutMsg::OpenFileInNewTab(path) => {
            open_file_in_new_tab(model, path);
            Some(Cmd::Redraw)
        }

        LayoutMsg::SplitFocused(direction) => {
            split_focused_group(model, direction);
            Some(Cmd::Redraw)
        }

        LayoutMsg::SplitGroup {
            group_id,
            direction,
        } => {
            split_group(model, group_id, direction);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseGroup(group_id) => {
            close_group(model, group_id);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseFocusedGroup => {
            let group_id = model.editor_area.focused_group_id;
            close_group(model, group_id);
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusGroup(group_id) => {
            if model.editor_area.groups.contains_key(&group_id) {
                model.editor_area.focused_group_id = group_id;
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusNextGroup => {
            focus_adjacent_group(model, true);
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusPrevGroup => {
            focus_adjacent_group(model, false);
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusGroupByIndex(index) => {
            // 1-indexed for keyboard shortcuts (Cmd+1, Cmd+2, etc.)
            let group_ids: Vec<GroupId> = collect_group_ids(&model.editor_area.layout);
            if index > 0 && index <= group_ids.len() {
                model.editor_area.focused_group_id = group_ids[index - 1];
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::MoveTab { tab_id, to_group } => {
            move_tab(model, tab_id, to_group);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseTab(tab_id) => {
            close_tab(model, tab_id);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseFocusedTab => {
            if let Some(tab) = model
                .editor_area
                .focused_group()
                .and_then(|g| g.active_tab())
            {
                let tab_id = tab.id;
                close_tab(model, tab_id);
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::NextTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = (group.active_tab_index + 1) % group.tabs.len();
                }
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::PrevTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = if group.active_tab_index == 0 {
                        group.tabs.len() - 1
                    } else {
                        group.active_tab_index - 1
                    };
                }
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::SwitchToTab(index) => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if index < group.tabs.len() {
                    group.active_tab_index = index;
                }
            }
            Some(Cmd::Redraw)
        }
    }
}

// ============================================================================
// Layout Helper Functions
// ============================================================================

/// Create a new untitled document in the focused group
fn new_tab_in_focused_group(model: &mut AppModel) {
    let group_id = model.editor_area.focused_group_id;

    // 1. Create new untitled document
    let doc_id = model.editor_area.next_document_id();
    let untitled_name = model.editor_area.next_untitled_name();
    let mut document = Document::new();
    document.id = Some(doc_id);
    document.untitled_name = Some(untitled_name);
    model.editor_area.documents.insert(doc_id, document);

    // 2. Create new editor state for this document
    let editor_id = model.editor_area.next_editor_id();
    let mut editor = EditorState::new();
    editor.id = Some(editor_id);
    editor.document_id = Some(doc_id);
    model.editor_area.editors.insert(editor_id, editor);

    // 3. Create tab in focused group
    let tab_id = model.editor_area.next_tab_id();
    let tab = Tab {
        id: tab_id,
        editor_id,
        is_pinned: false,
        is_preview: false,
    };

    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.push(tab);
        group.active_tab_index = group.tabs.len() - 1;
    }
}

/// Open a file in a new tab in the focused group
fn open_file_in_new_tab(model: &mut AppModel, path: PathBuf) {
    let group_id = model.editor_area.focused_group_id;

    // 1. Load the document from file
    let doc_id = model.editor_area.next_document_id();
    let document = match Document::from_file(path.clone()) {
        Ok(mut doc) => {
            doc.id = Some(doc_id);
            model.ui.set_status(format!("Opened: {}", path.display()));
            doc
        }
        Err(e) => {
            model
                .ui
                .set_status(format!("Error opening {}: {}", path.display(), e));
            return;
        }
    };
    model.editor_area.documents.insert(doc_id, document);

    // 2. Create new editor state for this document
    let editor_id = model.editor_area.next_editor_id();
    let mut editor = EditorState::new();
    editor.id = Some(editor_id);
    editor.document_id = Some(doc_id);
    model.editor_area.editors.insert(editor_id, editor);

    // 3. Create tab in focused group
    let tab_id = model.editor_area.next_tab_id();
    let tab = Tab {
        id: tab_id,
        editor_id,
        is_pinned: false,
        is_preview: false,
    };

    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.push(tab);
        group.active_tab_index = group.tabs.len() - 1;
    }
}

/// Split the focused group in the given direction
fn split_focused_group(model: &mut AppModel, direction: SplitDirection) {
    let group_id = model.editor_area.focused_group_id;
    split_group(model, group_id, direction);
}

/// Split a specific group in the given direction
fn split_group(model: &mut AppModel, group_id: GroupId, direction: SplitDirection) {
    // Get the document ID from the active tab in the group to split
    let doc_id = {
        let group = match model.editor_area.groups.get(&group_id) {
            Some(g) => g,
            None => return,
        };
        let editor_id = match group.active_editor_id() {
            Some(id) => id,
            None => return,
        };
        match model.editor_area.editors.get(&editor_id) {
            Some(e) => match e.document_id {
                Some(id) => id,
                None => return,
            },
            None => return,
        }
    };

    // Create a new editor for the same document
    let new_editor_id = model.editor_area.next_editor_id();
    let new_editor = {
        let mut editor = EditorState::new();
        editor.id = Some(new_editor_id);
        editor.document_id = Some(doc_id);
        editor
    };
    model.editor_area.editors.insert(new_editor_id, new_editor);

    // Create a new tab for the new editor
    let new_tab_id = model.editor_area.next_tab_id();
    let new_tab = Tab {
        id: new_tab_id,
        editor_id: new_editor_id,
        is_pinned: false,
        is_preview: false,
    };

    // Create a new group with the new tab
    let new_group_id = model.editor_area.next_group_id();
    let new_group = EditorGroup {
        id: new_group_id,
        tabs: vec![new_tab],
        active_tab_index: 0,
        rect: Default::default(),
    };
    model.editor_area.groups.insert(new_group_id, new_group);

    // Update the layout tree to include the new group
    insert_split_in_layout(
        &mut model.editor_area.layout,
        group_id,
        new_group_id,
        direction,
    );

    // Focus the new group
    model.editor_area.focused_group_id = new_group_id;
}

/// Insert a split into the layout tree, replacing the target group with a split container
fn insert_split_in_layout(
    layout: &mut LayoutNode,
    target_group: GroupId,
    new_group: GroupId,
    direction: SplitDirection,
) {
    match layout {
        LayoutNode::Group(id) if *id == target_group => {
            // Replace this group with a split containing both groups
            *layout = LayoutNode::Split(SplitContainer {
                direction,
                children: vec![
                    LayoutNode::Group(target_group),
                    LayoutNode::Group(new_group),
                ],
                ratios: vec![0.5, 0.5],
                min_sizes: vec![100.0, 100.0],
            });
        }
        LayoutNode::Group(_) => {
            // Not the target group, nothing to do
        }
        LayoutNode::Split(container) => {
            // Recursively search children
            for child in &mut container.children {
                insert_split_in_layout(child, target_group, new_group, direction);
            }
        }
    }
}

/// Close a group and remove it from the layout
fn close_group(model: &mut AppModel, group_id: GroupId) {
    // Don't close the last group
    if model.editor_area.groups.len() <= 1 {
        return;
    }

    // Remove the group from the layout tree
    let removed = remove_group_from_layout(&mut model.editor_area.layout, group_id);
    if !removed {
        return;
    }

    // Clean up the group's tabs and editors
    if let Some(group) = model.editor_area.groups.remove(&group_id) {
        for tab in group.tabs {
            model.editor_area.editors.remove(&tab.editor_id);
        }
    }

    // If we closed the focused group, focus another group
    if model.editor_area.focused_group_id == group_id {
        let group_ids: Vec<GroupId> = collect_group_ids(&model.editor_area.layout);
        if let Some(&new_focus) = group_ids.first() {
            model.editor_area.focused_group_id = new_focus;
        }
    }
}

/// Remove a group from the layout tree, collapsing splits as needed
/// Returns true if the group was found and removed
fn remove_group_from_layout(layout: &mut LayoutNode, group_id: GroupId) -> bool {
    match layout {
        LayoutNode::Group(id) => {
            // Can't remove at this level - parent needs to handle it
            *id == group_id
        }
        LayoutNode::Split(container) => {
            // Find and remove the group from children
            let mut found_index = None;
            for (i, child) in container.children.iter().enumerate() {
                if let LayoutNode::Group(id) = child {
                    if *id == group_id {
                        found_index = Some(i);
                        break;
                    }
                }
            }

            if let Some(index) = found_index {
                container.children.remove(index);
                container.ratios.remove(index);
                if !container.min_sizes.is_empty() {
                    container
                        .min_sizes
                        .remove(index.min(container.min_sizes.len() - 1));
                }

                // Normalize ratios
                let sum: f32 = container.ratios.iter().sum();
                if sum > 0.0 {
                    for ratio in &mut container.ratios {
                        *ratio /= sum;
                    }
                }

                // If only one child remains, collapse the split
                if container.children.len() == 1 {
                    let remaining = container.children.remove(0);
                    *layout = remaining;
                }

                return true;
            }

            // Recursively search children
            for child in &mut container.children {
                if remove_group_from_layout(child, group_id) {
                    // Check if we need to collapse after recursive removal
                    if let LayoutNode::Split(inner) = child {
                        if inner.children.len() == 1 {
                            let remaining = inner.children.remove(0);
                            *child = remaining;
                        }
                    }
                    return true;
                }
            }

            false
        }
    }
}

/// Collect all group IDs from the layout tree (in order)
fn collect_group_ids(layout: &LayoutNode) -> Vec<GroupId> {
    match layout {
        LayoutNode::Group(id) => vec![*id],
        LayoutNode::Split(container) => container
            .children
            .iter()
            .flat_map(collect_group_ids)
            .collect(),
    }
}

/// Focus the next or previous group
fn focus_adjacent_group(model: &mut AppModel, next: bool) {
    let group_ids = collect_group_ids(&model.editor_area.layout);
    if group_ids.len() <= 1 {
        return;
    }

    let current_idx = group_ids
        .iter()
        .position(|&id| id == model.editor_area.focused_group_id)
        .unwrap_or(0);

    let new_idx = if next {
        (current_idx + 1) % group_ids.len()
    } else if current_idx == 0 {
        group_ids.len() - 1
    } else {
        current_idx - 1
    };

    model.editor_area.focused_group_id = group_ids[new_idx];
}

/// Move a tab to a different group
fn move_tab(model: &mut AppModel, tab_id: TabId, to_group: GroupId) {
    // Verify target group exists before proceeding
    if !model.editor_area.groups.contains_key(&to_group) {
        return;
    }

    // Find the tab and its source group
    let mut found = None;
    for (gid, group) in &model.editor_area.groups {
        if let Some(idx) = group.tabs.iter().position(|t| t.id == tab_id) {
            found = Some((*gid, idx));
            break;
        }
    }

    let (source_group_id, tab_idx) = match found {
        Some(f) => f,
        None => return,
    };

    // Don't allow moving if it would leave the last group empty
    let source_group = match model.editor_area.groups.get(&source_group_id) {
        Some(g) => g,
        None => return,
    };

    if source_group.tabs.len() == 1 && model.editor_area.groups.len() == 1 {
        // Can't move the last tab from the last group
        return;
    }

    // Remove the tab from source group
    let tab = model
        .editor_area
        .groups
        .get_mut(&source_group_id)
        .unwrap()
        .tabs
        .remove(tab_idx);

    // Adjust active tab index in source group
    if let Some(source) = model.editor_area.groups.get_mut(&source_group_id) {
        if source.active_tab_index >= source.tabs.len() && !source.tabs.is_empty() {
            source.active_tab_index = source.tabs.len() - 1;
        }
    }

    // Add the tab to the target group
    if let Some(target_group) = model.editor_area.groups.get_mut(&to_group) {
        target_group.tabs.push(tab);
        target_group.active_tab_index = target_group.tabs.len() - 1;
    }

    // If source group is now empty, close it (unless it's the last group)
    if model
        .editor_area
        .groups
        .get(&source_group_id)
        .is_some_and(|g| g.tabs.is_empty())
        && model.editor_area.groups.len() > 1
    {
        close_group(model, source_group_id);
    }
}

/// Close a specific tab
fn close_tab(model: &mut AppModel, tab_id: TabId) {
    // Find the tab and its group
    let mut found = None;
    for (gid, group) in &model.editor_area.groups {
        if let Some(idx) = group.tabs.iter().position(|t| t.id == tab_id) {
            found = Some((*gid, idx));
            break;
        }
    }

    let (group_id, tab_idx) = match found {
        Some(f) => f,
        None => return,
    };

    // Check if this is the last tab in the last group - don't allow closing it
    let group = match model.editor_area.groups.get(&group_id) {
        Some(g) => g,
        None => return,
    };

    if group.tabs.len() == 1 && model.editor_area.groups.len() == 1 {
        // Can't close the last tab in the last group
        return;
    }

    // Get editor_id before removing
    let editor_id = model.editor_area.groups[&group_id].tabs[tab_idx].editor_id;

    // Remove the tab
    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.remove(tab_idx);
        if group.active_tab_index >= group.tabs.len() && !group.tabs.is_empty() {
            group.active_tab_index = group.tabs.len() - 1;
        }
    }

    // Remove the editor
    model.editor_area.editors.remove(&editor_id);

    // If the group is now empty, close it (unless it's the last group)
    if model
        .editor_area
        .groups
        .get(&group_id)
        .is_some_and(|g| g.tabs.is_empty())
        && model.editor_area.groups.len() > 1
    {
        close_group(model, group_id);
    }
}
