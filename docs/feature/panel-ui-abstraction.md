# Panel UI Abstraction

A unified docking panel system for workspace sidebar, terminal, outline, and future panels.

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** L (1-2 weeks)
> **Created:** 2025-01-07
> **Updated:** 2025-01-07
> **Milestone:** 3 - Workspace Features

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

The sidebar is implemented as a tightly-coupled component in `src/view/mod.rs` with state in `Workspace`. It only supports:

- Fixed left position
- Single panel type (file explorer)
- Manual resize via drag handle
- No abstraction for adding new panel types

### Goals

- **Unified Panel Trait**: Define a `Panel` trait that any dockable UI component can implement
- **Three-Dock Layout**: Support left, right, and bottom dock positions
- **Multi-Panel Docks**: Each dock can hold multiple panels with tab switching
- **Persistence**: Save/restore panel visibility, size, and position across sessions
- **Extensibility**: Make it trivial to add new panels (terminal, outline, AI chat, task runner)

### Non-Goals

- Panel zoom/maximize mode (not needed)
- Drag-and-drop panel movement between docks (future consideration)
- Activity bar with icons (keybindings sufficient, optional future addition)
- Top dock position (VS Code-style, not needed)

---

## Architecture

### Integration Points

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Window                                     │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌─────────────────────────────────┐  ┌──────────┐   │
│  │          │  │                                 │  │          │   │
│  │   Left   │  │                                 │  │  Right   │   │
│  │   Dock   │  │         Editor Area             │  │  Dock    │   │
│  │          │  │                                 │  │          │   │
│  │ ┌──────┐ │  │                                 │  │ ┌──────┐ │   │
│  │ │Panel │ │  │                                 │  │ │Panel │ │   │
│  │ │Tabs  │ │  │                                 │  │ │Tabs  │ │   │
│  │ └──────┘ │  │                                 │  │ └──────┘ │   │
│  │ ┌──────┐ │  │                                 │  │ ┌──────┐ │   │
│  │ │Active│ │  │                                 │  │ │Active│ │   │
│  │ │Panel │ │  │                                 │  │ │Panel │ │   │
│  │ │      │ │  │                                 │  │ │      │ │   │
│  │ └──────┘ │  │                                 │  │ └──────┘ │   │
│  └──────────┘  └─────────────────────────────────┘  └──────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Bottom Dock                             │   │
│  │  ┌─────────┬─────────┬─────────┐  ┌───────────────────────┐ │   │
│  │  │Terminal │ Output  │ Tasks   │  │    Active Panel       │ │   │
│  │  └─────────┴─────────┴─────────┘  │                       │ │   │
│  │                                    └───────────────────────┘ │   │
│  └─────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│                           Status Bar                                 │
│  [Explorer] [Outline]                    [Terminal] [Tasks] [Chat]  │
└─────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── panel/                    # New module (create)
│   ├── mod.rs               # Public exports, Panel trait
│   ├── dock.rs              # Dock container logic
│   ├── registry.rs          # Panel registration and lookup
│   └── persistence.rs       # Save/restore panel state
├── panels/                   # Concrete panel implementations
│   ├── mod.rs               # Re-exports all panels
│   ├── file_explorer.rs     # Migrated from current sidebar
│   ├── outline.rs           # Code outline (future)
│   ├── terminal.rs          # Terminal panel (future)
│   ├── task_runner.rs       # Task runner (future)
│   └── ai_chat.rs           # AI chat (future)
├── model/
│   ├── mod.rs               # Add DockState to AppModel
│   └── workspace.rs         # Remove sidebar-specific state (migrate)
├── view/
│   ├── mod.rs               # Integrate dock rendering
│   └── dock_renderer.rs     # New: dock rendering logic
└── messages.rs              # Add PanelMsg, DockMsg
```

### Message Flow

```
User Action (keybind/click)
         │
         ▼
┌─────────────────────┐
│  Msg::Panel(...)    │
│  or Msg::Dock(...)  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  update_panel()     │  ─── Routes to specific panel's update
│  update_dock()      │      logic based on panel type
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Cmd::Redraw        │
│  Cmd::SaveState     │  ─── Persist panel state on changes
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  render_docks()     │  ─── Calls Panel::render() for active panels
└─────────────────────┘
```

---

## Data Structures

### Core Types

```rust
/// Position where a dock can be placed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockPosition {
    Left,
    Right,
    Bottom,
}

impl DockPosition {
    /// Returns the axis this dock expands along
    pub fn axis(&self) -> Axis {
        match self {
            DockPosition::Left | DockPosition::Right => Axis::Vertical,
            DockPosition::Bottom => Axis::Horizontal,
        }
    }

    /// Returns valid positions for cycling (future: panel movement)
    pub fn cycle_next(&self) -> DockPosition {
        match self {
            DockPosition::Left => DockPosition::Bottom,
            DockPosition::Bottom => DockPosition::Right,
            DockPosition::Right => DockPosition::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}
```

### Panel Trait

```rust
/// Unique identifier for a panel type (used for persistence and lookup)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PanelId(&'static str);

impl PanelId {
    pub const FILE_EXPLORER: PanelId = PanelId("file_explorer");
    pub const OUTLINE: PanelId = PanelId("outline");
    pub const TERMINAL: PanelId = PanelId("terminal");
    pub const TASK_RUNNER: PanelId = PanelId("task_runner");
    pub const AI_CHAT: PanelId = PanelId("ai_chat");
    pub const TODO_LIST: PanelId = PanelId("todo_list");
}

/// Trait that all dockable panels must implement
pub trait Panel: Send + Sync {
    /// Unique identifier for this panel type
    fn id(&self) -> PanelId;

    /// Display name for tabs and tooltips
    fn display_name(&self) -> &'static str;

    /// Icon character for tab display (single char, e.g., "󰙅" for file tree)
    fn icon(&self) -> &'static str;

    /// Default dock position for this panel
    fn default_position(&self) -> DockPosition;

    /// Valid positions this panel can be placed
    fn valid_positions(&self) -> &[DockPosition] {
        &[DockPosition::Left, DockPosition::Right, DockPosition::Bottom]
    }

    /// Default size (width for left/right, height for bottom) in logical pixels
    fn default_size(&self) -> f32;

    /// Minimum size in logical pixels
    fn min_size(&self) -> f32 {
        150.0
    }

    /// Maximum size in logical pixels (None = no limit)
    fn max_size(&self) -> Option<f32> {
        None
    }

    /// Render the panel content
    fn render(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        rect: Rect,
    );

    /// Handle panel-specific messages, returns commands
    fn update(&mut self, msg: PanelMsg, model: &mut AppModel) -> Cmd;

    /// Whether this panel can receive keyboard focus
    fn focusable(&self) -> bool {
        true
    }

    /// Priority for ordering in dock tabs (lower = first)
    fn priority(&self) -> u32 {
        100
    }
}
```

### Dock State

```rust
/// State for a single dock (left, right, or bottom)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dock {
    /// Position of this dock
    pub position: DockPosition,

    /// Panels registered to this dock (by ID)
    pub panel_ids: Vec<PanelId>,

    /// Currently active panel index (None if dock is closed)
    pub active_index: Option<usize>,

    /// Whether the dock is visible
    pub is_open: bool,

    /// Size in logical pixels (width for left/right, height for bottom)
    pub size_logical: f32,
}

impl Dock {
    pub fn new(position: DockPosition) -> Self {
        Self {
            position,
            panel_ids: Vec::new(),
            active_index: None,
            is_open: false,
            size_logical: match position {
                DockPosition::Left | DockPosition::Right => 250.0,
                DockPosition::Bottom => 200.0,
            },
        }
    }

    /// Get the active panel ID, if any
    pub fn active_panel(&self) -> Option<PanelId> {
        self.active_index.and_then(|i| self.panel_ids.get(i).copied())
    }

    /// Activate a panel by ID, opening the dock if closed
    pub fn activate(&mut self, panel_id: PanelId) {
        if let Some(index) = self.panel_ids.iter().position(|id| *id == panel_id) {
            self.active_index = Some(index);
            self.is_open = true;
        }
    }

    /// Close the dock and clear active panel
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Physical size accounting for scale factor
    pub fn size(&self, scale_factor: f64) -> f32 {
        self.size_logical * scale_factor as f32
    }

    /// Set size from physical pixels
    pub fn set_size(&mut self, physical_size: f32, scale_factor: f64) {
        self.size_logical = physical_size / scale_factor as f32;
    }
}
```

### Dock Layout State (in AppModel)

```rust
/// Complete dock layout state, stored in AppModel
/// 
/// NOTE: Focus is NOT stored here. Focus lives in `UiState::focus` as
/// `FocusTarget::Dock(DockPosition)`. Use `UiState::focused_dock()` to query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayout {
    pub left: Dock,
    pub right: Dock,
    pub bottom: Dock,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            left: Dock::new(DockPosition::Left),
            right: Dock::new(DockPosition::Right),
            bottom: Dock::new(DockPosition::Bottom),
        }
    }
}

impl DockLayout {
    /// Get dock by position
    pub fn dock(&self, position: DockPosition) -> &Dock {
        match position {
            DockPosition::Left => &self.left,
            DockPosition::Right => &self.right,
            DockPosition::Bottom => &self.bottom,
        }
    }

    /// Get mutable dock by position
    pub fn dock_mut(&mut self, position: DockPosition) -> &mut Dock {
        match position {
            DockPosition::Left => &mut self.left,
            DockPosition::Right => &mut self.right,
            DockPosition::Bottom => &mut self.bottom,
        }
    }

    /// Find which dock contains a panel
    pub fn find_panel(&self, panel_id: PanelId) -> Option<DockPosition> {
        for pos in [DockPosition::Left, DockPosition::Right, DockPosition::Bottom] {
            if self.dock(pos).panel_ids.contains(&panel_id) {
                return Some(pos);
            }
        }
        None
    }

    /// Focus-then-toggle logic for panel keybindings (Cmd+1, Cmd+7, etc.)
    ///
    /// Behavior:
    /// - If the target dock is NOT focused: open dock, activate panel, focus dock
    /// - If the target dock IS focused on this panel: close dock, unfocus
    /// - If the target dock IS focused on a DIFFERENT panel: switch to this panel
    ///
    /// Returns true if dock is now open/focused, false if closed.
    /// Caller must update `ui.focus` based on return value.
    pub fn focus_or_toggle_panel(&mut self, panel_id: PanelId, ui: &UiState) -> (bool, Option<DockPosition>) {
        let Some(position) = self.find_panel(panel_id) else {
            return (false, None); // Panel not registered
        };

        let is_target_dock_focused = ui.focused_dock() == Some(position);
        let dock = self.dock_mut(position);
        let is_panel_active = dock.active_panel() == Some(panel_id);

        if is_target_dock_focused && is_panel_active && dock.is_open {
            // Already focused on this panel → close and unfocus
            dock.close();
            (false, None) // Caller should set focus to Editor
        } else {
            // Not focused on this panel → open, activate, focus
            dock.activate(panel_id);
            (true, Some(position)) // Caller should set focus to Dock(position)
        }
    }

    /// Close dock at given position
    pub fn close_dock(&mut self, position: DockPosition) {
        self.dock_mut(position).close();
    }

    /// Cycle to next panel in the specified dock
    pub fn next_panel_in_dock(&mut self, position: DockPosition) {
        let dock = self.dock_mut(position);
        if dock.panel_ids.len() > 1 {
            if let Some(current) = dock.active_index {
                dock.active_index = Some((current + 1) % dock.panel_ids.len());
            }
        }
    }

    /// Cycle to previous panel in the specified dock
    pub fn prev_panel_in_dock(&mut self, position: DockPosition) {
        let dock = self.dock_mut(position);
        if dock.panel_ids.len() > 1 {
            if let Some(current) = dock.active_index {
                let len = dock.panel_ids.len();
                dock.active_index = Some((current + len - 1) % len);
            }
        }
    }
}
```

### Panel Registry

```rust
/// Registry of all available panels (runtime)
pub struct PanelRegistry {
    panels: HashMap<PanelId, Box<dyn Panel>>,
}

impl PanelRegistry {
    pub fn new() -> Self {
        Self {
            panels: HashMap::new(),
        }
    }

    pub fn register(&mut self, panel: Box<dyn Panel>) {
        let id = panel.id();
        self.panels.insert(id, panel);
    }

    pub fn get(&self, id: PanelId) -> Option<&dyn Panel> {
        self.panels.get(&id).map(|p| p.as_ref())
    }

    pub fn get_mut(&mut self, id: PanelId) -> Option<&mut dyn Panel> {
        self.panels.get_mut(&id).map(|p| p.as_mut())
    }

    /// Get all registered panel IDs
    pub fn panel_ids(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.panels.keys().copied()
    }
}
```

### Messages

```rust
/// Panel-specific messages (delegated to individual panels)
#[derive(Debug, Clone)]
pub enum PanelMsg {
    /// File explorer messages
    FileExplorer(FileExplorerMsg),
    /// Outline panel messages
    Outline(OutlineMsg),
    /// Terminal messages
    Terminal(TerminalMsg),
    /// Task runner messages
    TaskRunner(TaskRunnerMsg),
    // Add more as panels are implemented
}

/// Dock-level messages (panel switching, resizing, visibility)
#[derive(Debug, Clone)]
pub enum DockMsg {
    /// Focus-then-toggle: If dock not focused, focus and open panel.
    /// If dock already focused on this panel, close dock.
    /// This is the primary keybinding action (Cmd+1, Cmd+2, etc.)
    FocusOrTogglePanel(PanelId),

    /// Close the currently focused dock and return focus to editor
    CloseFocusedDock,

    /// Start resizing a dock
    StartResize {
        position: DockPosition,
        initial_coord: f64,
    },

    /// Update dock size during resize
    UpdateResize {
        position: DockPosition,
        coord: f64,
    },

    /// End dock resize
    EndResize(DockPosition),

    /// Cycle to next panel in focused dock
    NextPanelInDock,

    /// Cycle to previous panel in focused dock
    PrevPanelInDock,
}

// In messages.rs, add to top-level Msg enum:
pub enum Msg {
    // ... existing variants ...
    Panel(PanelMsg),
    Dock(DockMsg),
}

// In update/mod.rs, add routing:
Msg::Dock(m)  => dock::update_dock(model, m),
Msg::Panel(m) => panel::update_panel(model, m),
```

### Persistence Format

```rust
/// Serializable dock layout for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDockLayout {
    pub left: PersistedDock,
    pub right: PersistedDock,
    pub bottom: PersistedDock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDock {
    pub panel_ids: Vec<String>,  // Panel IDs as strings for forward compatibility
    pub active_panel: Option<String>,
    pub is_open: bool,
    pub size: f32,
}

// Saved to: ~/.config/token-editor/dock-layout.yaml
```

---

## Focus System Integration

### FocusTarget Evolution

The current `FocusTarget` enum must evolve to support docks:

```rust
// Current (src/model/ui.rs)
pub enum FocusTarget {
    Editor,
    Sidebar,  // ← Remove this
    Modal,
}

// New
pub enum FocusTarget {
    /// Main editor text area (default)
    Editor,
    /// A dock panel has focus (replaces Sidebar)
    Dock(DockPosition),
    /// Modal dialog has focus
    Modal,
}
```

### Single Source of Truth

**Critical**: `UiState::focus` is the **sole source of truth** for focus. The `DockLayout::focused_dock` field is **removed**—derive it from `UiState::focus` instead:

```rust
impl DockLayout {
    /// Get focused dock position from UiState (derived, not stored)
    pub fn focused_dock(ui: &UiState) -> Option<DockPosition> {
        match ui.focus {
            FocusTarget::Dock(pos) => Some(pos),
            _ => None,
        }
    }
}
```

### Focus Helper Methods

Update `UiState` methods:

```rust
impl UiState {
    /// Focus a specific dock
    pub fn focus_dock(&mut self, position: DockPosition) {
        self.focus = FocusTarget::Dock(position);
    }

    /// Check if any dock is focused
    pub fn is_dock_focused(&self) -> bool {
        matches!(self.focus, FocusTarget::Dock(_))
    }

    /// Get focused dock position, if any
    pub fn focused_dock(&self) -> Option<DockPosition> {
        match self.focus {
            FocusTarget::Dock(pos) => Some(pos),
            _ => None,
        }
    }
}
```

### HoverRegion Evolution

```rust
// Current
pub enum HoverRegion {
    None,
    Sidebar,        // ← Remove
    SidebarResize,  // ← Remove
    EditorText,
    EditorTabBar,
    StatusBar,
    Modal,
    Splitter,
}

// New
pub enum HoverRegion {
    None,
    /// Hovering over a dock's content area
    DockContent(DockPosition),
    /// Hovering over a dock's resize handle
    DockResize(DockPosition),
    /// Hovering over a dock's tab bar
    DockTabBar(DockPosition),
    EditorText,
    EditorTabBar,
    StatusBar,
    Modal,
    Splitter,
}
```

---

## Input Routing

### Focus Precedence

Input is captured in this order (first match wins):

| Priority | Condition | Handler | Notes |
|----------|-----------|---------|-------|
| 1 | Splitter drag active | `LayoutMsg::CancelSplitterDrag` | Escape cancels |
| 2 | Modal active | `handle_modal_key()` | Modal consumes all input |
| 3 | CSV edit mode | `handle_csv_edit_key()` | CSV cell editing |
| 4 | Dock focused | `handle_dock_key()` | **New**: dock/panel input |
| 5 | Editor (default) | `handle_editor_key()` | Normal editing |

### KeyContext Updates

```rust
// In src/runtime/app.rs
pub struct KeyContext {
    // Existing
    pub has_selection: bool,
    pub has_multiple_cursors: bool,
    pub modal_active: bool,
    pub editor_focused: bool,
    
    // Updated (replaces sidebar_focused)
    pub dock_focused: bool,
    
    // New optional (for panel-specific bindings)
    pub active_panel: Option<PanelId>,
}

impl App {
    fn get_key_context(&self) -> KeyContext {
        let focus = self.model.ui.focus;
        let dock_focused = matches!(focus, FocusTarget::Dock(_));
        
        let active_panel = if let FocusTarget::Dock(pos) = focus {
            self.model.dock_layout.dock(pos).active_panel()
        } else {
            None
        };

        KeyContext {
            has_selection: /* ... */,
            has_multiple_cursors: /* ... */,
            modal_active: self.model.ui.has_modal(),
            editor_focused: matches!(focus, FocusTarget::Editor),
            dock_focused,
            active_panel,
        }
    }
}
```

### Generic Panel Input Handler

Replace `handle_sidebar_key` with generic `handle_dock_key`:

```rust
// In src/runtime/input.rs

fn handle_dock_key(
    model: &mut AppModel,
    key: &Key,
    modifiers: Modifiers,
) -> Option<Cmd> {
    let Some(dock_pos) = model.ui.focused_dock() else {
        return None;
    };

    let dock = model.dock_layout.dock(dock_pos);
    let Some(panel_id) = dock.active_panel() else {
        return None;
    };

    // Escape always returns focus to editor (does NOT close dock)
    if matches!(key, Key::Named(NamedKey::Escape)) {
        model.ui.focus_editor();
        return Some(Cmd::Redraw);
    }

    // Delegate to panel-specific handler based on panel_id
    match panel_id {
        PanelId::FILE_EXPLORER => handle_file_explorer_key(model, key, modifiers),
        PanelId::TERMINAL => handle_terminal_key(model, key, modifiers),
        PanelId::OUTLINE => handle_outline_key(model, key, modifiers),
        // ... other panels
        _ => None,
    }
}
```

### Keymap Condition Updates

```yaml
# keymap.yaml conditions
when:
  - "dock_focused"        # Any dock has focus
  - "editor_focused"      # Editor has focus
  - "modal_active"        # Modal is open
  - "panel:file_explorer" # Specific panel focused (optional)
  - "panel:terminal"      # Specific panel focused (optional)
```

---

## Mouse Hit-Testing

### Dock Geometry

Compute dock rectangles during layout:

```rust
/// Computed layout rectangles for all docks
pub struct DockGeometry {
    pub left: Option<Rect>,      // None if dock closed
    pub right: Option<Rect>,
    pub bottom: Option<Rect>,
    pub editor_area: Rect,       // Remaining space for editor
}

impl DockGeometry {
    pub fn compute(
        window_size: (f32, f32),
        dock_layout: &DockLayout,
        status_bar_height: f32,
        scale_factor: f64,
    ) -> Self {
        let (w, h) = window_size;
        let content_height = h - status_bar_height;

        let left_width = if dock_layout.left.is_open {
            dock_layout.left.size(scale_factor)
        } else {
            0.0
        };

        let right_width = if dock_layout.right.is_open {
            dock_layout.right.size(scale_factor)
        } else {
            0.0
        };

        let bottom_height = if dock_layout.bottom.is_open {
            dock_layout.bottom.size(scale_factor)
        } else {
            0.0
        };

        let editor_height = content_height - bottom_height;

        Self {
            left: if left_width > 0.0 {
                Some(Rect::new(0.0, 0.0, left_width, editor_height))
            } else {
                None
            },
            right: if right_width > 0.0 {
                Some(Rect::new(w - right_width, 0.0, right_width, editor_height))
            } else {
                None
            },
            bottom: if bottom_height > 0.0 {
                Some(Rect::new(left_width, editor_height, w - left_width - right_width, bottom_height))
            } else {
                None
            },
            editor_area: Rect::new(left_width, 0.0, w - left_width - right_width, editor_height),
        }
    }
}
```

### Hit-Test Function

```rust
/// Result of dock hit-testing
pub enum DockHitArea {
    /// In dock content area
    Content(DockPosition),
    /// In dock tab bar
    TabBar(DockPosition, Option<PanelId>),
    /// On resize handle
    ResizeHandle(DockPosition),
    /// Not in any dock
    None,
}

pub fn hit_test_docks(
    x: f32,
    y: f32,
    geometry: &DockGeometry,
    dock_layout: &DockLayout,
    tab_bar_height: f32,
    resize_handle_width: f32,
) -> DockHitArea {
    // Check left dock
    if let Some(rect) = &geometry.left {
        if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
            // Check resize handle (right edge)
            if x >= rect.x + rect.width - resize_handle_width {
                return DockHitArea::ResizeHandle(DockPosition::Left);
            }
            // Check tab bar (top)
            if y < rect.y + tab_bar_height {
                let panel_id = /* compute from x position */;
                return DockHitArea::TabBar(DockPosition::Left, panel_id);
            }
            return DockHitArea::Content(DockPosition::Left);
        }
    }

    // Check right dock (similar)
    // Check bottom dock (resize handle on top edge)
    
    DockHitArea::None
}
```

### Mouse Event Routing

```rust
// In app.rs handle_mouse_event
match hit_test_docks(x, y, &geometry, &self.model.dock_layout, TAB_HEIGHT, RESIZE_WIDTH) {
    DockHitArea::ResizeHandle(pos) => {
        // Start resize drag
        self.update(Msg::Dock(DockMsg::StartResize { position: pos, initial_coord: x }));
    }
    DockHitArea::TabBar(pos, Some(panel_id)) => {
        // Click on panel tab → activate that panel
        self.update(Msg::Dock(DockMsg::FocusOrTogglePanel(panel_id)));
    }
    DockHitArea::Content(pos) => {
        // Focus the dock, delegate click to panel
        self.model.ui.focus_dock(pos);
        // Panel-specific click handling...
    }
    DockHitArea::None => {
        // Continue to editor hit-testing
    }
}
```

---

## Command Registration

### New CommandIds

```rust
// In src/commands.rs
pub enum CommandId {
    // ... existing ...

    // Panel toggle commands (FocusOrToggle behavior)
    ToggleFileExplorer,
    ToggleTerminal,
    ToggleTaskRunner,
    ToggleTodoList,
    ToggleAiChat,
    ToggleOutline,

    // Dock navigation
    CloseFocusedDock,
    NextPanelInDock,
    PrevPanelInDock,
}
```

### Command to Message Mapping

```rust
impl CommandId {
    pub fn to_msgs(&self) -> Vec<Msg> {
        match self {
            CommandId::ToggleFileExplorer => {
                vec![Msg::Dock(DockMsg::FocusOrTogglePanel(PanelId::FILE_EXPLORER))]
            }
            CommandId::ToggleTerminal => {
                vec![Msg::Dock(DockMsg::FocusOrTogglePanel(PanelId::TERMINAL))]
            }
            CommandId::ToggleOutline => {
                vec![Msg::Dock(DockMsg::FocusOrTogglePanel(PanelId::OUTLINE))]
            }
            CommandId::CloseFocusedDock => {
                vec![Msg::Dock(DockMsg::CloseFocusedDock)]
            }
            CommandId::NextPanelInDock => {
                vec![Msg::Dock(DockMsg::NextPanelInDock)]
            }
            CommandId::PrevPanelInDock => {
                vec![Msg::Dock(DockMsg::PrevPanelInDock)]
            }
            // ... etc
        }
    }
}
```

### Default Keymap Entries

```rust
// In keymap defaults
vec![
    KeyBinding::new("cmd+1", CommandId::ToggleFileExplorer, &[]),
    KeyBinding::new("cmd+2", CommandId::ToggleTerminal, &[]),
    KeyBinding::new("cmd+3", CommandId::ToggleTaskRunner, &[]),
    KeyBinding::new("cmd+4", CommandId::ToggleTodoList, &[]),
    KeyBinding::new("cmd+5", CommandId::ToggleAiChat, &[]),
    KeyBinding::new("cmd+7", CommandId::ToggleOutline, &[]),
    KeyBinding::new("escape", CommandId::CloseFocusedDock, &["dock_focused"]),
    KeyBinding::new("cmd+]", CommandId::NextPanelInDock, &["dock_focused"]),
    KeyBinding::new("cmd+[", CommandId::PrevPanelInDock, &["dock_focused"]),
]
```

---

## Keybindings

### Focus-Then-Toggle Behavior

The keybinding system uses a "focus-then-toggle" pattern inspired by IntelliJ:

1. **First press**: Focus the dock and open the panel (if not already focused)
2. **Second press** (dock already focused): Close/toggle the panel

This allows intuitive navigation: pressing `Cmd+1` always takes you to the file explorer. Press again to close it. If you're in the file explorer and press `Cmd+7`, it focuses the outline panel. Press `Cmd+7` again to close it.

```
State: Editor focused, left dock closed
  → Cmd+1 → Opens left dock, shows file explorer, focuses left dock

State: Left dock focused (file explorer)
  → Cmd+1 → Closes left dock, returns focus to editor

State: Left dock focused (file explorer)
  → Cmd+7 → Opens right dock, shows outline, focuses right dock

State: Right dock focused (outline)
  → Cmd+7 → Closes right dock, returns focus to editor
```

### Default Bindings (IntelliJ-style)

| Action             | Mac      | Windows/Linux | Behavior                               |
| ------------------ | -------- | ------------- | -------------------------------------- |
| File Explorer      | `Cmd+1`  | `Alt+1`       | Focus-then-toggle left dock            |
| Terminal           | `Cmd+2`  | `Alt+2`       | Focus-then-toggle bottom dock          |
| Task Runner        | `Cmd+3`  | `Alt+3`       | Focus-then-toggle bottom dock          |
| TODO List          | `Cmd+4`  | `Alt+4`       | Focus-then-toggle bottom dock          |
| AI Chat            | `Cmd+5`  | `Alt+5`       | Focus-then-toggle right dock           |
| Outline            | `Cmd+7`  | `Alt+7`       | Focus-then-toggle right dock           |
| Return to Editor   | `Escape` | `Escape`      | Close focused dock, focus editor       |
| Next Panel in Dock | `Cmd+]`  | `Ctrl+]`      | Cycle active panel (when dock focused) |
| Prev Panel in Dock | `Cmd+[`  | `Ctrl+[`      | Cycle active panel (when dock focused) |

### Keymap Configuration

```yaml
# ~/.config/token-editor/keymap.yaml

# IntelliJ-style panel shortcuts (focus-then-toggle)
- key: "cmd+1"
  command: FocusOrTogglePanel
  args: { panel: "file_explorer" }
  when: ["always"]

- key: "cmd+2"
  command: FocusOrTogglePanel
  args: { panel: "terminal" }
  when: ["always"]

- key: "cmd+3"
  command: FocusOrTogglePanel
  args: { panel: "task_runner" }
  when: ["always"]

- key: "cmd+4"
  command: FocusOrTogglePanel
  args: { panel: "todo_list" }
  when: ["always"]

- key: "cmd+5"
  command: FocusOrTogglePanel
  args: { panel: "ai_chat" }
  when: ["always"]

- key: "cmd+7"
  command: FocusOrTogglePanel
  args: { panel: "outline" }
  when: ["always"]

# Escape to close dock and return to editor
- key: "escape"
  command: CloseFocusedDock
  when: ["dock_focused"]

# Panel cycling within a dock
- key: "cmd+]"
  command: NextPanelInDock
  when: ["dock_focused"]

- key: "cmd+["
  command: PrevPanelInDock
  when: ["dock_focused"]
```

---

## Implementation Plan

### Phase 1: Core Abstraction

**Effort:** M (3-5 days)

- [ ] Create `src/panel/mod.rs` with `Panel` trait
- [ ] Create `src/panel/dock.rs` with `Dock` and `DockLayout` structs
- [ ] Create `src/panel/registry.rs` with `PanelRegistry`
- [ ] Add `DockMsg` and `PanelMsg` to `src/messages.rs`
- [ ] Add `DockLayout` to `AppModel`
- [ ] Implement basic dock update logic in `src/update/`
- [ ] Unit tests for dock state management

### Phase 2: File Explorer Migration

**Effort:** M (3-5 days)

- [ ] Create `src/panels/file_explorer.rs` implementing `Panel` trait
- [ ] Move sidebar rendering logic from `src/view/mod.rs`
- [ ] Move sidebar state from `Workspace` to `FileExplorerPanel`
- [ ] Migrate `WorkspaceMsg` → `PanelMsg::FileExplorer(FileExplorerMsg)`
- [ ] Create `src/view/dock_renderer.rs` for dock rendering
- [ ] Integrate dock rendering in main view
- [ ] Wire up existing keybindings to new system
- [ ] Verify file explorer works identically to before

### Phase 3: Multi-Dock Layout

**Effort:** S (1-2 days)

- [ ] Implement layout calculation for all three docks
- [ ] Handle dock resize for left/right (width) and bottom (height)
- [ ] Render resize handles on dock edges
- [ ] Update editor area rect calculation to account for open docks

### Phase 4: Panel Tabs & Switching

**Effort:** S (1-2 days)

- [ ] Render panel tabs in dock header
- [ ] Handle tab clicks to switch active panel
- [ ] Implement keyboard cycling (`Cmd+]`, `Cmd+[`)
- [ ] Highlight active tab, dim inactive

### Phase 5: Persistence

**Effort:** S (1-2 days)

- [ ] Create `src/panel/persistence.rs`
- [ ] Save dock layout on state changes
- [ ] Load dock layout on startup
- [ ] Handle missing/invalid panel IDs gracefully
- [ ] Add `dock-layout.yaml` to `config_paths.rs`

### Phase 6: Polish & Second Panel

**Effort:** M (3-5 days)

- [ ] Implement a second panel (outline or terminal) to validate abstraction
- [ ] Error handling for panel initialization
- [ ] Status bar toggle buttons (optional, Zed-style)
- [ ] Documentation update
- [ ] Integration tests

### Future Phases (Not This Iteration)

- [ ] Panel drag-and-drop between docks
- [ ] Activity bar with panel icons
- [ ] Panel-specific settings persistence
- [ ] Custom panel registration API for plugins

---

## Testing Strategy

### Unit Tests

```rust
// tests/panel/dock_tests.rs

fn make_ui_with_focus(focus: FocusTarget) -> UiState {
    let mut ui = UiState::default();
    ui.focus = focus;
    ui
}

#[test]
fn test_focus_or_toggle_opens_closed_dock() {
    let mut layout = DockLayout::default();
    layout.left.panel_ids = vec![PanelId::FILE_EXPLORER];
    layout.left.is_open = false;
    let ui = make_ui_with_focus(FocusTarget::Editor);

    let (opened, new_focus) = layout.focus_or_toggle_panel(PanelId::FILE_EXPLORER, &ui);

    assert!(opened);
    assert!(layout.left.is_open);
    assert_eq!(layout.left.active_panel(), Some(PanelId::FILE_EXPLORER));
    assert_eq!(new_focus, Some(DockPosition::Left));
}

#[test]
fn test_focus_or_toggle_closes_when_focused_on_same_panel() {
    let mut layout = DockLayout::default();
    layout.left.panel_ids = vec![PanelId::FILE_EXPLORER];
    layout.left.is_open = true;
    layout.left.active_index = Some(0);
    let ui = make_ui_with_focus(FocusTarget::Dock(DockPosition::Left));

    let (opened, new_focus) = layout.focus_or_toggle_panel(PanelId::FILE_EXPLORER, &ui);

    assert!(!opened);
    assert!(!layout.left.is_open);
    assert_eq!(new_focus, None); // Caller sets focus to Editor
}

#[test]
fn test_focus_or_toggle_switches_focus_between_docks() {
    let mut layout = DockLayout::default();
    layout.left.panel_ids = vec![PanelId::FILE_EXPLORER];
    layout.left.is_open = true;
    layout.left.active_index = Some(0);
    layout.right.panel_ids = vec![PanelId::OUTLINE];
    layout.right.is_open = false;
    let ui = make_ui_with_focus(FocusTarget::Dock(DockPosition::Left));

    // Press Cmd+7 while focused on left dock
    let (opened, new_focus) = layout.focus_or_toggle_panel(PanelId::OUTLINE, &ui);

    assert!(opened);
    assert!(layout.right.is_open);
    assert_eq!(layout.right.active_panel(), Some(PanelId::OUTLINE));
    assert_eq!(new_focus, Some(DockPosition::Right));
    // Left dock stays open (caller doesn't close it)
    assert!(layout.left.is_open);
}

#[test]
fn test_focus_or_toggle_switches_panel_in_same_dock() {
    let mut layout = DockLayout::default();
    layout.bottom.panel_ids = vec![PanelId::TERMINAL, PanelId::TASK_RUNNER];
    layout.bottom.is_open = true;
    layout.bottom.active_index = Some(0); // Terminal active
    let ui = make_ui_with_focus(FocusTarget::Dock(DockPosition::Bottom));

    // Press Cmd+3 (task runner) while focused on terminal
    let (opened, new_focus) = layout.focus_or_toggle_panel(PanelId::TASK_RUNNER, &ui);

    assert!(opened);
    assert!(layout.bottom.is_open);
    assert_eq!(layout.bottom.active_panel(), Some(PanelId::TASK_RUNNER));
    assert_eq!(new_focus, Some(DockPosition::Bottom));
}

#[test]
fn test_close_dock() {
    let mut layout = DockLayout::default();
    layout.left.panel_ids = vec![PanelId::FILE_EXPLORER];
    layout.left.is_open = true;

    layout.close_dock(DockPosition::Left);

    assert!(!layout.left.is_open);
}

#[test]
fn test_next_panel_cycles() {
    let mut layout = DockLayout::default();
    layout.bottom.panel_ids = vec![PanelId::TERMINAL, PanelId::TASK_RUNNER, PanelId::TODO_LIST];
    layout.bottom.active_index = Some(0);

    layout.next_panel_in_dock(DockPosition::Bottom);
    assert_eq!(layout.bottom.active_index, Some(1));

    layout.next_panel_in_dock(DockPosition::Bottom);
    assert_eq!(layout.bottom.active_index, Some(2));

    layout.next_panel_in_dock(DockPosition::Bottom);
    assert_eq!(layout.bottom.active_index, Some(0)); // Wraps around
}

#[test]
fn test_dock_size_scaling() {
    let mut dock = Dock::new(DockPosition::Left);
    dock.size_logical = 200.0;

    assert_eq!(dock.size(2.0), 400.0);

    dock.set_size(300.0, 2.0);
    assert_eq!(dock.size_logical, 150.0);
}
```

### Integration Tests

1. **Migration Parity**: File explorer behavior identical before/after migration
2. **Multi-Panel Dock**: Add two panels to left dock, verify tab switching
3. **Persistence Round-Trip**: Save layout, restart, verify restored correctly
4. **Resize Constraints**: Verify min/max size enforcement

### Manual Testing Checklist

- [ ] File explorer opens with `Cmd+B`
- [ ] File explorer closes with `Cmd+B` when active
- [ ] Dock resize works with drag handle
- [ ] Dock respects min/max size constraints
- [ ] Tab switching works with clicks
- [ ] Tab switching works with `Cmd+]` / `Cmd+[`
- [ ] `Escape` returns focus to editor
- [ ] Layout persists across restart
- [ ] Empty dock (no panels) handles gracefully
- [ ] Three docks can be open simultaneously
- [ ] Editor area shrinks correctly for open docks

---

## References

### Internal Docs

- [Workspace implementation](../../src/model/workspace.rs)
- [Current sidebar rendering](../../src/view/mod.rs#L663-L851)
- [Config paths](../../src/config_paths.rs)
- [ROADMAP](../ROADMAP.md)

### External Resources

- [Zed Panel Trait](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/dock.rs#L96-L128)
- [Zed Dock Implementation](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/dock.rs)
- [VS Code Workbench Layout](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/layout/browser/layoutService.ts)

---

## Appendix

### Design Decisions

| Decision                 | Options Considered                         | Chosen        | Rationale                                |
| ------------------------ | ------------------------------------------ | ------------- | ---------------------------------------- |
| Trait vs Enum for panels | Enum (closed set), Trait (open)            | Trait         | Extensibility for future plugins         |
| Panel state storage      | In Panel impl, In AppModel, Separate store | In Panel impl | Encapsulation, each panel owns its state |
| Dock positions           | 3 (L/R/B), 4 (+Top)                        | 3             | Simpler, matches Zed, top rarely needed  |
| Tab placement            | Top of dock, Bottom of dock                | Top           | Matches Zed, VS Code conventions         |
| Persistence format       | YAML, JSON, Binary                         | YAML          | Consistent with other config files       |

### Open Questions

1. Should panels support lazy initialization (create on first open)?
2. How to handle panel-to-panel communication (e.g., outline ↔ editor)?
3. Should we support pinned tabs that can't be closed?

---

## Sidebar Migration Checklist

Code paths to update/remove when migrating to the dock system:

### Model Layer

- [ ] `src/model/ui.rs`:
  - [ ] Update `FocusTarget` enum (remove `Sidebar`, add `Dock(DockPosition)`)
  - [ ] Update `HoverRegion` enum (remove `Sidebar`/`SidebarResize`, add dock variants)
  - [ ] Remove `SidebarResizeState` (replaced by dock resize state)
  - [ ] Add `focus_dock()`, `is_dock_focused()`, `focused_dock()` methods

- [ ] `src/model/workspace.rs`:
  - [ ] Remove `sidebar_visible` field
  - [ ] Remove `sidebar_width_logical` field  
  - [ ] Remove `sidebar_width()` and `set_sidebar_width()` methods
  - [ ] Keep file tree state (`file_tree`, `expanded_folders`, `selected_item`, `scroll_offset`)

- [ ] `src/model/mod.rs`:
  - [ ] Add `dock_layout: DockLayout` to `AppModel`
  - [ ] Update re-exports

### Messages

- [ ] `src/messages.rs`:
  - [ ] Add `PanelMsg` enum
  - [ ] Add `DockMsg` enum  
  - [ ] Add `Msg::Panel(PanelMsg)` and `Msg::Dock(DockMsg)` variants
  - [ ] Update `WorkspaceMsg`:
    - [ ] Remove `ToggleSidebar`
    - [ ] Remove `StartSidebarResize`, `UpdateSidebarResize`, `EndSidebarResize`
    - [ ] Keep file tree messages (`ToggleFolder`, `SelectItem`, `OpenFile`, etc.)

### Update Layer

- [ ] `src/update/mod.rs`:
  - [ ] Add `Msg::Dock` and `Msg::Panel` routing
  
- [ ] `src/update/workspace.rs`:
  - [ ] Remove sidebar toggle/resize handling
  - [ ] Keep file tree update logic

- [ ] Create `src/update/dock.rs`:
  - [ ] `update_dock()` function for `DockMsg` handling

- [ ] Create `src/update/panel.rs`:
  - [ ] `update_panel()` function for `PanelMsg` routing

### Input Layer

- [ ] `src/runtime/input.rs`:
  - [ ] Replace `is_sidebar_focused()` with `is_dock_focused()` 
  - [ ] Replace `handle_sidebar_key()` with `handle_dock_key()`
  - [ ] Update focus precedence order

- [ ] `src/runtime/app.rs`:
  - [ ] Update `get_key_context()`: replace `sidebar_focused` with `dock_focused`
  - [ ] Update mouse hit-testing to use `DockGeometry` and `hit_test_docks()`
  - [ ] Remove sidebar-specific mouse handling
  - [ ] Add dock resize drag handling

### View Layer

- [ ] `src/view/mod.rs`:
  - [ ] Remove `render_sidebar()` function
  - [ ] Add `render_docks()` call in main render path
  - [ ] Update editor area rect calculation to account for docks
  - [ ] Remove `SidebarRenderContext`

- [ ] Create `src/view/dock_renderer.rs`:
  - [ ] `render_docks()` function
  - [ ] Dock background, border, tab bar rendering
  - [ ] Panel content rendering via `Panel::render()`

- [ ] `src/view/geometry.rs`:
  - [ ] Add `DockGeometry` struct
  - [ ] Add `DockGeometry::compute()` method
  - [ ] Update `ViewportGeometry` if needed

### Commands

- [ ] `src/commands.rs`:
  - [ ] Add `CommandId` variants for panel toggles
  - [ ] Add dock navigation commands
  - [ ] Add `to_msgs()` mappings

### Keymap

- [ ] `src/keymap/mod.rs`:
  - [ ] Update `KeyContext` struct (remove `sidebar_focused`, add `dock_focused`)
  - [ ] Add `active_panel` field if needed

- [ ] `keymap.yaml` (defaults):
  - [ ] Add `Cmd+1` through `Cmd+7` bindings
  - [ ] Add `Escape` with `dock_focused` condition
  - [ ] Add `Cmd+]`/`Cmd+[` for panel cycling

### New Files to Create

- [ ] `src/panel/mod.rs` - Panel trait and exports
- [ ] `src/panel/dock.rs` - Dock and DockLayout structs
- [ ] `src/panel/registry.rs` - PanelRegistry
- [ ] `src/panel/persistence.rs` - Save/load dock layout
- [ ] `src/panels/mod.rs` - Panel implementations
- [ ] `src/panels/file_explorer.rs` - Migrated sidebar
- [ ] `src/update/dock.rs` - Dock update logic
- [ ] `src/update/panel.rs` - Panel update routing
- [ ] `src/view/dock_renderer.rs` - Dock rendering

### Config

- [ ] `src/config_paths.rs`:
  - [ ] Add `dock_layout_file()` function returning `~/.config/token-editor/dock-layout.yaml`

### Future: Panel Movement Between Docks

When implementing panel movement, add:

```rust
impl Panel {
    /// Move this panel to a new dock position
    fn move_to(&self, position: DockPosition) -> bool {
        self.valid_positions().contains(&position)
    }
}

// DockMsg variant:
MovePanel {
    panel_id: PanelId,
    to_position: DockPosition,
}
```

Context menu on panel tab: "Move to Left/Right/Bottom"

### Changelog

| Date       | Change                                      |
| ---------- | ------------------------------------------- |
| 2025-01-07 | Initial draft based on Zed/VS Code research |
