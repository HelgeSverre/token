//! Status bar model - segments and layout
//!
//! Implements a structured, segment-based status bar system.

use std::time::{Duration, Instant};

/// Identifier for status bar segments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentId {
    /// File name display
    FileName,
    /// Modified indicator (e.g., "*")
    ModifiedIndicator,
    /// Cursor position (e.g., "Ln 42, Col 15")
    CursorPosition,
    /// Total line count (e.g., "1,234 Ln")
    LineCount,
    /// Selection info (e.g., "(42 chars)")
    Selection,
    /// Transient status messages (e.g., "Saved")
    StatusMessage,
}

/// Position of a segment in the status bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentPosition {
    Left,
    Center,
    Right,
}

/// Content of a segment
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentContent {
    /// Empty/hidden segment
    Empty,
    /// Text content
    Text(String),
}

impl SegmentContent {
    /// Get the display text for this content
    pub fn display_text(&self) -> &str {
        match self {
            SegmentContent::Empty => "",
            SegmentContent::Text(s) => s,
        }
    }

    /// Check if this content is empty (nothing to display)
    pub fn is_empty(&self) -> bool {
        match self {
            SegmentContent::Empty => true,
            SegmentContent::Text(s) => s.is_empty(),
        }
    }

    /// Get the character width of this content
    pub fn char_width(&self) -> usize {
        match self {
            SegmentContent::Empty => 0,
            SegmentContent::Text(s) => s.chars().count(),
        }
    }
}

/// A single segment in the status bar
#[derive(Debug, Clone)]
pub struct StatusSegment {
    /// Unique identifier
    pub id: SegmentId,
    /// Position in the status bar
    pub position: SegmentPosition,
    /// Content to display
    pub content: SegmentContent,
    /// Priority for overflow (higher = keep visible longer)
    pub priority: u8,
    /// Minimum width in characters (0 = flexible)
    pub min_width: usize,
}

impl StatusSegment {
    /// Create a new segment with the given ID and content
    pub fn new(id: SegmentId, content: SegmentContent) -> Self {
        // Determine default position based on segment type
        let position = match id {
            SegmentId::FileName | SegmentId::ModifiedIndicator | SegmentId::StatusMessage => {
                SegmentPosition::Left
            }
            SegmentId::Selection | SegmentId::CursorPosition | SegmentId::LineCount => {
                SegmentPosition::Right
            }
        };

        Self {
            id,
            position,
            content,
            priority: 0,
            min_width: 0,
        }
    }

    /// Set the priority (builder pattern)
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Set the minimum width (builder pattern)
    pub fn with_min_width(mut self, min_width: usize) -> Self {
        self.min_width = min_width;
        self
    }
}

/// The complete status bar state
#[derive(Debug, Clone)]
pub struct StatusBar {
    /// All segments in the status bar
    segments: Vec<StatusSegment>,
    /// Spacing between segments (character units)
    pub separator_spacing: usize,
    /// Padding on each side (character units)
    pub padding: usize,
}

impl StatusBar {
    /// Create a new status bar with default segments
    pub fn new() -> Self {
        Self {
            segments: vec![
                // Left segments
                StatusSegment::new(
                    SegmentId::FileName,
                    SegmentContent::Text("[No Name]".into()),
                )
                .with_priority(100),
                StatusSegment::new(SegmentId::ModifiedIndicator, SegmentContent::Empty)
                    .with_priority(90),
                StatusSegment::new(SegmentId::StatusMessage, SegmentContent::Empty)
                    .with_priority(50),
                // Right segments
                StatusSegment::new(SegmentId::Selection, SegmentContent::Empty).with_priority(40),
                StatusSegment::new(
                    SegmentId::CursorPosition,
                    SegmentContent::Text("Ln 1, Col 1".into()),
                )
                .with_priority(80)
                .with_min_width(12),
                StatusSegment::new(SegmentId::LineCount, SegmentContent::Text("1 Ln".into()))
                    .with_priority(60)
                    .with_min_width(6),
            ],
            separator_spacing: 2,
            padding: 2,
        }
    }

    /// Get a segment by ID (immutable)
    pub fn get_segment(&self, id: SegmentId) -> Option<&StatusSegment> {
        self.segments.iter().find(|s| s.id == id)
    }

    /// Get a segment by ID (mutable)
    pub fn get_segment_mut(&mut self, id: SegmentId) -> Option<&mut StatusSegment> {
        self.segments.iter_mut().find(|s| s.id == id)
    }

    /// Update a segment's content
    pub fn update_segment(&mut self, id: SegmentId, content: SegmentContent) {
        if let Some(segment) = self.get_segment_mut(id) {
            segment.content = content;
        }
    }

    /// Iterate over all segments
    pub fn all_segments(&self) -> impl Iterator<Item = &StatusSegment> {
        self.segments.iter()
    }

    /// Iterate over segments at a specific position
    pub fn segments_by_position(
        &self,
        position: SegmentPosition,
    ) -> impl Iterator<Item = &StatusSegment> {
        self.segments.iter().filter(move |s| s.position == position)
    }

    /// Iterate over visible segments (non-empty content)
    pub fn visible_segments(&self) -> impl Iterator<Item = &StatusSegment> {
        self.segments.iter().filter(|s| !s.content.is_empty())
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Transient Message
// =============================================================================

/// A transient status message that auto-expires
#[derive(Debug, Clone)]
pub struct TransientMessage {
    /// The message text
    pub text: String,
    /// When this message expires
    pub expires_at: Instant,
}

impl TransientMessage {
    /// Create a new transient message with the given duration
    pub fn new(text: impl Into<String>, duration: Duration) -> Self {
        Self {
            text: text.into(),
            expires_at: Instant::now() + duration,
        }
    }

    /// Check if this message has expired
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

// =============================================================================
// Layout
// =============================================================================

/// A rendered segment with calculated position
#[derive(Debug, Clone)]
pub struct RenderedSegment {
    /// Segment identifier
    pub id: SegmentId,
    /// X position in character units
    pub x: usize,
    /// Width in character units
    pub width: usize,
    /// The text content to render
    pub text: String,
}

/// Complete layout of the status bar
#[derive(Debug, Clone)]
pub struct StatusBarLayout {
    /// Left-aligned segments with positions
    pub left: Vec<RenderedSegment>,
    /// Center-aligned segments with positions
    pub center: Vec<RenderedSegment>,
    /// Right-aligned segments with positions
    pub right: Vec<RenderedSegment>,
    /// X positions of separator lines (in character units)
    pub separator_positions: Vec<usize>,
}

impl StatusBar {
    /// Calculate the layout for rendering
    ///
    /// # Arguments
    /// * `available_width` - Total available width in character units
    pub fn layout(&self, available_width: usize) -> StatusBarLayout {
        let mut left_segments = Vec::new();
        let mut right_segments = Vec::new();
        let mut separator_positions = Vec::new();

        // Layout left segments
        let mut left_x = self.padding;
        let mut prev_segment_end: Option<usize> = None;

        for seg in self
            .segments
            .iter()
            .filter(|s| s.position == SegmentPosition::Left)
        {
            if seg.content.is_empty() {
                continue;
            }

            // Add separator spacing if not first segment
            if let Some(prev_end) = prev_segment_end {
                left_x = prev_end + self.separator_spacing;
                // No separators between left segments (per design doc)
            }

            let width = seg.content.char_width();
            let text = seg.content.display_text().to_string();

            left_segments.push(RenderedSegment {
                id: seg.id,
                x: left_x,
                width,
                text,
            });

            prev_segment_end = Some(left_x + width);
        }

        // Layout right segments (from right edge, backwards)
        let mut right_x = available_width.saturating_sub(self.padding);
        let mut prev_segment_start: Option<usize> = None;

        // Iterate in reverse order to position from right edge
        let right_segs: Vec<_> = self
            .segments
            .iter()
            .filter(|s| s.position == SegmentPosition::Right && !s.content.is_empty())
            .collect();

        for seg in right_segs.iter().rev() {
            let width = seg.content.char_width();
            let text = seg.content.display_text().to_string();

            // Add separator if not first (rightmost) segment
            if let Some(prev_start) = prev_segment_start {
                // Record separator position (center of spacing)
                let sep_center = prev_start.saturating_sub(self.separator_spacing / 2);
                separator_positions.push(sep_center);
                right_x = prev_start.saturating_sub(self.separator_spacing);
            }

            right_x = right_x.saturating_sub(width);

            right_segments.push(RenderedSegment {
                id: seg.id,
                x: right_x,
                width,
                text,
            });

            prev_segment_start = Some(right_x);
        }

        // Reverse to get left-to-right order
        right_segments.reverse();
        separator_positions.reverse();

        StatusBarLayout {
            left: left_segments,
            center: vec![], // Not implemented yet
            right: right_segments,
            separator_positions,
        }
    }
}

// =============================================================================
// Sync Function
// =============================================================================

use super::AppModel;

/// Synchronize status bar segments with current document/editor state
pub fn sync_status_bar(model: &mut AppModel) {
    // FileName segment
    let filename = model
        .document
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "[No Name]".to_string());
    model
        .ui
        .status_bar
        .update_segment(SegmentId::FileName, SegmentContent::Text(filename));

    // ModifiedIndicator segment
    let modified = if model.document.is_modified {
        SegmentContent::Text("*".to_string())
    } else {
        SegmentContent::Empty
    };
    model
        .ui
        .status_bar
        .update_segment(SegmentId::ModifiedIndicator, modified);

    // CursorPosition segment
    let cursor = model.editor.cursor();
    let cursor_text = format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1);
    model
        .ui
        .status_bar
        .update_segment(SegmentId::CursorPosition, SegmentContent::Text(cursor_text));

    // LineCount segment
    let line_count = model.document.line_count();
    let line_text = format!("{} Ln", line_count);
    model
        .ui
        .status_bar
        .update_segment(SegmentId::LineCount, SegmentContent::Text(line_text));

    // Selection segment
    let selection_content = calculate_selection_info(model);
    model
        .ui
        .status_bar
        .update_segment(SegmentId::Selection, selection_content);
}

/// Calculate selection info for the Selection segment
fn calculate_selection_info(model: &AppModel) -> SegmentContent {
    // Get the first selection (primary)
    if let Some(selection) = model.editor.selections.first() {
        // Check if there's an actual selection (anchor != head)
        if selection.is_empty() {
            return SegmentContent::Empty;
        }

        // Calculate character count in selection
        let start = selection.start();
        let end = selection.end();
        let start_offset = model.document.cursor_to_offset(start.line, start.column);
        let end_offset = model.document.cursor_to_offset(end.line, end.column);
        let char_count = end_offset.saturating_sub(start_offset);

        if char_count > 0 {
            SegmentContent::Text(format!("({} chars)", char_count))
        } else {
            SegmentContent::Empty
        }
    } else {
        SegmentContent::Empty
    }
}
