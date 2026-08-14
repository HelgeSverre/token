//! Typed element identity.
//!
//! Clay identifies elements by hashed strings (`CLAY_ID`); this in-repo
//! adaptation uses a closed domain enum instead, so the mapping from solved
//! elements to `view::hit_test::HitTarget` is exhaustive and type-checked.

use crate::panel::{DockPosition, PanelId};

/// Identity for elements the rest of the app needs to find in a solved
/// [`crate::layout::LayoutSnapshot`] — for rect queries, hit-testing, and
/// row-list math. Purely structural elements (spacers, wrappers) carry no
/// key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiKey {
    // --- Dock chrome (P2) ---
    /// A dock's full rectangle.
    Dock(DockPosition),
    /// The tab-strip header row of a dock.
    DockHeader(DockPosition),
    /// One panel tab inside a dock header.
    DockTab(DockPosition, PanelId),
    /// The content area below a dock header, for the active panel.
    PanelContent(PanelId),
    /// The virtualized row list inside a panel's content area.
    PanelRows(PanelId),

    /// A cursor-anchored floating panel. Used by the layout benchmark and
    /// available to production overlay declarations as that migration grows.
    CursorOverlayPanel,

    // --- Window shell ---
    Sidebar,
    EditorArea,
    StatusBar,
}
