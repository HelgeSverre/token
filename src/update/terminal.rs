//! Terminal panel update handlers.
//!
//! PTY process management lives in the runtime layer (spawned via
//! `Cmd::SpawnTerminal`, see `src/runtime/app.rs`); this module only
//! updates `AppModel::terminal` in response to `TerminalMsg`.

use crate::commands::Cmd;
use crate::messages::TerminalMsg;
use crate::model::AppModel;
use crate::terminal::TabAction;

/// Fallback only when the terminal is not registered in a visible dock.
const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 80;

pub(super) fn update_terminal(model: &mut AppModel, msg: TerminalMsg) -> Option<Cmd> {
    match msg {
        TerminalMsg::Tab(TabAction::New) => {
            let session_id = model.terminal.begin_spawn()?;
            if let Some(position) = model
                .dock_layout
                .find_panel(crate::panel::PanelId::Terminal)
            {
                model
                    .dock_layout
                    .dock_mut(position)
                    .activate(crate::panel::PanelId::Terminal);
                model.ui.focus_dock(position);
            }
            model.recalculate_viewports();
            let size = super::dock::terminal_grid_size_for_model(model).unwrap_or(
                crate::panels::terminal::TerminalGridSize {
                    rows: DEFAULT_ROWS,
                    cols: DEFAULT_COLS,
                },
            );
            Some(Cmd::SpawnTerminal {
                session_id,
                rows: size.rows,
                cols: size.cols,
            })
        }

        TerminalMsg::Tab(TabAction::Close) => Some(Cmd::CloseTerminal {
            session_id: model.terminal.active_session()?.id,
        }),

        TerminalMsg::Tab(action) => {
            let count = model.terminal.sessions.len();
            if count == 0 {
                return None;
            }
            model.terminal.active = match action {
                TabAction::Select(id) => model.terminal.sessions.iter().position(|s| s.id == id)?,
                TabAction::Next => (model.terminal.active + 1) % count,
                TabAction::Previous => (model.terminal.active + count - 1) % count,
                TabAction::New | TabAction::Close => unreachable!(),
            };
            if let Some(position) = model
                .dock_layout
                .active_panel_position(crate::panel::PanelId::Terminal)
            {
                model.ui.focus_dock(position);
            }
            Some(super::dock::with_terminal_sync(model, Cmd::Redraw))
        }

        TerminalMsg::PtyOutput { session_id, data } => {
            if let Some(session) = model.terminal.session_mut(session_id) {
                let was_at_bottom = session.scroll_offset == 0;
                session.apply_bytes(&data);
                if was_at_bottom {
                    session.scroll_offset = 0;
                } else {
                    session.clamp_scroll_offset();
                }
            }
            Some(Cmd::Redraw)
        }

        TerminalMsg::ProcessExited { session_id, code } => {
            if let Some(session) = model.terminal.session_mut(session_id) {
                session.exited = true;
                session.exit_code = Some(code);
            }
            Some(Cmd::Redraw)
        }

        TerminalMsg::WriteToPty { session_id, bytes } => {
            if let Some(session) = model.terminal.session_mut(session_id) {
                session.pty.write(bytes);
            }
            None
        }

        TerminalMsg::Paste(text) => {
            if let Some(session) = model.terminal.active_session_mut() {
                session.pty.write(text.into_bytes());
            }
            None
        }

        TerminalMsg::TitleChanged { session_id, title } => {
            if let Some(session) = model.terminal.session_mut(session_id) {
                session.title = if title.is_empty() {
                    "Terminal".to_string()
                } else {
                    title
                };
            }
            Some(super::dock::with_terminal_sync(model, Cmd::Redraw))
        }

        TerminalMsg::Bell { session_id: _ } => {
            // MVP behavior: bells are intentionally ignored rather than
            // flashing the entire dock.
            None
        }

        TerminalMsg::Redraw { session_id: _ } => Some(Cmd::Redraw),

        TerminalMsg::ScrollUp(lines) => {
            if let Some(session) = model.terminal.active_session_mut() {
                session.scroll_offset = session.scroll_offset.saturating_add(lines);
                session.clamp_scroll_offset();
            }
            Some(Cmd::redraw_editor())
        }

        TerminalMsg::ScrollDown(lines) => {
            if let Some(session) = model.terminal.active_session_mut() {
                session.scroll_offset = session.scroll_offset.saturating_sub(lines);
                session.clamp_scroll_offset();
            }
            Some(Cmd::redraw_editor())
        }

        TerminalMsg::ScrollToBottom => {
            if let Some(session) = model.terminal.active_session_mut() {
                session.scroll_offset = 0;
            }
            Some(Cmd::redraw_editor())
        }

        TerminalMsg::Clear => {
            if let Some(session) = model.terminal.active_session_mut() {
                session.clear();
            }
            Some(Cmd::redraw_editor())
        }

        TerminalMsg::Resize { rows, cols } => {
            if let Some(session) = model.terminal.active_session_mut() {
                session.resize(rows as usize, cols as usize);
                session.clamp_scroll_offset();
            }
            Some(Cmd::redraw_editor())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    use crate::model::AppModel;
    use crate::terminal::{PtyHandle, TerminalSession};
    use alacritty_terminal::grid::Dimensions;
    use alacritty_terminal::index::{Column, Line};

    fn test_model() -> AppModel {
        AppModel::new(800, 600, 1.0)
    }

    fn push_test_session(model: &mut AppModel, rows: usize, cols: usize) {
        let (pty, _pty_rx) = PtyHandle::new_for_test();
        let (msg_tx, _msg_rx) = mpsc::channel();
        model
            .terminal
            .sessions
            .push(TerminalSession::new(7, rows, cols, pty, msg_tx));
    }

    fn active_history_size(model: &AppModel) -> usize {
        let session = model.terminal.active_session().unwrap();
        session
            .term()
            .grid()
            .total_lines()
            .saturating_sub(session.term().grid().screen_lines())
    }

    #[test]
    fn switching_terminal_tabs_keeps_history_and_reveals_the_active_tab() {
        let mut model = test_model();
        model
            .dock_layout
            .bottom
            .activate(crate::panel::PanelId::Terminal);
        let size = super::super::dock::terminal_grid_size_for_model(&model).unwrap();
        for id in 0..10 {
            push_test_session(&mut model, size.rows as usize, size.cols as usize);
            let session = model.terminal.sessions.last_mut().unwrap();
            session.id = id;
            session.apply_bytes("history\r\n".repeat(size.rows as usize + 8).as_bytes());
            session.scroll_offset = id;
        }
        update_terminal(&mut model, TerminalMsg::Tab(TabAction::Select(9)));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 9);
        assert!(model.terminal.tab_scroll > 0.0);
        let chrome = crate::layout::chrome::chrome(&model);
        let rect = chrome
            .rect(crate::layout::UiKey::TerminalAction(TabAction::Select(9)))
            .unwrap();
        assert!(matches!(
            crate::view::hit_test::hit_test_docks(
                &model,
                crate::view::hit_test::Point::new(
                    (rect.x + rect.width / 2.0) as f64,
                    (rect.y + rect.height / 2.0) as f64
                )
            ),
            Some(crate::view::hit_test::HitTarget::TerminalAction {
                action: TabAction::Select(9),
                ..
            })
        ));
        update_terminal(&mut model, TerminalMsg::Tab(TabAction::Next));
        assert_eq!(model.terminal.active, 0);
        assert_eq!(model.terminal.tab_scroll, 0.0);
        assert_eq!(model.terminal.sessions[9].scroll_offset, 9);
        update_terminal(&mut model, TerminalMsg::Tab(TabAction::Previous));
        assert_eq!(model.terminal.active, 9);
    }

    #[test]
    fn new_session_returns_spawn_terminal_command_with_incrementing_ids() {
        let mut model = test_model();

        let cmd = update_terminal(&mut model, TerminalMsg::Tab(TabAction::New));
        assert!(matches!(
            cmd,
            Some(Cmd::SpawnTerminal { session_id: 0, .. })
        ));

        // No session was actually pushed yet (that happens once the
        // runtime layer executes Cmd::SpawnTerminal and the PTY spawns
        // successfully), but the pending spawn marker prevents issuing a
        // second overlapping spawn.
        assert!(update_terminal(&mut model, TerminalMsg::Tab(TabAction::New)).is_none());
        model.terminal.clear_spawn_pending(0);
        assert!(matches!(
            update_terminal(&mut model, TerminalMsg::Tab(TabAction::New)),
            Some(Cmd::SpawnTerminal { session_id: 1, .. })
        ));
    }

    #[test]
    fn pty_output_is_dropped_silently_for_unknown_session() {
        let mut model = test_model();
        let cmd = update_terminal(
            &mut model,
            TerminalMsg::PtyOutput {
                session_id: 999,
                data: b"hello".to_vec(),
            },
        );
        assert!(matches!(cmd, Some(Cmd::Redraw)));
    }

    #[test]
    fn process_exited_is_a_noop_for_unknown_session() {
        let mut model = test_model();
        let cmd = update_terminal(
            &mut model,
            TerminalMsg::ProcessExited {
                session_id: 999,
                code: 1,
            },
        );
        assert!(matches!(cmd, Some(Cmd::Redraw)));
    }

    #[test]
    fn scroll_up_clamps_to_available_scrollback() {
        let mut model = test_model();
        push_test_session(&mut model, 2, 20);

        update_terminal(
            &mut model,
            TerminalMsg::PtyOutput {
                session_id: 7,
                data: b"one\r\ntwo\r\nthree\r\nfour\r\n".to_vec(),
            },
        );
        let history_size = active_history_size(&model);

        update_terminal(&mut model, TerminalMsg::ScrollUp(history_size + 100));

        assert_eq!(
            model.terminal.active_session().unwrap().scroll_offset,
            history_size
        );
    }

    #[test]
    fn pty_output_clamps_stale_scrollback_offset() {
        let mut model = test_model();
        push_test_session(&mut model, 2, 20);
        model.terminal.active_session_mut().unwrap().scroll_offset = 999;

        update_terminal(
            &mut model,
            TerminalMsg::PtyOutput {
                session_id: 7,
                data: b"one\r\ntwo\r\nthree\r\n".to_vec(),
            },
        );

        assert_eq!(
            model.terminal.active_session().unwrap().scroll_offset,
            active_history_size(&model)
        );
    }

    #[test]
    fn scroll_down_clamps_stale_scrollback_offset() {
        let mut model = test_model();
        push_test_session(&mut model, 2, 20);
        update_terminal(
            &mut model,
            TerminalMsg::PtyOutput {
                session_id: 7,
                data: b"one\r\ntwo\r\nthree\r\n".to_vec(),
            },
        );
        model.terminal.active_session_mut().unwrap().scroll_offset = 999;

        update_terminal(&mut model, TerminalMsg::ScrollDown(1));

        assert_eq!(
            model.terminal.active_session().unwrap().scroll_offset,
            active_history_size(&model)
        );
    }

    #[test]
    fn clear_resets_terminal_grid_and_scrollback_offset() {
        let mut model = test_model();
        push_test_session(&mut model, 2, 20);
        update_terminal(
            &mut model,
            TerminalMsg::PtyOutput {
                session_id: 7,
                data: b"one\r\ntwo\r\nthree\r\n".to_vec(),
            },
        );
        model.terminal.active_session_mut().unwrap().scroll_offset = 1;

        update_terminal(&mut model, TerminalMsg::Clear);

        let session = model.terminal.active_session().unwrap();
        let grid = session.term().grid();
        assert_eq!(session.scroll_offset, 0);
        assert_eq!(grid.total_lines(), grid.screen_lines());
        assert_eq!(grid[Line(0)][Column(0)].c, ' ');
    }
}
