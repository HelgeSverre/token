//! Model-owned completion session state and typed local identities.

use std::sync::Arc;

/// Monotonic identity for one menu or inline session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// Monotonic identity for asynchronous work within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(pub u64);

/// Stable identity for a menu candidate. Rows and labels are presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CandidateId {
    pub session: SessionId,
    pub serial: u64,
}

impl CandidateId {
    pub const UNASSIGNED: Self = Self {
        session: SessionId(0),
        serial: 0,
    };
}

/// All completion lifecycle state owned by the window model. Menu and inline
/// remain separate sessions, but dismissal and identity allocation have one
/// explicit home.
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub completion_menu: Option<super::menu::CompletionMenuState>,
    pub(crate) completion_commit: Option<super::menu::PendingCommit>,
    pub(crate) completion_path: Option<Arc<super::path::PathRequest>>,
    pub inline_suggestion: Option<super::inline::InlineSuggestionState>,
    pub inline_session: Option<super::provider::InlineSession>,
    pub inline_in_flight: bool,
    pub inline_failures: u32,
    pub next_session_id: u64,
    pub inline_next_request_id: u64,
    pub inline_statistics_failed: bool,
}

impl CompletionState {
    pub fn allocate_session(&mut self) -> Option<SessionId> {
        self.next_session_id = self.next_session_id.checked_add(1)?;
        Some(SessionId(self.next_session_id))
    }
}
