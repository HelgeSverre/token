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

Create this file to add or override keybindings, then restart Token or reopen
Settings → Keymap to load it. Automatic file watching is not implemented.
Palette and context-menu shortcut hints
come from the loaded bindings, including overrides, unbinding and conditions;
they are not a separate list of default shortcuts.

---

## Editing in Settings

Open the separate Settings page and select the **Keymap** category (Tab/Shift+Tab cycles categories). Search matches
command names, shortcuts and context details in the merged embedded/user list;
unassigned commands are listed separately. Click a binding row or press Enter to
record up to four keystrokes. Recording consumes shortcuts, including Quit and
debug keys, without running their commands. Repeated key-down events are ignored.

macOS menu accelerators (such as Cmd+Q/Cmd+H) and debug-build reserved function
keys are consumed with an explanatory error, not recorded as unusable overrides.
Native menu remapping is separate from this keymap editor. OS-reserved shortcuts
that never reach Token cannot be captured.

**Ctrl+Enter** or **Save** commits; **Escape** or **Cancel** discards the capture.
Backspace removes the last stroke. **Literal** records the next reserved control
literally, including Escape, Backspace and Ctrl+Enter. Conflict warnings compare
typed sequences, including prefixes, only in overlapping contexts on this OS.
Saving does not automatically resolve every conflict: a shorter prefix can still
shadow a chord, and overlapping conditions can still compete. Edit `keymap.yaml`
directly to remove such conflicts or change conditions.

The **Token** base uses embedded defaults. **Common** (`base: conventional`) changes only `cmd+p`
to File Finder, `cmd+shift+p` to Command Palette and `cmd+d` to Select Next
Occurrence; it is not a full emulation of another editor. Base chips save
immediately, and user overrides always win—including a legacy keymap containing
a full copy of the defaults.

Saves write only overrides and the optional `base: conventional`/`base: token`
choice to the user keymap, never to `config.yaml`. Rebinding preserves the selected
binding's conditions and other bindings at its old sequence. Generated overrides
are OS-local; portable and foreign-platform entries remain intact. Unknown YAML
keys are retained, but comments and formatting are not. Successful saves apply
immediately. Invalid, unreadable, stale or read-only files show an error without
changing the loaded keymap. Close/reopen Settings after an external edit.

The editor accepts at most 1 MiB and 2,048 merged bindings for this workflow.
Settings refuses symlink keymaps (and hard-linked keymaps on Unix), since atomic
replacement would change their identity; edit those directly. Saves use a
sidecar advisory lock, bounded reads, temporary-file replacement and an exact
text recheck. The lock coordinates Token writers, not arbitrary external editors;
avoid simultaneous external edits during a save. `plus` and `literal_space`
encode literal character keys, while `space` denotes the named Space key.

---

## Default Bindings

The embedded `keymap.yaml` is the single full default keymap. User entries are
merged into an independent copy: an identical keystroke sequence and condition
list replaces the matching default; otherwise the entry is added. `Unbound`
removes every binding with that exact sequence, regardless of its conditions.
User overrides do not mutate the cached defaults.

Command names are case-sensitive and match their bindable action names, including
`ToggleUsages` and `RestartLanguageServer`. Unknown names remain errors. An
invalid user keymap is logged and leaves embedded defaults active. If the
embedded YAML itself cannot be parsed, emergency Save/Open/Quit shortcuts remain
available; this is not a second full set of defaults.

## File Format

An optional `base: token` (default) or `base: conventional` chooses the base layer.
The latter changes only `cmd+p` to File Finder, `cmd+shift+p` to Command Palette
and `cmd+d` to Select Next Occurrence. Existing user entries are merged last and
keep precedence. Keymap reads are bounded to 1 MiB; preference snapshots allow
at most 2,048 merged bindings. Chord strokes share one parser, including Unicode
characters, F1–F24, `plus` for a literal `+`, and `literal_space` for a character
space (distinct from the named `space` key).

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

## Chord Sequences

Use spaces between keystrokes in the `key` string:

```yaml
bindings:
  - key: "ctrl+k ctrl+c"
    command: Copy
    when: ["has_selection"]
```

The editor waits for the next stroke after a chord prefix. A single-stroke
binding on that prefix takes precedence; an earlier complete chord can also
make a longer chord unreachable. Such shadowed chords are not shown as hints.
An unmatched next stroke clears the pending sequence. There is no chord timeout
yet. Chords currently start in editor keymap routing; modals and docks retain
their own key handling and single-stroke global shortcuts.

Palette hints describe editor conditions after the palette closes; context-menu
hints use the underlying menu target. Temporary popup/inline-suggestion states
are excluded. Hints are shortcuts to the command, not promises that the same
keys will bypass an open menu's navigation handling.

## Modifier Keys

`OpenInlineStatistics` is a bindable action for “Open Inline Completion
Statistics”; it has no default shortcut. It dismisses the current inline offer
before queuing the statistics file open, so that outcome is included in the read.

Inline alternatives use `alt+]` / `alt+[` while ghost text is visible. The named
actions are `NextInlineSuggestion` and `PrevInlineSuggestion`. On macOS, Option
shortcuts try the typed character first, then the current keyboard layout's
unmodified key. If neither interpretation has a binding, normal text input
retains its composed character. A chord advances only once per key event.

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
| `has_selection` | The active cursor has a selection |
| `no_selection` | The active cursor has no selection |
| `has_multiple_cursors` | More than one cursor active |
| `single_cursor` | Exactly one cursor |
| `modal_active` | A modal dialog is open |
| `modal_inactive` | No modal dialog is open |
| `editor_focused` | Focus is in the editor pane |
| `sidebar_focused` | Focus is in the sidebar file tree |

Conditions on one binding are ANDed together. Chord prefixes use the same
eligibility rules as completed bindings: an inactive branch cannot start or
prolong a pending sequence. Conditions are checked again on each stroke, so
changing selection or focus can make a pending branch ineligible. Other eligible
branches remain available, and existing single-stroke/chord precedence is unchanged.

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
| `FuzzyFileFinder` | Search workspace files |
| `NewFile` | Create new file |
| `NewTab` | Create new tab |
| `CloseTab` | Close current tab |
| `Quit` | Quit application |

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
| `ExpandSelection` | Expand through syntax scopes, then line/all ([details](../archived/syntax-aware-expand-selection.md)) |
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
| `ToggleSidebar` | Show/hide sidebar (legacy, use `ToggleFileExplorer`) |
| `RevealInSidebar` | Show current file in tree |
| `FileTreeSelectPrevious` | Select previous item in file tree |
| `FileTreeSelectNext` | Select next item in file tree |
| `FileTreeOpenOrToggle` | Open selected file or toggle folder |
| `FileTreeRefresh` | Refresh the file tree from disk |

### Panels/Docks

| Command | Description |
|---------|-------------|
| `ToggleFileExplorer` | Toggle file explorer panel |
| `ToggleTerminal` | Toggle terminal panel |
| `ToggleOutline` | Toggle outline panel |
| `CloseFocusedDock` | Close the currently focused dock |

### Markdown Preview

| Command | Description |
|---------|-------------|
| `MarkdownTogglePreview` | Toggle markdown preview pane |
| `MarkdownOpenPreviewToSide` | Open markdown preview to the side |

### Special

| Command | Description |
|---------|-------------|
| `EscapeSmartClear` | Smart escape cascade |
| `Unbound` | Disable a binding |
| `OpenLogFile` | Open the log file in the editor |

---

## Default Keybindings

### File Operations

| Action | Mac | Windows/Linux |
|--------|-----|---------------|
| Save | Cmd+S | Ctrl+S |
| Save As | Cmd+Shift+S | Ctrl+Shift+S |
| Open File | Cmd+O | Ctrl+O |
| Go to File | Cmd+Shift+O | Ctrl+Shift+O |
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
| Toggle File Explorer | Cmd+1 | Ctrl+1 |
| Toggle Terminal | Cmd+2 | Ctrl+2 |
| Toggle Outline | Cmd+7 | Ctrl+7 |
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

  # Remap Cmd+Shift+P to fuzzy file finder
  - key: "cmd+shift+p"
    command: FuzzyFileFinder

  # Disable duplicate (I never use it)
  - key: "cmd+d"
    command: Unbound

  # Custom split shortcuts
  - key: "cmd+\\"
    command: SplitVertical

  - key: "cmd+shift+\\"
    command: SplitHorizontal
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
