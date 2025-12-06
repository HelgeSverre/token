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
pub use ui::UiState;

use crate::theme::Theme;
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
#[derive(Debug, Clone)]
pub struct AppModel {
    /// Editor area containing all documents, editors, groups, and layout
    pub editor_area: EditorArea,
    /// UI state (status bar, cursor blink)
    pub ui: UiState,
    /// Theme for colors and styling
    pub theme: Theme,
    /// Window dimensions
    pub window_size: (u32, u32),
    /// Line height in pixels
    pub line_height: usize,
    /// Character width in pixels (monospace)
    pub char_width: f32,
}

impl AppModel {
    /// Create a new application model with the given window size
    pub fn new(window_width: u32, window_height: u32, file_path: Option<PathBuf>) -> Self {
        let line_height = 20;
        let char_width: f32 = 10.0; // Will be corrected by renderer with actual font metrics

        // Load file if provided, otherwise use demo text
        let (document, status_message) = match file_path {
            Some(path) => match Document::from_file(path.clone()) {
                Ok(doc) => {
                    let msg = format!("Loaded: {}", path.display());
                    (doc, msg)
                }
                Err(e) => {
                    let msg = format!("Error loading {}: {}", path.display(), e);
                    (Document::new(), msg)
                }
            },
            None => {
                let doc = Document::with_text(
                    "Hello, World!\nThis is a text editor built in Rust.\nUsing Elm architecture!\n\nStart typing to edit.\n"
                );
                (doc, "New file".to_string())
            }
        };

        // Calculate viewport dimensions
        let text_x = text_start_x(char_width).round();
        let visible_columns = ((window_width as f32 - text_x) / char_width).floor() as usize;
        // Subtract status bar height (1 line) from available height
        let status_bar_height = line_height;
        let visible_lines =
            (window_height as usize).saturating_sub(status_bar_height) / line_height;

        // Create editor state with viewport
        let editor = EditorState::with_viewport(visible_lines, visible_columns);

        // Create editor area with single document (migration path)
        let editor_area = EditorArea::single_document(document, editor);

        Self {
            editor_area,
            ui: UiState::with_status(status_message),
            theme: Theme::default(),
            window_size: (window_width, window_height),
            line_height,
            char_width,
        }
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
            .first_non_whitespace_column(self.editor().cursor().line)
    }

    /// Get last non-whitespace column on current line
    pub fn last_non_whitespace_column(&self) -> usize {
        self.document()
            .last_non_whitespace_column(self.editor().cursor().line)
    }
}
