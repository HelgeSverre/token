//! Document-targeted file-operation tokens. Runtime replies carry these back
//! unchanged; update consumes each token once, even after focus moves.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{Document, DocumentId, EditorId, GroupId, TabId};

/// A file/configuration preparation effect. The opaque sequence is consumed once on reply;
/// group, focus and navigation intent stay in the model, not in the worker.
#[derive(Debug, Clone)]
pub struct FileOpenRequest {
    pub source: FileOpenSource,
    pub known_documents: Vec<KnownFile>,
    pub policy: FileOpenPolicy,
    pub editorconfig: bool,
    pub(crate) sequence: u64,
}

/// Read-only snapshot for worker alias resolution, including cached identity.
#[derive(Debug, Clone)]
pub struct KnownFile {
    pub document_id: DocumentId,
    pub path: PathBuf,
    pub identity: Option<crate::util::FileIdentity>,
}

/// A path known now, or a configuration resource resolved by the file worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOpenSource {
    Path(PathBuf),
    Configuration(crate::commands::ConfigResource),
}

impl FileOpenSource {
    /// Available before preparation only for a directly requested path.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path),
            Self::Configuration(_) => None,
        }
    }
}

impl std::fmt::Display for FileOpenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::commands::ConfigResource;
        match self {
            Self::Path(path) => path.display().fmt(f),
            Self::Configuration(resource) => f.write_str(match resource {
                ConfigResource::Directory => "configuration directory",
                ConfigResource::EditorSettings => "editor configuration",
                ConfigResource::Keybindings => "keybindings",
                ConfigResource::Log => "log file",
                ConfigResource::InlineStatistics => "inline completion statistics",
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOpenPolicy {
    CreateOrOpen,
    /// Existing files of any supported view mode (session restore).
    Existing,
    ExistingText,
}

impl FileOpenRequest {
    /// Correlation key for runtime consumers waiting for installation.
    pub fn id(&self) -> u64 {
        self.sequence
    }
}

/// Disk preparation result, either a directory to reveal or a tab to install.
/// Existing documents are never reread, preserving
/// unsaved buffers even when the requested path is a symlink alias.
#[derive(Clone)]
pub enum PreparedFile {
    Directory {
        path: PathBuf,
    },
    Existing {
        document_id: DocumentId,
        path: PathBuf,
    },
    Loaded {
        document: Box<Document>,
        view_mode: super::ViewMode,
        tab_content: super::TabContent,
    },
}

impl std::fmt::Debug for PreparedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Directory { path } => f.debug_struct("Directory").field("path", path).finish(),
            Self::Existing { document_id, path } => f
                .debug_struct("Existing")
                .field("document_id", document_id)
                .field("path", path)
                .finish(),
            Self::Loaded { document, .. } => f
                .debug_struct("Loaded")
                .field("path", &document.file_path)
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum OpenPosition {
    Char { line: usize, column: usize },
    Lsp(lsp_types::Position),
}

#[derive(Debug, Clone)]
pub(crate) struct OpenOrigin {
    pub editor_id: EditorId,
    pub document_id: DocumentId,
    pub revision: u64,
    pub cursor: super::Cursor,
    pub selection: Option<super::Selection>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingFileOpen {
    pub group_id: GroupId,
    pub focus: super::ui::FocusTarget,
    pub active_tab: Option<TabId>,
    pub position: Option<OpenPosition>,
    pub origin: Option<OpenOrigin>,
    pub route_hint: Option<(PathBuf, crate::lsp::LspServerId, PathBuf)>,
    pub policy: FileOpenPolicy,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkspaceEditAction {
    Rename,
    CodeAction {
        title: String,
        command: Option<lsp_types::Command>,
        document_id: Option<DocumentId>,
    },
    Server {
        server_id: crate::lsp::LspServerId,
        root: PathBuf,
        request_id: serde_json::Value,
        label: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PendingWorkspaceEdit {
    pub waiting: std::collections::HashSet<u64>,
    pub expected: Vec<(DocumentId, u64, Option<PathBuf>)>,
    pub failed: bool,
    pub edit: lsp_types::WorkspaceEdit,
    pub action: WorkspaceEditAction,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FileOpenState {
    next: u64,
    pub pending: BTreeMap<u64, PendingFileOpen>,
    pub latest: std::collections::HashMap<GroupId, u64>,
    pub workspace_edits: Vec<PendingWorkspaceEdit>,
}

impl FileOpenState {
    pub fn begin(&mut self, target: PendingFileOpen, activates_tab: bool) -> u64 {
        self.next += 1;
        if activates_tab {
            self.latest.insert(target.group_id, self.next);
        }
        self.pending.insert(self.next, target);
        self.next
    }
}

#[derive(Debug, Clone)]
/// Identity and revision captured when a document starts a file operation.
/// Obtain requests from update commands; only a matching pending token is valid.
pub struct FileRequest {
    pub file_policy: Option<std::sync::Arc<crate::editorconfig::ResolvedFilePolicy>>,
    /// The initiating document, independent of the subsequently focused tab.
    pub document_id: DocumentId,
    /// Content revision used to reject stale reads.
    pub revision: u64,
    /// Original path used to reject a dialog/read for a replaced document.
    pub source_path: Option<PathBuf>,
    /// Cached aliases captured before a Save As destination is resolved.
    pub source_identity: Option<crate::util::FileIdentity>,
    /// Disk precondition for writes; unused by reads and dialogs.
    pub write_guard: FileWriteGuard,
    /// An explicit external reload retains the existing editor view mode.
    pub external_reload: bool,
    pub(crate) sequence: u64,
}

/// Content precondition captured by a save. Native Save As may replace a
/// different destination, but aliases of the original file retain the guard.
#[derive(Debug, Clone)]
pub struct FileWriteGuard {
    /// None means this document expects a new, nonexistent file.
    pub saved: Option<ropey::Rope>,
    /// A preceding save may still be in the ordered worker queue when the
    /// next request is captured, before its reply updates `saved`.
    pub queued: Option<ropey::Rope>,
    /// The user explicitly chose another destination in the native dialog.
    pub save_as: bool,
}

/// A bounded worker observation, distinct from the editor's saved snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskContent {
    Text(ropey::Rope),
    Missing,
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedFile {
    pub content: DiskContent,
    pub identity: Option<crate::util::FileIdentity>,
}

#[derive(Debug, Clone)]
pub struct ExternalFileChange {
    pub observed: ObservedFile,
    pub notified: bool,
}

#[derive(Debug, Clone)]
pub struct FileConflictState {
    pub document_id: DocumentId,
    pub path: PathBuf,
    pub revision: u64,
    pub observed: ObservedFile,
    pub selected_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum FileConflictAction {
    KeepEditing,
    Reload,
    Overwrite,
    SaveAs,
}

impl FileConflictState {
    pub fn actions(&self) -> &'static [FileConflictAction] {
        use FileConflictAction::*;
        match self.observed.content {
            DiskContent::Text(_) => &[KeepEditing, Reload, Overwrite, SaveAs],
            DiskContent::Missing => &[KeepEditing, Overwrite, SaveAs],
            DiskContent::Unavailable(_) => &[KeepEditing, SaveAs],
        }
    }

    pub fn label(&self, action: FileConflictAction) -> &'static str {
        match action {
            FileConflictAction::KeepEditing => "Keep Editing (leave disk unchanged)",
            FileConflictAction::Reload => "Reload from Disk (discard local edits)",
            FileConflictAction::Overwrite
                if matches!(self.observed.content, DiskContent::Missing) =>
            {
                "Recreate File with My Version"
            }
            FileConflictAction::Overwrite => "Overwrite Disk with My Version",
            FileConflictAction::SaveAs => "Save My Version As…",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileRequestKind {
    Read,
    Write,
    SaveDialog,
    Observe,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FileIoState {
    next: u64,
    latest_read: u64,
    latest_dialog: u64,
    latest_observe: u64,
    last_saved: u64,
    pending: BTreeMap<u64, FileRequestKind>,
    last_queued_write: Option<(PathBuf, ropey::Rope)>,
    pub check_again: bool,
}

impl FileIoState {
    pub fn begin(&mut self, kind: FileRequestKind) -> u64 {
        self.next += 1;
        match kind {
            FileRequestKind::Read => {
                self.latest_read = self.next;
                self.invalidate_observation();
            }
            FileRequestKind::Observe => self.latest_observe = self.next,
            FileRequestKind::SaveDialog => self.latest_dialog = self.next,
            FileRequestKind::Write => {
                self.invalidate_observation();
                self.latest_dialog = 0;
                // A save issued after a pending read commits the still-visible
                // buffer. Do not subsequently mark the read's different bytes
                // clean while that queued write replaces them on disk.
                self.latest_read = 0;
            }
        }
        self.pending.insert(self.next, kind);
        self.next
    }

    pub fn finish(&mut self, request: &FileRequest, kind: FileRequestKind) -> bool {
        if self.pending.remove(&request.sequence) != Some(kind) {
            return false;
        }
        if !self.pending(FileRequestKind::Write) {
            self.last_queued_write = None;
        }
        match kind {
            FileRequestKind::Observe => request.sequence == self.latest_observe,
            FileRequestKind::Read => request.sequence == self.latest_read,
            FileRequestKind::SaveDialog => request.sequence == self.latest_dialog,
            FileRequestKind::Write => request.sequence > self.last_saved,
        }
    }

    pub fn saved(&mut self, request: &FileRequest) {
        self.last_saved = request.sequence;
    }

    fn invalidate_observation(&mut self) {
        self.check_again |= self.pending(FileRequestKind::Observe);
        self.latest_observe = 0;
    }

    pub fn invalidate(&mut self) {
        self.pending.clear();
        self.last_queued_write = None;
    }

    pub fn previous_write(&self, path: &Path) -> Option<ropey::Rope> {
        self.last_queued_write
            .as_ref()
            .filter(|(queued_path, _)| queued_path == path)
            .map(|(_, content)| content.clone())
    }

    pub fn queue_write(&mut self, path: PathBuf, content: ropey::Rope) {
        self.last_queued_write = Some((path, content));
    }

    pub fn pending(&self, kind: FileRequestKind) -> bool {
        self.pending.values().any(|pending| *pending == kind)
    }
}
