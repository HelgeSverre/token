# Context Menu (Right-Click Menu)

A context-sensitive popup menu system triggered by right-click, providing quick access to relevant actions based on the clicked region.

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** L (1-2 weeks)
> **Created:** 2026-01-07
> **Updated:** 2026-01-07
> **Milestone:** TBD

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Shortcut Hint Integration](#shortcut-hint-integration)
6. [Implementation Plan](#implementation-plan)
7. [Testing Strategy](#testing-strategy)
8. [References](#references)

---

## Overview

### Current State

No existing context menu implementation. Right-click events are currently unhandled.

### Goals

- **Goal 1:** Provide context-sensitive actions via right-click in any UI region (editor, sidebar, tab bar, status bar, etc.)
- **Goal 2:** Display keyboard shortcut hints for menu items that have bound commands
- **Goal 3:** Support full keyboard navigation within the menu (arrows, Enter, Escape)
- **Goal 4:** Create an extensible architecture that allows adding new regions and menu items easily
- **Goal 5:** Click-away-to-close behavior for intuitive UX

### Non-Goals

- **Nested submenus:** V1 uses flat menus only. Submenus may be added in a future iteration.
- **Searchable/filterable menus:** No type-to-filter within context menus (command palette serves this purpose).
- **Platform-native menus:** We render custom menus via softbuffer, not NSMenu/Win32 menus.
- **Hover-to-expand:** No submenu expansion on hover (since no submenus in V1).

---

## Architecture

### Integration Points

```
┌─────────────────┐     ┌─────────────────┐
│  Right-Click    │────►│    app.rs       │
│  (MouseButton)  │     │  hit-test region│
└─────────────────┘     └────────┬────────┘
                                 │
                                 ▼
┌─────────────────┐     ┌─────────────────┐
│ ContextMenuMsg  │◄────│ Build menu from │
│ ::Open(request) │     │ target context  │
└────────┬────────┘     └─────────────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│   update()      │────►│ ContextMenuState│
│                 │     │ stored in UiState│
└────────┬────────┘     └─────────────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│   Renderer      │────►│  Draw menu at   │
│                 │     │  anchor position│
└─────────────────┘     └─────────────────┘
```

### Module Structure

```
src/
├── context_menu/              # New module (create)
│   ├── mod.rs                 # Public exports
│   ├── types.rs               # ContextMenuState, MenuItem, MenuAction, etc.
│   ├── builders.rs            # Per-region menu builders
│   └── shortcut_hints.rs      # Keymap → shortcut string lookup
├── model/
│   └── ui.rs                  # Add context_menu: Option<ContextMenuState>
├── messages.rs                # Add ContextMenuMsg enum
├── update/
│   └── context_menu.rs        # New update handler (create)
├── runtime/
│   └── app.rs                 # Right-click handling, keyboard interception
└── view/
    ├── mod.rs                 # Menu rendering
    └── geometry.rs            # Menu hit-testing
```

### Message Flow

**Opening a menu:**
1. User right-clicks anywhere in the window
2. `app.rs` receives `WindowEvent::MouseInput { button: MouseButton::Right, state: Pressed }`
3. `app.rs` hit-tests the click position to determine `ContextMenuTarget`
4. Dispatches `Msg::Ui(UiMsg::ContextMenu(ContextMenuMsg::Open(request)))`
5. `update()` builds menu items based on target, stores `ContextMenuState` in `UiState`
6. Renderer draws menu at anchor position

**Navigating the menu:**
1. Arrow keys / Enter / Escape intercepted in `app.rs` when `ui.context_menu.is_some()`
2. Dispatch `ContextMenuMsg::{MoveUp, MoveDown, Confirm, Cancel}`
3. `update()` modifies `active_index` or executes action and closes menu

**Activating an item:**
1. User clicks menu item or presses Enter
2. Dispatch `ContextMenuMsg::ActivateItem { index }`
3. `update()` executes the item's `MenuAction` (which produces `Msg`s)
4. Menu closes

**Closing the menu:**
1. User clicks outside menu, presses Escape, or activates an item
2. `model.ui.context_menu = None`

---

## Data Structures

### ContextMenuRegion

```rust
/// Which UI region spawned the menu (for debugging and potential special handling)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuRegion {
    Editor,
    Sidebar,
    EditorTabBar,
    StatusBar,
    Modal,
    Splitter,
}
```

### ContextMenuTarget

```rust
use std::path::PathBuf;
use crate::model::{GroupId, TabId};
use crate::model::editor::Position;

/// Detailed context about what was right-clicked
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    /// Right-click in editor text area
    Editor {
        group_id: GroupId,
        cursor_position: Position,
        has_selection: bool,
        file_path: Option<PathBuf>,
    },
    /// Right-click on a file/folder in sidebar
    SidebarItem {
        path: PathBuf,
        is_dir: bool,
    },
    /// Right-click on empty space in sidebar
    SidebarEmpty,
    /// Right-click on a tab
    Tab {
        group_id: GroupId,
        tab_id: TabId,
        is_dirty: bool,
    },
    /// Right-click on status bar
    StatusBar,
    /// Right-click on modal (future use)
    Modal,
    /// Right-click on splitter (future use)
    Splitter,
}
```

### ContextMenuRequest

```rust
/// Request to open a context menu
#[derive(Debug, Clone)]
pub struct ContextMenuRequest {
    /// Screen position where menu should appear (logical pixels)
    pub screen_pos: (f32, f32),
    /// What was clicked
    pub target: ContextMenuTarget,
}
```

### MenuAction

```rust
/// What happens when a menu item is activated
#[derive(Debug, Clone)]
pub enum MenuAction {
    /// Execute one or more messages
    Messages(Vec<crate::messages::Msg>),
    /// No action (used for disabled items)
    None,
}

impl MenuAction {
    /// Create action from a Command (most common case)
    pub fn from_command(cmd: crate::keymap::Command) -> Self {
        MenuAction::Messages(cmd.to_msgs())
    }
}
```

### MenuItem

```rust
/// A single item in a context menu
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Display label
    pub label: String,
    /// Whether the item can be activated
    pub enabled: bool,
    /// Optional keyboard shortcut hint (e.g., "⌘C" or "Ctrl+C")
    pub shortcut_hint: Option<String>,
    /// What happens when activated
    pub action: MenuAction,
    /// If true, renders as a visual separator line instead of a clickable item
    pub is_separator: bool,
}

impl MenuItem {
    /// Create a regular menu item from a Command
    pub fn from_command(label: impl Into<String>, cmd: Command, enabled: bool) -> Self {
        Self {
            label: label.into(),
            enabled,
            shortcut_hint: None, // Populated by shortcut hint system
            action: MenuAction::from_command(cmd),
            is_separator: false,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            enabled: false,
            shortcut_hint: None,
            action: MenuAction::None,
            is_separator: true,
        }
    }

    /// Create an item with custom messages (not tied to a Command)
    pub fn custom(label: impl Into<String>, msgs: Vec<Msg>, enabled: bool) -> Self {
        Self {
            label: label.into(),
            enabled,
            shortcut_hint: None,
            action: MenuAction::Messages(msgs),
            is_separator: false,
        }
    }
}
```

### ContextMenuState

```rust
/// State for an open context menu
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    /// Which region spawned this menu
    pub region: ContextMenuRegion,
    /// Screen-space anchor position (top-left of menu, logical pixels)
    pub anchor: (f32, f32),
    /// Menu items
    pub items: Vec<MenuItem>,
    /// Currently selected item index (for keyboard navigation)
    /// None = no selection yet (mouse-only interaction)
    pub active_index: Option<usize>,
}

impl ContextMenuState {
    /// Get the currently selected item, if any
    pub fn active_item(&self) -> Option<&MenuItem> {
        self.active_index.and_then(|i| self.items.get(i))
    }

    /// Move selection up, skipping separators and disabled items
    pub fn move_up(&mut self) {
        let len = self.items.len();
        if len == 0 { return; }

        let start = self.active_index.unwrap_or(0);
        let mut idx = start;

        loop {
            idx = if idx == 0 { len - 1 } else { idx - 1 };
            if self.items[idx].enabled && !self.items[idx].is_separator {
                self.active_index = Some(idx);
                return;
            }
            if idx == start { return; } // No valid items
        }
    }

    /// Move selection down, skipping separators and disabled items
    pub fn move_down(&mut self) {
        let len = self.items.len();
        if len == 0 { return; }

        let start = self.active_index.unwrap_or(len - 1);
        let mut idx = start;

        loop {
            idx = (idx + 1) % len;
            if self.items[idx].enabled && !self.items[idx].is_separator {
                self.active_index = Some(idx);
                return;
            }
            if idx == start { return; } // No valid items
        }
    }

    /// Select first valid item (for initial keyboard activation)
    pub fn select_first(&mut self) {
        for (i, item) in self.items.iter().enumerate() {
            if item.enabled && !item.is_separator {
                self.active_index = Some(i);
                return;
            }
        }
    }
}
```

### ContextMenuMsg

```rust
/// Messages for context menu interactions
/// 
/// Note: There is no `Open` variant. Menu building happens directly in `App`
/// (which owns the `Keymap` needed for shortcut hints) and stores the result
/// in `model.ui.context_menu`.
#[derive(Debug, Clone)]
pub enum ContextMenuMsg {
    /// Close the context menu without action
    Close,

    // Keyboard navigation
    /// Move selection up
    MoveUp,
    /// Move selection down
    MoveDown,
    /// Activate selected item (Enter)
    Confirm,
    /// Close menu (Escape)
    Cancel,

    // Mouse interaction
    /// Mouse hovering over item at index
    HoverItem { index: usize },
    /// Mouse clicked item at index
    ActivateItem { index: usize },
}
```

### UiMsg Extension

```rust
// In messages.rs, extend UiMsg:
#[derive(Debug, Clone)]
pub enum UiMsg {
    // ... existing variants ...

    /// Context menu messages
    ContextMenu(ContextMenuMsg),
}
```

### UiState Extension

```rust
// In model/ui.rs, add to UiState:
pub struct UiState {
    // ... existing fields ...

    /// Currently open context menu (if any)
    pub context_menu: Option<ContextMenuState>,
}

impl UiState {
    pub fn has_context_menu(&self) -> bool {
        self.context_menu.is_some()
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }
}
```

### KeyContext Extension

```rust
// In keymap/context.rs, add:
pub struct KeyContext {
    // ... existing fields ...

    /// Whether a context menu is open
    pub context_menu_active: bool,
}

// Add condition:
pub enum Condition {
    // ... existing variants ...

    /// Binding only active when context menu is open
    ContextMenuActive,
    /// Binding only active when no context menu is open
    ContextMenuInactive,
}
```

---

## Keybindings

### Menu Navigation Keys

When a context menu is open, these keys are intercepted before the normal keymap:

| Key | Action | Notes |
|-----|--------|-------|
| `↑` | Move selection up | Wraps around, skips separators |
| `↓` | Move selection down | Wraps around, skips separators |
| `Enter` | Activate selected item | Closes menu after action |
| `Space` | Activate selected item | Alternative to Enter |
| `Escape` | Close menu | No action taken |

### Keyboard Interception in app.rs

```rust
// In handle_event, before normal key handling:
if self.model.ui.has_context_menu() {
    if let WindowEvent::KeyboardInput { event, .. } = &event {
        if event.state == ElementState::Pressed {
            use winit::keyboard::KeyCode;
            if let PhysicalKey::Code(code) = event.physical_key {
                let cm_msg = match code {
                    KeyCode::ArrowUp => Some(ContextMenuMsg::MoveUp),
                    KeyCode::ArrowDown => Some(ContextMenuMsg::MoveDown),
                    KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                        Some(ContextMenuMsg::Confirm)
                    }
                    KeyCode::Escape => Some(ContextMenuMsg::Cancel),
                    _ => None,
                };

                if let Some(msg) = cm_msg {
                    return update(&mut self.model, Msg::Ui(UiMsg::ContextMenu(msg)));
                }
            }
        }
    }
}
```

---

## Shortcut Hint Integration

### Problem

Menu items should display their keyboard shortcuts (e.g., "Copy  ⌘C"). These shortcuts are defined in the keymap system, so menu builders need access to lookup shortcut strings for `Command`s.

### Solution: ShortcutHintProvider

Create a lookup system that queries the active `Keymap` for the primary binding of a `Command`:

```rust
// In context_menu/shortcut_hints.rs

use crate::keymap::{Command, Keymap, Keystroke};

/// Provides shortcut hint strings for Commands
pub struct ShortcutHintProvider<'a> {
    keymap: &'a Keymap,
}

impl<'a> ShortcutHintProvider<'a> {
    pub fn new(keymap: &'a Keymap) -> Self {
        Self { keymap }
    }

    /// Get the shortcut hint string for a command, if bound
    ///
    /// Returns platform-appropriate string like "⌘C" (macOS) or "Ctrl+C" (other)
    pub fn hint_for(&self, command: Command) -> Option<String> {
        // Find first binding for this command (ignoring context conditions)
        self.keymap
            .bindings()
            .iter()
            .find(|b| b.command == command)
            .map(|b| keystroke_to_hint(&b.keystrokes[0]))
    }
}

/// Convert a Keystroke to a display string
fn keystroke_to_hint(ks: &Keystroke) -> String {
    let mut parts = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if ks.modifiers.ctrl() { parts.push("⌃"); }
        if ks.modifiers.alt() { parts.push("⌥"); }
        if ks.modifiers.shift() { parts.push("⇧"); }
        if ks.modifiers.cmd() { parts.push("⌘"); }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if ks.modifiers.ctrl() { parts.push("Ctrl+"); }
        if ks.modifiers.alt() { parts.push("Alt+"); }
        if ks.modifiers.shift() { parts.push("Shift+"); }
        if ks.modifiers.cmd() { parts.push("Ctrl+"); } // Cmd maps to Ctrl on non-Mac
    }

    parts.push(&key_to_string(ks.key));
    parts.join("")
}

fn key_to_string(key: KeyCode) -> String {
    match key {
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        KeyCode::Enter => "↵".to_string(),
        KeyCode::Tab => "⇥".to_string(),
        KeyCode::Backspace => "⌫".to_string(),
        KeyCode::Delete => "⌦".to_string(),
        KeyCode::Escape => "Esc".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        // ... other keys ...
        _ => format!("{:?}", key),
    }
}
```

### Menu Builder Integration

Menu builders receive the `ShortcutHintProvider` and use it to populate hints:

```rust
// In context_menu/builders.rs

pub fn build_editor_menu(
    model: &AppModel,
    target: &EditorTarget,
    hints: &ShortcutHintProvider,
) -> Vec<MenuItem> {
    use Command::*;

    let has_selection = target.has_selection;

    vec![
        item("Undo", Undo, true, hints),
        item("Redo", Redo, true, hints),
        MenuItem::separator(),
        item("Cut", Cut, has_selection, hints),
        item("Copy", Copy, has_selection, hints),
        item("Paste", Paste, true, hints),
        MenuItem::separator(),
        item("Select All", SelectAll, true, hints),
    ]
}

fn item(label: &str, cmd: Command, enabled: bool, hints: &ShortcutHintProvider) -> MenuItem {
    MenuItem {
        label: label.to_string(),
        enabled,
        shortcut_hint: hints.hint_for(cmd),
        action: MenuAction::from_command(cmd),
        is_separator: false,
    }
}
```

### Keymap Access

The `App` struct already holds the `Keymap`. When building a menu:

```rust
// In update handler or App method:
fn build_context_menu(&self, request: &ContextMenuRequest) -> ContextMenuState {
    let hints = ShortcutHintProvider::new(&self.keymap);
    let items = match &request.target {
        ContextMenuTarget::Editor { .. } => build_editor_menu(&self.model, target, &hints),
        // ... other regions ...
    };
    // ...
}
```

**Note:** If `update()` doesn't have access to `Keymap`, the hint provider can be passed via the message or the menu building can happen in `App::process_cmd()` instead.

---

## Focus & Input Routing

### Modal Interaction

**Critical rule:** Context menus should NOT open while a modal is active.

```rust
// In app.rs right-click handler:
if self.model.ui.has_modal() {
    return None; // Ignore right-clicks when modal is open
}
```

Modals are full-screen overlay captures. Opening a context menu on top would create confusing UX and z-order issues.

### Click-Away Behavior

When a context menu is open, left-clicks are handled specially:

```rust
// In app.rs MouseButton::Left handler, at the TOP before other hit-testing:
if self.model.ui.has_context_menu() {
    let menu_rect = compute_menu_rect(&self.model.ui.context_menu.as_ref().unwrap(), &self.model.metrics);
    
    if is_point_in_rect(x, y, &menu_rect) {
        // Click inside menu → hit-test which item
        if let Some(index) = hit_test_menu_item(&self.model, x, y) {
            self.update(Msg::Ui(UiMsg::ContextMenu(ContextMenuMsg::ActivateItem { index })));
        }
    } else {
        // Click outside menu → close menu, CONSUME the click (don't pass through)
        self.model.ui.close_context_menu();
    }
    return Some(Cmd::Redraw);
}
// ... normal left-click handling continues only if no context menu was open
```

**Decision:** Click-away **consumes** the click. It does not also trigger actions in the underlying UI (sidebar selection, editor click, etc.). This matches modal behavior and standard platform conventions.

### Keyboard Interception Order

When context menu is open, keyboard events are captured **before** keymap lookup:

```
Keyboard Event Flow (with context menu):

1. WindowEvent::KeyboardInput received
2. IF context_menu.is_some():
   a. Arrow/Enter/Escape → dispatch ContextMenuMsg → return
   b. Other keys → close menu, DO NOT process further (consume)
3. ELSE: normal keymap lookup and fallback handling
```

**Decision:** While context menu is open, **only** navigation keys (↑↓↵⎋) are processed. All other keys close the menu and are consumed (not passed to editor/sidebar).

```rust
// In app.rs key handling, before keymap lookup:
if self.model.ui.has_context_menu() {
    if let WindowEvent::KeyboardInput { event, .. } = &event {
        if event.state == ElementState::Pressed {
            let cm_msg = match event.physical_key {
                PhysicalKey::Code(KeyCode::ArrowUp) => Some(ContextMenuMsg::MoveUp),
                PhysicalKey::Code(KeyCode::ArrowDown) => Some(ContextMenuMsg::MoveDown),
                PhysicalKey::Code(KeyCode::Enter | KeyCode::Space) => Some(ContextMenuMsg::Confirm),
                PhysicalKey::Code(KeyCode::Escape) => Some(ContextMenuMsg::Cancel),
                _ => {
                    // Any other key: close menu and consume
                    self.model.ui.close_context_menu();
                    return Some(Cmd::Redraw);
                }
            };
            if let Some(msg) = cm_msg {
                return self.update(Msg::Ui(UiMsg::ContextMenu(msg)));
            }
        }
    }
    return None; // Consume all other events while menu open
}
```

### Focus Model

Context menus do **not** change `FocusTarget`. The underlying region (Editor/Sidebar/Dock) retains focus:

- `FocusTarget` stays as-is (no `FocusTarget::ContextMenu` variant)
- `KeyContext::context_menu_active` is used to suppress normal keybindings
- Keyboard capture happens imperatively in `App`, not via keymap conditions

This keeps the architecture simple and avoids focus restoration complexity.

---

## Coordinate System & Damage

### Coordinate System

**All coordinates use physical pixels** (scaled for DPI), consistent with the rest of the codebase.

Update `ContextMenuRequest` and `ContextMenuState`:

```rust
pub struct ContextMenuRequest {
    /// Screen position where menu should appear (physical pixels)
    pub screen_pos: (f32, f32),
    /// What was clicked
    pub target: ContextMenuTarget,
}

pub struct ContextMenuState {
    /// Which region spawned this menu
    pub region: ContextMenuRegion,
    /// Screen-space anchor position (top-left of menu, physical pixels)
    pub anchor: (f32, f32),
    // ...
}
```

Mouse coordinates from winit are already in physical pixels. Sidebar width, row heights, etc. are also in physical pixels (scaled by `metrics.scale_factor`).

### Damage Handling

For V1, context menus force full redraws, same as modals:

```rust
// In Renderer::compute_effective_damage
fn compute_effective_damage(&self, model: &AppModel, requested: Damage) -> Damage {
    // Force full redraw for overlays
    if model.ui.has_modal() 
        || model.ui.has_context_menu()  // ← Add this
        || model.file_drop.is_some() 
    {
        return Damage::Full;
    }
    // ... rest of damage logic
}
```

This is simple and correct. Partial redraw optimization for menus can be added later if needed.

### Z-Order

Rendering order (back to front):

1. Editor area (sidebar, editor groups, tabs, splitters)
2. Status bar
3. Context menu (if open)
4. Modal (if open) — modals render on top of everything
5. Debug overlays (dev builds)

Since modals block context menus from opening, they won't overlap in practice.

---

## Menu Building Location

### Decision: Build Menus in App, Not Update

The `Keymap` is owned by `App`, not passed to `update()`. Building menus requires keymap access for shortcut hints.

**Approach:** Build `ContextMenuState` directly in `App` when handling right-click, then store it in `model.ui`:

```rust
// In app.rs right-click handler:
fn handle_right_click(&mut self, x: f32, y: f32) -> Option<Cmd> {
    // 1. Don't open if modal is active
    if self.model.ui.has_modal() {
        return None;
    }

    // 2. Close existing menu if any
    self.model.ui.close_context_menu();

    // 3. Hit-test to determine target
    let target = self.hit_test_context_menu_target(x, y)?;

    // 4. Build menu with shortcut hints (requires self.keymap)
    let hints = ShortcutHintProvider::new(&self.keymap);
    let items = match &target {
        ContextMenuTarget::Editor { has_selection, .. } => {
            build_editor_menu(&self.model, *has_selection, &hints)
        }
        ContextMenuTarget::SidebarItem { path, is_dir } => {
            build_sidebar_item_menu(path, *is_dir, &hints)
        }
        ContextMenuTarget::Tab { is_dirty, .. } => {
            build_tab_menu(*is_dirty, &hints)
        }
        // ... other targets
        _ => return None,
    };

    if items.is_empty() {
        return None;
    }

    // 5. Create and store menu state
    let region = target.to_region();
    self.model.ui.context_menu = Some(ContextMenuState {
        region,
        anchor: (x, y),
        items,
        active_index: None,
    });

    Some(Cmd::Redraw)
}
```

### Message Flow Simplification

With this approach, `ContextMenuMsg::Open` is no longer needed. Messages are only for user interactions:

```rust
pub enum ContextMenuMsg {
    /// Close the context menu without action
    Close,
    
    // Keyboard navigation
    MoveUp,
    MoveDown,
    Confirm,
    Cancel,
    
    // Mouse interaction
    HoverItem { index: usize },
    ActivateItem { index: usize },
}
```

The `Open` variant can be removed since menu building happens directly in `App`.

---

## Menu Action Execution

### Multi-Message Aggregation

When executing `MenuAction::Messages(Vec<Msg>)`, properly aggregate commands:

```rust
// In update/context_menu.rs or App
fn execute_menu_action(model: &mut AppModel, action: MenuAction) -> Option<Cmd> {
    match action {
        MenuAction::Messages(msgs) => {
            let mut cmds: Vec<Cmd> = Vec::new();
            for msg in msgs {
                if let Some(cmd) = update(model, msg) {
                    cmds.push(cmd);
                }
            }
            // Close menu after action
            model.ui.close_context_menu();
            
            // Aggregate commands
            match cmds.len() {
                0 => Some(Cmd::Redraw),
                1 => Some(cmds.pop().unwrap()),
                _ => Some(Cmd::Batch(cmds)),
            }
        }
        MenuAction::None => {
            // Disabled item or separator - do nothing
            None
        }
    }
}
```

### Confirm Handler

```rust
// In update_context_menu
ContextMenuMsg::Confirm => {
    if let Some(menu) = &model.ui.context_menu {
        if let Some(item) = menu.active_item() {
            if item.enabled && !item.is_separator {
                let action = item.action.clone();
                return execute_menu_action(model, action);
            }
        }
    }
    None
}
```

---

## Tab Hit-Testing

### Gap: Need to Know Which Tab Was Clicked

Current code has `is_in_group_tab_bar(x, y, group)` which returns `bool`, but for context menus we need to know **which specific tab** was right-clicked.

### Solution: Extract Tab Hit-Test Helper

```rust
// In view/geometry.rs

/// Result of tab bar hit-testing
pub struct TabHitResult {
    pub group_id: GroupId,
    pub tab_id: TabId,
    pub tab_index: usize,
}

/// Hit-test which tab (if any) is at the given position
pub fn hit_test_tab(
    x: f32,
    y: f32,
    model: &AppModel,
) -> Option<TabHitResult> {
    for group in model.editor_area.all_groups() {
        let layout = GroupLayout::new(group, model, model.metrics.char_width);
        
        // Check if in tab bar area
        if y < layout.rect_y() as f32 || y >= layout.rect_y() as f32 + layout.tab_bar_height as f32 {
            continue;
        }
        if x < layout.rect_x() as f32 || x >= layout.rect_x() as f32 + layout.rect_w() as f32 {
            continue;
        }
        
        // Find which tab
        let tab_x = x - layout.rect_x() as f32;
        let mut current_x = 0.0;
        
        for (idx, tab) in group.tabs.iter().enumerate() {
            let tab_width = compute_tab_width(tab, &model.metrics);
            if tab_x >= current_x && tab_x < current_x + tab_width {
                return Some(TabHitResult {
                    group_id: group.id,
                    tab_id: tab.id,
                    tab_index: idx,
                });
            }
            current_x += tab_width;
        }
    }
    None
}
```

This helper is used by both left-click (existing tab switching) and right-click (context menu).

---

## Keymap API Extension

### Need: Bindings-for-Command Lookup

The `Keymap` needs a method to find bindings for a given command:

```rust
// In keymap/mod.rs
impl Keymap {
    /// Find all bindings for a command
    pub fn bindings_for_command(&self, cmd: Command) -> impl Iterator<Item = &Keybinding> {
        self.bindings.iter().filter(move |b| b.command == cmd)
    }
}
```

### Keystroke Formatting

Centralize keystroke-to-string formatting (currently only exists as hardcoded strings in `commands::COMMANDS`):

```rust
// In keymap/format.rs (new file)

/// Format a keystroke for display
pub fn format_keystroke(ks: &Keystroke) -> String {
    let mut parts = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Mac uses symbols, no separators
        if ks.modifiers.ctrl() { parts.push("⌃".to_string()); }
        if ks.modifiers.alt() { parts.push("⌥".to_string()); }
        if ks.modifiers.shift() { parts.push("⇧".to_string()); }
        if ks.modifiers.cmd() { parts.push("⌘".to_string()); }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows/Linux uses "Ctrl+Alt+..." style
        if ks.modifiers.ctrl() || ks.modifiers.cmd() { parts.push("Ctrl".to_string()); }
        if ks.modifiers.alt() { parts.push("Alt".to_string()); }
        if ks.modifiers.shift() { parts.push("Shift".to_string()); }
    }

    parts.push(format_key(&ks.key));

    #[cfg(target_os = "macos")]
    { parts.join("") }

    #[cfg(not(target_os = "macos"))]
    { parts.join("+") }
}

fn format_key(key: &Key) -> String {
    match key {
        Key::Character(c) => c.to_uppercase(),
        Key::Named(NamedKey::Enter) => "↵".to_string(),
        Key::Named(NamedKey::Tab) => "⇥".to_string(),
        Key::Named(NamedKey::Backspace) => "⌫".to_string(),
        Key::Named(NamedKey::Delete) => "⌦".to_string(),
        Key::Named(NamedKey::Escape) => "Esc".to_string(),
        Key::Named(NamedKey::ArrowUp) => "↑".to_string(),
        Key::Named(NamedKey::ArrowDown) => "↓".to_string(),
        Key::Named(NamedKey::ArrowLeft) => "←".to_string(),
        Key::Named(NamedKey::ArrowRight) => "→".to_string(),
        Key::Named(NamedKey::Space) => "Space".to_string(),
        Key::Named(named) => format!("{:?}", named),
        _ => "?".to_string(),
    }
}
```

### Limitations (Documented)

For V1:
- **First binding only**: If multiple bindings exist for a command, only the first is shown
- **No chords**: Multi-keystroke sequences are not displayed (show first keystroke only)
- **Context-agnostic**: Bindings with conditions are shown regardless of current context

---

## Implementation Plan

### Phase 1: Core Infrastructure

**Effort:** M (3-5 days)

- [ ] Create `src/context_menu/mod.rs` module structure
- [ ] Define core types: `ContextMenuState`, `MenuItem`, `MenuAction`, `ContextMenuTarget`, `ContextMenuRequest`
- [ ] Add `ContextMenuMsg` to `messages.rs`
- [ ] Extend `UiMsg` with `ContextMenu(ContextMenuMsg)` variant
- [ ] Add `context_menu: Option<ContextMenuState>` to `UiState`
- [ ] Add `context_menu_active` to `KeyContext` and `Condition::ContextMenuActive/Inactive`
- [ ] Implement `update_context_menu()` handler in `update/context_menu.rs`
- [ ] Add right-click handling in `app.rs` with region hit-testing
- [ ] Implement keyboard interception for menu navigation

### Phase 2: Rendering & Hit Testing

**Effort:** M (3-5 days)

- [ ] Implement menu rendering in `view/mod.rs`:
  - Draw background panel with border/shadow
  - Render menu items with labels
  - Highlight active/hovered item
  - Render shortcut hints right-aligned
  - Render separators as horizontal lines
- [ ] Implement menu geometry helpers in `view/geometry.rs`:
  - `compute_menu_rect(anchor, items, metrics) -> Rect`
  - `hit_test_menu_item(menu_state, x, y, metrics) -> Option<usize>`
  - Clamp menu position to window bounds
- [ ] Wire up mouse hover → `ContextMenuMsg::HoverItem`
- [ ] Wire up mouse click → `ContextMenuMsg::ActivateItem` or `Close`

### Phase 3: Menu Builders & Shortcut Hints

**Effort:** M (3-5 days)

- [ ] Implement `ShortcutHintProvider` in `context_menu/shortcut_hints.rs`
- [ ] Create `context_menu/builders.rs` with per-region builders:
  - [ ] `build_editor_menu()` - Undo, Redo, Cut, Copy, Paste, Select All
  - [ ] `build_sidebar_item_menu()` - Open, Reveal in Finder (placeholder for file ops)
  - [ ] `build_sidebar_empty_menu()` - New File, New Folder (placeholder)
  - [ ] `build_tab_menu()` - Close, Close Others, Close All (placeholder)
  - [ ] `build_status_bar_menu()` - minimal or empty
- [ ] Pass keymap reference to menu building for shortcut lookup
- [ ] Unit tests for menu builders

### Phase 4: Polish & Edge Cases

**Effort:** S (1-2 days)

- [ ] Handle menu near window edges (reposition to stay visible)
- [ ] Close menu on window focus loss
- [ ] Close menu when modal opens
- [ ] Add Damage tracking for efficient redraws
- [ ] Integration tests
- [ ] Update CHANGELOG.md

### Phase 5: Future - File Operations (Deferred)

Items explicitly deferred to future iterations:

- [ ] Sidebar: New File in folder
- [ ] Sidebar: New Folder
- [ ] Sidebar: Rename file/folder
- [ ] Sidebar: Delete file/folder
- [ ] Sidebar: Copy path / Copy relative path
- [ ] Tab: Close Others to the Right
- [ ] Tab: Close Saved
- [ ] Tab: Move to new group/split
- [ ] Editor: Format Selection
- [ ] Editor: Toggle Comment

---

## Testing Strategy

### Unit Tests

```rust
// tests/context_menu.rs

#[test]
fn test_menu_move_down_skips_separators() {
    let mut state = ContextMenuState {
        region: ContextMenuRegion::Editor,
        anchor: (100.0, 100.0),
        items: vec![
            MenuItem::from_command("Cut", Command::Cut, true),
            MenuItem::separator(),
            MenuItem::from_command("Copy", Command::Copy, true),
        ],
        active_index: Some(0),
    };

    state.move_down();
    assert_eq!(state.active_index, Some(2)); // Skipped separator
}

#[test]
fn test_menu_move_up_wraps() {
    let mut state = ContextMenuState {
        region: ContextMenuRegion::Editor,
        anchor: (100.0, 100.0),
        items: vec![
            MenuItem::from_command("Cut", Command::Cut, true),
            MenuItem::from_command("Copy", Command::Copy, true),
        ],
        active_index: Some(0),
    };

    state.move_up();
    assert_eq!(state.active_index, Some(1)); // Wrapped to end
}

#[test]
fn test_menu_move_skips_disabled() {
    let mut state = ContextMenuState {
        region: ContextMenuRegion::Editor,
        anchor: (100.0, 100.0),
        items: vec![
            MenuItem::from_command("Cut", Command::Cut, false), // disabled
            MenuItem::from_command("Copy", Command::Copy, true),
        ],
        active_index: None,
    };

    state.select_first();
    assert_eq!(state.active_index, Some(1)); // Skipped disabled
}

#[test]
fn test_shortcut_hint_provider() {
    let keymap = Keymap::with_bindings(vec![
        Keybinding::new(
            Keystroke::new(KeyCode::Char('c'), Modifiers::cmd()),
            Command::Copy,
        ),
    ]);
    let hints = ShortcutHintProvider::new(&keymap);

    #[cfg(target_os = "macos")]
    assert_eq!(hints.hint_for(Command::Copy), Some("⌘C".to_string()));

    assert_eq!(hints.hint_for(Command::Paste), None); // Not bound
}
```

### Integration Tests

1. **Right-click in editor opens menu:** Simulate right-click at editor coordinates, verify `ContextMenuState` is populated with editor menu items.

2. **Keyboard navigation:** Open menu → press Down → verify active_index changes → press Enter → verify action executed and menu closed.

3. **Click outside closes menu:** Open menu → click outside menu bounds → verify menu closed, no action taken.

4. **Shortcut hints displayed:** Open editor menu → verify Copy item has "⌘C" hint (on macOS).

### Manual Testing Checklist

- [ ] Right-click in editor shows Cut/Copy/Paste menu
- [ ] Right-click on file in sidebar shows file menu
- [ ] Right-click on folder in sidebar shows folder menu
- [ ] Right-click on tab shows tab menu
- [ ] Arrow keys navigate menu items
- [ ] Enter activates selected item
- [ ] Escape closes menu
- [ ] Clicking outside menu closes it
- [ ] Menu repositions when near window edge
- [ ] Disabled items are visually distinct and not selectable
- [ ] Separators render as lines, not selectable
- [ ] Shortcut hints align correctly
- [ ] Menu appears at cursor position
- [ ] Menu closes when opening a modal (Cmd+A for command palette)

---

## References

### Internal Docs

- [Panel UI Abstraction](./panel-ui-abstraction.md) - Related overlay/popup patterns

### External Resources

- [VS Code Context Menu](https://code.visualstudio.com/docs/getstarted/userinterface#_context-menus) - Inspiration for menu organization
- [macOS HIG: Context Menus](https://developer.apple.com/design/human-interface-guidelines/context-menus) - Platform conventions

---

## Appendix

### Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|-------------------|--------|-----------|
| Menu state location | Separate struct vs ModalState variant | Separate `ContextMenuState` in `UiState` | Menus behave differently from modals (no input field, position-anchored) |
| Submenu support | V1 with submenus vs flat only | Flat only | Reduces complexity, covers 90% of use cases |
| Action representation | Store `Command` vs `Vec<Msg>` | `MenuAction::Messages(Vec<Msg>)` | Allows both Command-based and custom message actions |
| Shortcut hints | Hardcoded vs keymap lookup | Keymap lookup | Respects user keybinding customizations |
| Menu navigation | Arrow keys vs Tab | Arrow keys | Standard platform convention for menus |

### Open Questions

Resolved during spec:

1. ~~How to handle shortcut hints for user-customized keybindings?~~ → Use `ShortcutHintProvider` that queries active `Keymap`
2. ~~Should menus support search/filter?~~ → No, command palette serves this purpose
3. ~~How to add `context_menu_active` to `KeyContext`?~~ → Add field and corresponding `Condition` variants
4. ~~Where to build menus (App vs update)?~~ → In `App`, since it owns the `Keymap`
5. ~~Click-away pass-through vs consume?~~ → Consume (close menu only, don't trigger underlying actions)

---

## Implementation Checklist

### New Files to Create

- [ ] `src/context_menu/mod.rs` - Module exports
- [ ] `src/context_menu/types.rs` - `ContextMenuState`, `MenuItem`, `MenuAction`, etc.
- [ ] `src/context_menu/builders.rs` - Per-region menu builders
- [ ] `src/context_menu/shortcut_hints.rs` - `ShortcutHintProvider`
- [ ] `src/update/context_menu.rs` - `update_context_menu()` handler
- [ ] `src/keymap/format.rs` - `format_keystroke()` helper

### Model Layer

- [ ] `src/model/ui.rs`:
  - [ ] Add `context_menu: Option<ContextMenuState>` to `UiState`
  - [ ] Add `has_context_menu()` method
  - [ ] Add `close_context_menu()` method

### Messages

- [ ] `src/messages.rs`:
  - [ ] Add `ContextMenuMsg` enum
  - [ ] Add `UiMsg::ContextMenu(ContextMenuMsg)` variant

### Keymap Layer

- [ ] `src/keymap/mod.rs`:
  - [ ] Add `bindings_for_command(&self, cmd: Command)` method to `Keymap`

- [ ] `src/keymap/context.rs`:
  - [ ] Add `context_menu_active: bool` to `KeyContext`
  - [ ] Add `Condition::ContextMenuActive` and `Condition::ContextMenuInactive`

### Runtime Layer

- [ ] `src/runtime/app.rs`:
  - [ ] Add right-click handler: `WindowEvent::MouseInput { button: MouseButton::Right, .. }`
  - [ ] Add context menu keyboard interception (before keymap lookup)
  - [ ] Add click-away handling in left-click handler
  - [ ] Add `handle_right_click()` method
  - [ ] Add `hit_test_context_menu_target()` method
  - [ ] Update `get_key_context()` to include `context_menu_active`

### View Layer

- [ ] `src/view/mod.rs`:
  - [ ] Add `render_context_menu()` function
  - [ ] Call it in main render path after editor/status bar, before modals

- [ ] `src/view/geometry.rs`:
  - [ ] Add `compute_menu_rect()` function
  - [ ] Add `hit_test_menu_item()` function
  - [ ] Add `TabHitResult` struct
  - [ ] Add `hit_test_tab()` function (reusable for left-click too)

### Renderer/Damage

- [ ] `src/view/mod.rs` (or wherever `compute_effective_damage` lives):
  - [ ] Add `model.ui.has_context_menu()` check to force `Damage::Full`

### Update Layer

- [ ] `src/update/mod.rs`:
  - [ ] Add `Msg::Ui(UiMsg::ContextMenu(..))` routing to `update_context_menu()`

### Changelog

| Date | Change |
|------|--------|
| 2026-01-07 | Initial draft |
| 2026-01-07 | Added integration gaps: focus/input routing, coordinates, menu building location, tab hit-testing, keymap API |
