//! Webview manager for preview panes
//!
//! Manages wry WebView instances that overlay the editor window for rich preview.
//! Supports both Markdown (rendered to HTML) and HTML files (with local resource loading).

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use winit::window::Window;
use wry::{Rect, WebView, WebViewBuilder};

use token::model::editor_area::PreviewId;

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
}

impl WebviewManager {
    pub fn new() -> Self {
        Self {
            webviews: HashMap::new(),
            protocol_state: Arc::new(RwLock::new(ProtocolState {
                contents: HashMap::new(),
            })),
        }
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
        let window_height = window.inner_size().height;
        let protocol_state = Arc::clone(&self.protocol_state);
        let pid = preview_id;

        let webview = WebViewBuilder::new()
            .with_custom_protocol("token".to_string(), move |_webview_id, request| {
                handle_protocol_request(&protocol_state, pid, request)
            })
            .with_url(format!("token://preview-{}/index.html", preview_id.0))
            .with_bounds(to_wry_rect(bounds, scale_factor, window_height))
            .with_transparent(false)
            .with_navigation_handler(|url| {
                // Open external links in the default browser
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = open::that(&url);
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
    pub fn update_content(&mut self, preview_id: PreviewId, content: PreviewContent) {
        // Update stored content
        if let Ok(mut state) = self.protocol_state.write() {
            state.contents.insert(preview_id, content);
        }

        // Reload the webview to pick up new content
        if let Some(webview) = self.webviews.get(&preview_id) {
            let url = format!("token://preview-{}/index.html", preview_id.0);
            let _ = webview.load_url(&url);
        }
    }

    /// Update webview bounds (position and size)
    pub fn update_bounds(
        &self,
        preview_id: PreviewId,
        bounds: token::model::editor_area::Rect,
        scale_factor: f64,
        window_height: u32,
    ) {
        if let Some(webview) = self.webviews.get(&preview_id) {
            let wry_rect = to_wry_rect(bounds, scale_factor, window_height);
            let _ = webview.set_bounds(wry_rect);
        }
    }

    /// Scroll webview to a specific line (for scroll sync)
    #[allow(dead_code)]
    pub fn scroll_to_line(&self, preview_id: PreviewId, line: usize) {
        if let Some(webview) = self.webviews.get(&preview_id) {
            let js = format!("if(window.scrollToLine) window.scrollToLine({});", line);
            let _ = webview.evaluate_script(&js);
        }
    }

    /// Close and remove a webview
    pub fn close_webview(&mut self, preview_id: PreviewId) {
        self.webviews.remove(&preview_id);
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

    /// Set visibility for a specific webview
    #[allow(dead_code)]
    pub fn set_visible(&self, preview_id: PreviewId, visible: bool) {
        if let Some(webview) = self.webviews.get(&preview_id) {
            let _ = webview.set_visible(visible);
        }
    }

    /// Set visibility for all webviews (hide when modals are shown)
    pub fn set_all_visible(&self, visible: bool) {
        for webview in self.webviews.values() {
            let _ = webview.set_visible(visible);
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

/// Convert our Rect to wry's Rect with proper DPI and coordinate system conversion.
fn to_wry_rect(
    bounds: token::model::editor_area::Rect,
    scale_factor: f64,
    window_height_px: u32,
) -> Rect {
    use wry::dpi::{LogicalPosition, LogicalSize};

    // Convert physical pixels to logical points
    let logical_x = bounds.x as f64 / scale_factor;
    let logical_w = bounds.width as f64 / scale_factor;
    let logical_h = bounds.height as f64 / scale_factor;

    // Convert from top-left to bottom-left coordinate system (macOS)
    let window_height_logical = window_height_px as f64 / scale_factor;
    let logical_y = window_height_logical - (bounds.y as f64 / scale_factor + logical_h);

    Rect {
        position: LogicalPosition::new(logical_x, logical_y).into(),
        size: LogicalSize::new(logical_w, logical_h).into(),
    }
}
