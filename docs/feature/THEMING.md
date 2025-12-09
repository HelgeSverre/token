# Theming System Design

A pragmatic, YAML-based theming system inspired by JetBrains Fleet's JSON theme format.

---

## Overview

**Goals:**

- Simple file format (YAML) that's human-editable
- Comprehensive coverage of all UI components
- State modifiers (hover, active, focused, disabled)
- Semantic naming aligned with `EDITOR_UI_REFERENCE.md` glossary
- Easy to parse in Rust with `serde`

**Non-goals (for now):**

- Color palette references/aliases (add later if needed)
- Light theme support (dark-only initially)
- Theme inheritance/extending

---

## File Format: YAML

**Why YAML over JSON or key=value?**

| Format    | Pros                                           | Cons                       |
| --------- | ---------------------------------------------- | -------------------------- |
| **YAML**  | Comments, readable nesting, no trailing commas | Whitespace-sensitive       |
| JSON      | Fleet uses it, universal                       | No comments, verbose       |
| key=value | Simple                                         | No nesting, flat structure |

YAML maps cleanly to Rust structs via `serde_yaml`, supports comments for documentation, and is easy to hand-edit.

---

## Complete Theme Schema

### Structure

```yaml
version: 1 # Schema version for future migrations
name: "Theme Name"
author: "Author Name"
description: "Optional description"

ui: # All UI component colors
  window: { ... }
  editor: { ... }
  gutter: { ... }
  scrollbar: { ... }
  status_bar: { ... }
  popup: { ... }
  search: { ... }
  diagnostics: { ... }

syntax: # Syntax highlighting colors
  comment: { ... }
  keyword: { ... }
  # ... etc
```

### Naming Conventions

1. **snake_case** for all keys (matches Rust field names)
2. **Semantic over visual** names (`editor.background` not `dark_gray`)
3. **Components are nouns**, states are nested under them
4. **Hierarchy**: `component.subcomponent.property.state`

### State Modifiers

For interactive elements, states are nested:

```yaml
background:
  normal: "#1E1E1E" # Default state
  hover: "#2A2A2A" # Mouse hovering
  active: "#3A3A3A" # Being pressed/activated
  focused: "#252525" # Has keyboard focus
  disabled: "#151515" # Grayed out
```

If a state is missing, fall back to `normal`.

---

## Complete Key Reference

### UI Components

Based on `EDITOR_UI_REFERENCE.md` glossary with state modifiers where needed:

```yaml
ui:
  # ─────────────────────────────────────────────────────────────
  # Window / Chrome
  # ─────────────────────────────────────────────────────────────
  window:
    background: "#0D1117" # Main window background
    border: "#21262D" # Window border (if any)
    title_bar:
      background: "#161B22"
      foreground: "#C9D1D9"

  # ─────────────────────────────────────────────────────────────
  # Editor (Text Area)
  # ─────────────────────────────────────────────────────────────
  editor:
    background:
      normal: "#0D1117"
      focused: "#0D1117"
      disabled: "#090C10"
    foreground: "#C9D1D9" # Default text color

    # Current line highlight
    current_line:
      background: "#161B22"
      border: "#21262D" # Optional border around current line

    # Invisible characters (whitespace, tabs when shown)
    invisibles: "#484F58"

    # Selection
    selection:
      background:
        normal: "#264F78" # Active selection
        inactive: "#1D3A5C" # Unfocused window selection
      border: "#58A6FF" # Optional selection border

    # Cursor / Caret
    cursor:
      pipe:
        color: "#58A6FF" # Pipe cursor color
        width: 2 # Pixels
      block:
        background: "#58A6FF"
        foreground: "#0D1117" # Text color inside block cursor
      underline:
        color: "#58A6FF"
        height: 2 # Pixels
      blink_rate: 530 # Milliseconds (0 = no blink)

    # Indent guides
    indent_guide:
      normal: "#21262D"
      active: "#30363D" # Guide on current scope

    # Matching brackets
    bracket_match:
      background: "#2D333B"
      border: "#58A6FF"

    # Word highlight (same word under cursor)
    word_highlight:
      background: "#2D333B"
      border: "#484F58"

    # Line numbers (alternative location, can also be in gutter)
    line_number:
      foreground: "#484F58"
      foreground_active: "#C9D1D9" # Current line number

  # ─────────────────────────────────────────────────────────────
  # Gutter (Line Numbers, Fold Markers, etc.)
  # ─────────────────────────────────────────────────────────────
  gutter:
    background:
      normal: "#0D1117"
      focused: "#0D1117"
    separator: "#21262D" # Border between gutter and text

    # Line numbers
    line_number:
      foreground: "#484F58"
      foreground_active: "#C9D1D9" # Current line
      foreground_error: "#F85149" # Line with error
      foreground_warning: "#D29922" # Line with warning

    # Fold markers
    fold_marker:
      foreground: "#484F58"
      foreground_hover: "#C9D1D9"
      background_hover: "#21262D"

    # Breakpoints
    breakpoint:
      enabled: "#F85149"
      disabled: "#484F58"
      conditional: "#D29922"

    # Git diff indicators
    diff:
      added: "#3FB950"
      modified: "#58A6FF"
      deleted: "#F85149"

  # ─────────────────────────────────────────────────────────────
  # Scrollbar
  # ─────────────────────────────────────────────────────────────
  scrollbar:
    track:
      background: "#0D1117"
    thumb:
      background:
        normal: "#30363D"
        hover: "#484F58"
        active: "#6E7681"
    # Minimap markers (if you add minimap)
    marker:
      error: "#F85149"
      warning: "#D29922"
      search: "#58A6FF"
      selection: "#264F78"

  # ─────────────────────────────────────────────────────────────
  # Status Bar
  # ─────────────────────────────────────────────────────────────
  status_bar:
    background: "#161B22"
    foreground: "#8B949E"
    border: "#21262D"

    # Status bar items with states
    item:
      foreground:
        normal: "#8B949E"
        hover: "#C9D1D9"
        active: "#58A6FF"
      background:
        hover: "#21262D"
        active: "#0D419D"

    # Mode indicator (for vim-like modes)
    mode:
      normal:
        background: "#238636"
        foreground: "#FFFFFF"
      insert:
        background: "#58A6FF"
        foreground: "#FFFFFF"
      visual:
        background: "#A371F7"
        foreground: "#FFFFFF"
      command:
        background: "#D29922"
        foreground: "#000000"

  # ─────────────────────────────────────────────────────────────
  # Popups / Overlays (Autocomplete, Hover, etc.)
  # ─────────────────────────────────────────────────────────────
  popup:
    background: "#161B22"
    foreground: "#C9D1D9"
    border: "#30363D"
    shadow: "#010409CC" # With alpha for shadow

    # Autocomplete specific
    autocomplete:
      item:
        background:
          normal: "transparent"
          hover: "#21262D"
          selected: "#264F78"
        foreground:
          normal: "#C9D1D9"
          hover: "#C9D1D9"
          selected: "#FFFFFF"
      match_highlight: "#58A6FF" # Matched characters
      kind_icon: # Completion item kind icons
        function: "#D2A8FF"
        variable: "#79C0FF"
        keyword: "#FF7B72"
        type: "#7EE787"
        constant: "#79C0FF"

    # Hover tooltip
    hover:
      background: "#161B22"
      foreground: "#C9D1D9"
      border: "#30363D"
      code_background: "#0D1117" # Code blocks inside hover

    # Signature help
    signature:
      background: "#161B22"
      foreground: "#C9D1D9"
      parameter_active: "#58A6FF"

  # ─────────────────────────────────────────────────────────────
  # Search / Find
  # ─────────────────────────────────────────────────────────────
  search:
    input:
      background: "#0D1117"
      foreground: "#C9D1D9"
      border:
        normal: "#30363D"
        focused: "#58A6FF"
      placeholder: "#484F58"

    match:
      background: "#6E3914" # Find match highlight
      border: "#D29922"
      current: # Currently selected match
        background: "#9E6A03"
        border: "#F0883E"

    results:
      foreground: "#8B949E"
      match_count: "#58A6FF"

  # ─────────────────────────────────────────────────────────────
  # Diagnostics (Errors, Warnings, etc.)
  # ─────────────────────────────────────────────────────────────
  diagnostics:
    error:
      foreground: "#F85149"
      background: "#F8514926" # Subtle background tint
      underline: "#F85149"
      border: "#F85149"
    warning:
      foreground: "#D29922"
      background: "#D2992226"
      underline: "#D29922"
      border: "#D29922"
    info:
      foreground: "#58A6FF"
      background: "#58A6FF26"
      underline: "#58A6FF"
      border: "#58A6FF"
    hint:
      foreground: "#8B949E"
      background: "#8B949E26"
      underline: "#8B949E"
      border: "#8B949E"

  # ─────────────────────────────────────────────────────────────
  # Overlay (Command Palette, Go to Line, Find/Replace modals)
  # ─────────────────────────────────────────────────────────────
  overlay:
    background: "#2B2D30"          # Modal background
    foreground: "#E0E0E0"          # Default text color
    border: "#43454A"              # Modal border
    input_background: "#1E1E1E"    # Input field background
    selection_background: "#264F78" # Selected item in list
    highlight: "#80FF80"           # Success/cursor color
    warning: "#FFFF80"             # Warning indicator
    error: "#FF8080"               # Error indicator

  # ─────────────────────────────────────────────────────────────
  # Tabs (if you add tab bar)
  # ─────────────────────────────────────────────────────────────
  tab:
    bar:
      background: "#010409"
      border: "#21262D"
    item:
      background:
        normal: "transparent"
        hover: "#161B22"
        active: "#0D1117" # Selected tab
      foreground:
        normal: "#8B949E"
        hover: "#C9D1D9"
        active: "#C9D1D9"
      border:
        active: "#F78166" # Accent line on selected tab
      modified_indicator: "#D29922" # Dot for unsaved changes
      close_button:
        foreground:
          normal: "#484F58"
          hover: "#C9D1D9"
        background:
          hover: "#30363D"

# ═══════════════════════════════════════════════════════════════
# Syntax Highlighting
# ═══════════════════════════════════════════════════════════════
syntax:
  # Each entry has: foreground, background (optional), font_style (optional)
  # font_style is an array: ["bold", "italic", "underline", "strikethrough"]

  # ─────────────────────────────────────────────────────────────
  # Comments
  # ─────────────────────────────────────────────────────────────
  comment:
    foreground: "#8B949E"
    font_style: ["italic"]

  comment_doc:
    foreground: "#8B949E"
    font_style: ["italic"]

  # ─────────────────────────────────────────────────────────────
  # Keywords
  # ─────────────────────────────────────────────────────────────
  keyword:
    foreground: "#FF7B72"

  keyword_control: # if, else, for, while, return, etc.
    foreground: "#FF7B72"

  keyword_operator: # and, or, not, in, etc.
    foreground: "#FF7B72"

  keyword_declaration: # fn, let, const, struct, enum, etc.
    foreground: "#FF7B72"

  keyword_modifier: # pub, mut, static, async, etc.
    foreground: "#FF7B72"

  # ─────────────────────────────────────────────────────────────
  # Types
  # ─────────────────────────────────────────────────────────────
  type:
    foreground: "#7EE787"

  type_builtin: # i32, str, bool, etc.
    foreground: "#7EE787"

  type_parameter: # Generic type parameters <T>
    foreground: "#7EE787"
    font_style: ["italic"]

  # ─────────────────────────────────────────────────────────────
  # Functions
  # ─────────────────────────────────────────────────────────────
  function:
    foreground: "#D2A8FF"

  function_builtin: # println!, vec!, etc.
    foreground: "#D2A8FF"

  function_method: # Method calls
    foreground: "#D2A8FF"

  # ─────────────────────────────────────────────────────────────
  # Variables & Parameters
  # ─────────────────────────────────────────────────────────────
  variable:
    foreground: "#C9D1D9"

  variable_builtin: # self, super, crate
    foreground: "#FF7B72"

  parameter:
    foreground: "#FFA657"
    font_style: ["italic"]

  property: # Struct fields, object properties
    foreground: "#79C0FF"

  # ─────────────────────────────────────────────────────────────
  # Literals
  # ─────────────────────────────────────────────────────────────
  string:
    foreground: "#A5D6FF"

  string_escape: # \n, \t, etc.
    foreground: "#7EE787"

  string_regex: # Regular expressions
    foreground: "#7EE787"

  string_interpolation: # ${}, f-strings, etc.
    foreground: "#79C0FF"

  number:
    foreground: "#79C0FF"

  boolean:
    foreground: "#79C0FF"

  constant: # Named constants, enum variants
    foreground: "#79C0FF"

  # ─────────────────────────────────────────────────────────────
  # Operators & Punctuation
  # ─────────────────────────────────────────────────────────────
  operator:
    foreground: "#FF7B72"

  punctuation:
    foreground: "#C9D1D9"

  punctuation_bracket: # (), [], {}
    foreground: "#C9D1D9"

  punctuation_delimiter: # , ; :
    foreground: "#C9D1D9"

  # ─────────────────────────────────────────────────────────────
  # Special
  # ─────────────────────────────────────────────────────────────
  macro:
    foreground: "#7EE787"
    font_style: ["bold"]

  attribute: # #[derive], @decorator, etc.
    foreground: "#7EE787"

  namespace: # Module names, imports
    foreground: "#FFA657"

  label: # Loop labels, goto labels
    foreground: "#FFA657"

  # ─────────────────────────────────────────────────────────────
  # Markup (for markdown, etc.)
  # ─────────────────────────────────────────────────────────────
  markup_heading:
    foreground: "#79C0FF"
    font_style: ["bold"]

  markup_bold:
    foreground: "#C9D1D9"
    font_style: ["bold"]

  markup_italic:
    foreground: "#C9D1D9"
    font_style: ["italic"]

  markup_link:
    foreground: "#58A6FF"
    font_style: ["underline"]

  markup_code:
    foreground: "#A5D6FF"
    background: "#161B22"

  # ─────────────────────────────────────────────────────────────
  # Diagnostics in code
  # ─────────────────────────────────────────────────────────────
  diagnostic_error:
    foreground: "#F85149"
    font_style: ["underline"]

  diagnostic_warning:
    foreground: "#D29922"
    font_style: ["underline"]

  diagnostic_deprecated:
    foreground: "#8B949E"
    font_style: ["strikethrough"]

  # ─────────────────────────────────────────────────────────────
  # Invalid / Error tokens
  # ─────────────────────────────────────────────────────────────
  invalid:
    foreground: "#FFFFFF"
    background: "#F8514966"
```

---

## Example Theme: Fleet Dark

Based on actual JetBrains Fleet colors:

```yaml
version: 1
name: "Fleet Dark"
author: "Built-in"
description: "JetBrains Fleet-inspired dark theme"

ui:
  window:
    background: "#181818"
    border: "#2B2B2B"

  editor:
    background:
      normal: "#181818"
      focused: "#181818"
    foreground: "#BCBEC4"
    current_line:
      background: "#1E1E1E"
    invisibles: "#3E3E42"
    selection:
      background:
        normal: "#214283"
        inactive: "#2D4963"
    cursor:
      pipe:
        color: "#FFFEF8"
        width: 2
      block:
        background: "#FFFEF8"
        foreground: "#181818"
    indent_guide:
      normal: "#2B2B2B"
      active: "#3C3C3C"
    line_number:
      foreground: "#606366"
      foreground_active: "#A1A3AB"

  gutter:
    background:
      normal: "#181818"
    separator: "#2B2B2B"
    line_number:
      foreground: "#606366"
      foreground_active: "#A1A3AB"
    diff:
      added: "#4A9F4D"
      modified: "#527BB2"
      deleted: "#C75450"

  scrollbar:
    track:
      background: "#181818"
    thumb:
      background:
        normal: "#3C3C3C"
        hover: "#4A4A4A"
        active: "#5A5A5A"

  status_bar:
    background: "#1E1E1E"
    foreground: "#787A80"
    border: "#2B2B2B"

  popup:
    background: "#252526"
    foreground: "#BCBEC4"
    border: "#3C3C3C"
    shadow: "#00000066"
    autocomplete:
      item:
        background:
          selected: "#04395E"
        foreground:
          selected: "#FFFFFF"
      match_highlight: "#79B8FF"

  diagnostics:
    error:
      foreground: "#F75464"
      underline: "#F75464"
    warning:
      foreground: "#E9AA5C"
      underline: "#E9AA5C"
    info:
      foreground: "#6CB3EB"
      underline: "#6CB3EB"

syntax:
  comment:
    foreground: "#7A7E85"
    font_style: ["italic"]

  keyword:
    foreground: "#CF8E6D"

  keyword_control:
    foreground: "#CF8E6D"

  keyword_declaration:
    foreground: "#CF8E6D"

  type:
    foreground: "#2FBAA3"

  type_builtin:
    foreground: "#2FBAA3"

  function:
    foreground: "#57AAF7"

  function_builtin:
    foreground: "#57AAF7"

  variable:
    foreground: "#BCBEC4"

  variable_builtin:
    foreground: "#CF8E6D"

  parameter:
    foreground: "#BCBEC4"

  property:
    foreground: "#C77DBB"

  string:
    foreground: "#6AAB73"

  string_escape:
    foreground: "#CF8E6D"

  number:
    foreground: "#2AACB8"

  boolean:
    foreground: "#CF8E6D"

  constant:
    foreground: "#C77DBB"

  operator:
    foreground: "#BCBEC4"

  punctuation:
    foreground: "#BCBEC4"

  macro:
    foreground: "#57AAF7"

  attribute:
    foreground: "#BBB529"

  namespace:
    foreground: "#BCBEC4"
```

---

## Rust Integration

### Dependencies

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
```

### Core Types

```rust
use serde::Deserialize;

/// RGBA color (0-255 per channel)
#[derive(Debug, Clone, Copy, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

/// Parse "#RRGGBB" or "#RRGGBBAA"
fn parse_color(s: &str) -> Result<Color, String> {
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => Ok(Color {
            r: u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?,
            g: u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?,
            b: u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?,
            a: 255,
        }),
        8 => Ok(Color {
            r: u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?,
            g: u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?,
            b: u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?,
            a: u8::from_str_radix(&s[6..8], 16).map_err(|e| e.to_string())?,
        }),
        _ => Err(format!("Invalid color format: {}", s)),
    }
}

/// Stateful color (for components with hover/active/etc. states)
#[derive(Debug, Clone, Deserialize)]
pub struct StatefulColor {
    pub normal: String,
    pub hover: Option<String>,
    pub active: Option<String>,
    pub focused: Option<String>,
    pub disabled: Option<String>,
}

impl StatefulColor {
    pub fn resolve(&self, state: UiState) -> Color {
        let hex = match state {
            UiState::Hover => self.hover.as_ref().unwrap_or(&self.normal),
            UiState::Active => self.active.as_ref().unwrap_or(&self.normal),
            UiState::Focused => self.focused.as_ref().unwrap_or(&self.normal),
            UiState::Disabled => self.disabled.as_ref().unwrap_or(&self.normal),
            UiState::Normal => &self.normal,
        };
        parse_color(hex).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UiState {
    Normal,
    Hover,
    Active,
    Focused,
    Disabled,
}

/// Syntax style entry
#[derive(Debug, Clone, Deserialize)]
pub struct SyntaxStyle {
    pub foreground: String,
    pub background: Option<String>,
    #[serde(default)]
    pub font_style: Vec<String>,
}

impl SyntaxStyle {
    pub fn foreground_color(&self) -> Color {
        parse_color(&self.foreground).unwrap_or_default()
    }

    pub fn is_bold(&self) -> bool {
        self.font_style.iter().any(|s| s == "bold")
    }

    pub fn is_italic(&self) -> bool {
        self.font_style.iter().any(|s| s == "italic")
    }
}
```

### Loading Themes

```rust
use std::fs;
use std::path::Path;

pub fn load_theme(path: &Path) -> Result<Theme, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let theme: Theme = serde_yaml::from_str(&content)?;
    Ok(theme)
}

// In your Model/AppModel
pub struct AppModel {
    // ... other fields
    pub theme: Theme,
}

// At startup
let theme = load_theme(Path::new("themes/fleet-dark.yaml"))
    .unwrap_or_else(|_| Theme::default_dark());
```

### Using in Renderer

```rust
// Before (hardcoded)
const BACKGROUND: u32 = 0xFF1E1E1E;
ctx.fill_rect(0, 0, width, height, BACKGROUND);

// After (themed)
let bg = theme.ui.editor.background.resolve(UiState::Normal);
ctx.fill_rect(0, 0, width, height, bg.to_argb_u32());

// Syntax highlighting
let token_style = theme.syntax.style_for(SyntaxRole::Keyword);
ctx.set_color(token_style.foreground_color().to_argb_u32());
if token_style.is_bold() {
    ctx.set_font_weight(FontWeight::Bold);
}
```

---

## Architecture Integration

Theming fits into the recommended architecture as part of `AppModel`:

```rust
struct AppModel {
    documents: HashMap<DocumentId, Document>,
    editors: HashMap<EditorId, EditorState>,
    ui: UiState,
    debug: DebugState,
    theme: Theme,  // ← Theme lives here
}

enum Msg {
    // ...
    Theme(ThemeMsg),
}

enum ThemeMsg {
    Load(PathBuf),
    LoadCompleted(Result<Theme, String>),
    Reload,
}

enum Cmd {
    // ...
    LoadTheme { path: PathBuf },
}
```

---

## File Locations

```
~/.config/your-editor/
├── config.yaml           # General settings
└── themes/
    ├── fleet-dark.yaml   # Built-in
    ├── github-dark.yaml  # Built-in
    └── my-theme.yaml     # User-created
```

---

## Future Enhancements (Not Required Now)

1. **Color palette/aliases**: Define `palette.accent` and reference it
2. **Theme inheritance**: `extends: "fleet-dark"` for variants
3. **Light theme support**: Automatic contrast adjustments
4. **Hot reloading**: Watch theme file for changes
5. **VS Code theme import**: Parse `.json` themes from VS Code

---

## Summary

| Aspect       | Decision                                                                   |
| ------------ | -------------------------------------------------------------------------- |
| **Format**   | YAML                                                                       |
| **Naming**   | snake_case, semantic, hierarchical                                         |
| **States**   | Nested under component: `normal`, `hover`, `active`, `focused`, `disabled` |
| **Syntax**   | Role-based: `keyword`, `function`, `type`, etc.                            |
| **Parsing**  | `serde_yaml` → Rust structs                                                |
| **Storage**  | `theme: Theme` in `AppModel`                                               |
| **Messages** | `ThemeMsg::Load`, `ThemeMsg::LoadCompleted`                                |

This gives you a pragmatic, extensible theming system that covers all your UI needs while remaining simple to edit and maintain.
