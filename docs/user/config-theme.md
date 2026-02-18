# Theme Configuration Reference

Customize the appearance of Token Editor with themes.

---

## Configuration

### Theme Selection

The active theme is saved in your config file:

| Platform | Path |
|----------|------|
| macOS | `~/.config/token-editor/config.yaml` |
| Linux | `~/.config/token-editor/config.yaml` |
| Windows | `%APPDATA%\token-editor\config.yaml` |

```yaml
# config.yaml
theme: "fleet-dark"
```

### Theme Files

Custom themes are stored in:

| Platform | Path |
|----------|------|
| macOS | `~/.config/token-editor/themes/*.yaml` |
| Linux | `~/.config/token-editor/themes/*.yaml` |
| Windows | `%APPDATA%\token-editor\themes\*.yaml` |

---

## Built-in Themes

| Theme | ID | Description |
|-------|-----|-------------|
| Default Dark | `default-dark` | VS Code-inspired dark theme |
| Fleet Dark | `fleet-dark` | JetBrains Fleet dark theme |
| GitHub Dark | `github-dark` | GitHub dark theme |
| GitHub Light | `github-light` | GitHub light theme |

---

## Theme File Format

```yaml
version: 1
name: "My Custom Theme"
author: "Your Name"
description: "A custom theme description"

ui:
  editor:
    background: "#1E1E1E"
    foreground: "#D4D4D4"
    current_line_background: "#2A2A2A"
    cursor_color: "#FCE146"
    selection_background: "#264F78"
    secondary_cursor_color: "#FFFFFF80"

  gutter:
    background: "#1E1E1E"
    foreground: "#858585"
    foreground_active: "#C6C6C6"
    border_color: "#313438"

  status_bar:
    background: "#007ACC"
    foreground: "#FFFFFF"

  sidebar:
    background: "#252526"
    foreground: "#CCCCCC"
    selection_background: "#FFFFFF1A"
    selection_foreground: "#FFFFFF"
    hover_background: "#FFFFFF0D"
    folder_icon: "#DCDC8B"
    file_icon: "#9CDCFE"
    border: "#3C3C3C"

  tab_bar:
    background: "#252526"
    active_background: "#1E1E1E"
    active_foreground: "#FFFFFF"
    inactive_background: "#2D2D2D"
    inactive_foreground: "#808080"
    border: "#3C3C3C"
    modified_indicator: "#FFFFFF"

  overlay:
    border: "#43454A"
    background: "#2B2D30"
    foreground: "#E0E0E0"
    input_background: "#1E1E1E"
    selection_background: "#264F78"
    highlight: "#80FF80"
    warning: "#FFFF80"
    error: "#FF8080"

  syntax:
    keyword: "#C586C0"
    function: "#DCDCAA"
    function_builtin: "#DCDCAA"
    string: "#CE9178"
    number: "#B5CEA8"
    comment: "#6A9955"
    type: "#4EC9B0"
    variable: "#9CDCFE"
    variable_builtin: "#569CD6"
    property: "#9CDCFE"
    operator: "#D4D4D4"
    punctuation: "#D4D4D4"
    constant: "#569CD6"
    tag: "#569CD6"
    attribute: "#9CDCFE"
    escape: "#D7BA7D"
    label: "#D7BA7D"
    text: "#D4D4D4"
    text_emphasis: "#D4D4D4"
    text_strong: "#D4D4D4"
    text_title: "#569CD6"
    text_uri: "#3E9CD6"
```

---

## Color Formats

Colors can be specified in multiple formats:

| Format | Example | Description |
|--------|---------|-------------|
| Hex (6-digit) | `#FF5500` | RGB in hexadecimal |
| Hex (8-digit) | `#FF550080` | RGBA with alpha |
| Hex (3-digit) | `#F50` | Shorthand RGB (`#FF5500`) |
| Hex (4-digit) | `#F508` | Shorthand RGBA (`#FF550088`) |

### Alpha Channel

Use 8-digit hex for transparency:

```yaml
selection_background: "#264F7880"  # 50% opacity
secondary_cursor_color: "#FFFFFF40"  # 25% opacity
```

---

## UI Components

### Editor

| Property | Description |
|----------|-------------|
| `background` | Main editor background |
| `foreground` | Default text color |
| `current_line_background` | Highlight for cursor line |
| `cursor_color` | Cursor/caret color |
| `selection_background` | Selected text background |
| `secondary_cursor_color` | Multi-cursor secondary cursors |

### Gutter

| Property | Description |
|----------|-------------|
| `background` | Line number area background |
| `foreground` | Inactive line numbers |
| `foreground_active` | Current line number |
| `border_color` | Right border of gutter |

### Status Bar

| Property | Description |
|----------|-------------|
| `background` | Status bar background |
| `foreground` | Status bar text |

### Sidebar

| Property | Description |
|----------|-------------|
| `background` | File tree background |
| `foreground` | Default text color |
| `selection_background` | Selected item background |
| `selection_foreground` | Selected item text |
| `hover_background` | Hovered item background |
| `folder_icon` | Folder icon color |
| `file_icon` | File icon color |
| `border` | Right border of sidebar |

### Tab Bar

| Property | Description |
|----------|-------------|
| `background` | Tab bar background |
| `active_background` | Active tab background |
| `active_foreground` | Active tab text |
| `inactive_background` | Inactive tab background |
| `inactive_foreground` | Inactive tab text |
| `border` | Tab separator lines |
| `modified_indicator` | Unsaved file indicator dot |

### Overlay (Modals/Dialogs)

| Property | Description |
|----------|-------------|
| `border` | Modal border color |
| `background` | Modal background |
| `foreground` | Modal text |
| `input_background` | Text input background |
| `selection_background` | Selected list item |
| `highlight` | Success/match highlight |
| `warning` | Warning text |
| `error` | Error text |

---

## Syntax Highlighting

### Token Types

| Token | Description | Example |
|-------|-------------|---------|
| `keyword` | Language keywords | `if`, `else`, `fn`, `let` |
| `function` | Function names | `calculate()` |
| `function_builtin` | Built-in functions | `print()`, `len()` |
| `string` | String literals | `"hello"` |
| `number` | Numeric literals | `42`, `3.14` |
| `comment` | Comments | `// comment` |
| `type` | Type names | `String`, `Vec` |
| `variable` | Variable names | `count`, `name` |
| `variable_builtin` | Built-in variables | `self`, `this` |
| `property` | Object properties | `.length` |
| `operator` | Operators | `+`, `-`, `=` |
| `punctuation` | Punctuation | `{`, `}`, `;` |
| `constant` | Constants | `true`, `false`, `null` |
| `tag` | HTML/XML tags | `<div>` |
| `attribute` | Tag attributes | `class=` |
| `escape` | Escape sequences | `\n`, `\t` |
| `label` | Labels/symbols | `@decorator` |

### Markdown-Specific

| Token | Description |
|-------|-------------|
| `text` | Default text |
| `text_emphasis` | *Italic text* |
| `text_strong` | **Bold text** |
| `text_title` | # Headings |
| `text_uri` | Links |

---

## Creating a Custom Theme

### Step 1: Create Theme File

```bash
mkdir -p ~/.config/token-editor/themes
touch ~/.config/token-editor/themes/my-theme.yaml
```

### Step 2: Copy Base Theme

Start from a built-in theme:

```yaml
version: 1
name: "My Theme"
author: "Your Name"
description: "Custom theme based on default-dark"

ui:
  editor:
    background: "#1A1A2E"
    foreground: "#EAEAEA"
    # ... customize as needed
```

### Step 3: Select Theme

In-app: Use the Theme Picker (Cmd+Shift+T)

Or edit config directly:

```yaml
# ~/.config/token-editor/config.yaml
theme: "my-theme"
```

---

## Theme Inheritance (Future)

Currently themes must define all properties. Future versions may support:

```yaml
extends: "default-dark"
ui:
  editor:
    background: "#0D1117"  # Override only what you need
```

---

## Tips for Theme Design

### Contrast

Ensure sufficient contrast between:
- `foreground` and `background`
- `selection_background` and selected text
- `cursor_color` and `background`

Use a contrast checker tool for accessibility.

### Consistency

- Use consistent color families
- Match `editor.background` with `gutter.background`
- Coordinate `sidebar.background` with `tab_bar.background`

### Syntax Highlighting

- Keywords should stand out (often pink/purple)
- Strings in warm colors (orange/red)
- Comments should be subdued (gray/muted)
- Types in distinctive color (cyan/teal)

---

## Switching Themes

### Via Command Palette

1. Open Command Palette (Cmd+Shift+A)
2. Type "theme"
3. Select "Change Theme"
4. Choose from list

### Via Keyboard

Default: Cmd+Shift+T (if configured)

---

## Troubleshooting

### Theme Not Loading

1. Check file is valid YAML (use a linter)
2. Ensure `version: 1` is present
3. Verify file is in correct directory
4. Check for typos in color hex codes

### Colors Not Applying

1. Restart the editor after theme changes
2. Verify property names match exactly
3. Check for missing required properties

### Reverting to Default

Delete your config:

```bash
rm ~/.config/token-editor/config.yaml
```

Or set explicitly:

```yaml
# config.yaml
theme: "default-dark"
```
