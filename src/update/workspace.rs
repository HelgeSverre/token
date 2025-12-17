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
            }
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::ToggleFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.toggle_folder(&path);
            }
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::ExpandFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.expand_folder(&path);
            }
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::CollapseFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.collapse_folder(&path);
            }
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::SelectItem(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.selected_item = Some(path);
            }
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::SelectPrevious => {
            select_adjacent_item(model, -1);
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::SelectNext => {
            select_adjacent_item(model, 1);
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::OpenFile { path, preview: _ } => {
            // For now, always open as a permanent tab
            // TODO: Implement preview tab behavior
            update_layout(model, LayoutMsg::OpenFileInNewTab(path))
        }

        WorkspaceMsg::RevealActiveFile => {
            reveal_active_file(model);
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::StartSidebarResize { initial_x: _ } => {
            // Store initial resize state in UiState if needed
            // For now, we handle resize directly
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::UpdateSidebarResize { x } => {
            if let Some(workspace) = &mut model.workspace {
                let scale_factor = model.metrics.scale_factor;
                let min_width = workspace
                    .sidebar_width(scale_factor)
                    .max(model.metrics.sidebar_min_width_logical * scale_factor as f32);
                let max_width = model.metrics.sidebar_max_width_logical * scale_factor as f32;

                let new_width = (x as f32).clamp(min_width, max_width);
                workspace.set_sidebar_width(new_width, scale_factor);
            }
            Some(Cmd::Redraw)
        }

        WorkspaceMsg::EndSidebarResize => Some(Cmd::Redraw),

        WorkspaceMsg::Refresh => {
            if let Some(workspace) = &mut model.workspace {
                if let Err(e) = workspace.refresh() {
                    model.ui.set_status(format!("Failed to refresh: {}", e));
                } else {
                    model.ui.set_status("File tree refreshed");
                }
            }
            Some(Cmd::Redraw)
        }
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
