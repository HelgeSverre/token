# Snippets (aka Jetbrains "Live Templates")

User-defined code snippets with trigger word expansion and tabstop navigation

> **Status:** Planning
> **Priority:** P2
> **Effort:** L
> **Created:** 2025-12-19
> **Milestone:** 6 - Productivity
> **Feature ID:** F-180

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Snippet Syntax](#snippet-syntax)
5. [Expansion Engine](#expansion-engine)
6. [Keybindings](#keybindings)
7. [Implementation Plan](#implementation-plan)
8. [Testing Strategy](#testing-strategy)
9. [References](#references)

---

## Overview

### Current State

The editor currently has:

- Multi-cursor support with synchronized editing
- Configurable keybindings via YAML
- Language detection from file extension
- Tab/indent handling
- Undo/redo with atomic multi-cursor operations

However, there is no snippet system for expanding trigger words into templates with placeholders.

### Goals

1. **Trigger word expansion** - Type a prefix + Tab to expand to full snippet
2. **Tabstops** - Navigate between placeholders with Tab/Shift+Tab
3. **Placeholder text** - Default values shown and replaced on typing
4. **Mirror tabstops** - Same variable in multiple places updates together
5. **Language-scoped snippets** - Snippets active only for specific file types
6. **User configuration** - Snippets defined in YAML files
7. **Built-in snippets** - Common patterns for supported languages

### Non-Goals (This Phase)

- Visual snippet picker/palette (use command palette instead)
- Snippet variables like `$TM_FILENAME`, `$CURRENT_DATE` (future)
- Nested snippets (snippet within snippet)
- Choice/dropdown placeholders
- Regex transformations on placeholders
- Importing VS Code snippet format

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Snippet Expansion Flow                              │
│                                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────────┐  │
│  │  User types  │───▶│  Check for   │───▶│  Found: Expand snippet       │  │
│  │  "fn" + Tab  │    │  trigger     │    │  Not found: Insert tab       │  │
│  └──────────────┘    └──────────────┘    └───────────────┬──────────────┘  │
│                                                           │                 │
│                                                           ▼                 │
│                           ┌──────────────────────────────────────────────┐ │
│                           │             SnippetSession                    │ │
│                           │  - Active tabstops                            │ │
│                           │  - Current tabstop index                      │ │
│                           │  - Mirror groups                              │ │
│                           │  - Original cursor positions                  │ │
│                           └────────────────────┬─────────────────────────┘ │
│                                                │                            │
│  ┌──────────────┐    ┌──────────────┐         │                            │
│  │  Tab: Next   │◀───│  User edits  │◀────────┘                            │
│  │  tabstop     │    │  placeholder │                                      │
│  └──────────────┘    └──────────────┘                                      │
│         │                   │                                               │
│         │                   ▼                                               │
│         │           ┌──────────────────────────────────────────────────┐   │
│         │           │  Mirror Update: Sync all instances of $1, $2...  │   │
│         │           └──────────────────────────────────────────────────┘   │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  $0 reached: Exit snippet mode, place cursor at final position       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── snippets/                    # NEW MODULE
│   ├── mod.rs                   # Public exports
│   ├── registry.rs              # SnippetRegistry, loading, lookup
│   ├── parser.rs                # Parse snippet body syntax
│   ├── session.rs               # SnippetSession, tabstop navigation
│   ├── expansion.rs             # Expand snippet into document
│   └── builtin.rs               # Built-in snippets per language
├── model/
│   └── editor.rs                # + active_snippet_session: Option<SnippetSession>
├── update/
│   └── snippets.rs              # NEW: Snippet message handler
└── keymap/
    └── command.rs               # + ExpandSnippetOrTab, NextTabstop, etc.

~/.config/token-editor/
└── snippets/
    ├── rust.yaml                # User Rust snippets
    ├── javascript.yaml          # User JavaScript snippets
    └── global.yaml              # Language-agnostic snippets
```

### Integration with Multi-Cursor

Snippets work with multi-cursor mode:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Before: Multiple cursors, each with "fn" before it                         │
│                                                                              │
│    fn|                  fn|                  fn|                             │
│                                                                              │
│  After Tab (expand at each cursor):                                          │
│                                                                              │
│    fn [name]() {        fn [name]() {        fn [name]() {                   │
│        |                    |                    |                           │
│    }                    }                    }                               │
│                                                                              │
│  [name] placeholders are synchronized across all expansions                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Snippet Definition

```rust
// src/snippets/mod.rs

use std::collections::HashMap;
use crate::syntax::LanguageId;

/// A single snippet definition
#[derive(Debug, Clone)]
pub struct Snippet {
    /// Unique identifier (e.g., "fn", "for", "impl")
    pub prefix: String,

    /// Display name shown in UI
    pub name: String,

    /// Description for documentation
    pub description: Option<String>,

    /// The snippet body (with tabstops)
    pub body: SnippetBody,

    /// Language scope (None = global)
    pub scope: Option<LanguageId>,

    /// Source of this snippet
    pub source: SnippetSource,
}

/// Where the snippet was defined
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetSource {
    /// Built-in snippet embedded in binary
    Builtin,
    /// User-defined in config directory
    User,
}

/// Parsed snippet body ready for expansion
#[derive(Debug, Clone)]
pub struct SnippetBody {
    /// Body parts in order
    pub parts: Vec<SnippetPart>,
    /// All tabstop indices used (for validation)
    pub tabstop_indices: Vec<u32>,
    /// Whether this snippet has a final tabstop ($0)
    pub has_final_tabstop: bool,
}

/// A part of a snippet body
#[derive(Debug, Clone)]
pub enum SnippetPart {
    /// Literal text (no placeholders)
    Text(String),

    /// Simple tabstop: $1, $2, etc.
    Tabstop {
        index: u32,
    },

    /// Tabstop with placeholder: ${1:default}
    Placeholder {
        index: u32,
        default: String,
    },

    /// Mirror of another tabstop (same index, no default)
    Mirror {
        index: u32,
    },
}

impl SnippetBody {
    /// Get all unique tabstop indices in order
    pub fn tabstop_order(&self) -> Vec<u32> {
        let mut indices: Vec<u32> = self.parts
            .iter()
            .filter_map(|p| match p {
                SnippetPart::Tabstop { index } => Some(*index),
                SnippetPart::Placeholder { index, .. } => Some(*index),
                SnippetPart::Mirror { index } => Some(*index),
                SnippetPart::Text(_) => None,
            })
            .collect();

        indices.sort_unstable();
        indices.dedup();

        // Move 0 to the end (final tabstop)
        if let Some(pos) = indices.iter().position(|&i| i == 0) {
            indices.remove(pos);
            indices.push(0);
        }

        indices
    }
}
```

### Snippet Registry

```rust
// src/snippets/registry.rs

use std::collections::HashMap;
use std::path::PathBuf;
use crate::syntax::LanguageId;

/// Registry of all available snippets
#[derive(Debug, Default)]
pub struct SnippetRegistry {
    /// Snippets indexed by (language, prefix)
    /// None language = global snippets
    snippets: HashMap<(Option<LanguageId>, String), Snippet>,

    /// All prefixes for quick lookup during typing
    /// Maps language → set of prefixes
    prefix_cache: HashMap<Option<LanguageId>, Vec<String>>,
}

impl SnippetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load built-in snippets
    pub fn load_builtins(&mut self) {
        for snippet in builtin::rust_snippets() {
            self.register(snippet);
        }
        for snippet in builtin::javascript_snippets() {
            self.register(snippet);
        }
        for snippet in builtin::global_snippets() {
            self.register(snippet);
        }
    }

    /// Load user snippets from config directory
    pub fn load_user_snippets(&mut self) -> Result<(), String> {
        let snippets_dir = crate::config_paths::snippets_dir()
            .ok_or("Could not find config directory")?;

        if !snippets_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&snippets_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "yaml" || ext == "yml") {
                self.load_snippet_file(&path)?;
            }
        }

        Ok(())
    }

    fn load_snippet_file(&mut self, path: &PathBuf) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        let snippets: Vec<SnippetDef> = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        // Determine language scope from filename
        let language = path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|name| {
                if name == "global" {
                    None
                } else {
                    LanguageId::from_name(name)
                }
            });

        for def in snippets {
            let snippet = def.into_snippet(language)?;
            self.register(snippet);
        }

        Ok(())
    }

    /// Register a snippet
    pub fn register(&mut self, snippet: Snippet) {
        let key = (snippet.scope, snippet.prefix.clone());

        // Update prefix cache
        self.prefix_cache
            .entry(snippet.scope)
            .or_default()
            .push(snippet.prefix.clone());

        self.snippets.insert(key, snippet);
    }

    /// Find snippet by prefix for a language
    /// Checks language-specific first, then global
    pub fn find(&self, prefix: &str, language: LanguageId) -> Option<&Snippet> {
        // Try language-specific first
        if let Some(snippet) = self.snippets.get(&(Some(language), prefix.to_string())) {
            return Some(snippet);
        }

        // Fall back to global
        self.snippets.get(&(None, prefix.to_string()))
    }

    /// Check if a prefix could match any snippet for a language
    /// Used for optimizing when to check for expansion
    pub fn has_prefix(&self, prefix: &str, language: LanguageId) -> bool {
        let check = |lang: Option<LanguageId>| {
            self.prefix_cache
                .get(&lang)
                .is_some_and(|prefixes| prefixes.iter().any(|p| p.starts_with(prefix)))
        };

        check(Some(language)) || check(None)
    }

    /// Get all snippets for a language (for command palette)
    pub fn snippets_for_language(&self, language: LanguageId) -> Vec<&Snippet> {
        let mut result: Vec<&Snippet> = self.snippets
            .iter()
            .filter(|((scope, _), _)| {
                scope.is_none() || *scope == Some(language)
            })
            .map(|(_, snippet)| snippet)
            .collect();

        result.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        result
    }
}
```

### Snippet Session (Active Expansion)

```rust
// src/snippets/session.rs

use std::collections::HashMap;
use crate::model::editor::{Cursor, Selection};
use crate::model::editor_area::DocumentId;

/// An active snippet expansion session
#[derive(Debug, Clone)]
pub struct SnippetSession {
    /// Document this session is active in
    pub document_id: DocumentId,

    /// All tabstops in this expansion
    pub tabstops: Vec<TabstopInstance>,

    /// Current tabstop index (position in tabstops vec)
    pub current_tabstop: usize,

    /// Mirror groups: tabstop_index → positions that should mirror
    pub mirrors: HashMap<u32, Vec<TabstopPosition>>,

    /// Original document revision when expansion started
    pub start_revision: u64,

    /// Range of the entire snippet in document (for cancellation)
    pub snippet_range: (usize, usize), // (start_offset, end_offset)
}

/// A tabstop instance in the document
#[derive(Debug, Clone)]
pub struct TabstopInstance {
    /// Tabstop index from snippet ($1, $2, etc.)
    pub index: u32,

    /// Position in document
    pub position: TabstopPosition,

    /// Current value (may differ from original placeholder)
    pub current_value: String,

    /// Whether this is a placeholder (has default) vs simple tabstop
    pub is_placeholder: bool,
}

/// Position of a tabstop in the document
#[derive(Debug, Clone, Copy)]
pub struct TabstopPosition {
    /// Line number (0-indexed)
    pub line: usize,
    /// Start column
    pub start_col: usize,
    /// End column (exclusive)
    pub end_col: usize,
}

impl TabstopPosition {
    /// Convert to a Selection for the editor
    pub fn to_selection(&self) -> Selection {
        Selection {
            anchor: Cursor {
                line: self.line,
                column: self.start_col,
                desired_column: self.start_col,
            },
            head: Cursor {
                line: self.line,
                column: self.end_col,
                desired_column: self.end_col,
            },
        }
    }

    /// Check if this position contains a cursor
    pub fn contains(&self, line: usize, col: usize) -> bool {
        self.line == line && col >= self.start_col && col <= self.end_col
    }

    /// Adjust position after text insertion/deletion
    pub fn adjust(&mut self, edit_line: usize, edit_col: usize, delta: isize) {
        if self.line == edit_line {
            if self.start_col >= edit_col {
                self.start_col = (self.start_col as isize + delta).max(0) as usize;
            }
            if self.end_col >= edit_col {
                self.end_col = (self.end_col as isize + delta).max(0) as usize;
            }
        }
    }
}

impl SnippetSession {
    /// Create a new session from expanded tabstops
    pub fn new(
        document_id: DocumentId,
        tabstops: Vec<TabstopInstance>,
        mirrors: HashMap<u32, Vec<TabstopPosition>>,
        start_revision: u64,
        snippet_range: (usize, usize),
    ) -> Self {
        Self {
            document_id,
            tabstops,
            current_tabstop: 0,
            mirrors,
            start_revision,
            snippet_range,
        }
    }

    /// Get the current tabstop
    pub fn current(&self) -> Option<&TabstopInstance> {
        self.tabstops.get(self.current_tabstop)
    }

    /// Move to next tabstop, returns true if there was a next one
    pub fn next(&mut self) -> bool {
        if self.current_tabstop + 1 < self.tabstops.len() {
            self.current_tabstop += 1;
            true
        } else {
            false
        }
    }

    /// Move to previous tabstop, returns true if there was a previous one
    pub fn prev(&mut self) -> bool {
        if self.current_tabstop > 0 {
            self.current_tabstop -= 1;
            true
        } else {
            false
        }
    }

    /// Check if we're at the final tabstop ($0)
    pub fn is_at_final(&self) -> bool {
        self.current()
            .map(|t| t.index == 0)
            .unwrap_or(true)
    }

    /// Get all positions that should mirror the current tabstop
    pub fn current_mirrors(&self) -> Vec<TabstopPosition> {
        self.current()
            .and_then(|t| self.mirrors.get(&t.index))
            .cloned()
            .unwrap_or_default()
    }

    /// Update all tabstop positions after an edit
    pub fn adjust_for_edit(&mut self, line: usize, col: usize, delta: isize) {
        for tabstop in &mut self.tabstops {
            tabstop.position.adjust(line, col, delta);
        }

        for positions in self.mirrors.values_mut() {
            for pos in positions {
                pos.adjust(line, col, delta);
            }
        }

        // Adjust snippet range
        if delta != 0 {
            self.snippet_range.1 = (self.snippet_range.1 as isize + delta).max(0) as usize;
        }
    }

    /// Update the current tabstop's value and sync mirrors
    pub fn update_current_value(&mut self, new_value: String) {
        if let Some(tabstop) = self.tabstops.get_mut(self.current_tabstop) {
            let old_len = tabstop.current_value.len();
            let new_len = new_value.len();
            let delta = new_len as isize - old_len as isize;

            tabstop.current_value = new_value;
            tabstop.position.end_col = tabstop.position.start_col + new_len;

            // Adjust subsequent tabstops on the same line
            for other in &mut self.tabstops[self.current_tabstop + 1..] {
                if other.position.line == tabstop.position.line {
                    other.position.start_col = (other.position.start_col as isize + delta).max(0) as usize;
                    other.position.end_col = (other.position.end_col as isize + delta).max(0) as usize;
                }
            }
        }
    }
}
```

### Editor State Extension

```rust
// In src/model/editor.rs

pub struct EditorState {
    // ... existing fields ...

    /// Active snippet session, if any
    pub snippet_session: Option<SnippetSession>,
}

impl EditorState {
    /// Check if a snippet session is active
    pub fn in_snippet_mode(&self) -> bool {
        self.snippet_session.is_some()
    }

    /// Cancel any active snippet session
    pub fn cancel_snippet(&mut self) {
        self.snippet_session = None;
    }
}
```

### Messages

```rust
// In src/messages.rs

/// Snippet-related messages
#[derive(Debug, Clone)]
pub enum SnippetMsg {
    /// Try to expand snippet at cursor, or insert tab if none
    ExpandOrTab,

    /// Move to next tabstop in active session
    NextTabstop,

    /// Move to previous tabstop in active session
    PrevTabstop,

    /// Cancel active snippet session
    Cancel,

    /// User typed while in snippet mode - update current tabstop
    UpdateTabstop { text: String },

    /// Expand a specific snippet by prefix
    ExpandSnippet { prefix: String },

    /// Open snippet picker in command palette
    OpenSnippetPicker,
}

// Add to Msg enum:
pub enum Msg {
    // ... existing variants ...
    Snippet(SnippetMsg),
}
```

### Commands

```rust
// In src/commands.rs

pub enum Cmd {
    // ... existing variants ...

    /// Insert snippet expansion into document
    InsertSnippet {
        document_id: DocumentId,
        /// Position to insert at (char offset)
        position: usize,
        /// Length of trigger prefix to replace
        prefix_len: usize,
        /// Expanded text to insert
        text: String,
        /// Tabstop positions (relative to insert point)
        tabstops: Vec<(u32, usize, usize)>, // (index, start, end)
    },
}
```

---

## Snippet Syntax

### Basic Format

Snippets are defined in YAML files. The body uses a VS Code-compatible syntax subset:

```yaml
# ~/.config/token-editor/snippets/rust.yaml

- prefix: fn
  name: Function
  description: Create a function
  body: |
    fn ${1:name}(${2:args}) {
        $0
    }

- prefix: impl
  name: Impl Block
  description: Create an impl block
  body: |
    impl ${1:Type} {
        $0
    }

- prefix: test
  name: Test Function
  description: Create a test function
  body: |
    #[test]
    fn ${1:test_name}() {
        $0
    }
```

### Tabstop Syntax

| Syntax | Description | Example |
|--------|-------------|---------|
| `$1`, `$2`, ... | Simple tabstop | `fn $1() {}` |
| `${1:default}` | Tabstop with placeholder | `fn ${1:name}() {}` |
| `$0` | Final cursor position | `fn name() { $0 }` |
| `${1:default}` ... `$1` | Mirror (same index) | `let ${1:x} = $1;` |

### Placeholder Text

Placeholder text is selected when tabstop is active:

```
# Snippet: fn ${1:name}() { $0 }

# After expansion:
fn [name]() {
    |
}
   ^^^^^
   "name" is selected, typing replaces it
```

### Mirror Tabstops

When the same index appears multiple times, they are mirrored:

```yaml
- prefix: for
  name: For Loop
  body: |
    for ${1:i} in ${2:iter} {
        println!("{}", $1);
        $0
    }
```

Typing at `$1` updates both occurrences simultaneously.

### Escaping

| Escape | Meaning |
|--------|---------|
| `\$` | Literal `$` |
| `\}` | Literal `}` (inside placeholder) |
| `\\` | Literal `\` |

---

## Expansion Engine

### Trigger Detection

```rust
// src/snippets/expansion.rs

use crate::model::Document;
use crate::snippets::SnippetRegistry;
use crate::syntax::LanguageId;

/// Check if cursor position could trigger a snippet
pub fn find_trigger_word(doc: &Document, cursor_line: usize, cursor_col: usize) -> Option<String> {
    let line = doc.get_line(cursor_line)?;
    let line_str: String = line.chars().take(cursor_col).collect();

    // Find word boundary before cursor
    // Word = alphanumeric + underscore
    let word_start = line_str
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphanumeric() && *c != '_')
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);

    let word = &line_str[word_start..];
    if word.is_empty() {
        return None;
    }

    Some(word.to_string())
}

/// Expand a snippet at the cursor position
pub fn expand_snippet(
    doc: &mut Document,
    cursor_line: usize,
    cursor_col: usize,
    snippet: &Snippet,
) -> Result<SnippetSession, String> {
    // Find trigger word to replace
    let trigger = find_trigger_word(doc, cursor_line, cursor_col)
        .ok_or("No trigger word found")?;

    if trigger != snippet.prefix {
        return Err("Trigger word doesn't match prefix".to_string());
    }

    // Calculate insert position
    let prefix_start_col = cursor_col - trigger.len();
    let insert_offset = doc.cursor_to_offset(cursor_line, prefix_start_col);

    // Expand snippet body with indentation
    let indent = get_line_indent(doc, cursor_line);
    let (expanded_text, tabstops) = expand_body(&snippet.body, &indent);

    // Delete trigger word
    let trigger_end = doc.cursor_to_offset(cursor_line, cursor_col);
    // ... delete range [insert_offset, trigger_end)

    // Insert expanded text
    // ... insert expanded_text at insert_offset

    // Create session with tabstop positions
    let session = create_session(doc, insert_offset, tabstops);

    Ok(session)
}

/// Expand snippet body with indentation
fn expand_body(body: &SnippetBody, indent: &str) -> (String, Vec<(u32, usize, usize)>) {
    let mut result = String::new();
    let mut tabstops = Vec::new();
    let mut offset = 0;

    for part in &body.parts {
        match part {
            SnippetPart::Text(text) => {
                // Apply indentation to each line except first
                let indented = text
                    .lines()
                    .enumerate()
                    .map(|(i, line)| {
                        if i == 0 {
                            line.to_string()
                        } else {
                            format!("{}{}", indent, line)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                result.push_str(&indented);
                offset += indented.len();
            }

            SnippetPart::Tabstop { index } => {
                tabstops.push((*index, offset, offset));
            }

            SnippetPart::Placeholder { index, default } => {
                tabstops.push((*index, offset, offset + default.len()));
                result.push_str(default);
                offset += default.len();
            }

            SnippetPart::Mirror { index } => {
                // Mirrors are resolved after initial expansion
                tabstops.push((*index, offset, offset));
            }
        }
    }

    (result, tabstops)
}

fn get_line_indent(doc: &Document, line: usize) -> String {
    doc.get_line(line)
        .map(|l| {
            l.chars()
                .take_while(|c| c.is_whitespace() && *c != '\n')
                .collect()
        })
        .unwrap_or_default()
}
```

### Tabstop Navigation

```rust
// src/update/snippets.rs

use crate::model::AppModel;
use crate::commands::Cmd;
use crate::messages::SnippetMsg;

pub fn update_snippet(model: &mut AppModel, msg: SnippetMsg) -> Option<Cmd> {
    match msg {
        SnippetMsg::ExpandOrTab => {
            // Check if in snippet mode - if so, advance tabstop
            if let Some(editor) = model.focused_editor_mut() {
                if editor.in_snippet_mode() {
                    return handle_next_tabstop(model);
                }
            }

            // Otherwise, try to expand snippet
            try_expand_snippet(model)
        }

        SnippetMsg::NextTabstop => {
            handle_next_tabstop(model)
        }

        SnippetMsg::PrevTabstop => {
            handle_prev_tabstop(model)
        }

        SnippetMsg::Cancel => {
            if let Some(editor) = model.focused_editor_mut() {
                editor.cancel_snippet();
            }
            Some(Cmd::Redraw)
        }

        SnippetMsg::UpdateTabstop { text } => {
            update_current_tabstop(model, &text)
        }

        SnippetMsg::ExpandSnippet { prefix } => {
            expand_specific_snippet(model, &prefix)
        }

        SnippetMsg::OpenSnippetPicker => {
            // Open command palette filtered to snippets
            Some(Cmd::Redraw)
        }
    }
}

fn handle_next_tabstop(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.focused_editor_mut()?;
    let session = editor.snippet_session.as_mut()?;

    if session.is_at_final() {
        // Exit snippet mode
        editor.snippet_session = None;
        return Some(Cmd::Redraw);
    }

    if session.next() {
        // Select the new tabstop
        if let Some(tabstop) = session.current() {
            editor.cursors = vec![Cursor {
                line: tabstop.position.line,
                column: tabstop.position.end_col,
                desired_column: tabstop.position.end_col,
            }];
            editor.selections = vec![tabstop.position.to_selection()];
        }
        Some(Cmd::Redraw)
    } else {
        // No more tabstops - exit snippet mode
        editor.snippet_session = None;
        Some(Cmd::Redraw)
    }
}

fn handle_prev_tabstop(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.focused_editor_mut()?;
    let session = editor.snippet_session.as_mut()?;

    if session.prev() {
        // Select the previous tabstop
        if let Some(tabstop) = session.current() {
            editor.cursors = vec![Cursor {
                line: tabstop.position.line,
                column: tabstop.position.end_col,
                desired_column: tabstop.position.end_col,
            }];
            editor.selections = vec![tabstop.position.to_selection()];
        }
        Some(Cmd::Redraw)
    } else {
        None
    }
}

fn update_current_tabstop(model: &mut AppModel, text: &str) -> Option<Cmd> {
    // This is called when user types while in snippet mode
    // Update the current tabstop value and sync mirrors

    let doc_id = model.focused_document_id()?;
    let doc = model.editor_area.documents.get_mut(&doc_id)?;
    let editor = model.focused_editor_mut()?;
    let session = editor.snippet_session.as_mut()?;

    let tabstop = session.current()?.clone();
    let mirrors = session.current_mirrors();

    // Calculate the change
    let old_value = &tabstop.current_value;
    let new_value = text;

    // Update mirrors in reverse order (to preserve offsets)
    for mirror_pos in mirrors.iter().rev() {
        let start = doc.cursor_to_offset(mirror_pos.line, mirror_pos.start_col);
        let end = doc.cursor_to_offset(mirror_pos.line, mirror_pos.end_col);
        // Replace text at mirror position
        // ... doc.buffer manipulation ...
    }

    // Update the tabstop value in session
    session.update_current_value(new_value.to_string());

    Some(Cmd::Redraw)
}

fn try_expand_snippet(model: &mut AppModel) -> Option<Cmd> {
    let doc_id = model.focused_document_id()?;
    let doc = model.editor_area.documents.get(&doc_id)?;
    let editor = model.focused_editor()?;

    let cursor = editor.cursors.first()?;
    let language = doc.language;

    // Find trigger word
    let trigger = find_trigger_word(doc, cursor.line, cursor.column)?;

    // Look up snippet
    let snippet = model.snippet_registry.find(&trigger, language)?;

    // Expand it
    // ... call expand_snippet() ...

    Some(Cmd::Redraw)
}
```

---

## Keybindings

| Action | Key | Context | Command |
|--------|-----|---------|---------|
| Expand snippet or insert tab | `Tab` | No selection, word before cursor | `ExpandSnippetOrTab` |
| Next tabstop | `Tab` | In snippet mode | `SnippetNextTabstop` |
| Previous tabstop | `Shift+Tab` | In snippet mode | `SnippetPrevTabstop` |
| Cancel snippet | `Escape` | In snippet mode | `SnippetCancel` |
| Open snippet picker | `Cmd+Shift+;` | Any | `OpenSnippetPicker` |

### Keymap Configuration

```yaml
# keymap.yaml additions

# Snippet expansion
- key: "tab"
  command: ExpandSnippetOrTab
  when: ["!has_selection", "!in_snippet"]

- key: "tab"
  command: SnippetNextTabstop
  when: ["in_snippet"]

- key: "shift+tab"
  command: SnippetPrevTabstop
  when: ["in_snippet"]

- key: "escape"
  command: SnippetCancel
  when: ["in_snippet"]

- key: "cmd+shift+;"
  command: OpenSnippetPicker
```

### Context Conditions

New conditions for keymap:

```rust
// In src/keymap/context.rs

pub enum Condition {
    // ... existing conditions ...

    /// Editor is in snippet expansion mode
    InSnippet,

    /// Cursor has a word before it (potential trigger)
    HasTriggerWord,
}
```

---

## Implementation Plan

### Phase 1: Core Data Structures

**Effort:** M (2-3 days)

- [ ] Create `src/snippets/mod.rs` module structure
- [ ] Implement `Snippet`, `SnippetBody`, `SnippetPart` types
- [ ] Implement `SnippetRegistry` with registration and lookup
- [ ] Add `snippet_session` field to `EditorState`
- [ ] Add config path helper for snippets directory

**Test:** Registry lookup returns correct snippet for language.

### Phase 2: Snippet Parser

**Effort:** M (2-3 days)

- [ ] Implement snippet body parser (tabstops, placeholders)
- [ ] Handle escape sequences
- [ ] Parse YAML snippet definitions
- [ ] Validate tabstop indices
- [ ] Unit tests for parser edge cases

**Test:** Parse `"fn ${1:name}() { $0 }"` produces correct parts.

### Phase 3: Built-in Snippets

**Effort:** S (1-2 days)

- [ ] Create `src/snippets/builtin.rs`
- [ ] Add Rust snippets (fn, impl, struct, enum, test, match, etc.)
- [ ] Add JavaScript snippets (function, arrow, class, etc.)
- [ ] Add global snippets (todo, fixme, etc.)
- [ ] Load builtins on startup

**Test:** Built-in "fn" snippet available for Rust files.

### Phase 4: Expansion Engine

**Effort:** L (3-4 days)

- [ ] Implement trigger word detection
- [ ] Implement `expand_snippet()` function
- [ ] Handle indentation preservation
- [ ] Create `SnippetSession` from expansion
- [ ] Integrate with document editing
- [ ] Handle multi-cursor expansion

**Test:** Typing "fn" + Tab expands to function template.

### Phase 5: Tabstop Navigation

**Effort:** M (2-3 days)

- [ ] Implement `SnippetSession` tabstop tracking
- [ ] Add `NextTabstop` and `PrevTabstop` handlers
- [ ] Select tabstop text on navigation
- [ ] Handle final tabstop ($0) exit
- [ ] Wire keybindings for Tab/Shift+Tab in snippet mode

**Test:** Tab cycles through tabstops, Shift+Tab goes back.

### Phase 6: Mirror Updates

**Effort:** M (2-3 days)

- [ ] Track mirror positions in session
- [ ] Update mirrors on tabstop edit
- [ ] Maintain position consistency after edits
- [ ] Handle multi-line tabstops
- [ ] Test complex mirror scenarios

**Test:** Editing mirrored tabstop updates all instances.

### Phase 7: User Configuration

**Effort:** S (1-2 days)

- [ ] Load user snippets from config directory
- [ ] User snippets override builtins with same prefix
- [ ] Error handling for malformed snippet files
- [ ] Documentation for snippet format

**Test:** User snippet in ~/.config overrides builtin.

### Phase 8: Polish

**Effort:** S (1-2 days)

- [ ] Add snippet picker to command palette
- [ ] Visual indication of snippet mode in status bar
- [ ] Escape to cancel snippet mode
- [ ] Handle edge cases (empty file, multi-cursor)
- [ ] Performance optimization

**Test:** Command palette shows available snippets filtered by language.

---

## Testing Strategy

### Unit Tests

```rust
// tests/snippets.rs

#[test]
fn test_parse_simple_tabstop() {
    let body = parse_snippet_body("Hello $1 world $2");

    assert_eq!(body.parts.len(), 4);
    assert!(matches!(&body.parts[1], SnippetPart::Tabstop { index: 1 }));
    assert!(matches!(&body.parts[3], SnippetPart::Tabstop { index: 2 }));
}

#[test]
fn test_parse_placeholder() {
    let body = parse_snippet_body("fn ${1:name}() {}");

    assert!(matches!(
        &body.parts[1],
        SnippetPart::Placeholder { index: 1, default } if default == "name"
    ));
}

#[test]
fn test_parse_mirror() {
    let body = parse_snippet_body("let ${1:x} = $1;");

    let indices: Vec<u32> = body.parts
        .iter()
        .filter_map(|p| match p {
            SnippetPart::Tabstop { index } => Some(*index),
            SnippetPart::Placeholder { index, .. } => Some(*index),
            SnippetPart::Mirror { index } => Some(*index),
            _ => None,
        })
        .collect();

    assert_eq!(indices, vec![1, 1]); // Both have index 1
}

#[test]
fn test_tabstop_order() {
    let body = parse_snippet_body("$2 $1 $0 $3");

    assert_eq!(body.tabstop_order(), vec![1, 2, 3, 0]); // 0 is always last
}

#[test]
fn test_escape_sequences() {
    let body = parse_snippet_body(r"Cost: \$100");

    assert_eq!(body.parts.len(), 1);
    assert!(matches!(&body.parts[0], SnippetPart::Text(t) if t == "Cost: $100"));
}

#[test]
fn test_registry_language_scope() {
    let mut registry = SnippetRegistry::new();

    registry.register(Snippet {
        prefix: "fn".to_string(),
        name: "Rust Function".to_string(),
        description: None,
        body: parse_snippet_body("fn $1() {}"),
        scope: Some(LanguageId::Rust),
        source: SnippetSource::Builtin,
    });

    registry.register(Snippet {
        prefix: "fn".to_string(),
        name: "JS Function".to_string(),
        description: None,
        body: parse_snippet_body("function $1() {}"),
        scope: Some(LanguageId::JavaScript),
        source: SnippetSource::Builtin,
    });

    // Rust file should get Rust snippet
    let rust_fn = registry.find("fn", LanguageId::Rust).unwrap();
    assert!(rust_fn.body.parts[0].to_string().contains("fn "));

    // JS file should get JS snippet
    let js_fn = registry.find("fn", LanguageId::JavaScript).unwrap();
    assert!(js_fn.name.contains("JS"));
}

#[test]
fn test_session_navigation() {
    let session = create_test_session_with_tabstops(vec![1, 2, 3, 0]);

    assert_eq!(session.current().unwrap().index, 1);

    session.next();
    assert_eq!(session.current().unwrap().index, 2);

    session.next();
    assert_eq!(session.current().unwrap().index, 3);

    session.next();
    assert_eq!(session.current().unwrap().index, 0);
    assert!(session.is_at_final());
}

#[test]
fn test_trigger_word_detection() {
    let doc = Document::with_text("  fn");
    let trigger = find_trigger_word(&doc, 0, 4);

    assert_eq!(trigger, Some("fn".to_string()));
}

#[test]
fn test_trigger_word_with_prefix() {
    let doc = Document::with_text("let x = testfn");
    let trigger = find_trigger_word(&doc, 0, 14);

    assert_eq!(trigger, Some("testfn".to_string()));
}
```

### Integration Tests

```rust
// tests/snippet_integration.rs

#[test]
fn test_expand_snippet_replaces_trigger() {
    let mut model = test_model_with_rust("fn");
    position_cursor(&mut model, 0, 2);

    update(&mut model, Msg::Snippet(SnippetMsg::ExpandOrTab));

    let content = get_document_content(&model);
    assert!(content.starts_with("fn "));
    assert!(content.contains("()"));
}

#[test]
fn test_tab_advances_tabstop() {
    let mut model = test_model_with_expanded_snippet();

    // Type at first tabstop
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('m')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('i')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('n')));

    // Advance to next tabstop
    update(&mut model, Msg::Snippet(SnippetMsg::NextTabstop));

    // Verify we're at second tabstop
    let editor = model.focused_editor().unwrap();
    assert!(editor.in_snippet_mode());
    let session = editor.snippet_session.as_ref().unwrap();
    assert_eq!(session.current().unwrap().index, 2);
}

#[test]
fn test_mirrors_update_together() {
    let mut model = test_model_with_mirror_snippet(); // "let ${1:x} = $1;"

    // Type at first tabstop (which has a mirror)
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('y')));

    let content = get_document_content(&model);
    // Both occurrences should be "y"
    assert!(content.contains("let y = y;"));
}

#[test]
fn test_escape_cancels_snippet() {
    let mut model = test_model_with_expanded_snippet();

    assert!(model.focused_editor().unwrap().in_snippet_mode());

    update(&mut model, Msg::Snippet(SnippetMsg::Cancel));

    assert!(!model.focused_editor().unwrap().in_snippet_mode());
}

#[test]
fn test_final_tabstop_exits_mode() {
    let mut model = test_model_with_simple_snippet(); // Only $1 and $0

    // Navigate to end
    update(&mut model, Msg::Snippet(SnippetMsg::NextTabstop)); // to $0
    update(&mut model, Msg::Snippet(SnippetMsg::NextTabstop)); // should exit

    assert!(!model.focused_editor().unwrap().in_snippet_mode());
}

#[test]
fn test_multicursor_expansion() {
    let mut model = test_model_with_multicursor("fn\nfn\nfn");

    // Position cursors after each "fn"
    // ... setup ...

    update(&mut model, Msg::Snippet(SnippetMsg::ExpandOrTab));

    let content = get_document_content(&model);
    // Should have 3 expanded snippets
    assert_eq!(content.matches("fn ").count(), 3);
}
```

---

## Dependencies

```toml
# Cargo.toml additions

[dependencies]
# No new dependencies required - uses existing serde_yaml for config
```

The snippet system reuses existing infrastructure:
- `serde_yaml` for parsing snippet definitions
- Existing document editing for text manipulation
- Existing keymap system for commands

---

## Built-in Snippets

### Rust Snippets

```yaml
# Embedded in src/snippets/builtin.rs

- prefix: fn
  name: Function
  body: |
    fn ${1:name}(${2:args}) {
        $0
    }

- prefix: pfn
  name: Public Function
  body: |
    pub fn ${1:name}(${2:args}) {
        $0
    }

- prefix: afn
  name: Async Function
  body: |
    async fn ${1:name}(${2:args}) {
        $0
    }

- prefix: test
  name: Test Function
  body: |
    #[test]
    fn ${1:test_name}() {
        $0
    }

- prefix: struct
  name: Struct
  body: |
    struct ${1:Name} {
        $0
    }

- prefix: enum
  name: Enum
  body: |
    enum ${1:Name} {
        $0
    }

- prefix: impl
  name: Impl Block
  body: |
    impl ${1:Type} {
        $0
    }

- prefix: match
  name: Match Expression
  body: |
    match ${1:expr} {
        ${2:pattern} => $0,
    }

- prefix: if
  name: If Statement
  body: |
    if ${1:condition} {
        $0
    }

- prefix: ifl
  name: If Let
  body: |
    if let ${1:pattern} = ${2:expr} {
        $0
    }

- prefix: for
  name: For Loop
  body: |
    for ${1:item} in ${2:iter} {
        $0
    }

- prefix: while
  name: While Loop
  body: |
    while ${1:condition} {
        $0
    }

- prefix: loop
  name: Loop
  body: |
    loop {
        $0
    }

- prefix: mod
  name: Module
  body: |
    mod ${1:name} {
        $0
    }

- prefix: use
  name: Use Statement
  body: use ${1:crate}::${2:module};$0

- prefix: derive
  name: Derive Attribute
  body: "#[derive(${1:Debug, Clone})]$0"

- prefix: cfg
  name: Cfg Attribute
  body: "#[cfg(${1:feature = \"name\"})]$0"
```

### JavaScript Snippets

```yaml
- prefix: fn
  name: Function
  body: |
    function ${1:name}(${2:args}) {
        $0
    }

- prefix: afn
  name: Arrow Function
  body: "const ${1:name} = (${2:args}) => {
        $0
    };"

- prefix: class
  name: Class
  body: |
    class ${1:Name} {
        constructor(${2:args}) {
            $0
        }
    }

- prefix: if
  name: If Statement
  body: |
    if (${1:condition}) {
        $0
    }

- prefix: for
  name: For Loop
  body: |
    for (let ${1:i} = 0; $1 < ${2:length}; $1++) {
        $0
    }

- prefix: fore
  name: For Each
  body: |
    ${1:array}.forEach((${2:item}) => {
        $0
    });

- prefix: map
  name: Map
  body: "${1:array}.map((${2:item}) => $0)"

- prefix: filter
  name: Filter
  body: "${1:array}.filter((${2:item}) => $0)"

- prefix: try
  name: Try Catch
  body: |
    try {
        $0
    } catch (${1:error}) {
        console.error($1);
    }

- prefix: log
  name: Console Log
  body: "console.log(${1:value});$0"
```

### Global Snippets

```yaml
- prefix: todo
  name: TODO Comment
  body: "// TODO: $0"

- prefix: fixme
  name: FIXME Comment
  body: "// FIXME: $0"

- prefix: note
  name: NOTE Comment
  body: "// NOTE: $0"

- prefix: hack
  name: HACK Comment
  body: "// HACK: $0"

- prefix: date
  name: Current Date
  body: "${1:2025-01-01}$0"
```

---

## Future Enhancements

### Phase 2: Variables

Support VS Code-style variables:

- `$TM_FILENAME` - Current file name
- `$TM_FILENAME_BASE` - File name without extension
- `$TM_DIRECTORY` - Current directory
- `$CURRENT_YEAR`, `$CURRENT_MONTH`, `$CURRENT_DATE`
- `$CLIPBOARD` - Clipboard contents
- `$UUID` - Random UUID

### Phase 3: Transformations

Support regex transformations on placeholders:

```
${1/(.*)/${1:/upcase}/}  # Uppercase
${1/(.*)/${1:/capitalize}/}  # Capitalize
```

### Phase 4: Choice Placeholders

Support dropdown choices:

```
${1|one,two,three|}
```

### Phase 5: Nested Snippets

Allow snippets within snippets for composition.

---

## References

- [VS Code Snippet Syntax](https://code.visualstudio.com/docs/editor/userdefinedsnippets#_snippet-syntax)
- [TextMate Snippets](https://macromates.com/manual/en/snippets)
- [Feature: Keymapping](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) - Context conditions
- [Feature: Multi-Cursor](../archived/SELECTION_MULTICURSOR.md) - Multi-cursor handling
