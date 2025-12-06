# Status Bar Design

A structured, segment-based status bar system replacing the current pipe-delimited string approach.

---

## Overview

### Current Problems

1. **String concatenation** - Status bar is built via `format!()` with `|` separators
2. **Redundant info** - `status_message` often duplicates filename/state shown elsewhere
3. **No alignment control** - Everything left-aligned, no right-side segments
4. **No per-segment theming** - Single foreground color for all text
5. **No interactivity** - Segments can't be clicked or hovered

### Goals

- **Structured segments** with defined positions (left, center, right)
- **Typed segment content** (text, icon, clickable)
- **Per-segment theming** with hover states
- **Clean separation** of concerns (file info, cursor info, status messages, mode indicators)
- **Extensible** for future features (encoding, line ending, language mode)

---

## Architecture

### Data Structures

```rust
/// A segment in the status bar
#[derive(Debug, Clone)]
pub struct StatusSegment {
    /// Unique identifier for updates
    pub id: SegmentId,
    /// Content to display
    pub content: SegmentContent,
    /// Priority for overflow (higher = keep visible longer)
    pub priority: u8,
    /// Minimum width in characters (0 = flexible)
    pub min_width: usize,
    /// Click action (if interactive)
    pub on_click: Option<SegmentAction>,
}

/// Segment identifier for targeted updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentId {
    FileName,
    ModifiedIndicator,
    CursorPosition,
    LineCount,
    Selection,       // "42 chars" or "5 lines" when text selected
    Encoding,        // UTF-8, UTF-16, etc.
    LineEnding,      // LF, CRLF
    Language,        // Rust, Python, etc.
    GitBranch,       // Current branch
    StatusMessage,   // Transient messages ("Saved", "Loading...")
    EditorMode,      // Normal, Insert, Visual (for vim-style)
    Custom(u32),     // For plugins/extensions
}

/// What to display in a segment
#[derive(Debug, Clone)]
pub enum SegmentContent {
    Text(String),
    Icon { name: String, fallback: String },  // Icon with text fallback
    TextWithIcon { icon: String, text: String },
    Empty,  // Hidden/placeholder
}

/// Action when segment is clicked
#[derive(Debug, Clone)]
pub enum SegmentAction {
    OpenFilePicker,
    ToggleLineEnding,
    ChangeEncoding,
    ShowGitMenu,
    Custom(String),  // Named action for extensibility
}

/// Status bar layout
#[derive(Debug, Clone)]
pub struct StatusBar {
    /// Segments aligned to the left
    pub left: Vec<StatusSegment>,
    /// Segments in the center (optional)
    pub center: Vec<StatusSegment>,
    /// Segments aligned to the right
    pub right: Vec<StatusSegment>,
    /// Spacing between segments in character units (includes 1px separator line)
    pub separator_spacing: usize,
    /// Padding on each side in character units
    pub padding: usize,
}
```

### Default Layout

```
┌────────────────────────────────────────────────────────────────────────────┐
│ LEFT                              CENTER                             RIGHT │
│ [FileName][Modified]                                    [Ln:Col][Lines][…] │
└────────────────────────────────────────────────────────────────────────────┘

Expanded:
┌────────────────────────────────────────────────────────────────────────────┐
│  main.rs [+]                                      Ln 42, Col 15 │ 1,234 Ln │
└────────────────────────────────────────────────────────────────────────────┘
```

### Segment Definitions

```rust
impl StatusBar {
    pub fn default_layout() -> Self {
        Self {
            left: vec![
                StatusSegment {
                    id: SegmentId::FileName,
                    content: SegmentContent::Text("Untitled".into()),
                    priority: 100,  // Always show
                    min_width: 0,
                    on_click: Some(SegmentAction::OpenFilePicker),
                },
                StatusSegment {
                    id: SegmentId::ModifiedIndicator,
                    content: SegmentContent::Empty,  // Shows "[+]" when modified
                    priority: 90,
                    min_width: 0,
                    on_click: None,
                },
                StatusSegment {
                    id: SegmentId::StatusMessage,
                    content: SegmentContent::Empty,  // Transient messages
                    priority: 50,
                    min_width: 0,
                    on_click: None,
                },
            ],
            center: vec![],  // Empty by default
            right: vec![
                StatusSegment {
                    id: SegmentId::Selection,
                    content: SegmentContent::Empty,  // Shows when text selected
                    priority: 40,
                    min_width: 0,
                    on_click: None,
                },
                StatusSegment {
                    id: SegmentId::CursorPosition,
                    content: SegmentContent::Text("Ln 1, Col 1".into()),
                    priority: 80,
                    min_width: 12,
                    on_click: None,
                },
                StatusSegment {
                    id: SegmentId::LineCount,
                    content: SegmentContent::Text("1 Ln".into()),
                    priority: 60,
                    min_width: 6,
                    on_click: None,
                },
                StatusSegment {
                    id: SegmentId::Encoding,
                    content: SegmentContent::Text("UTF-8".into()),
                    priority: 30,
                    min_width: 0,
                    on_click: Some(SegmentAction::ChangeEncoding),
                },
                StatusSegment {
                    id: SegmentId::LineEnding,
                    content: SegmentContent::Text("LF".into()),
                    priority: 20,
                    min_width: 0,
                    on_click: Some(SegmentAction::ToggleLineEnding),
                },
            ],
            separator_spacing: 2,  // 1 char margin on each side of the 1px line
            padding: 2,
        }
    }
}
```

---

## Integration with Existing Architecture

### UiState Changes

```rust
// In model/ui.rs
pub struct UiState {
    /// Status bar with structured segments
    pub status_bar: StatusBar,

    /// Transient message with auto-clear timer
    pub transient_message: Option<TransientMessage>,

    // ... existing fields
    pub cursor_visible: bool,
    pub last_cursor_blink: Instant,
    pub is_loading: bool,
    pub is_saving: bool,
}

pub struct TransientMessage {
    pub text: String,
    pub expires_at: Instant,
    pub style: MessageStyle,
}

pub enum MessageStyle {
    Info,
    Success,
    Warning,
    Error,
}
```

### New Messages

```rust
// In messages.rs
pub enum UiMsg {
    // ... existing
    SetStatus(String),
    BlinkCursor,

    // New segment-based updates
    UpdateSegment { id: SegmentId, content: SegmentContent },
    SetTransientMessage { text: String, duration_ms: u64, style: MessageStyle },
    ClearTransientMessage,
    SegmentClicked(SegmentId),
}
```

### Update Functions

```rust
// In update.rs
pub fn update_ui(model: &mut AppModel, msg: UiMsg) -> Option<Cmd> {
    match msg {
        UiMsg::UpdateSegment { id, content } => {
            model.ui.status_bar.update_segment(id, content);
            Some(Cmd::Redraw)
        }

        UiMsg::SetTransientMessage { text, duration_ms, style } => {
            model.ui.transient_message = Some(TransientMessage {
                text,
                expires_at: Instant::now() + Duration::from_millis(duration_ms),
                style,
            });
            // Update the StatusMessage segment
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Text(text.clone()),
            );
            Some(Cmd::Redraw)
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Empty,
            );
            Some(Cmd::Redraw)
        }

        // ... other handlers
    }
}

// Auto-update segments after document/cursor changes
pub fn sync_status_bar(model: &mut AppModel) {
    let cursor = model.editor.cursor();

    // Cursor position
    model.ui.status_bar.update_segment(
        SegmentId::CursorPosition,
        SegmentContent::Text(format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1)),
    );

    // Line count
    model.ui.status_bar.update_segment(
        SegmentId::LineCount,
        SegmentContent::Text(format!("{} Ln", model.document.line_count())),
    );

    // File name
    let file_name = model.document.file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");
    model.ui.status_bar.update_segment(
        SegmentId::FileName,
        SegmentContent::Text(file_name.into()),
    );

    // Modified indicator
    model.ui.status_bar.update_segment(
        SegmentId::ModifiedIndicator,
        if model.document.is_modified {
            SegmentContent::Text("[+]".into())
        } else {
            SegmentContent::Empty
        },
    );
}
```

---

## Rendering

### Layout Algorithm

```rust
impl StatusBar {
    /// Calculate segment positions for rendering
    /// Returns layout with segment positions and separator line positions (in char units)
    pub fn layout(&self, available_width: usize) -> StatusBarLayout {
        let mut left_x = self.padding;
        let mut left_segments = Vec::new();
        let mut separator_positions = Vec::new();

        // Track previous segment end for separator placement
        let mut prev_segment_end: Option<usize> = None;

        for seg in self.left.iter() {
            if let SegmentContent::Empty = seg.content {
                continue;
            }

            if let Some(prev_end) = prev_segment_end {
                // Add separator spacing and record separator center position
                left_x = prev_end + self.separator_spacing;
                let sep_center = prev_end + self.separator_spacing / 2;
                separator_positions.push(sep_center);
            }

            let width = seg.content.char_width();
            left_segments.push(RenderedSegment {
                id: seg.id,
                x: left_x,
                width,
                content: seg.content.clone(),
            });
            prev_segment_end = Some(left_x + width);
        }

        // Render right segments (from right edge, backwards)
        let mut right_x = available_width - self.padding;
        let mut right_segments = Vec::new();
        let mut right_separators = Vec::new();
        let mut prev_segment_start: Option<usize> = None;

        for seg in self.right.iter().rev() {
            if let SegmentContent::Empty = seg.content {
                continue;
            }

            let width = seg.content.char_width();

            if let Some(prev_start) = prev_segment_start {
                // Add separator spacing and record separator center position
                right_x = prev_start - self.separator_spacing;
                let sep_center = prev_start - self.separator_spacing / 2;
                right_separators.push(sep_center);
            }

            right_x -= width;
            right_segments.push(RenderedSegment {
                id: seg.id,
                x: right_x,
                width,
                content: seg.content.clone(),
            });
            prev_segment_start = Some(right_x);
        }
        right_segments.reverse();
        separator_positions.extend(right_separators);

        StatusBarLayout {
            left: left_segments,
            center: vec![],  // TODO: center alignment
            right: right_segments,
            separator_positions,
        }
    }
}

struct RenderedSegment {
    id: SegmentId,
    x: usize,  // Character position
    width: usize,
    content: SegmentContent,
}

struct StatusBarLayout {
    left: Vec<RenderedSegment>,
    center: Vec<RenderedSegment>,
    right: Vec<RenderedSegment>,
    /// Positions of separator lines in character units (center of spacing)
    separator_positions: Vec<usize>,
}
```

### Theme Integration

```rust
// In theme.rs - extend StatusBarTheme
pub struct StatusBarTheme {
    pub background: Color,
    pub foreground: Color,
    /// Color for the 1px vertical separator lines between segments
    pub separator_color: Color,

    // Per-segment type colors
    pub file_name: SegmentStyle,
    pub modified_indicator: SegmentStyle,
    pub cursor_position: SegmentStyle,
    pub transient_message: TransientMessageStyles,
}

pub struct SegmentStyle {
    pub foreground: Color,
    pub background: Option<Color>,  // For highlighted segments
    pub foreground_hover: Option<Color>,
}

pub struct TransientMessageStyles {
    pub info: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}
```

### Separator Rendering

The separator is rendered as a 1px vertical line, similar to the gutter border:

```rust
// In Renderer::render_status_bar()
fn render_separators(&self, layout: &StatusBarLayout, buffer: &mut [u32], width: usize, status_y: usize, status_height: usize) {
    let sep_color = self.theme.status_bar.separator_color.to_argb_u32();
    let char_width = self.char_width;

    // Vertical inset for aesthetics (don't span full height)
    let y_start = status_y + 4;
    let y_end = status_y + status_height - 4;

    for &sep_char_x in &layout.separator_positions {
        let x_px = (sep_char_x as f32 * char_width).round() as usize;

        // Draw 1px vertical line
        for py in y_start..y_end {
            if x_px < width {
                buffer[py * width + x_px] = sep_color;
            }
        }
    }
}
```

This approach:

- Uses character-unit spacing (`separator_spacing: 2` = 1 char margin on each side)
- Draws a single pixel-wide line at the center of the spacing
- Matches the existing gutter border rendering pattern
- Scales correctly with font size and DPI

---

## Implementation Plan

### Phase 1: Core Data Structures

- [ ] Add `StatusSegment`, `SegmentId`, `SegmentContent` to `model/ui.rs`
- [ ] Add `StatusBar` struct with `left`/`center`/`right` segments
- [ ] Add `StatusBar::default_layout()`
- [ ] Replace `status_message: String` with `status_bar: StatusBar`

### Phase 2: Update Integration

- [ ] Add `UiMsg::UpdateSegment` message
- [ ] Add `sync_status_bar()` function called after document/cursor changes
- [ ] Add transient message support with expiry

### Phase 3: Rendering

- [ ] Implement `StatusBar::layout()`
- [ ] Update `Renderer::render()` to use layout
- [ ] Add separator rendering between segments

### Phase 4: Theming

- [ ] Extend `StatusBarTheme` with per-segment colors
- [ ] Update default themes with segment colors
- [ ] Add hover state support (future: mouse tracking)

### Phase 5: Interactivity (Future)

- [ ] Track mouse position over status bar
- [ ] Emit `SegmentClicked` messages
- [ ] Implement click handlers (file picker, encoding menu, etc.)

---

## Example Rendering

Before (current):

```
 main.rs [+]  |  Ln 42, Col 15  |  1234 lines  |  Saved
```

After (structured with 1px separator lines):

```
┌────────────────────────────────────────────────────────────────────────────┐
│  main.rs [+]  Saved                          Ln 42, Col 15 │ 1,234 Ln      │
│              ↑                                             ↑               │
│         (no separator between                        (1px vertical line    │
│          left segments)                               between right        │
│                                                       segments)            │
└────────────────────────────────────────────────────────────────────────────┘

Legend: │ = 1px rendered line (not a pipe character)
```

With selection active:

```
│  main.rs [+]                        (42 chars) │ Ln 42, Col 15 │ 1,234 Ln  │
```

With error message:

```
│  main.rs [+]  ⚠ Error: file not found               Ln 1, Col 1 │ 0 Ln     │
│               └─ styled red ─────────┘                                     │
```
