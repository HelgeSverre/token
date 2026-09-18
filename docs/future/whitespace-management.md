# Whitespace Management (F-090)

Tools for visualizing, converting, and cleaning up whitespace in documents.

> **Status:** 🚧 Partial — trim-on-save, indent style/size settings and smart Home shipped via the EditorConfig/text-settings work; visualization and conversion commands are not implemented
> **Priority:** P3 (Nice-to-have)
> **Effort:** M (3-5 days)
> **Created:** 2025-12-19
> **Updated:** 2026-09-17
> **Milestone:** 2 - Search & Editing

---

## Overview

Whitespace management provides tools to visualize invisible characters, convert between tabs and spaces, and trim unnecessary whitespace. These features help maintain consistent code formatting and catch whitespace-related issues.

---

## Features

### 1. Whitespace Visualization

Show invisible characters (spaces, tabs, newlines) with visible markers.

Display modes, marker glyphs, trailing highlighting and theme keys are specified in [whitespace-rendering.md](whitespace-rendering.md), which is the single home for marker rendering. Not implemented yet; `themes/*.yaml` have no whitespace keys.

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
| Trim Trailing Whitespace | - | Remove trailing spaces/tabs from all lines (on-demand command; not implemented) |
| Trim Trailing on Save | Setting | ✅ Implemented: EditorConfig `trim_trailing_whitespace` / `DocumentTextSettings.trim_trailing_whitespace`, applied as undoable save cleanup in `src/update/save_cleanup.rs` |
| Highlight Trailing | Setting | Show trailing whitespace with background color (not implemented) |

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
| Auto-trim on line change | Remove trailing whitespace when leaving a line (not implemented) |
| Preserve indent on empty lines | Keep indentation when adding blank lines |
| Smart backspace | Delete to previous indent level (not implemented) |
| Smart home | ✅ Implemented as `LineStartSmart` in `src/editable/messages.rs` |

---

## Data Structures

### Configuration

Indent style, indent size, tab width and trim-on-save already live in `DocumentTextSettings` (`src/model/text_settings.rs`, `IndentStyle::{Tab, Space}`), fed by EditorConfig and the `text.*` settings. The remaining proposed additions (types below do not exist yet):

```rust
// proposed, src/model/text_settings.rs or src/config.rs

#[derive(Debug, Clone, Default)]
pub struct WhitespaceConfig {
    /// Whitespace visualization mode
    pub render_mode: WhitespaceRenderMode,

    /// Highlight trailing whitespace
    pub highlight_trailing: bool,

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
// proposed, src/theme.rs

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
// proposed; none of these variants exist yet

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
// proposed, src/view/editor_text.rs

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

Existing (implemented): indentation and trim settings come from EditorConfig (`indent_style`, `indent_size`, `tab_width`, `trim_trailing_whitespace`) with `text.indent_style`, `text.indent_size` and `text.tab_width` in the settings catalog (`src/settings/catalog.rs`) as fallbacks when no EditorConfig rule applies. Conversion commands should read these rather than a separate `tab_size`/`use_tabs` pair.

Proposed additions for visualization:

```yaml
# ~/.config/token-editor/config.yaml
whitespace:
  # Visualization mode: none, selection, trailing, all
  render: "trailing"

  # Highlight trailing whitespace
  highlight_trailing: true
```

### Per-File Detection

Detect indentation style from file content when no EditorConfig rule applies. A tabs-vs-spaces heuristic already exists for completion insertions (`infer_style` in `src/completion/postprocess.rs`); the editor's `DocumentTextSettings` does not use it yet.

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
- [x] Trim on save works when enabled
- [ ] Per-file indentation detection works

---

## Implementation Plan

### Phase 1: Whitespace Visualization (3 days)

See [whitespace-rendering.md](whitespace-rendering.md); not started.

### Phase 2: Conversion Commands (2 days)

1. Implement `ConvertTabsToSpaces`
2. Implement `ConvertSpacesToTabs`
3. Add to command palette
4. Undo/redo support
5. Tests

### Phase 3: Trailing Whitespace (2 days)

1. Implement `TrimTrailingWhitespace` as an on-demand palette command
2. ~~Add `trim_on_save` setting~~ ✅ Done (`trim_trailing_whitespace` in `DocumentTextSettings`, `src/update/save_cleanup.rs`)
3. Add trailing whitespace highlighting
4. Tests

### Phase 4: Smart Features (2 days)

1. Per-file indentation detection for the editor (indent style/size settings ✅ exist; heuristic only in completion)
2. Smart backspace
3. Auto-trim on line change
4. Tests
5. ~~Smart home~~ ✅ Done (`LineStartSmart`)

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
