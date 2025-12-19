# Markdown Preview

Split view with source on left, rendered markdown on right with synchronized scrolling

> **Status:** Planning
> **Priority:** P2
> **Effort:** L
> **Created:** 2025-12-19
> **Milestone:** 5 - Insight Tools
> **Feature ID:** F-170

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Rendering](#rendering)
5. [Keybindings](#keybindings)
6. [Implementation Plan](#implementation-plan)
7. [Testing Strategy](#testing-strategy)
8. [References](#references)

---

## Overview

### Current State

The editor currently has:

- Syntax highlighting for Markdown files (tree-sitter-md)
- Split view infrastructure (`EditorArea`, `LayoutNode`, `SplitContainer`)
- CPU-based rendering with fontdue + softbuffer
- Theme system with customizable colors

However, there is no rendered preview of Markdown content. Users see only the raw Markdown source with syntax highlighting.

### Goals

1. **Live preview** - Render Markdown as formatted text in a preview pane
2. **Split view integration** - Preview appears as a special split alongside source
3. **Synchronized scrolling** - Scroll position synced between source and preview
4. **Basic formatting** - Headers, bold, italic, code, lists, blockquotes, links
5. **Theme integration** - Preview respects current editor theme colors
6. **Live updates** - Preview updates as user types (debounced)

### Non-Goals (This Phase)

- Image rendering (show placeholder or alt text)
- External link handling (no browser opening)
- Table rendering (show as plain text)
- LaTeX/math rendering
- Mermaid/diagram rendering
- Custom CSS styling
- Print/export to PDF

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Markdown Preview Architecture                        │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                         EditorArea Layout                             │  │
│  │  ┌────────────────────────┬────────────────────────┐                 │  │
│  │  │    Source Editor       │    Preview Pane        │                 │  │
│  │  │   (EditorGroup)        │   (PreviewPane)        │                 │  │
│  │  │                        │                        │                 │  │
│  │  │  # Heading             │  Heading               │                 │  │
│  │  │  Some **bold** text    │  Some bold text        │                 │  │
│  │  │                        │                        │                 │  │
│  │  │  - Item 1              │  - Item 1              │                 │  │
│  │  │  - Item 2              │  - Item 2              │                 │  │
│  │  │                        │                        │                 │  │
│  │  └────────────────────────┴────────────────────────┘                 │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                         Data Flow                                     │  │
│  │                                                                       │  │
│  │  Document.buffer ───▶ MarkdownParser ───▶ MarkdownAst ───▶ Renderer  │  │
│  │       │                    │                   │              │       │  │
│  │       │                    │                   │              ▼       │  │
│  │  (on edit)            (debounced)         (cached)        Preview    │  │
│  │                                                            Pane      │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── markdown/                    # NEW MODULE
│   ├── mod.rs                   # Public exports
│   ├── parser.rs                # Markdown → AST conversion (pulldown-cmark)
│   ├── ast.rs                   # MarkdownAst, MarkdownNode types
│   ├── layout.rs                # AST → LayoutLines conversion
│   └── render.rs                # Rendering formatted text
├── model/
│   ├── editor.rs                # + ViewMode::MarkdownPreview variant
│   └── preview.rs               # NEW: PreviewPane, PreviewState
├── update/
│   └── preview.rs               # NEW: Preview update handler
└── view/
    └── preview.rs               # NEW: Preview pane rendering
```

### Scroll Synchronization

```
Source Editor                    Preview Pane
┌──────────────┐                ┌──────────────┐
│              │                │              │
│   Line 1     │ ◄───────────► │   Para 1     │  (mapped via AST positions)
│   Line 2     │                │              │
│   Line 3     │                │   Para 2     │
│   ──────────►│ scroll         │   ──────────►│
│   Line 4     │                │   Para 3     │
│   Line 5     │                │              │
│              │                │              │
└──────────────┘                └──────────────┘

Mapping: Source line 3 (heading) → Preview element "Para 2" (rendered heading)
```

---

## Data Structures

### Markdown AST

```rust
// src/markdown/ast.rs

/// A node in the parsed Markdown AST
#[derive(Debug, Clone)]
pub enum MarkdownNode {
    /// Document root
    Document(Vec<MarkdownNode>),

    /// Heading with level (1-6) and inline content
    Heading {
        level: u8,
        content: Vec<InlineNode>,
        source_line: usize,
    },

    /// Paragraph with inline content
    Paragraph {
        content: Vec<InlineNode>,
        source_line: usize,
    },

    /// Code block with optional language
    CodeBlock {
        language: Option<String>,
        content: String,
        source_line: usize,
    },

    /// Blockquote containing nested nodes
    Blockquote {
        content: Vec<MarkdownNode>,
        source_line: usize,
    },

    /// Unordered list
    UnorderedList {
        items: Vec<ListItem>,
        source_line: usize,
    },

    /// Ordered list
    OrderedList {
        start: usize,
        items: Vec<ListItem>,
        source_line: usize,
    },

    /// Horizontal rule
    HorizontalRule {
        source_line: usize,
    },

    /// Thematic break (blank lines, used for spacing)
    ThematicBreak,
}

/// A list item containing block-level content
#[derive(Debug, Clone)]
pub struct ListItem {
    pub content: Vec<MarkdownNode>,
    pub source_line: usize,
}

/// Inline content within paragraphs and headings
#[derive(Debug, Clone)]
pub enum InlineNode {
    /// Plain text
    Text(String),
    /// Bold text
    Strong(Vec<InlineNode>),
    /// Italic text
    Emphasis(Vec<InlineNode>),
    /// Inline code
    Code(String),
    /// Link with text and URL
    Link { text: Vec<InlineNode>, url: String },
    /// Soft break (single newline in source)
    SoftBreak,
    /// Hard break (double newline or trailing spaces)
    HardBreak,
}

impl MarkdownNode {
    /// Get the source line this node starts on
    pub fn source_line(&self) -> Option<usize> {
        match self {
            MarkdownNode::Document(_) => None,
            MarkdownNode::Heading { source_line, .. } => Some(*source_line),
            MarkdownNode::Paragraph { source_line, .. } => Some(*source_line),
            MarkdownNode::CodeBlock { source_line, .. } => Some(*source_line),
            MarkdownNode::Blockquote { source_line, .. } => Some(*source_line),
            MarkdownNode::UnorderedList { source_line, .. } => Some(*source_line),
            MarkdownNode::OrderedList { source_line, .. } => Some(*source_line),
            MarkdownNode::HorizontalRule { source_line } => Some(*source_line),
            MarkdownNode::ThematicBreak => None,
        }
    }
}
```

### Markdown AST Container

```rust
// src/markdown/ast.rs

/// Parsed markdown document with source position mapping
#[derive(Debug, Clone)]
pub struct MarkdownAst {
    /// Root nodes of the document
    pub nodes: Vec<MarkdownNode>,
    /// Revision of source document this was parsed from
    pub source_revision: u64,
}

impl MarkdownAst {
    /// Build line mapping from source lines to preview elements
    pub fn build_line_map(&self) -> SourcePreviewMap {
        let mut map = SourcePreviewMap::new();
        let mut preview_y = 0;

        self.map_nodes(&self.nodes, &mut map, &mut preview_y);
        map
    }

    fn map_nodes(
        &self,
        nodes: &[MarkdownNode],
        map: &mut SourcePreviewMap,
        preview_y: &mut usize,
    ) {
        for node in nodes {
            if let Some(source_line) = node.source_line() {
                map.add(source_line, *preview_y);
            }
            *preview_y += self.node_height(node);
        }
    }

    /// Estimate rendered height of a node (in lines)
    fn node_height(&self, node: &MarkdownNode) -> usize {
        match node {
            MarkdownNode::Heading { .. } => 2, // Heading + spacing
            MarkdownNode::Paragraph { .. } => 2, // Paragraph + spacing
            MarkdownNode::CodeBlock { content, .. } => {
                content.lines().count() + 2 // Code + borders
            }
            MarkdownNode::Blockquote { content, .. } => {
                content.iter().map(|n| self.node_height(n)).sum::<usize>() + 1
            }
            MarkdownNode::UnorderedList { items, .. }
            | MarkdownNode::OrderedList { items, .. } => {
                items.len() + 1 // Items + spacing
            }
            MarkdownNode::HorizontalRule { .. } => 1,
            MarkdownNode::ThematicBreak => 1,
            MarkdownNode::Document(nodes) => {
                nodes.iter().map(|n| self.node_height(n)).sum()
            }
        }
    }
}

/// Bidirectional mapping between source lines and preview positions
#[derive(Debug, Clone, Default)]
pub struct SourcePreviewMap {
    /// source_line → preview_y
    source_to_preview: Vec<(usize, usize)>,
}

impl SourcePreviewMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, source_line: usize, preview_y: usize) {
        self.source_to_preview.push((source_line, preview_y));
    }

    /// Find preview Y position for a source line
    pub fn preview_y_for_source(&self, source_line: usize) -> Option<usize> {
        // Find the closest mapped line at or before source_line
        self.source_to_preview
            .iter()
            .filter(|(src, _)| *src <= source_line)
            .max_by_key(|(src, _)| *src)
            .map(|(_, preview_y)| *preview_y)
    }

    /// Find source line for a preview Y position
    pub fn source_line_for_preview(&self, preview_y: usize) -> Option<usize> {
        self.source_to_preview
            .iter()
            .filter(|(_, py)| *py <= preview_y)
            .max_by_key(|(_, py)| *py)
            .map(|(src, _)| *src)
    }
}
```

### Preview Pane State

```rust
// src/model/preview.rs

use crate::markdown::{MarkdownAst, SourcePreviewMap};
use crate::model::editor_area::{DocumentId, GroupId, Rect};

/// State for a markdown preview pane
#[derive(Debug, Clone)]
pub struct PreviewPane {
    /// Document being previewed
    pub document_id: DocumentId,

    /// Group ID of the source editor (for sync)
    pub source_group_id: GroupId,

    /// Parsed AST (cached, updated on document change)
    pub ast: Option<MarkdownAst>,

    /// Source ↔ Preview position mapping
    pub line_map: SourcePreviewMap,

    /// Current scroll offset in preview (in pixels)
    pub scroll_y: f32,

    /// Whether to sync scroll with source
    pub sync_scroll: bool,

    /// Layout rectangle for rendering
    pub rect: Rect,

    /// Computed layout lines (cached for rendering)
    pub layout_lines: Vec<PreviewLayoutLine>,
}

/// A single line in the preview layout
#[derive(Debug, Clone)]
pub struct PreviewLayoutLine {
    /// Y position in preview space
    pub y: f32,
    /// Height of this line
    pub height: f32,
    /// Content segments with formatting
    pub segments: Vec<PreviewSegment>,
    /// Source line this corresponds to (for scroll sync)
    pub source_line: Option<usize>,
}

/// A formatted segment within a preview line
#[derive(Debug, Clone)]
pub struct PreviewSegment {
    /// Text content
    pub text: String,
    /// Formatting style
    pub style: PreviewStyle,
    /// X offset from line start
    pub x_offset: f32,
}

/// Text styling for preview rendering
#[derive(Debug, Clone, Copy, Default)]
pub struct PreviewStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub heading_level: Option<u8>,
    pub is_link: bool,
    pub is_blockquote: bool,
    pub list_indent: usize,
}

impl PreviewPane {
    pub fn new(document_id: DocumentId, source_group_id: GroupId) -> Self {
        Self {
            document_id,
            source_group_id,
            ast: None,
            line_map: SourcePreviewMap::new(),
            scroll_y: 0.0,
            sync_scroll: true,
            rect: Rect::default(),
            layout_lines: Vec::new(),
        }
    }

    /// Update scroll position based on source editor scroll
    pub fn sync_from_source(&mut self, source_scroll_line: usize, line_height: f32) {
        if !self.sync_scroll {
            return;
        }

        if let Some(preview_line) = self.line_map.preview_y_for_source(source_scroll_line) {
            self.scroll_y = preview_line as f32 * line_height;
        }
    }
}
```

### Layout Extension

```rust
// In src/model/editor_area.rs

/// A node in the layout tree - either a group, split container, or preview
#[derive(Debug, Clone)]
pub enum LayoutNode {
    /// An editor group with tabs
    Group(GroupId),
    /// A split container with children
    Split(SplitContainer),
    /// A markdown preview pane (NEW)
    Preview(PreviewPaneId),
}

/// Unique identifier for a preview pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreviewPaneId(pub u64);
```

### Messages

```rust
// In src/messages.rs

/// Preview-related messages
#[derive(Debug, Clone)]
pub enum PreviewMsg {
    /// Toggle markdown preview for current document
    TogglePreview,

    /// Open preview in split to the right
    OpenPreview,

    /// Close preview pane
    ClosePreview,

    /// Parse completed with new AST
    ParseCompleted {
        document_id: DocumentId,
        revision: u64,
        ast: MarkdownAst,
    },

    /// Sync scroll from source editor
    SyncScroll {
        source_line: usize,
    },

    /// Toggle scroll synchronization
    ToggleSyncScroll,

    /// Manual scroll in preview pane
    Scroll(i32),
}

// Add to Msg enum:
pub enum Msg {
    // ... existing variants ...
    Preview(PreviewMsg),
}
```

### Commands

```rust
// In src/commands.rs

pub enum Cmd {
    // ... existing variants ...

    /// Parse markdown in background
    ParseMarkdown {
        document_id: DocumentId,
        revision: u64,
        content: String,
    },
}
```

### Theme Extension

```rust
// In src/theme.rs

pub struct Theme {
    // ... existing fields ...
    pub preview: PreviewTheme,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewTheme {
    /// Background color for preview pane
    pub background: Color,
    /// Default text color
    pub foreground: Color,
    /// Heading colors by level (h1-h6)
    pub heading_colors: [Color; 6],
    /// Code block background
    pub code_background: Color,
    /// Code text color
    pub code_foreground: Color,
    /// Inline code background
    pub inline_code_background: Color,
    /// Link color
    pub link_color: Color,
    /// Blockquote border color
    pub blockquote_border: Color,
    /// Blockquote text color
    pub blockquote_foreground: Color,
    /// Horizontal rule color
    pub hr_color: Color,
}

impl Default for PreviewTheme {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x1E, 0x1E, 0x1E),
            foreground: Color::rgb(0xD4, 0xD4, 0xD4),
            heading_colors: [
                Color::rgb(0x56, 0x9C, 0xD6), // h1 - blue
                Color::rgb(0x4E, 0xC9, 0xB0), // h2 - teal
                Color::rgb(0xDC, 0xDC, 0xAA), // h3 - yellow
                Color::rgb(0xCE, 0x91, 0x78), // h4 - orange
                Color::rgb(0xC5, 0x86, 0xC0), // h5 - purple
                Color::rgb(0x9C, 0xDC, 0xFE), // h6 - light blue
            ],
            code_background: Color::rgb(0x2D, 0x2D, 0x2D),
            code_foreground: Color::rgb(0xCE, 0x91, 0x78),
            inline_code_background: Color::rgb(0x3C, 0x3C, 0x3C),
            link_color: Color::rgb(0x56, 0x9C, 0xD6),
            blockquote_border: Color::rgb(0x56, 0x9C, 0xD6),
            blockquote_foreground: Color::rgb(0x9C, 0x9C, 0x9C),
            hr_color: Color::rgb(0x4E, 0x4E, 0x4E),
        }
    }
}
```

---

## Rendering

### Preview Pane Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Preview: README.md                                               [x]       │ <- Title bar
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Heading 1                                              (h1, large)  │   │
│  │  ═══════════════════════════════════════════════════════════════    │   │
│  │                                                                      │   │
│  │  This is a paragraph with some **bold** and *italic* text.          │   │
│  │                                                                      │   │
│  │  ┌────────────────────────────────────────────────────────────┐     │   │
│  │  │ > This is a blockquote with some quoted text that might   │     │   │
│  │  │ > wrap to multiple lines.                                  │     │   │
│  │  └────────────────────────────────────────────────────────────┘     │   │
│  │                                                                      │   │
│  │  • List item 1                                                       │   │
│  │  • List item 2                                                       │   │
│  │    • Nested item                                                     │   │
│  │  • List item 3                                                       │   │
│  │                                                                      │   │
│  │  ┌────────────────────────────────────────────────────────────┐     │   │
│  │  │  fn main() {                                               │     │   │
│  │  │      println!("Hello, world!");                            │     │   │
│  │  │  }                                                         │     │   │
│  │  └────────────────────────────────────────────────────────────┘     │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Rendering Logic

```rust
// src/view/preview.rs

use crate::markdown::{InlineNode, MarkdownNode};
use crate::model::preview::{PreviewLayoutLine, PreviewPane, PreviewSegment, PreviewStyle};
use crate::view::{Frame, TextPainter};
use crate::theme::PreviewTheme;

/// Render a preview pane
pub fn render_preview(
    frame: &mut Frame,
    painter: &mut TextPainter,
    preview: &PreviewPane,
    theme: &PreviewTheme,
    font_size: f32,
    line_height: f32,
) {
    let rect = &preview.rect;

    // Background
    frame.fill_rect(
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
        theme.background.to_argb_u32(),
    );

    // Title bar
    render_preview_title(frame, painter, preview, theme, rect, font_size);

    let content_y = rect.y + line_height + 4.0; // After title bar
    let content_height = rect.height - line_height - 4.0;

    // Render visible layout lines
    let visible_start = (preview.scroll_y / line_height) as usize;
    let visible_count = (content_height / line_height) as usize + 2;

    for (i, layout_line) in preview.layout_lines.iter().enumerate() {
        if i < visible_start {
            continue;
        }
        if i > visible_start + visible_count {
            break;
        }

        let y = content_y + (i - visible_start) as f32 * line_height - (preview.scroll_y % line_height);

        render_preview_line(
            frame,
            painter,
            layout_line,
            rect.x + 16.0, // Left padding
            y,
            theme,
            font_size,
        );
    }
}

fn render_preview_line(
    frame: &mut Frame,
    painter: &mut TextPainter,
    line: &PreviewLayoutLine,
    base_x: f32,
    y: f32,
    theme: &PreviewTheme,
    font_size: f32,
) {
    let mut x = base_x;

    for segment in &line.segments {
        let color = style_to_color(&segment.style, theme);
        let actual_font_size = style_to_font_size(&segment.style, font_size);

        // Handle special backgrounds (code, etc.)
        if segment.style.code {
            let text_width = segment.text.len() as f32 * (font_size * 0.6); // Approximate
            frame.fill_rect(
                (x - 2.0) as usize,
                y as usize,
                (text_width + 4.0) as usize,
                actual_font_size as usize + 2,
                theme.inline_code_background.to_argb_u32(),
            );
        }

        painter.draw_text(
            x as usize,
            y as usize,
            &segment.text,
            color.to_argb_u32(),
            actual_font_size,
        );

        x += segment.text.len() as f32 * (font_size * 0.6); // Approximate advance
    }
}

fn style_to_color(style: &PreviewStyle, theme: &PreviewTheme) -> Color {
    if style.is_link {
        return theme.link_color;
    }
    if style.is_blockquote {
        return theme.blockquote_foreground;
    }
    if style.code {
        return theme.code_foreground;
    }
    if let Some(level) = style.heading_level {
        let idx = (level.saturating_sub(1) as usize).min(5);
        return theme.heading_colors[idx];
    }
    theme.foreground
}

fn style_to_font_size(style: &PreviewStyle, base_size: f32) -> f32 {
    if let Some(level) = style.heading_level {
        match level {
            1 => base_size * 1.8,
            2 => base_size * 1.5,
            3 => base_size * 1.3,
            4 => base_size * 1.1,
            _ => base_size,
        }
    } else {
        base_size
    }
}
```

### Layout Computation

```rust
// src/markdown/layout.rs

use crate::markdown::{InlineNode, MarkdownAst, MarkdownNode};
use crate::model::preview::{PreviewLayoutLine, PreviewSegment, PreviewStyle};

/// Convert AST to layout lines for rendering
pub fn layout_ast(
    ast: &MarkdownAst,
    max_width: f32,
    char_width: f32,
) -> Vec<PreviewLayoutLine> {
    let mut lines = Vec::new();
    let mut y: f32 = 0.0;

    for node in &ast.nodes {
        layout_node(node, &mut lines, &mut y, 0, max_width, char_width);
    }

    lines
}

fn layout_node(
    node: &MarkdownNode,
    lines: &mut Vec<PreviewLayoutLine>,
    y: &mut f32,
    indent: usize,
    max_width: f32,
    char_width: f32,
) {
    let line_height = 20.0; // Base line height

    match node {
        MarkdownNode::Heading { level, content, source_line } => {
            let style = PreviewStyle {
                heading_level: Some(*level),
                ..Default::default()
            };

            let segments = layout_inline_content(content, style);
            lines.push(PreviewLayoutLine {
                y: *y,
                height: line_height * heading_scale(*level),
                segments,
                source_line: Some(*source_line),
            });

            *y += line_height * heading_scale(*level);

            // Underline for h1/h2
            if *level <= 2 {
                // Add separator line (handled in rendering)
                *y += 4.0;
            }

            *y += line_height * 0.5; // Spacing after heading
        }

        MarkdownNode::Paragraph { content, source_line } => {
            let style = PreviewStyle::default();
            let segments = layout_inline_content(content, style);

            // Word wrap handling would go here
            lines.push(PreviewLayoutLine {
                y: *y,
                height: line_height,
                segments,
                source_line: Some(*source_line),
            });

            *y += line_height + line_height * 0.5; // Line + spacing
        }

        MarkdownNode::CodeBlock { content, source_line, .. } => {
            for (i, code_line) in content.lines().enumerate() {
                let style = PreviewStyle {
                    code: true,
                    ..Default::default()
                };

                lines.push(PreviewLayoutLine {
                    y: *y,
                    height: line_height,
                    segments: vec![PreviewSegment {
                        text: code_line.to_string(),
                        style,
                        x_offset: 8.0, // Code block padding
                    }],
                    source_line: if i == 0 { Some(*source_line) } else { None },
                });

                *y += line_height;
            }

            *y += line_height * 0.5; // Spacing after code block
        }

        MarkdownNode::UnorderedList { items, source_line } => {
            for (i, item) in items.iter().enumerate() {
                let bullet = PreviewSegment {
                    text: "•".to_string(),
                    style: PreviewStyle::default(),
                    x_offset: indent as f32 * char_width,
                };

                // Layout item content
                let mut segments = vec![bullet];
                for item_node in &item.content {
                    // Simplified: just get text from first paragraph
                    if let MarkdownNode::Paragraph { content, .. } = item_node {
                        let style = PreviewStyle {
                            list_indent: indent + 1,
                            ..Default::default()
                        };
                        let mut inline_segments = layout_inline_content(content, style);
                        // Offset for bullet
                        for seg in &mut inline_segments {
                            seg.x_offset += (indent as f32 + 2.0) * char_width;
                        }
                        segments.extend(inline_segments);
                    }
                }

                lines.push(PreviewLayoutLine {
                    y: *y,
                    height: line_height,
                    segments,
                    source_line: if i == 0 { Some(*source_line) } else { Some(item.source_line) },
                });

                *y += line_height;
            }

            *y += line_height * 0.5;
        }

        MarkdownNode::Blockquote { content, source_line } => {
            for node in content {
                layout_node(node, lines, y, indent + 1, max_width, char_width);
            }
        }

        MarkdownNode::HorizontalRule { source_line } => {
            // Rendered as a thin line
            lines.push(PreviewLayoutLine {
                y: *y,
                height: 8.0,
                segments: vec![PreviewSegment {
                    text: "─".repeat(40),
                    style: PreviewStyle::default(),
                    x_offset: 0.0,
                }],
                source_line: Some(*source_line),
            });

            *y += 8.0 + line_height * 0.5;
        }

        _ => {}
    }
}

fn layout_inline_content(nodes: &[InlineNode], base_style: PreviewStyle) -> Vec<PreviewSegment> {
    let mut segments = Vec::new();
    let mut x_offset = 0.0;

    for node in nodes {
        match node {
            InlineNode::Text(text) => {
                segments.push(PreviewSegment {
                    text: text.clone(),
                    style: base_style,
                    x_offset,
                });
                x_offset += text.len() as f32 * 8.0; // Approximate
            }
            InlineNode::Strong(children) => {
                let mut bold_style = base_style;
                bold_style.bold = true;
                let child_segments = layout_inline_content(children, bold_style);
                for mut seg in child_segments {
                    seg.x_offset += x_offset;
                    x_offset += seg.text.len() as f32 * 8.0;
                    segments.push(seg);
                }
            }
            InlineNode::Emphasis(children) => {
                let mut italic_style = base_style;
                italic_style.italic = true;
                let child_segments = layout_inline_content(children, italic_style);
                for mut seg in child_segments {
                    seg.x_offset += x_offset;
                    x_offset += seg.text.len() as f32 * 8.0;
                    segments.push(seg);
                }
            }
            InlineNode::Code(code) => {
                let mut code_style = base_style;
                code_style.code = true;
                segments.push(PreviewSegment {
                    text: code.clone(),
                    style: code_style,
                    x_offset,
                });
                x_offset += code.len() as f32 * 8.0;
            }
            InlineNode::Link { text, .. } => {
                let mut link_style = base_style;
                link_style.is_link = true;
                let child_segments = layout_inline_content(text, link_style);
                for mut seg in child_segments {
                    seg.x_offset += x_offset;
                    x_offset += seg.text.len() as f32 * 8.0;
                    segments.push(seg);
                }
            }
            InlineNode::SoftBreak => {
                segments.push(PreviewSegment {
                    text: " ".to_string(),
                    style: base_style,
                    x_offset,
                });
                x_offset += 8.0;
            }
            InlineNode::HardBreak => {
                // Start new line - handled by caller
            }
        }
    }

    segments
}

fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.8,
        2 => 1.5,
        3 => 1.3,
        4 => 1.1,
        _ => 1.0,
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Command |
|--------|-----|---------------|---------|
| Toggle preview | `Cmd+Shift+V` | `Ctrl+Shift+V` | `MarkdownTogglePreview` |
| Open preview to side | `Cmd+K V` | `Ctrl+K V` | `MarkdownOpenPreviewToSide` |
| Toggle scroll sync | `Cmd+Shift+S` | `Ctrl+Shift+S` | `MarkdownToggleScrollSync` |

### Keymap Configuration

```yaml
# keymap.yaml additions

# Markdown preview
- key: "cmd+shift+v"
  command: MarkdownTogglePreview
  when: ["language:markdown"]

- key: "cmd+k v"
  command: MarkdownOpenPreviewToSide
  when: ["language:markdown"]

- key: "cmd+shift+s"
  command: MarkdownToggleScrollSync
  when: ["has_preview"]
```

---

## Implementation Plan

### Phase 1: Markdown Parser Integration

**Effort:** M (2-3 days)

- [ ] Add `pulldown-cmark` dependency
- [ ] Create `src/markdown/mod.rs` module structure
- [ ] Implement `MarkdownAst` and `MarkdownNode` types
- [ ] Create parser wrapper converting pulldown events to AST
- [ ] Add source line tracking during parsing
- [ ] Unit tests for parser

**Test:** Parse "# Hello\n\nWorld" produces Heading and Paragraph nodes.

### Phase 2: Preview Pane Model

**Effort:** M (2-3 days)

- [ ] Create `src/model/preview.rs` with `PreviewPane`
- [ ] Add `PreviewPaneId` and `LayoutNode::Preview` variant
- [ ] Implement `SourcePreviewMap` for scroll sync
- [ ] Add `PreviewTheme` to theme system
- [ ] Update theme YAML files with preview colors

**Test:** Creating a `PreviewPane` with document ID stores reference correctly.

### Phase 3: Layout System

**Effort:** M (3-4 days)

- [ ] Create `src/markdown/layout.rs`
- [ ] Implement `layout_ast()` converting AST to layout lines
- [ ] Handle inline formatting (bold, italic, code)
- [ ] Handle block elements (headings, lists, code blocks, blockquotes)
- [ ] Implement basic word wrapping

**Test:** Layout of heading produces line with correct height scaling.

### Phase 4: Preview Rendering

**Effort:** L (4-5 days)

- [ ] Create `src/view/preview.rs`
- [ ] Implement `render_preview()` function
- [ ] Integrate into main render loop
- [ ] Handle viewport scrolling
- [ ] Render title bar with close button
- [ ] Style different elements (headings, code, blockquotes)

**Test:** Preview pane renders formatted heading text.

### Phase 5: Message Flow and Updates

**Effort:** M (2-3 days)

- [ ] Add `PreviewMsg` to messages.rs
- [ ] Add `ParseMarkdown` command
- [ ] Create `src/update/preview.rs` handler
- [ ] Implement toggle preview command
- [ ] Trigger re-parse on document changes (debounced)
- [ ] Wire keybindings

**Test:** Cmd+Shift+V opens preview pane for .md file.

### Phase 6: Scroll Synchronization

**Effort:** M (2-3 days)

- [ ] Implement `SourcePreviewMap` population from AST
- [ ] Hook editor scroll events to update preview scroll
- [ ] Optional: hook preview scroll to update source scroll
- [ ] Add toggle for scroll sync
- [ ] Smooth scrolling animation

**Test:** Scrolling source editor scrolls preview to corresponding section.

### Phase 7: Polish

**Effort:** S (1-2 days)

- [ ] Handle edge cases (empty file, parsing errors)
- [ ] Add loading indicator during parse
- [ ] Improve code block rendering (syntax highlighting?)
- [ ] Performance optimization for large files
- [ ] Documentation

**Test:** Large README.md (1000+ lines) renders without lag.

---

## Testing Strategy

### Unit Tests

```rust
// tests/markdown_parser.rs

#[test]
fn test_parse_heading() {
    let md = "# Hello World";
    let ast = parse_markdown(md);

    assert_eq!(ast.nodes.len(), 1);
    assert!(matches!(
        &ast.nodes[0],
        MarkdownNode::Heading { level: 1, .. }
    ));
}

#[test]
fn test_parse_inline_formatting() {
    let md = "Some **bold** and *italic* text";
    let ast = parse_markdown(md);

    let MarkdownNode::Paragraph { content, .. } = &ast.nodes[0] else {
        panic!("Expected paragraph");
    };

    assert!(content.iter().any(|n| matches!(n, InlineNode::Strong(_))));
    assert!(content.iter().any(|n| matches!(n, InlineNode::Emphasis(_))));
}

#[test]
fn test_parse_code_block() {
    let md = "```rust\nfn main() {}\n```";
    let ast = parse_markdown(md);

    assert!(matches!(
        &ast.nodes[0],
        MarkdownNode::CodeBlock { language: Some(lang), .. } if lang == "rust"
    ));
}

#[test]
fn test_source_line_tracking() {
    let md = "# H1\n\nPara\n\n## H2";
    let ast = parse_markdown(md);

    let heading1 = &ast.nodes[0];
    assert_eq!(heading1.source_line(), Some(0));

    let para = &ast.nodes[1];
    assert_eq!(para.source_line(), Some(2));

    let heading2 = &ast.nodes[2];
    assert_eq!(heading2.source_line(), Some(4));
}

#[test]
fn test_line_map_basic() {
    let md = "# H1\n\nParagraph\n\n## H2";
    let ast = parse_markdown(md);
    let map = ast.build_line_map();

    // H1 at source line 0 → preview line 0
    assert_eq!(map.preview_y_for_source(0), Some(0));

    // Paragraph at source line 2 → somewhere after H1
    let para_y = map.preview_y_for_source(2);
    assert!(para_y.is_some());
    assert!(para_y.unwrap() > 0);
}
```

### Integration Tests

```rust
// tests/markdown_preview.rs

#[test]
fn test_toggle_preview_creates_split() {
    let mut model = test_model_with_markdown("# Test\n\nContent");

    update(&mut model, Msg::Preview(PreviewMsg::TogglePreview));

    // Should now have a split layout with preview
    assert!(matches!(model.editor_area.layout, LayoutNode::Split(_)));
}

#[test]
fn test_edit_updates_preview() {
    let mut model = test_model_with_markdown_and_preview("# Initial");

    // Edit the document
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));

    // After debounce, preview should update
    // (verify by checking AST revision matches document revision)
}

#[test]
fn test_scroll_sync() {
    let mut model = test_model_with_markdown_and_preview(LONG_MARKDOWN);

    // Scroll source to line 50
    let editor = model.focused_editor_mut().unwrap();
    editor.viewport.scroll_offset = 50;

    update(&mut model, Msg::Editor(EditorMsg::Scroll(50)));

    // Preview should have scrolled proportionally
    let preview = model.preview_panes.values().next().unwrap();
    assert!(preview.scroll_y > 0.0);
}
```

---

## Dependencies

```toml
# Cargo.toml additions

[dependencies]
pulldown-cmark = "0.10"  # Markdown parser (CommonMark compliant)
```

### Why `pulldown-cmark`?

- Fast, streaming parser
- CommonMark compliant
- Source position tracking
- Well-maintained, widely used
- No unsafe code
- Pure Rust

---

## Performance Considerations

### Debounced Parsing

Parse on document change with 150ms debounce:

```rust
const MARKDOWN_PARSE_DEBOUNCE_MS: u64 = 150;
```

### Large File Handling

For files > 5000 lines:

1. Parse only visible portion + buffer
2. Use virtual scrolling for layout
3. Cache rendered line bitmaps

### Memory

- AST is relatively lightweight (string slices into source)
- Layout lines can grow large - consider streaming layout
- Preview pane caches layout; clear on document change

---

## Future Enhancements

### Phase 2: Enhanced Rendering

- Syntax highlighting in code blocks (reuse tree-sitter)
- Image placeholders with dimensions
- Table rendering (basic grid)
- Checkbox rendering for task lists

### Phase 3: Interactivity

- Click links to open in browser
- Click headings to jump in source
- Copy code blocks to clipboard
- Hover tooltips for links

### Phase 4: Export

- Export to HTML
- Print preview
- PDF generation (via external tool)

---

## References

- [pulldown-cmark](https://crates.io/crates/pulldown-cmark) - Markdown parser
- [CommonMark Spec](https://spec.commonmark.org/) - Markdown standard
- [VS Code Markdown Preview](https://code.visualstudio.com/docs/languages/markdown) - Visual reference
- [Feature: Split View](../archived/SPLIT_VIEW.md) - Layout system reference
- [Feature: Syntax Highlighting](syntax-highlighting.md) - For code block highlighting
