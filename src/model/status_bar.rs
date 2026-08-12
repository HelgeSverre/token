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
    /// Caret count for multi-cursor (e.g., "4 carets")
    CaretCount,
    /// LSP diagnostics count for the focused document (e.g., "✗ 2 ⚠ 5"),
    /// hidden when clean (lsp-integration.md Phase 2).
    Diagnostics,
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
            SegmentId::Selection
            | SegmentId::CursorPosition
            | SegmentId::LineCount
            | SegmentId::CaretCount
            | SegmentId::Diagnostics => SegmentPosition::Right,
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
                StatusSegment::new(SegmentId::Diagnostics, SegmentContent::Empty).with_priority(70),
                StatusSegment::new(SegmentId::CaretCount, SegmentContent::Empty).with_priority(45),
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
    // Image mode: show image-specific info in status bar
    if let Some(image_state) = model
        .editor_area
        .focused_editor()
        .and_then(|e| e.view_mode.as_image())
    {
        let dims = format!("{}x{}", image_state.width, image_state.height);
        let zoom = format!("{}%", image_state.zoom_percent());
        let file_size = image_state.file_size_display();
        let format = image_state.format.clone();

        model
            .ui
            .status_bar
            .update_segment(SegmentId::CursorPosition, SegmentContent::Text(dims));
        model
            .ui
            .status_bar
            .update_segment(SegmentId::LineCount, SegmentContent::Text(zoom));
        model
            .ui
            .status_bar
            .update_segment(SegmentId::Selection, SegmentContent::Text(file_size));
        model
            .ui
            .status_bar
            .update_segment(SegmentId::CaretCount, SegmentContent::Text(format));
        model
            .ui
            .status_bar
            .update_segment(SegmentId::ModifiedIndicator, SegmentContent::Empty);
        // Image tabs have no diagnostics — clear a stale `✗ n ⚠ n` segment
        // left over from a previously focused text tab.
        model
            .ui
            .status_bar
            .update_segment(SegmentId::Diagnostics, SegmentContent::Empty);
        return;
    }

    // FileName segment
    let filename = model
        .document()
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
    let modified = if model.document().is_modified {
        SegmentContent::Text("*".to_string())
    } else {
        SegmentContent::Empty
    };
    model
        .ui
        .status_bar
        .update_segment(SegmentId::ModifiedIndicator, modified);

    // CursorPosition segment
    let cursor = model.editor().active_cursor();
    let cursor_text = format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1);
    model
        .ui
        .status_bar
        .update_segment(SegmentId::CursorPosition, SegmentContent::Text(cursor_text));

    // LineCount segment
    let line_count = model.document().line_count();
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

    // CaretCount segment (only visible with multiple cursors)
    let caret_content = if model.editor().cursor_count() > 1 {
        SegmentContent::Text(format!("{} carets", model.editor().cursor_count()))
    } else {
        SegmentContent::Empty
    };
    model
        .ui
        .status_bar
        .update_segment(SegmentId::CaretCount, caret_content);

    // Diagnostics segment (lsp-integration.md Phase 2): hidden when clean
    // (which also covers "no server" — no server means no diagnostics).
    let (errors, warnings) = count_diagnostics(&model.document().diagnostics);
    let diagnostics_content = if errors == 0 && warnings == 0 {
        SegmentContent::Empty
    } else {
        SegmentContent::Text(format!("✗ {errors} ⚠ {warnings}"))
    };
    model
        .ui
        .status_bar
        .update_segment(SegmentId::Diagnostics, diagnostics_content);

    // Message of the highest-severity diagnostic under the cursor, in the
    // same segment a status flash uses — a flash (or any explicit
    // `UpdateSegment`) always wins over this fallback; it only refreshes
    // text it owns (previously set by itself) or an empty segment, so it
    // never clobbers an explicit message that isn't backed by
    // `transient_message` (e.g. `UiMsg::UpdateSegment`).
    let owns_status_message = model.ui.status_message_is_diagnostic
        || model
            .ui
            .status_bar
            .get_segment(SegmentId::StatusMessage)
            .is_some_and(|s| s.content.is_empty());
    if model.ui.transient_message.is_none() && owns_status_message {
        let cursor = model.editor().active_cursor();
        let cursor_position = super::editor::Position::new(cursor.line, cursor.column);
        let message_content = match diagnostic_message_at_cursor(model.document(), cursor_position)
        {
            Some(text) => SegmentContent::Text(truncate_status_message(&text)),
            None => SegmentContent::Empty,
        };
        model.ui.status_message_is_diagnostic = !message_content.is_empty();
        model
            .ui
            .status_bar
            .update_segment(SegmentId::StatusMessage, message_content);
    }
}

/// Counts `(errors, warnings)` — diagnostics is the design doc's "✗ n ⚠ n"
/// shape; info/hint aren't counted in this segment.
fn count_diagnostics(diagnostics: &[lsp_types::Diagnostic]) -> (usize, usize) {
    diagnostics.iter().fold(
        (0, 0),
        |(errors, warnings), d| match super::diagnostic_mark(d.severity) {
            super::Mark::Warning => (errors, warnings + 1),
            super::Mark::Info => (errors, warnings),
            _ => (errors + 1, warnings),
        },
    )
}

/// The message of the highest-severity diagnostic whose range contains
/// `cursor`, if any (lsp-integration.md: "readable, not just visible").
/// `relatedInformation` isn't included here — it arrives with the hover
/// card in Phase 4 (`super::decorations::diagnostics_at_position`, shared
/// with this lookup).
fn diagnostic_message_at_cursor(
    document: &super::Document,
    cursor: super::editor::Position,
) -> Option<String> {
    super::decorations::diagnostics_at_position(document, cursor)
        .first()
        .map(|d| d.message.clone())
}

/// Flattens whitespace/newlines and caps the message length so a
/// multi-line `relatedInformation`-heavy message doesn't blow out the
/// status bar.
const MAX_STATUS_MESSAGE_CHARS: usize = 120;

fn truncate_status_message(text: &str) -> String {
    let flattened = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= MAX_STATUS_MESSAGE_CHARS {
        return flattened;
    }
    let mut truncated: String = flattened
        .chars()
        .take(MAX_STATUS_MESSAGE_CHARS.saturating_sub(1))
        .collect();
    truncated.push('…');
    truncated
}

/// Calculate selection info for the Selection segment
fn calculate_selection_info(model: &AppModel) -> SegmentContent {
    // Get the first selection (primary)
    if let Some(selection) = model.editor().selections.first() {
        // Check if there's an actual selection (anchor != head)
        if selection.is_empty() {
            return SegmentContent::Empty;
        }

        // Calculate character count in selection
        let start = selection.start();
        let end = selection.end();
        let start_offset = model.document().cursor_to_offset(start.line, start.column);
        let end_offset = model.document().cursor_to_offset(end.line, end.column);
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

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

    fn diagnostic(
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        severity: DiagnosticSeverity,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position::new(start_line, start_char),
                end: Position::new(end_line, end_char),
            },
            severity: Some(severity),
            message: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn count_diagnostics_splits_errors_and_warnings_and_ignores_info() {
        let diagnostics = vec![
            diagnostic(0, 0, 0, 1, DiagnosticSeverity::ERROR, "e1"),
            diagnostic(0, 0, 0, 1, DiagnosticSeverity::ERROR, "e2"),
            diagnostic(0, 0, 0, 1, DiagnosticSeverity::WARNING, "w1"),
            diagnostic(0, 0, 0, 1, DiagnosticSeverity::INFORMATION, "i1"),
        ];
        assert_eq!(count_diagnostics(&diagnostics), (2, 1));
    }

    #[test]
    fn count_diagnostics_empty_is_zero() {
        assert_eq!(count_diagnostics(&[]), (0, 0));
    }

    #[test]
    fn truncate_status_message_flattens_whitespace_under_limit() {
        let text = "line one\n  line two\tline three";
        assert_eq!(
            truncate_status_message(text),
            "line one line two line three"
        );
    }

    #[test]
    fn truncate_status_message_caps_length_with_ellipsis() {
        let text = "a".repeat(MAX_STATUS_MESSAGE_CHARS + 50);
        let truncated = truncate_status_message(&text);
        assert_eq!(truncated.chars().count(), MAX_STATUS_MESSAGE_CHARS);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn diagnostic_message_at_cursor_picks_highest_severity_in_range() {
        let mut document = crate::model::Document::with_text("hello world\nfoo bar\n");
        document.diagnostics = vec![
            diagnostic(0, 0, 0, 5, DiagnosticSeverity::WARNING, "warn: hello"),
            diagnostic(0, 0, 0, 5, DiagnosticSeverity::ERROR, "error: hello"),
        ];
        let message =
            diagnostic_message_at_cursor(&document, super::super::editor::Position::new(0, 2));
        assert_eq!(message.as_deref(), Some("error: hello"));
    }

    #[test]
    fn diagnostic_message_at_cursor_none_outside_range() {
        let mut document = crate::model::Document::with_text("hello world\n");
        document.diagnostics = vec![diagnostic(
            0,
            0,
            0,
            5,
            DiagnosticSeverity::ERROR,
            "error: hello",
        )];
        let message =
            diagnostic_message_at_cursor(&document, super::super::editor::Position::new(0, 8));
        assert_eq!(message, None);
    }

    #[test]
    fn diagnostic_message_at_cursor_none_for_a_line_an_edit_deleted() {
        // A diagnostic published against a since-shrunk buffer must not
        // be reported as if it were on whatever line it clamps onto.
        let mut document = crate::model::Document::with_text("aaaa\nbbbb");
        document.diagnostics = vec![diagnostic(9, 0, 9, 3, DiagnosticSeverity::ERROR, "boom")];
        let message =
            diagnostic_message_at_cursor(&document, super::super::editor::Position::new(1, 0));
        assert_eq!(message, None);
    }
}
