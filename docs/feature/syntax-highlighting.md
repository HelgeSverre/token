# Syntax Highlighting with Tree-sitter

**Status:** In Progress  
**Created:** 2025-12-07  
**Updated:** 2025-12-15  
**Effort:** L (1-2 weeks)

---

## Overview

Add syntax highlighting to the editor using [tree-sitter](https://tree-sitter.github.io/tree-sitter/) for incremental parsing. Initial support for **YAML, Markdown, and Rust** with architecture designed for easy language addition (target: 20+ languages).

### Goals

1. **Incremental parsing** — Only re-parse changed portions of the document
2. **Non-blocking** — Parsing runs in background thread, UI never waits
3. **Language injection** — Support PHP files containing HTML/CSS/JavaScript
4. **Theme integration** — Map highlight captures to existing theme system
5. **Future LSP ready** — Architecture supports semantic tokens overlay

### Non-Goals (This Phase)

- LSP integration (separate feature)
- Code folding based on syntax tree
- Go-to-definition, find references
- Soft wrapping aware highlighting (future enhancement)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Main Thread (Event Loop)                      │
│                                                                         │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────┐  │
│  │   Document   │───▶│   Update()   │───▶│  Cmd::ParseSyntax {...}  │  │
│  │   Changed    │    │              │    │                          │  │
│  └──────────────┘    └──────────────┘    └───────────┬──────────────┘  │
│                                                       │                 │
│  ┌──────────────────────────────────────────────────┐ │                 │
│  │                  process_cmd()                    │◀┘                │
│  │  Spawns background task, sends Msg on completion  │                  │
│  └───────────────────────────┬──────────────────────┘                  │
│                               │                                         │
│  ┌────────────────────────────▼─────────────────────────────────────┐  │
│  │  Msg::SyntaxUpdated { highlights, version }                       │  │
│  │  ───────────────────────────────────────────────────────────────  │  │
│  │  update() stores highlights in model, triggers Cmd::Redraw        │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────┐  │
│  │   Renderer   │───▶│ get_line_    │───▶│  Apply token colors to   │  │
│  │              │    │ highlights() │    │  text during draw_text() │  │
│  └──────────────┘    └──────────────┘    └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
                               │
                               │ mpsc channel
                               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Background Parser Thread                        │
│                                                                         │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────┐  │
│  │   Parser     │───▶│   tree.      │───▶│   QueryCursor.captures() │  │
│  │   .parse()   │    │   edit()     │    │   on visible range       │  │
│  └──────────────┘    └──────────────┘    └──────────────────────────┘  │
│                                                                         │
│  Maintains: current Tree, compiled Query, per-language Parser           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Token Model

```rust
/// A single highlighted span within a line
#[derive(Debug, Clone)]
pub struct HighlightToken {
    pub start_col: usize,      // Inclusive (0-indexed)
    pub end_col: usize,        // Exclusive
    pub highlight: HighlightId, // Index into HIGHLIGHT_NAMES
}

/// Highlight information for a single line
#[derive(Debug, Clone, Default)]
pub struct LineHighlights {
    pub tokens: Vec<HighlightToken>,
}

/// Complete highlight state for a document
#[derive(Debug, Clone)]
pub struct SyntaxHighlights {
    /// Map of line number → tokens
    pub lines: HashMap<usize, LineHighlights>,
    /// Document version this corresponds to
    pub version: u64,
    /// Primary language of document
    pub language: LanguageId,
}
```

### Highlight Names

Standard capture names mapped to theme colors:

```rust
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",           // @attribute
    "boolean",             // @boolean (true, false)
    "comment",             // @comment
    "constant",            // @constant
    "constant.builtin",    // @constant.builtin (null, nil)
    "constructor",         // @constructor (new Foo)
    "escape",              // @escape (string escapes)
    "function",            // @function
    "function.builtin",    // @function.builtin (echo, print)
    "function.method",     // @function.method
    "keyword",             // @keyword
    "keyword.return",      // @keyword.return
    "keyword.function",    // @keyword.function (function, fn)
    "keyword.operator",    // @keyword.operator (and, or)
    "number",              // @number
    "operator",            // @operator
    "property",            // @property
    "punctuation.bracket", // @punctuation.bracket
    "punctuation.delimiter", // @punctuation.delimiter
    "string",              // @string
    "string.special",      // @string.special (regex, heredoc)
    "tag",                 // @tag (HTML tags)
    "tag.attribute",       // @tag.attribute
    "type",                // @type
    "type.builtin",        // @type.builtin (int, string, bool)
    "variable",            // @variable
    "variable.builtin",    // @variable.builtin ($this, self)
    "variable.parameter",  // @variable.parameter
];

pub type HighlightId = u16; // Index into HIGHLIGHT_NAMES
```

### Language Configuration

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    PlainText,
    Php,
    Html,
    Css,
    JavaScript,
}

impl LanguageId {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "php" | "phtml" => LanguageId::Php,
            "html" | "htm" => LanguageId::Html,
            "css" => LanguageId::Css,
            "js" => LanguageId::JavaScript,
            _ => LanguageId::PlainText,
        }
    }
}
```

---

## Message & Command Flow

### New Messages

```rust
// In src/messages.rs

/// Syntax-related messages
#[derive(Debug, Clone)]
pub enum SyntaxMsg {
    /// Request syntax parsing (after edit debounce)
    RequestParse {
        document_id: DocumentId,
        version: u64,
    },
    /// Syntax tree updated (from background thread)
    ParseCompleted {
        document_id: DocumentId,
        version: u64,
        highlights: SyntaxHighlights,
    },
    /// Language changed for document
    LanguageChanged {
        document_id: DocumentId,
        language: LanguageId,
    },
}

// Add to top-level Msg enum
pub enum Msg {
    Editor(EditorMsg),
    Document(DocumentMsg),
    Ui(UiMsg),
    Layout(LayoutMsg),
    App(AppMsg),
    Syntax(SyntaxMsg),  // NEW
}
```

### New Commands

```rust
// In src/commands.rs

pub enum Cmd {
    None,
    Redraw,
    SaveFile { path: PathBuf, content: String },
    LoadFile { path: PathBuf },
    Batch(Vec<Cmd>),

    // NEW: Syntax highlighting commands
    ParseSyntax {
        document_id: DocumentId,
        source: String,      // Full document text
        version: u64,        // For staleness check
        language: LanguageId,
        edit: Option<EditInfo>, // For incremental parse
    },
}

/// Information about an edit for incremental parsing
#[derive(Debug, Clone)]
pub struct EditInfo {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_line: usize,
    pub start_col: usize,
    pub old_end_line: usize,
    pub old_end_col: usize,
    pub new_end_line: usize,
    pub new_end_col: usize,
}
```

---

## Integration Points

### Document Model Changes

```rust
// In src/model/document.rs

pub struct Document {
    pub buffer: Rope,
    pub undo_stack: Vec<EditOperation>,
    pub redo_stack: Vec<EditOperation>,
    pub file_path: Option<PathBuf>,
    pub is_modified: bool,

    // NEW: Syntax highlighting state
    pub language: LanguageId,
    pub syntax_highlights: Option<SyntaxHighlights>,
    pub syntax_version: u64,  // Last version we requested parse for
}

impl Document {
    /// Get highlight tokens for a line (returns empty if not available)
    pub fn get_line_highlights(&self, line: usize) -> &[HighlightToken] {
        self.syntax_highlights
            .as_ref()
            .and_then(|h| h.lines.get(&line))
            .map(|lh| lh.tokens.as_slice())
            .unwrap_or(&[])
    }
}
```

### Update Handler

```rust
// In src/update/syntax.rs (NEW FILE)

use crate::commands::Cmd;
use crate::messages::SyntaxMsg;
use crate::model::AppModel;

pub fn update_syntax(model: &mut AppModel, msg: SyntaxMsg) -> Option<Cmd> {
    match msg {
        SyntaxMsg::RequestParse { document_id, version } => {
            let doc = model.editor_area.documents.get(&document_id)?;

            // Only parse if version matches (avoid stale requests)
            if doc.version != version {
                return None;
            }

            let source = doc.buffer.to_string();
            let language = doc.language;

            Some(Cmd::ParseSyntax {
                document_id,
                source,
                version,
                language,
                edit: None, // Full parse on first request
            })
        }

        SyntaxMsg::ParseCompleted { document_id, version, highlights } => {
            if let Some(doc) = model.editor_area.documents.get_mut(&document_id) {
                // Only apply if version still matches (not stale)
                if doc.version == version {
                    doc.syntax_highlights = Some(highlights);
                }
            }
            Some(Cmd::Redraw)
        }

        SyntaxMsg::LanguageChanged { document_id, language } => {
            if let Some(doc) = model.editor_area.documents.get_mut(&document_id) {
                doc.language = language;
                doc.syntax_highlights = None; // Clear, will re-parse
            }
            // Trigger new parse
            let version = model.editor_area.documents
                .get(&document_id)
                .map(|d| d.version)
                .unwrap_or(0);
            Some(Cmd::ParseSyntax {
                document_id,
                source: model.editor_area.documents
                    .get(&document_id)
                    .map(|d| d.buffer.to_string())
                    .unwrap_or_default(),
                version,
                language,
                edit: None,
            })
        }
    }
}
```

### Command Processor

```rust
// In src/app.rs, add to process_cmd()

Cmd::ParseSyntax { document_id, source, version, language, edit } => {
    let tx = self.msg_tx.clone();

    std::thread::spawn(move || {
        // Create parser for language (or use cached)
        let highlights = parse_and_highlight(&source, language, edit);

        let _ = tx.send(Msg::Syntax(SyntaxMsg::ParseCompleted {
            document_id,
            version,
            highlights,
        }));
    });
}
```

---

## Background Parser Implementation

### Parser Pool

```rust
// In src/syntax/parser.rs (NEW FILE)

use std::collections::HashMap;
use tree_sitter::{Parser, Language, Tree, Query, QueryCursor, InputEdit, Point};

/// Thread-local parser state (parsers are !Sync)
pub struct ParserState {
    parsers: HashMap<LanguageId, Parser>,
    trees: HashMap<DocumentId, Tree>,
    queries: HashMap<LanguageId, Query>,
}

impl ParserState {
    pub fn new() -> Self {
        let mut state = Self {
            parsers: HashMap::new(),
            trees: HashMap::new(),
            queries: HashMap::new(),
        };

        // Pre-initialize common languages
        state.init_language(LanguageId::Php);
        state.init_language(LanguageId::Html);
        state.init_language(LanguageId::Css);

        state
    }

    fn init_language(&mut self, lang: LanguageId) {
        let (ts_lang, highlights_scm) = match lang {
            LanguageId::Php => (
                tree_sitter_php::LANGUAGE_PHP.into(),
                include_str!("../../queries/php/highlights.scm"),
            ),
            LanguageId::Html => (
                tree_sitter_html::LANGUAGE.into(),
                include_str!("../../queries/html/highlights.scm"),
            ),
            LanguageId::Css => (
                tree_sitter_css::LANGUAGE.into(),
                include_str!("../../queries/css/highlights.scm"),
            ),
            _ => return,
        };

        let mut parser = Parser::new();
        parser.set_language(&ts_lang).expect("Language setup failed");
        self.parsers.insert(lang, parser);

        if let Ok(query) = Query::new(&ts_lang, highlights_scm) {
            self.queries.insert(lang, query);
        }
    }

    /// Parse document and extract highlights
    pub fn parse_and_highlight(
        &mut self,
        source: &str,
        language: LanguageId,
        doc_id: DocumentId,
        edit: Option<&EditInfo>,
    ) -> SyntaxHighlights {
        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => return SyntaxHighlights::default(),
        };

        // Get or create tree
        let old_tree = if let Some(edit_info) = edit {
            self.trees.get_mut(&doc_id).map(|tree| {
                tree.edit(&edit_info.to_input_edit());
                tree.clone()
            })
        } else {
            None
        };

        // Parse (incremental if old_tree provided)
        let tree = match parser.parse(source.as_bytes(), old_tree.as_ref()) {
            Some(t) => t,
            None => return SyntaxHighlights::default(),
        };

        // Extract highlights using query
        let highlights = self.extract_highlights(source, &tree, language);

        // Cache tree for incremental parsing
        self.trees.insert(doc_id, tree);

        highlights
    }

    fn extract_highlights(
        &self,
        source: &str,
        tree: &Tree,
        language: LanguageId,
    ) -> SyntaxHighlights {
        let query = match self.queries.get(&language) {
            Some(q) => q,
            None => return SyntaxHighlights::default(),
        };

        let mut highlights = SyntaxHighlights {
            lines: HashMap::new(),
            version: 0,
            language,
        };

        let mut cursor = QueryCursor::new();
        let source_bytes = source.as_bytes();

        for (match_, capture_idx) in cursor.captures(query, tree.root_node(), source_bytes) {
            let capture = match_.captures[capture_idx];
            let capture_name = &query.capture_names()[capture.index as usize];

            // Map capture name to highlight ID
            let highlight_id = HIGHLIGHT_NAMES
                .iter()
                .position(|&name| name == *capture_name)
                .unwrap_or(0) as HighlightId;

            let node = capture.node;
            let start = node.start_position();
            let end = node.end_position();

            // Handle multi-line nodes (split into per-line tokens)
            for line in start.row..=end.row {
                let start_col = if line == start.row { start.column } else { 0 };
                let end_col = if line == end.row {
                    end.column
                } else {
                    // Get line length from source
                    source.lines().nth(line).map(|l| l.len()).unwrap_or(0)
                };

                highlights
                    .lines
                    .entry(line)
                    .or_default()
                    .tokens
                    .push(HighlightToken {
                        start_col,
                        end_col,
                        highlight: highlight_id,
                    });
            }
        }

        // Sort tokens within each line by start position
        for line_highlights in highlights.lines.values_mut() {
            line_highlights.tokens.sort_by_key(|t| t.start_col);
        }

        highlights
    }
}
```

### EditInfo Conversion

```rust
impl EditInfo {
    pub fn to_input_edit(&self) -> InputEdit {
        InputEdit {
            start_byte: self.start_byte,
            old_end_byte: self.old_end_byte,
            new_end_byte: self.new_end_byte,
            start_position: Point::new(self.start_line, self.start_col),
            old_end_position: Point::new(self.old_end_line, self.old_end_col),
            new_end_position: Point::new(self.new_end_line, self.new_end_col),
        }
    }

    /// Create EditInfo from a ropey edit
    pub fn from_rope_edit(
        rope: &Rope,
        char_start: usize,
        char_old_end: usize,
        new_text_len: usize,
    ) -> Self {
        let start_byte = rope.char_to_byte(char_start);
        let old_end_byte = rope.char_to_byte(char_old_end);
        let new_end_byte = start_byte + new_text_len;

        let start_line = rope.char_to_line(char_start);
        let start_col = char_start - rope.line_to_char(start_line);

        let old_end_line = rope.char_to_line(char_old_end.min(rope.len_chars()));
        let old_end_col = char_old_end.saturating_sub(
            rope.line_to_char(old_end_line)
        );

        // For new_end, we need to calculate based on new content
        // This is approximate; proper impl would track actual new lines
        let new_end_line = start_line; // Simplified
        let new_end_col = start_col + new_text_len;

        Self {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_line,
            start_col,
            old_end_line,
            old_end_col,
            new_end_line,
            new_end_col,
        }
    }
}
```

---

## Rendering Integration

### Theme Extension

```rust
// In src/theme.rs, add to EditorTheme

pub struct EditorTheme {
    pub background: Color,
    pub foreground: Color,
    pub cursor_color: Color,
    pub selection_background: Color,
    // ... existing fields ...

    // NEW: Syntax highlighting colors
    pub syntax: SyntaxTheme,
}

#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    pub keyword: Color,
    pub keyword_control: Color,   // if, for, return
    pub function: Color,
    pub function_builtin: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub type_name: Color,
    pub variable: Color,
    pub variable_builtin: Color,  // $this, self
    pub property: Color,
    pub operator: Color,
    pub punctuation: Color,
    pub tag: Color,               // HTML tags
    pub attribute: Color,         // HTML/XML attributes
    pub constant: Color,
    pub escape: Color,            // \n, \t in strings
}

impl SyntaxTheme {
    pub fn get_color(&self, highlight_id: HighlightId) -> Color {
        match HIGHLIGHT_NAMES.get(highlight_id as usize) {
            Some(&"keyword") | Some(&"keyword.function") | Some(&"keyword.return")
                => self.keyword,
            Some(&"function") | Some(&"function.method")
                => self.function,
            Some(&"function.builtin")
                => self.function_builtin,
            Some(&"string") | Some(&"string.special")
                => self.string,
            Some(&"number")
                => self.number,
            Some(&"comment")
                => self.comment,
            Some(&"type") | Some(&"type.builtin")
                => self.type_name,
            Some(&"variable") | Some(&"variable.parameter")
                => self.variable,
            Some(&"variable.builtin")
                => self.variable_builtin,
            Some(&"property") | Some(&"tag.attribute")
                => self.property,
            Some(&"operator") | Some(&"keyword.operator")
                => self.operator,
            Some(&"punctuation.bracket") | Some(&"punctuation.delimiter")
                => self.punctuation,
            Some(&"tag")
                => self.tag,
            Some(&"attribute")
                => self.attribute,
            Some(&"constant") | Some(&"constant.builtin") | Some(&"boolean")
                => self.constant,
            Some(&"escape")
                => self.escape,
            _ => self.variable, // Default fallback
        }
    }
}
```

### View Rendering

```rust
// In src/view.rs, modify text rendering

fn render_line_with_highlights(
    buffer: &mut [u32],
    font: &Font,
    glyph_cache: &mut GlyphCache,
    font_size: f32,
    ascent: f32,
    width: u32,
    height: u32,
    x: usize,
    y: usize,
    line_text: &str,
    tokens: &[HighlightToken],
    theme: &Theme,
    default_color: u32,
) {
    if tokens.is_empty() {
        // No highlighting, use default color
        draw_text(buffer, font, glyph_cache, font_size, ascent,
                  width, height, x, y, line_text, default_color);
        return;
    }

    let char_width = // ... get from renderer
    let mut current_col = 0;
    let mut token_idx = 0;

    for (col, ch) in line_text.char_indices() {
        // Find applicable token
        while token_idx < tokens.len() && tokens[token_idx].end_col <= col {
            token_idx += 1;
        }

        let color = if token_idx < tokens.len()
            && col >= tokens[token_idx].start_col
            && col < tokens[token_idx].end_col
        {
            theme.editor.syntax.get_color(tokens[token_idx].highlight).to_argb_u32()
        } else {
            default_color
        };

        // Draw single character with color
        let char_x = x + (col as f32 * char_width) as usize;
        draw_char(buffer, font, glyph_cache, font_size, ascent,
                  width, height, char_x, y, ch, color);
    }
}
```

---

## Language Injection (PHP + HTML/CSS)

PHP files often contain HTML, CSS, and JavaScript. Tree-sitter handles this via injection queries.

### Injection Query

```scheme
; queries/php/injections.scm

; HTML content between <?php ?> tags
((text) @injection.content
 (#set! injection.language "html"))

; CSS in style attributes
((attribute
  (attribute_name) @_name
  (quoted_attribute_value (attribute_value) @injection.content))
 (#eq? @_name "style")
 (#set! injection.language "css"))

; JavaScript in onclick, onload, etc.
((attribute
  (attribute_name) @_name
  (quoted_attribute_value (attribute_value) @injection.content))
 (#match? @_name "^on")
 (#set! injection.language "javascript"))
```

### Multi-Layer Parsing

```rust
pub struct SyntaxLayer {
    pub language: LanguageId,
    pub tree: Tree,
    pub ranges: Vec<Range>,  // Where this layer applies
    pub depth: usize,        // 0 = root, 1+ = injected
}

pub struct MultiLanguageDocument {
    pub layers: Vec<SyntaxLayer>,
}

impl MultiLanguageDocument {
    /// Get the most specific language at a byte position
    pub fn language_at(&self, byte: usize) -> LanguageId {
        self.layers
            .iter()
            .filter(|layer| {
                layer.ranges.iter().any(|r|
                    byte >= r.start_byte && byte < r.end_byte
                )
            })
            .max_by_key(|layer| layer.depth)
            .map(|layer| layer.language)
            .unwrap_or(LanguageId::PlainText)
    }
}
```

---

## Performance Considerations

### Debouncing

Don't parse on every keystroke:

```rust
const PARSE_DEBOUNCE_MS: u64 = 50;

// In document edit handler
fn schedule_parse(&mut self, document_id: DocumentId) {
    let now = Instant::now();
    self.pending_parse = Some((document_id, now));

    // In tick() or about_to_wait(), check if debounce elapsed
}

fn check_pending_parse(&mut self) -> Option<Cmd> {
    if let Some((doc_id, scheduled_at)) = self.pending_parse.take() {
        if scheduled_at.elapsed() >= Duration::from_millis(PARSE_DEBOUNCE_MS) {
            return Some(Cmd::ParseSyntax { ... });
        } else {
            self.pending_parse = Some((doc_id, scheduled_at)); // Put back
        }
    }
    None
}
```

### Viewport-Limited Queries

Only query visible lines + buffer:

```rust
const HIGHLIGHT_BUFFER_LINES: usize = 50;

fn extract_visible_highlights(
    &self,
    source: &str,
    tree: &Tree,
    language: LanguageId,
    visible_start_line: usize,
    visible_end_line: usize,
) -> SyntaxHighlights {
    let buffer_start = visible_start_line.saturating_sub(HIGHLIGHT_BUFFER_LINES);
    let buffer_end = visible_end_line + HIGHLIGHT_BUFFER_LINES;

    // Convert line range to byte range
    let start_byte = line_to_byte(source, buffer_start);
    let end_byte = line_to_byte(source, buffer_end);

    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(start_byte..end_byte);

    // ... rest of extraction
}
```

### Cancellation

Cancel stale parses when document changes again:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct ParseTask {
    cancel_flag: Arc<AtomicBool>,
    version: u64,
}

// Before spawning new parse, cancel previous
if let Some(old_task) = self.current_parse.take() {
    old_task.cancel_flag.store(true, Ordering::SeqCst);
}
```

---

## File Structure

```
src/
├── syntax/                  # NEW MODULE
│   ├── mod.rs              # Public exports
│   ├── parser.rs           # ParserState, background parsing
│   ├── highlights.rs       # HighlightToken, SyntaxHighlights
│   ├── languages.rs        # LanguageId, language detection
│   └── injection.rs        # Multi-language layer handling
├── update/
│   ├── mod.rs              # Add Msg::Syntax dispatch
│   └── syntax.rs           # NEW: SyntaxMsg handlers
├── commands.rs             # Add Cmd::ParseSyntax
├── messages.rs             # Add SyntaxMsg
└── theme.rs                # Add SyntaxTheme

queries/                     # NEW: Tree-sitter query files
├── php/
│   ├── highlights.scm
│   ├── injections.scm
│   └── locals.scm
├── html/
│   ├── highlights.scm
│   └── injections.scm
├── css/
│   └── highlights.scm
└── javascript/
    ├── highlights.scm
    └── locals.scm
```

---

## Dependencies

```toml
# Cargo.toml additions

[dependencies]
tree-sitter = "0.25"
tree-sitter-highlight = "0.25"  # Optional, if using high-level API

# Language grammars
tree-sitter-php = "0.24"
tree-sitter-html = "0.23"
tree-sitter-css = "0.25"
tree-sitter-javascript = "0.23"  # For PHP injection support
```

---

## Future: LSP Integration

The architecture is designed to support semantic highlighting from LSP:

```rust
/// Semantic tokens from LSP (richer than syntactic)
pub struct SemanticToken {
    pub line: usize,
    pub start_col: usize,
    pub length: usize,
    pub token_type: SemanticTokenType,
    pub modifiers: SemanticModifiers,
}

/// Merge semantic tokens with syntactic highlights
/// Semantic tokens take precedence where they exist
pub fn merge_highlights(
    syntactic: &SyntaxHighlights,
    semantic: &[SemanticToken],
) -> SyntaxHighlights {
    // Semantic tokens overlay syntactic tokens
    // Split syntactic tokens at semantic boundaries
    // ... merging logic
}
```

LSP message flow would be similar:

```
Document Changed → Cmd::RequestSemanticTokens { uri, version }
                      │
                      ▼ (background LSP request)

Msg::SemanticTokensReceived { uri, version, tokens }
                      │
                      ▼ (update handler)

Merge with syntactic → Store in document → Cmd::Redraw
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (3-4 days)

- [ ] Add `src/syntax/` module structure
- [ ] Define `HighlightToken`, `SyntaxHighlights`, `LanguageId`
- [ ] Add `SyntaxMsg` and `Cmd::ParseSyntax`
- [ ] Implement basic `update_syntax()` handler
- [ ] Add to `process_cmd()` for background parsing
- [ ] Add tree-sitter dependencies

### Phase 2: Single Language (2 days)

- [ ] Implement `ParserState` for PHP
- [ ] Add PHP highlights.scm query file
- [ ] Connect parsing to document edits
- [ ] Implement debouncing
- [ ] Basic tests

### Phase 3: Rendering (2 days)

- [ ] Add `SyntaxTheme` to theme system
- [ ] Update YAML theme files with syntax colors
- [ ] Modify `draw_text` to use token colors
- [ ] Optimize character-by-character rendering

### Phase 4: Multi-Language (2-3 days)

- [ ] Add HTML, CSS grammar support
- [ ] Implement injection parsing for PHP
- [ ] Add `MultiLanguageDocument` layer tracking
- [ ] Test PHP + HTML + CSS files

### Phase 5: Polish (1-2 days)

- [ ] Viewport-limited highlighting
- [ ] Cancellation of stale parses
- [ ] Performance profiling
- [ ] Documentation

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_php_keyword_highlighting() {
    let source = "<?php function foo() { return 42; }";
    let highlights = parse_and_highlight(source, LanguageId::Php, None);

    // "function" should be highlighted as keyword
    let line0 = &highlights.lines[&0];
    let keyword_token = line0.tokens.iter()
        .find(|t| t.start_col == 6 && t.end_col == 14)
        .expect("function keyword not found");
    assert_eq!(HIGHLIGHT_NAMES[keyword_token.highlight as usize], "keyword.function");
}

#[test]
fn test_incremental_parse() {
    let mut state = ParserState::new();
    let source1 = "<?php $x = 1;";
    state.parse_and_highlight(source1, LanguageId::Php, doc_id, None);

    // Insert " + 2" after "1"
    let edit = EditInfo { start_byte: 12, old_end_byte: 12, new_end_byte: 16, ... };
    let source2 = "<?php $x = 1 + 2;";
    let highlights = state.parse_and_highlight(source2, LanguageId::Php, doc_id, Some(&edit));

    // Should have operator "+" highlighted
    assert!(highlights.lines[&0].tokens.iter()
        .any(|t| HIGHLIGHT_NAMES[t.highlight as usize] == "operator"));
}
```

### Integration Tests

```rust
#[test]
fn test_edit_triggers_reparse() {
    let mut model = test_model_with_php();

    // Type a character
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('$')));

    // Check that ParseSyntax command was generated
    // (after debounce in real app)
}
```

---

## References

- [Tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Tree-sitter Syntax Highlighting](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html)
- [tree-sitter-highlight crate](https://crates.io/crates/tree-sitter-highlight)
- [tree-sitter-php](https://github.com/tree-sitter/tree-sitter-php)
- [nvim-treesitter queries](https://github.com/nvim-treesitter/nvim-treesitter/tree/master/queries) (high-quality highlight queries)
- Our async pattern: [src/app.rs](../src/app.rs) `process_cmd()` + `msg_tx`/`msg_rx` channels
