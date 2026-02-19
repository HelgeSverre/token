# Embedded Terminal Panel

Integrated terminal panel at the bottom of the editor

> **Status:** Planned
> **Priority:** P2
> **Effort:** L (8–14 days)
> **Created:** 2025-12-20
> **Updated:** 2026-02-19
> **Milestone:** 5 - Future

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Messages](#messages)
5. [Keybindings](#keybindings)
6. [Implementation Plan](#implementation-plan)
7. [Testing Strategy](#testing-strategy)
8. [Platform Considerations](#platform-considerations)
9. [Dependencies](#dependencies)
10. [References](#references)

---

## Overview

### Current State

The editor has a **full dock panel system** already built:

- ✅ `DockLayout` with Left/Right/Bottom docks (`src/panel/dock.rs`)
- ✅ `PanelId::Terminal` registered in bottom dock, `Cmd+2` keybinding wired
- ✅ Dock resize handles with mouse drag
- ✅ `DockRects` in `geometry.rs` for layout calculation
- ✅ `FocusTarget::Dock(DockPosition)` for keyboard focus routing
- ✅ `HitTarget` variants for dock resize, tabs, and content
- ⏳ Currently renders `PlaceholderPanel` with "Terminal panel coming soon..."

### Goals

1. **Real terminal emulation**: Full VT100/ANSI support via `alacritty_terminal` crate
2. **PTY integration**: Spawn user's default shell with proper escape sequences
3. **Rendering**: Grid of cells with colors, attributes, cursor — using existing fontdue renderer
4. **Input routing**: When dock is focused, keyboard input goes to PTY
5. **Scrollback**: Mouse wheel scrolls terminal history
6. **Copy/paste**: Clipboard integration

### Non-Goals (MVP)

- Multiple terminal tabs within the panel (post-MVP)
- Split terminal views
- Remote terminal/SSH integration
- Terminal multiplexer features (tmux-like)
- Custom shell prompt injection
- Text selection in terminal (post-MVP)
- Link detection / clickable URLs (post-MVP)
- Windows ConPTY support (follow-on)

---

## Architecture

### Integration with Existing Dock System

The terminal plugs into the existing dock infrastructure. **No new panel UI code needed.**

```
┌─────────────────────────────────────────────────┐
│  Existing Infrastructure (already built)         │
│  ┌────────────┐  ┌──────────┐  ┌─────────────┐ │
│  │ DockLayout │  │ DockRects│  │  HitTarget  │ │
│  │ + Dock     │  │ geometry │  │  hit_test   │ │
│  └────────────┘  └──────────┘  └─────────────┘ │
│  ┌────────────┐  ┌──────────┐  ┌─────────────┐ │
│  │ FocusTarget│  │ DockMsg  │  │ Resize drag │ │
│  │ + routing  │  │ handling │  │  + borders  │ │
│  └────────────┘  └──────────┘  └─────────────┘ │
└─────────────────────────────────────────────────┘
                      │
          ┌───────────▼───────────┐
          │  New: Terminal Engine  │
          │  ┌─────────────────┐  │
          │  │ TerminalSession │  │
          │  │ (alacritty_term)│  │
          │  └────────┬────────┘  │
          │           │           │
          │  ┌────────▼────────┐  │
          │  │  PTY Worker     │  │
          │  │  Thread (mpsc)  │  │
          │  └─────────────────┘  │
          └───────────────────────┘
```

### Event Flow

Matches Token's existing Elm architecture (`Message → Update → Command → Render`):

```
Keyboard Input (when dock focused)
    │
    ▼
translate_keys.rs: KeyEvent → bytes
    │
    ▼
pty_tx.send(bytes) → PTY Worker Thread → Shell Process
                                              │
                              PTY read loop ◄─┘
                                   │
                                   ▼
                     Msg::Terminal(PtyOutput { bytes })
                                   │
                                   ▼
                     update_terminal(): feed bytes into
                     alacritty_terminal Term → Cmd::Redraw
                                   │
                                   ▼
                     render_dock(): iterate term.grid cells
                     → fontdue glyph rendering with colors
```

### Module Structure

```
src/
├── terminal/
│   ├── mod.rs              # TerminalState, TerminalSession exports
│   ├── session.rs          # Wrapper around alacritty_terminal Term
│   ├── pty.rs              # portable-pty spawn + read/write threads
│   └── translate_keys.rs   # winit KeyEvent → terminal escape sequences
├── panels/
│   ├── terminal.rs         # Terminal panel rendering (replaces placeholder)
│   └── ...
├── update/
│   └── terminal.rs         # TerminalMsg handler
└── messages.rs             # TerminalMsg variants
```

---

## Data Structures

### TerminalState (in AppModel)

Terminal-specific state only. Visibility, height, and resizing are **owned by `Dock`**.

```rust
// In src/terminal/mod.rs

/// Terminal state — lives in AppModel alongside dock_layout
pub struct TerminalState {
    /// Active terminal sessions
    pub sessions: Vec<TerminalSession>,

    /// Index of the active session
    pub active: usize,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            active: 0,
        }
    }
}
```

### TerminalSession

```rust
// In src/terminal/session.rs

/// A single terminal session wrapping alacritty_terminal
pub struct TerminalSession {
    /// Display title (from shell or OSC title sequence)
    pub title: String,

    /// Channel to send bytes to the PTY write thread
    pub pty_tx: mpsc::Sender<Vec<u8>>,

    /// Terminal emulator core (parser + grid + scrollback)
    pub term: alacritty_terminal::Term<EventListener>,

    /// Scrollback view offset (lines scrolled up from bottom)
    pub scroll_offset: usize,

    /// Whether the shell process has exited
    pub exited: bool,

    /// Exit code if exited
    pub exit_code: Option<i32>,

    /// Last known grid dimensions (rows, cols)
    pub size: (usize, usize),
}
```

### PTY Worker

```rust
// In src/terminal/pty.rs

/// Spawn a PTY and return channels for communication
pub fn spawn_pty(
    cwd: &Path,
    rows: u16,
    cols: u16,
    msg_tx: mpsc::Sender<Msg>,
    session_id: usize,
) -> io::Result<mpsc::Sender<Vec<u8>>> {
    // 1. Detect shell: $SHELL or /bin/zsh (macOS) or /bin/sh
    // 2. Spawn via portable-pty
    // 3. Start read thread: blocking read → coalesce → Msg::Terminal(PtyOutput)
    // 4. Start write thread: recv from channel → write to PTY
    // 5. Return the write channel sender
}
```

---

## Messages

```rust
// In src/messages.rs — add to existing Msg enum

pub enum TerminalMsg {
    /// Spawn a new terminal session
    NewSession,

    /// Close the active terminal session
    CloseSession,

    /// Raw bytes received from PTY (sent by worker thread)
    PtyOutput { session_id: usize, data: Vec<u8> },

    /// PTY process exited
    ProcessExited { session_id: usize, code: i32 },

    /// Keyboard input to send to PTY
    KeyInput(KeyEvent),

    /// Paste text into terminal
    Paste(String),

    /// Scroll terminal scrollback
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollToBottom,

    /// Clear terminal buffer
    Clear,

    /// Resize terminal grid (triggered by dock resize)
    Resize { rows: u16, cols: u16 },
}
```

**Note**: Toggle/focus/panel switching is handled by existing `DockMsg` — not duplicated here.

---

## Keybindings

| Action | Mac | Windows/Linux | Handler |
|--------|-----|---------------|---------|
| Toggle terminal | Cmd+2 | Ctrl+2 | `DockMsg::FocusOrTogglePanel(PanelId::TERMINAL)` (existing) |
| New terminal | Ctrl+Shift+\` | Ctrl+Shift+\` | `TerminalMsg::NewSession` |
| Close terminal | — | — | `TerminalMsg::CloseSession` |
| Clear terminal | Cmd+K (in terminal) | Ctrl+L | `TerminalMsg::Clear` |
| Scroll up | Shift+PageUp | Shift+PageUp | `TerminalMsg::ScrollUp` |
| Scroll down | Shift+PageDown | Shift+PageDown | `TerminalMsg::ScrollDown` |
| Return to editor | Escape | Escape | `UiMsg::FocusEditor` (existing) |

All other keys (when terminal is focused) are sent directly to the PTY as escape sequences.

---

## Implementation Plan

### Phase 1: PTY Foundation + Terminal Core (3–4 days)

**Goal**: Spawn a shell, send/receive bytes, parse into a grid.

1. [ ] Add `portable-pty` and `alacritty_terminal` to `Cargo.toml`
2. [ ] Create `src/terminal/mod.rs` with `TerminalState` and `TerminalSession`
3. [ ] Implement `src/terminal/pty.rs`:
   - Detect default shell (`$SHELL` → `/bin/zsh` → `/bin/sh`)
   - Spawn PTY via `portable-pty`
   - Read thread: blocking read → coalesce into chunks → `Msg::Terminal(PtyOutput)`
   - Write channel for keyboard input
   - Process exit detection → `Msg::Terminal(ProcessExited)`
4. [ ] Implement `src/terminal/session.rs`:
   - Wrapper around `alacritty_terminal::Term`
   - `apply_bytes(&mut self, data: &[u8])` — feed to parser
   - `resize(&mut self, rows: u16, cols: u16)` — update grid + PTY
5. [ ] Add `TerminalMsg` to `src/messages.rs` and `Msg::Terminal` variant
6. [ ] Add `TerminalState` to `AppModel`
7. [ ] Implement `src/update/terminal.rs`:
   - Handle `PtyOutput` → feed bytes to session → `Cmd::Redraw`
   - Handle `ProcessExited` → mark session exited
   - Handle `NewSession` → spawn PTY + create session

**Verification**: `make build` compiles. Unit test spawns a PTY, sends `echo hello\n`, receives output.

### Phase 2: Terminal Rendering (3–4 days)

**Goal**: See actual terminal output in the dock panel.

1. [ ] Create `src/panels/terminal.rs` — terminal panel renderer
2. [ ] Replace `PlaceholderPanel` rendering for `PanelId::Terminal` in `src/view/mod.rs`
3. [ ] Render terminal grid:
   - Iterate `alacritty_terminal` grid cells
   - Map cell colors → theme colors (16-color palette + default fg/bg)
   - Render each character via existing `TextPainter`/fontdue
   - Render block cursor at terminal cursor position
4. [ ] Compute rows/cols from dock rect + `char_width`/`line_height`
5. [ ] Auto-spawn terminal session when panel first opens
6. [ ] Handle dock resize → `TerminalMsg::Resize` → PTY resize

**Verification**: Open terminal (Cmd+2), see shell prompt rendered in dock panel.

### Phase 3: Input Routing (2–3 days)

**Goal**: Type in the terminal and interact with the shell.

1. [ ] Implement `src/terminal/translate_keys.rs`:
   - Map winit `KeyEvent` → terminal escape sequences
   - Regular characters → UTF-8 bytes
   - Arrow keys → `\x1b[A/B/C/D`
   - Enter → `\r`, Backspace → `\x7f`
   - Ctrl+C → `\x03`, Ctrl+D → `\x04`, Ctrl+Z → `\x1a`
   - Function keys, Home/End/PgUp/PgDn
2. [ ] Route keyboard input when `FocusTarget::Dock(Bottom)` + active panel is Terminal
3. [ ] Implement paste: `TerminalMsg::Paste(text)` → send text bytes to PTY
4. [ ] Handle Escape → return focus to editor (existing `UiMsg::FocusEditor`)

**Verification**: Can type `ls`, `cargo build`, `vim` (cursor-based apps work).

### Phase 4: Scrollback + Polish (2–3 days)

**Goal**: Scroll through terminal history, handle edge cases.

1. [ ] Mouse wheel over terminal dock → scroll terminal scrollback
2. [ ] Auto-scroll to bottom on new output (unless user scrolled up)
3. [ ] Render scrollback indicator (e.g. line count or scrollbar)
4. [ ] Handle `TerminalMsg::Clear` — reset terminal buffer
5. [ ] SGR color support: 256-color + true color (24-bit)
6. [ ] Text attributes: bold, italic, underline, dim, inverse
7. [ ] Handle terminal bell (flash or ignore)
8. [ ] OSC title updates → update `TerminalSession.title`

**Verification**: Run `cargo build` with colored output, scroll up through build log, scroll back down.

### Post-MVP: Multiple Terminal Tabs

**Not part of initial implementation.** When needed:

1. [ ] Add tab bar rendering inside terminal panel
2. [ ] Tab switching (Ctrl+Tab within terminal)
3. [ ] New terminal button / Ctrl+Shift+\`
4. [ ] Show running command in tab title
5. [ ] Close individual terminals

---

## Eventing Model

Terminal I/O integrates with Token's existing async pattern (same as syntax highlighting worker, fs watcher):

```
                    ┌─────────────────────┐
                    │   Main Thread       │
                    │                     │
                    │  msg_rx.try_recv()  │◄─── PTY read thread sends
                    │  → update_terminal  │     Msg::Terminal(PtyOutput)
                    │  → Cmd::Redraw      │
                    │                     │
                    │  pty_tx.send(bytes) ─┼──► PTY write thread
                    └─────────────────────┘
```

**Backpressure**: PTY read thread coalesces output into chunks (up to 32KB or flush every 16ms) to avoid flooding the message queue during large builds.

**Wakeup**: If `ControlFlow::WaitUntil` causes latency, add `EventLoopProxy::send_event()` to wake the loop immediately on PTY output. Start without this — only add if needed.

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_key_translation_arrows() {
    assert_eq!(translate_key(Key::ArrowUp, Modifiers::empty()), b"\x1b[A");
    assert_eq!(translate_key(Key::ArrowDown, Modifiers::empty()), b"\x1b[B");
}

#[test]
fn test_key_translation_ctrl() {
    assert_eq!(translate_key(Key::Character("c"), Modifiers::CONTROL), b"\x03");
    assert_eq!(translate_key(Key::Character("d"), Modifiers::CONTROL), b"\x04");
}

#[test]
fn test_terminal_session_resize() {
    let mut session = create_test_session(80, 24);
    session.resize(120, 40);
    assert_eq!(session.size, (40, 120));
}
```

### Integration Tests

```rust
#[test]
fn test_pty_spawn_and_echo() {
    let (msg_tx, msg_rx) = mpsc::channel();
    let pty_tx = spawn_pty(Path::new("/tmp"), 24, 80, msg_tx, 0).unwrap();
    pty_tx.send(b"echo hello\n".to_vec()).unwrap();
    // Wait for PtyOutput message containing "hello"
}
```

### Manual Testing Checklist

- [ ] Cmd+2 opens terminal with shell prompt
- [ ] Type commands, see output with colors
- [ ] Arrow keys work (shell history, cursor movement)
- [ ] Ctrl+C interrupts running process
- [ ] Ctrl+D sends EOF
- [ ] Scrollback works (Shift+PageUp/Down, mouse wheel)
- [ ] Resize dock → terminal reflows correctly
- [ ] Escape returns focus to editor
- [ ] Paste works (Cmd+V)
- [ ] `vim`/`nano` work (alternate screen apps)
- [ ] Long build output renders without lag

---

## Platform Considerations

### macOS (MVP target)
- Default shell: `/bin/zsh` (via `$SHELL`)
- PTY via `portable-pty` (wraps `posix_openpt`)

### Linux
- Default shell: `$SHELL` or `/bin/bash`
- PTY via `portable-pty` (wraps `openpty`)

### Windows (Follow-on)
- ConPTY via `portable-pty` (has Windows support)
- PowerShell or cmd.exe as default
- May need additional escape sequence handling

---

## Dependencies

```toml
# Cargo.toml additions
portable-pty = "0.8"              # Cross-platform PTY spawning
alacritty_terminal = "0.24"       # Terminal emulator core (parser + grid + scrollback)
```

Pin versions and wrap `alacritty_terminal` behind `TerminalSession` so the dependency can be swapped if needed.

**Why not `vte` + custom grid?** Building a correct VT100 state machine with cursor modes, scroll regions, wrapping, and scrollback is 2–3 weeks of work. `alacritty_terminal` provides all of this battle-tested. Token only needs to render the resulting cell grid.

---

## References

- `portable-pty`: https://docs.rs/portable-pty/latest/portable_pty/
- `alacritty_terminal`: https://docs.rs/alacritty_terminal/latest/alacritty_terminal/
- VT100 escape sequences: https://vt100.net/docs/vt100-ug/chapter3.html
- ANSI escape codes: https://en.wikipedia.org/wiki/ANSI_escape_code
- Alacritty source: https://github.com/alacritty/alacritty (reference implementation)
- WezTerm source: https://github.com/wezterm/wezterm (alternative reference)
