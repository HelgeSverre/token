//! Document-targeted save preparation, independent of the focused pane.

use std::{path::PathBuf, sync::Arc};

use super::{Document, DocumentId};

/// Why a document is being saved. Automatic triggers never open native dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveReason {
    Manual,
    SaveAs,
    Idle,
    FocusLoss,
}

impl SaveReason {
    pub fn is_automatic(self) -> bool {
        matches!(self, Self::Idle | Self::FocusLoss)
    }
}

/// A save continuation. Only the document's current token may finish preparation.
#[derive(Debug, Clone)]
pub struct SaveIntent(Arc<SaveIntentData>);

/// Copy-on-write save parameters keep messages small while each continuation
/// retains its own settings snapshot and the same supersession token.
#[derive(Debug, Clone)]
pub struct SaveIntentData {
    pub document_id: DocumentId,
    pub revision: u64,
    pub(crate) language: crate::syntax::LanguageId,
    pub(crate) text_policy_generation: u64,
    pub(crate) resolution_generation: u64,
    pub(crate) settings: super::DocumentTextSettings,
    pub destination_policy: Option<Arc<crate::editorconfig::ResolvedFilePolicy>>,
    pub source_path: Option<PathBuf>,
    pub path: PathBuf,
    pub reason: SaveReason,
    pub(crate) automatic_policy: Option<crate::config::AutoSaveConfig>,
    token: Arc<()>,
}

impl SaveIntent {
    pub(crate) fn new(
        document: &Document,
        document_id: DocumentId,
        path: PathBuf,
        reason: SaveReason,
        automatic_policy: Option<crate::config::AutoSaveConfig>,
    ) -> Self {
        Self(Arc::new(SaveIntentData {
            document_id,
            revision: document.revision,
            language: document.language,
            text_policy_generation: document.text_policy_generation,
            resolution_generation: document.file_policy.generation,
            settings: document.text_settings,
            destination_policy: None,
            source_path: document.file_path.clone(),
            path,
            reason,
            automatic_policy,
            token: Arc::new(()),
        }))
    }

    pub(crate) fn is_current(&self, document: &Document) -> bool {
        document.file_path == self.source_path
            && document
                .pending_save
                .as_ref()
                .is_some_and(|pending| Arc::ptr_eq(&self.token, &pending.token))
    }
}

impl std::ops::Deref for SaveIntent {
    type Target = SaveIntentData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SaveIntent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}
