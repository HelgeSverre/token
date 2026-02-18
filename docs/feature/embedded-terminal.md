# Embedded Terminal Panel

Integrated terminal panel at the bottom of the editor

> **Status:** Planned
> **Priority:** P2
> **Effort:** XXL
> **Created:** 2025-12-20
> **Milestone:** 5 - Future

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor currently:
- Has no terminal integration
- Requires switching to external terminal for command execution
- No shell output capture or display

### Goals

1. **Bottom panel terminal**: Collapsible panel docked at bottom
2. **PTY integration**: Real pseudo-terminal with full escape sequence support
3. **Multiple terminal tabs**: Support for multiple terminal instances
4. **Resizable panel**: Drag handle to adjust terminal height
5. **Shell integration**: Detect user's default shell
6. **Output buffer**: Scrollable history with configurable limit
7. **Copy/paste support**: Full clipboard integration

### Non-Goals

- Split terminal views (horizontal splits within the panel)
- Remote terminal/SSH integration (first iteration)
- Terminal multiplexer features (tmux-like)
- Custom shell prompt injection

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Terminal Panel Architecture                        │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                         Editor Area (existing)                          │ │
│  │  ┌────────────────────┐  ┌────────────────────────────────────────┐   │ │
│  │  │     Gutter         │  │              Text Area                   │   │ │
│  │  │                    │  │                                          │   │ │
│  │  └────────────────────┘  └────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ ═══════════════════════ Drag Handle (3px) ═══════════════════════════ │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  Terminal Panel                                                        │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐ │ │
│  │  │ Tab Bar: [zsh] [npm run dev] [+]                          [x]    │ │ │
│  │  └──────────────────────────────────────────────────────────────────┘ │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐ │ │
│  │  │ user@machine:~/project$ cargo build                              │ │ │
│  │  │    Compiling token v0.1.0                                        │ │ │
│  │  │    Finished dev [unoptimized + debuginfo] target(s)              │ │ │
│  │  │ user@machine:~/project$ █                                        │ │ │
│  │  │                                                                   │ │ │
│  │  └──────────────────────────────────────────────────────────────────┘ │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Keyboard   │────▶│  Terminal   │────▶│     PTY     │────▶│    Shell    │
│   Input     │     │   Panel     │     │   Master    │     │   Process   │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                           ▲                   │
                           │                   ▼
                    ┌──────┴──────┐     ┌─────────────┐
                    │   Render    │◀────│   VT100     │
                    │   Buffer    │     │   Parser    │
                    └─────────────┘     └─────────────┘
```

### Module Structure

```
src/
├── terminal/
│   ├── mod.rs            # Terminal module exports
│   ├── pty.rs            # PTY spawning and management
│   ├── parser.rs         # VT100/ANSI escape sequence parser
│   ├── buffer.rs         # Terminal output buffer (ring buffer)
│   ├── grid.rs           # Character grid with attributes
│   └── renderer.rs       # Terminal content rendering
├── model/
│   └── app.rs            # Add TerminalPanel to AppModel
├── view/
│   └── terminal.rs       # Terminal panel rendering
└── update/
    └── terminal.rs       # Terminal message handling
```

---

## Data Structures

### TerminalPanel

```rust
// In src/terminal/mod.rs

/// The terminal panel containing multiple terminal tabs
#[derive(Debug)]
pub struct TerminalPanel {
    /// List of terminal instances
    pub terminals: Vec<Terminal>,
    
    /// Currently active terminal index
    pub active_index: usize,
    
    /// Whether the panel is visible
    pub visible: bool,
    
    /// Panel height in pixels
    pub height: u32,
    
    /// Minimum panel height
    pub min_height: u32,
    
    /// Maximum panel height (percentage of window)
    pub max_height_percent: f32,
    
    /// Whether panel is being resized
    pub resizing: bool,
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self {
            terminals: Vec::new(),
            active_index: 0,
            visible: false,
            height: 200,
            min_height: 100,
            max_height_percent: 0.6,
            resizing: false,
        }
    }
}
```

### Terminal Instance

```rust
// In src/terminal/mod.rs

/// A single terminal instance
#[derive(Debug)]
pub struct Terminal {
    /// Unique identifier
    pub id: usize,
    
    /// Display title (shell name or running command)
    pub title: String,
    
    /// PTY handle
    pub pty: PtyHandle,
    
    /// Character grid
    pub grid: TerminalGrid,
    
    /// Cursor position (row, col)
    pub cursor: (usize, usize),
    
    /// Scroll offset (lines scrolled up from bottom)
    pub scroll_offset: usize,
    
    /// Scrollback buffer
    pub scrollback: Vec<TerminalRow>,
    
    /// Max scrollback lines
    pub scrollback_limit: usize,
    
    /// Current working directory
    pub cwd: PathBuf,
    
    /// Whether terminal has exited
    pub exited: bool,
    
    /// Exit code if exited
    pub exit_code: Option<i32>,
}

/// Character grid for visible terminal area
#[derive(Debug, Clone)]
pub struct TerminalGrid {
    /// Grid dimensions (rows, cols)
    pub size: (usize, usize),
    
    /// Character cells
    pub cells: Vec<Vec<Cell>>,
}

/// A single cell in the terminal grid
#[derive(Debug, Clone, Default)]
pub struct Cell {
    /// Character (space if empty)
    pub char: char,
    
    /// Foreground color
    pub fg: Color,
    
    /// Background color
    pub bg: Color,
    
    /// Text attributes
    pub attrs: CellAttributes,
}

#[derive(Debug, Clone, Default)]
pub struct CellAttributes {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub dim: bool,
}
```

### PTY Handle

```rust
// In src/terminal/pty.rs

use std::process::Child;
use std::os::fd::OwnedFd;

/// PTY master handle for communication with shell
pub struct PtyHandle {
    /// Master file descriptor
    master_fd: OwnedFd,
    
    /// Child shell process
    child: Child,
    
    /// Read buffer
    read_buffer: Vec<u8>,
}

impl PtyHandle {
    /// Spawn a new PTY with the user's default shell
    pub fn spawn_shell(cwd: &Path, size: (u16, u16)) -> io::Result<Self> {
        // Use nix or portable-pty crate
        todo!()
    }
    
    /// Write bytes to the terminal (user input)
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        todo!()
    }
    
    /// Read available output from the terminal
    pub fn read(&mut self) -> io::Result<Vec<u8>> {
        todo!()
    }
    
    /// Resize the PTY
    pub fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        todo!()
    }
    
    /// Check if process has exited
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}
```

### VT100 Parser

```rust
// In src/terminal/parser.rs

/// VT100/ANSI escape sequence parser
pub struct VtParser {
    /// Current parser state
    state: ParserState,
    
    /// Accumulated parameter bytes
    params: Vec<u8>,
    
    /// Intermediate bytes
    intermediates: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    OscString,
}

/// Parsed terminal action
#[derive(Debug, Clone)]
pub enum TerminalAction {
    /// Print a character
    Print(char),
    
    /// Execute control character (C0/C1)
    Execute(u8),
    
    /// CSI sequence (cursor movement, colors, etc.)
    CsiDispatch { params: Vec<u16>, intermediates: Vec<u8>, final_byte: u8 },
    
    /// ESC sequence
    EscDispatch { intermediates: Vec<u8>, final_byte: u8 },
    
    /// OSC sequence (window title, etc.)
    OscDispatch(Vec<Vec<u8>>),
}

impl VtParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            intermediates: Vec::new(),
        }
    }
    
    /// Parse bytes and return actions
    pub fn parse(&mut self, data: &[u8]) -> Vec<TerminalAction> {
        // Implement VT100 state machine
        todo!()
    }
}
```

### Messages

```rust
// In src/messages.rs

pub enum TerminalMsg {
    /// Toggle terminal panel visibility
    Toggle,
    
    /// Focus the terminal panel
    Focus,
    
    /// Create a new terminal tab
    NewTerminal,
    
    /// Close the active terminal
    CloseTerminal,
    
    /// Switch to terminal tab by index
    SwitchTab(usize),
    
    /// Next terminal tab
    NextTab,
    
    /// Previous terminal tab
    PrevTab,
    
    /// Process keyboard input
    KeyInput(KeyEvent),
    
    /// Paste from clipboard
    Paste(String),
    
    /// Process PTY output
    PtyOutput { terminal_id: usize, data: Vec<u8> },
    
    /// Terminal process exited
    ProcessExited { terminal_id: usize, code: i32 },
    
    /// Resize panel (delta pixels)
    ResizePanel(i32),
    
    /// Start panel resize drag
    StartResize,
    
    /// End panel resize drag
    EndResize,
    
    /// Scroll up
    ScrollUp(usize),
    
    /// Scroll down
    ScrollDown(usize),
    
    /// Scroll to bottom
    ScrollToBottom,
    
    /// Clear terminal
    Clear,
    
    /// Run command in new terminal
    RunCommand(String),
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Message |
|--------|-----|---------------|---------|
| Toggle terminal | Ctrl+` | Ctrl+` | `TerminalMsg::Toggle` |
| New terminal | Ctrl+Shift+` | Ctrl+Shift+` | `TerminalMsg::NewTerminal` |
| Close terminal | Ctrl+W (in terminal) | Ctrl+W (in terminal) | `TerminalMsg::CloseTerminal` |
| Next terminal | Ctrl+Tab (in terminal) | Ctrl+Tab (in terminal) | `TerminalMsg::NextTab` |
| Previous terminal | Ctrl+Shift+Tab | Ctrl+Shift+Tab | `TerminalMsg::PrevTab` |
| Clear terminal | Cmd+K | Ctrl+L | `TerminalMsg::Clear` |
| Scroll up | Shift+PageUp | Shift+PageUp | `TerminalMsg::ScrollUp` |
| Scroll down | Shift+PageDown | Shift+PageDown | `TerminalMsg::ScrollDown` |
| Focus terminal | Ctrl+` | Ctrl+` | `TerminalMsg::Focus` |
| Return to editor | Escape | Escape | `UiMsg::FocusEditor` |

---

## Implementation Plan

### Phase 1: PTY Foundation

**Estimated effort: 4-5 days**

1. [ ] Add `portable-pty` or `nix` crate as dependency
2. [ ] Implement `PtyHandle` for spawning shells
3. [ ] Detect user's default shell (`$SHELL` or `/bin/sh`)
4. [ ] Set up async read loop for PTY output
5. [ ] Handle PTY resize signals (`SIGWINCH`)

**Dependencies:** None

### Phase 2: VT100 Parser

**Estimated effort: 5-6 days**

1. [ ] Implement VT100 state machine
2. [ ] Parse cursor movement sequences (CUU, CUD, CUF, CUB)
3. [ ] Parse erase sequences (ED, EL)
4. [ ] Parse SGR (Select Graphic Rendition) for colors
5. [ ] Parse scroll region sequences
6. [ ] Handle OSC sequences (window title)

**Dependencies:** Phase 1

### Phase 3: Terminal Grid & Buffer

**Estimated effort: 3-4 days**

1. [ ] Implement `TerminalGrid` with cell storage
2. [ ] Implement cursor movement within grid
3. [ ] Implement line wrapping
4. [ ] Implement scrollback buffer (ring buffer)
5. [ ] Handle terminal resize (reflow content)

**Dependencies:** Phase 2

### Phase 4: Panel UI

**Estimated effort: 3-4 days**

1. [ ] Add `TerminalPanel` to `AppModel`
2. [ ] Implement panel visibility toggle
3. [ ] Render drag handle between editor and terminal
4. [ ] Implement panel resizing
5. [ ] Layout recalculation when panel opens/closes

**Dependencies:** Phase 3

### Phase 5: Terminal Rendering

**Estimated effort: 4-5 days**

1. [ ] Render terminal grid characters
2. [ ] Render cursor (block, underline, bar)
3. [ ] Render text attributes (bold, italic, underline)
4. [ ] Render 16-color palette
5. [ ] Render 256-color palette
6. [ ] Render true color (24-bit)

**Dependencies:** Phase 4

### Phase 6: Input Handling

**Estimated effort: 3-4 days**

1. [ ] Route keyboard input to terminal when focused
2. [ ] Convert key events to escape sequences
3. [ ] Handle special keys (arrows, function keys)
4. [ ] Implement paste (Cmd+V)
5. [ ] Handle Ctrl+C, Ctrl+D, Ctrl+Z

**Dependencies:** Phase 5

### Phase 7: Tab Management

**Estimated effort: 2-3 days**

1. [ ] Implement tab bar rendering
2. [ ] Implement tab switching
3. [ ] Implement new terminal creation
4. [ ] Implement terminal close
5. [ ] Show running command in tab title

**Dependencies:** Phase 6

### Phase 8: Polish & Integration

**Estimated effort: 3-4 days**

1. [ ] Scroll support (mouse wheel, keyboard)
2. [ ] Text selection in terminal (future)
3. [ ] Link detection and clickable URLs (future)
4. [ ] Shell integration (directory tracking)
5. [ ] Performance optimization for large outputs

**Dependencies:** Phase 7

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_vt_parser_cursor_movement() {
    let mut parser = VtParser::new();
    let actions = parser.parse(b"\x1b[5A"); // Move up 5
    assert_eq!(actions.len(), 1);
    // Verify CsiDispatch with correct params
}

#[test]
fn test_grid_scroll() {
    let mut grid = TerminalGrid::new(24, 80);
    grid.scroll_up(5);
    // Verify first 5 rows are empty
}

#[test]
fn test_sgr_colors() {
    let mut parser = VtParser::new();
    let actions = parser.parse(b"\x1b[31mRed\x1b[0m");
    // Verify color change and reset
}
```

### Integration Tests

```rust
#[test]
fn test_terminal_spawn() {
    let pty = PtyHandle::spawn_shell(Path::new("/tmp"), (80, 24)).unwrap();
    // Verify process is running
}

#[test]
fn test_terminal_echo() {
    let mut pty = PtyHandle::spawn_shell(Path::new("/tmp"), (80, 24)).unwrap();
    pty.write(b"echo hello\n").unwrap();
    // Wait and verify output contains "hello"
}
```

### Manual Testing Checklist

- [ ] Toggle terminal with Ctrl+`
- [ ] Type commands and see output
- [ ] Cursor movement works correctly
- [ ] Colors render properly
- [ ] Scrollback works (Shift+PageUp/Down)
- [ ] Multiple terminals work
- [ ] Tab switching works
- [ ] Panel resize works
- [ ] Ctrl+C interrupts running process
- [ ] Terminal respects window resize
- [ ] Clear terminal works

---

## Platform Considerations

### macOS
- Use `/bin/zsh` as default shell
- PTY via `posix_openpt` / `nix` crate

### Linux
- Use `$SHELL` or `/bin/bash`
- PTY via `openpty` / `nix` crate

### Windows (Future)
- Use ConPTY for pseudo-terminal
- PowerShell or cmd.exe as default

---

## Dependencies

```toml
# Cargo.toml additions
portable-pty = "0.8"      # Cross-platform PTY (or use nix directly)
vte = "0.13"              # VT100 parser (or implement custom)
```

Alternative: Use `nix` crate directly for Unix PTY control.

---

## References

- VT100 escape sequences: https://vt100.net/docs/vt100-ug/chapter3.html
- ANSI escape codes: https://en.wikipedia.org/wiki/ANSI_escape_code
- `portable-pty`: https://docs.rs/portable-pty/latest/portable_pty/
- `vte` parser: https://docs.rs/vte/latest/vte/
- Alacritty terminal: https://github.com/alacritty/alacritty (reference)
- WezTerm: https://github.com/wezterm/wezterm (reference)
