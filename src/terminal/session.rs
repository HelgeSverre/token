//! Wrapper around `alacritty_terminal::Term` — the VT100/ANSI parser, grid,
//! and scrollback core. Kept behind this module so the underlying crate can
//! be swapped later without touching callers (see
//! `docs/feature/embedded-terminal.md`, "Dependencies").

use std::sync::mpsc::Sender;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;

use crate::messages::{Msg, TerminalMsg};

use super::pty::PtyHandle;

/// Terminal grid dimensions, in the shape `alacritty_terminal::Term` needs.
///
/// `alacritty_terminal` doesn't expose a ready-made non-test `Dimensions`
/// implementor (only a `term::test::TermSize` helper), so this is a small,
/// direct implementation for production use.
#[derive(Debug, Clone, Copy)]
struct GridSize {
    rows: usize,
    cols: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// Forwards `alacritty_terminal` events back into the Elm update loop as
/// `Msg::Terminal` variants, matching the async-worker pattern already used
/// for syntax highlighting and file-system watching (see
/// `src/runtime/app.rs`'s `syntax_worker_loop`).
///
/// Must stay cheap and non-blocking: `send_event` is called synchronously
/// from within VT sequence parsing.
#[derive(Clone)]
pub struct TerminalEventProxy {
    session_id: usize,
    msg_tx: Sender<Msg>,
}

impl TerminalEventProxy {
    pub fn new(session_id: usize, msg_tx: Sender<Msg>) -> Self {
        Self { session_id, msg_tx }
    }
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        let msg = match event {
            // Some escape sequences (cursor position reports, device status
            // queries) require the terminal emulator to write a response
            // back to the PTY. Dropping these would make interactive
            // full-screen apps (vim, htop) hang or misbehave.
            Event::PtyWrite(text) => TerminalMsg::WriteToPty {
                session_id: self.session_id,
                bytes: text.into_bytes(),
            },
            Event::Title(title) => TerminalMsg::TitleChanged {
                session_id: self.session_id,
                title,
            },
            Event::ResetTitle => TerminalMsg::TitleChanged {
                session_id: self.session_id,
                title: String::new(),
            },
            Event::Bell => TerminalMsg::Bell {
                session_id: self.session_id,
            },
            // Redraw-only signals -- coalesce into a single Redraw request
            // rather than threading every terminal event through Msg.
            Event::Wakeup | Event::MouseCursorDirty | Event::CursorBlinkingChange => {
                TerminalMsg::Redraw {
                    session_id: self.session_id,
                }
            }
            // Clipboard integration, color queries, and shutdown requests
            // are out of scope for the Phase 1 MVP (see "Non-Goals").
            Event::ClipboardStore(..)
            | Event::ClipboardLoad(..)
            | Event::ColorRequest(..)
            | Event::TextAreaSizeRequest(..)
            | Event::Exit
            | Event::ChildExit(_) => return,
        };
        // The receiving end (the main event loop) outlives every terminal
        // session, so a send error here only means the app is shutting
        // down -- safe to ignore.
        let _ = self.msg_tx.send(Msg::Terminal(msg));
    }
}

/// A single terminal session: PTY process + VT100/ANSI emulator core.
pub struct TerminalSession {
    /// Unique id for this session, stable for its lifetime. Used to route
    /// `Msg::Terminal` events (which may arrive from a background thread)
    /// back to the right session.
    pub id: usize,
    /// Display title (from shell or OSC title sequence).
    pub title: String,
    /// PTY process handle: write channel + resize + exit status.
    pub pty: PtyHandle,
    /// Terminal emulator core (parser + grid + scrollback).
    term: Term<TerminalEventProxy>,
    /// VT/ANSI byte-stream parser feeding into `term`.
    parser: Processor,
    /// Scrollback view offset (lines scrolled up from bottom).
    pub scroll_offset: usize,
    /// Whether the shell process has exited.
    pub exited: bool,
    /// Exit code if exited.
    pub exit_code: Option<i32>,
    /// Last known grid dimensions (rows, cols).
    pub size: (usize, usize),
}

impl TerminalSession {
    pub fn new(id: usize, rows: usize, cols: usize, pty: PtyHandle, msg_tx: Sender<Msg>) -> Self {
        let event_proxy = TerminalEventProxy::new(id, msg_tx);
        let size = GridSize { rows, cols };
        let term = Term::new(Config::default(), &size, event_proxy);

        Self {
            id,
            title: String::new(),
            pty,
            term,
            parser: Processor::new(),
            scroll_offset: 0,
            exited: false,
            exit_code: None,
            size: (rows, cols),
        }
    }

    /// Feed raw PTY output bytes into the VT parser, updating grid/cursor/
    /// scrollback state and (via `TerminalEventProxy`) emitting any
    /// resulting `Msg::Terminal` events (title changes, PTY write-backs,
    /// bell, redraw requests).
    pub fn apply_bytes(&mut self, data: &[u8]) {
        self.parser.advance(&mut self.term, data);
    }

    /// Resize the terminal grid and the underlying PTY to match.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        if self.size == (rows, cols) {
            return;
        }
        self.term.resize(GridSize { rows, cols });
        let _ = self.pty.resize(rows as u16, cols as u16);
        self.size = (rows, cols);
    }

    /// Read-only access to the terminal core, for rendering (grid cells,
    /// cursor position, colors) in a later phase.
    pub fn term(&self) -> &Term<TerminalEventProxy> {
        &self.term
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn test_proxy() -> (TerminalEventProxy, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        (TerminalEventProxy::new(0, tx), rx)
    }

    fn test_term(rows: usize, cols: usize) -> (Term<TerminalEventProxy>, mpsc::Receiver<Msg>) {
        let (proxy, rx) = test_proxy();
        let size = GridSize { rows, cols };
        (Term::new(Config::default(), &size, proxy), rx)
    }

    #[test]
    fn apply_bytes_writes_plain_text_into_the_grid() {
        let (mut term, _rx) = test_term(24, 80);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, b"hello");

        let grid = term.grid();
        let line: String = (0..5)
            .map(|col| {
                grid[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(col)].c
            })
            .collect();
        assert_eq!(line, "hello");
    }

    #[test]
    fn apply_bytes_handles_carriage_return_and_newline() {
        let (mut term, _rx) = test_term(24, 80);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, b"line1\r\nline2");

        let grid = term.grid();
        let first: String = (0..5)
            .map(|col| {
                grid[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(col)].c
            })
            .collect();
        let second: String = (0..5)
            .map(|col| {
                grid[alacritty_terminal::index::Line(1)][alacritty_terminal::index::Column(col)].c
            })
            .collect();
        assert_eq!(first, "line1");
        assert_eq!(second, "line2");
    }

    #[test]
    fn event_proxy_forwards_pty_write_event() {
        let (proxy, rx) = test_proxy();
        proxy.send_event(Event::PtyWrite("\x1b[0n".to_string()));

        match rx.try_recv().expect("expected a Msg::Terminal to be sent") {
            Msg::Terminal(TerminalMsg::WriteToPty { session_id, bytes }) => {
                assert_eq!(session_id, 0);
                assert_eq!(bytes, b"\x1b[0n");
            }
            other => panic!("expected WriteToPty, got {other:?}"),
        }
    }

    #[test]
    fn event_proxy_forwards_title_change() {
        let (proxy, rx) = test_proxy();
        proxy.send_event(Event::Title("my-shell".to_string()));

        match rx.try_recv().expect("expected a Msg::Terminal to be sent") {
            Msg::Terminal(TerminalMsg::TitleChanged { session_id, title }) => {
                assert_eq!(session_id, 0);
                assert_eq!(title, "my-shell");
            }
            other => panic!("expected TitleChanged, got {other:?}"),
        }
    }

    #[test]
    fn event_proxy_drops_out_of_scope_events_silently() {
        let (proxy, rx) = test_proxy();
        proxy.send_event(Event::Exit);
        assert!(rx.try_recv().is_err());
    }
}
