# Keymap Configuration Reference

Configure keyboard shortcuts for Token Editor.

---

## Configuration File

User keymaps are stored at:

| Platform | Path |
|----------|------|
| macOS | `~/.config/token-editor/keymap.yaml` |
| Linux | `~/.config/token-editor/keymap.yaml` |
| Windows | `%APPDATA%\token-editor\keymap.yaml` |

Create this file to add or override keybindings.

---

## File Format

```yaml
bindings:
  # Simple binding
  - key: "cmd+s"
    command: SaveFile

  # Binding with context condition
  - key: "tab"
    command: IndentLines
    when: ["has_selection"]

  # Platform-specific binding
  - key: "meta+left"
    command: MoveCursorLineStart
    platform: macos

  # Disable a default binding
  - key: "cmd+d"
    command: Unbound
```

---

## Modifier Keys

| Modifier | macOS | Windows/Linux |
|----------|-------|---------------|
| `cmd` | Command | Ctrl |
| `ctrl` | Control | Control |
| `shift` | Shift | Shift |
| `alt` | Option | Alt |
| `meta` | Command | Win |

**Note:** Use `cmd` for the platform "command" key. It maps to Command on macOS and Ctrl on Windows/Linux.

---

## Key Names

### Character Keys

Single characters: `a`, `b`, `c`, `1`, `2`, `3`, etc.

### Named Keys

| Key | Name |
|-----|------|
| Enter/Return | `enter` |
| Escape | `escape` |
| Tab | `tab` |
| Backspace | `backspace` |
| Delete | `delete` |
| Space | `space` |

### Arrow Keys

`up`, `down`, `left`, `right`

### Navigation Keys

`home`, `end`, `pageup`, `pagedown`, `insert`

### Function Keys

`f1`, `f2`, `f3`, `f4`, `f5`, `f6`, `f7`, `f8`, `f9`, `f10`, `f11`, `f12`

### Numpad Keys

`numpad0` through `numpad9`, `numpad_add`, `numpad_subtract`, `numpad_multiply`, `numpad_divide`, `numpad_decimal`, `numpad_enter`

---

## Context Conditions

Bindings can be conditional using the `when` field:

| Condition | Description |
|-----------|-------------|
| `has_selection` | One or more cursors have active selection |
| `no_selection` | No cursor has selection |
| `has_multiple_cursors` | More than one cursor active |
| `single_cursor` | Exactly one cursor |
| `in_editor` | Focus is in the editor pane |

### Example: Context-Aware Tab

```yaml
bindings:
  # Tab with selection: indent
  - key: "tab"
    command: IndentLines
    when: ["has_selection"]

  # Tab without selection: insert tab character
  - key: "tab"
    command: InsertTab
    when: ["no_selection"]
```

---

## Platform-Specific Bindings

Use `platform` to limit a binding to specific operating systems:

```yaml
bindings:
  # macOS only: Cmd+Arrow for line navigation
  - key: "meta+left"
    command: MoveCursorLineStart
    platform: macos

  - key: "meta+right"
    command: MoveCursorLineEnd
    platform: macos
```

Valid platform values: `macos`, `windows`, `linux`

---

## Disabling Default Bindings

Use `command: Unbound` to disable a default binding:

```yaml
bindings:
  # Disable Cmd+D duplicate
  - key: "cmd+d"
    command: Unbound
```

---

## Available Commands

### File Operations

| Command | Description |
|---------|-------------|
| `SaveFile` | Save current file |
| `SaveFileAs` | Save with new name |
| `OpenFile` | Open file dialog |
| `OpenFolder` | Open folder dialog |
| `NewTab` | Create new tab |
| `CloseTab` | Close current tab |

### Undo/Redo

| Command | Description |
|---------|-------------|
| `Undo` | Undo last edit |
| `Redo` | Redo undone edit |

### Clipboard

| Command | Description |
|---------|-------------|
| `Copy` | Copy selection |
| `Cut` | Cut selection |
| `Paste` | Paste from clipboard |

### Selection

| Command | Description |
|---------|-------------|
| `SelectAll` | Select entire document |
| `Duplicate` | Duplicate selection or line |
| `SelectNextOccurrence` | Add cursor at next match |
| `UnselectOccurrence` | Remove last added cursor |
| `ExpandSelection` | Expand to word/line/all |
| `ShrinkSelection` | Shrink to previous scope |
| `ClearSelection` | Clear all selections |
| `CollapseToSingleCursor` | Remove all but primary cursor |

### Navigation

| Command | Description |
|---------|-------------|
| `MoveCursorUp` | Move cursor up one line |
| `MoveCursorDown` | Move cursor down one line |
| `MoveCursorLeft` | Move cursor left one character |
| `MoveCursorRight` | Move cursor right one character |
| `MoveCursorLineStart` | Move to start of line |
| `MoveCursorLineEnd` | Move to end of line |
| `MoveCursorWordLeft` | Move to previous word |
| `MoveCursorWordRight` | Move to next word |
| `MoveCursorDocumentStart` | Move to document start |
| `MoveCursorDocumentEnd` | Move to document end |
| `PageUp` | Move up one page |
| `PageDown` | Move down one page |

### Navigation with Selection

All navigation commands have `*WithSelection` variants that extend the selection:

- `MoveCursorUpWithSelection`
- `MoveCursorDownWithSelection`
- `MoveCursorLeftWithSelection`
- `MoveCursorRightWithSelection`
- `MoveCursorLineStartWithSelection`
- `MoveCursorLineEndWithSelection`
- `MoveCursorWordLeftWithSelection`
- `MoveCursorWordRightWithSelection`
- `MoveCursorDocumentStartWithSelection`
- `MoveCursorDocumentEndWithSelection`
- `PageUpWithSelection`
- `PageDownWithSelection`

### Editing

| Command | Description |
|---------|-------------|
| `InsertNewline` | Insert line break |
| `InsertTab` | Insert tab character |
| `DeleteBackward` | Delete character before cursor |
| `DeleteForward` | Delete character after cursor |
| `DeleteWordBackward` | Delete word before cursor |
| `DeleteWordForward` | Delete word after cursor |
| `DeleteLine` | Delete current line |
| `IndentLines` | Indent selected lines |
| `UnindentLines` | Unindent selected lines |

### Modals/Dialogs

| Command | Description |
|---------|-------------|
| `ToggleCommandPalette` | Open/close command palette |
| `ToggleGotoLine` | Open/close go to line |
| `ToggleFindReplace` | Open/close find/replace |

### Layout

| Command | Description |
|---------|-------------|
| `SplitHorizontal` | Split pane horizontally |
| `SplitVertical` | Split pane vertically |
| `NextTab` | Switch to next tab |
| `PrevTab` | Switch to previous tab |
| `FocusNextGroup` | Focus next editor group |
| `FocusPrevGroup` | Focus previous editor group |
| `FocusGroup1` through `FocusGroup4` | Focus specific group |

### Workspace

| Command | Description |
|---------|-------------|
| `ToggleSidebar` | Show/hide sidebar |
| `RevealInSidebar` | Show current file in tree |

### Special

| Command | Description |
|---------|-------------|
| `EscapeSmartClear` | Smart escape cascade |
| `Unbound` | Disable a binding |

---

## Default Keybindings

### File Operations

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Save | Cmd+S | Ctrl+S |
| Save As | Cmd+Shift+S | Ctrl+Shift+S |
| Open File | Cmd+O | Ctrl+O |
| Open Folder | Cmd+Shift+O | Ctrl+Shift+O |
| New Tab | Cmd+Shift+N | Ctrl+Shift+N |
| Close Tab | Cmd+W | Ctrl+W |

### Editing

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Undo | Cmd+Z | Ctrl+Z |
| Redo | Cmd+Shift+Z | Ctrl+Shift+Z |
| Copy | Cmd+C | Ctrl+C |
| Cut | Cmd+X | Ctrl+X |
| Paste | Cmd+V | Ctrl+V |
| Select All | Cmd+A | Ctrl+A |
| Duplicate | Cmd+D | Ctrl+D |
| Delete Line | Cmd+Backspace | Ctrl+Backspace |

### Navigation

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Line Start | Cmd+Left or Home | Home |
| Line End | Cmd+Right or End | End |
| Word Left | Option+Left | Alt+Left |
| Word Right | Option+Right | Alt+Right |
| Document Start | Ctrl+Home | Ctrl+Home |
| Document End | Ctrl+End | Ctrl+End |

### Selection

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Expand Selection | Option+Up | Alt+Up |
| Shrink Selection | Option+Down | Alt+Down |
| Select Next Occurrence | Cmd+J | Ctrl+J |

### Dialogs

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Command Palette | Cmd+Shift+A | Ctrl+Shift+A |
| Go to Line | Cmd+L | Ctrl+L |
| Find/Replace | Cmd+F | Ctrl+F |

### Layout

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Toggle Sidebar | Cmd+1 | Ctrl+1 |
| Split Horizontal | Cmd+Shift+Alt+H | Ctrl+Shift+Alt+H |
| Split Vertical | Cmd+Shift+Alt+V | Ctrl+Shift+Alt+V |
| Next Tab | Cmd+Alt+Right | Ctrl+Alt+Right |
| Previous Tab | Cmd+Alt+Left | Ctrl+Alt+Left |

---

## Binding Precedence

When multiple bindings match a keystroke:

1. **Context conditions** are evaluated (most specific wins)
2. **Platform-specific** bindings take precedence on matching platforms
3. **User bindings** override default bindings with same key + conditions
4. **Later bindings** in the file override earlier ones with same key

---

## Example: Custom Configuration

```yaml
# ~/.config/token-editor/keymap.yaml
bindings:
  # Remap Cmd+P to command palette (VS Code style)
  - key: "cmd+p"
    command: ToggleCommandPalette

  # Remap Cmd+Shift+P to quick open
  - key: "cmd+shift+p"
    command: QuickOpen

  # Disable duplicate (I never use it)
  - key: "cmd+d"
    command: Unbound

  # Custom split shortcuts
  - key: "cmd+\\"
    command: SplitVertical

  - key: "cmd+shift+\\"
    command: SplitHorizontal

  # Vim-style j/k navigation in command palette
  - key: "ctrl+j"
    command: PaletteNext
    when: ["in_palette"]

  - key: "ctrl+k"
    command: PalettePrev
    when: ["in_palette"]
```

---

## Troubleshooting

### Binding Not Working

1. Check the key name is correct (see [Key Names](#key-names))
2. Verify modifiers are in order: `ctrl+shift+alt+key`
3. Check for conflicting bindings with `when` conditions
4. Ensure the file is valid YAML (use a linter)

### Finding Default Bindings

The default keymap is embedded in the binary. To see all defaults:

```bash
# View the source keymap.yaml
cat $(dirname $(which token))/keymap.yaml

# Or check the repository
# https://github.com/helgesverre/token/blob/main/keymap.yaml
```

### Resetting to Defaults

Delete your custom keymap file:

```bash
rm ~/.config/token-editor/keymap.yaml
```
