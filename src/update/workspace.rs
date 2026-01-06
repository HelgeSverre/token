//! Workspace message handlers (file tree sidebar)

use std::path::PathBuf;

use crate::commands::Cmd;
use crate::messages::{LayoutMsg, WorkspaceMsg};
use crate::model::AppModel;

use super::layout::update_layout;

/// Handle workspace messages (file tree, sidebar)
pub fn update_workspace(model: &mut AppModel, msg: WorkspaceMsg) -> Option<Cmd> {
    match msg {
        WorkspaceMsg::ToggleSidebar => {
            if let Some(workspace) = &mut model.workspace {
                workspace.sidebar_visible = !workspace.sidebar_visible;
                // If sidebar is hidden while focused, return focus to editor
                if !workspace.sidebar_visible
                    && matches!(model.ui.focus, crate::model::FocusTarget::Sidebar)
                {
                    model.ui.focus_editor();
                }
                tracing::trace!(
                    "Sidebar toggled: visible={}, focus={:?}",
                    workspace.sidebar_visible,
                    model.ui.focus
                );
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::ToggleFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.toggle_folder(&path);
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::ExpandFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.expand_folder(&path);
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::CollapseFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.collapse_folder(&path);
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::SelectItem(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.selected_item = Some(path);
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::SelectPrevious => {
            select_adjacent_item(model, -1);
            ensure_selection_visible(model);
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::SelectNext => {
            select_adjacent_item(model, 1);
            ensure_selection_visible(model);
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::SelectParent => {
            select_parent_folder(model);
            ensure_selection_visible(model);
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::OpenFile { path, preview: _ } => {
            // For now, always open as a permanent tab
            // TODO: Implement preview tab behavior
            update_layout(model, LayoutMsg::OpenFileInNewTab(path))
        }

        WorkspaceMsg::OpenOrToggle => {
            // Get the selected item and determine if it's a file or folder
            let action = model.workspace.as_ref().and_then(|ws| {
                ws.selected_item.as_ref().map(|path| {
                    let is_dir = path.is_dir();
                    (path.clone(), is_dir)
                })
            });

            match action {
                Some((path, true)) => {
                    // It's a folder - toggle expansion
                    if let Some(workspace) = &mut model.workspace {
                        workspace.toggle_folder(&path);
                    }
                    Some(Cmd::redraw_editor())
                }
                Some((path, false)) => {
                    // It's a file - open it
                    update_layout(model, LayoutMsg::OpenFileInNewTab(path))
                }
                None => {
                    // No selection
                    Some(Cmd::redraw_editor())
                }
            }
        }

        WorkspaceMsg::RevealActiveFile => {
            reveal_active_file(model);
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::StartSidebarResize { initial_x } => {
            if let Some(workspace) = &model.workspace {
                model.ui.sidebar_resize = Some(crate::model::SidebarResizeState {
                    start_x: initial_x,
                    original_width: workspace.sidebar_width_logical,
                });
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::UpdateSidebarResize { x } => {
            if let (Some(workspace), Some(resize_state)) =
                (&mut model.workspace, &model.ui.sidebar_resize)
            {
                let scale_factor = model.metrics.scale_factor;
                let min_width = model.metrics.sidebar_min_width_logical;
                let max_width = model.metrics.sidebar_max_width_logical;

                // Calculate delta in logical pixels
                let delta_physical = x - resize_state.start_x;
                let delta_logical = delta_physical as f32 / scale_factor as f32;
                let new_width_logical =
                    (resize_state.original_width + delta_logical).clamp(min_width, max_width);

                workspace.sidebar_width_logical = new_width_logical;
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::EndSidebarResize => {
            model.ui.sidebar_resize = None;
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::Refresh => {
            if let Some(workspace) = &mut model.workspace {
                if let Err(e) = workspace.refresh() {
                    model.ui.set_status(format!("Failed to refresh: {}", e));
                } else {
                    model.ui.set_status("File tree refreshed");
                }
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::Scroll { lines } => {
            if let Some(workspace) = &mut model.workspace {
                let total = workspace.visible_item_count();
                if total == 0 {
                    tracing::trace!("Sidebar scroll: no visible items");
                    return Some(Cmd::redraw_editor());
                }

                // Calculate how many rows fit in the sidebar viewport
                let row_height = model.metrics.file_tree_row_height;
                let sidebar_height = model.window_size.1 as usize;
                let visible_rows = if row_height > 0 {
                    sidebar_height / row_height
                } else {
                    20 // fallback
                };

                let max_offset = total.saturating_sub(visible_rows);
                let current = workspace.scroll_offset as i32;
                let new_offset = (current + lines).clamp(0, max_offset as i32) as usize;

                tracing::trace!(
                    "Sidebar scroll: lines={}, total={}, visible_rows={}, offset: {} -> {}",
                    lines,
                    total,
                    visible_rows,
                    current,
                    new_offset
                );

                workspace.scroll_offset = new_offset;
            }
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::FileSystemChange { paths } => {
            // Incrementally update the file tree for changed paths
            // This is much faster than a full refresh
            if let Some(workspace) = &mut model.workspace {
                if paths.is_empty() {
                    // No specific paths - do full refresh
                    if let Err(e) = workspace.refresh() {
                        tracing::warn!("Failed to refresh file tree: {}", e);
                    } else {
                        tracing::debug!("File tree fully refreshed");
                    }
                } else {
                    // Incremental update for specific changed paths
                    if let Err(e) = workspace.update_paths(&paths) {
                        tracing::warn!("Failed to update file tree: {}", e);
                    } else {
                        tracing::debug!(
                            "File tree incrementally updated for {} paths",
                            paths.len()
                        );
                    }
                }
            }
            Some(Cmd::redraw_editor())
        }
    }
}

/// Ensure the selected item is visible within the sidebar viewport.
/// Scrolls up or down as needed to bring the selection into view.
fn ensure_selection_visible(model: &mut AppModel) {
    let Some(workspace) = &model.workspace else {
        return;
    };
    let Some(selected) = &workspace.selected_item else {
        return;
    };

    // Find the visible index of the selected item
    let Some(selected_index) =
        find_visible_index(&workspace.file_tree, selected, &workspace.expanded_folders)
    else {
        return;
    };

    // Calculate viewport bounds
    let row_height = model.metrics.file_tree_row_height;
    let sidebar_height = model.window_size.1 as usize;
    let visible_rows = if row_height > 0 {
        sidebar_height / row_height
    } else {
        20
    };

    let scroll_offset = workspace.scroll_offset;
    let viewport_end = scroll_offset + visible_rows;

    // Determine if we need to scroll
    let new_offset = if selected_index < scroll_offset {
        // Selection is above viewport - scroll up
        selected_index
    } else if selected_index >= viewport_end {
        // Selection is below viewport - scroll down
        selected_index.saturating_sub(visible_rows.saturating_sub(1))
    } else {
        // Already visible
        return;
    };

    // Apply the scroll
    if let Some(ws) = &mut model.workspace {
        let total = ws.visible_item_count();
        let max_offset = total.saturating_sub(visible_rows);
        ws.scroll_offset = new_offset.min(max_offset);

        tracing::trace!(
            "Auto-scroll sidebar: selected_index={}, scroll_offset: {} -> {}",
            selected_index,
            scroll_offset,
            ws.scroll_offset
        );
    }
}

/// Select the parent folder of the currently selected item
///
/// Standard file tree behavior:
/// - From a file: select its containing folder
/// - From a collapsed folder: select its parent folder
/// - From a root item: do nothing (no parent to select)
fn select_parent_folder(model: &mut AppModel) {
    let Some(workspace) = &mut model.workspace else {
        return;
    };

    let Some(selected) = workspace.selected_item.clone() else {
        return;
    };

    // Get the parent path
    let Some(parent) = selected.parent() else {
        return; // Already at filesystem root
    };
    let parent_path = parent.to_path_buf();

    // Check if parent is within the workspace (not above the root)
    if !parent_path.starts_with(&workspace.root) {
        return; // Parent is above workspace root, don't navigate there
    }

    // Check if parent exists in the file tree (it should be a visible folder)
    // If the parent folder is in the tree, select it
    if workspace
        .file_tree
        .get_visible_item_by_path(&parent_path, &workspace.expanded_folders)
        .is_some()
    {
        workspace.selected_item = Some(parent_path);
    }
}

/// Select adjacent item in the file tree
fn select_adjacent_item(model: &mut AppModel, delta: i32) {
    let Some(workspace) = &mut model.workspace else {
        return;
    };

    let visible_count = workspace.visible_item_count();
    if visible_count == 0 {
        return;
    }

    // Find current selection index
    let current_index = if let Some(selected) = &workspace.selected_item {
        find_visible_index(&workspace.file_tree, selected, &workspace.expanded_folders)
    } else {
        None
    };

    let new_index = match current_index {
        Some(idx) => {
            let new_idx = idx as i32 + delta;
            new_idx.clamp(0, visible_count as i32 - 1) as usize
        }
        None => {
            // No selection, select first or last based on direction
            if delta > 0 {
                0
            } else {
                visible_count.saturating_sub(1)
            }
        }
    };

    // Get the item at new index
    if let Some(node) = workspace
        .file_tree
        .get_visible_item(new_index, &workspace.expanded_folders)
    {
        workspace.selected_item = Some(node.path.clone());
    }
}

/// Find the visible index of a path in the file tree
fn find_visible_index(
    tree: &crate::model::FileTree,
    target: &PathBuf,
    expanded: &std::collections::HashSet<PathBuf>,
) -> Option<usize> {
    let mut current = 0;
    for node in &tree.roots {
        if let Some(idx) = find_visible_index_node(node, target, &mut current, expanded) {
            return Some(idx);
        }
    }
    None
}

fn find_visible_index_node(
    node: &crate::model::FileNode,
    target: &PathBuf,
    current: &mut usize,
    expanded: &std::collections::HashSet<PathBuf>,
) -> Option<usize> {
    if &node.path == target {
        return Some(*current);
    }
    *current += 1;

    if node.is_dir && expanded.contains(&node.path) {
        for child in &node.children {
            if let Some(idx) = find_visible_index_node(child, target, current, expanded) {
                return Some(idx);
            }
        }
    }

    None
}

/// Reveal the currently active file in the tree
fn reveal_active_file(model: &mut AppModel) {
    // Get active file path
    let active_path = model
        .editor_area
        .focused_document()
        .and_then(|doc| doc.file_path.clone());

    let Some(path) = active_path else {
        model.ui.set_status("No file to reveal");
        return;
    };

    let Some(workspace) = &mut model.workspace else {
        model.ui.set_status("No workspace open");
        return;
    };

    // Check if file is within workspace
    if !path.starts_with(&workspace.root) {
        model.ui.set_status("File is outside workspace");
        return;
    }

    workspace.reveal_file(&path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ScaledMetrics, Workspace};

    fn test_workspace() -> Workspace {
        let metrics = ScaledMetrics::new(1.0);
        Workspace {
            root: PathBuf::from("/test"),
            expanded_folders: std::collections::HashSet::new(),
            selected_item: None,
            file_tree: crate::model::FileTree::default(),
            sidebar_visible: true,
            sidebar_width_logical: metrics.sidebar_default_width_logical,
            scroll_offset: 0,
        }
    }

    #[test]
    fn test_toggle_sidebar() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.workspace = Some(test_workspace());

        assert!(model.workspace.as_ref().unwrap().sidebar_visible);
        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar);
        assert!(!model.workspace.as_ref().unwrap().sidebar_visible);
        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar);
        assert!(model.workspace.as_ref().unwrap().sidebar_visible);
    }

    #[test]
    fn test_toggle_folder() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.workspace = Some(test_workspace());

        let folder = PathBuf::from("/test/src");
        assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder));

        update_workspace(&mut model, WorkspaceMsg::ToggleFolder(folder.clone()));
        assert!(model.workspace.as_ref().unwrap().is_expanded(&folder));

        update_workspace(&mut model, WorkspaceMsg::ToggleFolder(folder.clone()));
        assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder));
    }
}
