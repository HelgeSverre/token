//! Application model - the complete state of the editor
//!
//! This module contains all the state types following the Elm Architecture pattern.

pub mod document;
pub mod editor;
pub mod editor_area;
pub mod status_bar;
pub mod ui;
pub mod workspace;

pub use document::{Document, EditOperation};
pub use editor::{
    Cursor, EditorState, OccurrenceState, Position, RectangleSelectionState, ScrollRevealMode,
    Selection, ViewMode, Viewport,
};
pub use editor_area::{
    DocumentId, EditorArea, EditorGroup, EditorId, GroupId, LayoutNode, Rect, SplitContainer,
    SplitDirection, SplitterBar, Tab, TabId, SPLITTER_WIDTH,
};
pub use status_bar::{
    sync_status_bar, RenderedSegment, SegmentContent, SegmentId, SegmentPosition, StatusBar,
    StatusBarLayout, StatusSegment, TransientMessage,
};
pub use ui::{
    CommandPaletteState, DropState, FindReplaceState, GotoLineState, ModalId, ModalState,
    ThemePickerState, UiState,
};
pub use workspace::{FileExtension, FileNode, FileTree, Workspace};

use crate::config::EditorConfig;
use crate::config_paths;
#[cfg(debug_assertions)]
use crate::debug_overlay::DebugOverlay;
use crate::theme::{load_theme, Theme};
use crate::util::{is_likely_binary, validate_file_for_opening};
use std::path::PathBuf;

// ============================================================================
// Viewport Geometry - pure calculations for dimensions
// ============================================================================

/// Viewport geometry calculations (pure, no I/O)
///
/// Encapsulates window dimension calculations for editor viewports.
/// Initial values are estimates that get corrected by the renderer
/// once actual font metrics are available.
#[derive(Debug, Clone, Copy)]
pub struct ViewportGeometry {
    pub window_width: u32,
    pub window_height: u32,
    pub line_height: usize,
    pub char_width: f32,
    pub visible_lines: usize,
    pub visible_columns: usize,
}

impl ViewportGeometry {
    /// Default line height in pixels
    pub const DEFAULT_LINE_HEIGHT: usize = 20;
    /// Default character width (corrected by renderer with actual font metrics)
    pub const DEFAULT_CHAR_WIDTH: f32 = 10.0;

    pub fn new(window_width: u32, window_height: u32) -> Self {
        let line_height = Self::DEFAULT_LINE_HEIGHT;
        let char_width = Self::DEFAULT_CHAR_WIDTH;

        let visible_columns = Self::compute_visible_columns(window_width, char_width);
        let visible_lines = Self::compute_visible_lines(window_height, line_height, line_height);

        Self {
            window_width,
            window_height,
            line_height,
            char_width,
            visible_lines,
            visible_columns,
        }
    }

    /// Compute number of visible text lines given window height.
    ///
    /// This is the canonical calculation used across the codebase.
    #[inline]
    pub fn compute_visible_lines(
        window_height: u32,
        line_height: usize,
        status_bar_height: usize,
    ) -> usize {
        if line_height == 0 {
            return 25; // fallback
        }
        (window_height as usize).saturating_sub(status_bar_height) / line_height
    }

    /// Compute number of visible columns given window width.
    ///
    /// Uses `text_start_x()` for accurate gutter width calculation.
    /// This is the canonical calculation used across the codebase.
    #[inline]
    pub fn compute_visible_columns(window_width: u32, char_width: f32) -> usize {
        if char_width <= 0.0 {
            return 80; // fallback
        }
        let text_x = text_start_x(char_width).round();
        ((window_width as f32 - text_x) / char_width).floor() as usize
    }
}

// ============================================================================
// Session Initialization - file loading and editor setup
// ============================================================================

/// Result of initial session creation
pub struct InitialSession {
    pub editor_area: EditorArea,
    pub status_message: String,
}

/// Load configuration and theme from disk
fn load_config_and_theme() -> (EditorConfig, Theme) {
    config_paths::ensure_all_config_dirs();
    let config = EditorConfig::load();
    let theme = load_theme(&config.theme).unwrap_or_else(|e| {
        tracing::warn!(
            "Failed to load theme '{}': {}, using default",
            config.theme,
            e
        );
        Theme::default()
    });
    (config, theme)
}

/// Create initial session with documents and editor area
fn create_initial_session(file_paths: Vec<PathBuf>, geom: &ViewportGeometry) -> InitialSession {
    // Load first file or create empty document
    let (first_document, status_message) = if let Some(first_path) = file_paths.first() {
        // Validate and load the first file
        if let Err(e) = validate_file_for_opening(first_path) {
            let msg = e.user_message(&first_path.display().to_string());
            (Document::new(), msg)
        } else if is_likely_binary(first_path) {
            let msg = format!("Cannot open binary file: {}", first_path.display());
            (Document::new(), msg)
        } else {
            match Document::from_file(first_path.clone()) {
                Ok(doc) => {
                    let msg = if file_paths.len() > 1 {
                        format!("Opened {} files", file_paths.len())
                    } else {
                        format!("Loaded: {}", first_path.display())
                    };
                    (doc, msg)
                }
                Err(e) => {
                    let msg = format!("Error loading {}: {}", first_path.display(), e);
                    (Document::new(), msg)
                }
            }
        }
    } else {
        (Document::new(), "New file".to_string())
    };

    // Create editor state with viewport
    let editor = EditorState::with_viewport(geom.visible_lines, geom.visible_columns);

    // Create editor area with first document
    let mut editor_area = EditorArea::single_document(first_document, editor);

    // Open additional files as tabs
    for path in file_paths.into_iter().skip(1) {
        // Validate before attempting to open
        if let Err(e) = validate_file_for_opening(&path) {
            tracing::warn!("Skipping {}: {}", path.display(), e);
            continue;
        }
        if is_likely_binary(&path) {
            tracing::warn!("Skipping binary file: {}", path.display());
            continue;
        }

        let doc_id = editor_area.next_document_id();
        match Document::from_file(path.clone()) {
            Ok(mut doc) => {
                doc.id = Some(doc_id);
                editor_area.documents.insert(doc_id, doc);

                // Create editor for this document
                let editor_id = editor_area.next_editor_id();
                let mut editor =
                    EditorState::with_viewport(geom.visible_lines, geom.visible_columns);
                editor.id = Some(editor_id);
                editor.document_id = Some(doc_id);
                editor_area.editors.insert(editor_id, editor);

                // Create tab in focused group
                let tab_id = editor_area.next_tab_id();
                let tab = Tab {
                    id: tab_id,
                    editor_id,
                    is_pinned: false,
                    is_preview: false,
                };

                if let Some(group) = editor_area.groups.get_mut(&editor_area.focused_group_id) {
                    group.tabs.push(tab);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to open {}: {}", path.display(), e);
            }
        }
    }

    InitialSession {
        editor_area,
        status_message,
    }
}

/// Layout constant - width of line number gutter in characters (e.g., " 123 ")
pub const LINE_NUMBER_GUTTER_CHARS: usize = 5;

// ============================================================================
// Scaled Metrics - UI layout constants scaled for display DPI
// ============================================================================

/// Layout metrics scaled for the current display's scale factor.
///
/// All values are in physical pixels, computed from base values at scale factor 1.0.
/// This ensures UI elements (tab bar, splitters, padding) scale correctly on HiDPI displays.
#[derive(Debug, Clone, Copy)]
pub struct ScaledMetrics {
    /// Display scale factor (1.0 for standard, 2.0 for retina)
    pub scale_factor: f64,
    /// Tab bar height in physical pixels
    pub tab_bar_height: usize,
    /// Splitter width in physical pixels
    pub splitter_width: f32,
    /// Gutter padding (after line numbers, before border) in physical pixels
    pub gutter_padding: f32,
    /// Text area padding (after border, before content) in physical pixels
    pub text_area_padding: f32,
    /// Small UI padding in physical pixels (e.g., tab gap)
    pub padding_small: usize,
    /// Medium UI padding in physical pixels (e.g., tab content padding)
    pub padding_medium: usize,
    /// Large UI padding in physical pixels (e.g., text padding inside tabs)
    pub padding_large: usize,
    /// Border width in physical pixels
    pub border_width: usize,

    // === File Tree / Sidebar Metrics ===
    /// File tree row height in physical pixels
    pub file_tree_row_height: usize,
    /// File tree indent per nesting level in physical pixels
    pub file_tree_indent: f32,
    /// Default sidebar width in logical pixels (scale-independent)
    pub sidebar_default_width_logical: f32,
    /// Minimum sidebar width in logical pixels
    pub sidebar_min_width_logical: f32,
    /// Maximum sidebar width in logical pixels
    pub sidebar_max_width_logical: f32,
    /// Resize handle hit zone in physical pixels
    pub resize_handle_zone: usize,
}

impl ScaledMetrics {
    /// Base tab bar height at scale factor 1.0
    const BASE_TAB_BAR_HEIGHT: f64 = 28.0;
    /// Base splitter width at scale factor 1.0
    const BASE_SPLITTER_WIDTH: f64 = 6.0;
    /// Base gutter padding at scale factor 1.0
    const BASE_GUTTER_PADDING: f64 = 4.0;
    /// Base text area padding at scale factor 1.0
    const BASE_TEXT_AREA_PADDING: f64 = 8.0;
    /// Base small padding at scale factor 1.0
    const BASE_PADDING_SMALL: f64 = 2.0;
    /// Base medium padding at scale factor 1.0
    const BASE_PADDING_MEDIUM: f64 = 4.0;
    /// Base large padding at scale factor 1.0
    const BASE_PADDING_LARGE: f64 = 8.0;
    /// Base border width at scale factor 1.0
    const BASE_BORDER_WIDTH: f64 = 1.0;

    // === File Tree / Sidebar Base Values ===
    /// Base file tree row height at scale factor 1.0
    const BASE_FILE_TREE_ROW_HEIGHT: f64 = 22.0;
    /// Base file tree indent at scale factor 1.0
    const BASE_FILE_TREE_INDENT: f64 = 16.0;
    /// Default sidebar width in logical pixels (not scaled)
    const BASE_SIDEBAR_DEFAULT_WIDTH: f32 = 250.0;
    /// Minimum sidebar width in logical pixels (not scaled)
    const BASE_SIDEBAR_MIN_WIDTH: f32 = 150.0;
    /// Maximum sidebar width in logical pixels (not scaled)
    const BASE_SIDEBAR_MAX_WIDTH: f32 = 500.0;
    /// Base resize handle zone at scale factor 1.0
    const BASE_RESIZE_HANDLE_ZONE: f64 = 4.0;

    /// Create scaled metrics for the given display scale factor
    pub fn new(scale_factor: f64) -> Self {
        Self {
            scale_factor,
            tab_bar_height: (Self::BASE_TAB_BAR_HEIGHT * scale_factor).round() as usize,
            splitter_width: (Self::BASE_SPLITTER_WIDTH * scale_factor) as f32,
            gutter_padding: (Self::BASE_GUTTER_PADDING * scale_factor) as f32,
            text_area_padding: (Self::BASE_TEXT_AREA_PADDING * scale_factor) as f32,
            padding_small: (Self::BASE_PADDING_SMALL * scale_factor).round() as usize,
            padding_medium: (Self::BASE_PADDING_MEDIUM * scale_factor).round() as usize,
            padding_large: (Self::BASE_PADDING_LARGE * scale_factor).round() as usize,
            border_width: (Self::BASE_BORDER_WIDTH * scale_factor).round().max(1.0) as usize,
            // File tree metrics
            file_tree_row_height: (Self::BASE_FILE_TREE_ROW_HEIGHT * scale_factor).round() as usize,
            file_tree_indent: (Self::BASE_FILE_TREE_INDENT * scale_factor) as f32,
            sidebar_default_width_logical: Self::BASE_SIDEBAR_DEFAULT_WIDTH,
            sidebar_min_width_logical: Self::BASE_SIDEBAR_MIN_WIDTH,
            sidebar_max_width_logical: Self::BASE_SIDEBAR_MAX_WIDTH,
            resize_handle_zone: (Self::BASE_RESIZE_HANDLE_ZONE * scale_factor)
                .round()
                .max(2.0) as usize,
        }
    }
}

impl Default for ScaledMetrics {
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// Calculate the x-coordinate where text content begins (with metrics)
#[inline]
pub fn text_start_x_scaled(char_width: f32, metrics: &ScaledMetrics) -> f32 {
    let gutter_width = char_width * LINE_NUMBER_GUTTER_CHARS as f32 + metrics.gutter_padding;
    let border_width = metrics.border_width as f32;
    gutter_width + border_width + metrics.text_area_padding
}

/// Calculate the x-coordinate of the gutter border (with metrics)
#[inline]
pub fn gutter_border_x_scaled(char_width: f32, metrics: &ScaledMetrics) -> f32 {
    char_width * LINE_NUMBER_GUTTER_CHARS as f32 + metrics.gutter_padding
}

/// Calculate the x-coordinate where text content begins (legacy, uses scale factor 1.0)
#[inline]
pub fn text_start_x(char_width: f32) -> f32 {
    text_start_x_scaled(char_width, &ScaledMetrics::default())
}

/// Calculate the x-coordinate of the gutter border (legacy, uses scale factor 1.0)
#[inline]
pub fn gutter_border_x(char_width: f32) -> f32 {
    gutter_border_x_scaled(char_width, &ScaledMetrics::default())
}

/// The complete application model
#[derive(Debug)]
pub struct AppModel {
    /// Editor area containing all documents, editors, groups, and layout
    pub editor_area: EditorArea,
    /// UI state (status bar, cursor blink)
    pub ui: UiState,
    /// Theme for colors and styling
    pub theme: Theme,
    /// Persisted editor configuration
    pub config: EditorConfig,
    /// Window dimensions
    pub window_size: (u32, u32),
    /// Line height in pixels
    pub line_height: usize,
    /// Character width in pixels (monospace)
    pub char_width: f32,
    /// Scaled UI metrics for HiDPI support
    pub metrics: ScaledMetrics,
    /// Workspace with file tree sidebar (None if no directory opened)
    pub workspace: Option<Workspace>,
    /// Debug overlay state (debug builds only)
    #[cfg(debug_assertions)]
    pub debug_overlay: Option<DebugOverlay>,
}

impl AppModel {
    /// Create a new application model with the given window size and scale factor
    pub fn new(
        window_width: u32,
        window_height: u32,
        scale_factor: f64,
        file_paths: Vec<PathBuf>,
    ) -> Self {
        // Calculate viewport geometry
        let geom = ViewportGeometry::new(window_width, window_height);

        // Create scaled metrics for HiDPI support
        let metrics = ScaledMetrics::new(scale_factor);

        // Load config and theme
        let (config, theme) = load_config_and_theme();

        // Create initial session with documents
        let InitialSession {
            editor_area,
            status_message,
        } = create_initial_session(file_paths, &geom);

        Self {
            editor_area,
            ui: UiState::with_status(status_message),
            theme,
            config,
            window_size: (window_width, window_height),
            line_height: geom.line_height,
            char_width: geom.char_width,
            metrics,
            workspace: None,
            #[cfg(debug_assertions)]
            debug_overlay: Some(DebugOverlay::new()),
        }
    }

    /// Open a directory as workspace
    pub fn open_workspace(&mut self, root: PathBuf) {
        match Workspace::new(root.clone(), &self.metrics) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.ui
                    .set_status(format!("Opened workspace: {}", root.display()));
            }
            Err(e) => {
                self.ui
                    .set_status(format!("Failed to open workspace: {}", e));
            }
        }
    }

    /// Close the current workspace
    pub fn close_workspace(&mut self) {
        self.workspace = None;
    }

    /// Get workspace root directory (convenience accessor)
    pub fn workspace_root(&self) -> Option<&PathBuf> {
        self.workspace.as_ref().map(|ws| &ws.root)
    }

    /// Update scale factor and recalculate metrics
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.metrics = ScaledMetrics::new(scale_factor);
    }

    /// Recompute tab bar height based on current font metrics.
    ///
    /// Formula: glyph line height + vertical padding * 2.
    /// This ensures tab bar height scales correctly with font size and DPI.
    pub fn recompute_tab_bar_height_from_line_height(&mut self) {
        if self.line_height == 0 {
            return;
        }

        let glyph_height = self.line_height;
        let padding = self.metrics.padding_medium;

        self.metrics.tab_bar_height = glyph_height + padding * 2;
    }

    /// Get the focused editor (read-only), or None if no editor is focused
    /// Used for debug instrumentation
    pub fn focused_editor(&self) -> Option<&EditorState> {
        self.editor_area.focused_editor()
    }

    // =========================================================================
    // Accessor methods for backward compatibility
    // These delegate to editor_area's focused document/editor
    // =========================================================================

    /// Get the focused document (read-only)
    #[inline]
    pub fn document(&self) -> &Document {
        self.editor_area
            .focused_document()
            .expect("EditorArea must have at least one document")
    }

    /// Get the focused document (mutable)
    #[inline]
    pub fn document_mut(&mut self) -> &mut Document {
        self.editor_area
            .focused_document_mut()
            .expect("EditorArea must have at least one document")
    }

    /// Get the focused editor state (read-only)
    #[inline]
    pub fn editor(&self) -> &EditorState {
        self.editor_area
            .focused_editor()
            .expect("EditorArea must have at least one editor")
    }

    /// Get the focused editor state (mutable)
    #[inline]
    pub fn editor_mut(&mut self) -> &mut EditorState {
        self.editor_area
            .focused_editor_mut()
            .expect("EditorArea must have at least one editor")
    }

    /// Update viewport dimensions after window resize
    /// Updates ALL editors, not just the focused one (for split view support)
    pub fn resize(&mut self, width: u32, height: u32) {
        self.window_size = (width, height);

        let text_x = text_start_x_scaled(self.char_width, &self.metrics).round();
        let visible_columns = ((width as f32 - text_x) / self.char_width).floor() as usize;
        // Subtract status bar height (1 line) and tab bar height from available height
        let status_bar_height = self.line_height;
        let tab_bar_height = self.metrics.tab_bar_height;
        let available_height = (height as usize)
            .saturating_sub(status_bar_height)
            .saturating_sub(tab_bar_height);
        let visible_lines = if self.line_height > 0 {
            available_height / self.line_height
        } else {
            0
        };

        // FIX: Update ALL editors, not just the focused one
        for editor in self.editor_area.editors.values_mut() {
            editor.resize_viewport(visible_lines, visible_columns);
        }
    }

    /// Update char_width from actual font metrics
    /// Updates ALL editors for split view support
    pub fn set_char_width(&mut self, char_width: f32) {
        self.char_width = char_width;

        // Recalculate visible columns with new char width using scaled metrics
        let text_x = text_start_x_scaled(char_width, &self.metrics).round();
        let visible_columns = ((self.window_size.0 as f32 - text_x) / char_width).floor() as usize;

        // FIX: Update ALL editors, not just the focused one
        for editor in self.editor_area.editors.values_mut() {
            editor.viewport.visible_columns = visible_columns;
        }
    }

    // Convenience methods that delegate to sub-models

    /// Get the buffer offset for the current cursor position
    pub fn cursor_buffer_position(&self) -> usize {
        self.editor().cursor_offset(self.document())
    }

    /// Set cursor position from buffer offset (clears selection)
    pub fn set_cursor_from_position(&mut self, pos: usize) {
        let doc = self
            .editor_area
            .focused_document()
            .expect("must have document");
        let (line, column) = doc.offset_to_cursor(pos);
        let editor = self.editor_mut();
        editor.cursors[0].line = line;
        editor.cursors[0].column = column;
        editor.cursors[0].desired_column = None;
        editor.clear_selection();
    }

    /// Move cursor to buffer offset without clearing selection
    pub fn move_cursor_to_position(&mut self, pos: usize) {
        let doc = self
            .editor_area
            .focused_document()
            .expect("must have document");
        let (line, column) = doc.offset_to_cursor(pos);
        let editor = self.editor_mut();
        editor.cursors[0].line = line;
        editor.cursors[0].column = column;
        editor.cursors[0].desired_column = None;
    }

    /// Get the current line length
    pub fn current_line_length(&self) -> usize {
        self.editor().current_line_length(self.document())
    }

    /// Ensure cursor is visible in viewport (minimal scroll)
    /// Uses EditorArea helper to avoid cloning the document
    pub fn ensure_cursor_visible(&mut self) {
        self.editor_area
            .ensure_focused_cursor_visible(ScrollRevealMode::Minimal);
    }

    /// Ensure cursor is visible with direction-aware alignment
    ///
    /// When moving up, cursor is revealed at the top of the safe zone.
    /// When moving down, cursor is revealed at the bottom of the safe zone.
    /// For horizontal movement or no hint, uses minimal scroll.
    /// Uses EditorArea helper to avoid cloning the document
    pub fn ensure_cursor_visible_directional(&mut self, vertical_up: Option<bool>) {
        let mode = match vertical_up {
            Some(true) => ScrollRevealMode::TopAligned,
            Some(false) => ScrollRevealMode::BottomAligned,
            None => ScrollRevealMode::Minimal,
        };
        self.editor_area.ensure_focused_cursor_visible(mode);
    }

    /// Reset cursor blink timer
    pub fn reset_cursor_blink(&mut self) {
        self.ui.reset_cursor_blink();
    }

    /// Get a line from the document
    pub fn get_line(&self, line_idx: usize) -> Option<String> {
        self.document().get_line(line_idx)
    }

    /// Get line length
    pub fn line_length(&self, line_idx: usize) -> usize {
        self.document().line_length(line_idx)
    }

    /// Get first non-whitespace column on current line
    pub fn first_non_whitespace_column(&self) -> usize {
        self.document()
            .first_non_whitespace_column(self.editor().active_cursor().line)
    }

    /// Get last non-whitespace column on current line
    pub fn last_non_whitespace_column(&self) -> usize {
        self.document()
            .last_non_whitespace_column(self.editor().active_cursor().line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaled_metrics_standard() {
        let metrics = ScaledMetrics::new(1.0);
        assert_eq!(metrics.tab_bar_height, 28);
        assert_eq!(metrics.splitter_width, 6.0);
        assert_eq!(metrics.gutter_padding, 4.0);
        assert_eq!(metrics.text_area_padding, 8.0);
        assert_eq!(metrics.padding_small, 2);
        assert_eq!(metrics.padding_medium, 4);
        assert_eq!(metrics.padding_large, 8);
        assert_eq!(metrics.border_width, 1);
        // File tree metrics
        assert_eq!(metrics.file_tree_row_height, 22);
        assert_eq!(metrics.file_tree_indent, 16.0);
        assert_eq!(metrics.sidebar_default_width_logical, 250.0);
        assert_eq!(metrics.resize_handle_zone, 4);
    }

    #[test]
    fn test_scaled_metrics_retina() {
        let metrics = ScaledMetrics::new(2.0);
        assert_eq!(metrics.tab_bar_height, 56);
        assert_eq!(metrics.splitter_width, 12.0);
        assert_eq!(metrics.gutter_padding, 8.0);
        assert_eq!(metrics.text_area_padding, 16.0);
        assert_eq!(metrics.padding_small, 4);
        assert_eq!(metrics.padding_medium, 8);
        assert_eq!(metrics.padding_large, 16);
        assert_eq!(metrics.border_width, 2);
        // File tree metrics (row height and indent scale, sidebar width is logical)
        assert_eq!(metrics.file_tree_row_height, 44); // 22 * 2
        assert_eq!(metrics.file_tree_indent, 32.0); // 16 * 2
        assert_eq!(metrics.sidebar_default_width_logical, 250.0); // Not scaled
        assert_eq!(metrics.resize_handle_zone, 8); // 4 * 2
    }

    #[test]
    fn test_scaled_metrics_fractional() {
        let metrics = ScaledMetrics::new(1.5);
        assert_eq!(metrics.tab_bar_height, 42); // 28 * 1.5 = 42
        assert_eq!(metrics.splitter_width, 9.0); // 6 * 1.5 = 9
        assert_eq!(metrics.padding_small, 3); // 2 * 1.5 = 3
        assert_eq!(metrics.padding_medium, 6); // 4 * 1.5 = 6
    }

    #[test]
    fn test_scaled_metrics_border_minimum() {
        let metrics = ScaledMetrics::new(0.5);
        assert_eq!(metrics.border_width, 1);
    }

    #[test]
    fn test_text_start_x_scaled() {
        let metrics = ScaledMetrics::new(1.0);
        let char_width = 10.0;
        let result = text_start_x_scaled(char_width, &metrics);
        let expected = char_width * LINE_NUMBER_GUTTER_CHARS as f32 + 4.0 + 1.0 + 8.0;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_gutter_border_x_scaled() {
        let metrics = ScaledMetrics::new(2.0);
        let char_width = 10.0;
        let result = gutter_border_x_scaled(char_width, &metrics);
        let expected = char_width * LINE_NUMBER_GUTTER_CHARS as f32 + 8.0;
        assert_eq!(result, expected);
    }
}
