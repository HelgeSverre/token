# Whitespace Management (F-090)

Tools for visualizing, converting, and cleaning up whitespace in documents.

> **Status:** 📋 Planned
> **Priority:** P3 (Nice-to-have)
> **Effort:** M (3-5 days)
> **Created:** 2025-12-19
> **Updated:** 2025-12-19
> **Milestone:** 2 - Search & Editing

---

## Overview

Whitespace management provides tools to visualize invisible characters, convert between tabs and spaces, and trim unnecessary whitespace. These features help maintain consistent code formatting and catch whitespace-related issues.

---

## Features

### 1. Whitespace Visualization

Show invisible characters (spaces, tabs, newlines) with visible markers.

#### Display Modes

| Mode | Description |
|------|-------------|
| None | No whitespace visualization (default) |
| Selection | Show whitespace only in selected text |
| Trailing | Show only trailing whitespace |
| All | Show all whitespace characters |

#### Visual Markers

| Character | Symbol | Description |
|-----------|--------|-------------|
| Space | `·` (U+00B7) | Middle dot |
| Tab | `→` (U+2192) | Right arrow |
| Newline | `↵` (U+21B5) | Carriage return symbol |
| NBSP | `°` (U+00B0) | Degree symbol |

#### Theme Integration

```yaml
# themes/dark.yaml
whitespace:
  marker: "#4a4a4a"        # Subdued color for markers
  trailing_bg: "#3a2020"   # Background for trailing whitespace
```

### 2. Tab/Space Conversion

Convert between tabs and spaces while preserving alignment.

#### Commands

| Command | Description |
|---------|-------------|
| Convert Indentation to Spaces | Replace leading tabs with spaces |
| Convert Indentation to Tabs | Replace leading spaces with tabs |
| Convert All Tabs to Spaces | Replace all tabs (not just leading) |
| Convert All Spaces to Tabs | Replace all space runs with tabs |

#### Algorithm: Tabs to Spaces

```rust
fn convert_tabs_to_spaces(doc: &mut Document, tab_size: usize) {
    let text = doc.buffer.to_string();
    let mut result = String::new();
    let mut column = 0;

    for ch in text.chars() {
        match ch {
            '\t' => {
                // Calculate spaces to next tab stop
                let spaces = tab_size - (column % tab_size);
                result.push_str(&" ".repeat(spaces));
                column += spaces;
            }
            '\n' => {
                result.push(ch);
                column = 0;
            }
            _ => {
                result.push(ch);
                column += 1;
            }
        }
    }

    doc.replace_all(&result);
}
```

### 3. Trailing Whitespace

Handle whitespace at the end of lines.

#### Commands

| Command | Binding | Description |
|---------|---------|-------------|
| Trim Trailing Whitespace | - | Remove trailing spaces/tabs from all lines |
| Trim Trailing on Save | Setting | Auto-trim when saving |
| Highlight Trailing | Setting | Show trailing whitespace with background color |

#### Algorithm: Trim Trailing

```rust
fn trim_trailing_whitespace(doc: &mut Document) -> Vec<(usize, usize)> {
    let mut changes = Vec::new();

    for (line_idx, line) in doc.lines().enumerate() {
        let trimmed = line.trim_end();
        let trailing_len = line.len() - trimmed.len() - 1; // -1 for newline

        if trailing_len > 0 {
            changes.push((line_idx, trailing_len));
        }
    }

    // Apply changes in reverse order
    for (line, len) in changes.iter().rev() {
        let offset = doc.line_end_offset(*line) - len;
        doc.delete(offset, *len);
    }

    changes
}
```

### 4. Smart Whitespace

Intelligent whitespace handling during editing.

#### Features

| Feature | Description |
|---------|-------------|
| Auto-trim on line change | Remove trailing whitespace when leaving a line |
| Preserve indent on empty lines | Keep indentation when adding blank lines |
| Smart backspace | Delete to previous indent level |
| Smart home | Toggle between line start and first non-space |

---

## Data Structures

### Configuration

```rust
// src/config.rs

#[derive(Debug, Clone, Default)]
pub struct WhitespaceConfig {
    /// Whitespace visualization mode
    pub render_mode: WhitespaceRenderMode,

    /// Highlight trailing whitespace
    pub highlight_trailing: bool,

    /// Trim trailing whitespace on save
    pub trim_on_save: bool,

    /// Auto-trim when leaving a line
    pub auto_trim: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WhitespaceRenderMode {
    #[default]
    None,
    Selection,
    Trailing,
    All,
}
```

### Theme Extension

```rust
// src/theme.rs

#[derive(Debug, Clone)]
pub struct WhitespaceTheme {
    /// Color for whitespace markers
    pub marker: Color,

    /// Background color for trailing whitespace
    pub trailing_background: Color,
}
```

---

## Messages

```rust
// src/messages.rs

pub enum EditorMsg {
    /// Toggle whitespace visualization mode
    ToggleWhitespaceVisibility,
    /// Set specific whitespace mode
    SetWhitespaceMode(WhitespaceRenderMode),
}

pub enum DocumentMsg {
    /// Convert tabs to spaces in entire document
    ConvertTabsToSpaces,
    /// Convert spaces to tabs in entire document
    ConvertSpacesToTabs,
    /// Convert leading tabs to spaces only
    ConvertIndentationToSpaces,
    /// Convert leading spaces to tabs only
    ConvertIndentationToTabs,
    /// Trim trailing whitespace from all lines
    TrimTrailingWhitespace,
}
```

---

## Rendering

### Whitespace Markers

```rust
// src/view.rs

fn render_text_with_whitespace(
    frame: &mut Frame,
    painter: &mut TextPainter,
    text: &str,
    mode: WhitespaceRenderMode,
    selection: Option<&Selection>,
    theme: &WhitespaceTheme,
) {
    for (idx, ch) in text.chars().enumerate() {
        let should_show = match mode {
            WhitespaceRenderMode::None => false,
            WhitespaceRenderMode::All => true,
            WhitespaceRenderMode::Selection => {
                selection.map_or(false, |s| s.contains_offset(idx))
            }
            WhitespaceRenderMode::Trailing => {
                is_trailing_whitespace(text, idx)
            }
        };

        if should_show && ch.is_whitespace() {
            let marker = match ch {
                ' ' => '·',
                '\t' => '→',
                '\n' => '↵',
                _ => ch,
            };
            painter.draw_char(marker, theme.marker);
        } else {
            painter.draw_char(ch, text_color);
        }
    }
}
```

### Tab Width Calculation

```rust
fn visual_width(text: &str, tab_size: usize) -> usize {
    let mut width = 0;
    for ch in text.chars() {
        width += match ch {
            '\t' => tab_size - (width % tab_size),
            _ => 1,
        };
    }
    width
}
```

---

## Command Palette Integration

```yaml
# Commands available in palette
- id: whitespace.toggle
  title: "Toggle Whitespace Visibility"

- id: whitespace.showAll
  title: "Show All Whitespace"

- id: whitespace.showNone
  title: "Hide Whitespace"

- id: whitespace.convertTabsToSpaces
  title: "Convert Tabs to Spaces"

- id: whitespace.trimTrailing
  title: "Trim Trailing Whitespace"
```

---

## Settings

### Editor Config

```yaml
# ~/.config/token-editor/config.yaml
whitespace:
  # Visualization mode: none, selection, trailing, all
  render: "trailing"

  # Highlight trailing whitespace
  highlight_trailing: true

  # Trim on save
  trim_on_save: true

  # Tab size for conversion
  tab_size: 4

  # Use tabs vs spaces for indentation
  use_tabs: false
```

### Per-File Detection

Detect indentation style from file content:

```rust
fn detect_indentation(doc: &Document) -> IndentStyle {
    let mut tabs = 0;
    let mut spaces = 0;

    for line in doc.lines().take(100) {
        if line.starts_with('\t') {
            tabs += 1;
        } else if line.starts_with(' ') {
            spaces += 1;
        }
    }

    if tabs > spaces {
        IndentStyle::Tabs
    } else {
        IndentStyle::Spaces(detect_space_width(doc))
    }
}
```

---

## Testing

### Unit Tests

```rust
#[test]
fn test_tabs_to_spaces_conversion() {
    let input = "\tindented\n\t\tdouble";
    let expected = "    indented\n        double";
    assert_eq!(convert_tabs_to_spaces(input, 4), expected);
}

#[test]
fn test_trim_trailing_whitespace() {
    let input = "line1  \nline2\t\nline3";
    let expected = "line1\nline2\nline3";
    assert_eq!(trim_trailing(input), expected);
}

#[test]
fn test_visual_width_with_tabs() {
    assert_eq!(visual_width("\tfoo", 4), 7);  // Tab + 3 chars
    assert_eq!(visual_width("a\tb", 4), 5);   // 1 + 3 + 1
}
```

### Manual Testing Checklist

- [ ] Toggle whitespace visibility via command palette
- [ ] Whitespace markers render correctly
- [ ] Tab → spaces conversion preserves alignment
- [ ] Spaces → tabs conversion uses correct tab stops
- [ ] Trim trailing removes all trailing whitespace
- [ ] Trim on save works when enabled
- [ ] Per-file indentation detection works

---

## Implementation Plan

### Phase 1: Whitespace Visualization (3 days)

1. Add `WhitespaceConfig` and `WhitespaceRenderMode`
2. Extend theme with whitespace colors
3. Modify renderer to draw whitespace markers
4. Add toggle command to palette
5. Tests

### Phase 2: Conversion Commands (2 days)

1. Implement `ConvertTabsToSpaces`
2. Implement `ConvertSpacesToTabs`
3. Add to command palette
4. Undo/redo support
5. Tests

### Phase 3: Trailing Whitespace (2 days)

1. Implement `TrimTrailingWhitespace`
2. Add `trim_on_save` setting
3. Add trailing whitespace highlighting
4. Tests

### Phase 4: Smart Features (2 days)

1. Indentation detection
2. Smart backspace
3. Auto-trim on line change
4. Tests

---

## Dependencies

- **Theme System**: For whitespace marker colors
- **Command Palette**: For accessing commands
- **Settings System**: For persistence

---

## References

- [VS Code: Render Whitespace](https://code.visualstudio.com/docs/editor/codebasics#_whitespace-and-indentation)
- [EditorConfig](https://editorconfig.org/) - Standard for whitespace settings
- [VS Code settings](https://code.visualstudio.com/docs/getstarted/settings#_editor-whitespace) - `editor.renderWhitespace`
