//! Workspace message handlers (file tree sidebar)

use crate::commands::Cmd;
use crate::messages::{LayoutMsg, WorkspaceMsg};
use crate::model::AppModel;
use crate::util::visible_tree_index_of;

use super::layout::update_layout;

/// Handle workspace messages (file tree, sidebar)
pub(super) fn update_workspace(model: &mut AppModel, msg: WorkspaceMsg) -> Option<Cmd> {
    match msg {
        WorkspaceMsg::ToggleSidebar => {
            if let Some(workspace) = &mut model.workspace {
                workspace.sidebar_visible = !workspace.sidebar_visible;
                // Sync with dock layout
                model.dock_layout.left.is_open = workspace.sidebar_visible;
                // If sidebar is hidden while focused, return focus to editor
                if !workspace.sidebar_visible
                    && matches!(
                        model.ui.focus,
                        crate::model::FocusTarget::Dock(crate::panel::DockPosition::Left)
                    )
                {
                    model.ui.focus_editor();
                }
                tracing::trace!(
                    "Sidebar toggled: visible={}, focus={:?}",
                    workspace.sidebar_visible,
                    model.ui.focus
                );
            }
            // Revealing the sidebar re-clamps against the fresh viewport, so
            // an offset parked unclamped while hidden can never render.
            clamp_sidebar_scroll(model);
            Some(Cmd::redraw_editor())
        }

        WorkspaceMsg::ToggleFolder(path) => {
            if let Some(workspace) = &mut model.workspace {
                workspace.toggle_folder(&path);
            }
            clamp_sidebar_scroll(model);
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
            clamp_sidebar_scroll(model);
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
                    clamp_sidebar_scroll(model);
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

        WorkspaceMsg::RevealPath(path) => {
            reveal_path(model, &path);
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
                // Sync with dock layout
                model.dock_layout.left.size_logical = new_width_logical;
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
            // Calculate how many rows fit in the sidebar viewport
            let visible_rows = sidebar_visible_rows(model);
            if let Some(workspace) = &mut model.workspace {
                let total = workspace.visible_item_count();
                if total == 0 {
                    tracing::trace!("Sidebar scroll: no visible items");
                    return Some(Cmd::redraw_editor());
                }

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

/// Number of rows that fit in the sidebar viewport.
///
/// Uses the same solved `RowListView` the renderer and hit tester consume.
fn sidebar_visible_rows(model: &AppModel) -> usize {
    crate::layout::chrome::sidebar_rows(model)
        .row_list(crate::layout::UiKey::Sidebar)
        .map(|rows| rows.visible_capacity())
        .unwrap_or(0)
}

/// Clamp the sidebar scroll offset after the visible item count may have
/// shrunk (folder collapse, tree refresh), so the tree never scrolls past
/// its own content and renders blank.
///
/// A hidden sidebar has no viewport to clamp against; unlike the dock
/// panels (which reset to 0 when invisible), the offset is left untouched —
/// hiding the sidebar must not lose the user's place in the tree. Any path
/// that reveals the sidebar clamps immediately (`ToggleSidebar`), so a
/// stale offset can never reach a visible frame.
fn clamp_sidebar_scroll(model: &mut AppModel) {
    let rows = crate::layout::chrome::sidebar_rows(model).row_list(crate::layout::UiKey::Sidebar);
    let Some(rows) = rows else {
        return;
    };
    if let Some(workspace) = &mut model.workspace {
        workspace.scroll_offset = workspace.scroll_offset.min(rows.max_scroll());
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
    let Some(selected_index) = visible_tree_index_of(
        &workspace.file_tree.roots,
        |node: &crate::model::FileNode| {
            node.is_dir && workspace.expanded_folders.contains(&node.path)
        },
        |node: &crate::model::FileNode| &node.path == selected,
    ) else {
        return;
    };

    // Calculate viewport bounds
    let visible_rows = sidebar_visible_rows(model);

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
        visible_tree_index_of(
            &workspace.file_tree.roots,
            |node: &crate::model::FileNode| {
                node.is_dir && workspace.expanded_folders.contains(&node.path)
            },
            |node: &crate::model::FileNode| &node.path == selected,
        )
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

/// Follow navigation once per target change, without taking focus or opening docks.
pub(super) fn reconcile_auto_reveal(model: &mut AppModel) -> Option<Cmd> {
    if !model.config.explorer_auto_reveal {
        model.ui.explorer_auto_reveal = None;
        return None;
    }
    let target = (|| {
        let workspace = model.workspace.as_ref()?;
        let editor_id = model.editor_area.focused_editor_id()?;
        let path = model.editor_area.focused_document()?.file_path.as_ref()?;
        path.starts_with(&workspace.root)
            .then_some((workspace, editor_id, path))
    })();
    let Some((workspace, editor_id, path)) = target else {
        model.ui.explorer_auto_reveal = None;
        return None;
    };
    let explorer_visible = model
        .dock_layout
        .active_panel_position(crate::panel::PanelId::FILE_EXPLORER)
        .is_some();
    let same_target = model.ui.explorer_auto_reveal.as_ref().is_some_and(|last| {
        last.editor_id == editor_id && last.path == *path && last.workspace_root == workspace.root
    });
    if same_target {
        let last = model.ui.explorer_auto_reveal.as_mut()?;
        let became_visible = explorer_visible && !last.explorer_visible;
        last.explorer_visible = explorer_visible;
        // Respect manual tree selection and scrolling between navigations.
        // When reopening the explorer, scroll its current selection into view.
        if became_visible {
            ensure_selection_visible(model);
            return Some(Cmd::Redraw);
        }
        return None;
    }

    let path = path.clone();
    model.ui.explorer_auto_reveal = Some(crate::model::workspace::AutoRevealTarget {
        editor_id,
        path: path.clone(),
        workspace_root: workspace.root.clone(),
        explorer_visible,
    });
    if let Some(workspace) = &mut model.workspace {
        workspace.reveal_file(&path);
    }
    if explorer_visible {
        ensure_selection_visible(model);
    }
    Some(Cmd::Redraw)
}

/// Reveal the currently active file in the tree.
fn reveal_active_file(model: &mut AppModel) {
    let active_path = model
        .editor_area
        .focused_document()
        .and_then(|doc| doc.file_path.clone());

    let Some(path) = active_path else {
        model.ui.set_status("No file to reveal");
        return;
    };

    reveal_path(model, &path);
}

/// Expand/select/scroll the sidebar tree to `path` — the shared body of
/// `RevealActiveFile` (path = the focused document's) and `RevealPath`
/// (path = whatever the caller resolved, e.g. a right-clicked tab).
fn reveal_path(model: &mut AppModel, path: &std::path::Path) {
    let Some(workspace) = &mut model.workspace else {
        model.ui.set_status("No workspace open");
        return;
    };

    // Check if file is within workspace
    if !path.starts_with(&workspace.root) {
        model.ui.set_status("File is outside workspace");
        return;
    }

    // Show + focus the sidebar first (opens the dock if hidden, sets
    // FocusTarget::Dock(Left), syncs sidebar_visible, recalculates
    // viewports), then expand/select/scroll — so the scroll math runs
    // against the visible tree.
    crate::update::dock::update_dock(
        model,
        crate::messages::DockMsg::ActivatePanel(crate::panel::PanelId::FILE_EXPLORER),
    );
    if let Some(workspace) = &mut model.workspace {
        workspace.reveal_file(path);
    }
    ensure_selection_visible(model);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ScaledMetrics, Workspace};
    use std::path::PathBuf;

    fn auto_reveal_model() -> (AppModel, tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let nested = root.join("nested");
        std::fs::create_dir(&nested).unwrap();
        for index in 0..60 {
            std::fs::write(nested.join(format!("file{index:02}.rs")), "fn main() {}\n").unwrap();
        }
        let mut model = AppModel::new(800, 400, 1.0);
        model.open_workspace(root);
        (model, dir, nested.join("file59.rs"))
    }

    fn open_test_file(model: &mut AppModel, path: PathBuf) {
        let cmd = crate::update::update(
            model,
            crate::messages::Msg::Layout(LayoutMsg::OpenFileInNewTab(path)),
        );
        crate::update::finish_test_file_opens(model, cmd);
    }

    #[test]
    fn auto_reveal_palette_open_expands_scrolls_and_preserves_editor_focus() {
        use crate::messages::{ModalMsg, Msg, UiMsg};
        let (mut model, _dir, path) = auto_reveal_model();
        crate::update::update(&mut model, Msg::Ui(UiMsg::OpenFuzzyFileFinder));
        crate::update::update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("file59.rs".into()))),
        );
        let cmd = crate::update::update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));
        crate::update::finish_test_file_opens(&mut model, cmd);

        let workspace = model.workspace.as_ref().unwrap();
        assert_eq!(model.document().file_path.as_ref(), Some(&path));
        assert_eq!(workspace.selected_item.as_ref(), Some(&path));
        assert!(workspace.expanded_folders.contains(path.parent().unwrap()));
        assert!(workspace.scroll_offset > 0);
        assert_eq!(model.ui.focus, crate::model::FocusTarget::Editor);
        assert!(model.ui.active_modal.is_none());
    }

    #[test]
    fn auto_reveal_follows_tab_changes_but_respects_manual_tree_browsing() {
        use crate::messages::Msg;
        let (mut model, _dir, last) = auto_reveal_model();
        let first = last.with_file_name("file00.rs");
        open_test_file(&mut model, first.clone());
        open_test_file(&mut model, last.clone());
        crate::update::update(&mut model, Msg::Layout(LayoutMsg::PrevTab));
        assert_eq!(
            model.workspace.as_ref().unwrap().selected_item,
            Some(first.clone())
        );

        crate::update::update(
            &mut model,
            Msg::Workspace(WorkspaceMsg::SelectItem(last.clone())),
        );
        crate::update::update(
            &mut model,
            Msg::Workspace(WorkspaceMsg::CollapseFolder(first.parent().unwrap().into())),
        );
        assert_eq!(
            model.workspace.as_ref().unwrap().selected_item,
            Some(last.clone())
        );
        assert!(!model
            .workspace
            .as_ref()
            .unwrap()
            .expanded_folders
            .contains(first.parent().unwrap()));

        crate::update::update(&mut model, Msg::Layout(LayoutMsg::NextTab));
        assert_eq!(model.workspace.as_ref().unwrap().selected_item, Some(last));
        assert!(model
            .workspace
            .as_ref()
            .unwrap()
            .expanded_folders
            .contains(first.parent().unwrap()));
    }

    #[test]
    fn auto_reveal_reopening_active_file_reveals_it_again() {
        use crate::messages::Msg;
        let (mut model, _dir, path) = auto_reveal_model();
        open_test_file(&mut model, path.clone());
        crate::update::update(
            &mut model,
            Msg::Workspace(WorkspaceMsg::SelectItem(path.parent().unwrap().into())),
        );
        crate::update::update(
            &mut model,
            Msg::Workspace(WorkspaceMsg::CollapseFolder(path.parent().unwrap().into())),
        );
        open_test_file(&mut model, path.clone());
        let workspace = model.workspace.as_ref().unwrap();
        assert_eq!(workspace.selected_item.as_ref(), Some(&path));
        assert!(workspace.expanded_folders.contains(path.parent().unwrap()));
        assert!(workspace.scroll_offset > 0);
    }

    #[test]
    fn auto_reveal_keeps_hidden_explorer_closed_then_scrolls_when_shown() {
        use crate::messages::{DockMsg, Msg};
        use crate::panel::PanelId;
        let (mut model, _dir, path) = auto_reveal_model();
        crate::update::update(
            &mut model,
            Msg::Dock(DockMsg::TogglePanel(PanelId::FILE_EXPLORER)),
        );
        open_test_file(&mut model, path.clone());
        assert!(!model.dock_layout.left.is_open);
        assert_eq!(model.ui.focus, crate::model::FocusTarget::Editor);
        assert_eq!(model.workspace.as_ref().unwrap().selected_item, Some(path));
        assert_eq!(model.workspace.as_ref().unwrap().scroll_offset, 0);
        crate::update::update(
            &mut model,
            Msg::Dock(DockMsg::TogglePanel(PanelId::FILE_EXPLORER)),
        );
        assert!(model.workspace.as_ref().unwrap().scroll_offset > 0);
    }

    #[test]
    fn auto_reveal_opt_out_preserves_manual_reveal_and_outside_files_are_ignored() {
        use crate::messages::Msg;
        let (mut model, _dir, path) = auto_reveal_model();
        model.config.explorer_auto_reveal = false;
        open_test_file(&mut model, path.clone());
        assert!(model.workspace.as_ref().unwrap().selected_item.is_none());
        crate::update::update(&mut model, Msg::Workspace(WorkspaceMsg::RevealActiveFile));
        assert_eq!(
            model.workspace.as_ref().unwrap().selected_item,
            Some(path.clone())
        );

        model.config.explorer_auto_reveal = true;
        let outside = tempfile::NamedTempFile::new().unwrap();
        open_test_file(&mut model, outside.path().to_path_buf());
        assert_eq!(model.workspace.as_ref().unwrap().selected_item, Some(path));
    }

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
    fn reveal_active_file_expands_selects_and_focuses_the_sidebar() {
        use crate::model::FocusTarget;
        use crate::panel::DockPosition;

        let mut model = AppModel::new(800, 600, 1.0);
        let mut ws = test_workspace();
        ws.sidebar_visible = false; // hidden sidebar must be shown by reveal
        model.workspace = Some(ws);
        model.dock_layout.left.is_open = false;
        model.document_mut().file_path = Some(PathBuf::from("/test/src/deep/nested/file.rs"));

        update_workspace(&mut model, WorkspaceMsg::RevealActiveFile);

        let ws = model.workspace.as_ref().unwrap();
        assert!(ws.sidebar_visible, "reveal must show a hidden sidebar");
        assert!(ws.expanded_folders.contains(&PathBuf::from("/test/src")));
        assert!(ws
            .expanded_folders
            .contains(&PathBuf::from("/test/src/deep/nested")));
        assert_eq!(
            ws.selected_item,
            Some(PathBuf::from("/test/src/deep/nested/file.rs"))
        );
        assert_eq!(
            model.ui.focus,
            FocusTarget::Dock(DockPosition::Left),
            "reveal must move keyboard focus to the file explorer"
        );
    }

    #[test]
    fn reveal_active_file_re_expands_a_collapsed_root() {
        let mut model = AppModel::new(800, 600, 1.0);
        let mut ws = test_workspace();
        ws.collapse_folder(&ws.root.clone()); // simulate the user collapsing the root row
        assert!(!ws.expanded_folders.contains(&ws.root));
        model.workspace = Some(ws);
        model.document_mut().file_path = Some(PathBuf::from("/test/src/deep/nested/file.rs"));

        update_workspace(&mut model, WorkspaceMsg::RevealActiveFile);

        let ws = model.workspace.as_ref().unwrap();
        assert!(
            ws.expanded_folders.contains(&ws.root),
            "revealing a file must re-expand a collapsed root, or every \
             descendant stays hidden despite their own expansion state"
        );
        assert!(ws.expanded_folders.contains(&PathBuf::from("/test/src")));
        assert!(ws
            .expanded_folders
            .contains(&PathBuf::from("/test/src/deep/nested")));
    }

    #[test]
    fn reveal_active_file_outside_workspace_keeps_status_behavior() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.workspace = Some(test_workspace());
        model.document_mut().file_path = Some(PathBuf::from("/elsewhere/file.rs"));

        update_workspace(&mut model, WorkspaceMsg::RevealActiveFile);

        assert!(model.workspace.as_ref().unwrap().selected_item.is_none());
        assert!(!matches!(
            model.ui.focus,
            crate::model::FocusTarget::Dock(_)
        ));
    }

    #[test]
    fn test_toggle_sidebar() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.workspace = Some(test_workspace());

        assert!(model.workspace.as_ref().unwrap().sidebar_visible);
        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar);
        assert!(!model.workspace.as_ref().unwrap().sidebar_visible);
        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar);
        assert!(model.workspace.as_ref().unwrap().sidebar_visible);
    }

    #[test]
    fn revealing_the_sidebar_reclamps_a_stale_scroll_offset() {
        // While hidden, the sidebar has no viewport and its offset is left
        // untouched (it may even be parked unclamped by keyboard nav).
        // Toggling it back on must clamp against the fresh viewport so a
        // stale offset can never reach a visible frame.
        let mut model = AppModel::new(800, 600, 1.0);
        let mut ws = test_workspace();
        for i in 0..40 {
            ws.file_tree
                .roots
                .push(crate::model::FileNode::new_file(PathBuf::from(format!(
                    "/test/file{i}.rs"
                ))));
        }
        model.workspace = Some(ws);

        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar); // hide
        model.workspace.as_mut().unwrap().scroll_offset = 10_000;

        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar); // reveal

        let ws = model.workspace.as_ref().unwrap();
        let max_scroll = crate::layout::chrome::sidebar_rows(&model)
            .row_list(crate::layout::UiKey::Sidebar)
            .map(|rows| rows.max_scroll())
            .expect("a revealed sidebar must expose its row list");
        assert!(
            max_scroll > 0,
            "test setup: 40 rows must overflow the viewport for this assertion to bite"
        );
        assert_eq!(ws.scroll_offset, max_scroll);
    }

    #[test]
    fn hiding_the_sidebar_preserves_the_scroll_offset() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.workspace = Some(test_workspace());
        model.workspace.as_mut().unwrap().scroll_offset = 3;

        update_workspace(&mut model, WorkspaceMsg::ToggleSidebar); // hide

        assert_eq!(model.workspace.as_ref().unwrap().scroll_offset, 3);
    }

    #[test]
    fn test_toggle_folder() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.workspace = Some(test_workspace());

        let folder = PathBuf::from("/test/src");
        assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder));

        update_workspace(&mut model, WorkspaceMsg::ToggleFolder(folder.clone()));
        assert!(model.workspace.as_ref().unwrap().is_expanded(&folder));

        update_workspace(&mut model, WorkspaceMsg::ToggleFolder(folder.clone()));
        assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder));
    }
}
