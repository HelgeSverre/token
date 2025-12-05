//! Application model - the complete state of the editor
//!
//! This module contains all the state types following the Elm Architecture pattern.

pub mod document;
pub mod editor;
pub mod status_bar;
pub mod ui;

pub use document::{Document, EditOperation};
pub use editor::{
    Cursor, EditorState, Position, RectangleSelectionState, ScrollRevealMode, Selection, Viewport,
};
pub use status_bar::{sync_status_bar, RenderedSegment, SegmentContent, SegmentId, SegmentPosition, StatusBar, StatusBarLayout, StatusSegment, TransientMessage};
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
    /// Document state (text buffer, file, undo/redo)
    pub document: Document,
    /// Editor state (cursor, viewport, scroll settings)
    pub editor: EditorState,
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
        let visible_lines = (window_height as usize) / line_height;

        Self {
            document,
            editor: EditorState::with_viewport(visible_lines, visible_columns),
            ui: UiState::with_status(status_message),
            theme: Theme::default(),
            window_size: (window_width, window_height),
            line_height,
            char_width,
        }
    }

    /// Update viewport dimensions after window resize
    pub fn resize(&mut self, width: u32, height: u32) {
        self.window_size = (width, height);

        let text_x = text_start_x(self.char_width).round();
        let visible_columns = ((width as f32 - text_x) / self.char_width).floor() as usize;
        let visible_lines = (height as usize) / self.line_height;

        self.editor.resize_viewport(visible_lines, visible_columns);
    }

    /// Update char_width from actual font metrics
    pub fn set_char_width(&mut self, char_width: f32) {
        self.char_width = char_width;

        // Recalculate visible columns with new char width
        let text_x = text_start_x(char_width).round();
        let visible_columns =
            ((self.window_size.0 as f32 - text_x) / char_width).floor() as usize;

        self.editor.viewport.visible_columns = visible_columns;
    }

    // Convenience methods that delegate to sub-models

    /// Get the buffer offset for the current cursor position
    pub fn cursor_buffer_position(&self) -> usize {
        self.editor.cursor_offset(&self.document)
    }

    /// Set cursor position from buffer offset (clears selection)
    pub fn set_cursor_from_position(&mut self, pos: usize) {
        self.editor.set_cursor_from_offset(&self.document, pos);
    }

    /// Move cursor to buffer offset without clearing selection
    pub fn move_cursor_to_position(&mut self, pos: usize) {
        self.editor.move_cursor_to_offset(&self.document, pos);
    }

    /// Get the current line length
    pub fn current_line_length(&self) -> usize {
        self.editor.current_line_length(&self.document)
    }

    /// Ensure cursor is visible in viewport (minimal scroll)
    pub fn ensure_cursor_visible(&mut self) {
        self.editor.ensure_cursor_visible(&self.document);
    }

    /// Ensure cursor is visible with direction-aware alignment
    ///
    /// When moving up, cursor is revealed at the top of the safe zone.
    /// When moving down, cursor is revealed at the bottom of the safe zone.
    /// For horizontal movement or no hint, uses minimal scroll.
    pub fn ensure_cursor_visible_directional(&mut self, vertical_up: Option<bool>) {
        let mode = match vertical_up {
            Some(true) => ScrollRevealMode::TopAligned,
            Some(false) => ScrollRevealMode::BottomAligned,
            None => ScrollRevealMode::Minimal,
        };
        self.editor
            .ensure_cursor_visible_with_mode(&self.document, mode);
    }

    /// Reset cursor blink timer
    pub fn reset_cursor_blink(&mut self) {
        self.ui.reset_cursor_blink();
    }

    /// Get a line from the document
    pub fn get_line(&self, line_idx: usize) -> Option<String> {
        self.document.get_line(line_idx)
    }

    /// Get line length
    pub fn line_length(&self, line_idx: usize) -> usize {
        self.document.line_length(line_idx)
    }

    /// Get first non-whitespace column on current line
    pub fn first_non_whitespace_column(&self) -> usize {
        self.document
            .first_non_whitespace_column(self.editor.cursor().line)
    }

    /// Get last non-whitespace column on current line
    pub fn last_non_whitespace_column(&self) -> usize {
        self.document
            .last_non_whitespace_column(self.editor.cursor().line)
    }
}
