use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use ropey::Rope;
use softbuffer::{Context, Surface};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::application::ApplicationHandler;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::Window;

// Glyph cache key: (character, font_size as bits)
type GlyphCacheKey = (char, u32);
type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

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

    // UI state
    status_message: String,

    // Rendering cache
    line_height: usize,
    char_width: usize,

    // For cursor blinking
    cursor_visible: bool,
    last_cursor_blink: Instant,

    // Undo/Redo stacks
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
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
    fn new(window_width: u32, window_height: u32) -> Self {
        let line_height = 20;
        let char_width = 10;

        Self {
            buffer: Rope::from("Hello, World!\nThis is a text editor built in Rust.\nUsing Elm architecture!\n\nStart typing to edit.\n"),
            cursor: Cursor {
                line: 0,
                column: 0,
                desired_column: None,
            },
            viewport: Viewport {
                top_line: 0,
                left_column: 0,
                visible_lines: (window_height as usize) / line_height,
                visible_columns: (window_width as usize) / char_width,
            },
            window_size: (window_width, window_height),
            status_message: "Ready".to_string(),
            line_height,
            char_width,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
            line.len_chars().saturating_sub(if line.len_chars() > 0 && line.chars().last() == Some('\n') { 1 } else { 0 })
        } else {
            0
        }
    }

    fn line_length(&self, line_idx: usize) -> usize {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            line.len_chars().saturating_sub(if line.len_chars() > 0 && line.chars().last() == Some('\n') { 1 } else { 0 })
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
}

/// Check if a character is a punctuation/symbol boundary (not whitespace)
fn is_punctuation(ch: char) -> bool {
    matches!(ch, '/' | ':' | ',' | '.' | '-' | '(' | ')' | '{' | '}' | '[' | ']' | ';' | '"' | '\'' | '<' | '>' | '=' | '+' | '*' | '&' | '|' | '!' | '@' | '#' | '$' | '%' | '^' | '~' | '`' | '\\' | '?' | '_')
}

/// Character type for word navigation (IntelliJ-style)
#[derive(Debug, Clone, Copy, PartialEq)]
enum CharType {
    Whitespace,
    WordChar,      // Alphanumeric characters
    Punctuation,   // Symbols/boundaries
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
            model.viewport.visible_columns = (width as usize) / model.char_width;
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
                
                // Scroll if needed
                if model.cursor.line < model.viewport.top_line {
                    model.viewport.top_line = model.cursor.line;
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
                
                // Scroll if needed
                if model.cursor.line >= model.viewport.top_line + model.viewport.visible_lines {
                    model.viewport.top_line = model.cursor.line - model.viewport.visible_lines + 1;
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
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorLineStart => {
            model.cursor.column = 0;
            model.cursor.desired_column = None;
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorLineEnd => {
            model.cursor.column = model.current_line_length();
            model.cursor.desired_column = None;
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::InsertChar(ch) => {
            let cursor_before = model.cursor;
            let pos = model.cursor_buffer_position();

            model.buffer.insert_char(pos, ch);
            // Use set_cursor_from_position to ensure cursor.column matches actual buffer position
            // This handles the case where cursor.column was clamped in cursor_buffer_position()
            model.set_cursor_from_position(pos + 1);

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
            model.cursor.line += 1;
            model.cursor.column = 0;
            model.cursor.desired_column = None;

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
                    if prev_line.chars().last() == Some('\n') { 1 } else { 0 }
                );

                model.buffer.remove(pos - 1..pos);
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
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::DeleteForward => {
            let pos = model.cursor_buffer_position();
            if pos < model.buffer.len_chars() {
                let cursor_before = model.cursor;
                let deleted_char = model.buffer.char(pos).to_string();
                model.buffer.remove(pos..pos + 1);

                model.redo_stack.clear();
                model.undo_stack.push(EditOperation::Delete {
                    position: pos,
                    text: deleted_char,
                    cursor_before,
                    cursor_after: model.cursor,
                });
            }
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::MoveCursorDocumentStart => {
            model.cursor.line = 0;
            model.cursor.column = 0;
            model.cursor.desired_column = None;
            model.viewport.top_line = 0;
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
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::PageUp => {
            let jump = model.viewport.visible_lines.saturating_sub(2);
            model.cursor.line = model.cursor.line.saturating_sub(jump);
            model.viewport.top_line = model.viewport.top_line.saturating_sub(jump);
            model.cursor.desired_column = None;
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::PageDown => {
            let jump = model.viewport.visible_lines.saturating_sub(2);
            let max_line = model.buffer.len_lines().saturating_sub(1);
            model.cursor.line = (model.cursor.line + jump).min(max_line);
            if model.cursor.line >= model.viewport.top_line + model.viewport.visible_lines {
                model.viewport.top_line = model.cursor.line.saturating_sub(model.viewport.visible_lines - 1);
            }
            model.cursor.desired_column = None;
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::Undo => {
            if let Some(op) = model.undo_stack.pop() {
                match &op {
                    EditOperation::Insert { position, text, cursor_before, .. } => {
                        model.buffer.remove(*position..*position + text.chars().count());
                        model.cursor = *cursor_before;
                    }
                    EditOperation::Delete { position, text, cursor_before, .. } => {
                        model.buffer.insert(*position, text);
                        model.cursor = *cursor_before;
                    }
                }
                model.redo_stack.push(op);
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        Msg::Redo => {
            if let Some(op) = model.redo_stack.pop() {
                match &op {
                    EditOperation::Insert { position, text, cursor_after, .. } => {
                        model.buffer.insert(*position, text);
                        model.cursor = *cursor_after;
                    }
                    EditOperation::Delete { position, text, cursor_after, .. } => {
                        model.buffer.remove(*position..*position + text.chars().count());
                        model.cursor = *cursor_after;
                    }
                }
                model.undo_stack.push(op);
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
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        Msg::SetCursorPosition(line, column) => {
            model.cursor.line = line;
            model.cursor.column = column;
            model.cursor.desired_column = None;
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
    }
}

fn handle_key(model: &mut Model, key: Key, ctrl: bool, shift: bool, alt: bool, logo: bool) -> Option<Cmd> {
    match key {
        // Undo/Redo (Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y)
        Key::Character(ref s) if ctrl && s.eq_ignore_ascii_case("z") => {
            if shift {
                update(model, Msg::Redo)
            } else {
                update(model, Msg::Undo)
            }
        }
        Key::Character(ref s) if ctrl && s.eq_ignore_ascii_case("y") => {
            update(model, Msg::Redo)
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
            FontSettings::default()
        ).map_err(|e| anyhow::anyhow!("Failed to load font: {}", e))?;

        // Base font size 14pt, scaled for HiDPI
        let font_size = 14.0 * scale_factor as f32;

        // Get font line metrics for proper baseline positioning
        let line_metrics = font.horizontal_line_metrics(font_size)
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

    fn render(&mut self, model: &Model) -> Result<()> {
        // Resize surface if needed
        if self.width != model.window_size.0 || self.height != model.window_size.1 {
            self.width = model.window_size.0;
            self.height = model.window_size.1;
            self.surface.resize(
                NonZeroU32::new(self.width).unwrap(),
                NonZeroU32::new(self.height).unwrap()
            ).map_err(|e| anyhow::anyhow!("Failed to resize surface: {}", e))?;
        }

        let mut buffer = self.surface.buffer_mut()
            .map_err(|e| anyhow::anyhow!("Failed to get surface buffer: {}", e))?;

        // Clear screen (dark background)
        let bg_color = 0xFF1E1E1E;
        buffer.fill(bg_color);

        // Calculate scaled metrics using proper font line metrics
        let font_size = self.font_size;
        let ascent = self.line_metrics.ascent;
        let line_height = self.line_metrics.new_line_size.ceil() as usize;
        let char_width = self.char_width; // Use actual font metrics
        let text_start_x = (char_width * 6.0).round() as usize; // Space for line numbers
        let width = self.width;
        let height = self.height;

        // Render visible lines
        let visible_lines = (height as usize) / line_height;
        let end_line = (model.viewport.top_line + visible_lines).min(model.buffer.len_lines());

        for (screen_line, doc_line) in (model.viewport.top_line..end_line).enumerate() {
            if let Some(line_text) = model.get_line(doc_line) {
                let y = screen_line * line_height;

                // Draw line number (gray)
                let line_num_str = format!("{:4} ", doc_line + 1);
                draw_text(&mut buffer, &self.font, &mut self.glyph_cache, font_size, ascent, width, height, 0, y, &line_num_str, 0xFF606060);

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

                draw_text(&mut buffer, &self.font, &mut self.glyph_cache, font_size, ascent, width, height, text_start_x, y, &display_text, 0xFFE0E0E0);
            }
        }

        // Draw cursor
        if model.cursor_visible {
            let screen_line = model.cursor.line.saturating_sub(model.viewport.top_line);
            if screen_line < visible_lines {
                let cursor_column = model.cursor.column.saturating_sub(model.viewport.left_column);
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
        let status = format!(
            " {} | Ln {}, Col {} ",
            model.status_message,
            model.cursor.line + 1,
            model.cursor.column + 1
        );
        draw_text(&mut buffer, &self.font, &mut self.glyph_cache, font_size, ascent, width, height, 5, status_y + 2, &status, 0xFFB0B0B0);

        buffer.present()
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
        let text_start_x = (self.char_width * 6.0).round() as f64; // Same as render

        // Calculate line from y position
        let visual_line = (y / line_height).floor() as usize;
        let line = model.viewport.top_line + visual_line;
        let line = line.min(model.buffer.len_lines().saturating_sub(1));

        // Calculate column from x position
        let x_offset = x - text_start_x;
        let column = if x_offset > 0.0 {
            (x_offset / char_width).round() as usize
        } else {
            0
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
    y: usize,  // line_top position
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
                let bitmap_idx = (bitmap_y * metrics.width + bitmap_x) as usize;
                if bitmap_idx < bitmap.len() {
                    let alpha = bitmap[bitmap_idx];
                    if alpha > 0 {
                        let px = current_x as isize + bitmap_x as isize + metrics.xmin as isize;
                        let py = (glyph_top + bitmap_y as f32) as isize;

                        if px >= 0 && py >= 0 && (px as usize) < width as usize && (py as usize) < height as usize {
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

                            buffer[py * width as usize + px] = 0xFF000000 | (final_r << 16) | (final_g << 8) | final_b;
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
    fn new(window_width: u32, window_height: u32) -> Self {
        Self {
            model: Model::new(window_width, window_height),
            renderer: None,
            window: None,
            context: None,
            last_tick: Instant::now(),
            modifiers: ModifiersState::empty(),
            mouse_position: None,
        }
    }
    
    fn init_renderer(&mut self, window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<()> {
        self.renderer = Some(Renderer::new(window, context)?);
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
                    handle_key(&mut self.model, event.logical_key.clone(), ctrl, shift, alt, logo)
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
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        return update(&mut self.model, Msg::SetCursorPosition(line, column));
                    }
                }
                None
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
                .with_title("Rust Text Editor - Elm Architecture")
                .with_inner_size(LogicalSize::new(800, 600));
            
            let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
            let context = Context::new(Rc::clone(&window)).unwrap();
            
            self.init_renderer(Rc::clone(&window), &context).unwrap();
            self.window = Some(window);
            self.context = Some(context);
        }
    }
    
    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: winit::window::WindowId, event: WindowEvent) {
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
        // Schedule next wake-up for cursor blink (every 500ms)
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(500)
        ));

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

    let event_loop = EventLoop::new()?;
    let mut app = App::new(800, 600);

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
            status_message: "Test".to_string(),
            line_height: 20,
            char_width: 10,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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

            assert_eq!(actual_pos, expected_pos,
                "After inserting '{}', expected pos {} but got {}",
                ch, expected_pos, actual_pos);
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
}