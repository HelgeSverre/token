# Themes

Token supports fully customizable color themes via YAML configuration files.

---

## Available Themes

Token ships with fifteen built-in themes:

| Theme              | ID               | File                 | Light/Dark | Description                                                                        |
|--------------------|------------------|----------------------|------------|-------------------------------------------------------------------------------------|
| Default Dark       | `default-dark`   | `dark.yaml`          | Dark       | VS Code-inspired dark theme                                                         |
| Study              | `study`          | `study.yaml`         | Dark       | Cool graphite surfaces with restrained teal accents, from the performance-panel study |
| Fleet Dark         | `fleet-dark`     | `fleet-dark.yaml`    | Dark       | JetBrains Fleet-inspired dark theme                                                |
| GitHub Dark        | `github-dark`    | `github-dark.yaml`   | Dark       | GitHub's dark theme                                                                 |
| GitHub Light       | `github-light`   | `github-light.yaml`  | Light      | GitHub's light theme                                                               |
| Dracula            | `dracula`        | `dracula.yaml`       | Dark       | The iconic dark theme with bold purples, pinks, and cyans                          |
| Catppuccin Mocha   | `mocha`          | `mocha.yaml`         | Dark       | Soothing pastel theme with warm, cozy colors                                       |
| Nord               | `nord`           | `nord.yaml`          | Dark       | Arctic, north-bluish color palette with dimmed pastels                             |
| Tokyo Night        | `tokyo-night`    | `tokyo-night.yaml`   | Dark       | Neon lights of downtown Tokyo at night                                             |
| Gruvbox Dark       | `gruvbox-dark`   | `gruvbox-dark.yaml`  | Dark       | Retro groove color scheme with warm, earthy tones                                  |
| Liseth Solutions   | `liseth`         | `liseth.yaml`        | Dark       | Liseth Solutions brandbook: navy ground, cyan hero, blue and purple accents        |
| Jake               | `jake`           | `jake.yaml`          | Dark       | Rose on near-black, after jakefile.dev                                             |
| Sema               | `sema`           | `sema.yaml`          | Dark       | Warm parchment on charcoal with gold, after sema-lang.com                           |
| Glue               | `glue`           | `glue.yaml`          | Dark       | Graphite with a yellow highlighter, after getglue.dev                               |
| FEdit              | `fedit`          | `fedit.yaml`         | Dark       | Zinc terminal green, after fedit.dev                                                |

The same themes are shown in the website theme gallery
(`website/src/data/themes.ts`, generated from `themes/*.yaml` by
`website/scripts/generate-themes.mjs`, run from `website/` or automatically
via the `prebuild` hook of `npm run build`).

---

## Configuration

### Theme Location

- **Built-in themes**: Embedded at compile time from `themes/` directory
- **User themes**: `~/.config/token-editor/themes/` (create directory to add custom themes)

### Switching Themes

Use the Command Palette (Cmd+Shift+A) and search for "Switch Theme" to change themes.

---

## Theme Format

Themes use YAML with a structured format:

```yaml
version: 1
name: "My Custom Theme"
author: "Your Name"
description: "A brief description of the theme"

ui:
  editor:
    background: "#1E1E1E"
    foreground: "#D4D4D4"
    # ... more colors

  syntax:
    keyword: "#C586C0"
    function: "#DCDCAA"
    # ... more colors
```

### Color Format

Colors use hexadecimal format:
- **6-digit**: `#RRGGBB` (e.g., `#1E1E1E`)
- **8-digit**: `#RRGGBBAA` with alpha channel (e.g., `#FFFFFF80` for 50% opacity)

---

## Color Reference

### UI Colors

#### `editor` - Main Editor Area

| Key                      | Description                          |
|--------------------------|--------------------------------------|
| `background`             | Editor background color              |
| `foreground`             | Default text color                   |
| `current_line_background`| Background of the current line       |
| `cursor_color`           | Primary cursor color                 |
| `selection_background`   | Selected text background             |
| `secondary_cursor_color` | Multi-cursor secondary cursor color  |
| `bracket_match_background` | Matching-bracket highlight background (optional) |
| `ghost_text`             | Inline suggestion (ghost text) color (optional; derived from foreground/background when absent) |
| `indent_guide`           | Indentation guide line color         |

#### `gutter` - Line Numbers Area

| Key                | Description                           |
|--------------------|---------------------------------------|
| `background`       | Gutter background color               |
| `foreground`       | Line number color (inactive lines)    |
| `foreground_active`| Line number color (current line)      |
| `border_color`     | Border between gutter and editor      |

#### `status_bar` - Bottom Status Bar

| Key          | Description              |
|--------------|--------------------------|
| `background` | Status bar background    |
| `foreground` | Status bar text color    |
| `border`     | Status bar border color  |

#### `sidebar` - File Tree Sidebar

| Key                    | Description                          |
|------------------------|--------------------------------------|
| `background`           | Sidebar background                   |
| `foreground`           | Default text color                   |
| `selection_background` | Selected item background             |
| `selection_foreground` | Selected item text color             |
| `hover_background`     | Hovered item background              |
| `folder_icon`          | Folder icon color                    |
| `file_icon`            | File icon color                      |
| `border`               | Border between sidebar and editor    |

#### `tab_bar` - Tab Bar Area

| Key                  | Description                    |
|----------------------|--------------------------------|
| `background`         | Tab bar background             |
| `active_background`  | Active tab background          |
| `active_foreground`  | Active tab text color          |
| `inactive_background`| Inactive tab background        |
| `inactive_foreground`| Inactive tab text color        |
| `border`             | Tab bar border                 |
| `modified_indicator` | Unsaved changes indicator color|

#### `overlay` - Dialogs and Command Palette

| Key                    | Description                      |
|------------------------|----------------------------------|
| `border`               | Overlay border color             |
| `background`           | Overlay background               |
| `foreground`           | Overlay text color               |
| `input_background`     | Input field background           |
| `selection_background` | Selected item background         |
| `highlight`            | Highlight/match color            |
| `warning`              | Warning message color            |
| `error`                | Error message color              |

Extended palette keys (optional; when omitted they are derived from the core
colors — for example, hairlines are mixed from the panel background and
foreground, and severity colors fall back to the core error/warning):

| Key                     | Description                              |
|-------------------------|------------------------------------------|
| `accent`                | Accent color for active work             |
| `accent_bright`         | Brighter accent variant                  |
| `panel_background`      | Overlay panel background                |
| `panel_secondary`       | Secondary panel tier background         |
| `recessed_wash`         | Recessed background wash                 |
| `hairline`              | Thin separator lines                     |
| `selection_wash`        | Selected-row background wash             |
| `match_on_selection`    | Match text color on selected rows        |
| `text_primary`          | Primary text color                       |
| `text_bright`           | Bright text color                        |
| `text_secondary`        | Secondary text color                    |
| `text_dim`              | Dim text color                           |
| `keycap_bg`             | Shortcut keycap background               |
| `keycap_border`         | Shortcut keycap border                   |
| `keycap_fg`             | Shortcut keycap text                     |
| `severity_error`        | Error severity icon color                |
| `severity_error_text`   | Error severity text color                |
| `severity_warning`      | Warning severity icon color              |
| `severity_warning_text` | Warning severity text color              |
| `severity_info`         | Info severity icon color                 |
| `severity_info_text`    | Info severity text color                 |
| `severity_hint`         | Hint severity icon color                 |
| `severity_hint_text`    | Hint severity text color                 |

#### `button` - Buttons (optional)

| Key                   | Description                       |
|-----------------------|-----------------------------------|
| `background`          | Rest background                    |
| `background_hover`    | Hover background                   |
| `background_pressed`  | Pressed background                 |
| `background_selected` | Selected background                |
| `foreground`          | Label text color                   |
| `foreground_disabled` | Disabled label text color          |
| `border`              | Border color                       |
| `focus_ring`          | Focus ring color                   |

#### `csv` - CSV Viewer/Editor Mode

| Key                       | Description                       |
|---------------------------|-----------------------------------|
| `header_background`       | Background for column headers     |
| `header_foreground`       | Text color for column headers     |
| `grid_line`               | Color for cell grid lines         |
| `selected_cell_background`| Background of selected cell       |
| `selected_cell_border`    | Border around selected cell       |
| `number_foreground`       | Color for numeric cell values     |

#### `image_preview` - Image Viewer

| Key                  | Description                                          |
|----------------------|------------------------------------------------------|
| `checkerboard_light` | Light checkerboard square behind transparent images  |
| `checkerboard_dark`  | Dark checkerboard square behind transparent images   |
| `checkerboard_size`  | Checkerboard square size in pixels                  |

#### `scrollbar` - Scrollbars

| Key           | Description                    |
|---------------|--------------------------------|
| `track`       | Scrollbar track color          |
| `thumb`       | Scrollbar thumb color          |
| `thumb_hover` | Scrollbar thumb color on hover |

#### `splitter` - Split Panes

| Key          | Description               |
|--------------|---------------------------|
| `background` | Splitter divider color   |

### Syntax Colors

These colors are used for syntax highlighting across all supported languages:

| Key               | Description                        | Example Usage          |
|-------------------|------------------------------------|-----------------------|
| `keyword`         | Language keywords                  | `if`, `else`, `fn`    |
| `function`        | Function names                     | `println`, `map`      |
| `function_builtin`| Built-in functions                 | `len`, `print`        |
| `string`          | String literals                    | `"hello"`             |
| `number`          | Numeric literals                   | `42`, `3.14`          |
| `comment`         | Comments                           | `// comment`          |
| `type`            | Type names                         | `String`, `Vec`       |
| `variable`        | Variable names                     | `count`, `name`       |
| `variable_builtin`| Built-in variables                 | `self`, `this`        |
| `property`        | Object properties/fields           | `.field`, `.method`   |
| `operator`        | Operators                          | `+`, `-`, `=>`        |
| `punctuation`     | Punctuation                        | `{`, `}`, `;`         |
| `constant`        | Constants                          | `TRUE`, `NULL`        |
| `tag`             | HTML/XML tags                      | `<div>`, `<span>`     |
| `attribute`       | HTML/XML attributes                | `class=`, `id=`       |
| `escape`          | Escape sequences                   | `\n`, `\t`            |
| `label`           | Labels                             | `'lifetime`, `loop:`  |
| `text`            | Plain text (markdown)              |                       |
| `text_emphasis`   | Emphasized text                    | `*italic*`            |
| `text_strong`     | Strong text                        | `**bold**`            |
| `text_title`      | Headings                           | `# Title`             |
| `text_uri`        | URLs and URIs                      | `https://...`         |

---

## Creating a Custom Theme

1. **Create the themes directory**:
   ```bash
   mkdir -p ~/.config/token-editor/themes
   ```

2. **Create a new theme file** (e.g., `my-theme.yaml`):
   ```yaml
   version: 1
   name: "My Theme"
   author: "Your Name"
   description: "My custom Token theme"

   ui:
     editor:
       background: "#282C34"
       foreground: "#ABB2BF"
       current_line_background: "#2C313A"
       cursor_color: "#528BFF"
       selection_background: "#3E4451"
       secondary_cursor_color: "#528BFF80"

     gutter:
       background: "#282C34"
       foreground: "#4B5263"
       foreground_active: "#ABB2BF"
       border_color: "#3E4451"

     status_bar:
       background: "#21252B"
       foreground: "#9DA5B4"

     sidebar:
       background: "#21252B"
       foreground: "#ABB2BF"
       selection_background: "#2C313A"
       selection_foreground: "#FFFFFF"
       hover_background: "#2C313A80"
       folder_icon: "#E5C07B"
       file_icon: "#61AFEF"
       border: "#181A1F"

     tab_bar:
       background: "#21252B"
       active_background: "#282C34"
       active_foreground: "#FFFFFF"
       inactive_background: "#21252B"
       inactive_foreground: "#5C6370"
       border: "#181A1F"
       modified_indicator: "#E5C07B"

     overlay:
       border: "#181A1F"
       background: "#21252B"
       foreground: "#ABB2BF"
       input_background: "#1B1D23"
       selection_background: "#2C313A"
       highlight: "#98C379"
       warning: "#E5C07B"
       error: "#E06C75"

     csv:
       header_background: "#21252B"
       header_foreground: "#ABB2BF"
       grid_line: "#3E4451"
       selected_cell_background: "#2C313A80"
       selected_cell_border: "#61AFEF"
       number_foreground: "#D19A66"

     syntax:
       keyword: "#C678DD"
       function: "#61AFEF"
       function_builtin: "#61AFEF"
       string: "#98C379"
       number: "#D19A66"
       comment: "#5C6370"
       type: "#E5C07B"
       variable: "#E06C75"
       variable_builtin: "#E06C75"
       property: "#E06C75"
       operator: "#56B6C2"
       punctuation: "#ABB2BF"
       constant: "#D19A66"
       tag: "#E06C75"
       attribute: "#D19A66"
       escape: "#56B6C2"
       label: "#E06C75"
       text: "#ABB2BF"
       text_emphasis: "#ABB2BF"
       text_strong: "#ABB2BF"
       text_title: "#E06C75"
       text_uri: "#61AFEF"
   ```

3. **Restart Token** and select your theme from the Command Palette.

> **Note:** Sections and individual keys are optional — anything omitted falls
> back to defaults derived from the colors you do provide (for example,
> `overlay.hairline` is mixed from the panel background and foreground, and
> `editor.indent_guide` falls back to a foreground/background-derived color).
> A complete theme file like the example above will look consistent
> everywhere; a minimal one will still work.

---

## Tips

- **Start from an existing theme**: Copy one of the built-in themes and modify the colors.
- **Use a color picker**: Tools like [coolors.co](https://coolors.co) help create harmonious palettes.
- **Test with different file types**: Verify syntax colors look good across multiple languages.
- **Consider contrast**: Ensure sufficient contrast between foreground and background colors for readability.
