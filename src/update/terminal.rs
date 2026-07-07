//! Terminal panel update handlers.
//!
//! PTY process management lives in the runtime layer (spawned via
//! `Cmd::SpawnTerminal`, see `src/runtime/app.rs`); this module only
//! updates `AppModel::terminal` in response to `TerminalMsg`.

use crate::commands::Cmd;
use crate::messages::TerminalMsg;
use crate::model::AppModel;

/// Default terminal grid size for a newly spawned session, before the dock
/// panel has a resolved content rect to derive real rows/cols from (wired
/// in a later phase -- see docs/feature/embedded-terminal.md, Phase 2).
const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 80;

/// Monotonically increasing id for newly spawned terminal sessions.
fn next_session_id(model: &AppModel) -> usize {
    model
        .terminal
        .sessions
        .iter()
        .map(|s| s.id)
        .max()
        .map(|max| max + 1)
        .unwrap_or(0)
}

pub fn update_terminal(model: &mut AppModel, msg: TerminalMsg) -> Option<Cmd> {
    match msg {
        TerminalMsg::NewSession => {
            if model.terminal.has_pending_spawn() {
                return None;
            }

            let session_id = next_session_id(model);
            model.terminal.mark_spawn_pending(session_id);
            Some(Cmd::SpawnTerminal {
                session_id,
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
            })
        }

        TerminalMsg::CloseSession => {
            if model.terminal.sessions.is_empty() {
                return None;
            }
            let idx = model.terminal.active.min(model.terminal.sessions.len() - 1);
            let mut removed = model.terminal.sessions.remove(idx);
            removed.pty.kill();
            model.terminal.active = model
                .terminal
                .active
                .min(model.terminal.sessions.len().saturating_sub(1));
            Some(Cmd::redraw_editor())
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
            Some(Cmd::redraw_status_bar())
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
        AppModel::new(800, 600, 1.0, vec![])
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
    fn new_session_returns_spawn_terminal_command_with_incrementing_ids() {
        let mut model = test_model();

        let cmd = update_terminal(&mut model, TerminalMsg::NewSession);
        assert!(matches!(
            cmd,
            Some(Cmd::SpawnTerminal { session_id: 0, .. })
        ));

        // No session was actually pushed yet (that happens once the
        // runtime layer executes Cmd::SpawnTerminal and the PTY spawns
        // successfully), but the pending spawn marker prevents issuing a
        // second overlapping spawn for the same MVP terminal slot.
        assert!(update_terminal(&mut model, TerminalMsg::NewSession).is_none());
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
