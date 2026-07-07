//! Dock update handlers
//!
//! Handles dock-related messages for panel visibility, focus, and resizing.

use crate::commands::Cmd;
use crate::messages::{DockMsg, TerminalMsg};
use crate::model::{AppModel, FocusTarget};
use crate::panel::{DockPosition, PanelId};
use crate::panels::terminal::{grid_size_for_rect, TerminalGridSize};
use crate::view::geometry::{DockHeaderLayout, WindowLayout};

/// Sync workspace sidebar visibility with dock layout (left dock)
fn sync_workspace_with_dock(model: &mut AppModel) {
    if let Some(workspace) = &mut model.workspace {
        workspace.sidebar_visible = model.dock_layout.left.is_open;
        workspace.sidebar_width_logical = model.dock_layout.left.size_logical;
    }
}

fn next_terminal_session_id(model: &AppModel) -> usize {
    model
        .terminal
        .sessions
        .iter()
        .map(|session| session.id)
        .max()
        .map(|session_id| session_id + 1)
        .unwrap_or(0)
}

fn is_terminal_panel_open(model: &AppModel) -> bool {
    let dock = &model.dock_layout.bottom;
    dock.is_open && dock.active_panel() == Some(PanelId::TERMINAL)
}

fn terminal_grid_size_for_model(model: &AppModel) -> Option<TerminalGridSize> {
    let window_layout = WindowLayout::compute(model, model.line_height);
    let dock_rect = window_layout.bottom_dock_rect?;
    let layout = DockHeaderLayout::new(
        &model.dock_layout.bottom,
        dock_rect,
        &model.metrics,
        model.char_width,
    );

    Some(grid_size_for_rect(
        layout.content_rect,
        model.char_width,
        model.line_height,
    ))
}

fn physical_delta_to_logical(delta: f64, scale_factor: f64) -> f64 {
    delta / scale_factor.max(f64::EPSILON)
}

fn physical_dimension_to_logical(dimension: u32, scale_factor: f64) -> f32 {
    physical_delta_to_logical(dimension as f64, scale_factor) as f32
}

fn terminal_sync_command(model: &mut AppModel) -> Option<Cmd> {
    if !is_terminal_panel_open(model) {
        return None;
    }

    let grid_size = terminal_grid_size_for_model(model)?;
    if model.terminal.sessions.is_empty() {
        if model.terminal.has_pending_spawn() {
            return None;
        }

        let session_id = next_terminal_session_id(model);
        model.terminal.mark_spawn_pending(session_id);
        return Some(Cmd::SpawnTerminal {
            session_id,
            rows: grid_size.rows,
            cols: grid_size.cols,
        });
    }

    let desired_size = (grid_size.rows as usize, grid_size.cols as usize);
    let needs_resize = model
        .terminal
        .active_session()
        .is_some_and(|session| session.size != desired_size);

    if needs_resize {
        super::terminal::update_terminal(
            model,
            TerminalMsg::Resize {
                rows: grid_size.rows,
                cols: grid_size.cols,
            },
        )
    } else {
        None
    }
}

fn push_command(cmds: &mut Vec<Cmd>, cmd: Cmd) {
    match cmd {
        Cmd::Batch(inner) => cmds.extend(inner),
        other => cmds.push(other),
    }
}

pub(super) fn with_terminal_sync(model: &mut AppModel, cmd: Cmd) -> Cmd {
    let Some(sync_cmd) = terminal_sync_command(model) else {
        return cmd;
    };

    let mut cmds = Vec::new();
    push_command(&mut cmds, sync_cmd);
    push_command(&mut cmds, cmd);
    Cmd::Batch(cmds)
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

            let (opened, position) = model
                .dock_layout
                .focus_or_toggle_panel(panel_id, is_dock_focused);

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

            Some(with_terminal_sync(model, Cmd::Redraw))
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

                Some(with_terminal_sync(model, Cmd::Redraw))
            } else {
                None
            }
        }

        DockMsg::ActivatePanel(panel_id) => {
            // Non-toggling activation: open the dock, activate the panel,
            // and focus it. Clicking an already-active dock tab is a no-op
            // rather than a close (unlike FocusOrTogglePanel).
            if let Some(position) = model.dock_layout.find_panel(panel_id) {
                model.dock_layout.dock_mut(position).activate(panel_id);

                if position == DockPosition::Left {
                    model.ui.focus = FocusTarget::Sidebar;
                    sync_workspace_with_dock(model);
                } else {
                    model.ui.focus = FocusTarget::Dock(position);
                }

                model.recalculate_viewports();
                Some(with_terminal_sync(model, Cmd::Redraw))
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
            Some(with_terminal_sync(model, Cmd::Redraw))
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
            Some(with_terminal_sync(model, Cmd::Redraw))
        }

        DockMsg::ToggleDock(position) => {
            model.dock_layout.toggle_dock(position);
            if position == DockPosition::Left {
                sync_workspace_with_dock(model);
            }
            model.recalculate_viewports();
            Some(with_terminal_sync(model, Cmd::Redraw))
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
            Some(with_terminal_sync(model, Cmd::Redraw))
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
            Some(with_terminal_sync(model, Cmd::Redraw))
        }

        DockMsg::StartResize {
            position,
            initial_coord,
        } => {
            let dock = model.dock_layout.dock(position);
            let axis = match position {
                DockPosition::Left | DockPosition::Right => {
                    crate::model::ui::DockResizeAxis::Horizontal
                }
                DockPosition::Bottom => crate::model::ui::DockResizeAxis::Vertical,
            };

            if position == DockPosition::Left {
                // Keep sidebar resize in sync for existing workspace logic.
                model.ui.sidebar_resize = Some(crate::model::ui::SidebarResizeState {
                    start_x: initial_coord,
                    original_width: dock.size_logical,
                });
            }

            if position != DockPosition::Left {
                model.ui.dock_resize = Some(crate::model::ui::DockResizeState {
                    position,
                    axis,
                    start_coord: initial_coord,
                    original_size: dock.size_logical,
                });
            }
            Some(with_terminal_sync(model, Cmd::Redraw))
        }

        DockMsg::UpdateResize { coord } => {
            // Update the dock size during drag
            if let Some(ref resize_state) = model.ui.sidebar_resize {
                let delta = physical_delta_to_logical(
                    coord - resize_state.start_x,
                    model.metrics.scale_factor,
                );
                let new_width = (resize_state.original_width as f64 + delta)
                    .max(model.dock_layout.left.min_size() as f64)
                    as f32;
                let max_width =
                    physical_dimension_to_logical(model.window_size.0, model.metrics.scale_factor)
                        * model.dock_layout.left.max_size_fraction();
                model.dock_layout.left.size_logical = new_width.min(max_width);
                sync_workspace_with_dock(model);
                model.recalculate_viewports();
            }

            if let Some(ref resize_state) = model.ui.dock_resize {
                if resize_state.position != DockPosition::Left {
                    let dock = model.dock_layout.dock_mut(resize_state.position);
                    let delta = match resize_state.position {
                        DockPosition::Right | DockPosition::Bottom => {
                            resize_state.start_coord - coord
                        }
                        DockPosition::Left => coord - resize_state.start_coord,
                    };
                    let delta = physical_delta_to_logical(delta, model.metrics.scale_factor);
                    let min_size = dock.min_size();
                    let max_size = match resize_state.position {
                        DockPosition::Right => {
                            physical_dimension_to_logical(
                                model.window_size.0,
                                model.metrics.scale_factor,
                            ) * dock.max_size_fraction()
                        }
                        DockPosition::Bottom => {
                            physical_dimension_to_logical(
                                model.window_size.1,
                                model.metrics.scale_factor,
                            ) * dock.max_size_fraction()
                        }
                        DockPosition::Left => dock.size_logical,
                    };
                    let new_size = (resize_state.original_size as f64 + delta)
                        .clamp(min_size as f64, max_size as f64)
                        as f32;
                    dock.size_logical = new_size;
                    model.recalculate_viewports();
                }
            }
            Some(with_terminal_sync(model, Cmd::Redraw))
        }

        DockMsg::EndResize => {
            model.ui.sidebar_resize = None;
            model.ui.dock_resize = None;
            model.recalculate_viewports();
            Some(with_terminal_sync(model, Cmd::Redraw))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Cmd;
    use crate::model::AppModel;
    use crate::panel::{DockPosition, PanelId};
    use crate::panels::terminal::grid_size_for_rect;
    use crate::view::geometry::{DockHeaderLayout, WindowLayout};

    fn test_model() -> AppModel {
        AppModel::new(800, 600, 1.0, vec![])
    }

    fn hidpi_test_model() -> AppModel {
        AppModel::new(1600, 1200, 2.0, vec![])
    }

    fn expected_terminal_grid_size(model: &AppModel) -> crate::panels::terminal::TerminalGridSize {
        let window_layout = WindowLayout::compute(model, model.line_height);
        let dock_rect = window_layout
            .bottom_dock_rect
            .expect("terminal dock should be open");
        let content_rect = DockHeaderLayout::new(
            &model.dock_layout.bottom,
            dock_rect,
            &model.metrics,
            model.char_width,
        )
        .content_rect;

        grid_size_for_rect(content_rect, model.char_width, model.line_height)
    }

    fn contains_spawn_terminal(cmd: &Option<Cmd>) -> bool {
        cmd.as_ref().is_some_and(cmd_contains_spawn_terminal)
    }

    fn cmd_contains_spawn_terminal(cmd: &Cmd) -> bool {
        match cmd {
            Cmd::SpawnTerminal { .. } => true,
            Cmd::Batch(cmds) => cmds.iter().any(cmd_contains_spawn_terminal),
            _ => false,
        }
    }

    #[test]
    fn opening_terminal_panel_spawns_session_sized_to_dock_content() {
        let mut model = test_model();

        let cmd = update_dock(&mut model, DockMsg::TogglePanel(PanelId::TERMINAL));
        let expected = expected_terminal_grid_size(&model);

        let Some(Cmd::Batch(cmds)) = cmd else {
            panic!("expected terminal dock open to return a batched spawn + redraw command");
        };

        assert!(cmds.iter().any(|cmd| matches!(
            cmd,
            Cmd::SpawnTerminal {
                session_id: 0,
                rows,
                cols,
            } if *rows == expected.rows && *cols == expected.cols
        )));
        assert!(cmds.iter().any(|cmd| matches!(cmd, Cmd::Redraw)));
    }

    #[test]
    fn terminal_spawn_is_not_repeated_while_previous_spawn_is_pending() {
        let mut model = test_model();

        let first = update_dock(&mut model, DockMsg::TogglePanel(PanelId::TERMINAL));
        assert!(contains_spawn_terminal(&first));

        let second = update_dock(&mut model, DockMsg::FocusDock(DockPosition::Bottom));
        assert!(!contains_spawn_terminal(&second));
    }

    #[test]
    fn dragging_right_resize_handle_left_grows_right_dock() {
        let mut model = test_model();
        model.dock_layout.right.activate(PanelId::OUTLINE);
        let original_size = model.dock_layout.right.size_logical;

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Right,
                initial_coord: 600.0,
            },
        );
        update_dock(&mut model, DockMsg::UpdateResize { coord: 550.0 });

        assert!(model.dock_layout.right.size_logical > original_size);
    }

    #[test]
    fn right_dock_resize_converts_physical_delta_to_logical_size() {
        let mut model = hidpi_test_model();
        model.dock_layout.right.activate(PanelId::OUTLINE);
        let original_size = model.dock_layout.right.size_logical;

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Right,
                initial_coord: 1200.0,
            },
        );
        update_dock(&mut model, DockMsg::UpdateResize { coord: 1100.0 });

        assert_eq!(model.dock_layout.right.size_logical, original_size + 50.0);
    }

    #[test]
    fn right_dock_resize_max_uses_logical_window_size_on_hidpi() {
        let mut model = hidpi_test_model();
        model.dock_layout.right.activate(PanelId::OUTLINE);

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Right,
                initial_coord: 1200.0,
            },
        );
        update_dock(&mut model, DockMsg::UpdateResize { coord: 0.0 });

        assert_eq!(model.dock_layout.right.size_logical, 400.0);
    }

    #[test]
    fn dragging_bottom_resize_handle_up_grows_bottom_dock() {
        let mut model = test_model();
        model.dock_layout.bottom.activate(PanelId::TERMINAL);
        let original_size = model.dock_layout.bottom.size_logical;

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Bottom,
                initial_coord: 500.0,
            },
        );
        update_dock(&mut model, DockMsg::UpdateResize { coord: 450.0 });

        assert!(model.dock_layout.bottom.size_logical > original_size);
    }

    #[test]
    fn bottom_dock_resize_converts_physical_delta_to_logical_size() {
        let mut model = hidpi_test_model();
        model.dock_layout.bottom.activate(PanelId::TERMINAL);
        let original_size = model.dock_layout.bottom.size_logical;

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Bottom,
                initial_coord: 1000.0,
            },
        );
        update_dock(&mut model, DockMsg::UpdateResize { coord: 900.0 });

        assert_eq!(model.dock_layout.bottom.size_logical, original_size + 50.0);
    }

    #[test]
    fn bottom_dock_resize_max_uses_logical_window_size_on_hidpi() {
        let mut model = hidpi_test_model();
        model.dock_layout.bottom.activate(PanelId::TERMINAL);

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Bottom,
                initial_coord: 1000.0,
            },
        );
        update_dock(&mut model, DockMsg::UpdateResize { coord: 0.0 });

        assert_eq!(model.dock_layout.bottom.size_logical, 300.0);
    }

    #[test]
    fn ending_left_dock_resize_clears_both_resize_states() {
        let mut model = test_model();

        update_dock(
            &mut model,
            DockMsg::StartResize {
                position: DockPosition::Left,
                initial_coord: 250.0,
            },
        );
        update_dock(&mut model, DockMsg::EndResize);

        assert!(model.ui.sidebar_resize.is_none());
        assert!(model.ui.dock_resize.is_none());
    }
}
