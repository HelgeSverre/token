//! Status bar tests - TDD approach
//!
//! Tests for the structured status bar system with segments.

mod common;

use token::model::status_bar::{SegmentContent, SegmentId, StatusBar, StatusSegment};

// =============================================================================
// Phase 1: Core Data Structures
// =============================================================================

#[test]
fn test_segment_id_equality() {
    assert_eq!(SegmentId::FileName, SegmentId::FileName);
    assert_ne!(SegmentId::FileName, SegmentId::CursorPosition);
}

#[test]
fn test_segment_id_all_variants_exist() {
    // Ensure all expected IDs are defined
    let _ids = [
        SegmentId::FileName,
        SegmentId::ModifiedIndicator,
        SegmentId::CursorPosition,
        SegmentId::LineCount,
        SegmentId::Selection,
        SegmentId::StatusMessage,
    ];
}

#[test]
fn test_segment_content_text() {
    let content = SegmentContent::Text("hello".to_string());
    assert_eq!(content.display_text(), "hello");
}

#[test]
fn test_segment_content_empty() {
    let content = SegmentContent::Empty;
    assert_eq!(content.display_text(), "");
}

#[test]
fn test_segment_content_is_empty() {
    assert!(SegmentContent::Empty.is_empty());
    assert!(!SegmentContent::Text("hi".to_string()).is_empty());
    assert!(SegmentContent::Text("".to_string()).is_empty());
}

#[test]
fn test_segment_content_char_width() {
    assert_eq!(SegmentContent::Empty.char_width(), 0);
    assert_eq!(SegmentContent::Text("hello".to_string()).char_width(), 5);
    assert_eq!(SegmentContent::Text("".to_string()).char_width(), 0);
}

#[test]
fn test_status_segment_creation() {
    let segment = StatusSegment::new(
        SegmentId::FileName,
        SegmentContent::Text("test.rs".to_string()),
    );
    assert_eq!(segment.id, SegmentId::FileName);
    assert_eq!(segment.content.display_text(), "test.rs");
}

#[test]
fn test_status_segment_with_priority() {
    let segment = StatusSegment::new(SegmentId::FileName, SegmentContent::Empty).with_priority(10);
    assert_eq!(segment.priority, 10);
}

#[test]
fn test_status_segment_with_min_width() {
    let segment =
        StatusSegment::new(SegmentId::CursorPosition, SegmentContent::Empty).with_min_width(12);
    assert_eq!(segment.min_width, 12);
}

#[test]
fn test_status_bar_new_has_default_segments() {
    let bar = StatusBar::new();
    assert!(bar.get_segment(SegmentId::FileName).is_some());
    assert!(bar.get_segment(SegmentId::CursorPosition).is_some());
    assert!(bar.get_segment(SegmentId::LineCount).is_some());
}

#[test]
fn test_status_bar_get_segment() {
    let bar = StatusBar::new();
    let segment = bar.get_segment(SegmentId::FileName).unwrap();
    assert_eq!(segment.id, SegmentId::FileName);
}

#[test]
fn test_status_bar_get_segment_mut() {
    let mut bar = StatusBar::new();
    {
        let segment = bar.get_segment_mut(SegmentId::FileName).unwrap();
        segment.content = SegmentContent::Text("modified.rs".to_string());
    }
    let segment = bar.get_segment(SegmentId::FileName).unwrap();
    assert_eq!(segment.content.display_text(), "modified.rs");
}

#[test]
fn test_status_bar_update_segment() {
    let mut bar = StatusBar::new();
    bar.update_segment(SegmentId::FileName, SegmentContent::Text("new.rs".to_string()));

    let segment = bar.get_segment(SegmentId::FileName).unwrap();
    assert_eq!(segment.content.display_text(), "new.rs");
}

// =============================================================================
// Phase 2: Collection Operations
// =============================================================================

use token::model::status_bar::SegmentPosition;

#[test]
fn test_segments_by_position_left() {
    let bar = StatusBar::new();
    let left_segments: Vec<_> = bar.segments_by_position(SegmentPosition::Left).collect();

    // FileName, ModifiedIndicator, StatusMessage are on the left
    assert!(left_segments.iter().any(|s| s.id == SegmentId::FileName));
    assert!(left_segments
        .iter()
        .any(|s| s.id == SegmentId::ModifiedIndicator));
    assert!(left_segments
        .iter()
        .any(|s| s.id == SegmentId::StatusMessage));
}

#[test]
fn test_segments_by_position_right() {
    let bar = StatusBar::new();
    let right_segments: Vec<_> = bar.segments_by_position(SegmentPosition::Right).collect();

    // CursorPosition, LineCount, Selection are on the right
    assert!(right_segments
        .iter()
        .any(|s| s.id == SegmentId::CursorPosition));
    assert!(right_segments.iter().any(|s| s.id == SegmentId::LineCount));
    assert!(right_segments.iter().any(|s| s.id == SegmentId::Selection));
}

#[test]
fn test_segments_by_position_center_empty() {
    let bar = StatusBar::new();
    let center_segments: Vec<_> = bar.segments_by_position(SegmentPosition::Center).collect();

    // No center segments by default
    assert!(center_segments.is_empty());
}

#[test]
fn test_visible_segments_filters_empty() {
    let bar = StatusBar::new();
    // ModifiedIndicator starts as Empty
    let visible: Vec<_> = bar.visible_segments().collect();

    // ModifiedIndicator has Empty content, should be filtered out
    assert!(!visible.iter().any(|s| s.id == SegmentId::ModifiedIndicator));
    // FileName has content, should be included
    assert!(visible.iter().any(|s| s.id == SegmentId::FileName));
}

#[test]
fn test_visible_segments_includes_non_empty() {
    let mut bar = StatusBar::new();
    bar.update_segment(
        SegmentId::ModifiedIndicator,
        SegmentContent::Text("*".to_string()),
    );

    let visible: Vec<_> = bar.visible_segments().collect();
    assert!(visible.iter().any(|s| s.id == SegmentId::ModifiedIndicator));
}

#[test]
fn test_all_segments_iteration() {
    let bar = StatusBar::new();
    let all: Vec<_> = bar.all_segments().collect();

    // Should have 6 segments total
    assert_eq!(all.len(), 6);
}

// =============================================================================
// Phase 3: Sync Function
// =============================================================================

use common::{test_model, test_model_with_selection};
use std::path::PathBuf;
use token::model::status_bar::sync_status_bar;

#[test]
fn test_sync_filename_from_path() {
    let mut model = test_model("hello", 0, 0);
    model.document.file_path = Some(PathBuf::from("/path/to/test.rs"));

    sync_status_bar(&mut model);

    let segment = model.ui.status_bar.get_segment(SegmentId::FileName).unwrap();
    assert_eq!(segment.content.display_text(), "test.rs");
}

#[test]
fn test_sync_filename_no_path() {
    let mut model = test_model("hello", 0, 0);
    model.document.file_path = None;

    sync_status_bar(&mut model);

    let segment = model.ui.status_bar.get_segment(SegmentId::FileName).unwrap();
    assert_eq!(segment.content.display_text(), "[No Name]");
}

#[test]
fn test_sync_modified_indicator_clean() {
    let mut model = test_model("hello", 0, 0);
    model.document.is_modified = false;

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::ModifiedIndicator)
        .unwrap();
    assert!(segment.content.is_empty());
}

#[test]
fn test_sync_modified_indicator_dirty() {
    let mut model = test_model("hello", 0, 0);
    model.document.is_modified = true;

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::ModifiedIndicator)
        .unwrap();
    assert_eq!(segment.content.display_text(), "*");
}

#[test]
fn test_sync_cursor_position() {
    let mut model = test_model("hello\nworld", 1, 3);

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::CursorPosition)
        .unwrap();
    // Line 2, Col 4 (1-indexed display)
    assert_eq!(segment.content.display_text(), "Ln 2, Col 4");
}

#[test]
fn test_sync_line_count() {
    let mut model = test_model("line1\nline2\nline3", 0, 0);

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::LineCount)
        .unwrap();
    assert_eq!(segment.content.display_text(), "3 Ln");
}

#[test]
fn test_sync_line_count_single() {
    let mut model = test_model("one line", 0, 0);

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::LineCount)
        .unwrap();
    assert_eq!(segment.content.display_text(), "1 Ln");
}

#[test]
fn test_sync_selection_empty() {
    let mut model = test_model("hello world", 0, 5);
    // No selection - cursor only (anchor == head)

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::Selection)
        .unwrap();
    assert!(segment.content.is_empty());
}

#[test]
fn test_sync_selection_with_chars() {
    let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::Selection)
        .unwrap();
    assert_eq!(segment.content.display_text(), "(5 chars)");
}

#[test]
fn test_sync_selection_multiline() {
    // Selection spanning multiple lines
    let mut model = test_model_with_selection("hello\nworld\ntest", 0, 0, 2, 4);

    sync_status_bar(&mut model);

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::Selection)
        .unwrap();
    // Should show character count for multi-line selection
    let text = segment.content.display_text();
    assert!(text.contains("chars"), "Expected chars count, got: {}", text);
}

// =============================================================================
// Phase 4: Messages & Transient Message System
// =============================================================================

use std::time::Duration;
use token::messages::{Msg, UiMsg};
use token::model::status_bar::TransientMessage;
use token::update::update;

#[test]
fn test_transient_message_creation() {
    let msg = TransientMessage::new("Saved!", Duration::from_millis(3000));
    assert_eq!(msg.text, "Saved!");
    assert!(!msg.is_expired());
}

#[test]
fn test_transient_message_is_expired() {
    // Create a message that already expired (0 duration)
    let msg = TransientMessage::new("Test", Duration::from_millis(0));
    // Sleep briefly to ensure it's expired
    std::thread::sleep(Duration::from_millis(1));
    assert!(msg.is_expired());
}

#[test]
fn test_update_segment_message() {
    let mut model = test_model("hello", 0, 0);

    let _ = update(
        &mut model,
        Msg::Ui(UiMsg::UpdateSegment {
            id: SegmentId::StatusMessage,
            content: SegmentContent::Text("Custom status".to_string()),
        }),
    );

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::StatusMessage)
        .unwrap();
    assert_eq!(segment.content.display_text(), "Custom status");
}

#[test]
fn test_set_transient_message() {
    let mut model = test_model("hello", 0, 0);

    let _ = update(
        &mut model,
        Msg::Ui(UiMsg::SetTransientMessage {
            text: "Loading...".to_string(),
            duration_ms: 3000,
        }),
    );

    assert!(model.ui.transient_message.is_some());
    let transient = model.ui.transient_message.as_ref().unwrap();
    assert_eq!(transient.text, "Loading...");
}

#[test]
fn test_clear_transient_message() {
    let mut model = test_model("hello", 0, 0);

    // Set transient
    let _ = update(
        &mut model,
        Msg::Ui(UiMsg::SetTransientMessage {
            text: "Loading...".to_string(),
            duration_ms: 3000,
        }),
    );
    assert!(model.ui.transient_message.is_some());

    // Clear it
    let _ = update(&mut model, Msg::Ui(UiMsg::ClearTransientMessage));

    assert!(model.ui.transient_message.is_none());
}

#[test]
fn test_transient_updates_status_message_segment() {
    let mut model = test_model("hello", 0, 0);

    // Set transient message
    let _ = update(
        &mut model,
        Msg::Ui(UiMsg::SetTransientMessage {
            text: "Saving...".to_string(),
            duration_ms: 3000,
        }),
    );

    // StatusMessage segment should be updated
    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::StatusMessage)
        .unwrap();
    assert_eq!(segment.content.display_text(), "Saving...");
}

#[test]
fn test_clear_transient_clears_segment() {
    let mut model = test_model("hello", 0, 0);

    // Set then clear
    let _ = update(
        &mut model,
        Msg::Ui(UiMsg::SetTransientMessage {
            text: "Temp".to_string(),
            duration_ms: 3000,
        }),
    );
    let _ = update(&mut model, Msg::Ui(UiMsg::ClearTransientMessage));

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::StatusMessage)
        .unwrap();
    assert!(segment.content.is_empty());
}

// =============================================================================
// Phase 5: Layout Algorithm
// =============================================================================

#[test]
fn test_layout_returns_rendered_segments() {
    let bar = StatusBar::new();
    let layout = bar.layout(800); // 800 chars width

    // Should have some rendered segments
    assert!(!layout.left.is_empty() || !layout.right.is_empty());
}

#[test]
fn test_layout_left_segments_start_from_padding() {
    let bar = StatusBar::new();
    let layout = bar.layout(800);

    // Left segments should start near the padding
    if let Some(first) = layout.left.first() {
        assert_eq!(first.x, bar.padding);
    }
}

#[test]
fn test_layout_right_segments_end_near_right_edge() {
    let bar = StatusBar::new();
    let layout = bar.layout(100);

    // Right segments should end near the right edge (within padding)
    if let Some(last) = layout.right.last() {
        let segment_end = last.x + last.width;
        assert!(
            segment_end <= 100 - bar.padding + 1,
            "Segment ends at {}, but width is 100 with padding {}",
            segment_end,
            bar.padding
        );
    }
}

#[test]
fn test_layout_excludes_empty_segments() {
    let bar = StatusBar::new();
    let layout = bar.layout(800);

    // ModifiedIndicator starts empty, should not be in layout
    let has_modified = layout
        .left
        .iter()
        .chain(layout.right.iter())
        .any(|s| s.id == SegmentId::ModifiedIndicator);
    assert!(!has_modified, "Empty segments should not appear in layout");
}

#[test]
fn test_layout_includes_non_empty_segments() {
    let mut bar = StatusBar::new();
    bar.update_segment(
        SegmentId::ModifiedIndicator,
        SegmentContent::Text("*".to_string()),
    );
    let layout = bar.layout(800);

    // Now ModifiedIndicator should be in layout
    let has_modified = layout
        .left
        .iter()
        .any(|s| s.id == SegmentId::ModifiedIndicator);
    assert!(has_modified, "Non-empty segments should appear in layout");
}

#[test]
fn test_layout_segment_width_matches_content() {
    let mut bar = StatusBar::new();
    bar.update_segment(
        SegmentId::FileName,
        SegmentContent::Text("test.rs".to_string()),
    );
    let layout = bar.layout(800);

    let filename_segment = layout.left.iter().find(|s| s.id == SegmentId::FileName);
    assert!(filename_segment.is_some());
    let seg = filename_segment.unwrap();
    // "test.rs" is 7 chars
    assert_eq!(seg.width, 7);
}

#[test]
fn test_layout_separator_positions() {
    let mut bar = StatusBar::new();
    // Ensure we have multiple visible left segments
    bar.update_segment(
        SegmentId::ModifiedIndicator,
        SegmentContent::Text("*".to_string()),
    );
    let layout = bar.layout(800);

    // With multiple right segments (CursorPosition, LineCount), we should have separators
    // Separators should be positioned between segments
    if layout.right.len() >= 2 {
        assert!(
            !layout.separator_positions.is_empty(),
            "Should have separator positions between multiple segments"
        );
    }
}

#[test]
fn test_layout_left_segments_ordered_correctly() {
    let mut bar = StatusBar::new();
    bar.update_segment(
        SegmentId::ModifiedIndicator,
        SegmentContent::Text("*".to_string()),
    );
    bar.update_segment(
        SegmentId::StatusMessage,
        SegmentContent::Text("msg".to_string()),
    );
    let layout = bar.layout(800);

    // Left segments should be ordered: FileName, ModifiedIndicator, StatusMessage
    let positions: Vec<_> = layout.left.iter().map(|s| (s.id, s.x)).collect();

    // Verify each subsequent segment starts after the previous
    for window in positions.windows(2) {
        let (_, x1) = window[0];
        let (_, x2) = window[1];
        assert!(x2 > x1, "Segments should be ordered left to right");
    }
}

// =============================================================================
// Phase 8: Backward Compatibility
// =============================================================================

#[test]
fn test_set_status_backward_compatibility() {
    let mut model = test_model("hello", 0, 0);

    // Use old API
    let _ = update(&mut model, Msg::Ui(UiMsg::SetStatus("Hello World".to_string())));

    // Should update both legacy field and segment
    assert_eq!(model.ui.status_message, "Hello World");

    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::StatusMessage)
        .unwrap();
    assert_eq!(segment.content.display_text(), "Hello World");
}

#[test]
fn test_status_bar_syncs_after_cursor_movement() {
    let mut model = test_model("hello\nworld", 0, 0);

    // Move cursor
    let _ = update(
        &mut model,
        Msg::Editor(token::messages::EditorMsg::MoveCursor(
            token::messages::Direction::Down,
        )),
    );

    // Status bar should be synced automatically
    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::CursorPosition)
        .unwrap();
    assert!(
        segment.content.display_text().contains("Ln 2"),
        "Cursor position should be synced. Got: {}",
        segment.content.display_text()
    );
}

#[test]
fn test_status_bar_syncs_after_edit() {
    let mut model = test_model("hello", 0, 5);
    model.document.is_modified = false;

    // Make an edit
    let _ = update(
        &mut model,
        Msg::Document(token::messages::DocumentMsg::InsertChar('!')),
    );

    // Modified indicator should be synced
    let segment = model
        .ui
        .status_bar
        .get_segment(SegmentId::ModifiedIndicator)
        .unwrap();
    assert_eq!(
        segment.content.display_text(),
        "*",
        "Modified indicator should show after edit"
    );
}
