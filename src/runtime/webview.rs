//! Webview manager for markdown preview
//!
//! Manages wry WebView instances that overlay the editor window for rich markdown preview.

use std::collections::HashMap;
use std::rc::Rc;

use winit::window::Window;
use wry::{Rect, WebView, WebViewBuilder};

use token::model::editor_area::PreviewId;

/// Manages webview instances for markdown preview panes
pub struct WebviewManager {
    /// Active webviews indexed by preview ID
    webviews: HashMap<PreviewId, WebView>,
}

impl WebviewManager {
    pub fn new() -> Self {
        Self {
            webviews: HashMap::new(),
        }
    }

    /// Create a new webview for a preview pane
    pub fn create_webview(
        &mut self,
        preview_id: PreviewId,
        window: &Rc<Window>,
        bounds: token::model::editor_area::Rect,
        html: &str,
    ) -> Result<(), wry::Error> {
        // Don't create duplicate
        if self.webviews.contains_key(&preview_id) {
            return Ok(());
        }

        let scale_factor = window.scale_factor();
        let window_height = window.inner_size().height;

        let webview = WebViewBuilder::new()
            .with_html(html)
            .with_bounds(to_wry_rect(bounds, scale_factor, window_height))
            .with_transparent(false)
            .with_navigation_handler(|url| {
                // Open external links in the default browser
                if url.starts_with("http://") || url.starts_with("https://") {
                    // Open in default browser and block navigation in webview
                    let _ = open::that(&url);
                    false
                } else {
                    // Allow internal navigation (e.g., anchor links, about:blank)
                    true
                }
            })
            .build_as_child(window)?;

        self.webviews.insert(preview_id, webview);
        Ok(())
    }

    /// Update webview HTML content
    pub fn update_content(&self, preview_id: PreviewId, html: &str) {
        if let Some(webview) = self.webviews.get(&preview_id) {
            // Update content and re-run syntax highlighting
            let js = format!(
                "document.documentElement.innerHTML = {}; if (typeof hljs !== 'undefined') hljs.highlightAll();",
                serde_json::to_string(html).unwrap_or_default()
            );
            let _ = webview.evaluate_script(&js);
        }
    }

    /// Update webview bounds (position and size)
    ///
    /// `window_height` is needed to convert from top-left to bottom-left coordinates on macOS.
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

/// Convert our Rect to wry's Rect with proper DPI and coordinate system conversion.
///
/// Our editor coordinates (bounds) are in physical pixels with top-left origin
/// (softbuffer and fontdue work in physical pixels, char_width/line_height include scale_factor).
///
/// On macOS, NSView uses bottom-left origin, so we need to flip the Y coordinate.
/// We also convert to logical coordinates because wry's set_bounds expects logical points.
fn to_wry_rect(bounds: token::model::editor_area::Rect, scale_factor: f64, window_height_px: u32) -> Rect {
    use wry::dpi::{LogicalPosition, LogicalSize};

    // Convert physical pixels to logical points
    let logical_x = bounds.x as f64 / scale_factor;
    let logical_w = bounds.width as f64 / scale_factor;
    let logical_h = bounds.height as f64 / scale_factor;

    // Convert from top-left to bottom-left coordinate system (macOS)
    // In top-left: y is distance from top
    // In bottom-left: y is distance from bottom
    // Formula: bottom_y = window_height - (top_y + height)
    let window_height_logical = window_height_px as f64 / scale_factor;
    let logical_y = window_height_logical - (bounds.y as f64 / scale_factor + logical_h);

    Rect {
        position: LogicalPosition::new(logical_x, logical_y).into(),
        size: LogicalSize::new(logical_w, logical_h).into(),
    }
}
