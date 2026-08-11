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
        }
    }
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
        _ => {
            return Err(format!(
            "unknown automation command `{command}`; use state, document, actions, text, cursor, selection, action, scroll, profile, or syntax-profile"
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
    use super::{document_size_error, MAX_DOCUMENT_SIZE, MAX_MESSAGE_SIZE};
    use token::util::ByteSize;

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
}
