# Replace Enhancements (Advanced)

Preserve case, selection scope, and regex capture group references.

> **Status:** Planned
> **Priority:** P1
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 2 - Search & Editing
> **Prerequisite:** Basic replace implemented (see `docs/archived/replace-basic.md`)

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

### Already Implemented (v0.3.11)

- Replace single occurrence at cursor
- Replace All with single undo operation
- Case sensitivity (from find bar)

### Remaining Goals

1. **Preserve Case** - Match capitalization pattern of original text
2. **Selection scope** - Replace only within selection
3. **Regex capture groups** - Support `$1`, `$2`, etc. in replacement
4. **Preview replacement** - Show what the replacement will look like
5. **Replace confirmation** - Optional confirmation for large replacements

### Non-Goals

- Multi-file replace (workspace-wide search/replace)
- Replace with clipboard contents (can add as option)
- Interactive replace review (VS Code style)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Replace Enhancement System                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Find/Replace Bar                               │ │
│  │                                                                         │ │
│  │  Query:   [ function                               ] (3 of 42)         │ │
│  │  Replace: [ method                                 ]                    │ │
│  │                                                                         │ │
│  │  [Aa] Case  [W] Whole  [.*] Regex  [AB] Preserve  [=] Selection        │ │
│  │                                                                         │ │
│  │  [ Replace ] [ Replace All (42) ] [ Skip ]                             │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                 │                                            │
│                                 ▼                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                       Replacement Engine                                │ │
│  │                                                                         │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  ReplaceOperation                                               │   │ │
│  │  │  - matches: Vec<Match>                                          │   │ │
│  │  │  - replacement_template: String                                 │   │ │
│  │  │  - preserve_case: bool                                          │   │ │
│  │  │  - is_regex: bool                                               │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  │                          │                                              │ │
│  │                          ▼                                              │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  For each match:                                                │   │ │
│  │  │  1. Extract matched text                                        │   │ │
│  │  │  2. Resolve capture groups ($1, $2, ...)                       │   │ │
│  │  │  3. Apply case preservation if enabled                          │   │ │
│  │  │  4. Generate final replacement string                           │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  │                          │                                              │ │
│  │                          ▼                                              │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  Apply replacements (reverse order to preserve offsets)         │   │ │
│  │  │  - Single undo operation for Replace All                        │   │ │
│  │  │  - Update cursor positions                                      │   │ │
│  │  │  - Refresh search results                                       │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Case Preservation Logic

```
Original Text      Replacement     Preserve Case Result
─────────────────────────────────────────────────────────
"Function"     →   "method"    →   "Method"     (Title case)
"FUNCTION"     →   "method"    →   "METHOD"     (All caps)
"function"     →   "method"    →   "method"     (Lowercase)
"functionName" →   "method"    →   "methodName" (Camel case)
"FunctionName" →   "method"    →   "MethodName" (Pascal case)
```

---

## Data Structures

### Replacement Template

```rust
// src/replace.rs

use regex::Regex;

/// A compiled replacement template
#[derive(Debug, Clone)]
pub struct ReplacementTemplate {
    /// Original replacement string
    pub template: String,
    /// Parsed segments for substitution
    segments: Vec<ReplaceSegment>,
    /// Whether this uses capture groups
    pub uses_captures: bool,
}

/// A segment of the replacement string
#[derive(Debug, Clone)]
enum ReplaceSegment {
    /// Literal text
    Literal(String),
    /// Capture group reference ($0, $1, $2, ... or ${name})
    CaptureGroup(CaptureRef),
}

/// Reference to a capture group
#[derive(Debug, Clone)]
enum CaptureRef {
    /// Numbered capture group ($0, $1, etc.)
    Numbered(usize),
    /// Named capture group (${name})
    Named(String),
    /// Entire match ($0 or $&)
    EntireMatch,
}

impl ReplacementTemplate {
    /// Parse a replacement template string
    pub fn parse(template: &str) -> Self {
        let mut segments = Vec::new();
        let mut current_literal = String::new();
        let mut chars = template.chars().peekable();
        let mut uses_captures = false;

        while let Some(c) = chars.next() {
            if c == '$' {
                // Flush current literal
                if !current_literal.is_empty() {
                    segments.push(ReplaceSegment::Literal(current_literal.clone()));
                    current_literal.clear();
                }

                match chars.peek() {
                    Some('$') => {
                        // Escaped $
                        chars.next();
                        current_literal.push('$');
                    }
                    Some('&') | Some('0') => {
                        chars.next();
                        segments.push(ReplaceSegment::CaptureGroup(CaptureRef::EntireMatch));
                        uses_captures = true;
                    }
                    Some(c) if c.is_ascii_digit() => {
                        let mut num = String::new();
                        while let Some(&c) = chars.peek() {
                            if c.is_ascii_digit() {
                                num.push(c);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        let n: usize = num.parse().unwrap_or(0);
                        segments.push(ReplaceSegment::CaptureGroup(CaptureRef::Numbered(n)));
                        uses_captures = true;
                    }
                    Some('{') => {
                        chars.next(); // consume {
                        let mut name = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '}' {
                                chars.next();
                                break;
                            }
                            name.push(c);
                            chars.next();
                        }
                        segments.push(ReplaceSegment::CaptureGroup(CaptureRef::Named(name)));
                        uses_captures = true;
                    }
                    _ => {
                        current_literal.push('$');
                    }
                }
            } else {
                current_literal.push(c);
            }
        }

        // Flush remaining literal
        if !current_literal.is_empty() {
            segments.push(ReplaceSegment::Literal(current_literal));
        }

        Self {
            template: template.to_string(),
            segments,
            uses_captures,
        }
    }

    /// Apply template to a regex match to produce replacement string
    pub fn apply(&self, regex: &Regex, text: &str, match_range: (usize, usize)) -> String {
        let matched_text = &text[match_range.0..match_range.1];

        if !self.uses_captures {
            // Simple case: no capture groups
            return self.template.clone();
        }

        // Find the match with captures
        let captures = regex.captures(matched_text);

        let mut result = String::new();
        for segment in &self.segments {
            match segment {
                ReplaceSegment::Literal(s) => result.push_str(s),
                ReplaceSegment::CaptureGroup(cap_ref) => {
                    let capture_text = match cap_ref {
                        CaptureRef::EntireMatch => Some(matched_text),
                        CaptureRef::Numbered(n) => {
                            captures.as_ref().and_then(|c| c.get(*n).map(|m| m.as_str()))
                        }
                        CaptureRef::Named(name) => {
                            captures.as_ref().and_then(|c| c.name(name).map(|m| m.as_str()))
                        }
                    };
                    if let Some(text) = capture_text {
                        result.push_str(text);
                    }
                }
            }
        }

        result
    }
}
```

### Case Preservation

```rust
// src/replace.rs

/// Determines the case pattern of a string
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasePattern {
    /// All lowercase: "function"
    Lower,
    /// All uppercase: "FUNCTION"
    Upper,
    /// Title case (first letter uppercase): "Function"
    Title,
    /// Camel case (starts lower, has upper): "functionName"
    Camel,
    /// Pascal case (starts upper, has lower): "FunctionName"
    Pascal,
    /// Mixed or unknown pattern
    Mixed,
}

impl CasePattern {
    /// Detect the case pattern of a string
    pub fn detect(s: &str) -> Self {
        if s.is_empty() {
            return CasePattern::Mixed;
        }

        let chars: Vec<char> = s.chars().collect();
        let first = chars[0];
        let has_upper = chars.iter().any(|c| c.is_uppercase());
        let has_lower = chars.iter().any(|c| c.is_lowercase());

        if !has_upper && has_lower {
            return CasePattern::Lower;
        }
        if has_upper && !has_lower {
            return CasePattern::Upper;
        }

        // Mixed case
        if first.is_uppercase() {
            // Check for Pascal vs Title
            if chars.len() > 1 && chars[1..].iter().any(|c| c.is_uppercase()) {
                CasePattern::Pascal
            } else {
                CasePattern::Title
            }
        } else if first.is_lowercase() && chars[1..].iter().any(|c| c.is_uppercase()) {
            CasePattern::Camel
        } else {
            CasePattern::Mixed
        }
    }

    /// Apply this case pattern to a string
    pub fn apply(&self, s: &str) -> String {
        match self {
            CasePattern::Lower => s.to_lowercase(),
            CasePattern::Upper => s.to_uppercase(),
            CasePattern::Title => {
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().chain(chars.flat_map(|c| c.to_lowercase())).collect()
                    }
                }
            }
            CasePattern::Pascal => {
                // Uppercase first, preserve rest
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            }
            CasePattern::Camel => {
                // Lowercase first, preserve rest
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_lowercase().chain(chars).collect(),
                }
            }
            CasePattern::Mixed => s.to_string(),
        }
    }
}

/// Apply case preservation to a replacement
pub fn preserve_case(original: &str, replacement: &str) -> String {
    let pattern = CasePattern::detect(original);
    pattern.apply(replacement)
}
```

### Replace Operation

```rust
// src/replace.rs

use crate::search::{Match, SearchQuery, SearchResults};
use crate::model::Document;

/// A pending replace operation
#[derive(Debug, Clone)]
pub struct ReplaceOperation {
    /// Matches to replace
    pub matches: Vec<Match>,
    /// Replacement template
    pub template: ReplacementTemplate,
    /// Whether to preserve case
    pub preserve_case: bool,
    /// Whether the search used regex
    pub is_regex: bool,
    /// The compiled search pattern (for capture groups)
    pub search_regex: Option<Regex>,
}

impl ReplaceOperation {
    /// Create a replace operation from find state
    pub fn from_find_state(
        results: &SearchResults,
        replacement: &str,
        preserve_case: bool,
    ) -> Self {
        Self {
            matches: results.matches.clone(),
            template: ReplacementTemplate::parse(replacement),
            preserve_case,
            is_regex: results.query.is_regex,
            search_regex: results.query.compiled.clone(),
        }
    }

    /// Get the replacement text for a specific match
    pub fn replacement_for(&self, doc: &Document, match_info: &Match) -> String {
        let matched_text = doc.text_range(match_info.start, match_info.end);

        let replacement = if let (true, Some(regex)) = (self.is_regex, &self.search_regex) {
            self.template.apply(regex, &matched_text, (0, matched_text.len()))
        } else {
            self.template.template.clone()
        };

        if self.preserve_case {
            preserve_case(&matched_text, &replacement)
        } else {
            replacement
        }
    }

    /// Preview all replacements (for confirmation dialog)
    pub fn preview(&self, doc: &Document) -> Vec<ReplacementPreview> {
        self.matches
            .iter()
            .map(|m| {
                let original = doc.text_range(m.start, m.end);
                let replacement = self.replacement_for(doc, m);
                ReplacementPreview {
                    line: m.line,
                    original,
                    replacement,
                }
            })
            .collect()
    }

    /// Execute all replacements on the document
    ///
    /// Returns the text edits to apply (in reverse order for correct offsets)
    pub fn execute(&self, doc: &Document) -> Vec<TextEdit> {
        // Process in reverse order to maintain correct offsets
        self.matches
            .iter()
            .rev()
            .map(|m| {
                let replacement = self.replacement_for(doc, m);
                TextEdit {
                    start: m.start,
                    end: m.end,
                    new_text: replacement,
                }
            })
            .collect()
    }

    /// Get count of matches
    pub fn count(&self) -> usize {
        self.matches.len()
    }
}

/// Preview of a single replacement
#[derive(Debug, Clone)]
pub struct ReplacementPreview {
    /// Line number of the match
    pub line: usize,
    /// Original matched text
    pub original: String,
    /// What it will be replaced with
    pub replacement: String,
}

/// A text edit to apply
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// Start byte offset
    pub start: usize,
    /// End byte offset
    pub end: usize,
    /// New text to insert
    pub new_text: String,
}
```

### Enhanced FindReplaceState

```rust
// Add to FindReplaceState in src/model/ui.rs

impl FindReplaceState {
    // ... existing methods ...

    /// Preserve case during replacement
    pub preserve_case: bool,

    /// Toggle preserve case option
    pub fn toggle_preserve_case(&mut self) {
        self.preserve_case = !self.preserve_case;
    }

    /// Create replace operation for current match only
    pub fn replace_current(&self, doc: &Document) -> Option<ReplaceOperation> {
        let current = self.results.current_match()?;
        Some(ReplaceOperation {
            matches: vec![*current],
            template: ReplacementTemplate::parse(&self.replacement()),
            preserve_case: self.preserve_case,
            is_regex: self.use_regex,
            search_regex: self.results.query.compiled.clone(),
        })
    }

    /// Create replace operation for all matches
    pub fn replace_all(&self) -> ReplaceOperation {
        ReplaceOperation::from_find_state(
            &self.results,
            &self.replacement(),
            self.preserve_case,
        )
    }

    /// Check if Replace All should require confirmation
    pub fn should_confirm_replace_all(&self, threshold: usize) -> bool {
        self.results.count() > threshold
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open Find/Replace | Cmd+H | Ctrl+H | Open with replace field |
| Replace | Cmd+Shift+1 | Ctrl+Shift+1 | Replace current match |
| Replace & Find Next | Enter (in replace field) | Enter | Replace and move to next |
| Replace All | Cmd+Shift+Enter | Ctrl+Shift+Enter | Replace all matches |
| Toggle Preserve Case | Option+Cmd+P | Alt+P | Toggle case preservation |
| Skip (Find Next) | Cmd+G | F3 | Skip to next match |

---

## Implementation Plan

### Phase 1: Replacement Template

**Files:** `src/replace.rs`

- [ ] Create `ReplacementTemplate` struct
- [ ] Implement template parsing with capture group detection
- [ ] Support `$1`, `$2`, `$0`, `$&` syntax
- [ ] Support `${name}` for named groups
- [ ] Add unit tests for template parsing

**Test:** `ReplacementTemplate::parse("$1-$2")` correctly identifies capture groups.

### Phase 2: Case Preservation

**Files:** `src/replace.rs`

- [ ] Create `CasePattern` enum
- [ ] Implement `detect()` for case pattern recognition
- [ ] Implement `apply()` for case transformation
- [ ] Add `preserve_case()` helper function
- [ ] Add unit tests for all case patterns

**Test:** `preserve_case("HELLO", "world")` returns `"WORLD"`.

### Phase 3: Replace Operation

**Files:** `src/replace.rs`

- [ ] Create `ReplaceOperation` struct
- [ ] Implement `replacement_for()` with case preservation
- [ ] Implement `preview()` for confirmation
- [ ] Implement `execute()` returning text edits
- [ ] Process in reverse order for correct offsets

**Test:** Replace all generates correct text edits.

### Phase 4: UI Integration

**Files:** `src/model/ui.rs`

- [ ] Add `preserve_case` field to `FindReplaceState`
- [ ] Add toggle method
- [ ] Add `replace_current()` method
- [ ] Add `replace_all()` method
- [ ] Add confirmation threshold check

**Test:** Preserve case toggle updates state.

### Phase 5: Message Handling

**Files:** `src/messages.rs`, `src/update/modal.rs`

- [ ] Add `ModalMsg::Replace` message
- [ ] Add `ModalMsg::ReplaceAll` message
- [ ] Add `ModalMsg::TogglePreserveCase` message
- [ ] Handle replace with single undo group
- [ ] Refresh search results after replace

**Test:** Replace All can be undone in one step.

### Phase 6: Rendering

**Files:** `src/view/modal.rs`

- [ ] Add Replace button
- [ ] Add Replace All button with count
- [ ] Add preserve case toggle (AB icon)
- [ ] Show replacement preview on hover/focus
- [ ] Show confirmation dialog for large replacements

**Test:** Replace All button shows match count.

### Phase 7: Selection Scope

**Files:** `src/replace.rs`, `src/update/modal.rs`

- [ ] Filter matches to selection range
- [ ] Update selection after replacement
- [ ] Handle selection boundary edge cases
- [ ] Update match count for selection scope

**Test:** Replace All only affects selection when selection mode active.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_pattern_detection() {
        assert_eq!(CasePattern::detect("hello"), CasePattern::Lower);
        assert_eq!(CasePattern::detect("HELLO"), CasePattern::Upper);
        assert_eq!(CasePattern::detect("Hello"), CasePattern::Title);
        assert_eq!(CasePattern::detect("helloWorld"), CasePattern::Camel);
        assert_eq!(CasePattern::detect("HelloWorld"), CasePattern::Pascal);
    }

    #[test]
    fn test_case_pattern_apply() {
        assert_eq!(CasePattern::Lower.apply("WORLD"), "world");
        assert_eq!(CasePattern::Upper.apply("world"), "WORLD");
        assert_eq!(CasePattern::Title.apply("world"), "World");
        assert_eq!(CasePattern::Pascal.apply("test"), "Test");
        assert_eq!(CasePattern::Camel.apply("Test"), "test");
    }

    #[test]
    fn test_preserve_case() {
        assert_eq!(preserve_case("Function", "method"), "Method");
        assert_eq!(preserve_case("FUNCTION", "method"), "METHOD");
        assert_eq!(preserve_case("function", "method"), "method");
        assert_eq!(preserve_case("functionName", "methodCall"), "methodCall");
    }

    #[test]
    fn test_template_literal() {
        let template = ReplacementTemplate::parse("replacement");
        assert!(!template.uses_captures);
    }

    #[test]
    fn test_template_captures() {
        let template = ReplacementTemplate::parse("$1-$2");
        assert!(template.uses_captures);
    }

    #[test]
    fn test_template_escaped_dollar() {
        let template = ReplacementTemplate::parse("$$100");
        assert!(!template.uses_captures);
        // Should produce "$100" as output
    }

    #[test]
    fn test_template_named_group() {
        let template = ReplacementTemplate::parse("${name}");
        assert!(template.uses_captures);
    }

    #[test]
    fn test_replace_operation_reverse_order() {
        // Verify edits are in reverse order for correct offset handling
        let op = ReplaceOperation {
            matches: vec![
                Match { start: 0, end: 5, line: 0 },
                Match { start: 10, end: 15, line: 0 },
            ],
            template: ReplacementTemplate::parse("X"),
            preserve_case: false,
            is_regex: false,
            search_regex: None,
        };

        let edits = op.execute(&doc);
        assert!(edits[0].start > edits[1].start); // Reverse order
    }
}
```

### Integration Tests

```rust
// tests/replace_tests.rs

#[test]
fn test_replace_single() {
    // Find "foo"
    // Replace with "bar"
    // Verify first occurrence replaced
    // Verify cursor moved to next match
}

#[test]
fn test_replace_all() {
    // Find "foo" (3 occurrences)
    // Replace All with "bar"
    // Verify all replaced
    // Undo
    // Verify all restored
}

#[test]
fn test_replace_preserve_case() {
    // Document: "Hello hello HELLO"
    // Find "hello" (case insensitive)
    // Replace with "world" (preserve case)
    // Verify: "World world WORLD"
}

#[test]
fn test_replace_regex_captures() {
    // Document: "foo(bar)"
    // Find: "(\w+)\((\w+)\)" (regex)
    // Replace: "$2($1)"
    // Verify: "bar(foo)"
}

#[test]
fn test_replace_in_selection() {
    // Select portion of document
    // Find "the"
    // Replace All with "a"
    // Verify only selection affected
}
```

---

## References

- **Find Enhancements:** F-050 for search infrastructure
- **Existing code:** `src/model/ui.rs` - `FindReplaceState`
- **Undo system:** `src/model/document.rs` - `EditOperation`
- **VS Code:** Replace with preserve case option
- **Sublime Text:** Regex capture group replacement
- **IntelliJ:** Structural replace with case options
