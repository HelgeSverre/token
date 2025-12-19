# Keyboard Shortcuts

This document provides a complete reference for Token's keyboard shortcuts.

---

## Configuration

Token uses a YAML-based keymapping system with platform-aware modifiers.

### Config Location

- **Default bindings**: Embedded at compile time from `keymap.yaml`
- **User overrides**: `~/.config/token-editor/keymap.yaml` (create to customize)

### Modifier Keys

| Modifier | macOS      | Windows/Linux |
|----------|------------|---------------|
| `cmd`    | Command    | Ctrl          |
| `ctrl`   | Control    | Control       |
| `alt`    | Option     | Alt           |
| `shift`  | Shift      | Shift         |
| `meta`   | Command    | Win           |

The `cmd` modifier is the recommended cross-platform modifier. It maps to Command on macOS and Ctrl on Windows/Linux.

---

## Default Keybindings

### File Operations

| Action          | Shortcut          | Command         |
|-----------------|-------------------|-----------------|
| Save            | Cmd+S             | `SaveFile`      |
| Save As         | Cmd+Shift+S       | `SaveFileAs`    |
| Open File       | Cmd+O             | `OpenFile`      |
| Open Folder     | Cmd+Shift+O       | `OpenFolder`    |
| New Tab         | Cmd+Shift+N       | `NewTab`        |
| Close Tab       | Cmd+W             | `CloseTab`      |

### Undo/Redo

| Action | Shortcut      | Command |
|--------|---------------|---------|
| Undo   | Cmd+Z         | `Undo`  |
| Redo   | Cmd+Shift+Z   | `Redo`  |
| Redo   | Cmd+Y         | `Redo`  |

### Clipboard

| Action | Shortcut | Command |
|--------|----------|---------|
| Copy   | Cmd+C    | `Copy`  |
| Cut    | Cmd+X    | `Cut`   |
| Paste  | Cmd+V    | `Paste` |

### Selection

| Action                  | Shortcut      | Command                 |
|-------------------------|---------------|-------------------------|
| Select All              | Cmd+A         | `SelectAll`             |
| Duplicate Line/Selection| Cmd+D         | `Duplicate`             |
| Select Next Occurrence  | Cmd+J         | `SelectNextOccurrence`  |
| Unselect Last Occurrence| Cmd+Shift+J   | `UnselectOccurrence`    |

### Modals/Dialogs

| Action              | Shortcut      | Command                |
|---------------------|---------------|------------------------|
| Command Palette     | Cmd+Shift+A   | `ToggleCommandPalette` |
| Go to Line          | Cmd+L         | `ToggleGotoLine`       |
| Find/Replace        | Cmd+F         | `ToggleFindReplace`    |

### Workspace

| Action            | Shortcut      | Command           |
|-------------------|---------------|-------------------|
| Toggle Sidebar    | Cmd+1         | `ToggleSidebar`   |
| Reveal in Sidebar | Cmd+Shift+R   | `RevealInSidebar` |

### Layout: Splits

| Action            | Shortcut          | Command           |
|-------------------|-------------------|-------------------|
| Split Horizontal  | Cmd+Shift+Alt+H   | `SplitHorizontal` |
| Split Vertical    | Cmd+Shift+Alt+V   | `SplitVertical`   |

### Layout: Tabs

| Action    | Shortcut       | Command   |
|-----------|----------------|-----------|
| Next Tab  | Cmd+Alt+Right  | `NextTab` |
| Prev Tab  | Cmd+Alt+Left   | `PrevTab` |

### Layout: Focus Groups

| Action           | Shortcut        | Command          |
|------------------|-----------------|------------------|
| Focus Next Group | Ctrl+Tab        | `FocusNextGroup` |
| Focus Prev Group | Ctrl+Shift+Tab  | `FocusPrevGroup` |
| Focus Group 1    | Cmd+Shift+1     | `FocusGroup1`    |
| Focus Group 2    | Cmd+Shift+2     | `FocusGroup2`    |
| Focus Group 3    | Cmd+Shift+3     | `FocusGroup3`    |
| Focus Group 4    | Cmd+Shift+4     | `FocusGroup4`    |

**Numpad shortcuts** (no modifiers):

| Action           | Key              | Command          |
|------------------|------------------|------------------|
| Focus Group 1    | Numpad 1         | `FocusGroup1`    |
| Focus Group 2    | Numpad 2         | `FocusGroup2`    |
| Focus Group 3    | Numpad 3         | `FocusGroup3`    |
| Focus Group 4    | Numpad 4         | `FocusGroup4`    |
| Split Vertical   | Numpad +         | `SplitVertical`  |
| Split Horizontal | Numpad -         | `SplitHorizontal`|

### Basic Navigation

| Action               | Shortcut      | Command                    |
|----------------------|---------------|----------------------------|
| Move Up              | Up            | `MoveCursorUp`             |
| Move Down            | Down          | `MoveCursorDown`           |
| Move Left            | Left          | `MoveCursorLeft`           |
| Move Right           | Right         | `MoveCursorRight`          |
| Line Start           | Home          | `MoveCursorLineStart`      |
| Line End             | End           | `MoveCursorLineEnd`        |
| Page Up              | Page Up       | `PageUp`                   |
| Page Down            | Page Down     | `PageDown`                 |
| Word Left            | Alt+Left      | `MoveCursorWordLeft`       |
| Word Right           | Alt+Right     | `MoveCursorWordRight`      |
| Document Start       | Ctrl+Home     | `MoveCursorDocumentStart`  |
| Document End         | Ctrl+End      | `MoveCursorDocumentEnd`    |

**macOS-specific** (using Command key):

| Action     | Shortcut   | Command                |
|------------|------------|------------------------|
| Line Start | Cmd+Left   | `MoveCursorLineStart`  |
| Line End   | Cmd+Right  | `MoveCursorLineEnd`    |

### Selection Navigation

All navigation commands work with Shift to extend selection:

| Action                        | Shortcut            | Command                              |
|-------------------------------|---------------------|--------------------------------------|
| Select Up                     | Shift+Up            | `MoveCursorUpWithSelection`          |
| Select Down                   | Shift+Down          | `MoveCursorDownWithSelection`        |
| Select Left                   | Shift+Left          | `MoveCursorLeftWithSelection`        |
| Select Right                  | Shift+Right         | `MoveCursorRightWithSelection`       |
| Select to Line Start          | Shift+Home          | `MoveCursorLineStartWithSelection`   |
| Select to Line End            | Shift+End           | `MoveCursorLineEndWithSelection`     |
| Select Page Up                | Shift+Page Up       | `PageUpWithSelection`                |
| Select Page Down              | Shift+Page Down     | `PageDownWithSelection`              |
| Select Word Left              | Alt+Shift+Left      | `MoveCursorWordLeftWithSelection`    |
| Select Word Right             | Alt+Shift+Right     | `MoveCursorWordRightWithSelection`   |
| Select to Document Start      | Ctrl+Shift+Home     | `MoveCursorDocumentStartWithSelection` |
| Select to Document End        | Ctrl+Shift+End      | `MoveCursorDocumentEndWithSelection` |

**macOS-specific** (using Command key):

| Action               | Shortcut         | Command                            |
|----------------------|------------------|------------------------------------|
| Select to Line Start | Cmd+Shift+Left   | `MoveCursorLineStartWithSelection` |
| Select to Line End   | Cmd+Shift+Right  | `MoveCursorLineEndWithSelection`   |

### Editing

| Action              | Shortcut        | Command              | Context       |
|---------------------|-----------------|----------------------|---------------|
| Insert Newline      | Enter           | `InsertNewline`      |               |
| Delete Backward     | Backspace       | `DeleteBackward`     |               |
| Delete Forward      | Delete          | `DeleteForward`      |               |
| Delete Word Backward| Alt+Backspace   | `DeleteWordBackward` |               |
| Delete Word Forward | Alt+Delete      | `DeleteWordForward`  |               |
| Delete Line         | Cmd+Backspace   | `DeleteLine`         |               |
| Indent              | Tab             | `IndentLines`        | has_selection |
| Insert Tab          | Tab             | `InsertTab`          | no_selection  |
| Unindent            | Shift+Tab       | `UnindentLines`      |               |

### Expand/Shrink Selection

| Action           | Shortcut  | Command           |
|------------------|-----------|-------------------|
| Expand Selection | Alt+Up    | `ExpandSelection` |
| Shrink Selection | Alt+Down  | `ShrinkSelection` |

### Escape (Smart Clear)

Escape behavior is context-aware with cascading priority:

| Context              | Shortcut | Command                 |
|----------------------|----------|-------------------------|
| Multiple cursors     | Escape   | `CollapseToSingleCursor`|
| Has selection        | Escape   | `ClearSelection`        |
| No selection         | Escape   | `EscapeSmartClear`      |

---

## Context Conditions

Some bindings only activate in specific contexts. Use the `when` field to specify conditions:

| Condition              | Description                              |
|------------------------|------------------------------------------|
| `has_selection`        | At least one cursor has a selection      |
| `no_selection`         | No cursors have selections               |
| `has_multiple_cursors` | More than one cursor is active           |
| `single_cursor`        | Exactly one cursor is active             |
| `modal_active`         | A modal dialog is open                   |

Example:
```yaml
- key: "tab"
  command: IndentLines
  when: ["has_selection"]
```

---

## Custom Keybindings

To customize keybindings, create `~/.config/token-editor/keymap.yaml`:

```yaml
bindings:
  # Override an existing binding
  - key: "cmd+s"
    command: SaveFileAs  # Make Cmd+S always "Save As"

  # Add a new binding
  - key: "cmd+shift+d"
    command: DeleteLine

  # Context-aware binding
  - key: "cmd+enter"
    command: InsertNewlineBelow
    when: ["no_selection"]
```

User bindings are merged with defaults. User bindings take precedence over defaults when keys match.

---

## Disabling Default Bindings

To disable a default binding, use the `Unbound` command:

```yaml
bindings:
  # Disable Cmd+D (Duplicate)
  - key: "cmd+d"
    command: Unbound
```

---

## Key Names Reference

### Letters and Numbers
`a` through `z`, `0` through `9`

### Named Keys
`enter`, `escape`, `tab`, `backspace`, `delete`, `space`, `insert`

### Arrow Keys
`up`, `down`, `left`, `right`

### Navigation Keys
`home`, `end`, `pageup`, `pagedown`

### Function Keys
`f1` through `f12`

### Numpad Keys
`numpad0` through `numpad9`, `numpad_add`, `numpad_subtract`, `numpad_multiply`, `numpad_divide`, `numpad_decimal`, `numpad_enter`

---

## Platform-Specific Bindings

Use the `platform` field to create platform-specific bindings:

```yaml
bindings:
  # macOS only
  - key: "meta+left"
    command: MoveCursorLineStart
    platform: macos

  # Windows/Linux only
  - key: "ctrl+home"
    command: MoveCursorDocumentStart
    platform: windows
```

Valid platform values: `macos`, `windows`, `linux`
