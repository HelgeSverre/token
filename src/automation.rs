//! Cursor-free local automation for the running Token editor.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use token::model::AppModel;
use token::perf::PerfSnapshot;
use token::util::ByteSize;
use winit::event_loop::EventLoopProxy;

const SOCKET_ENV: &str = "TOKEN_AUTOMATION_SOCKET";
pub(crate) const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long discovery waits for one instance to describe itself.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_MESSAGE_SIZE: ByteSize = ByteSize::mebibytes(4);
const MAX_DOCUMENT_SIZE: ByteSize = ByteSize::mebibytes(3);

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AutomationRequest {
    State,
    Document,
    Actions,
    InsertText {
        text: String,
    },
    SetCursor {
        line: usize,
        column: usize,
    },
    SetSelection {
        anchor_line: usize,
        anchor_column: usize,
        head_line: usize,
        head_column: usize,
    },
    ExecuteAction {
        name: String,
    },
    ProfileSyntax {
        text: String,
    },
    Scroll {
        lines: i32,
    },
    ProfileFrames {
        frames: usize,
    },
    /// Type into the active overlay's input (command palette, etc.) — the
    /// only way automation can drive type→filter→accept without a real
    /// keyboard. Errors if no overlay is open.
    SetOverlayInput {
        text: String,
    },
    /// Open files in the running editor (the CLI handoff). Directories
    /// start a separate editor process. With `wait`, the response is
    /// held until every opened document has been closed or the editor
    /// exits — the `--wait` contract git and similar tools rely on.
    OpenPaths {
        paths: Vec<OpenPath>,
        #[serde(default)]
        wait: bool,
    },
}

/// One path in an `OpenPaths` request; `line`/`column` are 1-indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct OpenPath {
    pub path: std::path::PathBuf,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub column: Option<usize>,
}

impl OpenPath {
    /// Parse a CLI argument (`file`, `file:12`, `file:12:3`) into an
    /// absolute `OpenPath` so the request means the same thing in the
    /// editor's process, whatever its working directory is.
    pub(crate) fn from_arg(arg: &std::path::Path) -> Self {
        let (path, position) = token::cli::split_position(arg);
        let path = std::path::absolute(&path).unwrap_or(path);
        Self {
            path,
            line: position.map(|(line, _)| line),
            column: position.map(|(_, column)| column),
        }
    }
}

/// Why a client request did not produce a response.
#[derive(Debug)]
pub(crate) enum RequestError {
    /// Nothing is listening on the automation endpoint.
    NotRunning,
    /// The editor closed the connection before answering (it exited).
    Eof,
    /// `--instance` named an id no running editor advertises.
    NoSuchInstance(u32),
    Other(String),
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => f.write_str("no running Token editor was found"),
            Self::Eof => f.write_str("Token closed the connection before answering"),
            Self::NoSuchInstance(id) => write!(f, "no running Token editor has instance id {id}"),
            Self::Other(message) => f.write_str(message),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EditorSnapshot {
    /// The editor process id; one window per process, so this is also
    /// the instance id that `--instance` / `instance` target.
    pub instance_id: u32,
    pub workspace_root: Option<PathBuf>,
    /// Unix milliseconds of the last time this window gained focus
    /// (process start until then); `0` from `from_model`, the runtime
    /// fills it in. Discovery picks the largest value as the default.
    pub focused_at_ms: u64,
    pub window_width: u32,
    pub window_height: u32,
    pub document_name: String,
    pub revision: u64,
    pub modified: bool,
    pub line_count: usize,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub active_selection_index: usize,
    pub selections: Vec<SelectionSnapshot>,
    pub viewport_top_line: usize,
    pub viewport_left_column: usize,
    /// The active overlay (command palette, etc.), if one is open —
    /// `None` when no modal/overlay is showing.
    pub overlay: Option<OverlaySnapshot>,
    /// Marks-lane gutter state for each visible line, per
    /// editor-decorations.md. Empty today — no mark producer is wired yet.
    pub gutter_marks: Vec<GutterMarkSnapshot>,
    /// The menu-completion popup (autocomplete.md Phase 1), if open —
    /// `None` when no completion popup is showing.
    pub completion: Option<CompletionSnapshot>,
    /// LSP server states (lsp-integration.md), keyed by server id (e.g.
    /// `"rust-analyzer"`) — the render-only mirror `LspMsg::ServerStateChanged`
    /// drives, not the runtime's authoritative `LspManager`.
    pub lsp_servers: Vec<LspServerSnapshot>,
    /// Diagnostics counts for the focused document (lsp-integration.md
    /// Phase 2), queryable independent of the gutter marks visible in
    /// the current viewport.
    pub diagnostics: DiagnosticsSnapshot,
    /// The hover card (lsp-integration.md Phase 4), if open — `None` when
    /// no hover card is showing.
    pub hover: Option<HoverSnapshot>,
    /// The Problems panel, if open — `None` when the bottom dock isn't
    /// showing it.
    pub problems: Option<ProblemsSnapshot>,
    /// The Show Usages / multi-def popup, if open — `None` when
    /// `ui.cursor_overlay`'s kind isn't `References`.
    pub references: Option<ReferencesSnapshot>,
    /// The context menu (context-menu.md), if open — `None` when
    /// `ui.cursor_overlay`'s kind isn't `ContextMenu`.
    pub context_menu: Option<ContextMenuSnapshot>,
}

/// A read-only view of the open context menu, for automation to drive
/// open→navigate→confirm flows without a rendering backend. `rows` walks
/// `context_menu::selectable_items(&state.items)` — the menu's own
/// addressing space (separators excluded, same order `FlatIndex`/
/// `ContextMenuMsg::ActivateItem` use) — so this can never drift from what
/// the view renders or Enter/click activates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextMenuSnapshot {
    pub region: String,
    pub rows: Vec<ContextMenuRowSnapshot>,
    pub selected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextMenuRowSnapshot {
    pub label: String,
    pub enabled: bool,
}

fn context_menu_snapshot(model: &AppModel) -> Option<ContextMenuSnapshot> {
    if !matches!(
        model.ui.cursor_overlay,
        Some(token::model::CursorOverlayState {
            kind: token::model::CursorOverlayKind::ContextMenu,
            ..
        })
    ) {
        return None;
    }
    let selected = model.ui.cursor_overlay.map(|o| o.selected).unwrap_or(0);
    let state = model.ui.context_menu.as_ref()?;
    let rows = token::context_menu::selectable_items(&state.items)
        .map(|item| ContextMenuRowSnapshot {
            label: item.label.clone(),
            enabled: item.enabled,
        })
        .collect();
    let region = match state.region {
        token::context_menu::ContextMenuRegion::Editor => "editor",
        token::context_menu::ContextMenuRegion::EditorTabBar => "editor_tab_bar",
        token::context_menu::ContextMenuRegion::FileTree => "file_tree",
    };
    Some(ContextMenuSnapshot {
        region: region.to_owned(),
        rows,
        selected,
    })
}

/// A read-only view of the Show Usages / multi-def popup, for automation
/// to drive navigate→confirm flows without a rendering backend. `rows`
/// walks the stored `reference_list` — the popup's own ordering authority
/// — so this can never drift from what the view renders or Enter/click
/// activates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReferencesSnapshot {
    pub rows: Vec<ReferenceRowSnapshot>,
    pub selected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReferenceRowSnapshot {
    pub path: String,
    pub line: usize,
    pub col: usize,
}

fn references_snapshot(model: &AppModel) -> Option<ReferencesSnapshot> {
    if !matches!(
        model.ui.cursor_overlay,
        Some(token::model::CursorOverlayState {
            kind: token::model::CursorOverlayKind::References,
            ..
        })
    ) {
        return None;
    }
    let selected = model.ui.cursor_overlay.map(|o| o.selected).unwrap_or(0);
    let rows = model
        .ui
        .reference_list
        .as_ref()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let (line, col) = item.display_position(model);
                    ReferenceRowSnapshot {
                        path: item.path.display().to_string(),
                        line,
                        col,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ReferencesSnapshot { rows, selected })
}

/// A read-only view of the Problems panel, for automation to drive
/// navigate→confirm flows without a rendering backend. `rows` walks
/// `problems_rows(model)` — the panel's own ordering authority — so this
/// can never drift from what the view renders or Enter activates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProblemsSnapshot {
    pub errors: usize,
    pub warnings: usize,
    pub rows: Vec<ProblemsRowSnapshot>,
    pub selected: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProblemsRowSnapshot {
    pub label: String,
    pub kind: String,
}

fn problems_snapshot(model: &AppModel) -> Option<ProblemsSnapshot> {
    model
        .dock_layout
        .active_panel_position(token::panel::PanelId::PROBLEMS)?;
    let (errors, warnings) = token::update::problems::severity_counts(model);
    let rows = token::update::problems::problems_rows(model)
        .into_iter()
        .map(|row| match row {
            token::update::problems::ProblemsRow::File { path, count, .. } => ProblemsRowSnapshot {
                label: format!("{} ({count})", path.display()),
                kind: "file".to_owned(),
            },
            token::update::problems::ProblemsRow::Diagnostic { path, index } => {
                let message = model
                    .lsp
                    .diagnostics
                    .get(&path)
                    .and_then(|diags| diags.get(index))
                    .map(|d| d.message.clone())
                    .unwrap_or_default();
                ProblemsRowSnapshot {
                    label: message,
                    kind: "diagnostic".to_owned(),
                }
            }
        })
        .collect();
    Some(ProblemsSnapshot {
        errors,
        warnings,
        rows,
        selected: model.problems_panel.selected_index,
    })
}

/// Per-severity diagnostic counts for the focused document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DiagnosticsSnapshot {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
}

fn diagnostics_snapshot(document: &token::model::Document) -> DiagnosticsSnapshot {
    let mut snapshot = DiagnosticsSnapshot {
        errors: 0,
        warnings: 0,
        info: 0,
    };
    for diagnostic in &document.diagnostics {
        match token::model::diagnostic_mark(diagnostic.severity) {
            token::model::Mark::Warning => snapshot.warnings += 1,
            token::model::Mark::Info => snapshot.info += 1,
            _ => snapshot.errors += 1,
        }
    }
    snapshot
}

/// One entry in `EditorSnapshot::lsp_servers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LspServerSnapshot {
    pub id: String,
    pub state: String,
}

/// A read-only view of the menu-completion popup, for automation to drive
/// type -> filter -> accept flows without a rendering backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompletionSnapshot {
    pub item_count: usize,
    pub selected: usize,
    /// Labels in filtered/sorted (i.e. on-screen) order.
    pub items: Vec<String>,
}

fn completion_snapshot(model: &AppModel) -> Option<CompletionSnapshot> {
    let menu = model.ui.completion_menu.as_ref()?;
    let selected = model.ui.cursor_overlay.map(|o| o.selected).unwrap_or(0);
    let items = menu
        .filtered
        .iter()
        .filter_map(|(_, idx, _)| menu.items.get(*idx))
        .map(|item| item.label.clone())
        .collect::<Vec<_>>();
    Some(CompletionSnapshot {
        item_count: items.len(),
        selected,
        items,
    })
}

/// A read-only view of the hover card, for automation to assert content
/// (lsp-integration.md Phase 4) without a rendering backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HoverSnapshot {
    /// Plaintext hover content, if the server returned any.
    pub content: Option<String>,
    /// The primary (highest-severity) diagnostic banner shown alongside
    /// the hover content, if any diagnostic covers the cursor.
    pub banner_message: Option<String>,
    /// `relatedInformation` flattened the same way the card renders it
    /// ("note: <message> (<file>:<line>)" per entry), if any.
    pub related_information: Option<String>,
}

fn hover_snapshot(model: &AppModel) -> Option<HoverSnapshot> {
    if !matches!(
        model.ui.cursor_overlay,
        Some(token::model::CursorOverlayState {
            kind: token::model::CursorOverlayKind::Hover,
            ..
        })
    ) {
        return None;
    }
    let doc = model.try_document()?;
    let cursor = model.editor().active_cursor().to_position();
    let diagnostics = token::model::decorations::diagnostics_at_position(doc, cursor);
    Some(HoverSnapshot {
        content: model
            .ui
            .hover_card
            .as_ref()
            .and_then(|s| s.content.as_ref().map(|c| c.text.clone())),
        banner_message: diagnostics.first().map(|d| d.message.clone()),
        related_information: token::view::modal::related_information_text(&diagnostics),
    })
}

/// One line's gutter mark, as reported to automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GutterMarkSnapshot {
    pub line: usize,
    pub mark: String,
}

/// A read-only view of the active overlay, for automation to inspect
/// type→filter→accept flows without a rendering backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OverlaySnapshot {
    pub context: String,
    pub query: String,
    pub active_tab: Option<String>,
    pub rows: Vec<OverlayRowSnapshot>,
    pub selected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OverlayRowSnapshot {
    pub label: String,
    pub section: Option<String>,
}

fn overlay_snapshot(modal: &token::model::ModalState) -> Option<OverlaySnapshot> {
    match modal {
        token::model::ModalState::CommandPalette(state) => {
            use token::model::SearchTab;

            let command_row = |m: &token::model::CommandMatch| OverlayRowSnapshot {
                label: m.def.label.to_owned(),
                section: None,
            };
            let file_row = |m: &token::model::FileMatch| OverlayRowSnapshot {
                label: m.filename.clone(),
                section: None,
            };

            let (rows, selected): (Vec<OverlayRowSnapshot>, usize) = match state.active_tab {
                SearchTab::Files => (
                    state
                        .files
                        .as_ref()
                        .map(|f| f.results.iter().map(file_row).collect())
                        .unwrap_or_default(),
                    state.files.as_ref().map(|f| f.selected_index).unwrap_or(0),
                ),
                SearchTab::All => {
                    // Walk `search_everywhere_sections` — the doc's single
                    // ordering authority — instead of re-deriving group caps
                    // here, so automation's row order can't drift from what
                    // the view actually renders/selects against.
                    let mut commands = state.matches.iter().map(command_row);
                    let mut files = state
                        .files
                        .iter()
                        .flat_map(|f| f.results.iter().map(file_row));
                    let rows = token::update::search_everywhere_sections(state)
                        .into_iter()
                        .flat_map(|(title, len)| {
                            let source: &mut dyn Iterator<Item = OverlayRowSnapshot> =
                                if title == Some("Files") {
                                    &mut files
                                } else {
                                    &mut commands
                                };
                            source
                                .take(len)
                                .map(|row| OverlayRowSnapshot {
                                    section: title.map(|t| t.to_owned()),
                                    ..row
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect();
                    (rows, state.all_selected)
                }
                SearchTab::Symbols => (Vec::new(), 0),
                SearchTab::Commands => (
                    state
                        .matches
                        .iter()
                        .enumerate()
                        .map(|(i, m)| OverlayRowSnapshot {
                            section: (i < state.recent_count).then(|| "Recently Used".to_owned()),
                            ..command_row(m)
                        })
                        .collect(),
                    state.selected_index,
                ),
            };

            Some(OverlaySnapshot {
                context: "command_palette".to_owned(),
                query: state.input(),
                active_tab: Some(format!("{:?}", state.active_tab)),
                rows,
                selected,
            })
        }
        token::model::ModalState::GotoLine(state) => Some(OverlaySnapshot {
            context: "goto_line".to_owned(),
            query: state.input(),
            active_tab: None,
            rows: Vec::new(),
            selected: 0,
        }),
        token::model::ModalState::RenameSymbol(state) => Some(OverlaySnapshot {
            context: "rename_symbol".to_owned(),
            query: state.input(),
            active_tab: None,
            rows: Vec::new(),
            selected: 0,
        }),
        token::model::ModalState::FindReplace(state) => Some(OverlaySnapshot {
            context: "find_replace".to_owned(),
            query: state.query(),
            active_tab: None,
            rows: Vec::new(),
            selected: 0,
        }),
        token::model::ModalState::ThemePicker(state) => Some(OverlaySnapshot {
            context: "theme_picker".to_owned(),
            query: String::new(),
            active_tab: None,
            rows: state
                .themes
                .iter()
                .map(|t| OverlayRowSnapshot {
                    label: t.name.clone(),
                    section: Some(theme_source_title(t.source).to_owned()),
                })
                .collect(),
            selected: state.selected_index,
        }),
        token::model::ModalState::FileFinder(state) => Some(OverlaySnapshot {
            context: "file_finder".to_owned(),
            query: state.input(),
            active_tab: None,
            rows: state
                .results
                .iter()
                .map(|m| OverlayRowSnapshot {
                    label: m.filename.clone(),
                    section: None,
                })
                .collect(),
            selected: state.selected_index,
        }),
        token::model::ModalState::RecentFiles(state) => Some(OverlaySnapshot {
            context: "recent_files".to_owned(),
            query: state.input(),
            active_tab: None,
            rows: state
                .filtered_rows
                .iter()
                .map(|&i| {
                    let entry = &state.entries[i];
                    OverlayRowSnapshot {
                        label: entry
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        section: Some(recent_group_title(entry).to_owned()),
                    }
                })
                .collect(),
            selected: state.selected_index,
        }),
        token::model::ModalState::LspServers(state) => Some(OverlaySnapshot {
            context: "lsp_servers".to_owned(),
            query: String::new(),
            active_tab: None,
            rows: token::lsp::all_server_defs()
                .iter()
                .map(|def| OverlayRowSnapshot {
                    label: def.id.to_owned(),
                    section: None,
                })
                .collect(),
            selected: state.selected_index,
        }),
        token::model::ModalState::LanguagePicker(state) => Some(OverlaySnapshot {
            context: "language_picker".to_owned(),
            query: String::new(),
            active_tab: None,
            rows: token::syntax::LanguageId::all()
                .map(|language| OverlayRowSnapshot {
                    label: language.display_name().to_owned(),
                    section: None,
                })
                .collect(),
            selected: state.selected_index,
        }),
    }
}

fn theme_source_title(source: token::theme::ThemeSource) -> &'static str {
    match source {
        token::theme::ThemeSource::User => "User Themes",
        token::theme::ThemeSource::Builtin => "Built-in Themes",
    }
}

fn recent_group_title(entry: &token::recent_files::RecentEntry) -> &'static str {
    entry.group().title()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SelectionSnapshot {
    pub anchor_line: usize,
    pub anchor_column: usize,
    pub head_line: usize,
    pub head_column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ActionSnapshot {
    pub name: String,
    pub label: String,
    pub keybinding: String,
}

impl EditorSnapshot {
    pub(crate) fn from_model(model: &AppModel) -> Self {
        let document = model.document();
        let cursor = model.editor().active_cursor();
        let viewport = &model.editor().viewport;
        Self {
            instance_id: std::process::id(),
            workspace_root: model.workspace_root().cloned(),
            focused_at_ms: 0,
            window_width: model.window_size.0,
            window_height: model.window_size.1,
            document_name: document.display_name(),
            revision: document.revision,
            modified: document.is_modified,
            line_count: document.line_count(),
            cursor_line: cursor.line,
            cursor_column: cursor.column,
            active_selection_index: model.editor().active_cursor_index,
            selections: model
                .editor()
                .selections
                .iter()
                .map(|selection| SelectionSnapshot {
                    anchor_line: selection.anchor.line,
                    anchor_column: selection.anchor.column,
                    head_line: selection.head.line,
                    head_column: selection.head.column,
                })
                .collect(),
            viewport_top_line: viewport.top_line,
            viewport_left_column: viewport.left_column,
            overlay: model.ui.active_modal.as_ref().and_then(overlay_snapshot),
            gutter_marks: gutter_marks_snapshot(document, viewport),
            completion: completion_snapshot(model),
            lsp_servers: model
                .lsp
                .servers
                .iter()
                .map(|(id, state)| LspServerSnapshot {
                    id: id.0.clone(),
                    state: format!("{state:?}"),
                })
                .collect(),
            diagnostics: diagnostics_snapshot(document),
            hover: hover_snapshot(model),
            problems: problems_snapshot(model),
            references: references_snapshot(model),
            context_menu: context_menu_snapshot(model),
        }
    }
}

/// Marks-lane state for each visible line, per editor-decorations.md.
fn gutter_marks_snapshot(
    document: &token::model::Document,
    viewport: &token::model::Viewport,
) -> Vec<GutterMarkSnapshot> {
    let end_line = viewport
        .top_line
        .saturating_add(viewport.visible_lines)
        .min(document.line_count());

    (viewport.top_line..end_line)
        .filter_map(|line| {
            token::model::collect_line_marks(document, line)
                .mark
                .map(|mark| GutterMarkSnapshot {
                    line,
                    mark: format!("{mark:?}"),
                })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentSnapshot {
    pub text: String,
}

impl DocumentSnapshot {
    pub(crate) fn from_model(model: &AppModel) -> Result<Self, String> {
        let bytes = model.document().buffer.len_bytes();
        if let Some(error) = document_size_error(bytes) {
            return Err(error);
        }
        Ok(Self {
            text: model.document().buffer.to_string(),
        })
    }
}

fn document_size_error(bytes: usize) -> Option<String> {
    (bytes > MAX_DOCUMENT_SIZE.as_usize()).then(|| {
        format!(
            "document is {}; automation reads are limited to {MAX_DOCUMENT_SIZE}",
            ByteSize::bytes(bytes as u64)
        )
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutomationResponse {
    pub ok: bool,
    pub message: String,
    pub state: Option<EditorSnapshot>,
    pub document: Option<DocumentSnapshot>,
    pub performance: Option<PerfSnapshot>,
    pub actions: Option<Vec<ActionSnapshot>>,
    pub syntax_performance: Option<SyntaxPerfSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SyntaxPerfSnapshot {
    pub revision: u64,
    pub snapshot_ms: f64,
    pub queue_ms: f64,
    pub parse_highlight_ms: f64,
    pub parse_ms: Option<f64>,
    pub highlight_ms: Option<f64>,
    pub outline_ms: f64,
    pub worker_total_ms: f64,
    pub outline_extracted: bool,
    pub highlighted_line_count: usize,
    pub replaced_range_count: usize,
    pub apply_ms: f64,
    pub edit_to_present_ms: f64,
}

impl AutomationResponse {
    pub(crate) fn success(
        message: impl Into<String>,
        state: EditorSnapshot,
        performance: PerfSnapshot,
    ) -> Self {
        Self {
            ok: true,
            message: message.into(),
            state: Some(state),
            document: None,
            performance: Some(performance),
            actions: None,
            syntax_performance: None,
        }
    }

    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            state: None,
            document: None,
            performance: None,
            actions: None,
            syntax_performance: None,
        }
    }
}

pub(crate) struct AutomationEnvelope {
    pub request: AutomationRequest,
    pub response_tx: SyncSender<AutomationResponse>,
}

pub(crate) fn start_server(app_tx: Sender<AutomationEnvelope>, proxy: EventLoopProxy<()>) {
    std::thread::spawn(move || {
        if let Err(error) = server_loop(app_tx, proxy) {
            tracing::warn!("automation server stopped: {error}");
        }
    });
}

// ============================================================================
// Endpoints — one per editor process, advertised under `instances/`
// ============================================================================

/// Where one editor process listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Endpoint {
    #[cfg(unix)]
    Socket(PathBuf),
    #[cfg(windows)]
    Tcp(String),
}

/// The directory every running instance advertises itself in. Unix keys
/// it by effective uid; Windows' temp dir is already per user.
pub(crate) fn instances_dir() -> PathBuf {
    #[cfg(unix)]
    {
        std::env::temp_dir()
            .join(format!("token-{}", unsafe { libc::geteuid() }))
            .join("instances")
    }
    #[cfg(windows)]
    {
        std::env::temp_dir().join("token").join("instances")
    }
}

/// The file that advertises `pid` inside `dir`: the socket itself on
/// Unix, a text file holding the loopback port on Windows.
fn endpoint_file(dir: &Path, pid: u32) -> PathBuf {
    #[cfg(unix)]
    {
        dir.join(format!("{pid}.sock"))
    }
    #[cfg(windows)]
    {
        dir.join(format!("{pid}.port"))
    }
}

/// `TOKEN_AUTOMATION_SOCKET` pins both sides to one endpoint and turns
/// discovery off — how tests isolate an editor from the user's own.
fn fixed_endpoint() -> Option<Endpoint> {
    #[cfg(unix)]
    {
        std::env::var_os(SOCKET_ENV).map(|value| Endpoint::Socket(PathBuf::from(value)))
    }
    #[cfg(windows)]
    {
        std::env::var(SOCKET_ENV).ok().map(Endpoint::Tcp)
    }
}

fn endpoint_from_file(path: &Path) -> Option<Endpoint> {
    #[cfg(unix)]
    {
        Some(Endpoint::Socket(path.to_path_buf()))
    }
    #[cfg(windows)]
    {
        let port: u16 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
        Some(Endpoint::Tcp(format!("127.0.0.1:{port}")))
    }
}

/// The advertisement this process wrote, if any, so `exiting` can take
/// it down instead of leaving it for the next client to reap.
fn own_endpoint_file() -> Option<PathBuf> {
    match fixed_endpoint() {
        #[cfg(unix)]
        Some(Endpoint::Socket(path)) => Some(path),
        #[cfg(windows)]
        Some(Endpoint::Tcp(_)) => None,
        None => Some(endpoint_file(&instances_dir(), std::process::id())),
    }
}

pub(crate) fn remove_own_endpoint() {
    if let Some(path) = own_endpoint_file() {
        let _ = std::fs::remove_file(path);
    }
}

/// Create `dir` if needed and insist it is ours and private: the
/// automation socket accepts unauthenticated commands.
#[cfg(unix)]
fn harden_dir(dir: &Path) -> io::Result<()> {
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    match fs::create_dir(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(dir)?;
    if !metadata.file_type().is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "automation directory {} is not a user-owned directory",
                dir.display()
            ),
        ));
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

#[cfg(unix)]
fn server_loop(app_tx: Sender<AutomationEnvelope>, proxy: EventLoopProxy<()>) -> io::Result<()> {
    use std::fs;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixListener;

    let path = match fixed_endpoint() {
        Some(Endpoint::Socket(path)) => path,
        None => {
            let dir = instances_dir();
            if let Some(parent) = dir.parent() {
                harden_dir(parent)?;
            }
            harden_dir(&dir)?;
            endpoint_file(&dir, std::process::id())
        }
    };
    // A leftover with our pid means the pid was recycled from a dead
    // editor (or a test reused a fixed path); nobody can be listening.
    if path.exists() {
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and is not a socket", path.display()),
            ));
        }
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "automation socket {} is not owned by this user",
                    path.display()
                ),
            ));
        }
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    accept_loop(listener.incoming(), app_tx, proxy);
    Ok(())
}

#[cfg(windows)]
fn server_loop(app_tx: Sender<AutomationEnvelope>, proxy: EventLoopProxy<()>) -> io::Result<()> {
    use std::net::TcpListener;
    let listener = match fixed_endpoint() {
        Some(Endpoint::Tcp(address)) => TcpListener::bind(address)?,
        None => {
            let dir = instances_dir();
            std::fs::create_dir_all(&dir)?;
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let port = listener.local_addr()?.port();
            std::fs::write(endpoint_file(&dir, std::process::id()), port.to_string())?;
            listener
        }
    };
    accept_loop(listener.incoming(), app_tx, proxy);
    Ok(())
}

fn accept_loop<S>(
    incoming: impl Iterator<Item = io::Result<S>>,
    app_tx: Sender<AutomationEnvelope>,
    proxy: EventLoopProxy<()>,
) where
    S: Read + Write + Send + 'static,
{
    for stream in incoming {
        match stream {
            Ok(mut stream) => {
                let app_tx = app_tx.clone();
                let proxy = proxy.clone();
                std::thread::spawn(move || {
                    let response = handle_stream(&mut stream, app_tx, &proxy)
                        .unwrap_or_else(|error| AutomationResponse::error(error.to_string()));
                    let _ = serde_json::to_writer(&mut stream, &response);
                    let _ = stream.write_all(b"\n");
                });
            }
            Err(error) => tracing::warn!("automation connection failed: {error}"),
        }
    }
}

fn handle_stream(
    stream: &mut impl io::Read,
    app_tx: Sender<AutomationEnvelope>,
    proxy: &EventLoopProxy<()>,
) -> io::Result<AutomationResponse> {
    let mut line = String::new();
    BufReader::new((&mut *stream).take(MAX_MESSAGE_SIZE.as_u64())).read_line(&mut line)?;
    let request: AutomationRequest = serde_json::from_str(&line).map_err(io::Error::other)?;
    // A `--wait` handoff is answered when the documents close, which can
    // be hours later; every other request keeps the 30 s bound.
    let unbounded = matches!(request, AutomationRequest::OpenPaths { wait: true, .. });
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    app_tx
        .send(AutomationEnvelope {
            request,
            response_tx,
        })
        .map_err(io::Error::other)?;
    proxy.send_event(()).map_err(io::Error::other)?;
    if unbounded {
        response_rx.recv().map_err(io::Error::other)
    } else {
        response_rx
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(io::Error::other)
    }
}

// ============================================================================
// Discovery — which editors are running, and which one a client means
// ============================================================================

/// What one running editor says about itself; the subset of
/// `EditorSnapshot` that routing needs, parsed leniently so an older or
/// newer editor still shows up in the list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct InstanceInfo {
    pub instance_id: u32,
    pub workspace_root: Option<PathBuf>,
    pub document_name: String,
    pub focused_at_ms: u64,
}

#[derive(Deserialize)]
struct DiscoveryReply {
    #[serde(default)]
    state: Option<InstanceInfo>,
}

#[derive(Debug, Clone)]
pub(crate) struct Instance {
    pub info: InstanceInfo,
    pub endpoint: Endpoint,
}

/// Which running editor a request is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Instance(u32),
    /// The editor whose workspace contains the path, else the default.
    ForPath(PathBuf),
    /// The most recently focused editor.
    Default,
}

/// Every running editor, most recently focused first.
pub(crate) fn discover() -> Vec<Instance> {
    match fixed_endpoint() {
        Some(endpoint) => probe(&endpoint)
            .map(|info| vec![Instance { info, endpoint }])
            .unwrap_or_default(),
        None => discover_in(&instances_dir(), None),
    }
}

/// List the editors advertised in `dir`, skipping `exclude` (a caller
/// asking from inside an editor must not wait on itself). Dead
/// advertisements are removed on the way.
pub(crate) fn discover_in(dir: &Path, exclude: Option<u32>) -> Vec<Instance> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let candidates: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let pid: u32 = path.file_stem()?.to_str()?.parse().ok()?;
            (Some(pid) != exclude).then_some((pid, path))
        })
        .collect();
    let mut found = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = candidates
            .iter()
            .map(|(pid, path)| {
                scope.spawn(move || {
                    let Some(endpoint) = endpoint_from_file(path) else {
                        let _ = std::fs::remove_file(path);
                        return None;
                    };
                    match probe(&endpoint) {
                        Ok(info) if info.instance_id == *pid => Some(Instance { info, endpoint }),
                        Ok(_) => None,
                        Err(RequestError::NotRunning) => {
                            let _ = std::fs::remove_file(path);
                            None
                        }
                        Err(_) => None,
                    }
                })
            })
            .collect();
        found.extend(
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok().flatten()),
        );
    });
    found.sort_by_key(|instance| std::cmp::Reverse(instance.info.focused_at_ms));
    found
}

fn probe(endpoint: &Endpoint) -> Result<InstanceInfo, RequestError> {
    let body = exchange(endpoint, &AutomationRequest::State, Some(DISCOVERY_TIMEOUT))?;
    serde_json::from_str::<DiscoveryReply>(&body)
        .map_err(|error| RequestError::Other(format!("invalid response from Token: {error}")))?
        .state
        .ok_or_else(|| RequestError::Other("Token answered without a state".to_owned()))
}

/// Choose among `instances` for `target`; `ForPath` prefers the deepest
/// workspace containing the path and otherwise behaves like `Default`.
pub(crate) fn pick<'a>(instances: &'a [Instance], target: &Target) -> Option<&'a Instance> {
    match target {
        Target::Instance(id) => instances
            .iter()
            .find(|instance| instance.info.instance_id == *id),
        Target::ForPath(path) => {
            let path = canonical_or_parent(path);
            instances
                .iter()
                .filter(|instance| {
                    instance
                        .info
                        .workspace_root
                        .as_deref()
                        .is_some_and(|root| path.starts_with(root))
                })
                .max_by_key(|instance| {
                    instance
                        .info
                        .workspace_root
                        .as_ref()
                        .map_or(0, |root| root.components().count())
                })
                .or_else(|| pick(instances, &Target::Default))
        }
        Target::Default => instances
            .iter()
            .max_by_key(|instance| instance.info.focused_at_ms),
    }
}

/// Canonicalize for prefix matching; a file that does not exist yet
/// borrows its parent's canonical form.
fn canonical_or_parent(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        path.parent()
            .and_then(|parent| parent.canonicalize().ok())
            .map(|parent| match path.file_name() {
                Some(name) => parent.join(name),
                None => parent,
            })
            .unwrap_or_else(|| path.to_path_buf())
    })
}

fn resolve(target: &Target) -> Result<Endpoint, RequestError> {
    if let Some(endpoint) = fixed_endpoint() {
        return Ok(endpoint);
    }
    if let Target::Instance(id) = target {
        let file = endpoint_file(&instances_dir(), *id);
        return file
            .exists()
            .then(|| endpoint_from_file(&file))
            .flatten()
            .ok_or(RequestError::NoSuchInstance(*id));
    }
    let instances = discover();
    pick(&instances, target)
        .map(|instance| instance.endpoint.clone())
        .ok_or(RequestError::NotRunning)
}

// ============================================================================
// Client
// ============================================================================

/// Send one request to a running editor; `None` waits indefinitely for
/// the response (the `--wait` handoff).
pub(crate) fn request_with_timeout(
    target: &Target,
    request: AutomationRequest,
    timeout: Option<Duration>,
) -> Result<AutomationResponse, RequestError> {
    let endpoint = resolve(target)?;
    let body = exchange(&endpoint, &request, timeout)?;
    serde_json::from_str(&body)
        .map_err(|error| RequestError::Other(format!("invalid response from Token: {error}")))
}

/// One request/response line pair over a fresh connection.
fn exchange(
    endpoint: &Endpoint,
    request: &AutomationRequest,
    timeout: Option<Duration>,
) -> Result<String, RequestError> {
    let connect_error = |error: io::Error| match error.kind() {
        io::ErrorKind::NotFound
        | io::ErrorKind::ConnectionRefused
        | io::ErrorKind::AddrNotAvailable => RequestError::NotRunning,
        _ => RequestError::Other(format!("could not connect to Token: {error}")),
    };
    #[cfg(unix)]
    let Endpoint::Socket(path) = endpoint;
    #[cfg(unix)]
    let stream = std::os::unix::net::UnixStream::connect(path).map_err(connect_error)?;
    #[cfg(windows)]
    let Endpoint::Tcp(address) = endpoint;
    #[cfg(windows)]
    let stream = std::net::TcpStream::connect(address).map_err(connect_error)?;
    let other = |error: io::Error| RequestError::Other(error.to_string());
    stream.set_read_timeout(timeout).map_err(other)?;
    let mut writer = stream.try_clone().map_err(other)?;
    serde_json::to_writer(&mut writer, request)
        .map_err(|error| RequestError::Other(error.to_string()))?;
    writer.write_all(b"\n").map_err(other)?;
    let mut body = String::new();
    BufReader::new(stream)
        .take(MAX_MESSAGE_SIZE.as_u64())
        .read_line(&mut body)
        .map_err(other)?;
    if body.is_empty() {
        return Err(RequestError::Eof);
    }
    Ok(body)
}

// ============================================================================
// `token automate` CLI
// ============================================================================

enum CliCommand {
    Instances,
    Request(AutomationRequest),
}

pub(crate) fn run_cli(args: impl Iterator<Item = String>) -> Result<(), String> {
    let (target, command) = parse_cli(args)?;
    let request_value = match command {
        CliCommand::Instances => {
            let infos: Vec<InstanceInfo> = discover()
                .into_iter()
                .map(|instance| instance.info)
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&infos).map_err(|error| error.to_string())?
            );
            return Ok(());
        }
        CliCommand::Request(request) => request,
    };
    let response = request_with_timeout(&target, request_value, Some(RESPONSE_TIMEOUT))
        .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&response).map_err(|error| error.to_string())?
    );
    response.ok.then_some(()).ok_or(response.message)
}

/// `[--instance <id>] <command> [args…]`; the command defaults to `state`.
fn parse_cli(mut args: impl Iterator<Item = String>) -> Result<(Target, CliCommand), String> {
    let mut target = Target::Default;
    let mut command = args.next();
    while matches!(command.as_deref(), Some("--instance" | "-i")) {
        target = Target::Instance(parse_arg(args.next(), "instance id")?);
        command = args.next();
    }
    let command = command.unwrap_or_else(|| "state".to_owned());
    let request = match command.as_str() {
        "instances" => return Ok((target, CliCommand::Instances)),
        "state" => AutomationRequest::State,
        "document" => AutomationRequest::Document,
        "actions" => AutomationRequest::Actions,
        "text" => AutomationRequest::InsertText {
            text: args.collect::<Vec<_>>().join(" "),
        },
        "cursor" => AutomationRequest::SetCursor {
            line: parse_arg(args.next(), "line")?,
            column: parse_arg(args.next(), "column")?,
        },
        "selection" => AutomationRequest::SetSelection {
            anchor_line: parse_arg(args.next(), "anchor line")?,
            anchor_column: parse_arg(args.next(), "anchor column")?,
            head_line: parse_arg(args.next(), "head line")?,
            head_column: parse_arg(args.next(), "head column")?,
        },
        "action" => AutomationRequest::ExecuteAction {
            name: args.next().ok_or_else(|| "missing action name".to_owned())?,
        },
        "syntax-profile" => AutomationRequest::ProfileSyntax {
            text: args.collect::<Vec<_>>().join(" "),
        },
        "scroll" => AutomationRequest::Scroll {
            lines: parse_arg(args.next(), "lines")?,
        },
        "profile" => AutomationRequest::ProfileFrames {
            frames: parse_arg(args.next(), "frames")?,
        },
        "overlay-input" => AutomationRequest::SetOverlayInput {
            text: args.collect::<Vec<_>>().join(" "),
        },
        "open" => AutomationRequest::OpenPaths {
            paths: args
                .map(|arg| OpenPath::from_arg(std::path::Path::new(&arg)))
                .collect(),
            wait: false,
        },
        _ => {
            return Err(format!(
            "unknown automation command `{command}`; use instances, state, document, actions, text, cursor, selection, action, scroll, profile, syntax-profile, overlay-input, or open (prefix with --instance <id> to pick an editor)"
        ))
        }
    };
    Ok((target, CliCommand::Request(request)))
}

fn parse_arg<T: std::str::FromStr>(value: Option<String>, name: &str) -> Result<T, String> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}

#[cfg(test)]
mod tests {
    use super::{
        document_size_error, overlay_snapshot, parse_cli, pick, resolve, AutomationRequest,
        CliCommand, EditorSnapshot, Instance, InstanceInfo, OpenPath, RequestError, Target,
        MAX_DOCUMENT_SIZE, MAX_MESSAGE_SIZE,
    };
    use token::lsp::{LspServerId, ServerState};
    use token::model::ui::{FindReplaceState, GotoLineState, RecentFilesState, ThemePickerState};
    use token::model::{AppModel, ModalState};
    use token::recent_files::RecentEntry;
    use token::util::ByteSize;

    #[test]
    fn snapshot_exposes_lsp_server_states() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model
            .lsp
            .servers
            .insert(LspServerId::from("rust-analyzer"), ServerState::Ready);
        let snapshot = EditorSnapshot::from_model(&model);
        assert_eq!(snapshot.lsp_servers.len(), 1);
        assert_eq!(snapshot.lsp_servers[0].id, "rust-analyzer");
        assert_eq!(snapshot.lsp_servers[0].state, "Ready");
    }

    #[test]
    fn problems_snapshot_is_none_when_the_panel_is_closed() {
        let model = AppModel::new(800, 600, 1.0, vec![]);
        let snapshot = EditorSnapshot::from_model(&model);
        assert!(snapshot.problems.is_none());
    }

    #[test]
    fn problems_snapshot_reports_rows_counts_and_selection_when_open() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model
            .dock_layout
            .bottom
            .activate(token::panel::PanelId::PROBLEMS);
        model.lsp.diagnostics.insert(
            std::path::PathBuf::from("/proj/a.rs"),
            vec![lsp_types::Diagnostic {
                range: lsp_types::Range::default(),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                message: "boom".to_owned(),
                ..Default::default()
            }],
        );
        model.problems_panel.selected_index = Some(1);
        // Current-file filter: the diagnostics' file must be focused.
        model.document_mut().file_path = Some(std::path::PathBuf::from("/proj/a.rs"));

        let snapshot = EditorSnapshot::from_model(&model)
            .problems
            .expect("open Problems panel must report a snapshot");
        assert_eq!(snapshot.errors, 1);
        assert_eq!(snapshot.warnings, 0);
        assert_eq!(snapshot.rows.len(), 2);
        assert_eq!(snapshot.rows[0].kind, "file");
        assert_eq!(snapshot.rows[1].kind, "diagnostic");
        assert_eq!(snapshot.rows[1].label, "boom");
        assert_eq!(snapshot.selected, Some(1));
    }

    #[test]
    fn problems_snapshot_follows_the_panel_to_the_right_dock() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model
            .dock_layout
            .bottom
            .panel_ids
            .retain(|&panel| panel != token::panel::PanelId::PROBLEMS);
        model.dock_layout.bottom.active_index = Some(0);
        model
            .dock_layout
            .right
            .register_panel(token::panel::PanelId::PROBLEMS);
        model
            .dock_layout
            .right
            .activate(token::panel::PanelId::PROBLEMS);

        assert!(EditorSnapshot::from_model(&model).problems.is_some());
    }

    #[test]
    fn automation_limits_preserve_binary_thresholds() {
        assert_eq!(MAX_DOCUMENT_SIZE, ByteSize::mebibytes(3));
        assert_eq!(MAX_MESSAGE_SIZE, ByteSize::mebibytes(4));
        assert!(document_size_error(MAX_DOCUMENT_SIZE.as_usize()).is_none());
        assert_eq!(
            document_size_error(MAX_DOCUMENT_SIZE.as_usize() + 1).as_deref(),
            Some("document is 3.0 MiB; automation reads are limited to 3.0 MiB")
        );
    }

    // overlay-surface.md's Testing Strategy: "Automation: open each
    // context, assert the `overlay` snapshot". Every context must return
    // `Some`, not just CommandPalette — a client reading `state.overlay`
    // on Find/Replace, Go to Line, the theme picker, or recent files must
    // not see `null`.

    #[test]
    fn goto_line_overlay_snapshot_reports_the_query() {
        let mut state = GotoLineState::default();
        state.set_input("42");
        let modal = ModalState::GotoLine(state);
        let snapshot = overlay_snapshot(&modal).expect("Go to Line must report an overlay");
        assert_eq!(snapshot.context, "goto_line");
        assert_eq!(snapshot.query, "42");
    }

    #[test]
    fn find_replace_overlay_snapshot_reports_the_query() {
        let mut state = FindReplaceState::default();
        state.set_query("needle");
        let modal = ModalState::FindReplace(state);
        let snapshot = overlay_snapshot(&modal).expect("Find/Replace must report an overlay");
        assert_eq!(snapshot.context, "find_replace");
        assert_eq!(snapshot.query, "needle");
    }

    #[test]
    fn theme_picker_overlay_snapshot_reports_rows_and_selection() {
        let mut state = ThemePickerState::new("dark".to_owned());
        state.selected_index = 1;
        let expected_len = state.themes.len();
        let modal = ModalState::ThemePicker(state);
        let snapshot = overlay_snapshot(&modal).expect("theme picker must report an overlay");
        assert_eq!(snapshot.context, "theme_picker");
        assert_eq!(snapshot.rows.len(), expected_len);
        assert_eq!(snapshot.selected, 1);
    }

    #[test]
    fn all_tab_overlay_snapshot_matches_search_everywhere_sections_order() {
        use token::model::ui::{FileFinderState, FileMatch};
        use token::model::{CommandPaletteState, SearchTab};

        let mut default_state = CommandPaletteState::default();
        default_state.matches.truncate(2);
        let mut state = CommandPaletteState {
            active_tab: SearchTab::All,
            files_available: true,
            ..default_state
        };
        let root = std::path::PathBuf::from("/ws");
        let mut files = FileFinderState::new(Vec::new(), root.clone());
        files.results = vec![FileMatch::from_path(
            &root.join("a.rs"),
            &root,
            0,
            Vec::new(),
        )];
        state.files = Some(files);

        let sections = token::update::search_everywhere_sections(&state);
        let modal = ModalState::CommandPalette(state);
        let snapshot = overlay_snapshot(&modal).expect("command palette must report an overlay");

        assert_eq!(
            snapshot.rows.len(),
            sections.iter().map(|(_, len)| len).sum::<usize>(),
            "row count must equal search_everywhere_sections' row count"
        );
        // Section boundaries in the snapshot must line up with the
        // ordering authority's own (title, len) groups.
        let mut idx = 0;
        for (title, len) in sections {
            for row in &snapshot.rows[idx..idx + len] {
                assert_eq!(row.section.as_deref(), title);
            }
            idx += len;
        }
    }

    #[test]
    fn recent_files_overlay_snapshot_reports_filtered_rows_in_order() {
        let recent = token::recent_files::RecentFiles {
            version: token::recent_files::RecentFiles::CURRENT_VERSION,
            entries: vec![
                RecentEntry::new("/ws/a.rs".into(), None),
                RecentEntry::new("/ws/b.rs".into(), None),
            ],
        };
        let state = RecentFilesState::new(&recent, None);
        let modal = ModalState::RecentFiles(state);
        let snapshot = overlay_snapshot(&modal).expect("recent files must report an overlay");
        assert_eq!(snapshot.context, "recent_files");
        assert_eq!(snapshot.rows.len(), 2);
        assert_eq!(snapshot.selected, 0);
    }

    #[test]
    fn lsp_servers_overlay_snapshot_reports_a_row_per_registered_server() {
        use token::model::ui::LspServersState;

        let state = LspServersState {
            selected_index: 1,
            scroll_offset: 0,
        };
        let modal = ModalState::LspServers(state);
        let snapshot = overlay_snapshot(&modal).expect("lsp servers must report an overlay");
        assert_eq!(snapshot.context, "lsp_servers");
        assert_eq!(snapshot.rows.len(), token::lsp::all_server_defs().len());
        assert_eq!(snapshot.rows[0].label, token::lsp::all_server_defs()[0].id);
        assert_eq!(snapshot.selected, 1);
    }

    #[test]
    fn open_paths_wait_defaults_to_false() {
        let request: AutomationRequest = serde_json::from_str(
            r#"{"type":"open_paths","paths":[{"path":"/tmp/a.rs","line":3}]}"#,
        )
        .unwrap();
        let AutomationRequest::OpenPaths { paths, wait } = request else {
            panic!("expected open_paths");
        };
        assert!(!wait);
        assert_eq!(
            paths,
            vec![OpenPath {
                path: "/tmp/a.rs".into(),
                line: Some(3),
                column: None,
            }]
        );
    }

    #[test]
    fn open_path_from_arg_is_absolute_with_position() {
        let open = OpenPath::from_arg(std::path::Path::new("definitely/missing.rs:9:4"));
        assert!(open.path.is_absolute());
        assert!(open.path.ends_with("definitely/missing.rs"));
        assert_eq!((open.line, open.column), (Some(9), Some(4)));
    }

    fn instance(id: u32, root: Option<&str>, focused_at_ms: u64) -> Instance {
        Instance {
            info: InstanceInfo {
                instance_id: id,
                workspace_root: root.map(std::path::PathBuf::from),
                document_name: String::new(),
                focused_at_ms,
            },
            #[cfg(unix)]
            endpoint: super::Endpoint::Socket(std::path::PathBuf::from(format!("/{id}.sock"))),
            #[cfg(windows)]
            endpoint: super::Endpoint::Tcp(format!("127.0.0.1:{id}")),
        }
    }

    #[test]
    fn pick_prefers_the_deepest_workspace_containing_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let nested = root.join("nested");
        std::fs::create_dir(&nested).unwrap();
        let instances = [
            instance(1, Some(root.to_str().unwrap()), 100),
            instance(2, Some(nested.to_str().unwrap()), 1),
            instance(3, None, 999),
        ];
        let target = Target::ForPath(nested.join("not-yet-created.rs"));
        assert_eq!(pick(&instances, &target).unwrap().info.instance_id, 2);
        let target = Target::ForPath(root.join("top.rs"));
        assert_eq!(pick(&instances, &target).unwrap().info.instance_id, 1);
    }

    #[test]
    fn pick_falls_back_to_the_most_recently_focused() {
        let instances = [
            instance(1, Some("/definitely/elsewhere"), 5),
            instance(2, None, 50),
            instance(3, None, 7),
        ];
        assert_eq!(
            pick(&instances, &Target::Default).unwrap().info.instance_id,
            2
        );
        let unrelated = Target::ForPath(std::path::PathBuf::from("/nowhere/file.rs"));
        assert_eq!(pick(&instances, &unrelated).unwrap().info.instance_id, 2);
        assert_eq!(
            pick(&instances, &Target::Instance(3))
                .unwrap()
                .info
                .instance_id,
            3
        );
        assert!(pick(&instances, &Target::Instance(4)).is_none());
        assert!(pick(&[], &Target::Default).is_none());
    }

    #[test]
    fn resolve_reports_a_missing_instance() {
        if std::env::var_os(super::SOCKET_ENV).is_some() {
            return; // a fixed endpoint answers every target
        }
        assert!(matches!(
            resolve(&Target::Instance(u32::MAX)),
            Err(RequestError::NoSuchInstance(u32::MAX))
        ));
    }

    #[test]
    fn parse_cli_peels_the_instance_flag() {
        let (target, command) = parse_cli(
            ["--instance", "42", "cursor", "1", "2"]
                .map(String::from)
                .into_iter(),
        )
        .unwrap();
        assert_eq!(target, Target::Instance(42));
        assert!(matches!(
            command,
            CliCommand::Request(AutomationRequest::SetCursor { line: 1, column: 2 })
        ));
        let (target, command) = parse_cli(std::iter::empty()).unwrap();
        assert_eq!(target, Target::Default);
        assert!(matches!(
            command,
            CliCommand::Request(AutomationRequest::State)
        ));
        let (_, command) =
            parse_cli(["-i", "7", "instances"].map(String::from).into_iter()).unwrap();
        assert!(matches!(command, CliCommand::Instances));
        assert!(parse_cli(["--instance", "x"].map(String::from).into_iter()).is_err());
    }

    #[test]
    fn snapshot_exposes_instance_id_and_workspace_root() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let dir = tempfile::tempdir().unwrap();
        model.open_workspace(dir.path().to_path_buf());
        let value = serde_json::to_value(EditorSnapshot::from_model(&model)).unwrap();
        assert_eq!(value["instance_id"], std::process::id());
        assert!(value.get("process_id").is_none());
        assert!(value["workspace_root"].is_string());
        assert_eq!(value["focused_at_ms"], 0);
        let info: InstanceInfo = serde_json::from_value(value).unwrap();
        assert_eq!(info.instance_id, std::process::id());
    }

    /// A fake editor: answers every connection with one canned `state`.
    #[cfg(unix)]
    fn fake_instance(dir: &std::path::Path, id: u32, root: &str, focused_at_ms: u64) {
        use std::io::Write;
        let listener =
            std::os::unix::net::UnixListener::bind(dir.join(format!("{id}.sock"))).unwrap();
        let reply = format!(
            "{{\"ok\":true,\"state\":{{\"instance_id\":{id},\"workspace_root\":\"{root}\",\"focused_at_ms\":{focused_at_ms}}}}}\n"
        );
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let reply = reply.clone();
                std::thread::spawn(move || {
                    let mut stream = stream;
                    let mut line = String::new();
                    let _ = std::io::BufRead::read_line(
                        &mut std::io::BufReader::new(&stream),
                        &mut line,
                    );
                    let _ = stream.write_all(reply.as_bytes());
                });
            }
        });
    }

    #[cfg(unix)]
    #[test]
    fn discover_lists_live_instances_and_reaps_dead_ones() {
        let dir = tempfile::tempdir().unwrap();
        fake_instance(dir.path(), 7, "/a", 5);
        fake_instance(dir.path(), 9, "/b", 50);
        // A socket whose listener is gone, and junk that is not an instance.
        drop(std::os::unix::net::UnixListener::bind(dir.path().join("8.sock")).unwrap());
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();

        let found = super::discover_in(dir.path(), None);
        let ids: Vec<u32> = found.iter().map(|i| i.info.instance_id).collect();
        assert_eq!(ids, vec![9, 7], "most recently focused first");
        assert_eq!(
            found[1].info.workspace_root.as_deref(),
            Some(std::path::Path::new("/a"))
        );
        assert!(!dir.path().join("8.sock").exists(), "dead socket reaped");
        assert!(dir.path().join("7.sock").exists());

        let without_self = super::discover_in(dir.path(), Some(9));
        assert_eq!(without_self.len(), 1);
        assert_eq!(without_self[0].info.instance_id, 7);
    }

    #[cfg(unix)]
    #[test]
    fn discover_drops_an_instance_whose_id_does_not_match_its_file() {
        let dir = tempfile::tempdir().unwrap();
        fake_instance(dir.path(), 11, "/a", 5);
        std::fs::rename(dir.path().join("11.sock"), dir.path().join("12.sock")).unwrap();
        assert!(super::discover_in(dir.path(), None).is_empty());
    }
}
