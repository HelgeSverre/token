//! Application model - the complete state of the editor
//!
//! This module contains all the state types following the Elm Architecture pattern.

pub mod document;
pub mod editor;
pub mod editor_area;
pub mod status_bar;
pub mod ui;

pub use document::{Document, EditOperation};
pub use editor::{
    Cursor, EditorState, OccurrenceState, Position, RectangleSelectionState, ScrollRevealMode,
    Selection, Viewport,
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
    CommandPaletteState, FindReplaceState, GotoLineState, ModalId, ModalState, ThemePickerState,
    UiState,
};

use crate::config::EditorConfig;
#[cfg(debug_assertions)]
use crate::debug_overlay::DebugOverlay;
use crate::theme::{load_theme, Theme};
use std::path::PathBuf;

/// Layout constant - width of line number gutter in characters (e.g., " 123 ")
pub const LINE_NUMBER_GUTTER_CHARS: usize = 5;
/// Padding after line numbers, before the gutter border (pixels)
pub const GUTTER_PADDING_PX: f32 = 4.0;
/// Padding after the gutter border, before text content (pixels)  
pub const TEXT_AREA_PADDING_PX: f32 = 8.0;

/// Calculate the x-coordinate where text content begins
#[inline]
pub fn text_start_x(char_width: f32) -> f32 {
    let gutter_width = char_width * LINE_NUMBER_GUTTER_CHARS as f32 + GUTTER_PADDING_PX;
    let border_width = 1.0;
    gutter_width + border_width + TEXT_AREA_PADDING_PX
}

/// Calculate the x-coordinate of the gutter border
#[inline]
pub fn gutter_border_x(char_width: f32) -> f32 {
    char_width * LINE_NUMBER_GUTTER_CHARS as f32 + GUTTER_PADDING_PX
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
    /// Debug overlay state (debug builds only)
    #[cfg(debug_assertions)]
    pub debug_overlay: Option<DebugOverlay>,
}

impl AppModel {
    /// Create a new application model with the given window size
    pub fn new(window_width: u32, window_height: u32, file_paths: Vec<PathBuf>) -> Self {
        let line_height = 20;
        let char_width: f32 = 10.0; // Will be corrected by renderer with actual font metrics

        // Calculate viewport dimensions
        let text_x = text_start_x(char_width).round();
        let visible_columns = ((window_width as f32 - text_x) / char_width).floor() as usize;
        let status_bar_height = line_height;
        let visible_lines =
            (window_height as usize).saturating_sub(status_bar_height) / line_height;

        // Load first file or create demo document
        let (first_document, status_message) = if let Some(first_path) = file_paths.first() {
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
        } else {
            let doc = Document::with_text(
                "Hello, World!\nThis is a text editor built in Rust.\nUsing Elm architecture!\n\nStart typing to edit.\n"
            );
            (doc, "New file".to_string())
        };

        // Create editor state with viewport
        let editor = EditorState::with_viewport(visible_lines, visible_columns);

        // Create editor area with first document
        let mut editor_area = EditorArea::single_document(first_document, editor);

        // Open additional files as tabs
        for path in file_paths.into_iter().skip(1) {
            let doc_id = editor_area.next_document_id();
            match Document::from_file(path.clone()) {
                Ok(mut doc) => {
                    doc.id = Some(doc_id);
                    editor_area.documents.insert(doc_id, doc);

                    // Create editor for this document
                    let editor_id = editor_area.next_editor_id();
                    let mut editor = EditorState::with_viewport(visible_lines, visible_columns);
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

        // Ensure config directories exist
        EditorConfig::ensure_config_dirs();

        // Load config and theme
        let config = EditorConfig::load();
        let theme = load_theme(&config.theme).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to load theme '{}': {}, using default",
                config.theme,
                e
            );
            Theme::default()
        });

        Self {
            editor_area,
            ui: UiState::with_status(status_message),
            theme,
            config,
            window_size: (window_width, window_height),
            line_height,
            char_width,
            #[cfg(debug_assertions)]
            debug_overlay: Some(DebugOverlay::new()),
        }
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

        let text_x = text_start_x(self.char_width).round();
        let visible_columns = ((width as f32 - text_x) / self.char_width).floor() as usize;
        // Subtract status bar height (1 line) from available height
        let status_bar_height = self.line_height;
        let visible_lines = (height as usize).saturating_sub(status_bar_height) / self.line_height;

        // FIX: Update ALL editors, not just the focused one
        for editor in self.editor_area.editors.values_mut() {
            editor.resize_viewport(visible_lines, visible_columns);
        }
    }

    /// Update char_width from actual font metrics
    /// Updates ALL editors for split view support
    pub fn set_char_width(&mut self, char_width: f32) {
        self.char_width = char_width;

        // Recalculate visible columns with new char width
        let text_x = text_start_x(char_width).round();
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
