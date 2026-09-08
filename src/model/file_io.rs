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
    /// The initiating document, independent of the subsequently focused tab.
    pub document_id: DocumentId,
    /// Content revision used to reject stale reads.
    pub revision: u64,
    /// Original path used to reject a dialog/read for a replaced document.
    pub source_path: Option<PathBuf>,
    pub(crate) sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileRequestKind {
    Read,
    Write,
    SaveDialog,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FileIoState {
    next: u64,
    latest_read: u64,
    latest_dialog: u64,
    last_saved: u64,
    pending: BTreeMap<u64, FileRequestKind>,
}

impl FileIoState {
    pub fn begin(&mut self, kind: FileRequestKind) -> u64 {
        self.next += 1;
        match kind {
            FileRequestKind::Read => self.latest_read = self.next,
            FileRequestKind::SaveDialog => self.latest_dialog = self.next,
            FileRequestKind::Write => {
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
        match kind {
            FileRequestKind::Read => request.sequence == self.latest_read,
            FileRequestKind::SaveDialog => request.sequence == self.latest_dialog,
            FileRequestKind::Write => request.sequence > self.last_saved,
        }
    }

    pub fn saved(&mut self, request: &FileRequest) {
        self.last_saved = request.sequence;
    }

    pub fn invalidate(&mut self) {
        self.pending.clear();
    }

    pub fn pending(&self, kind: FileRequestKind) -> bool {
        self.pending.values().any(|pending| *pending == kind)
    }
}
