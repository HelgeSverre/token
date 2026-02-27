//! Layout message handlers (split views, tabs, groups)

use std::path::PathBuf;

use crate::commands::Cmd;
use crate::messages::LayoutMsg;
use crate::model::ui::SplitterDragState;
use crate::model::{
    AppModel, Document, EditorGroup, EditorState, GroupId, LayoutNode, Rect, SplitContainer,
    SplitDirection, Tab, TabId,
};
use crate::model::editor::{BinaryPlaceholderState, ImageTabState, TabContent};
use crate::util::{
    filename_for_display, is_likely_binary, is_supported_image, validate_file_for_opening,
    FileOpenError,
};

use super::syntax::schedule_syntax_parse;

/// Drag threshold in pixels before drag becomes active
const DRAG_THRESHOLD_PIXELS: f32 = 4.0;
/// Minimum pane size in pixels
const MIN_PANE_SIZE_PIXELS: f32 = 100.0;

/// Handle layout messages (split views, tabs, groups)
pub fn update_layout(model: &mut AppModel, msg: LayoutMsg) -> Option<Cmd> {
    match msg {
        LayoutMsg::NewTab => {
            new_tab_in_focused_group(model);
            sync_viewports(model);
            Some(Cmd::Redraw)
        }

        LayoutMsg::OpenFileInNewTab(path) => {
            let cmd = open_file_in_new_tab(model, path);
            sync_viewports(model);
            cmd
        }

        LayoutMsg::OpenWithDefaultApp(path) => Some(Cmd::OpenInExplorer { path }),

        LayoutMsg::SplitFocused(direction) => {
            split_focused_group(model, direction);
            sync_viewports(model);
            Some(Cmd::Redraw)
        }

        LayoutMsg::SplitGroup {
            group_id,
            direction,
        } => {
            split_group(model, group_id, direction);
            sync_viewports(model);
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
            model.outline_panel.scroll_offset = 0;
            model.outline_panel.selected_index = None;
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::FocusNextGroup => {
            focus_adjacent_group(model, true);
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::FocusPrevGroup => {
            focus_adjacent_group(model, false);
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::FocusGroupByIndex(index) => {
            // 1-indexed for keyboard shortcuts (Cmd+1, Cmd+2, etc.)
            let group_ids: Vec<GroupId> = collect_group_ids(&model.editor_area.layout);
            if index > 0 && index <= group_ids.len() {
                model.editor_area.focused_group_id = group_ids[index - 1];
            }
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::MoveTab { tab_id, to_group } => {
            move_tab(model, tab_id, to_group);
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::CloseTab(tab_id) => {
            close_tab(model, tab_id);
            Some(Cmd::redraw_editor())
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
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::NextTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = (group.active_tab_index + 1) % group.tabs.len();
                }
            }
            close_preview_if_not_markdown(model);
            model.outline_panel.scroll_offset = 0;
            model.outline_panel.selected_index = None;
            Some(Cmd::redraw_editor())
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
            close_preview_if_not_markdown(model);
            model.outline_panel.scroll_offset = 0;
            model.outline_panel.selected_index = None;
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::SwitchToTab(index) => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if index < group.tabs.len() {
                    group.active_tab_index = index;
                }
            }
            close_preview_if_not_markdown(model);
            model.outline_panel.scroll_offset = 0;
            model.outline_panel.selected_index = None;
            Some(Cmd::redraw_editor())
        }

        // === Splitter Dragging ===
        LayoutMsg::BeginSplitterDrag {
            splitter_index,
            position,
        } => {
            begin_splitter_drag(model, splitter_index, position);
            Some(Cmd::Redraw)
        }

        LayoutMsg::UpdateSplitterDrag { position } => {
            update_splitter_drag(model, position);
            Some(Cmd::Redraw)
        }

        LayoutMsg::EndSplitterDrag => {
            model.ui.splitter_drag = None;
            Some(Cmd::Redraw)
        }

        LayoutMsg::CancelSplitterDrag => {
            cancel_splitter_drag(model);
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
fn open_file_in_new_tab(model: &mut AppModel, path: PathBuf) -> Option<Cmd> {
    let filename = filename_for_display(&path);

    // 0. Check if file is already open - if so, focus it instead
    if let Some((_doc_id, group_id, tab_idx)) = model.editor_area.find_open_file(&path) {
        model.editor_area.focused_group_id = group_id;
        if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
            group.active_tab_index = tab_idx;
        }
        model.ui.set_status(format!("Switched to: {}", filename));
        return Some(Cmd::Redraw);
    }

    let group_id = model.editor_area.focused_group_id;

    // 1. Validate file and load/create document
    let doc_id = model.editor_area.next_document_id();
    let document = match validate_file_for_opening(&path) {
        Ok(()) => {
            // File exists - check for image files first
            if is_supported_image(&path) {
                // TODO: image::open() blocks the main thread during decode. For large images or slow
                // drives this freezes the UI. Fix: add Cmd::LoadImage to spawn decode on a background
                // thread, post Msg::ImageLoaded back via EventLoopProxy, and show a loading state.
                let img = match image::open(&path) {
                    Ok(img) => img.to_rgba8(),
                    Err(e) => {
                        model
                            .ui
                            .set_status(format!("Error opening image {}: {}", filename, e));
                        return Some(Cmd::Redraw);
                    }
                };
                let (width, height) = img.dimensions();
                let pixels = img.into_raw();

                let mut doc = Document::new();
                doc.id = Some(doc_id);
                doc.file_path = Some(path.clone());
                model.editor_area.documents.insert(doc_id, doc);
                model.record_file_opened(path.clone());

                let editor_id = model.editor_area.next_editor_id();
                let mut editor = EditorState::new();
                editor.id = Some(editor_id);
                editor.document_id = Some(doc_id);
                editor.tab_content = TabContent::Image(ImageTabState {
                    path,
                    pixels,
                    width,
                    height,
                });
                model.editor_area.editors.insert(editor_id, editor);

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

                model
                    .ui
                    .set_status(format!("Opened image: {} ({}×{})", filename, width, height));
                return Some(Cmd::Redraw);
            }

            // Check for binary content
            if is_likely_binary(&path) {
                let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                let mut doc = Document::new();
                doc.id = Some(doc_id);
                doc.file_path = Some(path.clone());
                model.editor_area.documents.insert(doc_id, doc);
                model.record_file_opened(path.clone());

                let editor_id = model.editor_area.next_editor_id();
                let mut editor = EditorState::new();
                editor.id = Some(editor_id);
                editor.document_id = Some(doc_id);
                editor.tab_content = TabContent::BinaryPlaceholder(BinaryPlaceholderState {
                    path,
                    size_bytes,
                });
                model.editor_area.editors.insert(editor_id, editor);

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

                model
                    .ui
                    .set_status(format!("Opened binary file: {}", filename));
                return Some(Cmd::Redraw);
            }

            // Load text document from file
            match Document::from_file(path.clone()) {
                Ok(mut doc) => {
                    doc.id = Some(doc_id);
                    model.ui.set_status(format!("Opened: {}", path.display()));
                    doc
                }
                Err(e) => {
                    model
                        .ui
                        .set_status(format!("Error opening {}: {}", path.display(), e));
                    return Some(Cmd::Redraw);
                }
            }
        }
        Err(FileOpenError::NotFound) => {
            // File doesn't exist - create new document with this path
            let mut doc = Document::new_with_path(path.clone());
            doc.id = Some(doc_id);
            model.ui.set_status(format!("New file: {}", path.display()));
            doc
        }
        Err(e) => {
            model.ui.set_status(e.user_message(&filename));
            return Some(Cmd::Redraw);
        }
    };
    model.editor_area.documents.insert(doc_id, document);

    // Record in recent files
    model.record_file_opened(path);

    // 4. Create new editor state for this document
    let editor_id = model.editor_area.next_editor_id();
    let mut editor = EditorState::new();
    editor.id = Some(editor_id);
    editor.document_id = Some(doc_id);
    model.editor_area.editors.insert(editor_id, editor);

    // 5. Create tab in focused group
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

    // 6. Schedule syntax parsing for the new document
    if let Some(parse_cmd) = schedule_syntax_parse(model, doc_id) {
        Some(Cmd::Batch(vec![Cmd::Redraw, parse_cmd]))
    } else {
        Some(Cmd::Redraw)
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
        attached_preview: None,
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
        LayoutNode::Empty => {}
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
        LayoutNode::Preview(_) => {
            // Preview panes are not split targets
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
        LayoutNode::Empty => false,
        LayoutNode::Group(id) => {
            // Can't remove at this level - parent needs to handle it
            *id == group_id
        }
        LayoutNode::Preview(_) => false,
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
        LayoutNode::Empty => vec![],
        LayoutNode::Group(id) => vec![*id],
        LayoutNode::Preview(_) => vec![],
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

    // Get editor_id and doc_id before removing
    let editor_id = model.editor_area.groups[&group_id].tabs[tab_idx].editor_id;
    let doc_id = model
        .editor_area
        .editors
        .get(&editor_id)
        .and_then(|e| e.document_id);

    // Remove the tab
    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.remove(tab_idx);
        if group.active_tab_index >= group.tabs.len() && !group.tabs.is_empty() {
            group.active_tab_index = group.tabs.len() - 1;
        }
    }

    // Remove the editor
    model.editor_area.editors.remove(&editor_id);

    // Close any preview attached to this group if it was for this document
    if let Some(did) = doc_id {
        if let Some(preview_id) = model.editor_area.find_preview_for_group(group_id) {
            if model
                .editor_area
                .previews
                .get(&preview_id)
                .is_some_and(|p| p.document_id == did)
            {
                model.editor_area.close_preview(preview_id);
            }
        }
    }

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

// ============================================================================
// Splitter Drag Helper Functions
// ============================================================================

/// Begin dragging a splitter
fn begin_splitter_drag(model: &mut AppModel, splitter_index: usize, position: (f32, f32)) {
    // We need to find:
    // 1. The SplitterBar for this index (to get direction and local index)
    // 2. The container that owns this splitter (to get ratios and size)
    // 3. The container's size in the relevant direction

    // First compute the layout to get splitter info
    // Use last_layout_rect which includes sidebar offset from render pass
    // Fallback includes sidebar offset for consistency
    let sidebar_width = model
        .workspace
        .as_ref()
        .filter(|ws| ws.sidebar_visible)
        .map(|ws| ws.sidebar_width(model.metrics.scale_factor))
        .unwrap_or(0.0);
    let available = model.editor_area.last_layout_rect.unwrap_or(Rect::new(
        sidebar_width,
        0.0,
        800.0 - sidebar_width,
        600.0,
    ));
    let splitters = model
        .editor_area
        .compute_layout_scaled(available, model.metrics.splitter_width);

    // Get the splitter bar
    let splitter = match splitters.get(splitter_index) {
        Some(s) => *s,
        None => return,
    };

    // Find the container and its size by traversing the layout tree
    let mut current_index = 0;
    let container_info = find_container_for_splitter(
        &model.editor_area.layout,
        splitter_index,
        &mut current_index,
        available,
    );

    let (original_ratios, container_size) = match container_info {
        Some(info) => info,
        None => return,
    };

    model.ui.splitter_drag = Some(SplitterDragState {
        splitter_index,
        local_index: splitter.index,
        start_position: position,
        original_ratios,
        direction: splitter.direction,
        container_size,
        active: false,
    });
}

/// Update splitter position during drag
fn update_splitter_drag(model: &mut AppModel, position: (f32, f32)) {
    // Extract needed fields without cloning the Vec<f32> on every frame
    let (splitter_index, local_idx, start_pos, direction, container_size, active) =
        match model.ui.splitter_drag.as_ref() {
            Some(state) => (
                state.splitter_index,
                state.local_index,
                state.start_position,
                state.direction,
                state.container_size,
                state.active,
            ),
            None => return,
        };

    // Calculate delta from start position
    let delta = match direction {
        SplitDirection::Horizontal => position.0 - start_pos.0,
        SplitDirection::Vertical => position.1 - start_pos.1,
    };

    // Check threshold if not yet active
    if !active {
        let distance =
            ((position.0 - start_pos.0).powi(2) + (position.1 - start_pos.1).powi(2)).sqrt();
        if distance < DRAG_THRESHOLD_PIXELS {
            return; // Threshold not exceeded yet
        }
        // Activate the drag
        if let Some(ref mut state) = model.ui.splitter_drag {
            state.active = true;
        }
    }

    // Get the two ratios we're adjusting (defensive: return early if indices invalid)
    let (left_ratio, right_ratio) = match model.ui.splitter_drag.as_ref() {
        Some(state) => {
            match (
                state.original_ratios.get(local_idx),
                state.original_ratios.get(local_idx + 1),
            ) {
                (Some(&l), Some(&r)) => (l, r),
                _ => return, // Layout changed underneath us; ignore this drag frame
            }
        }
        None => return,
    };

    // Calculate new ratios
    let ratio_delta = delta / container_size;
    let combined = left_ratio + right_ratio;

    // Calculate minimum ratio based on minimum pane size
    // Guard against tiny containers where 2*MIN_PANE_SIZE > container_size
    // which would cause clamp(min, max) to panic when min > max
    let raw_min_ratio = MIN_PANE_SIZE_PIXELS / container_size;
    let max_min_ratio = combined / 2.0;
    let effective_min_ratio = if raw_min_ratio <= max_min_ratio {
        raw_min_ratio
    } else {
        // Container too small to satisfy min pane size for both panes;
        // allow smaller panes but keep ratios non-negative
        0.01 // Small epsilon to prevent zero-width panes
    };

    // Apply delta with constraints
    let new_left =
        (left_ratio + ratio_delta).clamp(effective_min_ratio, combined - effective_min_ratio);
    let new_right = combined - new_left;

    // Find and update the container
    update_container_ratios_by_splitter(
        &mut model.editor_area.layout,
        splitter_index,
        local_idx,
        new_left,
        new_right,
    );
}

/// Cancel splitter drag and restore original ratios
fn cancel_splitter_drag(model: &mut AppModel) {
    let drag_state = match model.ui.splitter_drag.take() {
        Some(state) => state,
        None => return,
    };

    // Restore original ratios
    restore_container_ratios_by_splitter(
        &mut model.editor_area.layout,
        drag_state.splitter_index,
        &drag_state.original_ratios,
    );
}

// ============================================================================
// Splitter Container Traversal Helpers
// ============================================================================

/// Generic helper to visit the container owning a splitter by global index.
///
/// The layout tree is traversed depth-first. Each SplitContainer with N children
/// owns N-1 splitters (one between each pair of children). The global splitter
/// index is computed by summing splitter counts during traversal.
///
/// When the target container is found, the closure `f` is called with:
/// - A mutable reference to the container
/// - The local index within that container (which child boundary)
///
/// Returns true if the target was found and the closure was called.
fn visit_splitter_container_mut<F>(
    layout: &mut LayoutNode,
    target_index: usize,
    current_index: &mut usize,
    f: &mut F,
) -> bool
where
    F: FnMut(&mut SplitContainer, usize),
{
    match layout {
        LayoutNode::Empty => false,
        LayoutNode::Group(_) => false,
        LayoutNode::Preview(_) => false,
        LayoutNode::Split(container) => {
            let splitter_count = container.children.len().saturating_sub(1);

            // Check if target is in this container's splitter range
            if target_index >= *current_index && target_index < *current_index + splitter_count {
                let local_idx = target_index - *current_index;
                f(container, local_idx);
                return true;
            }

            *current_index += splitter_count;

            // Recurse into children
            for child in &mut container.children {
                if visit_splitter_container_mut(child, target_index, current_index, f) {
                    return true;
                }
            }

            false
        }
    }
}

/// Find the container that owns a given splitter by global index.
/// Returns (original_ratios, container_size) if found.
///
/// This needs a separate implementation because it requires calculating
/// child rects during traversal to determine container size.
fn find_container_for_splitter(
    layout: &LayoutNode,
    target_index: usize,
    current_index: &mut usize,
    rect: Rect,
) -> Option<(Vec<f32>, f32)> {
    match layout {
        LayoutNode::Empty => None,
        LayoutNode::Group(_) => None,
        LayoutNode::Preview(_) => None,
        LayoutNode::Split(container) => {
            let splitter_count = container.children.len().saturating_sub(1);

            // Check if target is in this container
            if target_index >= *current_index && target_index < *current_index + splitter_count {
                let size = match container.direction {
                    SplitDirection::Horizontal => rect.width,
                    SplitDirection::Vertical => rect.height,
                };
                return Some((container.ratios.clone(), size));
            }

            *current_index += splitter_count;

            // Recurse into children with their calculated rects
            let total_size = match container.direction {
                SplitDirection::Horizontal => rect.width,
                SplitDirection::Vertical => rect.height,
            };
            let mut offset = 0.0;

            for (i, child) in container.children.iter().enumerate() {
                let ratio = container
                    .ratios
                    .get(i)
                    .copied()
                    .unwrap_or(1.0 / container.children.len() as f32);
                let child_size = total_size * ratio;

                let child_rect = match container.direction {
                    SplitDirection::Horizontal => {
                        Rect::new(rect.x + offset, rect.y, child_size, rect.height)
                    }
                    SplitDirection::Vertical => {
                        Rect::new(rect.x, rect.y + offset, rect.width, child_size)
                    }
                };

                if let Some(result) =
                    find_container_for_splitter(child, target_index, current_index, child_rect)
                {
                    return Some(result);
                }

                offset += child_size;
            }

            None
        }
    }
}

/// Update ratios in the container that owns the target splitter.
///
/// Adjusts the two adjacent ratios (at local_idx and local_idx+1) to new values.
fn update_container_ratios_by_splitter(
    layout: &mut LayoutNode,
    target_index: usize,
    local_idx: usize,
    new_left: f32,
    new_right: f32,
) -> bool {
    let mut current_index = 0;
    visit_splitter_container_mut(
        layout,
        target_index,
        &mut current_index,
        &mut |container, _| {
            if local_idx < container.ratios.len() && local_idx + 1 < container.ratios.len() {
                container.ratios[local_idx] = new_left;
                container.ratios[local_idx + 1] = new_right;
            }
        },
    )
}

/// Restore original ratios to the container that owns the target splitter.
fn restore_container_ratios_by_splitter(
    layout: &mut LayoutNode,
    target_index: usize,
    original_ratios: &[f32],
) -> bool {
    let mut current_index = 0;
    visit_splitter_container_mut(
        layout,
        target_index,
        &mut current_index,
        &mut |container, _| {
            container.ratios = original_ratios.to_vec();
        },
    )
}

/// Close preview pane if the focused group's active tab changed.
/// Called when switching tabs to ensure preview stays relevant.
fn close_preview_if_not_markdown(model: &mut AppModel) {
    let group_id = model.editor_area.focused_group_id;
    model.editor_area.on_group_active_tab_changed(group_id);
}

/// Sync all editor viewports to their group's actual dimensions.
/// Call after creating new editors or changing group layout.
fn sync_viewports(model: &mut AppModel) {
    let line_height = model.line_height;
    let char_width = model.char_width;
    let tab_bar_height = model.metrics.tab_bar_height;
    model
        .editor_area
        .sync_all_viewports(line_height, char_width, tab_bar_height);
}
