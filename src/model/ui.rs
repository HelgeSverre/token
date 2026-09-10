//! UI state - status bar, cursor blink, modals, and other UI concerns

use super::editor_area::{GroupId, SplitDirection};
use super::status_bar::{SegmentContent, SegmentId, StatusBar, TransientMessage};
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use crate::panel::DockPosition;
use crate::theme::{list_available_themes, ThemeInfo};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::{
    cell::RefCell,
    sync::{Arc, OnceLock},
};

// ============================================================================
// Focus Management
// ============================================================================

/// Which top-level UI region currently has keyboard focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    /// Main editor text area (default)
    #[default]
    Editor,
    /// A dock panel, including the file explorer in the left dock
    Dock(DockPosition),
    /// Docked find/replace text field.
    FindBar,
    /// Modal dialog (command palette, goto line, etc.)
    Modal,
}

/// Which UI region the mouse is currently hovering over
///
/// Used for:
/// - Determining scroll event targets (sidebar vs editor vs dock)
/// - Setting appropriate cursor icons
/// - Visual hover feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverRegion {
    /// Not hovering over any tracked region
    #[default]
    None,
    /// Hovering over the sidebar file tree
    Sidebar,
    /// Hovering over the sidebar resize handle
    SidebarResize,
    /// Hovering over the editor text area
    EditorText,
    FindBar(Option<crate::view::find_bar::Control>),
    /// Hovering over the editor tab bar
    EditorTabBar,
    /// Hovering over the status bar
    StatusBar,
    /// Hovering over a modal dialog
    Modal,
    /// Hovering over a cursor-anchored popup (completion/hover) — distinct
    /// from `Modal` since wheel/scroll routes to `ui.cursor_overlay`, not
    /// `ui.active_modal` (overlay-surface.md Phase 5).
    CursorOverlay,
    /// Hovering over a splitter (split view resize handle)
    Splitter,
    /// Hovering over a dock panel (right or bottom)
    Dock(DockPosition),
    /// Hovering over a dock resize handle
    DockResize(DockPosition),
    /// Hovering over a preview pane
    Preview,
    /// Hovering over a button control in a specific editor group (e.g. the
    /// binary-placeholder "Open Anyway" button). Carrying the group id keeps
    /// hover state correct when multiple such buttons are visible across a
    /// split layout.
    Button(GroupId),
}

// ============================================================================
// Modal System
// ============================================================================

/// Identifies which modal is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalId {
    UnsavedChanges,
    FileConflict,
    Settings,
    /// Command palette (Shift+Cmd+A)
    CommandPalette,
    /// Go to line dialog (Cmd+L)
    GotoLine,
    /// Theme picker
    ThemePicker,
    /// File finder (Shift+Cmd+O) - fuzzy search files in workspace
    FileFinder,
    /// Recent files list (Cmd+E)
    RecentFiles,
    /// Language Servers picker — one row per registered server def, toggles
    /// `lsp.servers.<id>.enabled` (see `CommandId::ManageLanguageServers`).
    LspServers,
    /// "Set Language..." picker — one row per `syntax::registry::ALL_LANGUAGES`
    /// entry, pins the chosen language on the focused document.
    LanguagePicker,
    /// Rename Symbol prompt (Shift+F6) — opened by `LspMsg::RenameSymbol`
    /// only; it needs the caret context captured at request time.
    RenameSymbol,
}

/// Palette/pickers cap at 10 visible rows (overlay-surface.md Visual
/// Language > Overflow), shared by the update-layer scroll math and the view
/// so they can't drift apart.
pub const COMMAND_PALETTE_MAX_VISIBLE: usize = 10;

/// One row in the command palette's ordering-authority cache
/// (`update::ui::resolve_palette_rows`): a command plus the nucleo match
/// indices for the current query, used for match highlighting.
#[derive(Debug, Clone)]
pub struct CommandMatch {
    pub def: &'static crate::commands::CommandDef,
    pub indices: Vec<u32>,
}

/// Which tab of Search Everywhere (overlay-surface.md Phase 4) is active.
/// The palette modal absorbs the file finder: `Files` and `Symbols` are
/// `Unavailable` (dimmed, ⇥-skipped, unclickable) whenever no workspace is
/// open / no LSP workspace-symbols provider exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTab {
    All,
    Commands,
    Files,
    Symbols,
}

impl SearchTab {
    pub const ORDER: [SearchTab; 4] = [
        SearchTab::All,
        SearchTab::Commands,
        SearchTab::Files,
        SearchTab::Symbols,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchTab::All => "All",
            SearchTab::Commands => "Commands",
            SearchTab::Files => "Files",
            SearchTab::Symbols => "Symbols",
        }
    }
}

/// State for the Search Everywhere modal (overlay-surface.md Phase 4: All /
/// Commands / Files / Symbols tabs over one shared query). Absorbs the old
/// standalone File Finder — `files` is the finder's own state, populated
/// lazily on first activation of the Files tab rather than eagerly at open
/// time, so opening the palette to run a command never pays the file-tree
/// walk.
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// Editable state for the input field, shared across all tabs (the
    /// query persists when switching tabs).
    pub editable: EditableState<StringBuffer>,
    /// Index of selected command in `matches` — the Commands tab's
    /// selection (the ordering authority).
    pub selected_index: usize,
    /// Filtered and ranked commands for the current query — the single
    /// source of truth the view and `ModalMsg::Confirm`/`SelectNext` both
    /// read from, recomputed by `resolve_palette_rows` whenever the input
    /// changes. See overlay-surface.md "Ordering authority". When
    /// `recent_count > 0` the first `recent_count` entries are also the
    /// "Recently used" section (duplicated into the tail of the full list).
    pub matches: Vec<CommandMatch>,
    /// Scroll offset (in rows) for the Commands tab's visible window.
    pub scroll_offset: usize,
    /// How many of `matches`' leading entries form the "Recently used"
    /// section (0 unless the query is empty — overlay-surface.md: the
    /// section disappears on the first typed character).
    pub recent_count: usize,
    /// Which tab is active.
    pub active_tab: SearchTab,
    /// The Files tab's backing state — `None` until first activated (lazy
    /// population) or when no workspace is open.
    pub files: Option<FileFinderState>,
    /// Whether a workspace is open (Files/Symbols availability).
    pub files_available: bool,
    pub symbols: WorkspaceSymbolsState,
    /// The All tab's flat selection (its own tab, not scrollable — capped
    /// per-group summary).
    pub all_selected: usize,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            selected_index: 0,
            matches: crate::commands::all_commands()
                .map(|def| CommandMatch {
                    def,
                    indices: Vec::new(),
                })
                .collect(),
            scroll_offset: 0,
            recent_count: 0,
            // `Commands` (not `All`) so existing callers that construct
            // this directly (tests, restore-without-explicit-tab) keep
            // today's flat Commands-list behavior; `Cmd+Shift+A`
            // explicitly forces `All` at open time.
            active_tab: SearchTab::Commands,
            files: None,
            files_available: false,
            symbols: WorkspaceSymbolsState::default(),
            all_selected: 0,
        }
    }
}

impl CommandPaletteState {
    /// Get the input text (convenience accessor)
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Set the input text (replaces content)
    pub fn set_input(&mut self, text: &str) {
        self.editable.set_content(text);
    }

    /// Whether a tab has its required workspace/provider context.
    pub fn tab_available(&self, t: SearchTab) -> bool {
        match t {
            SearchTab::All | SearchTab::Commands => true,
            SearchTab::Files => self.files_available,
            SearchTab::Symbols => self.symbols.available,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceSymbolsState {
    pub available: bool,
    pub searching: bool,
    pub query_too_long: bool,
    pub results: crate::lsp::workspace_symbols::SymbolResults,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

impl WorkspaceSymbolsState {
    pub fn status(&self) -> Option<&'static str> {
        if !self.available {
            Some("No running language server supports workspace symbols")
        } else if self.query_too_long {
            Some("Symbol queries are limited to 256 characters")
        } else if self.searching {
            Some("Searching symbols…")
        } else if self.results.failures > 0 {
            Some("Some symbols unavailable — change query to retry")
        } else if self.results.truncated {
            Some("Showing the first 2,000 symbols — refine your query")
        } else if self.results.items.is_empty() {
            Some("No symbols match your query")
        } else {
            None
        }
    }
}

/// State for the goto line modal
#[derive(Debug, Clone)]
pub struct GotoLineState {
    /// Editable state for the input field (numeric + colon only)
    pub editable: EditableState<StringBuffer>,
}

impl Default for GotoLineState {
    fn default() -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::goto_line()),
        }
    }
}

impl GotoLineState {
    /// Get the input text (convenience accessor)
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Set the input text (replaces content)
    pub fn set_input(&mut self, text: &str) {
        self.editable.set_content(text);
    }
}

/// State for the Rename Symbol prompt: a single-line input prefilled with
/// the placeholder (selected, so typing replaces it) plus the document
/// context the `textDocument/rename` request must carry.
#[derive(Debug, Clone)]
pub struct RenameSymbolState {
    pub editable: EditableState<StringBuffer>,
    pub placeholder: String,
    pub document_id: crate::model::editor_area::DocumentId,
    pub revision: u64,
    pub position: crate::model::editor::Position,
}

impl RenameSymbolState {
    pub fn new(
        placeholder: String,
        document_id: crate::model::editor_area::DocumentId,
        revision: u64,
        position: crate::model::editor::Position,
    ) -> Self {
        let mut editable = EditableState::new(StringBuffer::new(), EditConstraints::single_line());
        editable.set_content(&placeholder);
        editable.select_all();
        Self {
            editable,
            placeholder,
            document_id,
            revision,
            position,
        }
    }

    pub fn input(&self) -> String {
        self.editable.text()
    }
}

/// Which field is focused in find/replace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FindReplaceField {
    #[default]
    Query,
    Replace,
}

/// Search session displayed in the active editor's docked find bar.
#[derive(Debug, Clone)]
pub struct FindReplaceState {
    /// Scope belongs to this document, never whichever tab happens to open next.
    pub(crate) document_id: Option<crate::model::DocumentId>,
    /// Editable state for the query field
    pub query_editable: EditableState<StringBuffer>,
    /// Editable state for the replacement field
    pub replace_editable: EditableState<StringBuffer>,
    /// Which field is currently focused
    pub focused_field: FindReplaceField,
    /// Whether replace mode is active (vs find-only)
    pub replace_mode: bool,
    /// Case-sensitive search
    pub case_sensitive: bool,
    /// Match whole words only (`\b` boundaries)
    pub whole_word: bool,
    /// Interpret the query as a regular expression
    pub use_regex: bool,
    /// Restrict matches to `scope` (find-enhancements.md Phase 7)
    pub selection_only: bool,
    /// Char-offset range captured from the primary selection when
    /// `selection_only` was switched on, then mapped through edits while the
    /// modal is active. `None` searches the whole document; an empty range stays scoped.
    pub scope: Option<(usize, usize)>,
    // Memoized derived data only; edits and option changes are checked on every read.
    search_cache: RefCell<Option<Arc<FindResults>>>,
    pending_search: Option<Arc<FindSearchRequest>>,
    search_failure: Option<(Arc<FindSearchRequest>, String)>,
}

/// Immutable background-search input. Its identity also guards reply ownership.
pub struct FindSearchRequest {
    document_id: Option<crate::model::editor_area::DocumentId>,
    revision: u64,
    buffer: ropey::Rope,
    pattern: String,
    options: (bool, bool, bool),
    scope: Option<(usize, usize)>,
}

impl std::fmt::Debug for FindSearchRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Update tracing formats messages even without a subscriber. Never copy
        // the document/query text into logs or walk a large result vector there.
        f.debug_struct("FindSearchRequest")
            .field("document_id", &self.document_id)
            .field("revision", &self.revision)
            .field(
                "buffer_size",
                &crate::util::ByteSize::bytes(self.buffer.len_bytes() as u64),
            )
            .field(
                "pattern_size",
                &crate::util::ByteSize::bytes(self.pattern.len() as u64),
            )
            .field("options", &self.options)
            .field("scope", &self.scope)
            .finish()
    }
}

impl FindSearchRequest {
    fn new(state: &FindReplaceState, document: &crate::model::Document) -> Arc<Self> {
        Arc::new(Self {
            document_id: document.id,
            revision: document.revision,
            buffer: document.buffer.clone(),
            pattern: state.query().to_owned(),
            options: (state.case_sensitive, state.whole_word, state.use_regex),
            scope: state.selection_only.then_some(state.scope).flatten(),
        })
    }

    fn matches(&self, state: &FindReplaceState, document: &crate::model::Document) -> bool {
        self.document_id == document.id
            && self.revision == document.revision
            && self.buffer.is_instance(&document.buffer)
            && self.pattern == state.query()
            && self.options == (state.case_sensitive, state.whole_word, state.use_regex)
            && self.scope == state.selection_only.then_some(state.scope).flatten()
    }

    /// Pure computation against this snapshot; the runtime may run it off-thread.
    pub fn compute(self: &Arc<Self>) -> Arc<FindResults> {
        let results = self.match_results();
        // Background display work includes the logical overview projection.
        // Explicit navigation's synchronous fallback keeps that projection lazy.
        results.lines();
        results
    }

    fn match_results(self: &Arc<Self>) -> Arc<FindResults> {
        let query = crate::search::SearchQuery::new(
            &self.pattern,
            self.options.0,
            self.options.1,
            self.options.2,
        );
        let mut matches = if query.is_valid() {
            query.find_all(&self.buffer.to_string())
        } else {
            Vec::new()
        };
        if let Some((start, end)) = self.scope {
            matches.retain(|m| m.start >= start && m.end <= end);
        }
        Arc::new(FindResults {
            matches: matches.into(),
            source: Arc::clone(self),
            lines: OnceLock::new(),
            error: query.error,
        })
    }
}

/// Shared search output, including the document-wide scrollbar projection.
pub struct FindResults {
    pub(crate) matches: Arc<[crate::search::Match]>,
    source: Arc<FindSearchRequest>,
    lines: OnceLock<Vec<usize>>,
    error: Option<String>,
}

impl std::fmt::Debug for FindResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FindResults")
            .field("source", &self.source)
            .field("match_count", &self.matches.len())
            .field("overview_ready", &self.lines.get().is_some())
            .field("has_error", &self.error.is_some())
            .finish()
    }
}

impl FindResults {
    /// Compute overview lines only when a scrollbar needs them. Reuse the Rope
    /// chunk for nearby matches and seek directly across gaps. Advance within
    /// each chunk so its prefix is never rescanned for subsequent matches.
    pub(crate) fn lines(&self) -> &[usize] {
        self.lines.get_or_init(|| {
            let buffer = &self.source.buffer;
            let mut result = Vec::new();
            let mut chunk = "";
            let mut char_offset = 0;
            let mut chunk_end = 0;
            let mut line = 0;
            for m in self.matches.iter() {
                if m.start >= chunk_end {
                    (chunk, _, char_offset, line) = buffer.chunk_at_char(m.start);
                    chunk_end = char_offset + chunk.chars().count();
                }
                let byte_offset = ropey::str_utils::char_to_byte_idx(chunk, m.start - char_offset);
                // Count against the full suffix before trimming: Ropey's helper
                // leaves a CRLF break pending when the match starts on its LF.
                line += ropey::str_utils::byte_to_line_idx(chunk, byte_offset);
                chunk = &chunk[byte_offset..];
                char_offset = m.start;
                if result.last() != Some(&line) {
                    result.push(line);
                }
            }
            result
        })
    }
}

/// What the find modal reports next to the query: the match count with
/// the current match's ordinal when the selection sits on one, or why
/// the query could not be compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindStatus {
    Error(String),
    Searching,
    Unavailable(String),
    Count {
        total: usize,
        current: Option<usize>,
    },
}

impl FindStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Searching => "Searching…".to_owned(),
            Self::Unavailable(error) => format!("Search unavailable: {error}"),
            // `regex::Error` renders as a multi-line diagnostic whose last
            // line is the reason ("error: unclosed group"); that is the
            // part that fits on a label row.
            Self::Error(error) => {
                let reason = error
                    .lines()
                    .last()
                    .unwrap_or_default()
                    .trim()
                    .trim_start_matches("error: ");
                format!("Invalid regex: {reason}")
            }
            Self::Count { total: 0, .. } => "No matches".to_owned(),
            Self::Count {
                total,
                current: Some(current),
            } => format!("{} of {total}", current + 1),
            Self::Count { total: 1, .. } => "1 match".to_owned(),
            Self::Count { total, .. } => format!("{total} matches"),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_) | Self::Unavailable(_))
    }
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self {
            document_id: None,
            query_editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            replace_editable: EditableState::new(
                StringBuffer::new(),
                EditConstraints::single_line(),
            ),
            focused_field: FindReplaceField::Query,
            replace_mode: false,
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            selection_only: false,
            scope: None,
            search_cache: RefCell::default(),
            pending_search: None,
            search_failure: None,
        }
    }
}

impl FindReplaceState {
    /// Every match the current query and scope produce — the one list
    /// navigation, replace, highlighting, and the status label all share.
    pub fn matches(&self, document: &crate::model::Document) -> Arc<[crate::search::Match]> {
        Arc::clone(&self.results(document).matches)
    }

    pub(crate) fn results(&self, document: &crate::model::Document) -> Arc<FindResults> {
        if let Some(results) = self.cached_results(document) {
            return results;
        }
        let results = FindSearchRequest::new(self, document).match_results();
        *self.search_cache.borrow_mut() = Some(Arc::clone(&results));
        results
    }

    fn cached_results(&self, document: &crate::model::Document) -> Option<Arc<FindResults>> {
        self.search_cache
            .borrow()
            .as_ref()
            .filter(|results| results.source.matches(self, document))
            .cloned()
    }

    fn needs_background(&self, document: &crate::model::Document) -> bool {
        !self.query().is_empty()
            && document.buffer.len_bytes() >= crate::util::ByteSize::kibibytes(256).as_usize()
    }

    /// Rendering must never trigger a large cold scan or use stale match offsets.
    pub(crate) fn display_results(
        &self,
        document: &crate::model::Document,
    ) -> Option<Arc<FindResults>> {
        self.cached_results(document)
            .or_else(|| (!self.needs_background(document)).then(|| self.results(document)))
    }

    pub(crate) fn prepare_search(
        &mut self,
        document: &crate::model::Document,
    ) -> Option<Arc<FindSearchRequest>> {
        if !self.needs_background(document) || self.cached_results(document).is_some() {
            self.pending_search = None;
            self.search_failure = None;
            return None;
        }
        if self
            .pending_search
            .as_ref()
            .is_some_and(|request| request.matches(self, document))
            || self
                .search_failure
                .as_ref()
                .is_some_and(|(request, _)| request.matches(self, document))
        {
            return None;
        }
        let request = FindSearchRequest::new(self, document);
        self.pending_search = Some(Arc::clone(&request));
        self.search_failure = None;
        Some(request)
    }

    pub(crate) fn finish_search(
        &mut self,
        document: &crate::model::Document,
        request: Arc<FindSearchRequest>,
        result: Result<Arc<FindResults>, String>,
    ) -> bool {
        if !self
            .pending_search
            .as_ref()
            .is_some_and(|pending| Arc::ptr_eq(pending, &request))
            || !request.matches(self, document)
        {
            return false;
        }
        match result {
            Ok(results) if Arc::ptr_eq(&results.source, &request) => {
                *self.search_cache.borrow_mut() = Some(results);
                self.search_failure = None;
            }
            Ok(_) => return false,
            Err(error) => self.search_failure = Some((request, error)),
        }
        self.pending_search = None;
        true
    }

    pub(crate) fn reset_search_session(&mut self) {
        self.pending_search = None;
        self.search_failure = None;
    }

    /// The status label input: `None` while the query is empty.
    pub fn status(
        &self,
        document: &crate::model::Document,
        selection: &crate::model::Selection,
    ) -> Option<FindStatus> {
        if self.query().is_empty() {
            return None;
        }
        if let Some((request, error)) = &self.search_failure {
            if request.matches(self, document) {
                return Some(FindStatus::Unavailable(error.clone()));
            }
        }
        let Some(results) = self.display_results(document) else {
            return Some(FindStatus::Searching);
        };
        if let Some(error) = &results.error {
            return Some(FindStatus::Error(error.clone()));
        }
        let matches = &results.matches;
        let current = (!selection.is_empty()).then(|| {
            let (start, end) = (selection.start(), selection.end());
            (
                document.cursor_to_offset(start.line, start.column),
                document.cursor_to_offset(end.line, end.column),
            )
        });
        Some(FindStatus::Count {
            total: matches.len(),
            current: current.and_then(|(start, end)| {
                matches
                    .binary_search_by_key(&(start, end), |m| (m.start, m.end))
                    .ok()
            }),
        })
    }

    /// Switch selection scope on (capturing `selection` as the range) or
    /// off. An empty selection cannot scope anything, so it switches off.
    pub fn set_selection_only(
        &mut self,
        on: bool,
        document: &crate::model::Document,
        selection: &crate::model::Selection,
    ) {
        if on && !selection.is_empty() {
            let (start, end) = (selection.start(), selection.end());
            self.scope = Some((
                document.cursor_to_offset(start.line, start.column),
                document.cursor_to_offset(end.line, end.column),
            ));
            self.selection_only = true;
        } else {
            self.scope = None;
            self.selection_only = false;
        }
    }

    /// Get the query text (convenience accessor)
    pub fn query(&self) -> String {
        self.query_editable.text()
    }

    /// Set the query text (replaces content)
    pub fn set_query(&mut self, text: &str) {
        self.query_editable.set_content(text);
    }

    /// Get the replacement text (convenience accessor)
    pub fn replacement(&self) -> String {
        self.replace_editable.text()
    }

    /// Set the replacement text (replaces content)
    pub fn set_replacement(&mut self, text: &str) {
        self.replace_editable.set_content(text);
    }

    /// Get the currently focused editable state
    pub fn focused_editable(&self) -> &EditableState<StringBuffer> {
        match self.focused_field {
            FindReplaceField::Query => &self.query_editable,
            FindReplaceField::Replace => &self.replace_editable,
        }
    }

    /// Get the currently focused editable state mutably
    pub fn focused_editable_mut(&mut self) -> &mut EditableState<StringBuffer> {
        match self.focused_field {
            FindReplaceField::Query => &mut self.query_editable,
            FindReplaceField::Replace => &mut self.replace_editable,
        }
    }

    /// Toggle focus between query and replacement fields
    pub fn toggle_field(&mut self) {
        self.focused_field = match self.focused_field {
            FindReplaceField::Query => FindReplaceField::Replace,
            FindReplaceField::Replace => FindReplaceField::Query,
        };
    }

    /// Build a compiled search query from the current text and options —
    /// the single source both find navigation and match-highlighting
    /// decorations read from, so they can never drift apart.
    pub fn build_query(&self) -> crate::search::SearchQuery {
        crate::search::SearchQuery::new(
            &self.query(),
            self.case_sensitive,
            self.whole_word,
            self.use_regex,
        )
    }
}

/// State for the theme picker modal
#[derive(Debug, Clone)]
pub struct ThemePickerState {
    /// Index of selected theme in list
    pub selected_index: usize,
    /// Cached list of available themes (refreshed when modal opens)
    pub themes: Vec<ThemeInfo>,
    /// Original theme ID for restore on cancel
    pub original_theme_id: String,
    /// Per-theme representative colors, parallel to `themes` (accent dot +
    /// palette strip in the picker rows).
    pub swatches: Vec<crate::theme::ThemeSwatch>,
    /// Scroll offset (in rows) for keeping a long theme list clipped and the
    /// selection visible instead of overflowing the modal.
    pub scroll_offset: usize,
}

impl ThemePickerState {
    /// Create a new theme picker state with the current theme for restore
    pub fn new(current_theme_id: String) -> Self {
        let themes = list_available_themes();
        // One theme parse per listed theme, same cost class as the live
        // preview-on-selection the picker already does.
        let swatches = themes
            .iter()
            .map(|info| {
                crate::theme::load_theme(&info.id)
                    .map(|t| t.swatch())
                    .unwrap_or_else(|_| crate::theme::ThemeSwatch::fallback())
            })
            .collect();
        Self {
            selected_index: 0,
            themes,
            swatches,
            original_theme_id: current_theme_id,
            scroll_offset: 0,
        }
    }
}

/// State for the Language Servers picker modal (dynamic — one row per
/// entry in `lsp::all_server_defs()`, built fresh from `model.lsp` /
/// `model.config.lsp` at render time each frame instead of cached here,
/// since the registry is static and the live state already lives in the
/// model — see `docs/feature/lsp-integration.md`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LspServersState {
    /// Index of selected server in `lsp::all_server_defs()`
    pub selected_index: usize,
    /// Scroll offset (in rows) for keeping a long server list clipped and
    /// the selection visible instead of overflowing the modal.
    pub scroll_offset: usize,
}

/// State for the "Set Language..." picker (rows come from the static
/// `syntax::registry::ALL_LANGUAGES`, so only the cursor lives here).
#[derive(Debug, Clone, Copy, Default)]
pub struct LanguagePickerState {
    /// Index of the selected language in `ALL_LANGUAGES`
    pub selected_index: usize,
    /// Scroll offset (in rows), see `LspServersState::scroll_offset`.
    pub scroll_offset: usize,
}

impl LanguagePickerState {
    /// Open with the document's current language preselected.
    pub fn new(current: crate::syntax::LanguageId) -> Self {
        let selected_index = crate::syntax::LanguageId::all()
            .position(|language| language == current)
            .unwrap_or(0);
        Self {
            selected_index,
            scroll_offset: 0,
        }
    }
}

/// A file match result from fuzzy search
#[derive(Debug, Clone)]
pub struct FileMatch {
    /// Full path to the file
    pub path: PathBuf,
    /// Just the file name (for display)
    pub filename: String,
    /// Path relative to workspace root (for display)
    pub relative_path: String,
    /// Fuzzy match score (higher = better match)
    pub score: u32,
    /// Character indices in filename that matched (for highlighting)
    pub indices: Vec<u32>,
}

impl FileMatch {
    /// Create a FileMatch from a path
    pub fn from_path(
        path: &std::path::Path,
        workspace_root: &std::path::Path,
        score: u32,
        indices: Vec<u32>,
    ) -> Self {
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let relative_path = path
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        Self {
            path: path.to_path_buf(),
            filename,
            relative_path,
            score,
            indices,
        }
    }
}

/// State for the file finder modal (fuzzy file search)
#[derive(Debug, Clone)]
pub struct FileFinderState {
    /// Editable state for the search input field
    pub editable: EditableState<StringBuffer>,
    /// Index of selected file in filtered results
    pub selected_index: usize,
    /// Filtered and ranked file results
    pub results: Vec<FileMatch>,
    /// All files in workspace (cached when modal opens)
    pub all_files: Vec<PathBuf>,
    /// Workspace root path (for computing relative paths)
    pub workspace_root: PathBuf,
    /// Scroll offset (in rows) for the visible window, maintained by the
    /// update layer (minimal-reveal scrolling) as selection moves.
    pub scroll_offset: usize,
}

impl FileFinderState {
    /// Create a new file finder state with the given files
    pub fn new(all_files: Vec<PathBuf>, workspace_root: PathBuf) -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            selected_index: 0,
            results: Vec::new(),
            all_files,
            workspace_root,
            scroll_offset: 0,
        }
    }

    /// Get the search input text
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Set the search input text
    pub fn set_input(&mut self, text: &str) {
        self.editable.set_content(text);
    }
}

/// State for the recent files modal
#[derive(Debug, Clone)]
pub struct RecentFilesState {
    /// Index into `filtered_rows` (the ordering authority) — `FlatIndex`
    /// space, not `entries` space.
    pub selected_index: usize,
    /// Cached entries for display
    pub entries: Vec<crate::recent_files::RecentEntry>,
    /// Editable state for optional filter input
    pub editable: EditableState<StringBuffer>,
    /// Scroll offset (in rows) for the visible window, maintained by the
    /// update layer (minimal-reveal scrolling) as selection moves.
    pub scroll_offset: usize,
    /// Ordering authority: indices into `entries`, filtered by the current
    /// query and grouped Pinned / Today / Yesterday / Earlier
    /// (overlay-surface.md "Ordering authority") — recomputed by
    /// `recompute_filtered_rows` whenever the input or entries change. The
    /// view's spec builder and `ModalMsg::Confirm`/`SelectNext` both index
    /// through this instead of re-deriving it independently.
    pub filtered_rows: Vec<usize>,
}

impl RecentFilesState {
    /// Create from the current recent files list.
    ///
    /// Keeps MRU ordering intact (including the currently open file) but, when
    /// the current file is at the top, preselects the next item to preserve
    /// quick-switch behavior on immediate confirm.
    pub fn new(
        recent: &crate::recent_files::RecentFiles,
        current_file: Option<&std::path::Path>,
    ) -> Self {
        let entries = recent.entries.clone();
        let selected_index = match current_file {
            Some(current)
                if entries.len() > 1
                    && entries
                        .first()
                        .is_some_and(|entry| entry.path.as_path() == current) =>
            {
                1
            }
            _ => 0,
        };
        let mut state = Self {
            selected_index,
            entries,
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            scroll_offset: 0,
            filtered_rows: Vec::new(),
        };
        state.recompute_filtered_rows();
        // Re-map the preselected entry index (computed above in `entries`
        // space) into `filtered_rows` space now that it's populated.
        state.selected_index = state
            .filtered_rows
            .iter()
            .position(|&i| i == selected_index)
            .unwrap_or(0);
        state
    }

    /// Get the filter text
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Recompute `filtered_rows`: entries matching the current query,
    /// stable-sorted by group (Pinned first, then Today/Yesterday/Earlier),
    /// preserving MRU order within each group. The ordering authority for
    /// the Recent Files modal — see `filtered_rows` doc comment.
    pub fn recompute_filtered_rows(&mut self) {
        let filter = self.input().to_lowercase();
        let mut order: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| filter.is_empty() || e.display_path().to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect();
        order.sort_by_key(|&i| self.entries[i].group() as u8);
        self.filtered_rows = order;
    }

    /// The entry currently selected in `FlatIndex` space, if any.
    pub fn selected_entry(&self) -> Option<&crate::recent_files::RecentEntry> {
        self.filtered_rows
            .get(self.selected_index)
            .and_then(|&i| self.entries.get(i))
    }
}

/// Union of all modal states
#[derive(Debug, Clone)]
pub enum ModalState {
    UnsavedChanges(super::UnsavedChangesState),
    FileConflict(super::FileConflictState),
    Settings(crate::settings::SettingsState),
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    ThemePicker(ThemePickerState),
    FileFinder(FileFinderState),
    RecentFiles(RecentFilesState),
    LspServers(LspServersState),
    LanguagePicker(LanguagePickerState),
    RenameSymbol(RenameSymbolState),
}

impl ModalState {
    /// Get the modal ID for this state
    pub fn id(&self) -> ModalId {
        match self {
            ModalState::UnsavedChanges(_) => ModalId::UnsavedChanges,
            ModalState::FileConflict(_) => ModalId::FileConflict,
            ModalState::Settings(_) => ModalId::Settings,
            ModalState::CommandPalette(_) => ModalId::CommandPalette,
            ModalState::GotoLine(_) => ModalId::GotoLine,
            ModalState::ThemePicker(_) => ModalId::ThemePicker,
            ModalState::FileFinder(_) => ModalId::FileFinder,
            ModalState::RecentFiles(_) => ModalId::RecentFiles,
            ModalState::LspServers(_) => ModalId::LspServers,
            ModalState::LanguagePicker(_) => ModalId::LanguagePicker,
            ModalState::RenameSymbol(_) => ModalId::RenameSymbol,
        }
    }
}

// ============================================================================
// Cursor-anchored popups (overlay-surface.md Phase 5)
// ============================================================================

/// Which cursor-anchored popup is currently open. Demo shells still exist
/// for manual exercising of the geometry/routing; the real consumers are
/// `Completion` (autocomplete.md Phase 1) and `Hover` (lsp-integration.md
/// Phase 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorOverlayKind {
    /// Demonstrates the Completion list shell (kind badges, dim signature).
    DebugCompletion,
    /// Demonstrates the hover `Zones` card (banner/code/text).
    DebugHover,
    /// The real menu-completion popup (autocomplete.md Phase 1), backed by
    /// `UiState::completion_menu`.
    Completion,
    /// The real `textDocument/hover` card (lsp-integration.md Phase 4),
    /// backed by `UiState::hover_card`. Dismissed on any keypress/edit/
    /// cursor move, same as `DebugHover`.
    Hover,
    /// The Show Usages / Find Usages popup (a `textDocument/references`
    /// reply with more than one location), backed by `UiState::
    /// reference_list`; also reused for a `textDocument/definition` reply
    /// with more than one location (multi-def upgrade). Up/Down navigate,
    /// Enter jumps to the selected row, any other key dismisses-and-
    /// consumes (a flat list with no query, per context-menu.md's
    /// routing policy).
    References,
    /// The Code Actions popup (`textDocument/codeAction`), backed by
    /// `UiState::code_action_list`. Same key routing as `References`.
    CodeActions,
    /// The right-click / Shift+F10 context menu (context-menu.md), backed
    /// by `UiState::context_menu`. Up/Down navigate (skipping disabled
    /// rows), Enter activates, Escape dismisses; any other key — even a
    /// modified one — dismisses and consumes (never reaches the editor).
    ContextMenu,
}

/// Reading position shared by hover and completion documentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocumentationState {
    pub scroll: usize,
    pub expanded: bool,
}

/// State for a cursor-anchored popup (`ui.cursor_overlay`), distinct from
/// `active_modal`: it does not hard-capture keyboard input — a dedicated
/// pre-editor branch in `runtime/input.rs::handle_key` consumes only
/// Up/Down/Enter/Esc/Tab while one is open and passes every other key
/// through to the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorOverlayState {
    pub kind: CursorOverlayKind,
    pub selected: usize,
    pub scroll: usize,
    /// Pointer hover is independent of keyboard selection and shares this popup's lifetime.
    pub hover_row: Option<usize>,
    /// Independent wrapped-row viewport for the current documentation.
    pub documentation: DocumentationState,
}

impl CursorOverlayState {
    pub fn new(kind: CursorOverlayKind) -> Self {
        Self {
            kind,
            selected: 0,
            scroll: 0,
            hover_row: None,
            documentation: DocumentationState::default(),
        }
    }

    pub fn reset_documentation(&mut self) {
        self.documentation = DocumentationState::default();
    }
}

/// Content for the currently open real hover card (`ui.cursor_overlay` ==
/// `Some(CursorOverlayState { kind: CursorOverlayKind::Hover, .. })`).
/// Diagnostics aren't stored here — the card always reads
/// `diagnostics_at_position` live against the current cursor, which stays
/// correct because the card is dismissed on any cursor move.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HoverCardState {
    /// Styled `textDocument/hover` content (markdown reduced to text +
    /// spans), or `None` when the server returned no hover info at this
    /// position (the card can still be showing diagnostics-only content).
    pub content: Option<super::StyledText>,
    /// The hovered text cell this card is anchored to, for a mouse-dwell
    /// hover (`LspMsg::ShowHoverAt`). `None` for a keyboard-invoked hover
    /// (Shift+Cmd+D), which anchors to the caret rect instead — see
    /// `view::modal::with_cursor_overlay_spec`'s `Hover` branch.
    pub anchor: Option<(usize, usize)>,
}

/// The current `textDocument/signatureHelp` result (`ui.signature_help`),
/// already flattened to plaintext by `lsp::client::signature_help_state`.
/// Independent of `cursor_overlay` so it coexists with the completion
/// menu (menu below the caret, signature help above).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpState {
    pub signatures: Vec<SignatureView>,
    /// Index into `signatures`, already clamped.
    pub active: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureView {
    pub label: String,
    /// `[start, end)` char offsets into `label` of the active parameter.
    pub active_parameter_range: Option<(usize, usize)>,
    /// Styled doc of the signature itself (the function's docs), if sent.
    pub doc: Option<super::StyledText>,
    /// Styled doc of the active parameter, if the server sent one.
    pub parameter_doc: Option<super::StyledText>,
}

/// Content for the currently open references/multi-def popup (`ui.
/// cursor_overlay` == `Some(CursorOverlayState { kind: CursorOverlayKind::
/// References, .. })`) — set alongside `cursor_overlay`, cleared together.
/// This is the ordering authority for the popup: built once at resolve
/// time (sorted by `(path, line)`), stored here; both the view's spec
/// builder and Enter/click activation index this same `Vec`.
pub type ReferenceList = Vec<crate::update::navigation::LocationItem>;

/// One row of the Code Actions popup (`ui.code_action_list`): a
/// `CodeActionOrCommand` flattened so activation is a plain match on
/// `edit` / `command`. A bare `Command` reply has only `command` set.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeActionItem {
    pub title: String,
    pub kind: Option<String>,
    pub is_preferred: bool,
    pub edit: Option<Box<lsp_types::WorkspaceEdit>>,
    pub command: Option<lsp_types::Command>,
}

/// Content for the currently open context menu (`ui.cursor_overlay` ==
/// `Some(CursorOverlayState { kind: CursorOverlayKind::ContextMenu, .. })`)
/// — set alongside `cursor_overlay`, cleared together. `items` is the
/// ordering authority (built once at open time by the region builder in
/// `context_menu::builders`; view and Enter/click activation both index
/// this same `Vec`, addressing it by the "non-separator items in order"
/// scheme `context_menu::selectable_items` defines). `anchor` is the
/// `Anchor::Cursor` pixel rect the doc's Overlay Context section
/// describes — `(x, y, h)`, `h: 0` for a raw click point, the real caret
/// line height for the Shift+F10 trigger — captured at open time since,
/// unlike every other cursor-overlay kind, a context menu's anchor isn't
/// re-derivable from live model state (no live "current click position").
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub items: Vec<crate::context_menu::MenuItem>,
    pub anchor: (usize, usize, usize),
    /// Which region built this menu — automation-facing (`context_menu`
    /// snapshot) and testing convenience; the view/routing layer doesn't
    /// need it (both index `items` directly).
    pub region: crate::context_menu::ContextMenuRegion,
}

// ============================================================================
// Drop State (file drag-and-drop feedback)
// ============================================================================

/// State for file drag-and-drop visual feedback
#[derive(Debug, Clone, Default)]
pub struct DropState {
    /// Files currently being hovered over the window
    pub hovered_files: Vec<PathBuf>,
    /// Whether files are currently being dragged over the window
    pub is_hovering: bool,
}

impl DropState {
    /// Start hovering with a file
    pub fn start_hover(&mut self, path: PathBuf) {
        if !self.hovered_files.contains(&path) {
            self.hovered_files.push(path);
        }
        self.is_hovering = true;
    }

    /// Cancel the hover (user dragged away)
    pub fn cancel_hover(&mut self) {
        self.hovered_files.clear();
        self.is_hovering = false;
    }

    /// Get display text for the hover overlay
    pub fn display_text(&self) -> String {
        match self.hovered_files.len() {
            0 => String::new(),
            1 => {
                let filename = self.hovered_files[0]
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                format!("Drop to open: {}", filename)
            }
            n => format!("Drop to open {} files", n),
        }
    }
}

// ============================================================================
// Splitter Drag State
// ============================================================================

/// State for splitter (resize handle) dragging
#[derive(Debug, Clone)]
pub struct SplitterDragState {
    /// Index of the splitter being dragged (into the splitters vec from compute_layout)
    pub splitter_index: usize,
    /// Local index within the container (which children boundary)
    pub local_index: usize,
    /// Starting mouse position when drag began (pixels)
    pub start_position: (f32, f32),
    /// Original ratios before drag started (for cancel/restore)
    pub original_ratios: Vec<f32>,
    /// Direction of the split (determines which axis to track)
    pub direction: SplitDirection,
    /// Container's total size in the drag direction (pixels)
    pub container_size: f32,
    /// Whether threshold exceeded (true = actively dragging with visual updates)
    pub active: bool,
}

// ============================================================================
// Sidebar Resize State
// ============================================================================

/// State for sidebar resize dragging
#[derive(Debug, Clone)]
pub struct SidebarResizeState {
    /// Starting mouse X position when drag began
    pub start_x: f64,
    /// Original sidebar width (logical pixels) before drag started
    pub original_width: f32,
}

/// Which axis a dock resize drag applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockResizeAxis {
    Horizontal,
    Vertical,
}

/// State for dock resize dragging (right or bottom dock).
#[derive(Debug, Clone)]
pub struct DockResizeState {
    /// Which dock is being resized.
    pub position: crate::panel::DockPosition,
    /// The axis along which the drag operates.
    pub axis: DockResizeAxis,
    /// Starting mouse coordinate on the drag axis when drag began (x for
    /// horizontal/left-right, y for vertical/top-bottom).
    pub start_coord: f64,
    /// Original dock size (logical pixels) before drag started.
    pub original_size: f32,
}

/// Which axis a scrollbar drag applies to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarDragAxis {
    Vertical,
    Horizontal,
}

/// State for dragging an editor tab (reorder within a group, or move to
/// another group by dropping on its tab bar).
#[derive(Debug, Clone, Copy)]
pub struct TabDragState {
    /// The tab being dragged
    pub tab_id: crate::model::editor_area::TabId,
    /// Mouse position at press time (drag activates past a threshold)
    pub press: (f64, f64),
    /// Current mouse position (drives the drag ghost rendering)
    pub current: (f64, f64),
    /// Whether the drag threshold has been exceeded
    pub active: bool,
}

/// The surface whose viewport a scrollbar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarTarget {
    Editor(crate::model::editor_area::EditorId),
    Modal(ModalId),
    Documentation {
        kind: CursorOverlayKind,
        selected: usize,
    },
}

/// Shared capture state for editor, modal and documentation scrollbar thumbs.
#[derive(Debug, Clone)]
pub struct ScrollbarDragState {
    pub target: ScrollbarTarget,
    /// Whether dragging the vertical or horizontal scrollbar
    pub axis: ScrollbarDragAxis,
    /// Where within the thumb the user clicked (pixels from thumb origin)
    pub grab_offset: f32,
    /// Track origin on the drag axis (y for vertical, x for horizontal)
    pub track_start: f32,
    /// Track length on the drag axis
    pub track_size: f32,
    /// Thumb size on the drag axis
    pub thumb_size: f32,
    /// Maximum scroll position for this axis
    pub max_scroll: usize,
}

impl ScrollbarDragState {
    /// Compute the scroll position from the current mouse coordinate during drag.
    pub fn position_from_mouse(&self, mouse_coord: f32) -> usize {
        crate::view::scrollbar::position_from_drag(
            mouse_coord,
            self.grab_offset,
            self.track_start,
            self.track_size,
            self.thumb_size,
            self.max_scroll,
        )
    }
}

/// UI state for the outline panel
#[derive(Debug, Clone, Default)]
pub struct OutlinePanelState {
    /// Index of selected item in flattened visible list
    pub selected_index: Option<usize>,
    /// Scroll offset (in items)
    pub scroll_offset: usize,
    /// Collapsed node keys: (kind, range) for unique identification
    pub collapsed:
        std::collections::HashSet<(crate::outline::OutlineKind, crate::outline::OutlineRange)>,
}

impl OutlinePanelState {
    /// Get a stable key for a node (for tracking collapse state)
    pub fn node_key(
        node: &crate::outline::OutlineNode,
    ) -> (crate::outline::OutlineKind, crate::outline::OutlineRange) {
        (node.kind, node.range)
    }

    /// Check if a node is collapsed
    pub fn is_collapsed(&self, node: &crate::outline::OutlineNode) -> bool {
        self.collapsed.contains(&Self::node_key(node))
    }

    /// Toggle collapse state of a node
    pub fn toggle_collapsed(&mut self, node: &crate::outline::OutlineNode) {
        let key = Self::node_key(node);
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key);
        }
    }
}

/// UI state for the Problems panel (mirrors `OutlinePanelState`).
#[derive(Debug, Clone)]
pub struct ProblemsPanelState {
    /// Index into the flat `problems_rows()` list.
    pub selected_index: Option<usize>,
    /// Scroll offset (in rows).
    pub scroll_offset: usize,
    /// Collapsed file groups.
    pub collapsed: std::collections::HashSet<std::path::PathBuf>,
    /// Scope: only the focused document's diagnostics (default) or every
    /// file in `model.lsp.diagnostics`.
    pub current_file_only: bool,
}

impl Default for ProblemsPanelState {
    fn default() -> Self {
        Self {
            selected_index: None,
            scroll_offset: 0,
            collapsed: Default::default(),
            current_file_only: true,
        }
    }
}

/// UI state - status messages and cursor animation
#[derive(Debug, Clone)]
pub struct UiState {
    pub workspace_symbol_request: Option<crate::lsp::workspace_symbols::SymbolSearchRequest>,
    pub(crate) next_workspace_symbol_request: u64,
    /// The active bindings and pending chord state, shared by dispatch and hints.
    pub keymap: crate::keymap::Keymap,
    /// Structured status bar with segments
    pub status_bar: StatusBar,
    /// Transient message with auto-expiry
    pub transient_message: Option<TransientMessage>,
    /// Whether the cursor is currently visible (for blinking)
    pub cursor_visible: bool,
    /// Timestamp of last cursor blink state change
    pub last_cursor_blink: Instant,
    /// Whether a file is currently being loaded
    pub is_loading: bool,
    /// Whether a file is currently being saved
    pub is_saving: bool,
    /// Currently active modal (if any)
    pub active_modal: Option<ModalState>,
    /// Non-modal search UI for the active text pane; retained while editing.
    pub find_bar: Option<FindReplaceState>,
    /// Captured input field during a pointer selection.
    pub find_selection_drag: Option<FindReplaceField>,
    /// Last command palette state (persisted for quick re-execution)
    pub last_command_palette: Option<CommandPaletteState>,
    /// A Settings draft temporarily left to inspect the application log.
    pub(crate) suspended_settings: Option<crate::settings::SettingsState>,
    /// Last find/replace state (persisted for quick re-use)
    pub last_find_replace: Option<FindReplaceState>,
    /// File drag-and-drop state
    pub drop_state: DropState,
    /// Splitter (resize handle) drag state
    pub splitter_drag: Option<SplitterDragState>,
    /// Sidebar resize drag state
    pub sidebar_resize: Option<SidebarResizeState>,
    /// Dock resize drag state (right/bottom dock resize handle)
    pub dock_resize: Option<DockResizeState>,
    /// Scrollbar thumb drag state
    pub scrollbar_drag: Option<ScrollbarDragState>,
    /// Editor tab drag state (reorder within a group / move between groups)
    pub tab_drag: Option<TabDragState>,
    /// Which UI region has keyboard focus
    pub focus: FocusTarget,
    /// Which UI region the mouse is currently hovering over
    pub hover: HoverRegion,
    /// Lines that contained cursors in the previous frame (for damage tracking)
    /// Used by cursor blink to determine which lines need redrawing
    pub previous_cursor_lines: Vec<usize>,
    /// Whether the current `StatusMessage` segment text was set by
    /// `sync_status_bar`'s diagnostic-under-cursor fallback rather than an
    /// explicit flash/`UpdateSegment` — lets the fallback refresh/clear its
    /// own text each sync without clobbering an explicit message
    /// (lsp-integration.md Phase 2).
    pub status_message_is_diagnostic: bool,
    /// `FlatIndex` of the modal row currently under the mouse, if any
    /// (overlay-surface.md Pointer: hover wash). Cleared whenever the mouse
    /// isn't over a row.
    pub modal_hover_row: Option<usize>,
    /// Cursor-anchored popup (completion/hover/debug demo), if one is open.
    /// Distinct from `active_modal` — see `CursorOverlayState`.
    pub cursor_overlay: Option<CursorOverlayState>,
    /// Completion session, including invisible sessions waiting for LSP/syntax.
    /// Only nonempty results own a `Completion` cursor overlay.
    pub completion_menu: Option<crate::completion::CompletionMenuState>,
    pub(crate) completion_commit: Option<crate::completion::menu::PendingCommit>,
    pub(crate) completion_path: Option<std::sync::Arc<crate::completion::path::PathRequest>>,
    /// Ghost text at the cursor (autocomplete.md Phase 2); paint and
    /// accept check `applies_to` before trusting it.
    pub inline_suggestion: Option<crate::completion::inline::InlineSuggestionState>,
    pub inline_session: Option<crate::completion::provider::InlineSession>,
    /// A worker request is out for the focused document.
    pub inline_in_flight: bool,
    /// Consecutive backend failures; auto-trigger pauses at the cap.
    pub inline_failures: u32,
    pub inline_next_request_id: u64,
    /// Avoid repeating a persistence failure on every suggestion.
    pub inline_statistics_failed: bool,
    /// Hover-card content (lsp-integration.md Phase 4), set alongside
    /// `cursor_overlay` being `Some(CursorOverlayKind::Hover)`. `None`
    /// whenever the hover card is closed.
    pub hover_card: Option<HoverCardState>,
    /// The Show Usages / multi-def popup's rows (lsp-integration.md's
    /// references feature), set alongside `cursor_overlay` being
    /// `Some(CursorOverlayKind::References)`. `None` whenever the popup is
    /// closed.
    pub reference_list: Option<ReferenceList>,
    /// The Code Actions popup's rows, set alongside `cursor_overlay` being
    /// `Some(CursorOverlayKind::CodeActions)`; preferred actions first.
    pub code_action_list: Option<Vec<CodeActionItem>>,
    /// Document and revision captured when the action menu was populated.
    pub code_action_origin: Option<(super::DocumentId, u64)>,
    /// The context menu's built items + open-time anchor
    /// (context-menu.md), set alongside `cursor_overlay` being
    /// `Some(CursorOverlayKind::ContextMenu)`. `None` whenever the menu is
    /// closed.
    pub context_menu: Option<ContextMenuState>,
    /// Ownership survives while the card is visible; dismissal invalidates
    /// pending replies too, even when the caret did not move (e.g. Escape).
    pub hover_request: Option<super::hover::HoverRequest>,
    /// Signature help float (`textDocument/signatureHelp`), anchored above
    /// the caret. Not a `cursor_overlay` kind: it never routes keys and
    /// yields visually to the completion menu. Dismissed on Escape (when
    /// no `cursor_overlay` claims it), caret leaving the line, focus or
    /// document change.
    pub signature_help: Option<SignatureHelpState>,
}

impl UiState {
    /// The visible reading surface owns documentation controls, not the editor.
    pub fn has_documentation(&self) -> bool {
        if self.has_modal() || self.focus != FocusTarget::Editor {
            return false;
        }
        self.cursor_overlay
            .is_some_and(|overlay| match overlay.kind {
                CursorOverlayKind::Hover => true,
                CursorOverlayKind::Completion => {
                    self.has_visible_completion()
                        && self.completion_menu.as_ref().is_some_and(|menu| {
                            menu.selected_documentation(overlay.selected).is_some()
                        })
                }
                CursorOverlayKind::DebugCompletion
                | CursorOverlayKind::DebugHover
                | CursorOverlayKind::References
                | CursorOverlayKind::CodeActions
                | CursorOverlayKind::ContextMenu => false,
            })
    }

    pub fn has_hover(&self) -> bool {
        self.hover_request.is_some()
            || self.hover_card.is_some()
            || self.cursor_overlay.is_some_and(|overlay| {
                matches!(
                    overlay.kind,
                    CursorOverlayKind::Hover | CursorOverlayKind::DebugHover
                )
            })
    }

    /// Dismiss only hover documentation, never an unrelated cursor overlay.
    pub fn dismiss_hover(&mut self) -> bool {
        let mut changed = self.hover_request.take().is_some();
        changed |= self.hover_card.take().is_some();
        if self.cursor_overlay.is_some_and(|overlay| {
            matches!(
                overlay.kind,
                CursorOverlayKind::Hover | CursorOverlayKind::DebugHover
            )
        }) {
            self.cursor_overlay = None;
            changed = true;
        }
        changed
    }

    /// Pending completion requests have state but do not own keys or suppress
    /// inline suggestions until there are rows to display.
    pub fn has_visible_completion(&self) -> bool {
        self.completion_menu
            .as_ref()
            .is_some_and(|menu| !menu.filtered.is_empty())
            && self
                .cursor_overlay
                .is_some_and(|overlay| overlay.kind == CursorOverlayKind::Completion)
    }

    /// Create a new UI state with default settings
    pub fn new() -> Self {
        Self {
            status_bar: StatusBar::new(),
            workspace_symbol_request: None,
            next_workspace_symbol_request: 0,
            keymap: crate::keymap::Keymap::with_bindings(crate::keymap::default_bindings()),
            transient_message: None,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            is_loading: false,
            is_saving: false,
            active_modal: None,
            find_bar: None,
            find_selection_drag: None,
            last_command_palette: None,
            suspended_settings: None,
            last_find_replace: None,
            drop_state: DropState::default(),
            splitter_drag: None,
            sidebar_resize: None,
            dock_resize: None,
            scrollbar_drag: None,
            tab_drag: None,
            focus: FocusTarget::Editor,
            hover: HoverRegion::None,
            status_message_is_diagnostic: false,
            previous_cursor_lines: Vec::new(),
            modal_hover_row: None,
            cursor_overlay: None,
            completion_menu: None,
            completion_commit: None,
            completion_path: None,
            inline_suggestion: None,
            inline_session: None,
            inline_in_flight: false,
            inline_failures: 0,
            inline_next_request_id: 0,
            inline_statistics_failed: false,
            hover_card: None,
            reference_list: None,
            code_action_list: None,
            code_action_origin: None,
            context_menu: None,
            hover_request: None,
            signature_help: None,
        }
    }

    /// Create a UI state with an initial status message
    pub fn with_status(message: impl Into<String>) -> Self {
        let mut state = Self::new();
        state.set_status(message);
        state
    }

    // =========================================================================
    // Focus Management
    // =========================================================================

    /// Check if a modal is currently active
    pub fn has_modal(&self) -> bool {
        self.active_modal.is_some()
    }

    pub fn open_find(&mut self, state: FindReplaceState) {
        self.close_modal();
        self.find_bar = Some(state);
        self.focus = FocusTarget::FindBar;
        self.reset_cursor_blink();
    }

    /// Open a modal (also sets focus to Modal)
    pub fn open_modal(&mut self, state: ModalState) {
        self.find_selection_drag = None;
        self.scrollbar_drag = None;
        self.active_modal = Some(state);
        self.focus = FocusTarget::Modal;
        self.modal_hover_row = None;
    }

    /// Close the active modal (returns focus to Editor)
    pub fn close_modal(&mut self) {
        self.find_selection_drag = None;
        self.scrollbar_drag = None;
        self.active_modal = None;
        self.focus = FocusTarget::Editor;
        self.modal_hover_row = None;
    }

    /// Set focus to the editor
    pub fn focus_editor(&mut self) {
        self.find_selection_drag = None;
        if self.focus != FocusTarget::Editor {
            tracing::trace!("Focus changed: {:?} -> Editor", self.focus);
            self.focus = FocusTarget::Editor;
        }
    }

    /// Set focus to a modal (prefer using open_modal instead)
    pub fn focus_modal(&mut self) {
        if self.focus != FocusTarget::Modal {
            tracing::trace!("Focus changed: {:?} -> Modal", self.focus);
            self.focus = FocusTarget::Modal;
        }
    }

    /// Set focus to a dock
    pub fn focus_dock(&mut self, position: DockPosition) {
        self.find_selection_drag = None;
        let target = FocusTarget::Dock(position);
        if self.focus != target {
            tracing::trace!("Focus changed: {:?} -> Dock({:?})", self.focus, position);
            self.focus = target;
        }
    }

    /// Get the currently focused dock position, if any
    pub fn focused_dock(&self) -> Option<DockPosition> {
        match self.focus {
            FocusTarget::Dock(pos) => Some(pos),
            _ => None,
        }
    }

    /// Reset cursor blink timer (call after user input)
    pub fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_blink = Instant::now();
    }

    /// Update cursor blink state based on elapsed time
    /// Returns true if the state changed (needs redraw)
    pub fn update_cursor_blink(&mut self, blink_interval: Duration) -> bool {
        if blink_interval.is_zero() {
            return !std::mem::replace(&mut self.cursor_visible, true);
        }
        if self.last_cursor_blink.elapsed() >= blink_interval {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_blink = Instant::now();
            true
        } else {
            false
        }
    }

    /// Default lifetime of a status flash message.
    pub const DEFAULT_STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(4);

    /// Flash a status message for the default duration.
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.set_status_for(message, Self::DEFAULT_STATUS_MESSAGE_DURATION);
    }

    /// Flash a status message for a custom duration.
    pub fn set_status_for(&mut self, message: impl Into<String>, duration: Duration) {
        let message = message.into();
        self.transient_message = Some(TransientMessage::new(message.clone(), duration));
        self.status_message_is_diagnostic = false;
        self.status_bar
            .update_segment(SegmentId::StatusMessage, SegmentContent::Text(message));
    }

    /// Clear the status message if its lifetime has elapsed. Returns true if
    /// something was cleared (the status bar needs a redraw).
    pub fn expire_status_message(&mut self) -> bool {
        if self
            .transient_message
            .as_ref()
            .is_some_and(|t| t.is_expired())
        {
            self.transient_message = None;
            self.status_message_is_diagnostic = false;
            self.status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Empty);
            true
        } else {
            false
        }
    }

    /// Check if the UI is busy (loading or saving)
    pub fn is_busy(&self) -> bool {
        self.is_loading || self.is_saving
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recent_files::{RecentEntry, RecentFiles};

    #[test]
    fn find_async_debug_omits_snapshot_text_and_match_payloads() {
        let document = crate::model::Document::with_text(&"private-needle\n".repeat(30_000));
        let mut state = FindReplaceState::default();
        state.set_query("private-needle");
        let request = state.prepare_search(&document).unwrap();
        let message = crate::messages::UiMsg::FindSearchCompleted {
            result: Ok(request.compute()),
            request,
        };
        let debug = format!("{message:?}");
        assert!(!debug.contains("private-needle"));
        assert!(debug.len() < crate::util::ByteSize::kibibytes(1).as_usize());
    }

    #[test]
    fn find_async_snapshot_rejects_a_different_document_with_shared_rope_and_revision() {
        let mut document = crate::model::Document::with_text(&"foo\n".repeat(70_000));
        document.id = Some(crate::model::editor_area::DocumentId(1));
        let mut state = FindReplaceState::default();
        state.set_query("foo");
        let request = state.prepare_search(&document).unwrap();
        assert!(state.display_results(&document).is_none());
        let mut other = document.clone();
        other.id = Some(crate::model::editor_area::DocumentId(2));
        assert!(other.buffer.is_instance(&document.buffer));
        assert!(!state.finish_search(&other, Arc::clone(&request), Ok(request.compute())));
        assert!(state.display_results(&other).is_none());
        assert!(state.finish_search(&document, Arc::clone(&request), Ok(request.compute())));
        assert_eq!(
            state.display_results(&document).unwrap().matches.len(),
            70_000
        );
        assert!(state.display_results(&other).is_none());
    }

    #[test]
    fn find_async_worker_results_have_overview_ready_but_explicit_matches_keep_it_lazy() {
        let document = crate::model::Document::with_text(&"foo\n".repeat(70_000));
        let mut state = FindReplaceState::default();
        state.set_query("foo");
        let request = state.prepare_search(&document).unwrap();
        let results = request.compute();
        assert_eq!(results.lines.get().unwrap().len(), 70_000);
        assert!(state.finish_search(&document, request, Ok(results)));
        let mut explicit = FindReplaceState::default();
        explicit.set_query("foo");
        assert_eq!(explicit.matches(&document).len(), 70_000);
        assert!(explicit.results(&document).lines.get().is_none());
    }

    fn make_entry(path: &str, workspace: Option<&str>) -> RecentEntry {
        RecentEntry {
            path: PathBuf::from(path),
            opened_at: 0,
            workspace: workspace.map(PathBuf::from),
            open_count: 1,
            pinned: false,
        }
    }

    fn make_recent(entries: Vec<RecentEntry>) -> RecentFiles {
        RecentFiles {
            version: 1,
            entries,
        }
    }

    #[test]
    fn test_recent_files_state_includes_current_file() {
        let recent = make_recent(vec![
            make_entry("/a.rs", None),
            make_entry("/b.rs", None),
            make_entry("/c.rs", None),
        ]);
        let state = RecentFilesState::new(&recent, Some(std::path::Path::new("/a.rs")));
        assert_eq!(state.entries.len(), 3);
        assert_eq!(state.entries[0].path, PathBuf::from("/a.rs"));
        assert_eq!(state.entries[1].path, PathBuf::from("/b.rs"));
        assert_eq!(state.entries[2].path, PathBuf::from("/c.rs"));
    }

    #[test]
    fn test_recent_files_state_preselects_previous_file_when_current_is_first() {
        let recent = make_recent(vec![
            make_entry("/a.rs", None),
            make_entry("/b.rs", None),
            make_entry("/c.rs", None),
        ]);
        let state = RecentFilesState::new(&recent, Some(std::path::Path::new("/a.rs")));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_recent_files_state_keeps_default_selection_when_current_is_only_entry() {
        let recent = make_recent(vec![make_entry("/a.rs", None)]);
        let state = RecentFilesState::new(&recent, Some(std::path::Path::new("/a.rs")));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_keeps_default_selection_when_current_not_first() {
        let recent = make_recent(vec![
            make_entry("/b.rs", None),
            make_entry("/a.rs", None),
            make_entry("/c.rs", None),
        ]);
        let state = RecentFilesState::new(&recent, Some(std::path::Path::new("/a.rs")));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_no_current_file() {
        let recent = make_recent(vec![make_entry("/a.rs", None), make_entry("/b.rs", None)]);
        let state = RecentFilesState::new(&recent, None);
        assert_eq!(state.entries.len(), 2);
    }

    #[test]
    fn test_recent_files_state_filter() {
        let recent = make_recent(vec![
            make_entry("/project/src/main.rs", Some("/project")),
            make_entry("/project/Cargo.toml", Some("/project")),
            make_entry("/other/README.md", None),
        ]);
        let mut state = RecentFilesState::new(&recent, None);

        // No filter — all entries
        state.recompute_filtered_rows();
        assert_eq!(state.filtered_rows.len(), 3);

        // Filter by "main"
        state.editable.set_content("main");
        state.recompute_filtered_rows();
        assert_eq!(state.filtered_rows.len(), 1);
        assert_eq!(
            state.entries[state.filtered_rows[0]].path,
            PathBuf::from("/project/src/main.rs")
        );
    }

    #[test]
    fn test_recent_files_state_filter_case_insensitive() {
        let recent = make_recent(vec![make_entry("/project/src/Main.rs", Some("/project"))]);
        let mut state = RecentFilesState::new(&recent, None);
        state.editable.set_content("main");
        state.recompute_filtered_rows();
        assert_eq!(state.filtered_rows.len(), 1);
    }

    #[test]
    fn test_recent_files_state_empty() {
        let recent = RecentFiles::default();
        let state = RecentFilesState::new(&recent, None);
        assert!(state.entries.is_empty());
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_initial_selection() {
        let recent = make_recent(vec![make_entry("/a.rs", None), make_entry("/b.rs", None)]);
        let state = RecentFilesState::new(&recent, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recompute_filtered_rows_puts_pinned_first_preserving_mru_order() {
        let mut a = make_entry("/a.rs", None);
        a.pinned = true;
        let mut c = make_entry("/c.rs", None);
        c.pinned = true;
        let recent = make_recent(vec![
            make_entry("/z.rs", None), // unpinned, MRU-first
            a,                         // pinned
            make_entry("/y.rs", None), // unpinned
            c,                         // pinned
        ]);
        let mut state = RecentFilesState::new(&recent, None);
        state.recompute_filtered_rows();

        // Pinned entries (a, c) sort before unpinned (z, y), each group
        // keeping its original MRU-relative order — not re-sorted by name.
        let ordered_paths: Vec<_> = state
            .filtered_rows
            .iter()
            .map(|&i| state.entries[i].path.clone())
            .collect();
        assert_eq!(
            ordered_paths,
            vec![
                PathBuf::from("/a.rs"),
                PathBuf::from("/c.rs"),
                PathBuf::from("/z.rs"),
                PathBuf::from("/y.rs"),
            ]
        );
    }

    #[test]
    fn test_recompute_filtered_rows_respects_query_filter() {
        let recent = make_recent(vec![
            make_entry("/project/main.rs", Some("/project")),
            make_entry("/project/lib.rs", Some("/project")),
        ]);
        let mut state = RecentFilesState::new(&recent, None);
        state.editable.set_content("main");
        state.recompute_filtered_rows();

        assert_eq!(state.filtered_rows.len(), 1);
        assert_eq!(
            state.entries[state.filtered_rows[0]].path,
            PathBuf::from("/project/main.rs")
        );
    }

    #[test]
    fn set_status_creates_an_expiring_transient() {
        let mut ui = UiState::new();
        ui.set_status("Configuration reloaded");
        let transient = ui
            .transient_message
            .as_ref()
            .expect("set_status must arm expiry");
        assert!(!transient.is_expired());
        assert_eq!(transient.text, "Configuration reloaded");
    }

    #[test]
    fn expire_status_message_clears_segment_after_lifetime() {
        let mut ui = UiState::new();
        ui.set_status_for("gone soon", Duration::ZERO);
        assert!(ui.expire_status_message());
        assert!(ui.transient_message.is_none());
        let segment = ui
            .status_bar
            .get_segment(SegmentId::StatusMessage)
            .expect("StatusMessage segment exists");
        assert!(segment.content.is_empty());
    }

    #[test]
    fn expire_status_message_is_a_noop_before_lifetime() {
        let mut ui = UiState::new();
        ui.set_status("still here");
        assert!(!ui.expire_status_message());
        assert!(ui.transient_message.is_some());
    }

    #[test]
    fn find_overview_lines_are_lazy_and_use_the_result_snapshot() {
        let mut document = crate::model::Document::new();
        document.buffer = ropey::Rope::from_str("cat\ncat\n");
        let mut state = FindReplaceState::default();
        state.set_query("cat");
        let results = state.results(&document);
        assert_eq!(state.matches(&document).len(), 2);
        assert!(results.lines.get().is_none());
        document.buffer = ropey::Rope::from_str("cat cat");
        assert_eq!(results.lines(), &[0, 1]);
        assert_eq!(state.results(&document).lines(), &[0]);
    }

    #[test]
    fn find_overview_lines_match_rope_coordinates_for_dense_sparse_and_eof_matches() {
        // Cross many chunk boundaries, including CRLF and Unicode line endings.
        let chunked = "猫猫\r\n猫\r猫\n猫\u{0085}猫\u{2028}猫\u{2029}".repeat(2_000);
        let long_line = "猫".repeat(10_000);
        for text in [
            "",
            "猫猫\r\n猫\r猫\n",
            "a\u{0085}猫\u{2028}b\u{2029}猫\n",
            "猫\n",
            "猫",
            chunked.as_str(),
            long_line.as_str(),
        ] {
            let mut document = crate::model::Document::new();
            document.buffer = ropey::Rope::from_str(text);
            for pattern in ["猫", ".", "(?s).", "(?m)^", "$", "(?s).*"] {
                let mut state = FindReplaceState {
                    use_regex: true,
                    ..Default::default()
                };
                state.set_query(pattern);
                let results = state.results(&document);
                let mut expected: Vec<_> = results
                    .matches
                    .iter()
                    .map(|m| document.buffer.char_to_line(m.start))
                    .collect();
                expected.dedup();
                assert_eq!(
                    results.lines(),
                    expected,
                    "text={text:?}, pattern={pattern}"
                );
            }
        }
        let mut document = crate::model::Document::new();
        document.buffer = ropey::Rope::from_str(&format!("cat\n{}cat\n", "plain\n".repeat(10_000)));
        let mut state = FindReplaceState::default();
        state.set_query("cat");
        assert_eq!(state.results(&document).lines(), &[0, 10_001]);
    }

    #[test]
    fn theme_picker_swatches_parallel_the_theme_list() {
        let state = ThemePickerState::new("default-dark".to_string());
        assert_eq!(
            state.swatches.len(),
            state.themes.len(),
            "every listed theme needs a swatch (fallback on load failure)"
        );
        // Bundled themes all load, so at least one swatch must differ from
        // the neutral fallback.
        let fallback = crate::theme::ThemeSwatch::fallback();
        assert!(
            state.swatches.iter().any(|s| s.accent != fallback.accent),
            "bundled themes should produce real accent colors"
        );
    }
}
