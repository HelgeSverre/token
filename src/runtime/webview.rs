//! Webview manager for preview panes
//!
//! Manages wry WebView instances that overlay the editor window for rich preview.
//! Supports both Markdown (rendered to HTML) and HTML files (with local resource loading).

use std::collections::HashMap;
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

mod resources;
pub use resources::{Navigation, PreviewContent, PreviewLocation};
use resources::{PreviewDocument, ResourceJob};
use token::preview_resources::ResourceError;

pub struct NavigationEvent {
    pub preview_id: PreviewId,
    pub document: Arc<PreviewDocument>,
    pub action: Navigation,
    pub url: String,
}

/// Shared state for custom protocol handler
struct ProtocolState {
    /// Current HTML content indexed by preview ID
    contents: HashMap<PreviewId, Arc<PreviewDocument>>,
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
    resource_tx: mpsc::SyncSender<ResourceJob>,
    navigation_tx: mpsc::SyncSender<NavigationEvent>,
    navigation_rx: mpsc::Receiver<NavigationEvent>,
    generation: u64,
}

impl WebviewManager {
    pub fn new() -> Self {
        let (snapshot_tx, snapshot_rx) = mpsc::channel();
        let (navigation_tx, navigation_rx) = mpsc::sync_channel(32);
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
            resource_tx: resources::start_worker(),
            navigation_tx,
            navigation_rx,
            generation: 0,
        }
    }

    pub fn with_wake(wake: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        let mut manager = Self::new();
        manager.wake = wake;
        manager
    }

    fn install_content(
        &mut self,
        id: PreviewId,
        content: PreviewContent,
    ) -> anyhow::Result<Arc<PreviewDocument>> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Preview generation exhausted"))?;
        let document = PreviewDocument::new(id, self.generation, content)?;
        let mut state = self
            .protocol_state
            .write()
            .map_err(|_| anyhow::anyhow!("Preview state poisoned"))?;
        if let Some(old) = state.contents.insert(id, Arc::clone(&document)) {
            old.revoke();
        }
        Ok(document)
    }

    pub fn location_matches(&self, id: PreviewId, location: &PreviewLocation) -> bool {
        self.protocol_state.read().ok().is_some_and(|state| {
            state
                .contents
                .get(&id)
                .is_some_and(|doc| doc.is_active() && doc.location == *location)
        })
    }

    /// Revoke immediately after model changes, before consuming queued callbacks.
    pub fn reconcile(&self, model: &token::AppModel) {
        if let Ok(state) = self.protocol_state.read() {
            for (id, document) in &state.contents {
                let current = model
                    .editor_area
                    .previews
                    .get(id)
                    .and_then(|preview| {
                        let source = model.editor_area.documents.get(&preview.document_id)?;
                        Some(
                            !preview.needs_refresh(source.revision)
                                && document.location
                                    == PreviewLocation::from_document(
                                        source,
                                        model.workspace.as_ref(),
                                    ),
                        )
                    })
                    .unwrap_or(false);
                if !current {
                    document.revoke();
                }
            }
        }
    }

    pub fn navigate_document(&self, id: PreviewId, url: &str) {
        if let Some(webview) = self.webviews.get(&id) {
            if let Err(error) = webview.load_url(url) {
                tracing::warn!(%error, "Could not navigate preview anchor");
            }
        }
    }

    pub fn navigation_events(&self) -> Vec<NavigationEvent> {
        self.navigation_rx
            .try_iter()
            .filter(|event| event.document.is_active())
            .collect()
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

        let document = self
            .install_content(preview_id, content)
            .map_err(|error| wry::Error::Io(std::io::Error::other(error)))?;
        let url = document.url.clone();
        let scale_factor = window.scale_factor();
        let protocol_state = Arc::clone(&self.protocol_state);
        let resource_tx = self.resource_tx.clone();
        let navigation_state = Arc::clone(&self.protocol_state);
        let navigation_tx = self.navigation_tx.clone();
        let wake = self.wake.clone();
        let new_window_state = Arc::clone(&self.protocol_state);
        let new_window_tx = self.navigation_tx.clone();
        let new_window_wake = self.wake.clone();
        let result =
            WebViewBuilder::new()
                .with_asynchronous_custom_protocol(
                    "token".to_string(),
                    move |_webview_id, request, responder| {
                        let document = protocol_state
                            .read()
                            .ok()
                            .and_then(|state| state.contents.get(&preview_id).cloned());
                        let Some(document) = document else {
                            responder.respond(resources::error_response(ResourceError::Stale));
                            return;
                        };
                        if let Err(error) = resource_tx.try_send(ResourceJob {
                            document,
                            request,
                            responder,
                        }) {
                            let (mpsc::TrySendError::Full(job)
                            | mpsc::TrySendError::Disconnected(job)) = error;
                            job.responder
                                .respond(resources::error_response(ResourceError::Unavailable));
                        }
                    },
                )
                .with_url(url)
                .with_bounds(to_wry_rect(bounds, scale_factor))
                .with_transparent(false)
                .with_visible(!self.backdrop.active)
                .with_navigation_handler(move |url| {
                    route_navigation(
                        &navigation_state,
                        preview_id,
                        &url,
                        &navigation_tx,
                        wake.as_ref(),
                        false,
                    )
                })
                .with_new_window_req_handler(move |url, _features| {
                    route_navigation(
                        &new_window_state,
                        preview_id,
                        &url,
                        &new_window_tx,
                        new_window_wake.as_ref(),
                        true,
                    );
                    wry::NewWindowResponse::Deny
                })
                .build_as_child(window);
        let webview = match result {
            Ok(webview) => webview,
            Err(error) => {
                self.close_webview(preview_id);
                return Err(error);
            }
        };

        self.webviews.insert(preview_id, webview);
        Ok(())
    }

    /// Update webview content
    pub fn update_content(
        &mut self,
        preview_id: PreviewId,
        content: PreviewContent,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.webviews.contains_key(&preview_id),
            "Preview webview {preview_id:?} is missing"
        );
        let document = self.install_content(preview_id, content)?;
        if let Some(webview) = self.webviews.get(&preview_id) {
            webview.load_url(&document.url)?;
        }
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
            if let Some(document) = state.contents.remove(&preview_id) {
                document.revoke();
            }
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

fn route_navigation(
    state: &SharedProtocolState,
    preview_id: PreviewId,
    url: &str,
    sender: &mpsc::SyncSender<NavigationEvent>,
    wake: Option<&Arc<dyn Fn() + Send + Sync>>,
    new_window: bool,
) -> bool {
    let document = state
        .read()
        .ok()
        .and_then(|s| s.contents.get(&preview_id).cloned());
    let Some(document) = document else {
        return false;
    };
    match document.navigation(url) {
        Ok(Navigation::Document) if !new_window => true,
        Ok(action) => {
            if sender
                .try_send(NavigationEvent {
                    preview_id,
                    document,
                    action,
                    url: url.to_owned(),
                })
                .is_ok()
            {
                if let Some(wake) = wake {
                    wake();
                }
            }
            false
        }
        Err(error) => {
            tracing::debug!(%error, "Rejected preview navigation");
            false
        }
    }
}

impl Drop for WebviewManager {
    fn drop(&mut self) {
        if let Ok(state) = self.protocol_state.read() {
            for document in state.contents.values() {
                document.revoke();
            }
        }
    }
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
    fn install_fixture(
        manager: &mut WebviewManager,
        model: &mut token::AppModel,
        id: PreviewId,
    ) -> Arc<PreviewDocument> {
        let document_id = model.editor_area.previews[&id].document_id;
        let document = &model.editor_area.documents[&document_id];
        let revision = document.revision;
        let content = PreviewContent {
            html: "page".into(),
            location: PreviewLocation::from_document(document, model.workspace.as_ref()),
        };
        let installed = manager.install_content(id, content).unwrap();
        model
            .editor_area
            .previews
            .get_mut(&id)
            .unwrap()
            .mark_rendered(document_id, revision);
        installed
    }

    #[test]
    fn resource_context_lifecycle_tracks_save_as_workspace_refresh_switch_and_close() {
        let root = tempfile::tempdir().unwrap();
        let mut model = token::AppModel::new(800, 600, 1.0);
        model.document_mut().file_path = Some(root.path().join("first.md"));
        let id = model.editor_area.open_preview_for_focused_group().unwrap();
        let mut manager = WebviewManager::new();
        let initial = install_fixture(&mut manager, &mut model, id);
        manager.reconcile(&model);
        assert!(initial.is_active());
        // Save As changes the path without changing the revision.
        model.document_mut().file_path = Some(root.path().join("second.md"));
        manager.reconcile(&model);
        assert!(!initial.is_active());
        let saved = install_fixture(&mut manager, &mut model, id);
        assert_ne!(saved.url, initial.url);
        model.workspace =
            Some(token::model::Workspace::new(root.path().into(), &model.metrics).unwrap());
        manager.reconcile(&model);
        assert!(!saved.is_active());
        let workspace = install_fixture(&mut manager, &mut model, id);
        model
            .editor_area
            .previews
            .get_mut(&id)
            .unwrap()
            .invalidate();
        manager.reconcile(&model);
        assert!(!workspace.is_active());
        let refreshed = install_fixture(&mut manager, &mut model, id);
        // A document switch at the same revision must invalidate the old grant.
        let replacement_id = model.editor_area.next_document_id();
        model.editor_area.documents.insert(
            replacement_id,
            token::model::Document::new_with_path(root.path().join("third.md")),
        );
        model.editor_area.previews.get_mut(&id).unwrap().document_id = replacement_id;
        manager.reconcile(&model);
        assert!(!refreshed.is_active());
        let switched = install_fixture(&mut manager, &mut model, id);
        model.editor_area.close_preview(id);
        manager.reconcile(&model);
        assert!(!switched.is_active());
    }

    #[test]
    fn navigation_queue_uses_current_origin_and_discards_replaced_generations() {
        let root = tempfile::tempdir().unwrap();
        let mut model = token::AppModel::new(800, 600, 1.0);
        model.document_mut().file_path = Some(root.path().join("first.md"));
        let id = model.editor_area.open_preview_for_focused_group().unwrap();
        let mut manager = WebviewManager::new();
        let doc = install_fixture(&mut manager, &mut model, id);
        let local = url::Url::parse(&doc.url)
            .unwrap()
            .join("other.md#heading")
            .unwrap();
        assert!(!route_navigation(
            &manager.protocol_state,
            id,
            local.as_str(),
            &manager.navigation_tx,
            None,
            false
        ));
        assert_eq!(manager.navigation_events().len(), 1);
        assert!(route_navigation(
            &manager.protocol_state,
            id,
            &format!("{}#heading", doc.url),
            &manager.navigation_tx,
            None,
            false
        ));
        assert!(manager.navigation_events().is_empty());
        assert!(!route_navigation(
            &manager.protocol_state,
            id,
            local.as_str(),
            &manager.navigation_tx,
            None,
            false
        ));
        // New-window anchors are queued back into this preview; external and
        // local targets use exactly the same classifier as normal navigation.
        assert!(!route_navigation(
            &manager.protocol_state,
            id,
            &format!("{}#heading", doc.url),
            &manager.navigation_tx,
            None,
            true
        ));
        let events = manager.navigation_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[1].action, Navigation::Document));
        for _ in 0..40 {
            route_navigation(
                &manager.protocol_state,
                id,
                local.as_str(),
                &manager.navigation_tx,
                None,
                true,
            );
        }
        assert_eq!(manager.navigation_events().len(), 32);
        route_navigation(
            &manager.protocol_state,
            id,
            local.as_str(),
            &manager.navigation_tx,
            None,
            false,
        );
        let replacement = install_fixture(&mut manager, &mut model, id);
        assert!(!doc.is_active());
        assert!(manager.navigation_events().is_empty());
        assert!(!route_navigation(
            &manager.protocol_state,
            id,
            &doc.url,
            &manager.navigation_tx,
            None,
            false
        ));
        manager.close_webview(id);
        assert!(!replacement.is_active());
    }

    #[test]
    fn updating_missing_preview_reports_failure_without_storing_content() {
        let mut manager = super::WebviewManager::new();
        let id = token::model::editor_area::PreviewId(42);
        assert!(manager
            .update_content(
                id,
                super::PreviewContent {
                    html: "<h1>test</h1>".into(),
                    location: PreviewLocation::default()
                }
            )
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
