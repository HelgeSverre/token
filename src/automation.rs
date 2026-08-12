//! Cursor-free local automation for the running Token editor.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use token::model::AppModel;
use token::perf::PerfSnapshot;
use token::util::ByteSize;
use winit::event_loop::EventLoopProxy;

const SOCKET_ENV: &str = "TOKEN_AUTOMATION_SOCKET";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EditorSnapshot {
    pub process_id: u32,
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
        content: model.ui.hover_card.as_ref().and_then(|s| s.content.clone()),
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
            process_id: std::process::id(),
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

#[cfg(unix)]
fn server_loop(app_tx: Sender<AutomationEnvelope>, proxy: EventLoopProxy<()>) -> io::Result<()> {
    use std::fs;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixListener;

    let path = socket_path();
    if std::env::var_os(SOCKET_ENV).is_none() {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "socket has no parent directory",
            )
        })?;
        match fs::create_dir(parent) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(parent)?;
        if !metadata.file_type().is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "automation directory {} is not a user-owned directory",
                    parent.display()
                ),
            ));
        }
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
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
        if std::os::unix::net::UnixStream::connect(&path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!(
                    "another Token automation server is using {}",
                    path.display()
                ),
            ));
        }
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    for stream in listener.incoming() {
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
    Ok(())
}

#[cfg(windows)]
fn server_loop(app_tx: Sender<AutomationEnvelope>, proxy: EventLoopProxy<()>) -> io::Result<()> {
    use std::net::TcpListener;
    let listener = TcpListener::bind(endpoint())?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let response = handle_stream(&mut stream, app_tx.clone(), &proxy)
            .unwrap_or_else(|error| AutomationResponse::error(error.to_string()));
        serde_json::to_writer(&mut stream, &response).map_err(io::Error::other)?;
        stream.write_all(b"\n")?;
    }
    Ok(())
}

fn handle_stream(
    stream: &mut impl io::Read,
    app_tx: Sender<AutomationEnvelope>,
    proxy: &EventLoopProxy<()>,
) -> io::Result<AutomationResponse> {
    let mut line = String::new();
    BufReader::new((&mut *stream).take(MAX_MESSAGE_SIZE.as_u64())).read_line(&mut line)?;
    let request = serde_json::from_str(&line).map_err(io::Error::other)?;
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    app_tx
        .send(AutomationEnvelope {
            request,
            response_tx,
        })
        .map_err(io::Error::other)?;
    proxy.send_event(()).map_err(io::Error::other)?;
    response_rx
        .recv_timeout(RESPONSE_TIMEOUT)
        .map_err(io::Error::other)
}

pub(crate) fn request(request: AutomationRequest) -> Result<AutomationResponse, String> {
    #[cfg(unix)]
    let stream = std::os::unix::net::UnixStream::connect(socket_path())
        .map_err(|error| format!("could not connect to Token: {error}"))?;
    #[cfg(windows)]
    let stream = std::net::TcpStream::connect(endpoint())
        .map_err(|error| format!("could not connect to Token: {error}"))?;
    stream
        .set_read_timeout(Some(RESPONSE_TIMEOUT))
        .map_err(|error| error.to_string())?;
    let mut writer = stream.try_clone().map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut writer, &request).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())?;
    serde_json::from_reader(BufReader::new(stream).take(MAX_MESSAGE_SIZE.as_u64()))
        .map_err(|error| format!("invalid response from Token: {error}"))
}

#[cfg(unix)]
fn socket_path() -> std::path::PathBuf {
    std::env::var_os(SOCKET_ENV)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("token-{}/automation.sock", unsafe {
                libc::geteuid()
            }))
        })
}

#[cfg(windows)]
fn endpoint() -> String {
    std::env::var(SOCKET_ENV).unwrap_or_else(|_| "127.0.0.1:49371".to_owned())
}

pub(crate) fn run_cli(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let command = args.next().unwrap_or_else(|| "state".to_owned());
    let request_value = match command.as_str() {
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
        _ => {
            return Err(format!(
            "unknown automation command `{command}`; use state, document, actions, text, cursor, selection, action, scroll, profile, syntax-profile, or overlay-input"
        ))
        }
    };
    let response = request(request_value)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&response).map_err(|error| error.to_string())?
    );
    response.ok.then_some(()).ok_or(response.message)
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
        document_size_error, overlay_snapshot, EditorSnapshot, MAX_DOCUMENT_SIZE, MAX_MESSAGE_SIZE,
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
}
