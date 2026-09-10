//! An explicit close intention survives asynchronous saves, never failed saves.

use std::{collections::VecDeque, path::PathBuf};

use super::{DocumentId, EditorId, TabId};

#[derive(Debug, Clone)]
pub(crate) enum CloseTarget {
    Tabs(Vec<TabId>),
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsavedDocument {
    pub id: DocumentId,
    pub revision: u64,
    pub path: Option<PathBuf>,
    pub cells: Vec<(EditorId, crate::csv::CellPosition, String)>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CloseSaves {
    pub remaining: VecDeque<DocumentId>,
    pub waiting: Option<DocumentId>,
}

/// Save / Discard / Cancel confirmation shared by tab, group and application close.
#[derive(Debug, Clone)]
pub struct UnsavedChangesState {
    pub(crate) target: CloseTarget,
    pub(crate) documents: Vec<UnsavedDocument>,
    pub(crate) saves: Option<CloseSaves>,
    pub selected_index: usize,
}

impl UnsavedChangesState {
    pub fn actions(&self) -> &'static [&'static str] {
        if self.saves.is_some() {
            &["Cancel Closing"]
        } else if self.documents.len() == 1 {
            &["Cancel", "Save", "Discard Changes"]
        } else {
            &["Cancel", "Save All", "Discard All Changes"]
        }
    }

    pub fn title(&self) -> &'static str {
        if self.saves.is_some() {
            "Saving before closing…"
        } else {
            "Save changes before closing?"
        }
    }

    pub fn description(&self) -> String {
        self.documents
            .iter()
            .map(|document| {
                document.path.as_ref().map_or_else(
                    || format!("Untitled {}", document.id.0),
                    |path| path.display().to_string(),
                )
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
