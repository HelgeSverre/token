use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use ropey::Rope;
use softbuffer::{Context, Surface};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::Window;

// Glyph cache key: (character, font_size as bits)
type GlyphCacheKey = (char, u32);
type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

// Color constants for rendering
const CURRENT_LINE_HIGHLIGHT: u32 = 0xFF2A2A2A; // Current line highlight (subtle)

// Layout constant - width of line number gutter in characters
const LINE_NUMBER_GUTTER_CHARS: usize = 6;

// ============================================================================
// MODEL - Application State (Elm's Model)
// ============================================================================

#[derive(Debug, Clone)]
struct Model {
    // Document state
    buffer: Rope,
    cursor: Cursor,

    // View state
    viewport: Viewport,
    window_size: (u32, u32),
    scroll_padding: usize, // Rows of padding to maintain above/below cursor

    // UI state
    status_message: String,

    // Rendering cache
    line_height: usize,
    char_width: f32,

    // For cursor blinking
    cursor_visible: bool,
    last_cursor_blink: Instant,

    // Undo/Redo stacks
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,

    // File tracking
    file_path: Option<PathBuf>, // Path to currently open file
    is_modified: bool,          // Whether buffer has unsaved changes
}

#[derive(Debug, Clone, Copy)]
struct Cursor {
    line: usize,
    column: usize,
    // Desired column for vertical movement
    desired_column: Option<usize>,
}

#[derive(Debug, Clone)]
struct Viewport {
    top_line: usize,
    left_column: usize,
    visible_lines: usize,
    visible_columns: usize,
}

#[derive(Debug, Clone)]
enum EditOperation {
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
}

impl Model {
    fn new(window_width: u32, window_height: u32, file_path: Option<PathBuf>) -> Self {
        let line_height = 20;
        let char_width: f32 = 10.0; // Will be corrected by renderer with actual font metrics

        // Load file if provided, otherwise use demo text
        let (buffer, file_path, status_message) = match file_path {
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let msg = format!("Loaded: {}", path.display());
                    (Rope::from(content), Some(path), msg)
                }
                Err(e) => {
                    let msg = format!("Error loading {}: {}", path.display(), e);
                    (Rope::from(""), None, msg)
                }
            },
            None => {
                (
                    Rope::from("Hello, World!\nThis is a text editor built in Rust.\nUsing Elm architecture!\n\nStart typing to edit.\n"),
                    None,
                    "New file".to_string()
                )
            }
        };

        Self {
            buffer,
            cursor: Cursor {
                line: 0,
                column: 0,
                desired_column: None,
            },
            viewport: Viewport {
                top_line: 0,
                left_column: 0,
                visible_lines: (window_height as usize) / line_height,
                visible_columns: {
                    let text_start_x = (char_width * LINE_NUMBER_GUTTER_CHARS as f32).round();
                    ((window_width as f32 - text_start_x) / char_width).floor() as usize
                },
            },
            window_size: (window_width, window_height),
            scroll_padding: 1, // Default 1 row padding (JetBrains-style)
            status_message,
            line_height,
            char_width,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            file_path,
            is_modified: false, // Fresh file starts unmodified
        }
    }

    fn get_line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            Some(line.to_string())
        } else {
            None
        }
    }

    fn cursor_buffer_position(&self) -> usize {
        let mut pos = 0;
        for i in 0..self.cursor.line {
            if i < self.buffer.len_lines() {
                pos += self.buffer.line(i).len_chars();
            }
        }
        pos + self.cursor.column.min(self.current_line_length())
    }

    fn current_line_length(&self) -> usize {
        if self.cursor.line < self.buffer.len_lines() {
            let line = self.buffer.line(self.cursor.line);
            // Don't count the newline character
            line.len_chars().saturating_sub(
                if line.len_chars() > 0 && line.chars().last() == Some('\n') {
                    1
                } else {
                    0
                },
            )
        } else {
            0
        }
    }

    /// Returns the column of the first non-whitespace character on the current line.
    /// Returns 0 if the line is empty or contains only whitespace.
    fn first_non_whitespace_column(&self) -> usize {
        if self.cursor.line >= self.buffer.len_lines() {
            return 0;
        }
        let line = self.buffer.line(self.cursor.line);
        line.chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .count()
    }

    /// Returns the column after the last non-whitespace character on the current line.
    /// Returns line length if line has no trailing whitespace, or 0 if line is empty/whitespace-only.
    fn last_non_whitespace_column(&self) -> usize {
        if self.cursor.line >= self.buffer.len_lines() {
            return 0;
        }
        let line = self.buffer.line(self.cursor.line);
        let line_str: String = line.chars().collect();
        let trimmed = line_str.trim_end_matches(|c: char| c.is_whitespace());
        trimmed.len()
    }

    fn line_length(&self, line_idx: usize) -> usize {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            line.len_chars().saturating_sub(
                if line.len_chars() > 0 && line.chars().last() == Some('\n') {
                    1
                } else {
                    0
                },
            )
        } else {
            0
        }
    }

    fn set_cursor_from_position(&mut self, pos: usize) {
        let mut remaining = pos;
        for line_idx in 0..self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            let line_len = line.len_chars();
            if remaining < line_len {
                self.cursor.line = line_idx;
                self.cursor.column = remaining;
                self.cursor.desired_column = None;
                return;
            }
            remaining -= line_len;
        }
        // Past end - go to end of buffer
        self.cursor.line = self.buffer.len_lines().saturating_sub(1);
        self.cursor.column = self.current_line_length();
        self.cursor.desired_column = None;
    }

    fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_blink = Instant::now();
    }

    fn ensure_cursor_visible(&mut self) {
        let padding = self.scroll_padding;
        let total_lines = self.buffer.len_lines();

        // Vertical scrolling
        if total_lines > self.viewport.visible_lines {
            let top_boundary = self.viewport.top_line + padding;
            let bottom_boundary =
                self.viewport.top_line + self.viewport.visible_lines - padding - 1;

            // Snap viewport to show cursor with padding
            if self.cursor.line < top_boundary {
                self.viewport.top_line = self.cursor.line.saturating_sub(padding);
            } else if self.cursor.line > bottom_boundary {
                let desired_top = self.cursor.line + padding + 1;
                self.viewport.top_line = desired_top.saturating_sub(self.viewport.visible_lines);
            }

            // Clamp to valid range
            let max_top = total_lines.saturating_sub(self.viewport.visible_lines);
            self.viewport.top_line = self.viewport.top_line.min(max_top);
        } else {
            self.viewport.top_line = 0;
        }

        // Horizontal scrolling (always check, independent of vertical)
        const HORIZONTAL_MARGIN: usize = 4;
        let left_safe = self.viewport.left_column + HORIZONTAL_MARGIN;
        let right_safe = self.viewport.left_column + self.viewport.visible_columns - HORIZONTAL_MARGIN;

        if self.cursor.column < left_safe {
            // Scroll left: put cursor exactly at left safe boundary
            self.viewport.left_column = self.cursor.column.saturating_sub(HORIZONTAL_MARGIN);
        } else if self.cursor.column >= right_safe {
            // Scroll right: put cursor exactly at right safe boundary
            self.viewport.left_column =
                self.cursor.column + HORIZONTAL_MARGIN + 1 - self.viewport.visible_columns;
        }
    }
}

/// Check if a character is a punctuation/symbol boundary (not whitespace)
fn is_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '/' | ':'
            | ','
            | '.'
            | '-'
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | ';'
            | '"'
            | '\''
            | '<'
            | '>'
            | '='
            | '+'
            | '*'
            | '&'
            | '|'
            | '!'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '~'
            | '`'
            | '\\'
            | '?'
            | '_'
    )
}

/// Character type for word navigation (IntelliJ-style)
#[derive(Debug, Clone, Copy, PartialEq)]
enum CharType {
    Whitespace,
    WordChar,    // Alphanumeric characters
    Punctuation, // Symbols/boundaries
}

fn char_type(ch: char) -> CharType {
    if ch.is_whitespace() {
        CharType::Whitespace
    } else if is_punctuation(ch) {
        CharType::Punctuation
    } else {
        CharType::WordChar
    }
}

/// Check if a character is a word boundary (symbol or whitespace)
#[allow(dead_code)]
fn is_word_boundary(ch: char) -> bool {
    ch.is_whitespace() || is_punctuation(ch)
}

// ============================================================================
// MESSAGES - Events that can occur (Elm's Msg)
// ============================================================================

#[derive(Debug, Clone)]
enum Msg {
    // Window events
    Resize(u32, u32),

    // Cursor events
    MoveCursorUp,
    MoveCursorDown,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorLineStart,
    MoveCursorLineEnd,

    // Editing events
    InsertChar(char),
    InsertNewline,
    DeleteBackward,
    DeleteForward,

    // Document navigation
    MoveCursorDocumentStart,
    MoveCursorDocumentEnd,
    PageUp,
    PageDown,

    // Undo/Redo
    Undo,
    Redo,

    // Word navigation
    MoveCursorWordLeft,
    MoveCursorWordRight,

    // Mouse
    SetCursorPosition(usize, usize), // (line, column)

    // Viewport scrolling
    ScrollViewport(i32),           // Positive = scroll down, negative = scroll up
    ScrollViewportHorizontal(i32), // Positive = scroll right, negative = scroll left

    // File operations
    SaveFile, // Save current file (Ctrl+S / Cmd+S)

    // Animation
    BlinkCursor,
}

// ============================================================================
// UPDATE - Pure state transformation (Elm's update)
// ============================================================================

fn update(model: &mut Model, msg: Msg) -> Option<Cmd> {
    match msg {
        Msg::Resize(width, height) => {
            model.window_size = (width, height);
            model.viewport.visible_lines = (height as usize) / model.line_height;
            let text_start_x = (model.char_width * LINE_NUMBER_GUTTER_CHARS as f32).round();
            model.viewport.visible_columns =
                ((width as f32 - text_start_x) / model.char_width).floor() as usize;
            Some(Cmd::Redraw)
        }

        Msg::BlinkCursor => {
            let now = Instant::now();
            if now.duration_since(model.last_cursor_blink) > Duration::from_millis(500) {
                model.cursor_visible = !model.cursor_visible;
                model.last_cursor_blink = now;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        Msg::MoveCursorUp => {
            if model.cursor.line > 0 {
                model.cursor.line -= 1;

                // Maintain desired column for vertical movement
                let desired = model.cursor.desired_column.unwrap_or(model.cursor.column);
                let line_len = model.current_line_length();
                model.cursor.column = desired.min(line_len);
                model.cursor.desired_column = Some(desired);

                // Scroll only if cursor crosses top boundary (JetBrains-style)
                let padding = model.scroll_padding;
                let top_boundary = model.viewport.top_line + padding;

                if model.cursor.line < top_boundary && model.viewport.top_line > 0 {
                    model.viewport.top_line = model.cursor.line.saturating_sub(padding);
                }
            }
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorDown => {
            if model.cursor.line < model.buffer.len_lines().saturating_sub(1) {
                model.cursor.line += 1;

                // Maintain desired column for vertical movement
                let desired = model.cursor.desired_column.unwrap_or(model.cursor.column);
                let line_len = model.current_line_length();
                model.cursor.column = desired.min(line_len);
                model.cursor.desired_column = Some(desired);

                // Scroll only if cursor crosses bottom boundary (JetBrains-style)
                let padding = model.scroll_padding;
                let bottom_boundary =
                    model.viewport.top_line + model.viewport.visible_lines - padding - 1;
                let max_top = model
                    .buffer
                    .len_lines()
                    .saturating_sub(model.viewport.visible_lines);

                if model.cursor.line > bottom_boundary && model.viewport.top_line < max_top {
                    let desired_top = model.cursor.line + padding + 1;
                    model.viewport.top_line = desired_top
                        .saturating_sub(model.viewport.visible_lines)
                        .min(max_top);
                }
            }
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorLeft => {
            if model.cursor.column > 0 {
                model.cursor.column -= 1;
                model.cursor.desired_column = None;
            } else if model.cursor.line > 0 {
                // Move to end of previous line
                model.cursor.line -= 1;
                model.cursor.column = model.current_line_length();
                model.cursor.desired_column = None;
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorRight => {
            let line_len = model.current_line_length();
            if model.cursor.column < line_len {
                model.cursor.column += 1;
                model.cursor.desired_column = None;
            } else if model.cursor.line < model.buffer.len_lines().saturating_sub(1) {
                // Move to start of next line
                model.cursor.line += 1;
                model.cursor.column = 0;
                model.cursor.desired_column = None;
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorLineStart => {
            let first_non_ws = model.first_non_whitespace_column();
            if model.cursor.column == first_non_ws {
                // At first non-whitespace → go to column 0
                model.cursor.column = 0;
            } else {
                // Anywhere else → go to first non-whitespace
                model.cursor.column = first_non_ws;
            }
            model.cursor.desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorLineEnd => {
            let line_end = model.current_line_length();
            let last_non_ws = model.last_non_whitespace_column();
            if model.cursor.column == last_non_ws {
                // At last non-whitespace → go to line end
                model.cursor.column = line_end;
            } else {
                // Anywhere else → go to last non-whitespace
                model.cursor.column = last_non_ws;
            }
            model.cursor.desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::InsertChar(ch) => {
            let cursor_before = model.cursor;
            let pos = model.cursor_buffer_position();

            model.buffer.insert_char(pos, ch);
            model.is_modified = true;
            // Use set_cursor_from_position to ensure cursor.column matches actual buffer position
            // This handles the case where cursor.column was clamped in cursor_buffer_position()
            model.set_cursor_from_position(pos + 1);
            model.ensure_cursor_visible();

            model.redo_stack.clear();
            model.undo_stack.push(EditOperation::Insert {
                position: pos,
                text: ch.to_string(),
                cursor_before,
                cursor_after: model.cursor,
            });

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::InsertNewline => {
            let cursor_before = model.cursor;
            let pos = model.cursor_buffer_position();

            model.buffer.insert_char(pos, '\n');
            model.is_modified = true;
            model.cursor.line += 1;
            model.cursor.column = 0;
            model.cursor.desired_column = None;
            model.ensure_cursor_visible();

            model.redo_stack.clear();
            model.undo_stack.push(EditOperation::Insert {
                position: pos,
                text: "\n".to_string(),
                cursor_before,
                cursor_after: model.cursor,
            });

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::DeleteBackward => {
            if model.cursor.column > 0 {
                let cursor_before = model.cursor;
                let pos = model.cursor_buffer_position();
                let deleted_char = model.buffer.char(pos - 1).to_string();

                model.buffer.remove(pos - 1..pos);
                model.is_modified = true;
                model.cursor.column -= 1;

                model.redo_stack.clear();
                model.undo_stack.push(EditOperation::Delete {
                    position: pos - 1,
                    text: deleted_char,
                    cursor_before,
                    cursor_after: model.cursor,
                });
            } else if model.cursor.line > 0 {
                // Join with previous line
                let cursor_before = model.cursor;
                let pos = model.cursor_buffer_position();

                // Get length of previous line BEFORE removing the newline
                // This is where the cursor should end up after the join
                let prev_line_idx = model.cursor.line - 1;
                let prev_line = model.buffer.line(prev_line_idx);
                let join_column = prev_line.len_chars().saturating_sub(
                    if prev_line.chars().last() == Some('\n') {
                        1
                    } else {
                        0
                    },
                );

                model.buffer.remove(pos - 1..pos);
                model.is_modified = true;
                model.cursor.line -= 1;
                model.cursor.column = join_column;

                model.redo_stack.clear();
                model.undo_stack.push(EditOperation::Delete {
                    position: pos - 1,
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after: model.cursor,
                });
            }
            model.cursor.desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::DeleteForward => {
            let pos = model.cursor_buffer_position();
            if pos < model.buffer.len_chars() {
                let cursor_before = model.cursor;
                let deleted_char = model.buffer.char(pos).to_string();
                model.buffer.remove(pos..pos + 1);
                model.is_modified = true;

                model.redo_stack.clear();
                model.undo_stack.push(EditOperation::Delete {
                    position: pos,
                    text: deleted_char,
                    cursor_before,
                    cursor_after: model.cursor,
                });
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorDocumentStart => {
            model.cursor.line = 0;
            model.cursor.column = 0;
            model.cursor.desired_column = None;
            model.viewport.top_line = 0;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorDocumentEnd => {
            model.cursor.line = model.buffer.len_lines().saturating_sub(1);
            model.cursor.column = model.current_line_length();
            model.cursor.desired_column = None;
            // Scroll to show cursor
            if model.cursor.line >= model.viewport.visible_lines {
                model.viewport.top_line = model.cursor.line - model.viewport.visible_lines + 1;
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::PageUp => {
            let jump = model.viewport.visible_lines.saturating_sub(2);
            model.cursor.line = model.cursor.line.saturating_sub(jump);

            // Maintain desired column for vertical movement (same as MoveCursorUp)
            let desired = model.cursor.desired_column.unwrap_or(model.cursor.column);
            let line_len = model.current_line_length();
            model.cursor.column = desired.min(line_len);
            model.cursor.desired_column = Some(desired);

            model.viewport.top_line = model.viewport.top_line.saturating_sub(jump);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::PageDown => {
            let jump = model.viewport.visible_lines.saturating_sub(2);
            let max_line = model.buffer.len_lines().saturating_sub(1);
            model.cursor.line = (model.cursor.line + jump).min(max_line);

            // Maintain desired column for vertical movement (same as MoveCursorDown)
            let desired = model.cursor.desired_column.unwrap_or(model.cursor.column);
            let line_len = model.current_line_length();
            model.cursor.column = desired.min(line_len);
            model.cursor.desired_column = Some(desired);

            if model.cursor.line >= model.viewport.top_line + model.viewport.visible_lines {
                model.viewport.top_line = model
                    .cursor
                    .line
                    .saturating_sub(model.viewport.visible_lines - 1);
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::Undo => {
            if let Some(op) = model.undo_stack.pop() {
                match &op {
                    EditOperation::Insert {
                        position,
                        text,
                        cursor_before,
                        ..
                    } => {
                        model
                            .buffer
                            .remove(*position..*position + text.chars().count());
                        model.cursor = *cursor_before;
                    }
                    EditOperation::Delete {
                        position,
                        text,
                        cursor_before,
                        ..
                    } => {
                        model.buffer.insert(*position, text);
                        model.cursor = *cursor_before;
                    }
                }
                model.is_modified = true;
                model.redo_stack.push(op);
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        Msg::Redo => {
            if let Some(op) = model.redo_stack.pop() {
                match &op {
                    EditOperation::Insert {
                        position,
                        text,
                        cursor_after,
                        ..
                    } => {
                        model.buffer.insert(*position, text);
                        model.cursor = *cursor_after;
                    }
                    EditOperation::Delete {
                        position,
                        text,
                        cursor_after,
                        ..
                    } => {
                        model
                            .buffer
                            .remove(*position..*position + text.chars().count());
                        model.cursor = *cursor_after;
                    }
                }
                model.is_modified = true;
                model.undo_stack.push(op);
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        Msg::MoveCursorWordLeft => {
            // IntelliJ-style: treat whitespace as its own navigable unit
            // Moving left: go to the START of the current character type group
            let pos = model.cursor_buffer_position();
            if pos == 0 {
                return Some(Cmd::Redraw);
            }

            let text: String = model.buffer.slice(..pos).chars().collect();
            let chars: Vec<char> = text.chars().collect();
            let mut i = chars.len();

            // Look at the character before the cursor
            if i > 0 {
                let current_type = char_type(chars[i - 1]);
                // Skip all characters of the same type
                while i > 0 && char_type(chars[i - 1]) == current_type {
                    i -= 1;
                }
            }

            model.set_cursor_from_position(i);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorWordRight => {
            // IntelliJ-style: treat whitespace as its own navigable unit
            // Moving right: go to the END of the current character type group
            let pos = model.cursor_buffer_position();
            let total_chars = model.buffer.len_chars();
            if pos >= total_chars {
                return Some(Cmd::Redraw);
            }

            let text: String = model.buffer.slice(pos..).chars().collect();
            let chars: Vec<char> = text.chars().collect();
            let mut i = 0;

            if !chars.is_empty() {
                let current_type = char_type(chars[0]);
                // Skip all characters of the same type
                while i < chars.len() && char_type(chars[i]) == current_type {
                    i += 1;
                }
            }

            model.set_cursor_from_position(pos + i);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::SetCursorPosition(line, column) => {
            model.cursor.line = line;
            model.cursor.column = column;
            model.cursor.desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::ScrollViewport(delta) => {
            let total_lines = model.buffer.len_lines();
            if total_lines <= model.viewport.visible_lines {
                return None; // No scrolling needed
            }

            let max_top = total_lines.saturating_sub(model.viewport.visible_lines);

            if delta > 0 {
                // Scroll down
                model.viewport.top_line = (model.viewport.top_line + delta as usize).min(max_top);
            } else if delta < 0 {
                // Scroll up
                model.viewport.top_line =
                    model.viewport.top_line.saturating_sub(delta.abs() as usize);
            }

            Some(Cmd::Redraw)
        }

        Msg::ScrollViewportHorizontal(delta) => {
            // Find maximum line length in visible area
            let max_line_len = (model.viewport.top_line
                ..model.viewport.top_line + model.viewport.visible_lines)
                .filter_map(|i| {
                    if i < model.buffer.len_lines() {
                        Some(model.line_length(i))
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0);

            // Only scroll if content is wider than viewport
            if max_line_len <= model.viewport.visible_columns {
                model.viewport.left_column = 0;
                return None;
            }

            let max_left = max_line_len.saturating_sub(model.viewport.visible_columns);

            if delta > 0 {
                // Scroll right
                model.viewport.left_column =
                    (model.viewport.left_column + delta as usize).min(max_left);
            } else if delta < 0 {
                // Scroll left
                model.viewport.left_column = model
                    .viewport
                    .left_column
                    .saturating_sub(delta.abs() as usize);
            }

            Some(Cmd::Redraw)
        }

        Msg::SaveFile => {
            match &model.file_path {
                Some(path) => {
                    // Write buffer to file
                    match std::fs::write(path, model.buffer.to_string()) {
                        Ok(_) => {
                            model.is_modified = false;
                            model.status_message = format!("Saved: {}", path.display());
                        }
                        Err(e) => {
                            model.status_message = format!("Error saving: {}", e);
                        }
                    }
                }
                None => {
                    model.status_message = "No file path - cannot save".to_string();
                }
            }
            Some(Cmd::Redraw)
        }
    }
}

fn handle_key(
    model: &mut Model,
    key: Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
) -> Option<Cmd> {
    match key {
        // Undo/Redo (Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y)
        Key::Character(ref s) if ctrl && s.eq_ignore_ascii_case("z") => {
            if shift {
                update(model, Msg::Redo)
            } else {
                update(model, Msg::Undo)
            }
        }
        Key::Character(ref s) if ctrl && s.eq_ignore_ascii_case("y") => update(model, Msg::Redo),

        // Save file (Ctrl+S on Windows/Linux, Cmd+S on macOS)
        Key::Character(ref s) if s.eq_ignore_ascii_case("s") && (ctrl || logo) => {
            update(model, Msg::SaveFile)
        }

        // Document navigation (Ctrl+Home/End)
        Key::Named(NamedKey::Home) if ctrl => update(model, Msg::MoveCursorDocumentStart),
        Key::Named(NamedKey::End) if ctrl => update(model, Msg::MoveCursorDocumentEnd),

        // Line navigation (Cmd+Arrow on macOS)
        Key::Named(NamedKey::ArrowLeft) if logo => update(model, Msg::MoveCursorLineStart),
        Key::Named(NamedKey::ArrowRight) if logo => update(model, Msg::MoveCursorLineEnd),

        // Line navigation (Home/End keys)
        Key::Named(NamedKey::Home) => update(model, Msg::MoveCursorLineStart),
        Key::Named(NamedKey::End) => update(model, Msg::MoveCursorLineEnd),

        // Page navigation
        Key::Named(NamedKey::PageUp) => update(model, Msg::PageUp),
        Key::Named(NamedKey::PageDown) => update(model, Msg::PageDown),

        // Word navigation (Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt => update(model, Msg::MoveCursorWordLeft),
        Key::Named(NamedKey::ArrowRight) if alt => update(model, Msg::MoveCursorWordRight),

        // Arrow keys
        Key::Named(NamedKey::ArrowUp) => update(model, Msg::MoveCursorUp),
        Key::Named(NamedKey::ArrowDown) => update(model, Msg::MoveCursorDown),
        Key::Named(NamedKey::ArrowLeft) => update(model, Msg::MoveCursorLeft),
        Key::Named(NamedKey::ArrowRight) => update(model, Msg::MoveCursorRight),

        // Editing
        Key::Named(NamedKey::Enter) => update(model, Msg::InsertNewline),
        Key::Named(NamedKey::Backspace) => update(model, Msg::DeleteBackward),
        Key::Named(NamedKey::Delete) => update(model, Msg::DeleteForward),
        Key::Named(NamedKey::Space) if !ctrl => update(model, Msg::InsertChar(' ')),

        // Character input (only when no Ctrl)
        Key::Character(ref s) if !ctrl => {
            let mut cmd = None;
            for ch in s.chars() {
                cmd = update(model, Msg::InsertChar(ch)).or(cmd);
            }
            cmd
        }

        _ => None,
    }
}

// ============================================================================
// COMMANDS - Side effects to perform (Elm's Cmd)
// ============================================================================

#[derive(Debug)]
enum Cmd {
    Redraw,
}

// ============================================================================
// VIEW - Render the model to screen
// ============================================================================

struct Renderer {
    font: Font,
    surface: Surface<Rc<Window>, Rc<Window>>,
    width: u32,
    height: u32,
    font_size: f32,
    line_metrics: LineMetrics,
    glyph_cache: GlyphCache,
    char_width: f32, // Cached from actual font metrics for consistent positioning
}

impl Renderer {
    fn new(window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<Self> {
        let scale_factor = window.scale_factor();
        let (width, height) = {
            let size = window.inner_size();
            (size.width, size.height)
        };

        let surface = Surface::new(context, Rc::clone(&window))
            .map_err(|e| anyhow::anyhow!("Failed to create surface: {}", e))?;

        // Load JetBrains Mono font
        let font = Font::from_bytes(
            include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
            FontSettings::default(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to load font: {}", e))?;

        // Base font size 14pt, scaled for HiDPI
        let font_size = 14.0 * scale_factor as f32;

        // Get font line metrics for proper baseline positioning
        let line_metrics = font
            .horizontal_line_metrics(font_size)
            .expect("Font missing horizontal line metrics");

        // Get actual character width from font metrics (use 'M' as reference for monospace)
        let (metrics, _) = font.rasterize('M', font_size);
        let char_width = metrics.advance_width;

        Ok(Self {
            font,
            surface,
            width,
            height,
            font_size,
            line_metrics,
            glyph_cache: HashMap::new(),
            char_width,
        })
    }

    fn char_width(&self) -> f32 {
        self.char_width
    }

    fn render(&mut self, model: &Model) -> Result<()> {
        // Resize surface if needed
        if self.width != model.window_size.0 || self.height != model.window_size.1 {
            self.width = model.window_size.0;
            self.height = model.window_size.1;
            self.surface
                .resize(
                    NonZeroU32::new(self.width).unwrap(),
                    NonZeroU32::new(self.height).unwrap(),
                )
                .map_err(|e| anyhow::anyhow!("Failed to resize surface: {}", e))?;
        }

        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| anyhow::anyhow!("Failed to get surface buffer: {}", e))?;

        // Clear screen (dark background)
        let bg_color = 0xFF1E1E1E;
        buffer.fill(bg_color);

        // Calculate scaled metrics using proper font line metrics
        let font_size = self.font_size;
        let ascent = self.line_metrics.ascent;
        let line_height = self.line_metrics.new_line_size.ceil() as usize;
        let char_width = self.char_width; // Use actual font metrics
        let text_start_x = (char_width * LINE_NUMBER_GUTTER_CHARS as f32).round() as usize;
        let width = self.width;
        let height = self.height;

        // Render visible lines
        let visible_lines = (height as usize) / line_height;
        let end_line = (model.viewport.top_line + visible_lines).min(model.buffer.len_lines());

        // Draw current line highlight (if cursor visible in viewport)
        if model.cursor.line >= model.viewport.top_line && model.cursor.line < end_line {
            let screen_line = model.cursor.line - model.viewport.top_line;
            let highlight_y = screen_line * line_height;

            for py in highlight_y..(highlight_y + line_height) {
                for px in 0..(width as usize) {
                    if py < height as usize {
                        buffer[py * width as usize + px] = CURRENT_LINE_HIGHLIGHT;
                    }
                }
            }
        }

        for (screen_line, doc_line) in (model.viewport.top_line..end_line).enumerate() {
            if let Some(line_text) = model.get_line(doc_line) {
                let y = screen_line * line_height;

                // Draw line number (gray)
                let line_num_str = format!("{:4} ", doc_line + 1);
                draw_text(
                    &mut buffer,
                    &self.font,
                    &mut self.glyph_cache,
                    font_size,
                    ascent,
                    width,
                    height,
                    0,
                    y,
                    &line_num_str,
                    0xFF606060,
                );

                // Draw text content
                let visible_text = if line_text.ends_with('\n') {
                    &line_text[..line_text.len() - 1]
                } else {
                    &line_text
                };

                let display_text: String = visible_text
                    .chars()
                    .skip(model.viewport.left_column)
                    .take(((width as f32 - text_start_x as f32) / char_width).floor() as usize)
                    .collect();

                draw_text(
                    &mut buffer,
                    &self.font,
                    &mut self.glyph_cache,
                    font_size,
                    ascent,
                    width,
                    height,
                    text_start_x,
                    y,
                    &display_text,
                    0xFFE0E0E0,
                );
            }
        }

        // Draw cursor (only if within visible viewport)
        if model.cursor_visible {
            let cursor_in_vertical_view = model.cursor.line >= model.viewport.top_line
                && model.cursor.line < model.viewport.top_line + visible_lines;
            // Calculate actual visible columns using renderer's char_width (not model's hardcoded value)
            let actual_visible_columns =
                ((width as f32 - text_start_x as f32) / char_width).floor() as usize;
            let cursor_in_horizontal_view = model.cursor.column >= model.viewport.left_column
                && model.cursor.column < model.viewport.left_column + actual_visible_columns;

            if cursor_in_vertical_view && cursor_in_horizontal_view {
                let screen_line = model.cursor.line - model.viewport.top_line;
                let cursor_column = model.cursor.column - model.viewport.left_column;
                let x = (text_start_x as f32 + cursor_column as f32 * char_width).round() as usize;
                let y = screen_line * line_height;
                let cursor_color = 0xFFFFFF00; // Yellow

                for dy in 0..(line_height - 2) {
                    for dx in 0..2 {
                        let px = x + dx;
                        let py = y + dy + 1;
                        if px < width as usize && py < height as usize {
                            buffer[py * width as usize + px] = cursor_color;
                        }
                    }
                }
            }
        }

        // Draw status bar
        let status_y = height as usize - line_height;
        for py in status_y..height as usize {
            for px in 0..width as usize {
                buffer[py * width as usize + px] = 0xFF303030;
            }
        }
        // File info with modified flag
        let file_info = match &model.file_path {
            Some(path) => {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("untitled");
                let modified = if model.is_modified { "*" } else { "" };
                format!("{}{}", filename, modified)
            }
            None => "[No Name]".to_string(),
        };

        let status = format!(
            " {} | Ln {}, Col {} | {} ",
            file_info,
            model.cursor.line + 1,
            model.cursor.column + 1,
            model.status_message
        );
        draw_text(
            &mut buffer,
            &self.font,
            &mut self.glyph_cache,
            font_size,
            ascent,
            width,
            height,
            5,
            status_y + 2,
            &status,
            0xFFB0B0B0,
        );

        buffer
            .present()
            .map_err(|e| anyhow::anyhow!("Failed to present buffer: {}", e))?;
        Ok(())
    }

    #[allow(dead_code)]
    fn get_char_width(&mut self) -> f32 {
        let key = ('m', self.font_size.to_bits());
        if let Some((metrics, _)) = self.glyph_cache.get(&key) {
            metrics.advance_width
        } else {
            let (metrics, bitmap) = self.font.rasterize('m', self.font_size);
            let width = metrics.advance_width;
            self.glyph_cache.insert(key, (metrics, bitmap));
            width
        }
    }

    fn pixel_to_cursor(&mut self, x: f64, y: f64, model: &Model) -> (usize, usize) {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        let char_width = self.char_width as f64; // Use cached char_width for consistency
        let text_start_x = (self.char_width * LINE_NUMBER_GUTTER_CHARS as f32).round() as f64;

        // Calculate line from y position
        let visual_line = (y / line_height).floor() as usize;
        let line = model.viewport.top_line + visual_line;
        let line = line.min(model.buffer.len_lines().saturating_sub(1));

        // Calculate column from x position (add left_column for horizontal scroll offset)
        let x_offset = x - text_start_x;
        let column = if x_offset > 0.0 {
            model.viewport.left_column + (x_offset / char_width).round() as usize
        } else {
            model.viewport.left_column
        };

        // Clamp column to line length
        let line_len = model.line_length(line);
        let column = column.min(line_len);

        (line, column)
    }
}

fn draw_text(
    buffer: &mut [u32],
    font: &Font,
    glyph_cache: &mut GlyphCache,
    font_size: f32,
    ascent: f32,
    width: u32,
    height: u32,
    x: usize,
    y: usize, // line_top position
    text: &str,
    color: u32,
) {
    let mut current_x = x as f32;

    // Calculate baseline position: line_top + ascent
    let baseline = y as f32 + ascent;

    for ch in text.chars() {
        // Use cached glyph or rasterize
        let key = (ch, font_size.to_bits());
        if !glyph_cache.contains_key(&key) {
            let (metrics, bitmap) = font.rasterize(ch, font_size);
            glyph_cache.insert(key, (metrics, bitmap));
        }
        let (metrics, bitmap) = glyph_cache.get(&key).unwrap();

        // Draw the rasterized glyph
        // Position glyph for PositiveYDown coordinate system
        // (matches fontdue's layout.rs: y = -height - ymin)
        let glyph_top = baseline - metrics.height as f32 - metrics.ymin as f32;

        for bitmap_y in 0..metrics.height {
            for bitmap_x in 0..metrics.width {
                let bitmap_idx = bitmap_y * metrics.width + bitmap_x;
                if bitmap_idx < bitmap.len() {
                    let alpha = bitmap[bitmap_idx];
                    if alpha > 0 {
                        let px = current_x as isize + bitmap_x as isize + metrics.xmin as isize;
                        let py = (glyph_top + bitmap_y as f32) as isize;

                        if px >= 0
                            && py >= 0
                            && (px as usize) < width as usize
                            && (py as usize) < height as usize
                        {
                            let px = px as usize;
                            let py = py as usize;

                            // Blend the glyph with background based on alpha
                            let alpha_f = alpha as f32 / 255.0;
                            let bg_pixel = buffer[py * width as usize + px];

                            let bg_r = ((bg_pixel >> 16) & 0xFF) as f32;
                            let bg_g = ((bg_pixel >> 8) & 0xFF) as f32;
                            let bg_b = (bg_pixel & 0xFF) as f32;

                            let fg_r = ((color >> 16) & 0xFF) as f32;
                            let fg_g = ((color >> 8) & 0xFF) as f32;
                            let fg_b = (color & 0xFF) as f32;

                            let final_r = (bg_r * (1.0 - alpha_f) + fg_r * alpha_f) as u32;
                            let final_g = (bg_g * (1.0 - alpha_f) + fg_g * alpha_f) as u32;
                            let final_b = (bg_b * (1.0 - alpha_f) + fg_b * alpha_f) as u32;

                            buffer[py * width as usize + px] =
                                0xFF000000 | (final_r << 16) | (final_g << 8) | final_b;
                        }
                    }
                }
            }
        }

        // Advance to the next character position
        current_x += metrics.advance_width;
    }
}

// ============================================================================
// APPLICATION - Main event loop
// ============================================================================

struct App {
    model: Model,
    renderer: Option<Renderer>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    last_tick: Instant,
    modifiers: ModifiersState,
    mouse_position: Option<(f64, f64)>,
}

impl App {
    fn new(window_width: u32, window_height: u32, file_path: Option<PathBuf>) -> Self {
        Self {
            model: Model::new(window_width, window_height, file_path),
            renderer: None,
            window: None,
            context: None,
            last_tick: Instant::now(),
            modifiers: ModifiersState::empty(),
            mouse_position: None,
        }
    }

    fn init_renderer(&mut self, window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<()> {
        let renderer = Renderer::new(window, context)?;

        // Sync actual char_width from renderer to model for accurate viewport calculations
        self.model.char_width = renderer.char_width();

        // Recalculate visible_columns using same formula as renderer
        let text_start_x = (self.model.char_width * LINE_NUMBER_GUTTER_CHARS as f32).round();
        self.model.viewport.visible_columns =
            ((self.model.window_size.0 as f32 - text_start_x) / self.model.char_width).floor()
                as usize;

        self.renderer = Some(renderer);
        Ok(())
    }

    fn handle_event(&mut self, event: &WindowEvent) -> Option<Cmd> {
        match event {
            WindowEvent::Resized(size) => {
                update(&mut self.model, Msg::Resize(size.width, size.height))
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    let alt = self.modifiers.alt_key();
                    let logo = self.modifiers.super_key(); // Cmd on macOS
                    handle_key(
                        &mut self.model,
                        event.logical_key.clone(),
                        ctrl,
                        shift,
                        alt,
                        logo,
                    )
                } else {
                    None
                }
            }
            WindowEvent::RedrawRequested => {
                // Actually perform the render here
                if let Err(e) = self.render() {
                    eprintln!("Render error: {}", e);
                }
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Some((position.x, position.y));
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        return update(&mut self.model, Msg::SetCursorPosition(line, column));
                    }
                }
                None
            }
            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                let (h_delta, v_delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        // y is positive for scroll up, negative for scroll down
                        // We want positive to mean scroll down, so negate y
                        // x is already positive for scroll right on macOS (including Shift+scroll)
                        ((x * 3.0) as i32, (-y * 3.0) as i32)
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        // Convert pixels to lines/columns (approximate)
                        let line_height = self.model.line_height as f64;
                        let char_width = self.model.char_width as f64;
                        ((pos.x / char_width) as i32, (-pos.y / line_height) as i32)
                    }
                };

                // Handle vertical scroll
                let v_cmd = if v_delta != 0 {
                    update(&mut self.model, Msg::ScrollViewport(v_delta))
                } else {
                    None
                };

                // Handle horizontal scroll
                let h_cmd = if h_delta != 0 {
                    update(&mut self.model, Msg::ScrollViewportHorizontal(h_delta))
                } else {
                    None
                };

                // Return Redraw if either scrolled
                v_cmd.or(h_cmd)
            }
            _ => None,
        }
    }

    fn render(&mut self) -> Result<()> {
        if let Some(renderer) = &mut self.renderer {
            renderer.render(&self.model)?;
        }
        Ok(())
    }

    fn tick(&mut self) -> Option<Cmd> {
        // Handle animations
        update(&mut self.model, Msg::BlinkCursor)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Token")
                .with_inner_size(LogicalSize::new(800, 600));

            let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
            let context = Context::new(Rc::clone(&window)).unwrap();

            self.init_renderer(Rc::clone(&window), &context).unwrap();
            self.window = Some(window);
            self.context = Some(context);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let should_exit = matches!(event, WindowEvent::CloseRequested);
        let should_redraw = if let Some(window) = &self.window {
            if window_id == window.id() && !should_exit {
                self.handle_event(&event).is_some()
            } else {
                false
            }
        } else {
            false
        };

        if should_exit {
            event_loop.exit();
        } else if should_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Use Poll to make event loop responsive to scrolling and user input
        // This ensures immediate response to mouse wheel, touchpad, and keyboard events
        event_loop.set_control_flow(ControlFlow::Poll);

        // Only tick for cursor blinking animation
        let now = Instant::now();
        if now.duration_since(self.last_tick) > Duration::from_millis(500) {
            self.last_tick = now;
            if self.tick().is_some() {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }
}

// ============================================================================
// MAIN - Entry point
// ============================================================================

fn main() -> Result<()> {
    env_logger::init();

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let file_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let event_loop = EventLoop::new()?;
    let mut app = App::new(800, 600, file_path);

    event_loop.run_app(&mut app)?;

    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test model with given text and cursor position
    fn test_model(text: &str, line: usize, column: usize) -> Model {
        Model {
            buffer: Rope::from(text),
            cursor: Cursor {
                line,
                column,
                desired_column: None,
            },
            viewport: Viewport {
                top_line: 0,
                left_column: 0,
                visible_lines: 25,
                visible_columns: 80,
            },
            window_size: (800, 600),
            scroll_padding: 1, // Default padding for tests
            status_message: "Test".to_string(),
            line_height: 20,
            char_width: 10.0,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            file_path: None,    // Tests don't need file paths
            is_modified: false, // Start unmodified
        }
    }

    /// Helper to get buffer content as string
    fn buffer_to_string(model: &Model) -> String {
        model.buffer.to_string()
    }

    // ========================================================================
    // cursor_buffer_position() tests
    // ========================================================================

    #[test]
    fn test_cursor_buffer_position_start_of_file() {
        let model = test_model("hello\nworld\n", 0, 0);
        assert_eq!(model.cursor_buffer_position(), 0);
    }

    #[test]
    fn test_cursor_buffer_position_middle_of_first_line() {
        let model = test_model("hello\nworld\n", 0, 3);
        assert_eq!(model.cursor_buffer_position(), 3); // "hel|lo"
    }

    #[test]
    fn test_cursor_buffer_position_end_of_first_line() {
        let model = test_model("hello\nworld\n", 0, 5);
        assert_eq!(model.cursor_buffer_position(), 5); // "hello|"
    }

    #[test]
    fn test_cursor_buffer_position_start_of_second_line() {
        let model = test_model("hello\nworld\n", 1, 0);
        // "hello\n" = 6 chars, so position 6 is start of "world"
        assert_eq!(model.cursor_buffer_position(), 6);
    }

    #[test]
    fn test_cursor_buffer_position_middle_of_second_line() {
        let model = test_model("hello\nworld\n", 1, 3);
        // "hello\n" = 6 chars, + 3 = 9
        assert_eq!(model.cursor_buffer_position(), 9); // "wor|ld"
    }

    #[test]
    fn test_cursor_buffer_position_empty_line() {
        let model = test_model("hello\n\nworld\n", 1, 0);
        // "hello\n" = 6 chars, empty line at position 6
        assert_eq!(model.cursor_buffer_position(), 6);
    }

    #[test]
    fn test_cursor_buffer_position_after_empty_line() {
        let model = test_model("hello\n\nworld\n", 2, 0);
        // "hello\n" = 6, "\n" = 1, so "world" starts at 7
        assert_eq!(model.cursor_buffer_position(), 7);
    }

    #[test]
    fn test_cursor_buffer_position_clamped_column() {
        // Column exceeds line length - should be clamped
        let model = test_model("hi\nworld\n", 0, 10);
        // Line "hi" has length 2, so column should clamp to 2
        assert_eq!(model.cursor_buffer_position(), 2);
    }

    // ========================================================================
    // current_line_length() tests
    // ========================================================================

    #[test]
    fn test_current_line_length_with_newline() {
        let model = test_model("hello\nworld\n", 0, 0);
        // "hello\n" has 6 chars, but length should be 5 (excluding newline)
        assert_eq!(model.current_line_length(), 5);
    }

    #[test]
    fn test_current_line_length_without_newline() {
        let model = test_model("hello", 0, 0);
        // "hello" has no newline, length is 5
        assert_eq!(model.current_line_length(), 5);
    }

    #[test]
    fn test_current_line_length_empty_line() {
        let model = test_model("hello\n\nworld\n", 1, 0);
        // Empty line has length 0
        assert_eq!(model.current_line_length(), 0);
    }

    #[test]
    fn test_current_line_length_last_line_with_newline() {
        let model = test_model("hello\nworld\n", 1, 0);
        // "world\n" has 6 chars, length should be 5
        assert_eq!(model.current_line_length(), 5);
    }

    // ========================================================================
    // InsertChar tests
    // ========================================================================

    #[test]
    fn test_insert_char_at_start() {
        let mut model = test_model("hello", 0, 0);
        update(&mut model, Msg::InsertChar('X'));

        assert_eq!(buffer_to_string(&model), "Xhello");
        assert_eq!(model.cursor.column, 1);
        assert_eq!(model.cursor.line, 0);
    }

    #[test]
    fn test_insert_char_at_middle() {
        let mut model = test_model("hello", 0, 2);
        update(&mut model, Msg::InsertChar('X'));

        assert_eq!(buffer_to_string(&model), "heXllo");
        assert_eq!(model.cursor.column, 3);
    }

    #[test]
    fn test_insert_char_at_end() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::InsertChar('X'));

        assert_eq!(buffer_to_string(&model), "helloX");
        assert_eq!(model.cursor.column, 6);
    }

    #[test]
    fn test_insert_space_at_middle() {
        let mut model = test_model("helloworld", 0, 5);
        update(&mut model, Msg::InsertChar(' '));

        assert_eq!(buffer_to_string(&model), "hello world");
        assert_eq!(model.cursor.column, 6);
    }

    #[test]
    fn test_insert_multiple_chars_consecutively() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::InsertChar(' '));
        update(&mut model, Msg::InsertChar('w'));
        update(&mut model, Msg::InsertChar('o'));
        update(&mut model, Msg::InsertChar('r'));
        update(&mut model, Msg::InsertChar('l'));
        update(&mut model, Msg::InsertChar('d'));

        assert_eq!(buffer_to_string(&model), "hello world");
        assert_eq!(model.cursor.column, 11);
    }

    #[test]
    fn test_insert_char_on_second_line() {
        let mut model = test_model("hello\nworld", 1, 2);
        update(&mut model, Msg::InsertChar('X'));

        assert_eq!(buffer_to_string(&model), "hello\nwoXrld");
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 3);
    }

    #[test]
    fn test_insert_multiple_spaces_middle_of_line() {
        let mut model = test_model("helloworld", 0, 5);

        // Insert 3 spaces consecutively - this tests the "playing catchup" bug
        update(&mut model, Msg::InsertChar(' '));
        assert_eq!(buffer_to_string(&model), "hello world");
        assert_eq!(model.cursor.column, 6);

        update(&mut model, Msg::InsertChar(' '));
        assert_eq!(buffer_to_string(&model), "hello  world");
        assert_eq!(model.cursor.column, 7);

        update(&mut model, Msg::InsertChar(' '));
        assert_eq!(buffer_to_string(&model), "hello   world");
        assert_eq!(model.cursor.column, 8);
    }

    #[test]
    fn test_insert_after_cursor_position_clamped() {
        // This tests the suspected bug: cursor.column > line length
        let mut model = test_model("hi", 0, 10); // column 10 on 2-char line

        // Position should be clamped to 2
        let pos = model.cursor_buffer_position();
        assert_eq!(pos, 2);

        // Insert should happen at clamped position
        update(&mut model, Msg::InsertChar('X'));
        assert_eq!(buffer_to_string(&model), "hiX");

        // After insert, cursor.column should be valid
        assert!(model.cursor.column <= model.current_line_length());
    }

    // ========================================================================
    // InsertNewline tests
    // ========================================================================

    #[test]
    fn test_insert_newline_at_end() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::InsertNewline);

        assert_eq!(buffer_to_string(&model), "hello\n");
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_insert_newline_at_middle() {
        let mut model = test_model("hello", 0, 2);
        update(&mut model, Msg::InsertNewline);

        assert_eq!(buffer_to_string(&model), "he\nllo");
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_insert_newline_at_start() {
        let mut model = test_model("hello", 0, 0);
        update(&mut model, Msg::InsertNewline);

        assert_eq!(buffer_to_string(&model), "\nhello");
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 0);
    }

    // ========================================================================
    // DeleteBackward tests
    // ========================================================================

    #[test]
    fn test_delete_backward_middle_of_line() {
        let mut model = test_model("hello", 0, 3);
        update(&mut model, Msg::DeleteBackward);

        assert_eq!(buffer_to_string(&model), "helo");
        assert_eq!(model.cursor.column, 2);
    }

    #[test]
    fn test_delete_backward_at_start_of_line() {
        let mut model = test_model("hello", 0, 0);
        update(&mut model, Msg::DeleteBackward);

        // Nothing should happen
        assert_eq!(buffer_to_string(&model), "hello");
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_delete_backward_joins_lines() {
        let mut model = test_model("hello\nworld", 1, 0);
        update(&mut model, Msg::DeleteBackward);

        assert_eq!(buffer_to_string(&model), "helloworld");
        assert_eq!(model.cursor.line, 0);
        assert_eq!(model.cursor.column, 5); // End of "hello"
    }

    #[test]
    fn test_delete_backward_after_empty_line() {
        let mut model = test_model("hello\n\nworld", 2, 0);
        update(&mut model, Msg::DeleteBackward);

        assert_eq!(buffer_to_string(&model), "hello\nworld");
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 0);
    }

    // ========================================================================
    // DeleteForward tests
    // ========================================================================

    #[test]
    fn test_delete_forward_middle_of_line() {
        let mut model = test_model("hello", 0, 2);
        update(&mut model, Msg::DeleteForward);

        assert_eq!(buffer_to_string(&model), "helo");
        assert_eq!(model.cursor.column, 2); // Unchanged
    }

    #[test]
    fn test_delete_forward_at_end_of_line() {
        let mut model = test_model("hello\nworld", 0, 5);
        update(&mut model, Msg::DeleteForward);

        // Should delete the newline, joining lines
        assert_eq!(buffer_to_string(&model), "helloworld");
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_delete_forward_at_end_of_buffer() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::DeleteForward);

        // Nothing to delete
        assert_eq!(buffer_to_string(&model), "hello");
    }

    // ========================================================================
    // Cursor movement tests
    // ========================================================================

    #[test]
    fn test_move_cursor_left() {
        let mut model = test_model("hello", 0, 3);
        update(&mut model, Msg::MoveCursorLeft);

        assert_eq!(model.cursor.column, 2);
    }

    #[test]
    fn test_move_cursor_left_at_start_of_line() {
        let mut model = test_model("hello\nworld", 1, 0);
        update(&mut model, Msg::MoveCursorLeft);

        // Should move to end of previous line
        assert_eq!(model.cursor.line, 0);
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_move_cursor_right() {
        let mut model = test_model("hello", 0, 2);
        update(&mut model, Msg::MoveCursorRight);

        assert_eq!(model.cursor.column, 3);
    }

    #[test]
    fn test_move_cursor_right_at_end_of_line() {
        let mut model = test_model("hello\nworld", 0, 5);
        update(&mut model, Msg::MoveCursorRight);

        // Should move to start of next line
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_move_cursor_up() {
        let mut model = test_model("hello\nworld", 1, 3);
        update(&mut model, Msg::MoveCursorUp);

        assert_eq!(model.cursor.line, 0);
        assert_eq!(model.cursor.column, 3);
    }

    #[test]
    fn test_move_cursor_up_preserves_desired_column() {
        let mut model = test_model("hello\nhi\nworld", 0, 4);

        // Move down to short line "hi"
        update(&mut model, Msg::MoveCursorDown);
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 2); // Clamped to "hi" length

        // Move down to "world"
        update(&mut model, Msg::MoveCursorDown);
        assert_eq!(model.cursor.line, 2);
        assert_eq!(model.cursor.column, 4); // Restored to desired column
    }

    #[test]
    fn test_move_cursor_down() {
        let mut model = test_model("hello\nworld", 0, 3);
        update(&mut model, Msg::MoveCursorDown);

        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 3);
    }

    // ========================================================================
    // Smart Home/End tests (toggle between line edge and non-whitespace)
    // ========================================================================

    #[test]
    fn test_smart_home_from_middle() {
        // From middle of line → first non-whitespace
        let mut model = test_model("    hello", 0, 6);
        update(&mut model, Msg::MoveCursorLineStart);
        assert_eq!(model.cursor.column, 4); // First non-ws is at column 4
    }

    #[test]
    fn test_smart_home_from_column_zero() {
        // From column 0 → first non-whitespace
        let mut model = test_model("    hello", 0, 0);
        update(&mut model, Msg::MoveCursorLineStart);
        assert_eq!(model.cursor.column, 4); // First non-ws is at column 4
    }

    #[test]
    fn test_smart_home_toggle() {
        // From first non-ws → back to column 0
        let mut model = test_model("    hello", 0, 4);
        update(&mut model, Msg::MoveCursorLineStart);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_smart_home_no_leading_whitespace() {
        // Line with no leading whitespace: stays at 0
        let mut model = test_model("hello", 0, 0);
        update(&mut model, Msg::MoveCursorLineStart);
        assert_eq!(model.cursor.column, 0); // first_non_ws is 0, so stays at 0
    }

    #[test]
    fn test_smart_home_empty_line() {
        // Empty line: stays at 0
        let mut model = test_model("", 0, 0);
        update(&mut model, Msg::MoveCursorLineStart);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_smart_home_whitespace_only_line() {
        // Whitespace-only line: 0 → end of whitespace
        let mut model = test_model("    ", 0, 0);
        update(&mut model, Msg::MoveCursorLineStart);
        assert_eq!(model.cursor.column, 4); // All whitespace, so first_non_ws is line length
    }

    #[test]
    fn test_smart_end_from_middle() {
        // From middle of line → last non-whitespace
        let mut model = test_model("hello    ", 0, 3);
        update(&mut model, Msg::MoveCursorLineEnd);
        assert_eq!(model.cursor.column, 5); // After 'o' in "hello"
    }

    #[test]
    fn test_smart_end_from_line_end() {
        // From end of line → last non-whitespace
        let mut model = test_model("hello    ", 0, 9);
        update(&mut model, Msg::MoveCursorLineEnd);
        assert_eq!(model.cursor.column, 5); // After 'o' in "hello"
    }

    #[test]
    fn test_smart_end_toggle() {
        // From last non-ws → back to end
        let mut model = test_model("hello    ", 0, 5);
        update(&mut model, Msg::MoveCursorLineEnd);
        assert_eq!(model.cursor.column, 9);
    }

    #[test]
    fn test_smart_end_no_trailing_whitespace() {
        // Line with no trailing whitespace: stays at end
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::MoveCursorLineEnd);
        assert_eq!(model.cursor.column, 5); // last_non_ws = line_end, so stays
    }

    #[test]
    fn test_smart_end_empty_line() {
        // Empty line: stays at 0
        let mut model = test_model("", 0, 0);
        update(&mut model, Msg::MoveCursorLineEnd);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_smart_end_whitespace_only_line() {
        // Whitespace-only line: end → 0 (last_non_ws is 0)
        let mut model = test_model("    ", 0, 4);
        update(&mut model, Msg::MoveCursorLineEnd);
        assert_eq!(model.cursor.column, 0); // No non-whitespace chars
    }

    // ========================================================================
    // Word navigation tests (IntelliJ-style: whitespace is a navigable unit)
    // ========================================================================

    #[test]
    fn test_word_left_from_end() {
        let mut model = test_model("hello world", 0, 11);
        update(&mut model, Msg::MoveCursorWordLeft);

        // Should move to start of "world"
        assert_eq!(model.cursor.column, 6);
    }

    #[test]
    fn test_word_left_stops_at_whitespace_start() {
        // IntelliJ-style: whitespace is its own navigable unit
        // From middle of whitespace, go to start of whitespace (end of "hello")
        let mut model = test_model("hello   world", 0, 8);
        update(&mut model, Msg::MoveCursorWordLeft);

        // Should stop at start of whitespace (end of "hello")
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_word_right_stops_at_word_end() {
        // IntelliJ-style: from start of word, go to END of current word
        let mut model = test_model("hello world", 0, 0);
        update(&mut model, Msg::MoveCursorWordRight);

        // Should move to end of "hello", not past the space
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_word_right_through_whitespace() {
        // From end of "hello" (start of whitespace), go through whitespace to start of "world"
        let mut model = test_model("hello   world", 0, 5);
        update(&mut model, Msg::MoveCursorWordRight);

        // Should stop at end of whitespace (start of "world")
        assert_eq!(model.cursor.column, 8);
    }

    #[test]
    fn test_word_left_through_word() {
        // From start of "world", go to start of whitespace (end of "hello")
        let mut model = test_model("hello   world", 0, 8);
        update(&mut model, Msg::MoveCursorWordLeft);

        // Should stop at start of whitespace
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_word_navigation_full_sequence() {
        // Test full navigation through: "hello     world"
        // Positions: h=0, e=1, l=2, l=3, o=4, ' '=5,6,7,8,9, w=10, o=11, r=12, l=13, d=14
        let mut model = test_model("hello     world", 0, 0);

        // From 0, word right should go to 5 (end of "hello")
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 5);

        // From 5, word right should go to 10 (end of whitespace = start of "world")
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 10);

        // From 10, word right should go to 15 (end of "world")
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 15);

        // From 15, word left should go to 10 (start of "world")
        update(&mut model, Msg::MoveCursorWordLeft);
        assert_eq!(model.cursor.column, 10);

        // From 10, word left should go to 5 (start of whitespace = end of "hello")
        update(&mut model, Msg::MoveCursorWordLeft);
        assert_eq!(model.cursor.column, 5);

        // From 5, word left should go to 0 (start of "hello")
        update(&mut model, Msg::MoveCursorWordLeft);
        assert_eq!(model.cursor.column, 0);
    }

    #[test]
    fn test_word_navigation_with_punctuation() {
        // Test: "hello, world"
        // Positions: h=0, e=1, l=2, l=3, o=4, ,=5, ' '=6, w=7, o=8, r=9, l=10, d=11
        let mut model = test_model("hello, world", 0, 0);

        // From 0, word right should go to 5 (end of "hello")
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 5);

        // From 5, word right should go to 6 (end of punctuation ",")
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 6);

        // From 6, word right should go to 7 (end of space)
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 7);

        // From 7, word right should go to 12 (end of "world")
        update(&mut model, Msg::MoveCursorWordRight);
        assert_eq!(model.cursor.column, 12);
    }

    // ========================================================================
    // Undo/Redo tests
    // ========================================================================

    #[test]
    fn test_undo_insert() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::InsertChar('X'));

        assert_eq!(buffer_to_string(&model), "helloX");

        update(&mut model, Msg::Undo);

        assert_eq!(buffer_to_string(&model), "hello");
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_redo_insert() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::InsertChar('X'));
        update(&mut model, Msg::Undo);
        update(&mut model, Msg::Redo);

        assert_eq!(buffer_to_string(&model), "helloX");
        assert_eq!(model.cursor.column, 6);
    }

    #[test]
    fn test_undo_delete() {
        let mut model = test_model("hello", 0, 5);
        update(&mut model, Msg::DeleteBackward);

        assert_eq!(buffer_to_string(&model), "hell");

        update(&mut model, Msg::Undo);

        assert_eq!(buffer_to_string(&model), "hello");
        assert_eq!(model.cursor.column, 5);
    }

    // ========================================================================
    // set_cursor_from_position tests
    // ========================================================================

    #[test]
    fn test_set_cursor_from_position_first_line() {
        let mut model = test_model("hello\nworld", 0, 0);
        model.set_cursor_from_position(3);

        assert_eq!(model.cursor.line, 0);
        assert_eq!(model.cursor.column, 3);
    }

    #[test]
    fn test_set_cursor_from_position_second_line() {
        let mut model = test_model("hello\nworld", 0, 0);
        model.set_cursor_from_position(8); // "hello\nwo|rld"

        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 2);
    }

    #[test]
    fn test_set_cursor_from_position_at_newline() {
        let mut model = test_model("hello\nworld", 0, 0);
        model.set_cursor_from_position(5); // "hello|" just before newline

        assert_eq!(model.cursor.line, 0);
        assert_eq!(model.cursor.column, 5);
    }

    #[test]
    fn test_set_cursor_from_position_past_end() {
        let mut model = test_model("hello\nworld", 0, 0);
        model.set_cursor_from_position(100);

        // Should clamp to end of buffer
        assert_eq!(model.cursor.line, 1);
        assert_eq!(model.cursor.column, 5); // End of "world"
    }

    // ========================================================================
    // Edge case / regression tests
    // ========================================================================

    #[test]
    fn test_insert_preserves_cursor_buffer_position_consistency() {
        let mut model = test_model("hello world", 0, 6); // "hello |world"

        // After each insert, cursor position should match buffer position
        for ch in "foo".chars() {
            let before_pos = model.cursor_buffer_position();
            update(&mut model, Msg::InsertChar(ch));
            let after_pos = model.cursor_buffer_position();

            // Buffer position should advance by 1
            assert_eq!(after_pos, before_pos + 1);

            // Cursor column should match
            assert_eq!(model.cursor.column, after_pos - 0); // On line 0
        }

        assert_eq!(buffer_to_string(&model), "hello fooworld");
    }

    #[test]
    fn test_multiple_inserts_middle_of_line_no_drift() {
        // This specifically tests the "playing catchup" bug
        let mut model = test_model("the quick brown fox", 0, 10); // "the quick |brown fox"

        let initial_pos = model.cursor_buffer_position();
        assert_eq!(initial_pos, 10);

        // Insert multiple characters and verify no drift
        let insertions = "very ";
        for (i, ch) in insertions.chars().enumerate() {
            update(&mut model, Msg::InsertChar(ch));

            let expected_pos = initial_pos + i + 1;
            let actual_pos = model.cursor_buffer_position();

            assert_eq!(
                actual_pos, expected_pos,
                "After inserting '{}', expected pos {} but got {}",
                ch, expected_pos, actual_pos
            );
        }

        assert_eq!(buffer_to_string(&model), "the quick very brown fox");
    }

    #[test]
    fn test_cursor_column_never_exceeds_line_length_after_operations() {
        let mut model = test_model("hello\nworld", 0, 3);

        // Various operations
        update(&mut model, Msg::InsertChar('X'));
        assert!(model.cursor.column <= model.current_line_length());

        update(&mut model, Msg::DeleteBackward);
        assert!(model.cursor.column <= model.current_line_length());

        update(&mut model, Msg::MoveCursorRight);
        assert!(model.cursor.column <= model.current_line_length());

        update(&mut model, Msg::MoveCursorDown);
        assert!(model.cursor.column <= model.current_line_length());
    }

    #[test]
    fn test_empty_buffer() {
        let mut model = test_model("", 0, 0);

        assert_eq!(model.cursor_buffer_position(), 0);
        assert_eq!(model.current_line_length(), 0);

        update(&mut model, Msg::InsertChar('a'));
        assert_eq!(buffer_to_string(&model), "a");
        assert_eq!(model.cursor.column, 1);
    }

    #[test]
    fn test_single_newline_buffer() {
        let mut model = test_model("\n", 0, 0);

        assert_eq!(model.current_line_length(), 0);

        update(&mut model, Msg::InsertChar('a'));
        assert_eq!(buffer_to_string(&model), "a\n");
    }

    // ========================================================================
    // Scrolling tests - JetBrains-Style Boundary Scrolling
    // ========================================================================

    #[test]
    fn test_scroll_no_scroll_when_content_fits() {
        // Document with fewer lines than viewport
        let mut model = test_model("line1\nline2\nline3\n", 0, 0);
        model.viewport.visible_lines = 25;

        // Move down multiple times - should not scroll
        for _ in 0..3 {
            update(&mut model, Msg::MoveCursorDown);
        }

        assert_eq!(model.viewport.top_line, 0);
        assert_eq!(model.cursor.line, 3);
    }

    #[test]
    fn test_scroll_down_boundary_crossing() {
        // Create 30 lines of text
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_lines = 10;
        model.scroll_padding = 1;

        // Initially at top
        assert_eq!(model.viewport.top_line, 0);
        assert_eq!(model.cursor.line, 0);

        // Move to line 8 (bottom_boundary = top_line + visible_lines - padding - 1 = 0 + 10 - 1 - 1 = 8)
        for _ in 0..8 {
            update(&mut model, Msg::MoveCursorDown);
        }

        // Should not have scrolled yet (cursor at boundary)
        assert_eq!(model.viewport.top_line, 0);
        assert_eq!(model.cursor.line, 8);

        // Move one more line down - should trigger scroll
        update(&mut model, Msg::MoveCursorDown);

        // Viewport should scroll to maintain padding
        // cursor is now at line 9, bottom_boundary was 8, so we need to scroll
        // desired_top = cursor.line + padding + 1 = 9 + 1 + 1 = 11
        // viewport.top_line = (11 - visible_lines) = (11 - 10) = 1
        assert_eq!(model.cursor.line, 9);
        assert_eq!(model.viewport.top_line, 1);
    }

    #[test]
    fn test_scroll_up_boundary_crossing() {
        // Create 30 lines of text
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 15, 0);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 10; // Start scrolled down
        model.scroll_padding = 1;

        // cursor at line 15, top_line at 10
        // top_boundary = top_line + padding = 10 + 1 = 11

        // Move up to line 11 (the boundary)
        for _ in 0..4 {
            update(&mut model, Msg::MoveCursorUp);
        }

        // Should not have scrolled yet (cursor at boundary)
        assert_eq!(model.viewport.top_line, 10);
        assert_eq!(model.cursor.line, 11);

        // Move one more line up - should trigger scroll
        update(&mut model, Msg::MoveCursorUp);

        // Viewport should scroll to maintain padding
        // cursor is now at line 10, should scroll up
        // viewport.top_line = cursor.line - padding = 10 - 1 = 9
        assert_eq!(model.cursor.line, 10);
        assert_eq!(model.viewport.top_line, 9);
    }

    #[test]
    fn test_scroll_mouse_wheel_independent() {
        // Create 50 lines
        let text = (0..50)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 5, 0);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 0;

        // Cursor at line 5, viewport at top
        assert_eq!(model.cursor.line, 5);
        assert_eq!(model.viewport.top_line, 0);

        // Scroll down 10 lines with mouse wheel
        update(&mut model, Msg::ScrollViewport(10));

        // Viewport should move but cursor stays at line 5
        assert_eq!(model.cursor.line, 5);
        assert_eq!(model.viewport.top_line, 10);
    }

    #[test]
    fn test_scroll_snap_back_on_insert() {
        // Create 50 lines
        let text = (0..50)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 5, 0);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 0;

        // Scroll viewport away from cursor using mouse wheel
        update(&mut model, Msg::ScrollViewport(20));
        assert_eq!(model.viewport.top_line, 20);
        assert_eq!(model.cursor.line, 5); // Cursor off-screen

        // Insert a character - should snap back
        update(&mut model, Msg::InsertChar('X'));

        // Viewport should snap to show cursor with padding
        // cursor at line 5, padding = 1
        // Should scroll to show cursor in visible range with padding
        assert_eq!(model.cursor.line, 5);
        assert!(model.viewport.top_line <= 5 - model.scroll_padding);
        assert!(model.viewport.top_line + model.viewport.visible_lines > 5 + model.scroll_padding);
    }

    #[test]
    fn test_scroll_snap_back_on_newline() {
        // Create 50 lines
        let text = (0..50)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 5, 4);
        model.viewport.visible_lines = 10;

        // Scroll viewport away
        update(&mut model, Msg::ScrollViewport(20));
        assert_eq!(model.viewport.top_line, 20);

        // Insert newline - should snap back
        update(&mut model, Msg::InsertNewline);

        // Cursor should be at line 6 now
        assert_eq!(model.cursor.line, 6);
        // Viewport should show cursor with padding
        assert!(model.viewport.top_line <= 6 - model.scroll_padding);
        assert!(model.viewport.top_line + model.viewport.visible_lines > 6 + model.scroll_padding);
    }

    #[test]
    fn test_scroll_padding_configurable() {
        // Test with different padding values
        let text = (0..50)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        // Test with padding = 3
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_lines = 10;
        model.scroll_padding = 3;

        // bottom_boundary = 0 + 10 - 3 - 1 = 6
        // Move to line 6
        for _ in 0..6 {
            update(&mut model, Msg::MoveCursorDown);
        }
        assert_eq!(model.viewport.top_line, 0);

        // Move one more - should scroll
        update(&mut model, Msg::MoveCursorDown);
        assert_eq!(model.cursor.line, 7);
        assert!(model.viewport.top_line > 0);
    }

    #[test]
    fn test_scroll_at_document_boundaries() {
        // Test at start of document
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_lines = 10;

        // Try to scroll up when already at top
        update(&mut model, Msg::MoveCursorUp);
        assert_eq!(model.cursor.line, 0);
        assert_eq!(model.viewport.top_line, 0);

        // Test at end of document
        // Text has 31 lines total (line0 through line29, plus empty line 30 from trailing \n)
        let last_line = model.buffer.len_lines().saturating_sub(1);
        model.cursor.line = last_line;
        model.viewport.top_line = 20;

        // Try to scroll down when at bottom
        update(&mut model, Msg::MoveCursorDown);
        assert_eq!(model.cursor.line, last_line); // Should stay at last line
    }

    #[test]
    fn test_scroll_wheel_boundaries() {
        // Test mouse wheel scrolling respects boundaries
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut model = test_model(&text, 15, 0);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 5;

        // Scroll up past the top
        update(&mut model, Msg::ScrollViewport(-10));
        assert_eq!(model.viewport.top_line, 0);

        // Scroll down past the bottom
        model.viewport.top_line = 15;
        update(&mut model, Msg::ScrollViewport(10));
        // Text has 31 lines (0-30), max_top = 31 - 10 = 21
        let max_top = model
            .buffer
            .len_lines()
            .saturating_sub(model.viewport.visible_lines);
        assert_eq!(model.viewport.top_line, max_top);

        // Try to scroll further down
        update(&mut model, Msg::ScrollViewport(10));
        assert_eq!(model.viewport.top_line, max_top); // Should stay at max
    }

    // ========================================================================
    // Cursor off-screen visibility tests
    // ========================================================================

    #[test]
    fn test_cursor_position_unchanged_during_scroll() {
        // Scrolling viewport should not change cursor position
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let mut model = test_model(&text, 5, 2);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 0;

        // Scroll down - cursor should stay at line 5, column 2
        update(&mut model, Msg::ScrollViewport(10));
        assert_eq!(model.cursor.line, 5);
        assert_eq!(model.cursor.column, 2);
        assert!(model.viewport.top_line > 5); // Viewport moved past cursor
    }

    #[test]
    fn test_cursor_off_screen_above_viewport() {
        // When cursor is above viewport, it should be considered off-screen
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let mut model = test_model(&text, 5, 0);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 10; // Viewport starts at line 10, cursor at line 5

        // Cursor is above viewport - verify positions
        assert!(model.cursor.line < model.viewport.top_line);
    }

    #[test]
    fn test_cursor_off_screen_below_viewport() {
        // When cursor is below viewport, it should be considered off-screen
        let text = (0..30)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let mut model = test_model(&text, 25, 0);
        model.viewport.visible_lines = 10;
        model.viewport.top_line = 0; // Viewport at top, cursor at line 25

        // Cursor is below viewport
        assert!(model.cursor.line >= model.viewport.top_line + model.viewport.visible_lines);
    }

    // ========================================================================
    // Horizontal scroll tests
    // ========================================================================

    #[test]
    fn test_horizontal_scroll_right() {
        let text = "a".repeat(200); // 200 character line
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_columns = 80;
        model.viewport.left_column = 0;

        update(&mut model, Msg::ScrollViewportHorizontal(10));
        assert_eq!(model.viewport.left_column, 10);
    }

    #[test]
    fn test_horizontal_scroll_left() {
        let text = "a".repeat(200);
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_columns = 80;
        model.viewport.left_column = 50;

        update(&mut model, Msg::ScrollViewportHorizontal(-10));
        assert_eq!(model.viewport.left_column, 40);
    }

    #[test]
    fn test_horizontal_scroll_left_boundary() {
        let text = "a".repeat(200);
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_columns = 80;
        model.viewport.left_column = 5;

        // Try to scroll left past 0
        update(&mut model, Msg::ScrollViewportHorizontal(-10));
        assert_eq!(model.viewport.left_column, 0);
    }

    #[test]
    fn test_horizontal_scroll_right_boundary() {
        let text = "a".repeat(100); // 100 char line
        let mut model = test_model(&text, 0, 0);
        model.viewport.visible_columns = 80;
        model.viewport.left_column = 0;

        // Try to scroll right past max (100 - 80 = 20)
        update(&mut model, Msg::ScrollViewportHorizontal(50));
        assert_eq!(model.viewport.left_column, 20); // max_left = 100 - 80 = 20
    }

    #[test]
    fn test_horizontal_scroll_no_scroll_when_content_fits() {
        let text = "short line";
        let mut model = test_model(text, 0, 0);
        model.viewport.visible_columns = 80;
        model.viewport.left_column = 0;

        // Content fits, no scroll should happen
        let result = update(&mut model, Msg::ScrollViewportHorizontal(10));
        assert!(result.is_none());
        assert_eq!(model.viewport.left_column, 0);
    }

    #[test]
    fn test_horizontal_scroll_cursor_position_unchanged() {
        let text = "a".repeat(200);
        let mut model = test_model(&text, 0, 50);
        model.viewport.visible_columns = 80;
        model.viewport.left_column = 0;

        // Scroll right - cursor should stay at column 50
        update(&mut model, Msg::ScrollViewportHorizontal(100));
        assert_eq!(model.cursor.column, 50);
    }

    // ========================================================================
    // PageUp/PageDown tests - Column Preservation
    // ========================================================================

    #[test]
    fn test_page_up_preserves_desired_column() {
        // Create text with lines of varying lengths
        let text = "short\nmedium line\nthis is a very long line\nshort\nmedium\n".to_string()
            + &(0..30)
                .map(|i| format!("line {}", i))
                .collect::<Vec<_>>()
                .join("\n");

        let mut model = test_model(&text, 20, 15); // Start at line 20, column 15
        model.viewport.visible_lines = 10;

        // PageUp should jump ~8 lines (visible_lines - 2)
        update(&mut model, Msg::PageUp);

        // Should be at line 12 now (20 - 8)
        assert_eq!(model.cursor.line, 12);

        // desired_column should be preserved
        assert_eq!(model.cursor.desired_column, Some(15));

        // If line 12 is shorter than 15 chars, column should be clamped
        let line_len = model.current_line_length();
        assert_eq!(model.cursor.column, 15.min(line_len));
    }

    #[test]
    fn test_page_down_preserves_desired_column() {
        let text = (0..50)
            .map(|i| {
                if i % 3 == 0 {
                    "short".to_string()
                } else {
                    format!("this is line number {}", i)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut model = test_model(&text, 10, 18); // Start at line 10, column 18
        model.viewport.visible_lines = 10;

        // PageDown should jump ~8 lines
        update(&mut model, Msg::PageDown);

        // Should be at line 18 now (10 + 8)
        assert_eq!(model.cursor.line, 18);

        // desired_column should be preserved
        assert_eq!(model.cursor.desired_column, Some(18));

        // Column should be clamped if line is shorter
        let line_len = model.current_line_length();
        assert_eq!(model.cursor.column, 18.min(line_len));
    }

    #[test]
    fn test_multiple_page_jumps_preserve_column() {
        let text = (0..100)
            .map(|i| {
                if i % 5 == 0 {
                    "x".to_string() // Very short lines
                } else {
                    format!("this is a longer line number {}", i)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut model = test_model(&text, 51, 25); // Start at line 51 (NOT a multiple of 5)
        model.viewport.visible_lines = 10;

        // PageUp twice
        update(&mut model, Msg::PageUp);
        update(&mut model, Msg::PageUp);

        // PageDown twice (should return to original line)
        update(&mut model, Msg::PageDown);
        update(&mut model, Msg::PageDown);

        // Should be back at line 51
        assert_eq!(model.cursor.line, 51);

        // Column should be restored to 25
        assert_eq!(model.cursor.column, 25);
        assert_eq!(model.cursor.desired_column, Some(25));
    }

    #[test]
    fn test_page_up_to_short_line_clamps_column() {
        let text = "x\ny\nz\n".to_string()  // Lines 0-2 are 1 char
            + &(3..50).map(|i| format!("this is a very long line {}", i)).collect::<Vec<_>>().join("\n");

        let mut model = test_model(&text, 20, 30); // Start at line 20, column 30
        model.viewport.visible_lines = 10;

        // PageUp multiple times to reach short lines at top
        update(&mut model, Msg::PageUp); // Line 12
        update(&mut model, Msg::PageUp); // Line 4

        assert_eq!(model.cursor.line, 4);
        assert_eq!(model.cursor.desired_column, Some(30)); // Remembers 30

        // PageUp once more to line 0 (very short)
        update(&mut model, Msg::PageUp);

        // Should be clamped to line length (1)
        assert!(model.cursor.line <= 2); // One of the short lines
        assert_eq!(model.cursor.column, 1); // Clamped to short line length
        assert_eq!(model.cursor.desired_column, Some(30)); // Still remembers 30

        // PageDown to long line
        update(&mut model, Msg::PageDown);

        // Column should restore toward 30
        let line_len = model.current_line_length();
        assert_eq!(model.cursor.column, 30.min(line_len));
    }
}
