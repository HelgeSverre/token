//! Webview manager for preview panes
//!
//! Manages wry WebView instances that overlay the editor window for rich preview.
//! Supports both Markdown (rendered to HTML) and HTML files (with local resource loading).

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use winit::window::Window;
use wry::{Rect, WebView, WebViewBuilder};

use token::model::editor_area::PreviewId;
use token::view::PreviewSnapshots;

mod backdrop;
mod snapshot;

struct SnapshotResult {
    preview_id: PreviewId,
    generation: u64,
    image: anyhow::Result<image::RgbaImage>,
}

/// Content source for a preview - either generated HTML or a file with base directory
#[derive(Clone)]
pub enum PreviewContent {
    /// Pre-rendered HTML content (e.g., from Markdown)
    Html(String),
    /// HTML file with base directory for resolving relative resources
    HtmlFile {
        /// The HTML content to display
        html: String,
        /// Base directory for resolving relative resource paths
        base_dir: PathBuf,
    },
}

/// Shared state for custom protocol handler
struct ProtocolState {
    /// Current HTML content indexed by preview ID
    contents: HashMap<PreviewId, PreviewContent>,
}

type SharedProtocolState = Arc<RwLock<ProtocolState>>;

/// Manages webview instances for preview panes
pub struct WebviewManager {
    /// Active webviews indexed by preview ID
    webviews: HashMap<PreviewId, WebView>,
    /// Shared state for custom protocol handler
    protocol_state: SharedProtocolState,
    backdrop: backdrop::Backdrop,
    snapshot_tx: mpsc::Sender<SnapshotResult>,
    snapshot_rx: mpsc::Receiver<SnapshotResult>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    visible: bool,
}

impl WebviewManager {
    pub fn new() -> Self {
        let (snapshot_tx, snapshot_rx) = mpsc::channel();
        Self {
            webviews: HashMap::new(),
            protocol_state: Arc::new(RwLock::new(ProtocolState {
                contents: HashMap::new(),
            })),
            backdrop: backdrop::Backdrop::default(),
            snapshot_tx,
            snapshot_rx,
            wake: None,
            visible: true,
        }
    }

    pub fn with_wake(wake: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        Self {
            wake,
            ..Self::new()
        }
    }

    /// Capture before hiding native children. The caller keeps the last frame
    /// on screen until captures finish or the short deadline expires.
    pub fn prepare_overlay(&mut self, active: bool, now: Instant) -> bool {
        if !active {
            self.backdrop.resume();
            return true;
        }
        if !self.backdrop.active {
            self.backdrop.begin(self.webviews.keys().copied(), now);
            let generation = self.backdrop.generation;
            for (&preview_id, webview) in &self.webviews {
                let tx = self.snapshot_tx.clone();
                let wake = self.wake.clone();
                let result = snapshot::capture(webview, move |image| {
                    if tx
                        .send(SnapshotResult {
                            preview_id,
                            generation,
                            image,
                        })
                        .is_ok()
                    {
                        if let Some(wake) = wake {
                            wake();
                        }
                    }
                });
                if let Err(error) = result {
                    tracing::warn!(?preview_id, %error, "Could not capture preview backdrop");
                    self.backdrop.finish(preview_id, generation, None);
                }
            }
        }
        self.poll_snapshots(now);
        self.backdrop.deadline().is_none()
    }

    pub fn poll_snapshots(&mut self, now: Instant) -> bool {
        // Expire first so results delivered after the deadline cannot replace
        // an already displayed fallback with a stale capture.
        let mut changed = self.backdrop.expire(now);
        if changed {
            tracing::debug!("Preview backdrop capture timed out; using native fallback");
        }
        while let Ok(result) = self.snapshot_rx.try_recv() {
            let image = match result.image {
                Ok(image) => Some(image),
                Err(error) => {
                    tracing::warn!(preview = ?result.preview_id, %error, "Preview backdrop capture failed");
                    None
                }
            };
            let dimensions = image.as_ref().map(image::RgbaImage::dimensions);
            let accepted = self
                .backdrop
                .finish(result.preview_id, result.generation, image);
            tracing::debug!(preview = ?result.preview_id, ?dimensions, accepted, "Preview backdrop capture completed");
            changed |= accepted;
        }
        changed
    }

    pub fn snapshot_deadline(&self) -> Option<Instant> {
        self.backdrop.deadline()
    }

    pub fn snapshots(&self) -> &PreviewSnapshots {
        &self.backdrop.images
    }

    /// Create a new webview for a preview pane with custom protocol support
    pub fn create_webview(
        &mut self,
        preview_id: PreviewId,
        window: &Rc<Window>,
        bounds: token::model::editor_area::Rect,
        content: PreviewContent,
    ) -> Result<(), wry::Error> {
        // Don't create duplicate
        if self.webviews.contains_key(&preview_id) {
            return Ok(());
        }

        // Store content for protocol handler
        if let Ok(mut state) = self.protocol_state.write() {
            state.contents.insert(preview_id, content);
        }

        let scale_factor = window.scale_factor();
        let protocol_state = Arc::clone(&self.protocol_state);
        let pid = preview_id;

        let webview = WebViewBuilder::new()
            .with_custom_protocol("token".to_string(), move |_webview_id, request| {
                handle_protocol_request(&protocol_state, pid, request)
            })
            .with_url(format!("token://preview-{}/index.html", preview_id.0))
            .with_bounds(to_wry_rect(bounds, scale_factor))
            .with_transparent(false)
            .with_visible(!self.backdrop.active)
            .with_navigation_handler(|url| {
                // Open external links in the default browser
                if token::util::is_web_url(&url) {
                    super::open_web_url(url);
                    false
                } else {
                    // Allow internal navigation (token://, anchor links)
                    true
                }
            })
            .build_as_child(window)?;

        self.webviews.insert(preview_id, webview);
        Ok(())
    }

    /// Update webview content
    pub fn update_content(
        &mut self,
        preview_id: PreviewId,
        content: PreviewContent,
    ) -> anyhow::Result<()> {
        let webview = self
            .webviews
            .get(&preview_id)
            .ok_or_else(|| anyhow::anyhow!("Preview webview {preview_id:?} is missing"))?;
        // Update stored content
        {
            let mut state = self
                .protocol_state
                .write()
                .map_err(|_| anyhow::anyhow!("Preview content lock is poisoned"))?;
            state.contents.insert(preview_id, content);
        }

        // Reload the webview to pick up new content
        let url = format!("token://preview-{}/index.html", preview_id.0);
        webview.load_url(&url)?;
        self.backdrop.remove(preview_id);
        Ok(())
    }

    /// Update webview bounds (position and size)
    pub fn update_bounds(
        &self,
        preview_id: PreviewId,
        bounds: token::model::editor_area::Rect,
        scale_factor: f64,
    ) {
        if let Some(webview) = self.webviews.get(&preview_id) {
            let wry_rect = to_wry_rect(bounds, scale_factor);
            let _ = webview.set_bounds(wry_rect);
        }
    }

    /// Close and remove a webview
    pub fn close_webview(&mut self, preview_id: PreviewId) {
        self.webviews.remove(&preview_id);
        self.backdrop.remove(preview_id);
        if let Ok(mut state) = self.protocol_state.write() {
            state.contents.remove(&preview_id);
        }
    }

    /// Check if a webview exists for a preview
    pub fn has_webview(&self, preview_id: PreviewId) -> bool {
        self.webviews.contains_key(&preview_id)
    }

    /// Get all active preview IDs
    pub fn active_previews(&self) -> Vec<PreviewId> {
        self.webviews.keys().copied().collect()
    }

    /// Set visibility for all webviews (hide when modals are shown)
    pub fn set_all_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        let mut succeeded = true;
        for webview in self.webviews.values() {
            if let Err(error) = webview.set_visible(visible) {
                tracing::warn!(%error, visible, "Could not change preview visibility");
                succeeded = false;
            }
        }
        if succeeded {
            self.visible = visible;
        }
    }
}

impl Default for WebviewManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle custom protocol requests
fn handle_protocol_request(
    state: &SharedProtocolState,
    preview_id: PreviewId,
    request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<Cow<'static, [u8]>> {
    use wry::http::Response;

    let path = request.uri().path();

    // Get content for this preview
    let state_guard = match state.read() {
        Ok(s) => s,
        Err(_) => return error_response(500, "Internal error"),
    };

    let content = match state_guard.contents.get(&preview_id) {
        Some(c) => c.clone(),
        None => return error_response(404, "Preview not found"),
    };

    drop(state_guard); // Release lock before I/O

    match content {
        PreviewContent::Html(html) => {
            // Simple HTML content - serve the HTML for any request
            if path == "/index.html" || path == "/" {
                Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Cow::Owned(html.into_bytes()))
                    .unwrap_or_else(|_| error_response(500, "Response error"))
            } else {
                error_response(404, "Not found")
            }
        }
        PreviewContent::HtmlFile { html, base_dir } => {
            if path == "/index.html" || path == "/" {
                // Serve the HTML content
                Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Cow::Owned(html.into_bytes()))
                    .unwrap_or_else(|_| error_response(500, "Response error"))
            } else {
                // Serve local resource from base_dir
                serve_local_file(&base_dir, path)
            }
        }
    }
}

/// Serve a local file from the base directory
fn serve_local_file(
    base_dir: &std::path::Path,
    path: &str,
) -> wry::http::Response<Cow<'static, [u8]>> {
    use wry::http::Response;

    // Remove leading slash and decode URL
    let relative_path = path.trim_start_matches('/');

    // Security: prevent directory traversal
    if relative_path.contains("..") {
        return error_response(403, "Forbidden");
    }

    let file_path = base_dir.join(relative_path);

    // Security: ensure the resolved path is within base_dir
    match file_path.canonicalize() {
        Ok(canonical) => {
            if let Ok(base_canonical) = base_dir.canonicalize() {
                if !canonical.starts_with(&base_canonical) {
                    return error_response(403, "Forbidden");
                }
            }
        }
        Err(_) => return error_response(404, "Not found"),
    }

    // Read file content
    let content = match std::fs::read(&file_path) {
        Ok(c) => c,
        Err(_) => return error_response(404, "Not found"),
    };

    // Determine MIME type
    let mime_type = guess_mime_type(&file_path);

    Response::builder()
        .header("Content-Type", mime_type)
        .body(Cow::Owned(content))
        .unwrap_or_else(|_| error_response(500, "Response error"))
}

/// Guess MIME type from file extension
fn guess_mime_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("xml") => "application/xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Create an error response
fn error_response(status: u16, message: &str) -> wry::http::Response<Cow<'static, [u8]>> {
    use wry::http::Response;

    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .body(Cow::Owned(message.as_bytes().to_vec()))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Cow::Borrowed(b"Error" as &[u8]))
                .unwrap()
        })
}

/// Convert our physical-pixel Rect to wry's top-left logical coordinate system.
fn to_wry_rect(bounds: token::model::editor_area::Rect, scale_factor: f64) -> Rect {
    use wry::dpi::{LogicalPosition, LogicalSize};

    let logical_x = bounds.x as f64 / scale_factor;
    let logical_y = bounds.y as f64 / scale_factor;
    let logical_w = bounds.width as f64 / scale_factor;
    let logical_h = bounds.height as f64 / scale_factor;

    Rect {
        position: LogicalPosition::new(logical_x, logical_y).into(),
        size: LogicalSize::new(logical_w, logical_h).into(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn updating_missing_preview_reports_failure_without_storing_content() {
        let mut manager = super::WebviewManager::new();
        let id = token::model::editor_area::PreviewId(42);
        assert!(manager
            .update_content(id, super::PreviewContent::Html("<h1>test</h1>".into()))
            .is_err());
        assert!(manager.protocol_state.read().unwrap().contents.is_empty());
    }

    use super::*;
    use token::model::editor_area::Rect as EditorRect;
    use wry::dpi::{LogicalPosition, LogicalSize};

    #[test]
    fn wry_rect_preserves_top_left_coordinates_at_one_x() {
        let rect = to_wry_rect(EditorRect::new(400.0, 27.0, 400.0, 554.0), 1.0);

        assert_eq!(
            rect.position.to_logical::<f64>(1.0),
            LogicalPosition::new(400.0, 27.0)
        );
        assert_eq!(
            rect.size.to_logical::<f64>(1.0),
            LogicalSize::new(400.0, 554.0)
        );
    }

    #[test]
    fn wry_rect_converts_physical_pixels_to_logical_coordinates() {
        let rect = to_wry_rect(EditorRect::new(800.0, 54.0, 800.0, 708.0), 2.0);

        assert_eq!(
            rect.position.to_logical::<f64>(1.0),
            LogicalPosition::new(400.0, 27.0)
        );
        assert_eq!(
            rect.size.to_logical::<f64>(1.0),
            LogicalSize::new(400.0, 354.0)
        );
    }

    #[test]
    fn bottom_dock_shrinks_webview_without_moving_its_top_edge() {
        let full_height = to_wry_rect(EditorRect::new(400.0, 27.0, 400.0, 554.0), 1.0);
        let dock_open = to_wry_rect(EditorRect::new(400.0, 27.0, 400.0, 354.0), 1.0);

        assert_eq!(full_height.position, dock_open.position);
        assert_eq!(
            dock_open.size.to_logical::<f64>(1.0),
            LogicalSize::new(400.0, 354.0)
        );
    }
}
