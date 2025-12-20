# Markdown Preview

Live preview pane for Markdown files with synchronized scrolling

> **Status:** 📋 Planned
> **Priority:** P2
> **Effort:** M (webview) / L (native)
> **Created:** 2025-12-19
> **Updated:** 2025-12-20
> **Milestone:** 5 - Insight Tools
> **Feature ID:** F-170

---

## Table of Contents

1. [Overview](#overview)
2. [Approach Comparison](#approach-comparison)
3. [Recommended: Webview Approach](#recommended-webview-approach)
4. [Alternative: Native Rendering](#alternative-native-rendering)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor has:
- Syntax highlighting for Markdown (tree-sitter-md)
- Split view infrastructure (`EditorArea`, `LayoutNode`, `SplitContainer`)
- winit-based event loop

No rendered preview exists—users see only raw Markdown with syntax highlighting.

### Goals

1. **Live preview** - Render Markdown as formatted HTML in a preview pane
2. **Synchronized scrolling** - Scroll position linked between source and preview
3. **Split integration** - Preview appears as a pane alongside source editor
4. **Theme-aware** - Preview respects editor theme colors
5. **Live updates** - Preview updates as user types (debounced)

### Non-Goals

- Image upload/embedding
- LaTeX/math rendering (future)
- Mermaid diagram rendering (future)
- Export to PDF
- WYSIWYG editing in preview

---

## Approach Comparison

| Aspect | Webview (wry) | Native Rendering |
|--------|---------------|------------------|
| **Effort** | M (3-5 days) | L (1-2 weeks) |
| **Rendering quality** | Excellent (full CSS) | Basic (styled text) |
| **Scroll sync** | Easy (JS callbacks) | Complex (line mapping) |
| **Dependencies** | +wry (~WebKit/Edge) | +pulldown-cmark only |
| **Binary size** | Minimal (uses OS webview) | No change |
| **Maintenance** | Low (leverage HTML/CSS) | High (custom renderer) |
| **Platform support** | macOS/Windows/Linux | Same |

### Recommendation: Webview

The webview approach is **simpler and more maintainable**:
- Markdown → HTML is a solved problem (`pulldown-cmark`)
- CSS handles all styling (headers, code blocks, lists)
- JS handles scroll sync with 10 lines of code
- No need to build a custom formatted text renderer

---

## Recommended: Webview Approach

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Markdown Preview with Webview                         │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         EditorArea Layout                            │    │
│  │  ┌────────────────────────┬─────────────────────────┐               │    │
│  │  │    Source Editor       │    Webview Preview      │               │    │
│  │  │   (existing)           │   (wry WebView)         │               │    │
│  │  │                        │                         │               │    │
│  │  │  # Heading             │  <h1>Heading</h1>       │               │    │
│  │  │  Some **bold** text    │  Some <b>bold</b> text  │               │    │
│  │  │                        │                         │               │    │
│  │  └────────────────────────┴─────────────────────────┘               │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                           Data Flow                                  │    │
│  │                                                                      │    │
│  │   Document ──► pulldown-cmark ──► HTML String ──► WebView.eval()    │    │
│  │                                                                      │    │
│  │   Scroll Sync:                                                       │    │
│  │   Source scroll ──► JS: window.scrollTo(lineY) ──► Preview scrolls  │    │
│  │   Preview scroll ──► IPC callback ──► Source scrolls                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Dependencies

```toml
# Cargo.toml additions
[dependencies]
wry = "0.50"           # Cross-platform webview (uses OS webview)
pulldown-cmark = "0.12" # Markdown → HTML
```

**Note:** `wry` uses the OS-provided webview (WebKit on macOS/Linux, WebView2 on Windows), so binary size increase is minimal.

### Module Structure

```
src/
├── markdown/                    # NEW MODULE
│   ├── mod.rs                   # Public exports
│   ├── renderer.rs              # Markdown → HTML via pulldown-cmark
│   └── preview.rs               # PreviewPane with wry WebView
├── model/
│   └── editor_area.rs           # + LayoutNode::Preview variant
├── update/
│   └── preview.rs               # NEW: Preview message handler
└── messages.rs                  # + PreviewMsg enum
```

### Core Implementation

#### Markdown to HTML

```rust
// src/markdown/renderer.rs

use pulldown_cmark::{html, Options, Parser};

/// Convert markdown to HTML with source line markers
pub fn markdown_to_html(markdown: &str, theme: &PreviewTheme) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let parser = Parser::new_ext(markdown, options);

    // Wrap with line markers for scroll sync
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Wrap in styled HTML document
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>{}</style>
</head>
<body>
    <div id="content">{}</div>
    <script>{}</script>
</body>
</html>"#,
        generate_css(theme),
        add_line_markers(&html_output, markdown),
        SCROLL_SYNC_JS
    )
}

fn generate_css(theme: &PreviewTheme) -> String {
    format!(r#"
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
            font-size: 14px;
            line-height: 1.6;
            color: {};
            background: {};
            padding: 20px;
            max-width: 800px;
            margin: 0 auto;
        }}
        h1, h2, h3, h4, h5, h6 {{
            color: {};
            border-bottom: 1px solid {};
            padding-bottom: 0.3em;
        }}
        code {{
            background: {};
            padding: 0.2em 0.4em;
            border-radius: 3px;
            font-family: "SF Mono", Consolas, monospace;
        }}
        pre {{
            background: {};
            padding: 16px;
            border-radius: 6px;
            overflow-x: auto;
        }}
        pre code {{
            background: none;
            padding: 0;
        }}
        blockquote {{
            border-left: 4px solid {};
            margin: 0;
            padding-left: 16px;
            color: {};
        }}
        a {{
            color: {};
        }}
        [data-line] {{
            scroll-margin-top: 20px;
        }}
    "#,
        theme.text,
        theme.background,
        theme.heading,
        theme.border,
        theme.code_background,
        theme.code_background,
        theme.accent,
        theme.muted,
        theme.link,
    )
}

/// Add data-line attributes to elements for scroll sync
fn add_line_markers(html: &str, markdown: &str) -> String {
    // Simple approach: wrap each block-level element with line number
    // More sophisticated: use pulldown-cmark's offset tracking
    html.to_string() // TODO: Add line markers
}

const SCROLL_SYNC_JS: &str = r#"
    // Receive scroll position from editor
    window.scrollToLine = function(line) {
        const el = document.querySelector(`[data-line="${line}"]`);
        if (el) {
            el.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
    };

    // Report scroll position back to editor
    let lastReportedLine = -1;
    window.addEventListener('scroll', () => {
        const elements = document.querySelectorAll('[data-line]');
        for (const el of elements) {
            const rect = el.getBoundingClientRect();
            if (rect.top >= 0) {
                const line = parseInt(el.dataset.line, 10);
                if (line !== lastReportedLine) {
                    lastReportedLine = line;
                    window.ipc.postMessage(JSON.stringify({ type: 'scroll', line }));
                }
                break;
            }
        }
    });
"#;
```

#### Preview Pane with Webview

```rust
// src/markdown/preview.rs

use wry::{WebView, WebViewBuilder, Rect};
use crate::model::editor_area::DocumentId;

/// A markdown preview pane using an embedded webview
pub struct PreviewPane {
    /// The webview instance
    webview: WebView,

    /// Document being previewed
    pub document_id: DocumentId,

    /// Whether scroll sync is enabled
    pub sync_scroll: bool,

    /// Last rendered revision (for change detection)
    last_revision: u64,
}

impl PreviewPane {
    /// Create a new preview pane as a child of the window
    pub fn new(
        window: &impl raw_window_handle::HasWindowHandle,
        document_id: DocumentId,
        bounds: Rect,
        ipc_handler: impl Fn(String) + 'static,
    ) -> Result<Self, wry::Error> {
        let webview = WebViewBuilder::new()
            .with_bounds(bounds)
            .with_ipc_handler(move |msg| {
                ipc_handler(msg.body().to_string());
            })
            .with_devtools(cfg!(debug_assertions))
            .build_as_child(window)?;

        Ok(Self {
            webview,
            document_id,
            sync_scroll: true,
            last_revision: 0,
        })
    }

    /// Update preview content
    pub fn update_content(&mut self, html: &str, revision: u64) {
        if revision != self.last_revision {
            // Use data URL to load content (avoids CORS issues)
            let encoded = base64::encode(html);
            let data_url = format!("data:text/html;base64,{}", encoded);
            let _ = self.webview.load_url(&data_url);
            self.last_revision = revision;
        }
    }

    /// Scroll preview to match source line
    pub fn scroll_to_line(&self, line: usize) {
        if self.sync_scroll {
            let js = format!("window.scrollToLine({})", line);
            let _ = self.webview.evaluate_script(&js);
        }
    }

    /// Update bounds when layout changes
    pub fn set_bounds(&mut self, bounds: Rect) {
        let _ = self.webview.set_bounds(bounds);
    }
}
```

#### Preview Theme

```rust
// src/markdown/mod.rs

/// Theme colors for markdown preview
#[derive(Debug, Clone)]
pub struct PreviewTheme {
    pub background: String,
    pub text: String,
    pub heading: String,
    pub link: String,
    pub code_background: String,
    pub border: String,
    pub accent: String,
    pub muted: String,
}

impl PreviewTheme {
    /// Generate from editor theme
    pub fn from_editor_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            background: theme.editor.background.to_css(),
            text: theme.editor.text.to_css(),
            heading: theme.syntax.keyword.to_css(),
            link: theme.syntax.string.to_css(),
            code_background: theme.editor.gutter_bg.to_css(),
            border: theme.editor.line_number.to_css(),
            accent: theme.syntax.function.to_css(),
            muted: theme.editor.line_number.to_css(),
        }
    }
}
```

### Messages

```rust
// src/messages.rs additions

#[derive(Debug, Clone)]
pub enum PreviewMsg {
    /// Toggle preview for current document
    Toggle,

    /// Open preview to the side
    Open,

    /// Close preview
    Close,

    /// Update preview content (after document edit)
    Refresh,

    /// Scroll preview to line (from source scroll)
    ScrollToLine(usize),

    /// Scroll source to line (from preview scroll via IPC)
    SyncFromPreview(usize),

    /// Toggle scroll synchronization
    ToggleSync,
}
```

---

## Alternative: Native Rendering

If adding a webview dependency is undesirable, a simpler native approach:

### Styled Text Rendering

```rust
// Render markdown as styled lines using existing TextPainter

pub struct MarkdownLine {
    pub segments: Vec<StyledSegment>,
    pub source_line: usize,
}

pub struct StyledSegment {
    pub text: String,
    pub style: MarkdownStyle,
}

pub enum MarkdownStyle {
    Normal,
    Heading { level: u8 },
    Bold,
    Italic,
    Code,
    Link,
    ListItem { indent: usize },
    Blockquote,
}
```

This approach uses the existing `TextPainter` but with additional style handling. However, it requires:
- Custom font scaling for headings
- Custom rendering for code blocks
- Manual line wrapping
- More complex scroll sync mapping

**Recommendation:** Start with webview; consider native if webview proves problematic.

---

## Implementation Plan

### Phase 1: Core Infrastructure

**Effort:** S (1-2 days)

- [ ] Add `wry` and `pulldown-cmark` dependencies
- [ ] Create `src/markdown/mod.rs` module structure
- [ ] Implement `markdown_to_html()` with basic styling
- [ ] Create `PreviewTheme` from editor theme

**Test:** Call `markdown_to_html()` and verify output.

### Phase 2: Webview Integration

**Effort:** M (2-3 days)

- [ ] Implement `PreviewPane` with wry WebView
- [ ] Add `LayoutNode::Preview` variant to layout system
- [ ] Create `update/preview.rs` message handler
- [ ] Wire `PreviewMsg::Toggle` to Cmd+Shift+V
- [ ] Render preview pane in correct layout position

**Test:** Cmd+Shift+V opens preview pane with rendered markdown.

### Phase 3: Content Updates

**Effort:** S (1-2 days)

- [ ] Trigger preview refresh on document changes (debounced 150ms)
- [ ] Track document revision to avoid redundant updates
- [ ] Handle theme changes (re-render with new CSS)

**Test:** Type in source, preview updates after short delay.

### Phase 4: Scroll Synchronization

**Effort:** M (2-3 days)

- [ ] Add `data-line` attributes to HTML elements
- [ ] Implement `scrollToLine()` JS function
- [ ] Hook source scroll to call `preview.scroll_to_line()`
- [ ] Implement IPC handler for preview → source sync
- [ ] Add toggle for scroll sync

**Test:** Scroll in source, preview follows. Scroll in preview, source follows.

### Phase 5: Polish

**Effort:** S (1 day)

- [ ] Add loading indicator during render
- [ ] Handle edge cases (empty file, very large file)
- [ ] Close preview when switching to non-markdown file
- [ ] Add to command palette

**Test:** Full workflow feels responsive and polished.

---

## Keybindings

| Action | Mac | Windows/Linux | Command |
|--------|-----|---------------|---------|
| Toggle preview | `Cmd+Shift+V` | `Ctrl+Shift+V` | `MarkdownTogglePreview` |
| Open to side | `Cmd+K V` | `Ctrl+K V` | `MarkdownOpenPreviewToSide` |

```yaml
# keymap.yaml additions
- key: "cmd+shift+v"
  command: MarkdownTogglePreview
  when: ["language:markdown"]

- key: "cmd+k v"
  command: MarkdownOpenPreviewToSide
  when: ["language:markdown"]
```

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_markdown_to_html_basic() {
    let md = "# Hello\n\nWorld";
    let html = markdown_to_html(md, &PreviewTheme::default());

    assert!(html.contains("<h1>Hello</h1>"));
    assert!(html.contains("<p>World</p>"));
}

#[test]
fn test_markdown_to_html_code_block() {
    let md = "```rust\nfn main() {}\n```";
    let html = markdown_to_html(md, &PreviewTheme::default());

    assert!(html.contains("<pre>"));
    assert!(html.contains("<code"));
    assert!(html.contains("fn main()"));
}

#[test]
fn test_preview_theme_from_editor_theme() {
    let editor_theme = Theme::default();
    let preview_theme = PreviewTheme::from_editor_theme(&editor_theme);

    assert!(!preview_theme.background.is_empty());
    assert!(!preview_theme.text.is_empty());
}
```

### Manual Testing

- [ ] Toggle preview opens/closes pane
- [ ] Preview renders headings, lists, code blocks, links
- [ ] Preview updates on source edit
- [ ] Scroll sync works in both directions
- [ ] Theme colors match editor
- [ ] Works with split views
- [ ] Handles large markdown files

---

## Future Enhancements

### Phase 2: Enhanced Rendering

- Syntax highlighting in code blocks (highlight.js)
- Image rendering (if local file)
- Table rendering

### Phase 3: Interactivity

- Click link to open in browser
- Click heading to jump in source
- Copy code block button

---

## References

- [wry](https://github.com/tauri-apps/wry) - Cross-platform webview library
- [pulldown-cmark](https://crates.io/crates/pulldown-cmark) - Markdown parser
- [VS Code Markdown Preview](https://code.visualstudio.com/docs/languages/markdown) - Reference implementation
