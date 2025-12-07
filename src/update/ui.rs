//! UI message handlers (status bar, cursor blink, transient messages)

use std::time::Duration;

use crate::commands::Cmd;
use crate::messages::UiMsg;
use crate::model::{AppModel, SegmentContent, SegmentId, TransientMessage};

/// Handle UI messages (status bar, cursor blink)
pub fn update_ui(model: &mut AppModel, msg: UiMsg) -> Option<Cmd> {
    match msg {
        UiMsg::SetStatus(message) => {
            // Legacy: also update the StatusMessage segment
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Text(message.clone()),
            );
            model.ui.set_status(message);
            Some(Cmd::Redraw)
        }

        UiMsg::BlinkCursor => {
            if model.ui.update_cursor_blink(Duration::from_millis(500)) {
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        UiMsg::UpdateSegment { id, content } => {
            model.ui.status_bar.update_segment(id, content);
            Some(Cmd::Redraw)
        }

        UiMsg::SetTransientMessage { text, duration_ms } => {
            let transient = TransientMessage::new(text.clone(), Duration::from_millis(duration_ms));
            model.ui.transient_message = Some(transient);
            // Also update the StatusMessage segment
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Text(text));
            Some(Cmd::Redraw)
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Empty);
            Some(Cmd::Redraw)
        }
    }
}
