# GUI Architecture Review & Recommendations (v2)

A comprehensive analysis of the current rendering architecture, comparison with Rust GUI frameworks, and recommendations for building UI abstractions.

**Version 2** adds: Command Palette system design, Modal Overlay abstraction, and focus management patterns.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [Rust GUI Framework Comparison](#rust-gui-framework-comparison)
4. [Recommended Approach](#recommended-approach)
5. [Proposed Abstractions](#proposed-abstractions)
6. [Command Palette & Modal Overlay System](#command-palette--modal-overlay-system) _(NEW)_
7. [Honest Critique](#honest-critique)
8. [Migration Path](#migration-path)
9. [When to Consider Advanced Paths](#when-to-consider-advanced-paths)

---

## Executive Summary

**Recommendation:** Keep your existing Elm-style core + winit/softbuffer/fontdue stack, and build a _thin, editor-focused view layer_ on top rather than adopting egui/iced.

**Why:**

- Editors have "weird" requirements (huge documents, partial painting, custom cursor/selection) that are awkward in general GUI frameworks
- You already have a solid Elm Architecture foundation
- Migration to egui/iced is not incremental—it would require rewriting view and input layers
- Your direct control over text rendering is an advantage for a code editor

**Effort Estimate:**

- Initial file split (as planned): 1-3 hours
- Basic Frame/Painter abstraction: 1-2 hours
- Widget extraction per region: 3-8 hours total (done incrementally)
- Command palette + modal system: 1-2 days
- Full cleanup: ongoing small increments

---

## Current Architecture Analysis

### What You Have

```
┌─────────────────────────────────────────────────────────────────┐
│                        Architecture                              │
├─────────────────────────────────────────────────────────────────┤
│  winit (event loop) → App (ApplicationHandler)                  │
│         ↓                                                        │
│  Events → handle_key/mouse → Msg                                │
│         ↓                                                        │
│  update(model, msg) → Option<Cmd>                               │
│         ↓                                                        │
│  Renderer::render_impl() → pixel buffer                         │
│         ↓                                                        │
│  softbuffer Surface → Window                                    │
└─────────────────────────────────────────────────────────────────┘
```

### File Size Analysis

| File          | Lines | Contents                                                                     |
| ------------- | ----- | ---------------------------------------------------------------------------- |
| `main.rs`     | ~3000 | Renderer, PerfStats, App, ApplicationHandler, handle_key, draw_text, main()  |
| `update.rs`   | ~2600 | update dispatcher, cursor helpers, layout helpers                            |
| `model/`      | ~2300 | Well-organized: document.rs, editor.rs, editor_area.rs, status_bar.rs, ui.rs |
| `theme.rs`    | ~540  | Theme loading and color types                                                |
| `overlay.rs`  | ~285  | Overlay rendering utilities                                                  |
| `messages.rs` | ~260  | Msg enums                                                                    |

### Strengths 

1. **Model layer is clean and well factored**
   - Clear separation: `Document`, `EditorState`, `EditorArea`
   - Proper encapsulation of concerns

2. **EditorArea layout tree is solid**
   - `LayoutNode` with recursive `SplitContainer`
   - `compute_layout()` produces `Rect`s for groups
   - Splitter bar hit-testing already works

3. **Overlay system is a good pattern**
   - `OverlayConfig` with anchoring
   - `blend_pixel()` for alpha compositing
   - Reusable rendering utilities

4. **Elm Architecture is a great fit for editors**
   - Message-driven updates
   - Predictable state flow
   - Easy to test

### Weaknesses

1. **`main.rs` is a "god file"** mixing:
   - Winit integration
   - Input handling
   - Rendering logic (low-level pixel loops)
   - Performance overlay
   - Tests

2. **Rendering is too low-level everywhere**
   - Manual x/y pixel index computation inline
   - Direct buffer indexing repeated in many places
   - Copy/paste patterns instead of reuse

3. **No explicit view/widget layer**
   - Tab bars, splitters, status bar, gutter, text area are conceptually distinct
   - But implemented as ad-hoc regions within monolithic render functions

4. **Layout logic scattered between model and render**
   - Groups/splitters have `compute_layout` ✓
   - Tab bars, gutter bounds, text region bounds computed inline in render ✗

5. **Input and rendering coupled by "knowledge"**
   - Both recreate same coordinate math for hit-testing and painting
   - Changes need updates in multiple spots

---

## Rust GUI Framework Comparison

### Framework Overview

| Feature                    | egui                 | iced         | slint        | GPUI (Zed)       | Custom  |
| -------------------------- | -------------------- | ------------ | ------------ | ---------------- | ------- |
| **Syntax Highlighting**    | Via plugin           | Built-in     | Not designed | Built-in         | Manual  |
| **Text Rendering Quality** | Good                 | Good         | Fair\*       | Excellent        | Custom  |
| **Large Document Perf**    | ⚠️ Full layout/frame | Good         | Fair         | Excellent        | Depends |
| **Accessibility**          | ✓ AccessKit          | ✗ Broken     | Unknown      | ✗ Poor           | Custom  |
| **Code Editor Examples**   | Few                  | Some         | None         | Production (Zed) | Many    |
| **Immediate Mode**         | ✓ Yes                | ✗ No (Elm)   | ✗ No (Decl)  | Hybrid           | -       |
| **winit Integration**      | eframe               | iced_winit   | Direct       | Custom           | Direct  |
| **Maturity**               | Stable               | Experimental | Stable       | Production       | -       |

\*slint femtovg backend has known text rendering issues

### Why Not egui/iced?

1. **Migration is not incremental**
   - They want to own the event loop and rendering
   - You'd effectively rewrite view and input layers

2. **Editors have weird requirements**
   - Huge documents with partial painting
   - Custom cursor/selection logic
   - Code-style gutters, inline diagnostics
   - Awkward in general GUI frameworks

3. **You'd lose your tuned text path**
   - fontdue gives you direct control
   - Framework text rendering may not match your needs

### What to Learn From Them

| Pattern                       | Framework    | Apply To Token                        |
| ----------------------------- | ------------ | ------------------------------------- |
| Separate layout from painting | All          | Yes - extend `compute_layout` pattern |
| Single "painter" context      | egui, Druid  | Yes - introduce `Frame` abstraction   |
| Widget-per-concern functions  | iced, Druid  | Yes - extract render functions        |
| Consistent coordinate spaces  | All          | Yes - centralize conversion helpers   |
| Theming via struct            | All          | Already done ✓                        |
| Memoized text caching         | egui         | Already have glyph cache ✓            |
| PickerDelegate pattern        | Zed          | Yes - for command palette             |
| Modal focus capture           | Zed, VS Code | Yes - for overlays                    |

---

## Recommended Approach

### High-Level Architecture Target

```
┌─────────────────────────────────────────────────────────────────┐
│                    Target Architecture                           │
├─────────────────────────────────────────────────────────────────┤
│  main.rs (~100-200 lines)                                       │
│    └─ CLI args, EventLoop, wiring                               │
│                                                                  │
│  app.rs                                                         │
│    └─ App struct, ApplicationHandler, mouse/drag state         │
│                                                                  │
│  input.rs                                                       │
│    └─ handle_key, event→Msg mapping, modal key routing          │
│                                                                  │
│  view.rs (or view/)                                             │
│    ├─ Renderer struct (surface, font, glyph cache)              │
│    ├─ Frame abstraction (drawing primitives)                    │
│    ├─ TextPainter (text rendering)                              │
│    └─ Widget functions:                                         │
│        ├─ render_editor_area()                                  │
│        ├─ render_editor_group()                                 │
│        ├─ render_tab_bar()                                      │
│        ├─ render_gutter()                                       │
│        ├─ render_text_area()                                    │
│        ├─ render_splitters()                                    │
│        ├─ render_status_bar()                                   │
│        └─ render_modals()           ← NEW                       │
│                                                                  │
│  modals/ (NEW)                                                  │
│    ├─ mod.rs                        Modal system                │
│    ├─ command_palette.rs            Command palette state/render│
│    ├─ goto_line.rs                  Go to line dialog           │
│    └─ find_replace.rs               Find/replace dialog         │
│                                                                  │
│  commands.rs (NEW)                                              │
│    └─ CommandId enum, COMMANDS registry                         │
│                                                                  │
│  perf.rs (debug only)                                           │
│    └─ PerfStats, render_perf_overlay                            │
└─────────────────────────────────────────────────────────────────┘
```

---

## Proposed Abstractions

### 1. Frame/Painter Abstraction

Replace direct buffer indexing with a simple drawing API:

```rust
pub struct Frame<'a> {
    pub buffer: &'a mut [u32],
    pub width: usize,
    pub height: usize,
}

impl<'a> Frame<'a> {
    #[inline]
    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    pub fn fill_rect(&mut self, rect: Rect, color: u32) {
        let x0 = rect.x.max(0.0) as usize;
        let y0 = rect.y.max(0.0) as usize;
        let x1 = (rect.x + rect.width).min(self.width as f32) as usize;
        let y1 = (rect.y + rect.height).min(self.height as f32) as usize;

        for y in y0..y1 {
            let row = &mut self.buffer[y * self.width..y * self.width + self.width];
            for x in x0..x1 {
                row[x] = color;
            }
        }
    }

    pub fn draw_hline(&mut self, y: usize, x0: usize, x1: usize, color: u32) { /* ... */ }
    pub fn draw_vline(&mut self, x: usize, y0: usize, y1: usize, color: u32) { /* ... */ }

    // Alpha blending for overlays
    pub fn fill_rect_blend(&mut self, rect: Rect, color: u32) { /* ... */ }
}
```

### 2. TextPainter Abstraction

Encapsulate text rendering:

```rust
pub struct TextPainter<'a> {
    pub font: &'a Font,
    pub glyph_cache: &'a mut GlyphCache,
    pub font_size: f32,
    pub ascent: f32,
    pub char_width: f32,
}

impl<'a> TextPainter<'a> {
    pub fn draw_text(
        &mut self,
        frame: &mut Frame,
        x_px: usize,
        y_px: usize,
        text: &str,
        color: u32,
    ) {
        // Wraps existing draw_text logic
    }

    pub fn line_height(&self) -> f32 {
        self.font_size * 1.4 // or from line metrics
    }

    pub fn baseline_offset(&self) -> f32 {
        self.ascent
    }

    pub fn measure_text(&self, text: &str) -> f32 {
        text.chars().count() as f32 * self.char_width
    }
}
```

### 3. Rect Layout Helpers

Extend the existing `Rect` with splitting utilities:

```rust
impl Rect {
    /// Split off top portion, returns (top, rest)
    pub fn split_top(self, height: f32) -> (Rect, Rect) {
        let top = Rect::new(self.x, self.y, self.width, height.min(self.height));
        let rest = Rect::new(
            self.x,
            self.y + top.height,
            self.width,
            self.height - top.height,
        );
        (top, rest)
    }

    /// Split off left portion, returns (left, rest)
    pub fn split_left(self, width: f32) -> (Rect, Rect) {
        let left = Rect::new(self.x, self.y, width.min(self.width), self.height);
        let rest = Rect::new(
            self.x + left.width,
            self.y,
            self.width - left.width,
            self.height,
        );
        (left, rest)
    }

    /// Inset by padding on all sides
    pub fn inset(self, padding: f32) -> Rect {
        Rect::new(
            self.x + padding,
            self.y + padding,
            (self.width - 2.0 * padding).max(0.0),
            (self.height - 2.0 * padding).max(0.0),
        )
    }
}
```

### 4. View Widget Functions

Plain functions, not traits. Each takes geometry, model subset, and painter:

```rust
pub fn render_editor_group(
    frame: &mut Frame,
    text: &mut TextPainter,
    group: &EditorGroup,
    editor: &EditorState,
    document: &Document,
    rect: Rect,
    is_focused: bool,
    theme: &Theme,
) {
    let (tab_bar_rect, content_rect) = rect.split_top(TAB_BAR_HEIGHT as f32);

    render_tab_bar(frame, text, group, tab_bar_rect, theme);

    let text_start = text_start_x(text.char_width);
    let (gutter_rect, text_area_rect) = content_rect.split_left(text_start);

    render_gutter(frame, text, editor, document, gutter_rect, theme);
    render_text_area(frame, text, editor, document, text_area_rect, is_focused, theme);
}
```

### 5. Shared Layout Computation

Compute rects once, use for both rendering and hit-testing:

```rust
/// Computed layout for a group, used by both rendering and input
pub struct GroupLayout {
    pub tab_bar_rect: Rect,
    pub tab_rects: Vec<(TabId, Rect)>,
    pub content_rect: Rect,
    pub gutter_rect: Rect,
    pub text_area_rect: Rect,
}

impl GroupLayout {
    pub fn compute(group_rect: Rect, group: &EditorGroup, char_width: f32) -> Self {
        let (tab_bar_rect, content_rect) = group_rect.split_top(TAB_BAR_HEIGHT as f32);
        let tab_rects = compute_tab_rects(group, tab_bar_rect, char_width);

        let text_start = text_start_x(char_width);
        let (gutter_rect, text_area_rect) = content_rect.split_left(text_start);

        Self { tab_bar_rect, tab_rects, content_rect, gutter_rect, text_area_rect }
    }

    pub fn tab_at_point(&self, x: f32, y: f32) -> Option<TabId> {
        self.tab_rects.iter()
            .find(|(_, rect)| rect.contains(x, y))
            .map(|(id, _)| *id)
    }
}
```

---

## Command Palette & Modal Overlay System

This section details how to implement a command palette (Cmd+Shift+P) and other modal overlays in a way that fits with the existing Elm Architecture and extends the current `overlay.rs` primitives.

### Design Goals

1. **Reuse existing overlay primitives** - `OverlayConfig`, `OverlayBounds`, `blend_pixel()`
2. **Elm-style consistency** - All state changes through `Msg` → `update`
3. **Single active modal** - One modal at a time captures keyboard focus
4. **Easy to add new modals** - Adding goto-line, find/replace follows same pattern
5. **Fuzzy search support** - For command palette filtering

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Modal System Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  UiState.active_modal: Option<ModalState>                       │
│       │                                                          │
│       ├─ None → Normal editor mode, keys go to editor           │
│       │                                                          │
│       └─ Some(modal) → Modal captures focus                     │
│            ├─ CommandPalette(CommandPaletteState)               │
│            ├─ GotoLine(GotoLineState)                           │
│            └─ FindReplace(FindReplaceState)                     │
│                                                                  │
│  Input Flow:                                                    │
│  ┌──────────┐   ┌───────────────┐   ┌────────────────┐         │
│  │ Keyboard │ → │ active_modal? │ → │ Modal handlers │         │
│  │  Event   │   │   is Some?    │   │ (palette keys) │         │
│  └──────────┘   └───────────────┘   └────────────────┘         │
│                        │ No                                      │
│                        ↓                                         │
│                 ┌────────────────┐                              │
│                 │ Editor handlers│                              │
│                 │ (normal keys)  │                              │
│                 └────────────────┘                              │
│                                                                  │
│  Render Flow:                                                   │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐        │
│  │ Editor Area  │ → │ Status Bar   │ → │ Modal Overlay│        │
│  │              │   │              │   │ (on top)     │        │
│  └──────────────┘   └──────────────┘   └──────────────┘        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 1. Modal State Model

Extend `UiState` in `model/ui.rs`:

```rust
// model/ui.rs

/// The currently active modal overlay
#[derive(Debug, Clone)]
pub enum ModalState {
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    FindReplace(FindReplaceState),
}

#[derive(Debug, Clone)]
pub struct UiState {
    // ... existing fields ...
    pub status_message: String,
    pub status_bar: StatusBar,
    pub transient_message: Option<TransientMessage>,
    pub cursor_visible: bool,
    pub last_cursor_blink: Instant,
    pub is_loading: bool,
    pub is_saving: bool,

    /// Active modal overlay (captures keyboard focus when Some)
    pub active_modal: Option<ModalState>,
}
```

### 2. Command Palette State

```rust
// modals/command_palette.rs

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// Current search query
    pub query: String,
    /// Selected item index in filtered list
    pub selected_index: usize,
    /// Indexes into COMMANDS that match the query
    pub matches: Vec<usize>,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        let mut state = Self {
            query: String::new(),
            selected_index: 0,
            matches: Vec::new(),
        };
        state.recompute_matches();
        state
    }

    pub fn recompute_matches(&mut self) {
        use crate::commands::COMMANDS;

        self.matches.clear();

        if self.query.is_empty() {
            // Show all commands in declaration order
            self.matches.extend(0..COMMANDS.len());
        } else {
            // Fuzzy match against command titles
            let query_lower = self.query.to_lowercase();
            let mut scored: Vec<(usize, i32)> = COMMANDS
                .iter()
                .enumerate()
                .filter_map(|(idx, cmd)| {
                    fuzzy_score(&cmd.title.to_lowercase(), &query_lower)
                        .map(|score| (idx, score))
                })
                .collect();

            // Sort by score descending
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.matches = scored.into_iter().map(|(idx, _)| idx).collect();
        }

        // Clamp selection
        self.selected_index = self.selected_index
            .min(self.matches.len().saturating_sub(1));
    }
}

/// Simple fuzzy scoring: returns Some(score) if all query chars found in order
fn fuzzy_score(text: &str, query: &str) -> Option<i32> {
    let mut score = 0;
    let mut text_chars = text.chars().peekable();

    for qc in query.chars() {
        loop {
            match text_chars.next() {
                Some(tc) if tc == qc => {
                    score += 1;
                    break;
                }
                Some(_) => continue,
                None => return None, // Query char not found
            }
        }
    }

    Some(score)
}
```

### 3. Command Registry

```rust
// commands.rs

/// Unique identifier for each command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    // Meta / Palette
    ShowCommandPalette,
    GotoLine,
    FindInFile,

    // File operations
    NewFile,
    OpenFile,
    SaveFile,
    CloseFile,

    // Layout
    SplitRight,
    SplitDown,
    CloseGroup,
    CloseTab,
    NextTab,
    PrevTab,
    FocusNextGroup,
    FocusPrevGroup,

    // Editor
    Undo,
    Redo,
    SelectAll,
    ToggleComment,

    // View
    TogglePerfOverlay,
}

/// Command metadata for the palette
pub struct Command {
    pub id: CommandId,
    pub title: &'static str,
    pub category: &'static str,
    pub shortcut_hint: Option<&'static str>,
}

/// All registered commands
pub static COMMANDS: &[Command] = &[
    Command {
        id: CommandId::ShowCommandPalette,
        title: "View: Command Palette",
        category: "View",
        shortcut_hint: Some("⇧⌘P"),
    },
    Command {
        id: CommandId::GotoLine,
        title: "Go to Line...",
        category: "Navigation",
        shortcut_hint: Some("⌘G"),
    },
    Command {
        id: CommandId::FindInFile,
        title: "Find in File",
        category: "Search",
        shortcut_hint: Some("⌘F"),
    },
    Command {
        id: CommandId::SaveFile,
        title: "File: Save",
        category: "File",
        shortcut_hint: Some("⌘S"),
    },
    Command {
        id: CommandId::SplitRight,
        title: "View: Split Editor Right",
        category: "View",
        shortcut_hint: Some("⌘\\"),
    },
    Command {
        id: CommandId::SplitDown,
        title: "View: Split Editor Down",
        category: "View",
        shortcut_hint: None,
    },
    Command {
        id: CommandId::Undo,
        title: "Edit: Undo",
        category: "Edit",
        shortcut_hint: Some("⌘Z"),
    },
    Command {
        id: CommandId::Redo,
        title: "Edit: Redo",
        category: "Edit",
        shortcut_hint: Some("⇧⌘Z"),
    },
    // ... add more as needed
];
```

### 4. Message Types

```rust
// messages.rs additions

/// Messages for command palette interaction
#[derive(Debug, Clone)]
pub enum CommandPaletteMsg {
    InsertChar(char),
    Backspace,
    SelectionUp,
    SelectionDown,
    Confirm,
    Cancel,
}

/// Messages for goto line dialog
#[derive(Debug, Clone)]
pub enum GotoLineMsg {
    InsertChar(char),
    Backspace,
    Confirm,
    Cancel,
}

/// Messages for find/replace
#[derive(Debug, Clone)]
pub enum FindReplaceMsg {
    InsertFindChar(char),
    DeleteFindChar,
    NextMatch,
    PrevMatch,
    Cancel,
}

/// Wrapper for all modal messages
#[derive(Debug, Clone)]
pub enum ModalMsg {
    CommandPalette(CommandPaletteMsg),
    GotoLine(GotoLineMsg),
    FindReplace(FindReplaceMsg),
}

/// Extended UiMsg
#[derive(Debug, Clone)]
pub enum UiMsg {
    // ... existing variants ...
    SetStatus(String),
    BlinkCursor,

    // Modal management
    OpenCommandPalette,
    OpenGotoLine,
    OpenFindReplace,
    CloseModal,
    Modal(ModalMsg),
}

/// Extended AppMsg
#[derive(Debug, Clone)]
pub enum AppMsg {
    // ... existing variants ...
    Resize(u32, u32),
    SaveFile,
    Quit,

    /// Execute a command from the palette or keybinding
    ExecuteCommand(CommandId),
}
```

### 5. Update Handlers

```rust
// update.rs additions

fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    match msg {
        // ... existing matches ...

        Msg::Ui(UiMsg::OpenCommandPalette) => {
            model.ui.active_modal = Some(ModalState::CommandPalette(
                CommandPaletteState::new()
            ));
            Some(Cmd::Redraw)
        }

        Msg::Ui(UiMsg::CloseModal) => {
            model.ui.active_modal = None;
            Some(Cmd::Redraw)
        }

        Msg::Ui(UiMsg::Modal(modal_msg)) => {
            update_modal(model, modal_msg);
            Some(Cmd::Redraw)
        }

        Msg::App(AppMsg::ExecuteCommand(cmd_id)) => {
            execute_command(model, cmd_id)
        }

        // ... rest ...
    }
}

fn update_modal(model: &mut AppModel, msg: ModalMsg) {
    match (&mut model.ui.active_modal, msg) {
        (Some(ModalState::CommandPalette(state)), ModalMsg::CommandPalette(m)) => {
            update_command_palette(model, state, m);
        }
        (Some(ModalState::GotoLine(state)), ModalMsg::GotoLine(m)) => {
            update_goto_line(model, state, m);
        }
        _ => {}
    }
}

fn update_command_palette(model: &mut AppModel, state: &mut CommandPaletteState, msg: CommandPaletteMsg) {
    use CommandPaletteMsg::*;

    match msg {
        InsertChar(ch) => {
            state.query.push(ch);
            state.recompute_matches();
        }
        Backspace => {
            state.query.pop();
            state.recompute_matches();
        }
        SelectionUp => {
            state.selected_index = state.selected_index.saturating_sub(1);
        }
        SelectionDown => {
            if !state.matches.is_empty() {
                state.selected_index = (state.selected_index + 1)
                    .min(state.matches.len() - 1);
            }
        }
        Confirm => {
            if let Some(&cmd_idx) = state.matches.get(state.selected_index) {
                let cmd_id = crate::commands::COMMANDS[cmd_idx].id;
                model.ui.active_modal = None;
                execute_command(model, cmd_id);
            }
        }
        Cancel => {
            model.ui.active_modal = None;
        }
    }
}

fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    use CommandId::*;

    match cmd_id {
        ShowCommandPalette => {
            model.ui.active_modal = Some(ModalState::CommandPalette(
                CommandPaletteState::new()
            ));
            Some(Cmd::Redraw)
        }
        GotoLine => {
            model.ui.active_modal = Some(ModalState::GotoLine(
                GotoLineState::new()
            ));
            Some(Cmd::Redraw)
        }
        SaveFile => {
            // Delegate to existing save logic
            update(model, Msg::App(AppMsg::SaveFile))
        }
        SplitRight => {
            update(model, Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)))
        }
        Undo => {
            update(model, Msg::Document(DocumentMsg::Undo))
        }
        // ... map other commands to existing Msg variants
        _ => Some(Cmd::Redraw)
    }
}
```

### 6. Input Routing (Focus Capture)

```rust
// input.rs

pub fn handle_key(model: &AppModel, event: &KeyEvent) -> Option<Msg> {
    // 1. Check if a modal is active and should capture this key
    if model.ui.active_modal.is_some() {
        return handle_modal_key(model, event);
    }

    // 2. Otherwise, normal editor key handling
    handle_editor_key(model, event)
}

fn handle_modal_key(model: &AppModel, event: &KeyEvent) -> Option<Msg> {
    use winit::keyboard::{Key, NamedKey};

    if event.state != ElementState::Pressed {
        return None;
    }

    // Common modal keys
    match &event.logical_key {
        Key::Named(NamedKey::Escape) => {
            return Some(Msg::Ui(UiMsg::CloseModal));
        }
        Key::Named(NamedKey::ArrowUp) => {
            return Some(Msg::Ui(UiMsg::Modal(
                ModalMsg::CommandPalette(CommandPaletteMsg::SelectionUp)
            )));
        }
        Key::Named(NamedKey::ArrowDown) => {
            return Some(Msg::Ui(UiMsg::Modal(
                ModalMsg::CommandPalette(CommandPaletteMsg::SelectionDown)
            )));
        }
        Key::Named(NamedKey::Enter) => {
            return Some(Msg::Ui(UiMsg::Modal(
                ModalMsg::CommandPalette(CommandPaletteMsg::Confirm)
            )));
        }
        Key::Named(NamedKey::Backspace) => {
            return Some(Msg::Ui(UiMsg::Modal(
                ModalMsg::CommandPalette(CommandPaletteMsg::Backspace)
            )));
        }
        Key::Character(s) => {
            if let Some(ch) = s.chars().next() {
                if !ch.is_control() {
                    return Some(Msg::Ui(UiMsg::Modal(
                        ModalMsg::CommandPalette(CommandPaletteMsg::InsertChar(ch))
                    )));
                }
            }
        }
        _ => {}
    }

    None
}
```

### 7. Rendering Modals

Extend `overlay.rs` concepts for modal rendering:

```rust
// view/modals.rs

use crate::overlay::{OverlayConfig, OverlayAnchor, render_overlay_background, render_overlay_border};
use crate::model::ui::{UiState, ModalState};
use crate::view::{Frame, TextPainter};
use crate::theme::Theme;
use crate::commands::COMMANDS;

/// Render active modal on top of everything else
pub fn render_modals(
    frame: &mut Frame,
    text: &mut TextPainter,
    ui: &UiState,
    theme: &Theme,
) {
    let Some(modal) = &ui.active_modal else { return };

    // 1. Dim the background
    let dim_color = 0x80000000; // 50% black
    for pixel in frame.buffer.iter_mut() {
        *pixel = blend_pixel(dim_color, *pixel);
    }

    // 2. Render the specific modal
    match modal {
        ModalState::CommandPalette(state) => {
            render_command_palette(frame, text, state, theme);
        }
        ModalState::GotoLine(state) => {
            render_goto_line(frame, text, state, theme);
        }
        ModalState::FindReplace(state) => {
            render_find_replace(frame, text, state, theme);
        }
    }
}

fn render_command_palette(
    frame: &mut Frame,
    text: &mut TextPainter,
    state: &CommandPaletteState,
    theme: &Theme,
) {
    let vw = frame.width;
    let vh = frame.height;

    // Palette dimensions
    let width = (vw as f32 * 0.5).max(400.0).min(600.0) as usize;
    let line_h = text.line_height() as usize;
    let max_visible_items = 12;
    let height = line_h * 2 + line_h * max_visible_items.min(state.matches.len().max(1));

    // Center horizontally, near top vertically
    let x = (vw - width) / 2;
    let y = vh / 6;

    let bounds = OverlayBounds { x, y, width, height };

    // Background + border
    render_overlay_background(frame.buffer, &bounds, theme.overlay.background.to_argb_u32(), vw, vh);
    render_overlay_border(frame.buffer, &bounds, theme.overlay.border.to_argb_u32(), vw, vh);

    let padding = 8.0;
    let mut cursor_y = y as f32 + padding;

    // Input field
    let input_bg = theme.overlay.input_background.to_argb_u32();
    let input_rect = Rect::new(x as f32 + padding, cursor_y, width as f32 - 2.0 * padding, line_h as f32);
    frame.fill_rect(input_rect, input_bg);

    let prompt = format!("> {}", state.query);
    text.draw_text(frame, (x as f32 + padding + 4.0) as usize, cursor_y as usize, &prompt, theme.overlay.text.to_argb_u32());

    cursor_y += line_h as f32 + padding;

    // Separator line
    frame.draw_hline(cursor_y as usize, x + padding as usize, x + width - padding as usize, theme.overlay.border.to_argb_u32());
    cursor_y += 1.0;

    // Command list
    for (visible_idx, &cmd_idx) in state.matches.iter().take(max_visible_items).enumerate() {
        let cmd = &COMMANDS[cmd_idx];
        let is_selected = visible_idx == state.selected_index;

        let row_rect = Rect::new(x as f32, cursor_y, width as f32, line_h as f32);

        if is_selected {
            frame.fill_rect(row_rect, theme.overlay.selection_background.to_argb_u32());
        }

        // Command title (left)
        text.draw_text(
            frame,
            (x as f32 + padding) as usize,
            cursor_y as usize,
            cmd.title,
            theme.overlay.text.to_argb_u32(),
        );

        // Shortcut hint (right-aligned)
        if let Some(shortcut) = cmd.shortcut_hint {
            let shortcut_width = text.measure_text(shortcut);
            text.draw_text(
                frame,
                (x as f32 + width as f32 - padding - shortcut_width) as usize,
                cursor_y as usize,
                shortcut,
                theme.overlay.hint.to_argb_u32(),
            );
        }

        cursor_y += line_h as f32;
    }
}
```

### 8. Theme Extensions

Add modal/overlay colors to theme:

```rust
// In theme.rs, add to UiThemeData:

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OverlayThemeData {
    pub background: String,
    pub border: String,
    pub text: String,
    pub hint: String,
    pub input_background: String,
    pub selection_background: String,
}
```

Example YAML:

```yaml
ui:
  overlay:
    background: "#1E1E1E"
    border: "#3C3C3C"
    text: "#CCCCCC"
    hint: "#808080"
    input_background: "#2D2D2D"
    selection_background: "#094771"
```

### 9. Integration into Main Render Loop

```rust
// In view.rs or Renderer::render_impl

fn render_impl(&mut self, model: &AppModel) {
    let mut frame = Frame::new(&mut self.buffer, self.width, self.height);
    let mut text = TextPainter::new(&self.font, &mut self.glyph_cache, self.font_size);

    // 1. Render main editor UI
    render_editor_area(&mut frame, &mut text, model);
    render_status_bar(&mut frame, &mut text, model);

    // 2. Render modal overlays on top (if any)
    render_modals(&mut frame, &mut text, &model.ui, &model.theme);

    // 3. Debug overlay last
    #[cfg(debug_assertions)]
    if self.show_perf_overlay {
        render_perf_overlay(&mut frame, &mut text, &self.perf_stats);
    }
}
```

### Modal System Summary

```
┌────────────────────────────────────────────────────────────────┐
│                    Modal Overlay Stack                          │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                 Command Palette Modal                    │   │
│  │  ┌─────────────────────────────────────────────────────┐│   │
│  │  │ > search query                                      ││   │
│  │  ├─────────────────────────────────────────────────────┤│   │
│  │  │ ▸ View: Command Palette              ⇧⌘P            ││   │
│  │  │   Go to Line...                      ⌘G             ││   │
│  │  │   Find in File                       ⌘F             ││   │
│  │  │   File: Save                         ⌘S             ││   │
│  │  │   View: Split Editor Right           ⌘\             ││   │
│  │  └─────────────────────────────────────────────────────┘│   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ░░░░░░░░░░░░░░░░ Dimmed Editor Background ░░░░░░░░░░░░░░░░    │
│                                                                 │
└────────────────────────────────────────────────────────────────┘

Key Bindings:
  Escape    → Close modal
  ↑/↓       → Navigate selection
  Enter     → Confirm selection
  Typing    → Filter commands
```

### Adding New Modals

To add a new modal (e.g., "Open File" dialog):

1. Add `ModalState::OpenFile(OpenFileState)` variant
2. Create `OpenFileState` struct with its state (path input, file list, etc.)
3. Add `OpenFileMsg` enum with its messages
4. Add `ModalMsg::OpenFile(OpenFileMsg)` variant
5. Add `update_open_file()` handler
6. Add `render_open_file()` function
7. Add keybinding to `OpenCommandPalette` or `OpenFile` CommandId

The system scales linearly—each modal is self-contained.

---

## Honest Critique

### What's Working Well

| Aspect                  | Assessment                                        |
| ----------------------- | ------------------------------------------------- |
| Model layer (`model/*`) | ✓ Clean, well-factored, good encapsulation        |
| Elm Architecture        | ✓ Great fit for an editor, predictable state flow |
| EditorArea layout tree  | ✓ Solid foundation for splits and groups          |
| Overlay system          | ✓ Good reusable pattern, worth expanding          |
| Theme system            | ✓ Centralized, YAML-based, extensible             |

### What Needs Improvement

| Issue                               | Impact                                | Severity |
| ----------------------------------- | ------------------------------------- | -------- |
| `main.rs` as god file (~3000 lines) | Hard to reason about, hard to test    | High     |
| Raw buffer indexing everywhere      | Error-prone, discourages reuse        | Medium   |
| No widget/view layer                | Adding UI elements is risky           | Medium   |
| Layout logic scattered              | Same calculations in render and input | Medium   |
| Coupled input/render coordinates    | Changes need multiple updates         | Medium   |
| No modal/focus system               | Can't implement command palette       | Medium   |

### Patterns to Avoid Going Forward

1. **Don't add more code to `main.rs`**
   - Route new features through proper modules

2. **Don't compute coordinates inline in render loops**
   - Extract to layout helpers first

3. **Don't duplicate hit-test and render coordinate logic**
   - Share computed `Rect`s

4. **Don't hard-code colors**
   - Always go through `theme`

5. **Don't bypass the modal focus system**
   - All keyboard events check `active_modal` first

---

## Migration Path

### Phase 1: Implement Planned File Split (1-3 hours)

Follow `ORGANIZATION-CODEBASE.md`:

```bash
src/
  main.rs          # Entry point only
  app.rs           # App struct + ApplicationHandler
  input.rs         # handle_key + event→Msg
  view.rs          # Renderer + drawing
  perf.rs          # PerfStats (debug only)
```

### Phase 2: Introduce Frame/TextPainter (1-2 hours)

1. Define `Frame` and `TextPainter` structs in `view.rs`
2. Refactor `Renderer::render_impl` to use new abstractions
3. Convert one function (status bar) to use new abstractions
4. Verify visually, run tests

### Phase 3: Extract Widget Functions (3-8 hours, piecemeal)

| Order | Widget              | Effort | Dependencies                        |
| ----- | ------------------- | ------ | ----------------------------------- |
| 1     | Status bar          | S      | Already self-contained              |
| 2     | Splitters           | S      | Uses existing `SplitterBar`         |
| 3     | Tab bar             | M      | Need `compute_tab_rects`            |
| 4     | Single editor group | M      | Composes tab bar, gutter, text area |
| 5     | Gutter              | M      | Line number rendering               |
| 6     | Text area           | L      | Largest, most complex               |
| 7     | All groups          | S      | Generalize single group             |

### Phase 4: Add Modal System (1-2 days)

| Order | Task                              | Effort |
| ----- | --------------------------------- | ------ |
| 1     | Add `ModalState` to `UiState`     | S      |
| 2     | Add modal message types           | S      |
| 3     | Add input routing for modals      | M      |
| 4     | Implement command registry        | S      |
| 5     | Implement `CommandPaletteState`   | M      |
| 6     | Add `render_modals()` function    | M      |
| 7     | Add theme extensions for overlays | S      |
| 8     | Wire up Cmd+Shift+P keybinding    | S      |
| 9     | Add fuzzy search                  | M      |

### Phase 5: Consolidate Layout (2-4 hours)

1. Create `GroupLayout` struct
2. Compute once per frame in render prep
3. Use same layout in input handling for hit-testing
4. Remove duplicate coordinate calculations

### Phase 6: Additional Modals (Ongoing)

- Add Goto Line dialog
- Add Find/Replace dialog
- Add Open File dialog
- Each follows the same pattern

---

## When to Consider Advanced Paths

### Stay with Current Approach If:

- Building a focused code editor
- Don't need plugin-defined arbitrary UI
- Single frontend (winit/softbuffer)
- Handful of predefined modals

### Consider Full Widget System If:

- Want plugin-injected UI panels
- Need docking, complex panels, tree views
- Want to support multiple frontends (TUI, web)
- Need dynamic modal registration

### If Going Advanced:

```rust
// Trait-based modal system
pub trait ModalWidget {
    type Msg;

    fn update(&mut self, msg: Self::Msg, model: &mut AppModel) -> ModalResult;
    fn render(&self, frame: &mut Frame, text: &mut TextPainter, theme: &Theme);
    fn overlay_config(&self, viewport: (usize, usize)) -> OverlayConfig;
}

pub enum ModalResult {
    StayOpen,
    Close,
    CloseAndExecute(CommandId),
}

// Dynamic registration
pub struct ModalRegistry {
    modals: HashMap<TypeId, Box<dyn AnyModal>>,
}
```

But this is only worth the complexity for truly extensible UIs.

---

## Appendix: Component Diagram with Modals

```
┌──────────────────────────────────────────────────────────────────────┐
│                           Window                                      │
├──────────────────────────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ Editor Area                                                       │ │
│ │ ┌───────────────────────────┬─────┬───────────────────────────┐  │ │
│ │ │ Editor Group (Left)       │ ▐ ▌ │ Editor Group (Right)      │  │ │
│ │ │ ┌───────────────────────┐ │  S  │ ┌───────────────────────┐ │  │ │
│ │ │ │ Tab Bar               │ │  P  │ │ Tab Bar               │ │  │ │
│ │ │ │ [file.rs] [main.rs]   │ │  L  │ │ [other.rs]            │ │  │ │
│ │ │ └───────────────────────┘ │  I  │ └───────────────────────┘ │  │ │
│ │ │ ┌────┬──────────────────┐ │  T  │ ┌────┬──────────────────┐ │  │ │
│ │ │ │ G  │ Text Area        │ │  T  │ │ G  │ Text Area        │ │  │ │
│ │ │ │ U  │                  │ │  E  │ │ U  │                  │ │  │ │
│ │ │ │ T  │ fn main() {      │ │  R  │ │ T  │ struct Foo {     │ │  │ │
│ │ │ │ T  │     println!();  │ │     │ │ T  │     bar: i32,    │ │  │ │
│ │ │ │ E  │ }                │ │     │ │ E  │ }                │ │  │ │
│ │ │ │ R  │                  │ │     │ │ R  │                  │ │  │ │
│ │ │ └────┴──────────────────┘ │     │ └────┴──────────────────┘ │  │ │
│ │ └───────────────────────────┴─────┴───────────────────────────┘  │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ Status Bar                                                        │ │
│ │ [file.rs] [Ln 42, Col 12] [UTF-8] [Rust]              [Modified] │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ ░░░░░░░░░░░░░░░░░░░ Modal Overlay Layer ░░░░░░░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░┌─────────────────────────────────────┐░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░│ Command Palette                     │░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░│ > search                            │░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░├─────────────────────────────────────┤░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░│ ▸ File: Save                   ⌘S   │░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░│   Go to Line...                ⌘G   │░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░│   View: Split Right            ⌘\   │░░░░░░░░░░░░░░░░░░ │ │
│ │ ░░░░░░░└─────────────────────────────────────┘░░░░░░░░░░░░░░░░░░ │ │
│ └──────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘

Component Responsibilities:
────────────────────────────
Tab Bar        → render_tab_bar()        → Tab switching, close buttons
Gutter         → render_gutter()         → Line numbers, fold icons
Text Area      → render_text_area()      → Code, cursor, selection
Splitter       → render_splitters()      → Drag resize between groups
Status Bar     → render_status_bar()     → File info, cursor position
Modal Layer    → render_modals()         → Command palette, goto line, find
```

---

## References

- [ORGANIZATION-CODEBASE.md](ORGANIZATION-CODEBASE.md) - Existing refactoring plan
- [EDITOR_UI_REFERENCE.md](../EDITOR_UI_REFERENCE.md) - Text editor UI concepts
- [areweguiyet.com](https://areweguiyet.com) - Rust GUI framework comparison
- [GPUI docs](https://www.gpui.rs/) - Production code editor GUI framework
- [Zed command_palette](https://github.com/zed-industries/zed/blob/main/crates/command_palette/src/command_palette.rs) - Command palette implementation
- [Zed picker](https://github.com/zed-industries/zed/blob/main/crates/picker/src/picker.rs) - Picker delegate pattern
- [iced](https://github.com/iced-rs/iced) - Elm-style Rust GUI
- [egui](https://github.com/emilk/egui) - Immediate mode Rust GUI

---

## Comprehensive GUI Action Plan (2025-12-07)

Based on research from Zed (GPUI), Helix, Alacritty, and Wezterm, here is the prioritized implementation roadmap.

### Research Summary

| Project        | Key Patterns Adopted                                                          | Patterns Skipped                                            |
| -------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------- |
| **Zed (GPUI)** | Modal focus capture, FocusHandle concept, hitbox blocking for overlays        | Full 3-phase render, entity system, DispatchTree complexity |
| **Helix**      | Compositor with layer stack, overlay wrapper, EventResult::Consumed/Ignored   | Full trait-based Component system, callback pattern         |
| **Alacritty**  | Line-level damage tracking, frame damage accumulation, double-buffered damage | Platform-specific Wayland optimizations                     |
| **Wezterm**    | Layer-based z-ordering concept                                                | Quad-based GPU rendering, triple-buffered vertices          |

### Phase 1 – Frame/Painter Abstraction

**Goal:** Centralize drawing primitives; stop indexing pixel buffer directly everywhere.  
**Effort:** M (1–3h)  
**User Impact:** None (internal refactor, unblocks everything else)

**Files to modify:**

- `src/view.rs` – Add `Frame` and `TextPainter` structs
- `src/overlay.rs` – Migrate to use `Frame` helpers

**Steps:**

1. Add `Frame` struct with `clear()`, `fill_rect()`, `blend_pixel()` methods
2. Add `TextPainter` wrapper for fontdue + glyph cache
3. Wrap `Renderer::render_impl()` to create `Frame` from softbuffer
4. Migrate existing pixel loops to `Frame` methods (status bar → tab bar → gutter → text area)
5. Migrate `overlay.rs` to take `&mut Frame` instead of raw buffer

```rust
pub struct Frame<'a> {
    pub buffer: &'a mut [u32],
    pub width: usize,
    pub height: usize,
}

impl<'a> Frame<'a> {
    pub fn clear(&mut self, color: u32);
    pub fn fill_rect(&mut self, rect: Rect, color: u32);
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32);
}

pub struct TextPainter<'a> {
    font: &'a Font,
    glyph_cache: &'a mut GlyphCache,
}
```

---

### Phase 2 – Widget Extraction & Geometry Centralization

**Goal:** Transform monolithic render function into composable widget functions.  
**Effort:** M–L (3–8h, incremental)  
**User Impact:** Invisible, improves maintainability

**Files to modify:**

- `src/view.rs` – Extract widget functions
- `src/model/editor_area.rs` or new `src/view/geometry.rs` – Centralize geometry helpers

**Steps:**

1. Extract high-level widget renderers:
   - `render_root()` – orchestrates all rendering
   - `render_editor_area()` – groups + splitters
   - `render_editor_group()` – tab bar + editor pane
   - `render_tab_bar()`, `render_gutter()`, `render_text_area()`
   - `render_splitters()`, `render_status_bar()`

2. Centralize geometry helpers (from EDITOR_UI_REFERENCE.md):
   - `compute_visible_lines()`
   - Line/column ↔ pixel conversions
   - Gutter width computation

3. Unify hit-testing geometry between `input.rs` and `view.rs`
   - Single source of truth for tab bar rect, text area rect, gutter rect per group

**Structure (when view.rs > 1200 LOC):**

```
src/view/
  mod.rs          # Exports Renderer, Frame, TextPainter
  editor.rs       # Editor area widgets
  chrome.rs       # Tab bar, status bar
  modals.rs       # Overlay rendering
  geometry.rs     # Shared coordinate helpers
```

---

### Phase 3 – Basic Modal/Focus System

**Goal:** Add minimal modal overlay + focus capture mechanism.  
**Effort:** M (1–3h)  
**User Impact:** Foundation only (add placeholder modal to test)

**Files to modify:**

- `src/model/ui.rs` – Add `ModalState` enum, extend `UiState`
- `src/messages.rs` – Add `ModalMsg`, extend `UiMsg`
- `src/update/ui.rs` – Handle modal state changes
- `src/input.rs` – Implement keyboard focus capture
- `src/view.rs` – Add `render_modals()` with dim background

**Model changes:**

```rust
pub enum ModalState {
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    FindReplace(FindReplaceState),
}

pub struct UiState {
    pub active_modal: Option<ModalState>,
    // existing fields...
}
```

**Input routing (focus capture):**

```rust
pub fn handle_key(model: &AppModel, event: &KeyEvent) -> Option<Msg> {
    if model.ui.active_modal.is_some() {
        return handle_modal_key(model, event);
    }
    handle_editor_key(model, event)
}
```

**Rendering (layer 2):**

```rust
fn render_modals(frame: &mut Frame, text: &mut TextPainter, ui: &UiState, theme: &Theme) {
    let Some(modal) = &ui.active_modal else { return };

    // 1. Dim background (Zed-style BlockMouse)
    let dim_color = 0x80000000;
    for pixel in frame.buffer.iter_mut() {
        *pixel = blend_pixel(dim_color, *pixel);
    }

    // 2. Render modal content
    match modal { /* ... */ }
}
```

---

### Phase 4 – Command Palette (Full Vertical Slice)

**Goal:** Ship real, useful command palette on modal system.  
**Effort:** L (1–2d)  
**User Impact:** HIGH – visible feature, anchors modal system

**Files to create/modify:**

- `src/commands.rs` – Add `CommandId` enum, `COMMANDS` registry
- `src/messages.rs` – Add `AppMsg::ExecuteCommand`
- `src/model/ui.rs` – Add `CommandPaletteState`, `CommandPaletteMsg`
- `src/update/app.rs` – Add `execute_command()` dispatcher
- `src/update/ui.rs` – Add `update_command_palette()`
- `src/input.rs` – Extend modal key routing, add Cmd+P binding
- `src/view.rs` – Add `render_command_palette()`

**Command registry:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    ShowCommandPalette,
    GotoLine,
    SaveFile,
    SplitRight,
    SplitDown,
    Undo,
    Redo,
    // ...
}

pub struct Command {
    pub id: CommandId,
    pub label: &'static str,
    pub description: &'static str,
    pub default_keybinding: Option<&'static str>,
}

pub static COMMANDS: &[Command] = &[/* ... */];
```

**Execute command flow:**

```rust
fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    match cmd_id {
        CommandId::ShowCommandPalette => { /* open palette */ }
        CommandId::SaveFile => update_app(model, AppMsg::SaveFile),
        CommandId::SplitRight => update_layout(model, LayoutMsg::SplitFocusedHorizontal),
        CommandId::Undo => update_document(model, DocumentMsg::Undo),
        // ...
    }
}
```

---

### Phase 5 – General Overlay/Compositor & Mouse Blocking

**Goal:** Make overlays first-class layers with predictable z-order and event behavior.  
**Effort:** M (1–3h)  
**User Impact:** Subtle – clicks don't leak through modals

**Files to modify:**

- `src/view.rs` – Formalize layer ordering
- `src/model/ui.rs` – Add `ModalRuntimeGeometry` for hit-testing
- `src/input.rs` – Add mouse blocking for active modal

**Layer stack (conceptual):**

```rust
pub fn render_root(...) {
    render_editor_area(...);    // layer 0
    render_status_bar(...);     // layer 1
    render_modals(...);         // layer 2
    render_perf_overlay(...);   // layer 3 (debug)
}
```

**Mouse blocking:**

```rust
pub fn handle_mouse(model: &AppModel, event: &MouseEvent) -> Option<Msg> {
    if let Some(geom) = &model.ui.active_modal_geometry {
        if point_in_rect(event.position, geom.rect) {
            // Route to modal (future: clicking in search results)
            return Some(Msg::Ui(UiMsg::Modal(/* ... */)));
        } else {
            // Click outside modal → close
            return Some(Msg::Ui(UiMsg::CloseModal));
        }
    }
    handle_editor_mouse(model, event)
}
```

---

### Phase 6 – Goto Line & Find/Replace Modals

**Goal:** Reuse modal infrastructure for high-value overlays.  
**Effort:** L (1–3h each, ~1d total)  
**User Impact:** HIGH – complete basic modal feature set

**Goto Line:**

- Add `GotoLineState` with `input: String`
- Add `GotoLineMsg` variants
- Parse input, move cursor, adjust scroll using EDITOR_UI_REFERENCE helpers
- Render centered smaller modal with single input line

**Find/Replace:**

- Add `FindReplaceState` with `query`, `replacement`, `mode`, `case_sensitive`
- Wire into existing occurrence selection in `update/editor.rs`
- Render as bar at top or modal dialog

**Keyboard shortcuts:**

- `Cmd+G` or `Ctrl+G` → `GotoLine`
- `Cmd+F` → `Find`
- `Cmd+Shift+F` or `Cmd+H` → `Replace`

---

### Phase 7 – Damage Tracking (After UI Stabilizes)

**Goal:** Add modest damage system for partial redraws.  
**Effort:** L–XL (1–3d)  
**User Impact:** Better performance on large files / high-DPI

**Files to modify:**

- `src/view.rs` – Add `FrameDamage` type
- `src/commands.rs` – Extend `Cmd` with damage hints
- `src/app.rs` (binary) – Store damage in `Renderer`

**Damage types:**

```rust
#[derive(Default, Clone)]
pub struct FrameDamage {
    pub full: bool,
    pub rects: Vec<Rect>,
}

impl FrameDamage {
    pub fn full() -> Self { Self { full: true, rects: Vec::new() } }
    pub fn add_rect(&mut self, rect: Rect) {
        if !self.full { self.rects.push(rect); }
    }
}
```

**Extended Cmd (optional):**

```rust
pub enum Cmd {
    Redraw,
    RedrawRect(Rect),
    RedrawLines { group_id: GroupId, line_range: Range<usize> },
    // ...
}
```

**Guardrail:** Start with `full = true` for all changes; only enable partial after thorough testing. Use feature flag for rollout.

---

### Summary Timeline

| Phase                 | Effort      | Dependencies | Priority                   |
| --------------------- | ----------- | ------------ | -------------------------- |
| 1. Frame/Painter      | M (1–3h)    | None         | **P0** (foundation)        |
| 2. Widget Extraction  | M–L (3–8h)  | Phase 1      | **P0** (foundation)        |
| 3. Modal/Focus System | M (1–3h)    | Phase 1      | **P0** (unblocks features) |
| 4. Command Palette    | L (1–2d)    | Phase 3      | **P1** (high user value)   |
| 5. Compositor/Mouse   | M (1–3h)    | Phase 3      | **P2** (polish)            |
| 6. Goto/Find Modals   | L (1d)      | Phase 4      | **P1** (high user value)   |
| 7. Damage Tracking    | L–XL (1–3d) | Phase 2      | **P3** (optimization)      |

**Total estimated effort:** 2–3 weeks of focused work

---

### When to Consider Advanced Path

Revisit more complex designs (Zed-style 3-phase render, trait-based components, plugin architecture) only if:

1. You want **reactive UI widgets** beyond the editor (embedded terminals, complex file trees)
2. You add a **second frontend** (TUI or web), making trait-based design worthwhile
3. Profiling shows **still CPU/GPU bound** after damage tracking on large files

Until then, this phased plan keeps the system simple, testable, and incremental.
