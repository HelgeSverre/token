//! Document model - represents the text buffer and file state

use ropey::Rope;
use std::path::PathBuf;

use super::editor::{Cursor, EditorState, Selection};
use super::editor_area::{DocumentId, EditorId};
use crate::syntax::{LanguageId, SyntaxHighlights, SyntaxTreeSnapshot};

/// Represents an edit operation for undo/redo functionality
#[derive(Debug, Clone)]
pub enum EditOperation {
    Insert {
        position: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    Delete {
        position: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    /// Replace operation - used when typing over a selection to make undo atomic
    Replace {
        position: usize,
        deleted_text: String,
        inserted_text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    /// Batch operation - groups multiple edits for atomic multi-cursor undo/redo
    Batch {
        /// Individual operations (applied in order for redo, reverse order for undo)
        operations: Vec<EditOperation>,
        /// Lossless selection state of every existing pane before the batch.
        editors_before: Vec<EditorEditState>,
        /// Corresponding state after caret placement and deduplication.
        editors_after: Vec<EditorEditState>,
    },
}

/// Opaque undo snapshot tied to one editor, independent of current focus.
/// Only selection state is retained, not layout, caches or the document buffer.
#[derive(Debug, Clone)]
pub struct EditorEditState {
    pub(crate) editor_id: EditorId,
    cursors: Vec<Cursor>,
    selections: Vec<Selection>,
    active_cursor_index: usize,
}

impl EditorEditState {
    pub(crate) fn capture(editor_id: EditorId, editor: &EditorState) -> Self {
        Self {
            editor_id,
            cursors: editor.cursors.clone(),
            selections: editor.selections.clone(),
            active_cursor_index: editor.active_cursor_index,
        }
    }

    pub(crate) fn restore(&self, editor: &mut EditorState) {
        editor.cursors.clone_from(&self.cursors);
        editor.selections.clone_from(&self.selections);
        editor.active_cursor_index = self.active_cursor_index;
        editor.occurrence_state = None;
        editor.clear_selection_history();
    }
}

/// Document state - the text buffer and associated file metadata
#[derive(Debug, Clone)]
pub struct Document {
    /// Unique identifier (set when added to EditorArea)
    pub id: Option<DocumentId>,

    /// The text buffer
    pub buffer: Rope,
    /// Path to the file on disk (None for new/unsaved files)
    pub file_path: Option<PathBuf>,
    file_identity: Option<crate::util::FileIdentity>,
    /// Display name for untitled documents (e.g., "Untitled", "Untitled-2")
    pub untitled_name: Option<String>,
    /// Whether the buffer has unsaved changes
    pub is_modified: bool,
    /// Undo stack
    pub undo_stack: Vec<EditOperation>,
    /// Redo stack
    pub redo_stack: Vec<EditOperation>,
    /// Cheap immutable snapshot of the bytes last successfully written. History
    /// depth alone is not an identity: undo followed by a new branch can reuse it.
    saved_buffer: Option<Rope>,
    /// Path to which the saved snapshot belongs. Changing a document's path
    /// cannot transfer an unrelated file's overwrite permission.
    saved_path: Option<PathBuf>,
    pub(crate) file_io: super::FileIoState,
    pub external_change: Option<super::ExternalFileChange>,

    // === Syntax Highlighting ===
    /// Detected language for syntax highlighting
    pub language: LanguageId,
    /// Session-only "Set Language..." override: when set, `language` is
    /// kept across external reload and Save As instead of re-detected
    /// from the path. Never persisted.
    pub language_pinned: bool,
    /// Current syntax highlights (updated asynchronously)
    pub syntax_highlights: Option<SyntaxHighlights>,
    /// Parsed base-language tree for syntax-aware editor operations.
    pub syntax_tree: Option<SyntaxTreeSnapshot>,
    /// Parsed outline data (functions, structs, etc.)
    pub outline: Option<crate::outline::OutlineData>,
    /// Document revision counter (incremented on each edit)
    /// Used for staleness checking in async parsing
    pub revision: u64,

    // === LSP ===
    /// Diagnostics projection for this document (lsp-integration.md Phase
    /// 2) — refreshed from `LspManager`'s authoritative
    /// `HashMap<Uri, Vec<Diagnostic>>` store on publish and on open.
    /// Positions are LSP (UTF-16) coordinates; converted to editor
    /// char-columns lazily at render/collection time via
    /// `lsp::position::lsp_to_position`, which also clamps into the
    /// current buffer (see `model::collect_line_marks`).
    pub diagnostics: Vec<lsp_types::Diagnostic>,
}

impl Document {
    /// Create a new empty document
    pub fn new() -> Self {
        Self {
            id: None,
            buffer: Rope::from(""),
            file_path: None,
            file_identity: None,
            untitled_name: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            saved_buffer: Some(Rope::new()),
            saved_path: None,
            file_io: Default::default(),
            external_change: None,
            language: LanguageId::PlainText,
            language_pinned: false,
            syntax_highlights: None,
            syntax_tree: None,
            outline: None,
            revision: 0,
            diagnostics: Vec::new(),
        }
    }

    /// Create a document with initial text
    pub fn with_text(text: &str) -> Self {
        let buffer = Rope::from(text);
        Self {
            saved_buffer: Some(buffer.clone()),
            buffer,
            ..Self::new()
        }
    }

    /// Load a document from a file path
    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(&path)?;
        Ok(Self::from_loaded_text(
            &content,
            crate::util::FileIdentity::resolve(path),
        ))
    }

    /// Install worker-read text with its resolved identity, without further I/O.
    pub fn from_loaded_text(content: &str, identity: crate::util::FileIdentity) -> Self {
        Self {
            file_path: Some(identity.source().to_path_buf()),
            saved_path: Some(identity.source().to_path_buf()),
            language: LanguageId::from_path(identity.source()),
            file_identity: Some(identity),
            ..Self::with_text(content)
        }
    }

    /// An identity is usable only while its original path still belongs to us.
    /// Direct path changes cannot accidentally retain aliases of the old file.
    pub fn file_identity(&self) -> Option<&crate::util::FileIdentity> {
        self.file_identity
            .as_ref()
            .filter(|identity| self.file_path.as_deref() == Some(identity.source()))
    }

    pub fn set_file_identity(&mut self, identity: Option<crate::util::FileIdentity>) {
        self.file_identity =
            identity.filter(|identity| self.file_path.as_deref() == Some(identity.source()));
    }

    pub fn matches_file_path(&self, path: &std::path::Path) -> bool {
        self.file_path.as_deref() == Some(path)
            || self
                .file_identity()
                .is_some_and(|identity| identity.matches_path(path))
    }

    /// Create a new empty document with a target file path
    ///
    /// Used when the user specifies a non-existent file path on the command line.
    /// The file will be created when the user saves.
    pub fn new_with_path(path: PathBuf) -> Self {
        let language = LanguageId::from_path(&path);
        Self {
            file_path: Some(path),
            is_modified: true, // Mark as modified since file doesn't exist yet
            language,
            // No saved state exists yet (file doesn't exist on disk), so
            // Undo/Redo can't clear the dirty flag until an actual save happens.
            saved_buffer: None,
            ..Self::new()
        }
    }

    pub(crate) fn begin_file_request(
        &mut self,
        kind: super::FileRequestKind,
    ) -> Option<super::FileRequest> {
        Some(super::FileRequest {
            document_id: self.id?,
            revision: self.revision,
            source_path: self.file_path.clone(),
            source_identity: self.file_identity().cloned(),
            external_reload: false,
            write_guard: super::FileWriteGuard {
                saved: self.saved_disk_content().cloned(),
                queued: self
                    .file_path
                    .as_deref()
                    .and_then(|path| self.file_io.previous_write(path)),
                save_as: false,
            },
            sequence: self.file_io.begin(kind),
        })
    }

    pub(crate) fn record_saved_buffer(&mut self, buffer: Rope) {
        self.saved_buffer = Some(buffer);
        self.saved_path = self.file_path.clone();
        self.external_change = None;
        self.refresh_modified();
    }

    pub(crate) fn saved_disk_content(&self) -> Option<&Rope> {
        self.saved_buffer
            .as_ref()
            .filter(|_| self.saved_path.is_some() && self.saved_path == self.file_path)
    }

    pub(crate) fn refresh_modified(&mut self) {
        self.is_modified = !self
            .saved_buffer
            .as_ref()
            .is_some_and(|saved| saved.is_instance(&self.buffer) || saved == &self.buffer);
    }

    /// Get the display name for this document.
    /// Returns the filename if saved, the untitled name if set, or "Untitled" as fallback.
    pub fn display_name(&self) -> String {
        if let Some(path) = &self.file_path {
            if let Some(name) = path.file_name() {
                return name.to_string_lossy().to_string();
            }
        }
        if let Some(name) = &self.untitled_name {
            return name.clone();
        }
        "Untitled".to_string()
    }

    /// Get the number of lines in the document
    pub fn line_count(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Get a line by index (allocates a String)
    ///
    /// For rendering hot paths, prefer `get_line_slice()` which can avoid allocation
    /// by iterating over the rope slice directly.
    pub fn get_line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            Some(line.to_string())
        } else {
            None
        }
    }

    /// Get a line as a RopeSlice for zero-allocation iteration
    ///
    /// Use this in rendering hot paths to avoid String allocation.
    /// The returned slice can be iterated with `.chars()` or converted
    /// to a contiguous slice with `Cow<str>` when needed.
    #[inline]
    pub fn get_line_slice(&self, line_idx: usize) -> Option<ropey::RopeSlice<'_>> {
        if line_idx < self.buffer.len_lines() {
            Some(self.buffer.line(line_idx))
        } else {
            None
        }
    }

    /// Get line content as Cow<str>, avoiding allocation when possible
    ///
    /// Returns Cow::Borrowed if the line is stored contiguously in a single chunk,
    /// otherwise returns Cow::Owned with the line as a String.
    /// Also trims the trailing newline for display purposes.
    #[inline]
    pub fn get_line_cow(&self, line_idx: usize) -> Option<std::borrow::Cow<'_, str>> {
        use std::borrow::Cow;

        if line_idx >= self.buffer.len_lines() {
            return None;
        }

        let line = self.buffer.line(line_idx);
        let len = line.len_chars();

        // Calculate trim length (remove trailing newline)
        let trim_len = if len > 0 && line.char(len - 1) == '\n' {
            if len > 1 && line.char(len - 2) == '\r' {
                2 // CRLF
            } else {
                1 // LF
            }
        } else {
            0
        };

        let trimmed = line.slice(..len - trim_len);

        // Try to get as a contiguous slice (zero allocation)
        if let Some(s) = trimmed.as_str() {
            Some(Cow::Borrowed(s))
        } else {
            // Falls back to allocation only when line spans multiple chunks
            Some(Cow::Owned(trimmed.to_string()))
        }
    }

    /// Get the length of a line (excluding the trailing line ending).
    ///
    /// Mirrors the CRLF-aware trim in `get_line_cow`: a trailing `\r\n`
    /// excludes 2 chars, a lone trailing `\n` excludes 1. Without this a
    /// CRLF file would overcount by 1 (the `\r`), diverging from what's
    /// actually rendered and from `view::helpers::trim_line_ending`.
    pub fn line_length(&self, line_idx: usize) -> usize {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            let len = line.len_chars();
            let trim_len = if len > 0 && line.char(len - 1) == '\n' {
                if len > 1 && line.char(len - 2) == '\r' {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            len - trim_len
        } else {
            0
        }
    }

    /// Convert a (line, column) position to a buffer offset
    /// Uses ropey's O(log n) line_to_char method instead of O(n) iteration
    pub fn cursor_to_offset(&self, line: usize, column: usize) -> usize {
        if line >= self.buffer.len_lines() {
            return self.buffer.len_chars();
        }
        let line_start = self.buffer.line_to_char(line);
        line_start + column.min(self.line_length(line))
    }

    /// Convert a buffer offset to (line, column) position
    /// Uses ropey's O(log n) char_to_line method instead of O(n) iteration
    pub fn offset_to_cursor(&self, offset: usize) -> (usize, usize) {
        let clamped = offset.min(self.buffer.len_chars());
        let line = self.buffer.char_to_line(clamped);
        let line_start = self.buffer.line_to_char(line);
        (line, clamped - line_start)
    }

    /// Get the column of the first non-whitespace character on a line
    pub fn first_non_whitespace_column(&self, line_idx: usize) -> usize {
        if line_idx >= self.buffer.len_lines() {
            return 0;
        }
        let line = self.buffer.line(line_idx);
        line.chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .count()
    }

    /// Get the column after the last non-whitespace character on a line
    pub fn last_non_whitespace_column(&self, line_idx: usize) -> usize {
        if line_idx >= self.buffer.len_lines() {
            return 0;
        }
        let line = self.buffer.line(line_idx);
        let line_str: String = line.chars().collect();
        let trimmed = line_str.trim_end_matches(|c: char| c.is_whitespace());
        trimmed.chars().count()
    }

    /// Push an edit operation onto the undo stack and clear redo stack
    pub fn push_edit(&mut self, op: EditOperation) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.is_modified = true;
        self.revision = self.revision.wrapping_add(1);
        // Keep existing syntax highlights until new ones arrive.
        // This prevents "flash of unstyled text" during the debounce window.
        // The revision check in ParseCompleted ensures only matching highlights are applied.
    }

    /// Get highlight tokens for a specific line
    pub fn get_line_highlights(&self, line: usize) -> &[crate::syntax::HighlightToken] {
        self.syntax_highlights
            .as_ref()
            .and_then(|h| h.get_line(line))
            .map(|lh| lh.tokens.as_slice())
            .unwrap_or(&[])
    }

    /// Find all occurrences of text in the document
    /// Returns Vec of (start_char_offset, end_char_offset) in character indices
    pub fn find_all_occurrences(&self, needle: &str) -> Vec<(usize, usize)> {
        self.find_all_occurrences_with_options(needle, true)
    }

    /// Find all occurrences with case sensitivity option
    /// Returns Vec of (start_char_offset, end_char_offset) in character indices
    pub fn find_all_occurrences_with_options(
        &self,
        needle: &str,
        case_sensitive: bool,
    ) -> Vec<(usize, usize)> {
        if needle.is_empty() {
            return Vec::new();
        }

        let haystack = self.buffer.to_string();
        if case_sensitive {
            find_case_sensitive(&haystack, needle)
        } else {
            find_case_insensitive(&haystack, needle)
        }
    }

    /// Find next occurrence after given offset (wraps back to start)
    pub fn find_next_occurrence(
        &self,
        needle: &str,
        after_offset: usize,
    ) -> Option<(usize, usize)> {
        self.find_next_occurrence_with_options(needle, after_offset, true)
    }

    /// Find next occurrence with case sensitivity option (wraps back to start)
    pub fn find_next_occurrence_with_options(
        &self,
        needle: &str,
        after_offset: usize,
        case_sensitive: bool,
    ) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }

        let occurrences = self.find_all_occurrences_with_options(needle, case_sensitive);

        // Find first occurrence after current position
        if let Some(&occ) = occurrences.iter().find(|(start, _)| *start > after_offset) {
            return Some(occ);
        }

        // Wrap around to first occurrence
        occurrences.first().copied()
    }

    /// Find previous occurrence before given offset (wraps to end)
    pub fn find_prev_occurrence_with_options(
        &self,
        needle: &str,
        before_offset: usize,
        case_sensitive: bool,
    ) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }

        let occurrences = self.find_all_occurrences_with_options(needle, case_sensitive);

        // Find last occurrence before current position
        if let Some(&occ) = occurrences
            .iter()
            .rev()
            .find(|(start, _)| *start < before_offset)
        {
            return Some(occ);
        }

        // Wrap around to last occurrence
        occurrences.last().copied()
    }

    /// Run a `search::SearchQuery` (regex/whole-word/case options) against
    /// the whole document, returning char-offset matches — the richer
    /// engine used by find/replace navigation and match-highlighting
    /// decorations. See `find-enhancements.md`.
    pub fn search_matches(&self, query: &crate::search::SearchQuery) -> Vec<crate::search::Match> {
        query.find_all(&self.buffer.to_string())
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

fn find_case_sensitive(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    let needle_chars = needle.chars().count();
    let mut results = Vec::new();
    let mut search_byte = 0;
    let mut search_char = 0;

    while let Some(relative_byte) = haystack[search_byte..].find(needle) {
        let match_byte = search_byte + relative_byte;
        let match_char = search_char + haystack[search_byte..match_byte].chars().count();
        results.push((match_char, match_char + needle_chars));

        let advance = haystack[match_byte..]
            .chars()
            .next()
            .map_or(1, char::len_utf8);
        search_byte = match_byte + advance;
        search_char = match_char + 1;
    }

    results
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    if haystack.is_ascii() && needle.is_ascii() {
        return find_ascii_case_insensitive(haystack, needle);
    }

    let mut folded = String::with_capacity(haystack.len());
    let mut boundaries = Vec::with_capacity(haystack.chars().count() + 1);
    boundaries.push((0, 0));

    for (original_char, ch) in haystack.chars().enumerate() {
        for folded_char in ch.to_lowercase() {
            if boundaries.last().map(|entry| entry.0) != Some(folded.len()) {
                boundaries.push((folded.len(), original_char));
            }
            folded.push(folded_char);
        }
        let boundary = (folded.len(), original_char + 1);
        if let Some(last) = boundaries.last_mut().filter(|last| last.0 == boundary.0) {
            *last = boundary;
        } else {
            boundaries.push(boundary);
        }
    }

    let folded_needle = needle.to_lowercase();
    let mut results = Vec::new();
    let mut search_byte = 0;
    while let Some(relative_byte) = folded[search_byte..].find(&folded_needle) {
        let start_byte = search_byte + relative_byte;
        let end_byte = start_byte + folded_needle.len();
        if let (Ok(start), Ok(end)) = (
            boundaries.binary_search_by_key(&start_byte, |entry| entry.0),
            boundaries.binary_search_by_key(&end_byte, |entry| entry.0),
        ) {
            results.push((boundaries[start].1, boundaries[end].1));
        }
        search_byte = start_byte
            + folded[start_byte..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
    }
    results
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > haystack.len() {
        return Vec::new();
    }

    (0..=haystack.len() - needle.len())
        .filter(|&start| {
            haystack[start..start + needle.len()]
                .iter()
                .zip(needle)
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        })
        .map(|start| (start, start + needle.len()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::FileIdentity;
    use std::path::Path;

    #[test]
    fn file_identity_path_changes_invalidate_old_aliases_without_io() {
        let old = Path::new("/fixture/old-link.rs");
        let canonical = Path::new("/fixture/old-real.rs");
        let mut doc = Document::from_loaded_text(
            "unsaved",
            FileIdentity::from_resolved(old.into(), canonical),
        );
        assert!(doc.matches_file_path(old));
        assert!(doc.matches_file_path(canonical));
        doc.file_path = Some("/fixture/new-link.rs".into());
        assert!(doc.file_identity().is_none());
        assert!(!doc.matches_file_path(old));
        assert!(!doc.matches_file_path(canonical));
        doc.set_file_identity(Some(FileIdentity::from_resolved(old.into(), canonical)));
        assert!(
            doc.file_identity().is_none(),
            "a stale reply must not bind the old identity"
        );
        doc.set_file_identity(Some(FileIdentity::from_resolved(
            "/fixture/new-link.rs".into(),
            Path::new("/fixture/new-real.rs"),
        )));
        assert!(doc.matches_file_path(Path::new("/fixture/new-real.rs")));
        assert_eq!(doc.buffer.to_string(), "unsaved");
    }

    // ========================================================================
    // Document creation tests
    // ========================================================================

    #[test]
    fn test_new_document_has_no_path() {
        let doc = Document::new();
        assert!(doc.file_path.is_none());
        assert!(!doc.is_modified);
    }

    #[test]
    fn test_new_document_empty_stacks() {
        let doc = Document::new();
        assert!(doc.undo_stack.is_empty());
        assert!(doc.redo_stack.is_empty());
    }

    #[test]
    fn test_new_document_default_language() {
        let doc = Document::new();
        assert_eq!(doc.language, LanguageId::PlainText);
    }

    #[test]
    fn test_with_text_creates_buffer() {
        let doc = Document::with_text("hello\nworld");
        assert_eq!(doc.buffer.to_string(), "hello\nworld");
        assert_eq!(doc.line_count(), 2);
    }

    // ========================================================================
    // Document::new_with_path tests
    // ========================================================================

    #[test]
    fn test_new_with_path_preserves_path() {
        let path = PathBuf::from("/tmp/newfile.rs");
        let doc = Document::new_with_path(path.clone());

        assert_eq!(doc.file_path, Some(path));
        assert!(doc.is_modified); // Should be marked modified since file doesn't exist
        assert_eq!(doc.buffer.to_string(), ""); // Empty content
    }

    #[test]
    fn test_new_with_path_detects_language() {
        let rs_doc = Document::new_with_path(PathBuf::from("test.rs"));
        assert_eq!(rs_doc.language, LanguageId::Rust);

        let py_doc = Document::new_with_path(PathBuf::from("script.py"));
        assert_eq!(py_doc.language, LanguageId::Python);

        let txt_doc = Document::new_with_path(PathBuf::from("readme.txt"));
        assert_eq!(txt_doc.language, LanguageId::PlainText);
    }

    #[test]
    fn test_new_with_path_detects_all_common_languages() {
        let test_cases = [
            ("file.js", LanguageId::JavaScript),
            ("file.ts", LanguageId::TypeScript),
            ("file.tsx", LanguageId::Tsx), // TSX is a separate language
            ("file.json", LanguageId::Json),
            ("file.yaml", LanguageId::Yaml),
            ("file.yml", LanguageId::Yaml),
            ("file.toml", LanguageId::Toml),
            ("file.md", LanguageId::Markdown),
            ("file.html", LanguageId::Html),
            ("file.css", LanguageId::Css),
            ("file.go", LanguageId::Go),
            ("file.c", LanguageId::C),
            ("file.cpp", LanguageId::Cpp),
            ("file.java", LanguageId::Java),
            ("file.sh", LanguageId::Bash),
            ("file.php", LanguageId::Php),
        ];

        for (filename, expected_lang) in test_cases {
            let doc = Document::new_with_path(PathBuf::from(filename));
            assert_eq!(
                doc.language, expected_lang,
                "Language detection failed for {}",
                filename
            );
        }
    }

    #[test]
    fn test_new_with_path_empty_stacks() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert!(doc.undo_stack.is_empty());
        assert!(doc.redo_stack.is_empty());
    }

    #[test]
    fn test_new_with_path_no_syntax_highlights() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert!(doc.syntax_highlights.is_none());
    }

    #[test]
    fn test_new_with_path_zero_revision() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert_eq!(doc.revision, 0);
    }

    #[test]
    fn test_new_with_path_no_id() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_new_with_path_absolute_path() {
        let path = PathBuf::from("/home/user/projects/myapp/src/main.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
    }

    #[test]
    fn test_new_with_path_relative_path() {
        let path = PathBuf::from("./src/lib.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
    }

    #[test]
    fn test_new_with_path_windows_style_path() {
        let path = PathBuf::from("C:\\Users\\dev\\project\\main.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
        assert_eq!(doc.language, LanguageId::Rust);
    }

    #[test]
    fn test_new_with_path_no_extension() {
        // Files without extensions default to PlainText (unless they're special)
        let doc = Document::new_with_path(PathBuf::from("README"));
        assert_eq!(doc.language, LanguageId::PlainText);
    }

    #[test]
    fn test_new_with_path_makefile() {
        // Makefiles use their dedicated grammar.
        let doc = Document::new_with_path(PathBuf::from("Makefile"));
        assert_eq!(doc.language, LanguageId::Make);
    }

    #[test]
    fn test_new_with_path_hidden_file() {
        let doc = Document::new_with_path(PathBuf::from(".gitignore"));
        assert!(doc.file_path.is_some());
        assert!(doc.is_modified);
    }

    #[test]
    fn test_new_with_path_deeply_nested() {
        let path = PathBuf::from("/a/b/c/d/e/f/g/h/file.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
    }

    // ========================================================================
    // Display name tests
    // ========================================================================

    #[test]
    fn test_display_name_with_path() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/myfile.rs"));
        assert_eq!(doc.display_name(), "myfile.rs");
    }

    #[test]
    fn test_display_name_with_untitled() {
        let mut doc = Document::new();
        doc.untitled_name = Some("Untitled-3".to_string());
        assert_eq!(doc.display_name(), "Untitled-3");
    }

    #[test]
    fn test_display_name_fallback() {
        let doc = Document::new();
        assert_eq!(doc.display_name(), "Untitled");
    }

    // ========================================================================
    // Line operations tests
    // ========================================================================

    #[test]
    fn test_line_count_empty() {
        let doc = Document::new();
        assert_eq!(doc.line_count(), 1); // Empty rope has 1 line
    }

    #[test]
    fn test_line_count_single_line() {
        let doc = Document::with_text("hello");
        assert_eq!(doc.line_count(), 1);
    }

    #[test]
    fn test_line_count_multiple_lines() {
        let doc = Document::with_text("line1\nline2\nline3");
        assert_eq!(doc.line_count(), 3);
    }

    #[test]
    fn test_line_length_excludes_newline() {
        let doc = Document::with_text("hello\nworld\n");
        assert_eq!(doc.line_length(0), 5); // "hello" not "hello\n"
        assert_eq!(doc.line_length(1), 5); // "world"
    }

    #[test]
    fn test_line_length_empty_line() {
        let doc = Document::with_text("hello\n\nworld");
        assert_eq!(doc.line_length(1), 0); // Empty line
    }

    #[test]
    fn test_get_line_valid() {
        let doc = Document::with_text("first\nsecond\nthird");
        assert_eq!(doc.get_line(0), Some("first\n".to_string()));
        assert_eq!(doc.get_line(1), Some("second\n".to_string()));
        assert_eq!(doc.get_line(2), Some("third".to_string()));
    }

    #[test]
    fn test_get_line_out_of_bounds() {
        let doc = Document::with_text("single line");
        assert!(doc.get_line(99).is_none());
    }

    // ========================================================================
    // Cursor/offset conversion tests
    // ========================================================================

    #[test]
    fn test_cursor_to_offset_start() {
        let doc = Document::with_text("hello\nworld");
        assert_eq!(doc.cursor_to_offset(0, 0), 0);
    }

    #[test]
    fn test_cursor_to_offset_second_line() {
        let doc = Document::with_text("hello\nworld");
        assert_eq!(doc.cursor_to_offset(1, 0), 6);
    }

    #[test]
    fn test_offset_to_cursor_roundtrip() {
        let doc = Document::with_text("first\nsecond\nthird");
        for offset in 0..doc.buffer.len_chars() {
            let (line, col) = doc.offset_to_cursor(offset);
            let result = doc.cursor_to_offset(line, col);
            assert_eq!(result, offset);
        }
    }

    // ========================================================================
    // Edit operation tests
    // ========================================================================

    #[test]
    fn test_push_edit_increments_revision() {
        let mut doc = Document::with_text("hello");
        let initial_rev = doc.revision;

        doc.push_edit(EditOperation::Insert {
            position: 0,
            text: "X".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        assert_eq!(doc.revision, initial_rev + 1);
    }

    #[test]
    fn test_push_edit_marks_modified() {
        let mut doc = Document::with_text("hello");
        doc.is_modified = false;

        doc.push_edit(EditOperation::Insert {
            position: 0,
            text: "X".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        assert!(doc.is_modified);
    }

    #[test]
    fn test_push_edit_clears_redo_stack() {
        let mut doc = Document::with_text("hello");
        doc.redo_stack.push(EditOperation::Insert {
            position: 0,
            text: "old".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        doc.push_edit(EditOperation::Insert {
            position: 0,
            text: "new".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        assert!(doc.redo_stack.is_empty());
    }

    // ========================================================================
    // Find tests
    // ========================================================================

    #[test]
    fn test_find_all_occurrences_basic() {
        let doc = Document::with_text("abc abc abc");
        let results = doc.find_all_occurrences("abc");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_find_all_occurrences_empty_needle() {
        let doc = Document::with_text("hello");
        let results = doc.find_all_occurrences("");
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_next_occurrence_wraps() {
        let doc = Document::with_text("abc xyz abc");
        // After position 5, next "abc" is at 8
        let result = doc.find_next_occurrence("abc", 5);
        assert_eq!(result, Some((8, 11)));

        // After position 10, wraps to first occurrence at 0
        let result = doc.find_next_occurrence("abc", 10);
        assert_eq!(result, Some((0, 3)));
    }

    #[test]
    fn test_find_next_occurrence_empty_needle() {
        let doc = Document::with_text("hello");
        assert_eq!(doc.find_next_occurrence("", 0), None);
    }

    #[test]
    fn test_find_next_occurrence_not_found() {
        let doc = Document::with_text("abc xyz");
        assert_eq!(doc.find_next_occurrence("zzz", 0), None);
    }

    #[test]
    fn test_find_next_occurrence_from_match_start() {
        let doc = Document::with_text("abc xyz abc");
        // Start at first match start: should find next match (not same one)
        // because we search for occurrences where start > after_offset
        assert_eq!(doc.find_next_occurrence("abc", 0), Some((8, 11)));
    }

    #[test]
    fn test_find_next_occurrence_start_past_end() {
        let doc = Document::with_text("abc xyz");
        let len = doc.buffer.len_chars();
        // Past end should wrap to first
        assert_eq!(doc.find_next_occurrence("abc", len + 10), Some((0, 3)));
    }

    // ========================================================================
    // Case-insensitive find tests
    // ========================================================================

    #[test]
    fn test_find_case_insensitive_basic() {
        let doc = Document::with_text("Hello HELLO hello");
        let results = doc.find_all_occurrences_with_options("hello", false);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], (0, 5));
        assert_eq!(results[1], (6, 11));
        assert_eq!(results[2], (12, 17));
    }

    #[test]
    fn test_find_case_sensitive_basic() {
        let doc = Document::with_text("Hello HELLO hello");
        let results = doc.find_all_occurrences_with_options("hello", true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (12, 17));
    }

    #[test]
    fn test_find_next_case_insensitive() {
        let doc = Document::with_text("Hello HELLO hello");
        let result = doc.find_next_occurrence_with_options("HELLO", 0, false);
        assert_eq!(result, Some((6, 11))); // First after position 0
    }

    // ========================================================================
    // Unicode find tests
    // ========================================================================

    #[test]
    fn test_find_unicode_single_char() {
        let doc = Document::with_text("αβγ αβγ");
        let results = doc.find_all_occurrences("β");
        assert_eq!(results, vec![(1, 2), (5, 6)]);
    }

    #[test]
    fn test_find_unicode_word() {
        let doc = Document::with_text("café café");
        let results = doc.find_all_occurrences("café");
        assert_eq!(results, vec![(0, 4), (5, 9)]);
    }

    #[test]
    fn test_find_unicode_emoji() {
        let doc = Document::with_text("hello 🎉 world 🎉 end");
        let results = doc.find_all_occurrences("🎉");
        assert_eq!(results, vec![(6, 7), (14, 15)]);
    }

    #[test]
    fn test_find_unicode_mixed() {
        let doc = Document::with_text("日本語テスト 日本語");
        let results = doc.find_all_occurrences("日本語");
        assert_eq!(results, vec![(0, 3), (7, 10)]);
    }

    #[test]
    fn test_find_unicode_case_insensitive() {
        let doc = Document::with_text("Ößer ößer ÖSSER");
        // German sharp s case folding
        let results = doc.find_all_occurrences_with_options("ößer", false);
        // Note: simple lowercase may not handle ẞ properly, but ö should work
        assert!(results.len() >= 2);
    }

    // ========================================================================
    // Find previous tests
    // ========================================================================

    #[test]
    fn test_find_prev_occurrence_basic() {
        let doc = Document::with_text("abc xyz abc");
        // Before position 5, prev "abc" is at 0
        let result = doc.find_prev_occurrence_with_options("abc", 5, true);
        assert_eq!(result, Some((0, 3)));

        // Before position 10 (after second "abc" starts at 8), prev is at 8
        let result = doc.find_prev_occurrence_with_options("abc", 10, true);
        assert_eq!(result, Some((8, 11)));
    }

    #[test]
    fn test_find_prev_occurrence_wraps() {
        let doc = Document::with_text("abc xyz abc");
        // Before position 2 (inside first match), should wrap to last
        let result = doc.find_prev_occurrence_with_options("abc", 0, true);
        assert_eq!(result, Some((8, 11)));
    }

    #[test]
    fn test_find_prev_occurrence_empty_needle() {
        let doc = Document::with_text("hello");
        assert_eq!(doc.find_prev_occurrence_with_options("", 5, true), None);
    }

    // ========================================================================
    // Overlapping match tests
    // ========================================================================

    #[test]
    fn test_find_overlapping_matches() {
        let doc = Document::with_text("aaaa");
        let results = doc.find_all_occurrences("aa");
        // Should find overlapping: (0,2), (1,3), (2,4)
        assert_eq!(results, vec![(0, 2), (1, 3), (2, 4)]);
    }

    #[test]
    fn test_find_overlapping_pattern() {
        let doc = Document::with_text("ababa");
        let results = doc.find_all_occurrences("aba");
        // Should find overlapping: (0,3), (2,5)
        assert_eq!(results, vec![(0, 3), (2, 5)]);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_find_in_empty_document() {
        let doc = Document::with_text("");
        assert!(doc.find_all_occurrences("test").is_empty());
        assert_eq!(doc.find_next_occurrence("test", 0), None);
    }

    #[test]
    fn test_find_needle_longer_than_haystack() {
        let doc = Document::with_text("ab");
        assert!(doc.find_all_occurrences("abcdef").is_empty());
    }

    #[test]
    fn test_find_exact_match() {
        let doc = Document::with_text("hello");
        let results = doc.find_all_occurrences("hello");
        assert_eq!(results, vec![(0, 5)]);
    }

    #[test]
    fn test_find_with_newlines() {
        let doc = Document::with_text("line1\nline2\nline1");
        let results = doc.find_all_occurrences("line1");
        assert_eq!(results, vec![(0, 5), (12, 17)]);
    }

    #[test]
    fn test_find_newline_character() {
        let doc = Document::with_text("a\nb\nc");
        let results = doc.find_all_occurrences("\n");
        assert_eq!(results, vec![(1, 2), (3, 4)]);
    }
}
