//! Hit-testing types and functions for unified mouse event handling
//!
//! This module provides a centralized system for determining which UI element
//! is under a given point, and for handling mouse events in a consistent way.
//!
//! The design follows a "hit-test → dispatch" pattern:
//! 1. `hit_test_ui()` determines the highest-priority `HitTarget` at a point
//! 2. Event handlers match on `(HitTarget, MouseButton, click_count)` to dispatch behavior
//! 3. Handlers return `EventResult` to indicate consumption, focus changes, and redraw needs
//!
//! This replaces ad-hoc if/else chains in app.rs with explicit priority ordering
//! and shared hit-testing logic across left/middle/right clicks.

use std::path::PathBuf;

use winit::event::MouseButton;
use winit::keyboard::ModifiersState;

use crate::model::editor_area::{DocumentId, EditorId, GroupId, PreviewId, TabId};
use crate::model::{AppModel, FocusTarget, TextViewportMap};

use crate::layout::editor::{EditorTabBarLayout, PreviewPaneLayout};

use super::geometry::TreeRowLayout;

// ============================================================================
// Core Types
// ============================================================================

/// A point in window coordinates (physical pixels)
#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A mouse event with all relevant context for hit-testing and dispatch
#[derive(Clone, Debug)]
pub struct MouseEvent {
    /// Position in window coordinates
    pub pos: Point,
    /// Which mouse button
    pub button: MouseButton,
    /// Active keyboard modifiers
    pub modifiers: ModifiersState,
}

impl MouseEvent {
    pub fn new(x: f64, y: f64, button: MouseButton, modifiers: ModifiersState) -> Self {
        Self {
            pos: Point::new(x, y),
            button,
            modifiers,
        }
    }

    /// Check if shift modifier is active
    #[inline]
    pub fn cmd(&self) -> bool {
        self.modifiers.super_key()
    }

    pub fn shift(&self) -> bool {
        self.modifiers.shift_key()
    }

    /// Check if alt/option modifier is active
    #[inline]
    pub fn alt(&self) -> bool {
        self.modifiers.alt_key()
    }
}

// ============================================================================
// Hit Targets
// ============================================================================

/// Logical targets in the UI that can receive mouse events.
///
/// These are returned by hit-testing and used by event handlers to dispatch
/// behavior. The variants carry enough context to handle the event without
/// re-querying the model.
///
/// Note: Some variant fields are not currently read but are populated for
/// future use (e.g., context menus, detailed click handling).
#[derive(Clone, Debug)]
pub enum HitTarget {
    /// Modal overlay (command palette, goto line, find/replace, etc.), hit
    /// somewhere that isn't a selectable row (header, footer, panel
    /// padding). `inside` indicates whether the click was inside or outside
    /// the modal bounds — outside dismisses.
    Modal { inside: bool },

    /// A selectable row within a `Body::List` modal — row click sets
    /// selection and activates in one step (overlay-surface.md Pointer).
    ModalRow { flat_index: usize },

    /// A tab in the Search Everywhere tab bar — click switches tabs
    /// (overlay-surface.md Pointer: "Tab click switches tabs").
    ModalTab { index: usize },

    /// Inside a cursor-anchored popup (completion/hover) — non-blocking, per
    /// overlay-surface.md Phase 5: the click lands in the popup instead of
    /// falling through to the editor, and doesn't move the text cursor.
    /// `flat_index` is `Some` when a selectable row (`Body::List`) was hit.
    CursorOverlay { flat_index: Option<usize> },

    /// Status bar at the bottom of the window
    StatusBar,

    /// Sidebar resize handle (the border between sidebar and editor area)
    SidebarResize,

    /// Sidebar file tree area (but not on a specific item)
    SidebarEmpty,

    /// A specific item in the sidebar file tree
    SidebarItem {
        path: PathBuf,
        row: usize,
        is_dir: bool,
        clicked_on_chevron: bool,
    },

    /// A splitter bar between split panes
    Splitter {
        index: usize,
        direction: crate::model::editor_area::SplitDirection,
    },

    /// Header area of a preview pane (can be middle-clicked to close)
    PreviewHeader { preview_id: PreviewId },

    /// Content area of a preview pane (webview or native rendering)
    PreviewContent { preview_id: PreviewId },

    /// A specific tab in a group's tab bar
    GroupTab {
        group_id: GroupId,
        tab_index: usize,
        tab_id: TabId,
    },

    /// Empty area of a group's tab bar (no specific tab)
    GroupTabBarEmpty { group_id: GroupId },

    /// Editor gutter (line numbers, and once shipped: marks/fold/diff lanes)
    EditorGutter {
        group_id: GroupId,
        editor_id: EditorId,
        line: usize,
        /// Which lane was clicked, if any active lane covers the x
        /// coordinate (see `GutterLayout::lane_at`). `None` for today's
        /// line-numbers-only gutter width past the active lanes, or when
        /// no lane is active at all.
        lane: Option<super::geometry::LaneId>,
    },

    /// Editor text content area
    EditorContent {
        group_id: GroupId,
        editor_id: EditorId,
        document_id: DocumentId,
    },

    /// A cell in CSV grid view mode
    CsvCell {
        group_id: GroupId,
        editor_id: EditorId,
        row: usize,
        col: usize,
    },

    /// Dock resize handle (between dock and editor area)
    DockResize {
        position: crate::panel::DockPosition,
    },

    /// A tab in a dock's tab bar
    DockTab {
        position: crate::panel::DockPosition,
        panel_id: crate::panel::PanelId,
    },

    /// Empty area of a dock's tab bar (no specific tab)
    DockTabBarEmpty {
        position: crate::panel::DockPosition,
    },

    /// Dock content area (the active panel's content)
    DockContent {
        position: crate::panel::DockPosition,
        active_panel_id: crate::panel::PanelId,
    },

    /// "Open with Default Application" button on binary placeholder tab
    BinaryPlaceholderButton { group_id: GroupId },

    /// Image content area (pan/zoom viewer)
    ImageContent {
        group_id: GroupId,
        editor_id: EditorId,
    },

    /// Vertical scrollbar thumb (drag to scroll)
    ScrollbarThumbVertical {
        group_id: GroupId,
        editor_id: EditorId,
        /// Where within the thumb the user clicked (pixels from thumb top)
        grab_offset: f32,
        /// Geometry needed for drag computation (stored as raw values)
        track_y: f32,
        track_h: f32,
        thumb_h: f32,
        max_scroll: usize,
    },

    /// Vertical scrollbar track (click to jump)
    ScrollbarTrackVertical {
        group_id: GroupId,
        editor_id: EditorId,
        /// Y coordinate of the click
        coord: f32,
        track_y: f32,
        track_h: f32,
        thumb_h: f32,
        max_scroll: usize,
    },

    /// Horizontal scrollbar thumb (drag to scroll)
    ScrollbarThumbHorizontal {
        group_id: GroupId,
        editor_id: EditorId,
        grab_offset: f32,
        track_x: f32,
        track_w: f32,
        thumb_w: f32,
        max_scroll: usize,
    },

    /// Horizontal scrollbar track (click to jump)
    ScrollbarTrackHorizontal {
        group_id: GroupId,
        editor_id: EditorId,
        coord: f32,
        track_x: f32,
        track_w: f32,
        thumb_w: f32,
        max_scroll: usize,
    },
}

impl HitTarget {
    /// Get the appropriate mouse cursor icon for this hit target
    pub fn cursor_icon(&self) -> winit::window::CursorIcon {
        use crate::model::editor_area::SplitDirection;
        use winit::window::CursorIcon;

        match self {
            HitTarget::EditorContent { .. } | HitTarget::CsvCell { .. } => CursorIcon::Text,
            HitTarget::BinaryPlaceholderButton { .. } => CursorIcon::Pointer,
            HitTarget::SidebarResize => CursorIcon::ColResize,
            HitTarget::DockResize { position } => match position {
                crate::panel::DockPosition::Right | crate::panel::DockPosition::Left => {
                    CursorIcon::ColResize
                }
                crate::panel::DockPosition::Bottom => CursorIcon::RowResize,
            },
            HitTarget::Splitter { direction, .. } => match direction {
                SplitDirection::Horizontal => CursorIcon::ColResize,
                SplitDirection::Vertical => CursorIcon::RowResize,
            },
            _ => CursorIcon::Default,
        }
    }

    /// Get the hover region for this hit target (used for input routing)
    pub fn hover_region(&self) -> crate::model::HoverRegion {
        use crate::model::HoverRegion;

        match self {
            HitTarget::Modal { .. } | HitTarget::ModalRow { .. } | HitTarget::ModalTab { .. } => {
                HoverRegion::Modal
            }
            HitTarget::CursorOverlay { .. } => HoverRegion::CursorOverlay,
            HitTarget::StatusBar => HoverRegion::StatusBar,
            HitTarget::SidebarResize => HoverRegion::SidebarResize,
            HitTarget::SidebarEmpty | HitTarget::SidebarItem { .. } => HoverRegion::Sidebar,
            HitTarget::Splitter { .. } => HoverRegion::Splitter,
            HitTarget::PreviewHeader { .. } | HitTarget::PreviewContent { .. } => {
                HoverRegion::Preview
            }
            HitTarget::GroupTab { .. } | HitTarget::GroupTabBarEmpty { .. } => {
                HoverRegion::EditorTabBar
            }
            HitTarget::DockResize { position } => HoverRegion::DockResize(*position),
            HitTarget::DockTab { position, .. }
            | HitTarget::DockTabBarEmpty { position }
            | HitTarget::DockContent { position, .. } => HoverRegion::Dock(*position),
            HitTarget::BinaryPlaceholderButton { group_id } => HoverRegion::Button(*group_id),
            // Editor content, gutter, image content, CSV cells, and scrollbars map to EditorText.
            HitTarget::EditorGutter { .. }
            | HitTarget::EditorContent { .. }
            | HitTarget::ImageContent { .. }
            | HitTarget::CsvCell { .. }
            | HitTarget::ScrollbarThumbVertical { .. }
            | HitTarget::ScrollbarTrackVertical { .. }
            | HitTarget::ScrollbarThumbHorizontal { .. }
            | HitTarget::ScrollbarTrackHorizontal { .. } => HoverRegion::EditorText,
        }
    }
}

// ============================================================================
// Event Results
// ============================================================================

/// Result of handling a mouse event
#[derive(Clone, Debug)]
pub enum EventResult {
    /// Event was fully handled; stop propagation
    Consumed {
        /// Whether a redraw is needed (ignored if cmd is Some)
        redraw: bool,
        /// Optional focus change to apply
        focus: Option<FocusTarget>,
        /// Optional command to execute (takes precedence over redraw flag)
        cmd: Option<crate::commands::Cmd>,
    },

    /// Event was not handled by this target; allow fallback handling
    Bubble,
}

impl EventResult {
    /// Create a consumed result that requests redraw but no focus change
    pub fn consumed_redraw() -> Self {
        Self::Consumed {
            redraw: true,
            focus: None,
            cmd: None,
        }
    }

    /// Create a consumed result with focus change and redraw
    pub fn consumed_with_focus(focus: FocusTarget) -> Self {
        Self::Consumed {
            redraw: true,
            focus: Some(focus),
            cmd: None,
        }
    }

    /// Create a consumed result with no redraw (event blocked but nothing changed)
    pub fn consumed_no_redraw() -> Self {
        Self::Consumed {
            redraw: false,
            focus: None,
            cmd: None,
        }
    }

    /// Create a consumed result with a command and focus change
    pub fn consumed_with_cmd(cmd: Option<crate::commands::Cmd>, focus: FocusTarget) -> Self {
        Self::Consumed {
            redraw: cmd.is_none(), // Only set redraw if no cmd
            focus: Some(focus),
            cmd,
        }
    }
}

// ============================================================================
// Hit-Testing Functions
// ============================================================================

/// Hit-test the modal overlay.
///
/// If a modal is active, returns `HitTarget::Modal` with `inside` indicating
/// whether the point is inside the modal bounds. This allows the caller to
/// decide whether to close the modal (outside click) or handle the click
/// inside the modal.
pub fn hit_test_modal(model: &AppModel, pt: Point) -> Option<HitTarget> {
    if !model.ui.has_modal() {
        return None;
    }

    let ww = model.window_size.0 as usize;
    let wh = model.window_size.1 as usize;
    let sf = model.metrics.scale_factor;
    let (x, y) = (pt.x as usize, pt.y as usize);

    // Build the exact same shape-only spec+layout the renderer computes
    // (one layout, two consumers — overlay-surface.md "Hit-testing") and
    // test the point against it.
    super::modal::with_modal_overlay_layout(model, ww, wh, sf, |spec, layout| {
        match super::overlay_surface::hit_test(spec, layout, x, y) {
            super::overlay_surface::OverlayHit::Outside => HitTarget::Modal { inside: false },
            super::overlay_surface::OverlayHit::Row(flat_index) => HitTarget::ModalRow {
                flat_index: flat_index.0,
            },
            super::overlay_surface::OverlayHit::Inside => HitTarget::Modal { inside: true },
            super::overlay_surface::OverlayHit::Tab(index) => HitTarget::ModalTab { index },
        }
    })
}

/// Hit-test a cursor-anchored popup. Unlike `hit_test_modal`, this only
/// returns `Some` for points *inside* the popup panel — a click outside
/// isn't claimed here (overlay-surface.md Phase 5: popups are non-blocking,
/// so an outside click still falls through to whatever's under it; the
/// caller is responsible for dismissing the popup separately).
pub fn hit_test_cursor_overlay(
    model: &AppModel,
    pt: Point,
    measure: &mut dyn crate::layout::TextMeasure,
) -> Option<HitTarget> {
    model.ui.cursor_overlay?;

    let ww = model.window_size.0 as usize;
    let wh = model.window_size.1 as usize;
    let sf = model.metrics.scale_factor;
    let (x, y) = (pt.x as usize, pt.y as usize);

    super::modal::with_cursor_overlay_spec(model, |spec| {
        // Measured with the caller's measure — the render path lays the
        // same spec out through the glyph cache, so the rects tested here
        // are the rects that were painted.
        let layout = super::overlay_surface::layout_measured(spec, ww, wh, sf, measure);
        match super::overlay_surface::hit_test(spec, &layout, x, y) {
            super::overlay_surface::OverlayHit::Outside => None,
            super::overlay_surface::OverlayHit::Row(flat_index) => Some(HitTarget::CursorOverlay {
                flat_index: Some(flat_index.0),
            }),
            super::overlay_surface::OverlayHit::Inside
            | super::overlay_surface::OverlayHit::Tab(_) => {
                Some(HitTarget::CursorOverlay { flat_index: None })
            }
        }
    })
    .flatten()
}

/// Hit-test the status bar at the bottom of the window.
pub fn hit_test_status_bar(model: &AppModel, pt: Point) -> Option<HitTarget> {
    let shell = crate::layout::chrome::shell(model);
    hit_test_status_bar_in(&shell, pt)
}

fn hit_test_status_bar_in(chrome: &crate::layout::LayoutSnapshot, pt: Point) -> Option<HitTarget> {
    chrome
        .rect(crate::layout::UiKey::StatusBar)
        .filter(|rect| rect.contains(pt.x as f32, pt.y as f32))
        .map(|_| HitTarget::StatusBar)
}

/// Hit-test the sidebar resize handle.
///
/// Returns `SidebarResize` if the point is within the resize hit zone
/// (a few pixels on either side of the sidebar border).
pub fn hit_test_sidebar_resize(model: &AppModel, pt: Point) -> Option<HitTarget> {
    let shell = crate::layout::chrome::shell(model);
    hit_test_sidebar_resize_in(model, &shell, pt)
}

fn hit_test_sidebar_resize_in(
    model: &AppModel,
    chrome: &crate::layout::LayoutSnapshot,
    pt: Point,
) -> Option<HitTarget> {
    const SIDEBAR_RESIZE_HIT_ZONE: f64 = 4.0;

    model.workspace.as_ref()?;
    let sidebar = chrome.rect(crate::layout::UiKey::Sidebar)?;
    let sidebar_right = (sidebar.x + sidebar.width) as f64;
    let resize_zone_start = sidebar_right - SIDEBAR_RESIZE_HIT_ZONE;
    let resize_zone_end = sidebar_right + SIDEBAR_RESIZE_HIT_ZONE;

    if pt.x >= resize_zone_start
        && pt.x <= resize_zone_end
        && pt.y >= sidebar.y as f64
        && pt.y < (sidebar.y + sidebar.height) as f64
    {
        Some(HitTarget::SidebarResize)
    } else {
        None
    }
}

/// Hit-test the sidebar file tree.
///
/// Returns `SidebarItem` if clicking on a file/folder, or `SidebarEmpty`
/// if clicking in the sidebar area but not on an item.
pub fn hit_test_sidebar(model: &AppModel, pt: Point) -> Option<HitTarget> {
    let sidebar = crate::layout::chrome::sidebar_rows(model);
    hit_test_sidebar_in(model, &sidebar, pt)
}

fn hit_test_sidebar_in(
    model: &AppModel,
    chrome: &crate::layout::LayoutSnapshot,
    pt: Point,
) -> Option<HitTarget> {
    let workspace = model.workspace.as_ref()?;
    let rows = chrome.row_list(crate::layout::UiKey::Sidebar)?;
    if !rows.rect().contains(pt.x as f32, pt.y as f32) {
        return None;
    }
    let Some(clicked_row) = rows.row_at_y(pt.y as f32) else {
        return Some(HitTarget::SidebarEmpty);
    };

    if let Some((node, depth)) = workspace
        .file_tree
        .get_visible_item_with_depth(clicked_row, &workspace.expanded_folders)
    {
        // Share intra-row chevron geometry with the renderer.
        let tree = TreeRowLayout::from_metrics(&model.metrics);
        let chevron_start = rows.rect().x as f64 + tree.x_offset(depth) as f64;
        let chevron_end = chevron_start + tree.indicator_width as f64;
        let clicked_on_chevron = node.is_dir && pt.x >= chevron_start && pt.x < chevron_end;

        Some(HitTarget::SidebarItem {
            path: node.path.clone(),
            row: clicked_row,
            is_dir: node.is_dir,
            clicked_on_chevron,
        })
    } else {
        Some(HitTarget::SidebarEmpty)
    }
}

/// Hit-test splitter bars between split panes.
///
/// Requires the pre-computed splitters from `EditorArea::compute_layout_scaled()`.
pub fn hit_test_splitters(
    _model: &AppModel,
    pt: Point,
    splitters: &[crate::model::editor_area::SplitterBar],
) -> Option<HitTarget> {
    for (i, splitter) in splitters.iter().enumerate() {
        if splitter.rect.contains(pt.x as f32, pt.y as f32) {
            return Some(HitTarget::Splitter {
                index: i,
                direction: splitter.direction,
            });
        }
    }
    None
}

/// Hit-test preview panes.
///
/// Returns `PreviewHeader` if clicking on the header area, or `PreviewContent`
/// if clicking in the content area.
pub fn hit_test_previews(model: &AppModel, pt: Point) -> Option<HitTarget> {
    for (&preview_id, preview) in &model.editor_area.previews {
        if preview.rect.contains(pt.x as f32, pt.y as f32) {
            let layout = PreviewPaneLayout::new(preview_id, preview.rect, &model.metrics);
            if layout.is_in_header(pt.x, pt.y) {
                return Some(HitTarget::PreviewHeader { preview_id });
            } else if layout.is_in_content(pt.x, pt.y) {
                return Some(HitTarget::PreviewContent { preview_id });
            }
        }
    }
    None
}

/// Hit-test editor groups (tab bar and content area).
///
/// Returns `GroupTab` if clicking on a specific tab, `GroupTabBarEmpty` if
/// clicking in the tab bar but not on a tab, or `EditorContent`/`CsvCell`
/// if clicking in the editor content area.
pub fn hit_test_groups(model: &AppModel, pt: Point, char_width: f32) -> Option<HitTarget> {
    // First check which group contains the point
    let group_id = model.editor_area.group_at_point(pt.x as f32, pt.y as f32)?;
    let group = model.editor_area.groups.get(&group_id)?;
    let tab_bar = EditorTabBarLayout::new(group, model, char_width);
    // Single source of truth for group geometry, shared by the scrollbar
    // branch and the gutter/content branch below (mirrors the render path's
    // use of GroupLayout in `EditorRenderContext`).
    let layout = super::geometry::GroupLayout::new(group, model, char_width);

    // Check if in tab bar
    if tab_bar.contains(pt.x, pt.y) {
        if let Some(tab_id) = tab_bar.tab_at(pt.x, pt.y) {
            let tab_index = group.tabs.iter().position(|tab| tab.id == tab_id)?;
            return Some(HitTarget::GroupTab {
                group_id,
                tab_index,
                tab_id,
            });
        }

        return Some(HitTarget::GroupTabBarEmpty { group_id });
    }

    // Get the active editor for this group
    let editor_id = group.active_editor_id()?;
    let editor = model.editor_area.editors.get(&editor_id)?;

    // For image mode, return ImageContent for the whole content area
    if editor.view_mode.is_image() {
        return Some(HitTarget::ImageContent {
            group_id,
            editor_id,
        });
    }

    // For BinaryPlaceholder tabs, check button hit area
    if let crate::model::TabContent::BinaryPlaceholder(_) = &editor.tab_content {
        let line_height = model.line_height;
        let bp_layout = super::geometry::binary_placeholder_layout(
            layout.content_rect,
            line_height,
            char_width,
            model.metrics.padding_large,
            model.metrics.padding_medium,
            super::geometry::BINARY_PLACEHOLDER_BUTTON_LABEL,
        );

        if bp_layout.button_rect.contains(pt.x as f32, pt.y as f32) {
            return Some(HitTarget::BinaryPlaceholderButton { group_id });
        }
    }

    let doc_id = editor.document_id?;
    let document = model.editor_area.documents.get(&doc_id)?;

    // Check scrollbar hit areas (before gutter/content, since they overlay the right edge)
    if model.config.show_scrollbar
        && matches!(editor.tab_content, crate::model::TabContent::Text)
        && !editor.view_mode.is_csv()
    {
        use super::scrollbar::{ScrollbarGeometry, ScrollbarState};
        let sw = model.metrics.scrollbar_width;
        let viewport = &editor.viewport;
        let visible_lines = layout.visible_lines(model.line_height);
        let visible_columns = layout.visible_columns(char_width);
        let x = pt.x as f32;
        let y = pt.y as f32;

        // Vertical scrollbar
        if let Some(v_track) = layout.v_scrollbar_rect(sw) {
            if v_track.contains(x, y) {
                let line_count = document.line_count();
                let v_state = ScrollbarState::new(line_count, visible_lines, viewport.top_line);
                let v_geo = ScrollbarGeometry::vertical(v_track, &v_state);
                if v_geo.needed && v_geo.hits_thumb(x, y) {
                    let grab_offset = y - v_geo.thumb_rect.y;
                    return Some(HitTarget::ScrollbarThumbVertical {
                        group_id,
                        editor_id,
                        grab_offset,
                        track_y: v_track.y,
                        track_h: v_track.height,
                        thumb_h: v_geo.thumb_rect.height,
                        max_scroll: v_state.max_position(),
                    });
                }
                if v_geo.hits_track(x, y) {
                    return Some(HitTarget::ScrollbarTrackVertical {
                        group_id,
                        editor_id,
                        coord: y,
                        track_y: v_track.y,
                        track_h: v_track.height,
                        thumb_h: v_geo.thumb_rect.height,
                        max_scroll: v_state.max_position(),
                    });
                }
            }
        }

        // Horizontal scrollbar
        if let Some(h_track) = layout.h_scrollbar_rect(sw) {
            if h_track.contains(x, y) {
                let top = viewport.top_line;
                let bottom = (top + visible_lines).min(document.line_count());
                let max_len = (top..bottom)
                    .map(|i| document.line_length(i))
                    .max()
                    .unwrap_or(0);
                let h_state = ScrollbarState::new(max_len, visible_columns, viewport.left_column);
                if h_state.needs_scroll() {
                    let h_geo = ScrollbarGeometry::horizontal(h_track, &h_state);
                    if h_geo.hits_thumb(x, y) {
                        let grab_offset = x - h_geo.thumb_rect.x;
                        return Some(HitTarget::ScrollbarThumbHorizontal {
                            group_id,
                            editor_id,
                            grab_offset,
                            track_x: h_track.x,
                            track_w: h_track.width,
                            thumb_w: h_geo.thumb_rect.width,
                            max_scroll: h_state.max_position(),
                        });
                    }
                    return Some(HitTarget::ScrollbarTrackHorizontal {
                        group_id,
                        editor_id,
                        coord: x,
                        track_x: h_track.x,
                        track_w: h_track.width,
                        thumb_w: h_geo.thumb_rect.width,
                        max_scroll: h_state.max_position(),
                    });
                }
            }
        }
    }

    // Check if in CSV mode
    if editor.view_mode.is_csv() {
        // For CSV mode, we could compute the exact cell here
        // For now, return a placeholder that the caller can refine
        return Some(HitTarget::CsvCell {
            group_id,
            editor_id,
            row: 0,
            col: 0,
        });
    }

    // Check if in gutter area
    let gutter_x_end = layout.gutter_right_x as f64;
    let content_y_start = layout.content_y() as f64;

    if pt.x >= group.rect.x as f64 && pt.x < gutter_x_end && pt.y >= content_y_start {
        // Compute which line was clicked
        let local_y = pt.y - content_y_start;
        let viewport = TextViewportMap::new(&editor.viewport, document.line_count());
        let line = viewport.doc_line_for_pixel_y(local_y, model.line_height as f64);
        let x_in_gutter = (pt.x - group.rect.x as f64).max(0.0) as usize;
        let lane = layout.gutter.lane_at(x_in_gutter);
        return Some(HitTarget::EditorGutter {
            group_id,
            editor_id,
            line,
            lane,
        });
    }

    // Editor content area
    Some(HitTarget::EditorContent {
        group_id,
        editor_id,
        document_id: doc_id,
    })
}

/// Hit-test dock panels (right and bottom docks).
///
/// Computes dock rectangles and checks if the point falls within any open dock.
/// Returns `DockContent` with the active panel ID for content clicks, or
/// `DockResize` if the point is over the resizable border between a dock and
/// the editor area.
pub fn hit_test_docks(model: &AppModel, pt: Point) -> Option<HitTarget> {
    let shell = crate::layout::chrome::shell(model);
    if !point_may_hit_dock(model, &shell, pt) {
        return None;
    }
    let chrome = crate::layout::chrome::chrome(model);
    hit_test_docks_in(model, &chrome, pt)
}

/// Cheap outer-rect guard for the full dock solve. The full snapshot needs
/// active panel contents and virtual row counts; most pointer events occur in
/// the editor and need none of that work.
fn point_may_hit_dock(model: &AppModel, shell: &crate::layout::LayoutSnapshot, pt: Point) -> bool {
    use crate::layout::UiKey;
    use crate::panel::DockPosition;

    let hit_zone = model.metrics.resize_handle_zone as f64;
    let over_right = shell
        .rect(UiKey::Dock(DockPosition::Right))
        .is_some_and(|rect| {
            pt.x >= rect.x as f64 - hit_zone
                && pt.x < (rect.x + rect.width) as f64
                && pt.y >= rect.y as f64
                && pt.y < (rect.y + rect.height) as f64
        });
    let over_bottom = shell
        .rect(UiKey::Dock(DockPosition::Bottom))
        .is_some_and(|rect| {
            pt.x >= rect.x as f64
                && pt.x < (rect.x + rect.width) as f64
                && pt.y >= rect.y as f64 - hit_zone
                && pt.y < (rect.y + rect.height) as f64
        });
    over_right || over_bottom
}

fn hit_test_docks_in(
    model: &AppModel,
    chrome: &crate::layout::LayoutSnapshot,
    pt: Point,
) -> Option<HitTarget> {
    use crate::layout::UiKey;

    let hit_zone = model.metrics.resize_handle_zone as f64;

    // Resize handles first: a hit-slop band around the dock's inner edge,
    // procedural on top of the solved dock rects (the engine has no
    // hit-slop concept).
    if let Some(right_rect) = chrome.rect(UiKey::Dock(crate::panel::DockPosition::Right)) {
        let resize_zone_start = right_rect.x as f64 - hit_zone;
        let resize_zone_end = right_rect.x as f64 + hit_zone;
        if pt.x >= resize_zone_start
            && pt.x <= resize_zone_end
            && pt.y >= right_rect.y as f64
            && pt.y < (right_rect.y + right_rect.height) as f64
        {
            return Some(HitTarget::DockResize {
                position: crate::panel::DockPosition::Right,
            });
        }
    }
    if let Some(bottom_rect) = chrome.rect(UiKey::Dock(crate::panel::DockPosition::Bottom)) {
        let resize_zone_start = bottom_rect.y as f64 - hit_zone;
        let resize_zone_end = bottom_rect.y as f64 + hit_zone;
        if pt.y >= resize_zone_start
            && pt.y <= resize_zone_end
            && pt.x >= bottom_rect.x as f64
            && pt.x < (bottom_rect.x + bottom_rect.width) as f64
        {
            return Some(HitTarget::DockResize {
                position: crate::panel::DockPosition::Bottom,
            });
        }
    }

    // Everything else: the topmost solved element at the point, mapped
    // exhaustively onto hit targets — the same geometry the renderer paints.
    match chrome.hit(pt.x as f32, pt.y as f32)? {
        UiKey::DockTab(position, panel_id) => Some(HitTarget::DockTab { position, panel_id }),
        UiKey::DockHeader(position) => Some(HitTarget::DockTabBarEmpty { position }),
        UiKey::PanelContent(_) | UiKey::PanelRows(_) => {
            let position = [
                crate::panel::DockPosition::Right,
                crate::panel::DockPosition::Bottom,
            ]
            .into_iter()
            .find(|&pos| {
                chrome
                    .rect(UiKey::Dock(pos))
                    .is_some_and(|r| r.contains(pt.x as f32, pt.y as f32))
            })?;
            let dock = model.dock_layout.dock(position);
            match dock.active_panel() {
                Some(panel_id) => Some(HitTarget::DockContent {
                    position,
                    active_panel_id: panel_id,
                }),
                None => Some(HitTarget::DockTabBarEmpty { position }),
            }
        }
        // The dock body outside header/content (shouldn't occur — the two
        // children tile it) falls through as content-less dock space.
        UiKey::Dock(position) => Some(HitTarget::DockTabBarEmpty { position }),
        _ => None,
    }
}

/// Main hit-testing function that checks all UI regions in priority order.
///
/// Returns the highest-priority `HitTarget` at the given point, or `None`
/// if the point is not over any interactive region.
///
/// # Priority Order (highest first)
/// 1. Modal overlay (blocks everything when active)
/// 2. Status bar (always on top at bottom of window)
/// 3. Sidebar resize handle
/// 4. Sidebar file tree
/// 5. Dock panels (right, bottom)
/// 6. Splitter bars
/// 7. Preview panes (header and content)
/// 8. Editor groups (tab bar, gutter, content)
pub fn hit_test_ui(
    model: &AppModel,
    pt: Point,
    char_width: f32,
    measure: &mut dyn crate::layout::TextMeasure,
) -> Option<HitTarget> {
    // 0. Cursor-anchored popup (non-blocking: only claims points inside its
    // own panel, so an outside click still reaches whatever's under it —
    // see `hit_test_cursor_overlay`).
    if let Some(target) = hit_test_cursor_overlay(model, pt, measure) {
        return Some(target);
    }

    // 1. Modal overlay (highest priority)
    if let Some(target) = hit_test_modal(model, pt) {
        return Some(target);
    }

    // The cheap shell drives the common path. Sidebar or dock rows are solved
    // lazily only when the pointer can hit that region.
    let shell = crate::layout::chrome::shell(model);

    // 2. Status bar
    if let Some(target) = hit_test_status_bar_in(&shell, pt) {
        return Some(target);
    }

    // 3. Sidebar resize handle
    if let Some(target) = hit_test_sidebar_resize_in(model, &shell, pt) {
        return Some(target);
    }

    // 4. Sidebar file tree
    if shell
        .rect(crate::layout::UiKey::Sidebar)
        .is_some_and(|rect| rect.contains(pt.x as f32, pt.y as f32))
    {
        let sidebar = crate::layout::chrome::sidebar_rows(model);
        if let Some(target) = hit_test_sidebar_in(model, &sidebar, pt) {
            return Some(target);
        }
    }

    // 5. Dock panels (must be checked before editor groups, which may overlap)
    if point_may_hit_dock(model, &shell, pt) {
        let chrome = crate::layout::chrome::chrome(model);
        if let Some(target) = hit_test_docks_in(model, &chrome, pt) {
            return Some(target);
        }
    }

    // 6. Splitter bars use the same solved editor-area rect.
    let editor_area_rect = shell
        .rect(crate::layout::UiKey::EditorArea)
        .expect("window chrome always declares the editor area");

    // Group/preview rects are already kept current by the render pass's own
    // `compute_layout_scaled` call, so hit-testing only needs splitters and
    // can derive them read-only instead of cloning the entire `EditorArea`
    // (every open document's undo/redo stacks included) on every mouse event.
    let splitters = model
        .editor_area
        .compute_splitters(editor_area_rect, model.metrics.splitter_width);

    if let Some(target) = hit_test_splitters(model, pt, &splitters) {
        return Some(target);
    }

    // 7. Preview panes
    if let Some(target) = hit_test_previews(model, pt) {
        return Some(target);
    }

    // 8. Editor groups
    hit_test_groups(model, pt, char_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_new() {
        let pt = Point::new(100.0, 200.0);
        assert_eq!(pt.x, 100.0);
        assert_eq!(pt.y, 200.0);
    }

    #[test]
    fn test_mouse_event_helpers() {
        let event = MouseEvent::new(50.0, 50.0, MouseButton::Left, ModifiersState::empty());
        assert!(!event.shift());
        assert!(!event.alt());
    }

    #[test]
    fn status_bar_hit_test_uses_the_solved_shell_rect() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.status_bar_height = 20;

        assert!(matches!(
            hit_test_status_bar(&model, Point::new(10.0, 590.0)),
            Some(HitTarget::StatusBar)
        ));
        assert!(hit_test_status_bar(&model, Point::new(10.0, 579.0)).is_none());
        assert!(hit_test_status_bar(&model, Point::new(801.0, 590.0)).is_none());
    }

    #[test]
    fn dock_hit_guard_includes_contents_and_external_resize_slop() {
        use crate::panel::{DockPosition, PanelId};

        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.dock_layout.right.activate(PanelId::Outline);
        model.dock_layout.bottom.activate(PanelId::Terminal);
        let shell = crate::layout::chrome::shell(&model);
        let right = shell
            .rect(crate::layout::UiKey::Dock(DockPosition::Right))
            .unwrap();
        let bottom = shell
            .rect(crate::layout::UiKey::Dock(DockPosition::Bottom))
            .unwrap();

        assert!(!point_may_hit_dock(
            &model,
            &shell,
            Point::new(100.0, 100.0)
        ));
        assert!(point_may_hit_dock(
            &model,
            &shell,
            Point::new(
                right.x as f64 - model.metrics.resize_handle_zone as f64 + 1.0,
                right.y as f64 + 10.0,
            )
        ));
        assert!(point_may_hit_dock(
            &model,
            &shell,
            Point::new(
                bottom.x as f64 + 10.0,
                bottom.y as f64 - model.metrics.resize_handle_zone as f64 + 1.0,
            )
        ));
    }

    /// Regression test: `hit_test_groups` must classify gutter vs. content
    /// clicks using the exact same `GroupLayout` geometry that the render
    /// path (`EditorRenderContext`) uses, not a hand-rederived copy. Before
    /// this fix the gutter/content branch computed its own
    /// `gutter_width`/`gutter_x_end` locals independently of the
    /// `GroupLayout` built for the scrollbar branch above it.
    #[test]
    fn test_hit_test_groups_gutter_content_boundary_matches_group_layout() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let available = crate::model::editor_area::Rect::new(0.0, 0.0, 800.0, 600.0);
        model.editor_area.compute_layout(available);

        let group_id = model
            .editor_area
            .group_at_point(10.0, 10.0)
            .expect("a group should exist near the window origin");
        let group = model
            .editor_area
            .groups
            .get(&group_id)
            .expect("group_at_point returned a valid group id");
        let char_width = model.char_width;

        // The single source of truth both hit-testing and rendering should
        // agree on.
        let layout = crate::view::geometry::GroupLayout::new(group, &model, char_width);
        let y = layout.content_y() as f64 + 5.0;

        // A point just left of GroupLayout's gutter/content boundary must
        // resolve to the gutter.
        let left_of_boundary = Point::new(layout.gutter_right_x as f64 - 1.0, y);
        match hit_test_groups(&model, left_of_boundary, char_width) {
            Some(HitTarget::EditorGutter { .. }) => {}
            other => panic!(
                "expected EditorGutter just left of gutter_right_x, got {:?}",
                other
            ),
        }

        // A point just right of the same boundary must resolve to content,
        // not the gutter.
        let right_of_boundary = Point::new(layout.gutter_right_x as f64 + 1.0, y);
        match hit_test_groups(&model, right_of_boundary, char_width) {
            Some(HitTarget::EditorContent { .. }) => {}
            other => panic!(
                "expected EditorContent just right of gutter_right_x, got {:?}",
                other
            ),
        }
    }
}
