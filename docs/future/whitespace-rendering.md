# Whitespace Rendering

Indent guides, tab markers, space markers, EOL markers, and trailing whitespace highlighting.

> **Status:** Partially implemented (indent guides shipped in v0.7.0; markers and trailing highlight still planned)
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 2 - Search & Editing

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

The editor renders text with basic whitespace handling:
- Tabs expanded to spaces for display
- **Indent guides shipped in v0.7.0** (`config.indent_guides` toggle, default on, in
  Settings > Appearance > Editor chrome; theme key `ui.editor.indent_guide`; rendered
  in `src/view/editor_text.rs`). Guide spacing is inferred from the document's
  indentation steps (2- and 4-space both work) rather than derived from `tab_size`.
  Guides render only in plain text view mode, are not extended through blank
  lines, and there is no active-guide highlight at the cursor level.
- No visual indicators for whitespace characters (tab/space/EOL markers)
- No trailing whitespace highlighting (only save-time `trim_trailing_whitespace`
  in `src/model/text_settings.rs`)

### Goals

1. **Indent guides** - Vertical lines showing indentation levels *(done, v0.7.0)*
2. **Tab markers** - Visual indicator for tab characters
3. **Space markers** - Optional dots for space characters
4. **EOL markers** - Optional newline/CR/CRLF indicators
5. **Trailing whitespace** - Highlight trailing spaces and tabs
6. **Configurable** - User can toggle each feature independently
7. **Theme support** - Colors from theme definition

### Non-Goals

- Word wrap guides (separate feature)
- Bracket pair guides (different from indent guides)
- Minimap whitespace (minimap is separate feature)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Whitespace Rendering System                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Visual Examples                                │ │
│  │                                                                         │ │
│  │  Indent Guides (vertical bars at indentation levels):                  │ │
│  │                                                                         │ │
│  │  fn main() {                                                           │ │
│  │  │   let x = 5;                                                        │ │
│  │  │   if x > 0 {                                                        │ │
│  │  │   │   println!("positive");                                         │ │
│  │  │   }                                                                 │ │
│  │  }                                                                     │ │
│  │                                                                         │ │
│  │  ─────────────────────────────────────────────────────────────────     │ │
│  │                                                                         │ │
│  │  Tab Markers (arrow pointing right):                                   │ │
│  │                                                                         │ │
│  │  →   indented with tab                                                 │ │
│  │  →   →   double tab                                                    │ │
│  │                                                                         │ │
│  │  ─────────────────────────────────────────────────────────────────     │ │
│  │                                                                         │ │
│  │  Space Markers (middle dots):                                          │ │
│  │                                                                         │ │
│  │  hello·world····                                                       │ │
│  │        ^     ^^^^                                                       │ │
│  │        space trailing spaces                                            │ │
│  │                                                                         │ │
│  │  ─────────────────────────────────────────────────────────────────     │ │
│  │                                                                         │ │
│  │  EOL Markers:                                                          │ │
│  │                                                                         │ │
│  │  Line with LF ending¶                                                  │ │
│  │  Line with CRLF ending↵                                                │ │
│  │                                                                         │ │
│  │  ─────────────────────────────────────────────────────────────────     │ │
│  │                                                                         │ │
│  │  Trailing Whitespace (highlighted background):                         │ │
│  │                                                                         │ │
│  │  Some text▓▓▓▓                                                         │ │
│  │           ^^^^                                                          │ │
│  │           trailing spaces with red/orange background                    │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Rendering Pipeline

```
┌───────────────────────────────────────────────────────────────────────────┐
│                          Render Line Pipeline                              │
│                                                                            │
│  For each visible line:                                                    │
│                                                                            │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                  │
│  │ 1. Compute  │────▶│ 2. Render   │────▶│ 3. Render   │                  │
│  │ Indentation │     │ Indent      │     │ Whitespace  │                  │
│  │ Level       │     │ Guides      │     │ Markers     │                  │
│  └─────────────┘     └─────────────┘     └─────────────┘                  │
│                                                 │                          │
│                                                 ▼                          │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                  │
│  │ 6. Apply    │◀────│ 5. Render   │◀────│ 4. Render   │                  │
│  │ Trailing WS │     │ EOL Marker  │     │ Text with   │                  │
│  │ Highlight   │     │             │     │ Syntax      │                  │
│  └─────────────┘     └─────────────┘     └─────────────┘                  │
│                                                                            │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Whitespace Configuration

> Indent guides already ship as a flat `indent_guides: bool` on the main config
> (`src/config.rs`, settings entry in `src/settings/catalog.rs`). Tab width comes
> from `text_settings`, and guide spacing is inferred, so the struct below only
> proposes the *new* keys for markers and trailing highlight.

```rust
// src/config.rs or src/model/mod.rs

use serde::{Deserialize, Serialize};

/// Configuration for whitespace markers and trailing highlight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitespaceConfig {
    /// Render tab markers (arrows)
    #[serde(default)]
    pub tab_markers: bool,

    /// When to render space markers
    #[serde(default)]
    pub space_markers: WhitespaceVisibility,

    /// Render EOL markers (newline symbols)
    #[serde(default)]
    pub eol_markers: bool,

    /// Highlight trailing whitespace
    #[serde(default = "default_true")]
    pub trailing_whitespace: bool,
}

/// When to show space markers
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum WhitespaceVisibility {
    /// Never show space markers
    #[default]
    None,
    /// Show only in selections
    Selection,
    /// Show only trailing spaces
    Trailing,
    /// Always show all spaces
    All,
}

fn default_true() -> bool {
    true
}

impl Default for WhitespaceConfig {
    fn default() -> Self {
        Self {
            indent_guides: true,
            tab_markers: false,
            space_markers: WhitespaceVisibility::None,
            eol_markers: false,
            trailing_whitespace: true,
        }
    }
}
```

### Theme Extensions

> `indent_guide` already exists as a plain field on the editor theme
> (`src/theme.rs`, YAML key `ui.editor.indent_guide`, see `docs/THEMES.md`), with a
> foreground/background-derived fallback for older custom themes. Only the keys
> below are new. `indent_guide_active` was not shipped; decide whether the
> active-guide highlight is still wanted before adding it.

```rust
// Add to src/theme.rs

/// Colors for whitespace rendering
#[derive(Debug, Clone)]
pub struct WhitespaceTheme {
    /// Color for tab markers
    pub tab_marker: Color,
    /// Color for space markers
    pub space_marker: Color,
    /// Color for EOL markers
    pub eol_marker: Color,
    /// Background color for trailing whitespace
    pub trailing_whitespace_bg: Color,
}

impl Default for WhitespaceTheme {
    fn default() -> Self {
        Self {
            tab_marker: Color::rgba(0x60, 0x60, 0x60, 0x80),
            space_marker: Color::rgba(0x60, 0x60, 0x60, 0x60),
            eol_marker: Color::rgba(0x60, 0x60, 0x60, 0x60),
            trailing_whitespace_bg: Color::rgba(0xFF, 0x00, 0x00, 0x30),
        }
    }
}
```

### Indent Guide Calculation

Shipped in v0.7.0 with a different design than originally proposed here: there
is no `IndentGuides` type or `src/view/whitespace.rs`. Guide columns are computed
in `src/view/editor_text.rs` (`indentation_columns`) from the document's observed
indentation steps, using the existing text-column geometry for spaces and tabs,
with horizontal clipping and no guides on soft-wrap continuation rows. See the
`indent_guides_*` tests in that file for the inference behavior.

### Whitespace Markers

```rust
// src/view/whitespace.rs

/// Unicode characters for whitespace markers
pub mod markers {
    /// Tab marker: rightwards arrow
    pub const TAB: char = '\u{2192}'; // →

    /// Space marker: middle dot
    pub const SPACE: char = '\u{00B7}'; // ·

    /// LF marker: pilcrow sign
    pub const LF: char = '\u{00B6}'; // ¶

    /// CRLF marker: return symbol
    pub const CRLF: char = '\u{21B5}'; // ↵

    /// CR marker: leftwards arrow
    pub const CR: char = '\u{2190}'; // ←
}

/// Represents whitespace to render on a line
#[derive(Debug, Clone)]
pub struct WhitespaceMarkers {
    /// Tab positions (column index)
    pub tabs: Vec<usize>,
    /// Space positions (column index)
    pub spaces: Vec<usize>,
    /// Trailing whitespace range (start_column, end_column)
    pub trailing: Option<(usize, usize)>,
    /// End of line type
    pub eol: Option<EolType>,
}

/// Type of line ending
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolType {
    Lf,
    Crlf,
    Cr,
}

impl WhitespaceMarkers {
    /// Analyze a line for whitespace
    pub fn analyze(
        line: &str,
        visibility: WhitespaceVisibility,
        selection_range: Option<(usize, usize)>,
    ) -> Self {
        let mut tabs = Vec::new();
        let mut spaces = Vec::new();
        let mut trailing_start = None;
        let mut column = 0;
        let mut last_non_ws_column = 0;

        // Detect EOL
        let eol = if line.ends_with("\r\n") {
            Some(EolType::Crlf)
        } else if line.ends_with('\n') {
            Some(EolType::Lf)
        } else if line.ends_with('\r') {
            Some(EolType::Cr)
        } else {
            None
        };

        // Strip EOL for analysis
        let content = line.trim_end_matches(['\r', '\n']);

        for c in content.chars() {
            let in_selection = selection_range
                .map(|(start, end)| column >= start && column < end)
                .unwrap_or(false);

            match c {
                '\t' => {
                    match visibility {
                        WhitespaceVisibility::All => tabs.push(column),
                        WhitespaceVisibility::Selection if in_selection => tabs.push(column),
                        WhitespaceVisibility::Trailing => {} // Will be added if trailing
                        WhitespaceVisibility::None => {}
                    }
                    column += 1; // Tab counts as 1 for column tracking
                }
                ' ' => {
                    match visibility {
                        WhitespaceVisibility::All => spaces.push(column),
                        WhitespaceVisibility::Selection if in_selection => spaces.push(column),
                        WhitespaceVisibility::Trailing => {} // Will be added if trailing
                        WhitespaceVisibility::None => {}
                    }
                    column += 1;
                }
                _ => {
                    last_non_ws_column = column + 1;
                    column += 1;
                }
            }
        }

        // Check for trailing whitespace
        let trailing = if last_non_ws_column < column {
            Some((last_non_ws_column, column))
        } else {
            None
        };

        // For trailing visibility, add trailing whitespace markers
        if visibility == WhitespaceVisibility::Trailing {
            if let Some((start, end)) = trailing {
                for col in start..end {
                    // Determine if tab or space (simplified - assumes spaces for trailing)
                    spaces.push(col);
                }
            }
        }

        Self {
            tabs,
            spaces,
            trailing,
            eol,
        }
    }
}
```

### Rendering Integration

```rust
// src/view/editor_text.rs (additions)

impl Renderer {
    /// Render whitespace indicators for a line
    pub fn render_whitespace(
        &mut self,
        line: &str,
        y: f32,
        text_start_x: f32,
        char_width: f32,
        config: &WhitespaceConfig,
        theme: &WhitespaceTheme,
        selection_range: Option<(usize, usize)>,
    ) {
        let markers = WhitespaceMarkers::analyze(line, config.space_markers, selection_range);

        // Render trailing whitespace background
        if config.trailing_whitespace {
            if let Some((start, end)) = markers.trailing {
                let x = text_start_x + start as f32 * char_width;
                let width = (end - start) as f32 * char_width;
                self.fill_rect(
                    x,
                    y,
                    width,
                    self.line_height as f32,
                    theme.trailing_whitespace_bg,
                );
            }
        }

        // Render tab markers
        if config.tab_markers {
            for &col in &markers.tabs {
                let x = text_start_x + col as f32 * char_width;
                self.draw_char(markers::TAB, x, y, theme.tab_marker);
            }
        }

        // Render space markers
        if config.space_markers != WhitespaceVisibility::None {
            for &col in &markers.spaces {
                let x = text_start_x + col as f32 * char_width;
                self.draw_char(markers::SPACE, x, y, theme.space_marker);
            }
        }

        // Render EOL marker
        if config.eol_markers {
            if let Some(eol) = markers.eol {
                let line_end_col = line.trim_end_matches(['\r', '\n']).chars().count();
                let x = text_start_x + line_end_col as f32 * char_width;
                let marker = match eol {
                    EolType::Lf => markers::LF,
                    EolType::Crlf => markers::CRLF,
                    EolType::Cr => markers::CR,
                };
                self.draw_char(marker, x, y, theme.eol_marker);
            }
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Toggle Indent Guides | - | - | Via settings or command palette |
| Toggle Whitespace | - | - | Via command palette |
| Toggle Trailing WS | - | - | Via command palette |

Note: These are typically configured in settings rather than keyboard shortcuts. Today the indent-guides toggle exists only in Settings (Appearance > Editor chrome); no toggle commands exist in `src/keymap/command.rs` yet (see Phase 8).

---

## Implementation Plan

### Phase 1: Configuration

**Files:** `src/config.rs`

- [x] `indent_guides` toggle (flat bool on main config, settings catalog entry) - v0.7.0
- [ ] Add `WhitespaceConfig` struct (markers + trailing highlight)
- [ ] Add `WhitespaceVisibility` enum
- [ ] Serialize/deserialize in config file
- [ ] Add defaults

**Test:** Config loads with default whitespace settings.

### Phase 2: Theme Extensions

**Files:** `src/theme.rs`, themes/*.yaml

- [x] `ui.editor.indent_guide` in all bundled themes, with fallback - v0.7.0
- [ ] Add `WhitespaceTheme` struct
- [ ] Add colors for marker/trailing elements
- [ ] Update theme files with whitespace section
- [ ] Add defaults for themes without whitespace

**Test:** Theme includes whitespace colors.

### Phase 3: Indent Guides

**Files:** `src/view/editor_text.rs`

- [x] Guide column calculation (inferred indent steps, not `IndentGuides::for_line`) - v0.7.0
- [x] Handle tabs and spaces correctly - v0.7.0
- [ ] Handle blank lines (extend guides) - not shipped; guides come only from a line's own leading whitespace

**Test:** `indent_guides_*` tests in `src/view/editor_text.rs`.

### Phase 4: Whitespace Markers

**Files:** `src/view/whitespace.rs`

- [ ] Create `WhitespaceMarkers` struct
- [ ] Implement `analyze()` method
- [ ] Handle visibility modes
- [ ] Detect EOL types

**Test:** Trailing whitespace detected correctly.

### Phase 5: Rendering - Trailing

**Files:** `src/view/editor_text.rs`

- [ ] Render trailing whitespace background
- [ ] Apply theme color
- [ ] Test with multiple lines

**Test:** Trailing whitespace has highlighted background.

### Phase 6: Rendering - Markers

**Files:** `src/view/editor_text.rs`

- [ ] Render tab arrows
- [ ] Render space dots
- [ ] Render EOL markers
- [ ] Position markers correctly

**Test:** Tab character shows arrow glyph.

### Phase 7: Rendering - Guides

**Files:** `src/view/editor_text.rs`

- [x] Render vertical indent lines - v0.7.0
- [ ] Highlight active guide at cursor level (optional, not shipped)
- [ ] Handle blank lines properly (extend guides through blank lines)
- [x] Smooth scrolling support - v0.7.0

**Test:** Indent guides align with code blocks.

### Phase 8: Commands

**Files:** `src/keymap/command.rs`

- [ ] Add toggle commands for each feature
- [ ] Update command palette display names
- [ ] Implement toggle handlers

**Test:** Toggle commands change config.

### Phase 9: Settings Persistence

**Files:** `src/config.rs`

- [x] `indent_guides` persists via config - v0.7.0
- [ ] Save marker/trailing preferences
- [ ] Load on startup
- [ ] Per-language overrides (optional)

**Test:** Whitespace settings persist across sessions.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_trailing() {
        let markers = WhitespaceMarkers::analyze("hello   ", WhitespaceVisibility::Trailing, None);
        assert_eq!(markers.trailing, Some((5, 8)));
    }

    #[test]
    fn test_whitespace_no_trailing() {
        let markers = WhitespaceMarkers::analyze("hello", WhitespaceVisibility::Trailing, None);
        assert_eq!(markers.trailing, None);
    }

    #[test]
    fn test_eol_detection_lf() {
        let markers = WhitespaceMarkers::analyze("hello\n", WhitespaceVisibility::None, None);
        assert_eq!(markers.eol, Some(EolType::Lf));
    }

    #[test]
    fn test_eol_detection_crlf() {
        let markers = WhitespaceMarkers::analyze("hello\r\n", WhitespaceVisibility::None, None);
        assert_eq!(markers.eol, Some(EolType::Crlf));
    }

    #[test]
    fn test_space_visibility_all() {
        let markers = WhitespaceMarkers::analyze("a b c", WhitespaceVisibility::All, None);
        assert_eq!(markers.spaces.len(), 2);
    }

    #[test]
    fn test_space_visibility_selection() {
        let markers = WhitespaceMarkers::analyze("a b c", WhitespaceVisibility::Selection, Some((1, 3)));
        assert_eq!(markers.spaces.len(), 1);
    }
}
```

### Integration Tests

```rust
// tests/whitespace_tests.rs

#[test]
fn test_render_trailing_whitespace() {
    // Create document with trailing whitespace
    // Render
    // Verify highlight appears at correct position
}

#[test]
fn test_indent_guides_consistency() {
    // Covered by indent_guides_* tests in src/view/editor_text.rs
    // Active-guide-follows-cursor only if Phase 7 highlight is picked up
}

#[test]
fn test_config_toggle() {
    // Toggle indent guides off
    // Verify guides not rendered
    // Toggle back on
    // Verify guides appear
}
```

---

## References

- **Theme system:** `src/theme.rs` - Color definitions
- **Rendering:** `src/view/editor_text.rs` - Text rendering and indent guides
- **Config:** `src/config.rs` - Editor configuration (`indent_guides`); `src/settings/catalog.rs` - Settings entry
- **Shipped design:** [indent-guides.md](../feature/indent-guides.md), `docs/CHANGELOG.md` v0.7.0
- **VS Code:** Whitespace rendering settings
- **Sublime Text:** Draw white space setting
- **Unicode:** Whitespace marker characters
