//! Typed element identity.
//!
//! Clay identifies elements by hashed strings (`CLAY_ID`); this in-repo
//! adaptation uses a closed domain enum instead, so the mapping from solved
//! elements to `view::hit_test::HitTarget` is exhaustive and type-checked.

use crate::model::editor_area::{GroupId, PreviewId, TabId};
use crate::panel::{DockPosition, PanelId};

/// Identity for elements the rest of the app needs to find in a solved
/// [`crate::layout::LayoutSnapshot`] — for rect queries, hit-testing, and
/// row-list math. Purely structural elements (spacers, wrappers) carry no
/// key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiKey {
    // --- Editor chrome ---
    /// One editor group's complete tab strip.
    EditorTabBar(GroupId),
    /// One editor tab inside its group's horizontally scrolling strip.
    EditorTab(GroupId, TabId),
    /// A preview pane's full rectangle.
    PreviewPane(PreviewId),
    /// The fixed-height preview header.
    PreviewHeader(PreviewId),
    /// The preview content below the header. Its border box hosts the
    /// webview; its padded content box hosts the native fallback.
    PreviewContent(PreviewId),

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

    // --- Floating overlay surfaces ---
    OverlayPanel,
    OverlayTabBar,
    OverlayTab(usize),
    OverlayHeader,
    OverlayRows,
    OverlayFieldLabel(usize),
    OverlayFieldInput(usize),
    OverlayZoneBanner,
    OverlayZoneCode,
    OverlayZoneText,
    OverlayFooter,

    // --- Window shell ---
    Sidebar,
    EditorArea,
    StatusBar,
}
