//! Dock update handlers
//!
//! Handles dock-related messages for panel visibility, focus, and resizing.

use crate::commands::Cmd;
use crate::messages::DockMsg;
use crate::model::{AppModel, FocusTarget};
use crate::panel::DockPosition;

/// Sync workspace sidebar visibility with dock layout (left dock)
fn sync_workspace_with_dock(model: &mut AppModel) {
    if let Some(workspace) = &mut model.workspace {
        workspace.sidebar_visible = model.dock_layout.left.is_open;
        workspace.sidebar_width_logical = model.dock_layout.left.size_logical;
    }
}

/// Update function for dock messages
pub fn update_dock(model: &mut AppModel, msg: DockMsg) -> Option<Cmd> {
    match msg {
        DockMsg::FocusOrTogglePanel(panel_id) => {
            let is_dock_focused = |pos: DockPosition| -> bool {
                match pos {
                    DockPosition::Left => model.ui.focus == FocusTarget::Sidebar,
                    DockPosition::Right | DockPosition::Bottom => {
                        model.ui.focus == FocusTarget::Dock(pos)
                    }
                }
            };

            let (opened, position) = model.dock_layout.focus_or_toggle_panel(panel_id, is_dock_focused);

            // Update focus based on result
            if opened {
                if let Some(pos) = position {
                    if pos == DockPosition::Left {
                        model.ui.focus = FocusTarget::Sidebar;
                    } else {
                        model.ui.focus = FocusTarget::Dock(pos);
                    }
                }
            } else {
                model.ui.focus = FocusTarget::Editor;
            }

            // Sync workspace state for left dock (file explorer)
            sync_workspace_with_dock(model);

            // Recalculate viewport dimensions since dock visibility affects editor area
            model.recalculate_viewports();

            Some(Cmd::Redraw)
        }

        DockMsg::TogglePanel(panel_id) => {
            // Pure toggle: open if closed, close if open. Focus-agnostic.
            // Used by command palette where focus state is irrelevant.
            if let Some(position) = model.dock_layout.find_panel(panel_id) {
                let dock = model.dock_layout.dock_mut(position);
                let is_active = dock.active_panel() == Some(panel_id);

                if is_active && dock.is_open {
                    // Panel is currently active and visible → close the dock
                    dock.close();
                } else {
                    // Otherwise ensure dock is open and this panel is active
                    dock.activate(panel_id);
                }

                // Sync workspace state for left dock
                if position == DockPosition::Left {
                    sync_workspace_with_dock(model);
                }

                // Recalculate viewport dimensions since dock visibility affects editor area
                model.recalculate_viewports();

                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        DockMsg::CloseFocusedDock => {
            // Close the dock that currently has focus
            match model.ui.focus {
                FocusTarget::Sidebar => {
                    model.dock_layout.close_dock(DockPosition::Left);
                    sync_workspace_with_dock(model);
                }
                FocusTarget::Dock(pos) => {
                    model.dock_layout.close_dock(pos);
                }
                _ => {}
            }
            model.ui.focus = FocusTarget::Editor;
            model.recalculate_viewports();
            Some(Cmd::Redraw)
        }

        DockMsg::FocusDock(position) => {
            model.dock_layout.dock_mut(position).is_open = true;
            if position == DockPosition::Left {
                model.ui.focus = FocusTarget::Sidebar;
                sync_workspace_with_dock(model);
            } else {
                model.ui.focus = FocusTarget::Dock(position);
            }
            model.recalculate_viewports();
            Some(Cmd::Redraw)
        }

        DockMsg::ToggleDock(position) => {
            model.dock_layout.toggle_dock(position);
            if position == DockPosition::Left {
                sync_workspace_with_dock(model);
            }
            model.recalculate_viewports();
            Some(Cmd::Redraw)
        }

        DockMsg::NextPanelInDock => {
            // Cycle in the focused dock
            match model.ui.focus {
                FocusTarget::Sidebar => {
                    model.dock_layout.next_panel_in_dock(DockPosition::Left);
                }
                FocusTarget::Dock(pos) => {
                    model.dock_layout.next_panel_in_dock(pos);
                }
                _ => {}
            }
            Some(Cmd::Redraw)
        }

        DockMsg::PrevPanelInDock => {
            match model.ui.focus {
                FocusTarget::Sidebar => {
                    model.dock_layout.prev_panel_in_dock(DockPosition::Left);
                }
                FocusTarget::Dock(pos) => {
                    model.dock_layout.prev_panel_in_dock(pos);
                }
                _ => {}
            }
            Some(Cmd::Redraw)
        }

        DockMsg::StartResize { position, initial_coord } => {
            // Store resize state - we'll need to track this somewhere
            // For now, delegate to existing sidebar resize for left dock
            if position == DockPosition::Left {
                model.ui.sidebar_resize = Some(crate::model::SidebarResizeState {
                    start_x: initial_coord,
                    original_width: model.dock_layout.left.size_logical,
                });
            }
            // TODO: add resize state for right/bottom docks
            Some(Cmd::Redraw)
        }

        DockMsg::UpdateResize { coord } => {
            // Update the dock size during drag
            if let Some(ref resize_state) = model.ui.sidebar_resize {
                let delta = coord - resize_state.start_x;
                let new_width = (resize_state.original_width as f64 + delta)
                    .max(model.dock_layout.left.min_size() as f64) as f32;
                let max_width = model.window_size.0 as f32 * model.dock_layout.left.max_size_fraction();
                model.dock_layout.left.size_logical = new_width.min(max_width);
                sync_workspace_with_dock(model);
                model.recalculate_viewports();
            }
            // TODO: handle right/bottom dock resize
            Some(Cmd::Redraw)
        }

        DockMsg::EndResize => {
            model.ui.sidebar_resize = None;
            model.recalculate_viewports();
            Some(Cmd::Redraw)
        }
    }
}
