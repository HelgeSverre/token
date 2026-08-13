//! Mouse event handling using the unified hit-test system
//!
//! This module provides centralized mouse event dispatch that:
//! - Uses `hit_test_ui()` to determine the target under the cursor
//! - Dispatches behavior based on (target, button, click_count)
//! - Handles focus changes consistently
//! - Shares hit-testing logic across left/middle/right clicks

use std::time::{Duration, Instant};

use winit::event::MouseButton;
use winit::keyboard::ModifiersState;

use token::commands::Cmd;
use token::messages::{
    CompletionMsg, CsvMsg, EditorMsg, ImageMsg, LayoutMsg, ModalMsg, Msg, OutlineMsg, PreviewMsg,
    TerminalMsg, UiMsg, WorkspaceMsg,
};
use token::model::AppModel;
use token::panel::DockPosition;
use token::update::update;
use token::util::visible_tree_row_at_index;

use token::model::editor_area::GroupId;
use token::view::geometry::TabBarLayout;
use token::view::hit_test::{hit_test_ui, EventResult, HitTarget, MouseEvent};
use token::view::Renderer;

/// Identifies what was clicked, so rapid clicks on unrelated targets
/// (e.g. a sidebar row then an editor line) never count as double-clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickRegion {
    Editor {
        group: token::model::editor_area::GroupId,
        line: usize,
        column: usize,
    },
    Sidebar {
        row: usize,
    },
    Outline {
        row: usize,
    },
    Problems {
        row: usize,
    },
    BinaryPlaceholder {
        group: token::model::editor_area::GroupId,
    },
}

/// Click tracking state for double/triple click detection
pub struct ClickTracker {
    pub last_click_time: Instant,
    pub last_click_region: Option<ClickRegion>,
    pub click_count: u32,
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self {
            last_click_time: Instant::now() - Duration::from_secs(10),
            last_click_region: None,
            click_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use token::model::HoverRegion;
    use token::panel::{DockPosition, PanelId};
    use token::terminal::{PtyHandle, TerminalSession};

    fn terminal_model_with_history() -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.dock_layout.bottom.activate(PanelId::TERMINAL);
        model.ui.hover = HoverRegion::Dock(DockPosition::Bottom);

        let (pty, _pty_rx) = PtyHandle::new_for_test();
        let (msg_tx, _msg_rx) = mpsc::channel();
        let mut session = TerminalSession::new(7, 4, 20, pty, msg_tx);
        session.apply_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\n");
        model.terminal.sessions.push(session);
        model
    }

    // ========================================================================
    // Interactive gutter lane suppression (editor-decorations.md)
    // ========================================================================

    fn gutter_target(lane: Option<token::view::geometry::LaneId>) -> HitTarget {
        HitTarget::EditorGutter {
            group_id: token::model::editor_area::GroupId(0),
            editor_id: token::model::editor_area::EditorId(0),
            line: 0,
            lane,
        }
    }

    #[test]
    fn interactive_lane_press_does_not_arm_content_drag() {
        use token::view::geometry::LaneId;

        assert!(
            !arms_content_drag(&gutter_target(Some(LaneId::Fold))),
            "a chevron click must not arm text-selection drag"
        );
    }

    #[test]
    fn non_interactive_gutter_press_arms_content_drag() {
        assert!(
            arms_content_drag(&gutter_target(None)),
            "line-number gutter clicks must keep arming drag, same as today"
        );
    }

    #[test]
    fn editor_content_press_arms_content_drag() {
        let target = HitTarget::EditorContent {
            group_id: token::model::editor_area::GroupId(0),
            editor_id: token::model::editor_area::EditorId(0),
            document_id: token::model::editor_area::DocumentId(0),
        };
        assert!(arms_content_drag(&target));
    }

    #[test]
    fn interactive_lane_click_is_suppressed() {
        use token::view::geometry::LaneId;

        let result = interactive_gutter_lane_click(Some(LaneId::Fold));
        assert!(matches!(
            result,
            Some(EventResult::Consumed { redraw: false, .. })
        ));
    }

    #[test]
    fn non_interactive_lane_click_falls_through() {
        assert!(interactive_gutter_lane_click(None).is_none());
    }

    #[test]
    fn cursor_overlay_row_click_accepts_a_completion_item() {
        use token::messages::DocumentMsg;
        use token::model::{Cursor, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().buffer = ropey::Rope::from_str("value_one\n\n");
        model.editor_mut().cursors[0] = Cursor::at(1, 0);
        model.editor_mut().clear_selection();
        for ch in "val".chars() {
            update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        }
        assert!(model.ui.completion_menu.is_some(), "menu should be open");
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        let result = handle_cursor_overlay_click(&mut model, Some(0));

        assert!(matches!(result, EventResult::Consumed { redraw: true, .. }));
        assert!(
            model.ui.completion_menu.is_none(),
            "a row click must accept and close the menu, not just select the row"
        );
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "value_one");
    }

    #[test]
    fn cursor_overlay_row_click_activates_a_context_menu_item() {
        use token::context_menu::MenuItem;
        use token::messages::LayoutMsg;
        use token::model::{ContextMenuState, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        // A second tab so "Close" (targeting tab 0) has something to close
        // without hitting the "can't close the last tab" guard.
        update(&mut model, Msg::Layout(LayoutMsg::NewTab));
        let group_id = model.editor_area.focused_group_id;
        let first_tab = model.editor_area.groups[&group_id].tabs[0].id;

        let items = vec![MenuItem::custom(
            "Close",
            true,
            vec![Msg::Layout(LayoutMsg::CloseTab(first_tab))],
        )];
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::ContextMenu));
        model.ui.context_menu = Some(ContextMenuState {
            items,
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::EditorTabBar,
        });

        let result = handle_cursor_overlay_click(&mut model, Some(0));

        assert!(matches!(result, EventResult::Consumed { redraw: true, .. }));
        assert!(
            model.ui.context_menu.is_none(),
            "activation closes the menu"
        );
        assert_eq!(model.editor_area.groups[&group_id].tabs.len(), 1);
        assert!(!model.editor_area.groups[&group_id]
            .tabs
            .iter()
            .any(|t| t.id == first_tab));
    }

    #[test]
    fn cursor_overlay_row_click_runs_the_activated_items_cmd() {
        // Regression: `handle_cursor_overlay_click` used to discard
        // `update()`'s returned Cmd and return `consumed_redraw()`
        // (cmd: None) unconditionally, so a mouse click on e.g. "Copy
        // Absolute Path" closed the menu but never ran the
        // `CopyToClipboard` command the keyboard Enter path produces.
        use token::context_menu::MenuItem;
        use token::messages::ContextMenuMsg;
        use token::model::{ContextMenuState, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let path = std::path::PathBuf::from("/tmp/x.rs");
        let items = vec![MenuItem::custom(
            "Copy Absolute Path",
            true,
            vec![Msg::ContextMenu(ContextMenuMsg::CopyPath {
                path,
                relative: false,
            })],
        )];
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::ContextMenu));
        model.ui.context_menu = Some(ContextMenuState {
            items,
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::Editor,
        });

        let result = handle_cursor_overlay_click(&mut model, Some(0));

        match result {
            EventResult::Consumed { cmd: Some(cmd), .. } => {
                assert!(
                    matches!(&cmd, Cmd::Batch(cmds) if cmds.iter().any(|c| matches!(c, Cmd::CopyToClipboard(_)))),
                    "expected the CopyToClipboard cmd to reach the caller, got {cmd:?}"
                );
            }
            other => {
                panic!("expected a Consumed result carrying the activation's Cmd, got {other:?}")
            }
        }
    }

    #[test]
    fn mouse_wheel_up_over_terminal_dock_scrolls_scrollback() {
        let mut model = terminal_model_with_history();

        let cmd = handle_mouse_wheel(&mut model, Some((0.0, 0.0)), 0, -3);

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 3);
    }

    #[test]
    fn mouse_wheel_down_over_terminal_dock_scrolls_toward_bottom() {
        let mut model = terminal_model_with_history();
        model.terminal.active_session_mut().unwrap().scroll_offset = 4;

        let cmd = handle_mouse_wheel(&mut model, Some((0.0, 0.0)), 0, 2);

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 2);
    }

    #[test]
    fn cursor_overlay_wheel_scroll_clamps_to_row_count() {
        // The debug Completion demo has exactly `MAX_VISIBLE_COMPLETION`
        // rows, so its whole list is always visible and `max_scroll` is 0 —
        // any number of downward notches must leave `scroll` at 0 rather
        // than accumulating unboundedly (regression: previously required
        // as many upward notches to "unwind" before the (already-fully-
        // visible) window would move again).
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.hover = HoverRegion::CursorOverlay;
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::DebugCompletion,
        ));
        assert_eq!(
            token::view::modal::debug_completion_row_count(),
            token::view::overlay_surface::MAX_VISIBLE_COMPLETION
        );

        for _ in 0..20 {
            handle_mouse_wheel(&mut model, None, 0, 3);
        }
        assert_eq!(model.ui.cursor_overlay.unwrap().scroll, 0);

        handle_mouse_wheel(&mut model, None, 0, -3);
        assert_eq!(model.ui.cursor_overlay.unwrap().scroll, 0);
    }

    #[test]
    fn tab_bar_horizontal_scroll_matches_content_direction() {
        // Regression: horizontal tab scrolling was inverted once trackpad
        // horizontal deltas stopped truncating to zero. Positive `h_delta`
        // must reveal tabs further right (positive `delta_px`), matching
        // editor horizontal scrolling.
        assert_eq!(tab_bar_scroll_delta_px(2, 0, 10), Some(20));
        assert_eq!(tab_bar_scroll_delta_px(-2, 0, 10), Some(-20));
    }

    #[test]
    fn tab_bar_horizontal_takes_precedence_over_vertical() {
        assert_eq!(tab_bar_scroll_delta_px(1, 5, 10), Some(10));
    }

    #[test]
    fn tab_bar_vertical_wheel_falls_back_with_inverted_sign() {
        // Plain mouse wheel (no X axis) keeps its legacy repurposed sign.
        assert_eq!(tab_bar_scroll_delta_px(0, 3, 10), Some(-30));
    }

    #[test]
    fn tab_bar_no_scroll_when_both_axes_are_zero() {
        assert_eq!(tab_bar_scroll_delta_px(0, 0, 10), None);
    }

    // ========================================================================
    // Context menu (context-menu.md)
    // ========================================================================

    fn right_click_event() -> MouseEvent {
        MouseEvent::new(50.0, 60.0, MouseButton::Right, ModifiersState::empty())
    }

    #[test]
    fn right_click_on_editor_content_opens_the_editor_menu() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;
        let editor_id = model.editor_area.focused_editor_id().unwrap();
        let document_id = model.editor_area.focused_document_id().unwrap();
        let target = HitTarget::EditorContent {
            group_id,
            editor_id,
            document_id,
        };

        let result = handle_right_click(&mut model, &target, &right_click_event());

        assert!(matches!(result, EventResult::Consumed { .. }));
        assert!(model.ui.context_menu.is_some(), "the editor menu opened");
        assert_eq!(
            model.ui.context_menu.unwrap().region,
            token::context_menu::ContextMenuRegion::Editor
        );
    }

    #[test]
    fn right_click_on_a_sidebar_item_opens_the_file_tree_menu() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let target = HitTarget::SidebarItem {
            path: std::path::PathBuf::from("/tmp/foo.rs"),
            row: 0,
            is_dir: false,
            clicked_on_chevron: false,
        };

        handle_right_click(&mut model, &target, &right_click_event());

        assert_eq!(
            model.ui.context_menu.unwrap().region,
            token::context_menu::ContextMenuRegion::FileTree
        );
    }

    #[test]
    fn right_click_on_a_region_with_no_v1_menu_bubbles() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let result = handle_right_click(&mut model, &HitTarget::StatusBar, &right_click_event());
        assert!(matches!(result, EventResult::Bubble));
        assert!(model.ui.context_menu.is_none());
    }

    #[test]
    fn right_click_is_a_no_op_while_a_modal_is_open() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.active_modal = Some(token::model::ModalState::GotoLine(Default::default()));
        let target = HitTarget::SidebarItem {
            path: std::path::PathBuf::from("/tmp/foo.rs"),
            row: 0,
            is_dir: false,
            clicked_on_chevron: false,
        };

        let result = handle_right_click(&mut model, &target, &right_click_event());

        assert!(model.ui.context_menu.is_none());
        assert!(
            matches!(result, EventResult::Consumed { .. }),
            "still consumed — no menu opened, but the click shouldn't fall through either"
        );
    }

    // ========================================================================
    // Mouse-press preamble policy (context-menu.md §Mouse): click-away
    // consumes, and the right-click-reopen exception (075f957).
    // ========================================================================

    #[test]
    fn click_away_from_a_context_menu_dismisses_and_swallows_the_click() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::ContextMenu,
        ));
        model.ui.context_menu = Some(token::model::ContextMenuState {
            items: vec![],
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::Editor,
        });

        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Left);

        assert!(dismissal.dismissed);
        assert!(
            dismissal.swallow,
            "a left-click that dismisses a context menu must not also act on what it landed on"
        );
        assert!(model.ui.cursor_overlay.is_none());
        assert!(model.ui.context_menu.is_none());
    }

    #[test]
    fn a_right_click_dismisses_a_context_menu_without_swallowing_so_it_can_reopen() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::ContextMenu,
        ));
        model.ui.context_menu = Some(token::model::ContextMenuState {
            items: vec![],
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::Editor,
        });

        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Right);

        assert!(dismissal.dismissed);
        assert!(
            !dismissal.swallow,
            "a right-click on a second target must still reach handle_right_click to reopen"
        );
    }

    #[test]
    fn click_away_from_a_non_context_menu_overlay_dismisses_without_swallowing() {
        // Completion/hover/references are non-blocking (overlay-surface.md
        // Phase 5): click-away dismisses but falls through to whatever's
        // under it — only the context menu swallows.
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::Completion,
        ));

        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Left);

        assert!(dismissal.dismissed);
        assert!(!dismissal.swallow);
    }

    #[test]
    fn click_on_the_cursor_overlay_itself_never_dismisses() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::ContextMenu,
        ));

        let dismissal = dismiss_overlay_for_press(
            &mut model,
            &HitTarget::CursorOverlay {
                flat_index: Some(0),
            },
            MouseButton::Left,
        );

        assert!(!dismissal.dismissed);
        assert!(!dismissal.swallow);
        assert!(model.ui.cursor_overlay.is_some());
    }

    #[test]
    fn no_open_overlay_is_a_no_op() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Left);
        assert!(!dismissal.dismissed);
        assert!(!dismissal.swallow);
    }

    #[test]
    fn tab_file_path_resolves_the_tabs_document_not_the_focused_one() {
        let model = AppModel::new(800, 600, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;
        let tab_id = model
            .editor_area
            .groups
            .get(&group_id)
            .unwrap()
            .active_tab()
            .unwrap()
            .id;
        // An untitled buffer has no path.
        assert_eq!(tab_file_path(&model, group_id, tab_id), None);
    }
}

impl ClickTracker {
    /// Update click count based on timing and click target
    ///
    /// Returns the new click count (1, 2, or 3)
    pub fn track_click(&mut self, region: ClickRegion) -> u8 {
        let now = Instant::now();
        let double_click_time = Duration::from_millis(300);

        let is_rapid_click = now.duration_since(self.last_click_time) < double_click_time;
        let is_same_target = self.last_click_region == Some(region);

        if is_rapid_click && is_same_target {
            self.click_count += 1;
            if self.click_count > 3 {
                self.click_count = 1;
            }
        } else {
            self.click_count = 1;
        }

        self.last_click_time = now;
        self.last_click_region = Some(region);

        self.click_count as u8
    }
}

/// Tracks drag state for text selection (left mouse button drag).
///
/// Encapsulates the state machine: idle → mouse down → threshold exceeded → dragging.
/// Also handles auto-scroll throttling during drag.
#[derive(Default)]
pub struct DragState {
    left_mouse_down: bool,
    start_position: Option<(f64, f64)>,
    active: bool,
    last_auto_scroll: Option<Instant>,
}

impl DragState {
    /// Whether the left mouse button is currently held down.
    pub fn is_down(&self) -> bool {
        self.left_mouse_down
    }

    /// Start tracking a potential drag from the given position.
    pub fn begin(&mut self, x: f64, y: f64) {
        self.left_mouse_down = true;
        self.start_position = Some((x, y));
        self.active = false;
    }

    /// End the drag (mouse released).
    pub fn end(&mut self) {
        self.left_mouse_down = false;
        self.start_position = None;
        self.active = false;
        self.last_auto_scroll = None;
    }

    /// Whether a drag is currently active (threshold exceeded).
    pub fn is_active(&self) -> bool {
        self.left_mouse_down && self.active
    }

    /// Check if mouse movement exceeds the drag threshold (4px).
    /// Returns the start position if the threshold was just crossed, None otherwise.
    pub fn check_threshold(&mut self, x: f64, y: f64) -> Option<(f64, f64)> {
        const DRAG_THRESHOLD_PIXELS: f64 = 4.0;

        if self.active || !self.left_mouse_down {
            return None;
        }

        if let Some((start_x, start_y)) = self.start_position {
            let dx = x - start_x;
            let dy = y - start_y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance >= DRAG_THRESHOLD_PIXELS {
                self.active = true;
                return Some((start_x, start_y));
            }
        }
        None
    }

    /// Attempt auto-scroll during drag. Returns a scroll direction (+1 or -1)
    /// if the cursor is outside the visible area and enough time has passed.
    pub fn try_auto_scroll(&mut self, y: f64, status_bar_top: f64) -> Option<i32> {
        const AUTO_SCROLL_INTERVAL_MS: u64 = 80;

        let direction = if y < 0.0 {
            Some(-1)
        } else if y >= status_bar_top {
            Some(1)
        } else {
            None
        };

        let direction = direction?;

        let now = Instant::now();
        if let Some(last) = self.last_auto_scroll {
            if now.duration_since(last) < Duration::from_millis(AUTO_SCROLL_INTERVAL_MS) {
                return None;
            }
        }

        self.last_auto_scroll = Some(now);
        Some(direction)
    }
}

/// Distance in pixels before an armed tab drag becomes active
const TAB_DRAG_THRESHOLD_PIXELS: f64 = 4.0;

/// Find the group whose tab bar contains the point, along with the tab index
/// under the cursor (or the last index when over the empty tail of the bar).
fn tab_bar_target_at(model: &AppModel, x: f64, y: f64) -> Option<(GroupId, usize)> {
    model.editor_area.groups.values().find_map(|group| {
        let layout = TabBarLayout::new(group, model, model.char_width);
        if !layout.contains(x, y) {
            return None;
        }
        let index = layout
            .tab_at(x, y)
            .map(|tab| tab.index)
            .unwrap_or_else(|| group.tabs.len().saturating_sub(1));
        Some((group.id, index))
    })
}

/// Find the group that currently owns a tab.
fn tab_owning_group(model: &AppModel, tab_id: token::model::editor_area::TabId) -> Option<GroupId> {
    model
        .editor_area
        .groups
        .iter()
        .find_map(|(id, g)| g.tabs.iter().any(|t| t.id == tab_id).then_some(*id))
}

/// Update an armed/active tab drag on mouse move.
///
/// The drag is fully live: hovering a tab bar reorders the tab into that
/// slot (moving it between groups first if needed), and hovering another
/// pane's content area moves the tab into that pane. Releasing simply drops
/// the tab where it already is (`end_tab_drag`).
pub fn update_tab_drag(model: &mut AppModel, x: f64, y: f64) -> Option<Cmd> {
    let drag = model.ui.tab_drag.as_mut()?;
    drag.current = (x, y);

    if !drag.active {
        let dx = x - drag.press.0;
        let dy = y - drag.press.1;
        if (dx * dx + dy * dy).sqrt() < TAB_DRAG_THRESHOLD_PIXELS {
            return None;
        }
        drag.active = true;
    }

    let tab_id = drag.tab_id;
    let owning_group = tab_owning_group(model, tab_id)?;

    // Tab bars take priority; a pane's content area targets that pane's
    // tab end, but only for *other* panes (dragging into your own pane's
    // text area must not reorder anything).
    let target = tab_bar_target_at(model, x, y).or_else(|| {
        model.editor_area.groups.values().find_map(|g| {
            (g.id != owning_group && g.rect.contains(x as f32, y as f32))
                .then_some((g.id, usize::MAX))
        })
    });

    if let Some((group_id, index)) = target {
        if group_id != owning_group {
            update(
                model,
                Msg::Layout(LayoutMsg::MoveTab {
                    tab_id,
                    to_group: group_id,
                }),
            );
            update(model, Msg::Layout(LayoutMsg::FocusGroup(group_id)));
        }
        // ReorderTab clamps the index, so usize::MAX means "keep at end"
        update(
            model,
            Msg::Layout(LayoutMsg::ReorderTab {
                tab_id,
                to_index: index,
            }),
        );
    }

    // Full redraw every move: the drag ghost follows the cursor anywhere
    Some(Cmd::Redraw)
}

/// Finish a tab drag on mouse release.
///
/// Moves/reorders happen live during the drag, so this only clears the
/// drag state and repaints to remove the ghost.
pub fn end_tab_drag(model: &mut AppModel) -> Option<Cmd> {
    let drag = model.ui.tab_drag.take()?;
    if !drag.active {
        return None; // plain click, no drag happened
    }
    Some(Cmd::Redraw)
}

/// Construct a MouseEvent from raw input data
pub fn make_mouse_event(
    x: f64,
    y: f64,
    button: MouseButton,
    modifiers: ModifiersState,
) -> MouseEvent {
    MouseEvent::new(x, y, button, modifiers)
}

/// Result of mouse press handling, including state changes for the App
#[derive(Debug, Clone)]
pub struct MousePressResult {
    /// Command to execute (usually Redraw or None)
    pub cmd: Option<Cmd>,
    /// Whether to start tracking left mouse drag (for text selection)
    pub start_drag_tracking: bool,
}

/// Outcome of `dismiss_overlay_for_press`: whether an open cursor-anchored
/// popup was dismissed by this press, and whether the press must be
/// swallowed outright (consumed, not click-through) rather than falling
/// through to whatever's under it.
struct OverlayDismissal {
    dismissed: bool,
    swallow: bool,
}

/// The mouse-press preamble's overlay policy (context-menu.md §Mouse,
/// 075f957) — factored out of `handle_mouse_press` so it's unit-testable
/// without a `Renderer`/real `Window`.
///
/// Cursor-anchored popups are non-blocking (overlay-surface.md Phase 5): a
/// click that lands outside the popup dismisses it but still falls through
/// to whatever's actually under the cursor. The context menu is the one
/// exception (context-menu.md "Mouse: click-away consumes, not
/// click-through" — JetBrains behavior, not VS Code's) — except a
/// right-click while a context menu is open must still reach
/// `dispatch_mouse_press`/`handle_right_click` so a new menu can open at
/// the new target (context-menu.md Phase 2: "another cursor overlay
/// already open -> close the old one first and re-open").
fn dismiss_overlay_for_press(
    model: &mut AppModel,
    target: &HitTarget,
    button: MouseButton,
) -> OverlayDismissal {
    let dismissed_kind = model.ui.cursor_overlay.map(|s| s.kind);
    let dismissed =
        model.ui.cursor_overlay.is_some() && !matches!(target, HitTarget::CursorOverlay { .. });
    if dismissed {
        model.ui.cursor_overlay = None;
        model.ui.completion_menu = None;
        model.ui.hover_card = None;
        model.ui.reference_list = None;
        model.ui.context_menu = None;
    }
    let swallow = dismissed
        && dismissed_kind == Some(token::model::CursorOverlayKind::ContextMenu)
        && button != MouseButton::Right;
    OverlayDismissal { dismissed, swallow }
}

/// Handle a mouse press event using the unified hit-test system
///
/// This is the main entry point for mouse click handling. It:
/// 1. Performs hit-testing to find the target
/// 2. Dispatches to the appropriate handler based on (target, button)
/// 3. Applies focus changes from EventResult
/// 4. Returns MousePressResult with command and state changes
pub fn handle_mouse_press(
    model: &mut AppModel,
    renderer: &mut Renderer,
    event: MouseEvent,
    click_tracker: &mut ClickTracker,
) -> MousePressResult {
    let char_width = renderer.char_width();
    let pt = event.pos;

    // Perform hit-testing, measuring text through the renderer's glyph
    // cache so overlay geometry matches what was painted.
    let target = {
        let mut painter = renderer.text_painter();
        let mut measure = token::layout::PainterMeasure::new(&mut painter);
        hit_test_ui(model, pt, char_width, &mut measure)
    };
    let Some(target) = target else {
        return MousePressResult {
            cmd: None,
            start_drag_tracking: false,
        };
    };

    let dismissal = dismiss_overlay_for_press(model, &target, event.button);
    let dismissed_cursor_overlay = dismissal.dismissed;
    if dismissal.swallow {
        // Consumed, not click-through: the dismissing click must not also
        // act on whatever it landed on.
        return MousePressResult {
            cmd: Some(Cmd::Redraw),
            start_drag_tracking: false,
        };
    }

    // Track if we're clicking on editor content (for drag tracking).
    // Interactive gutter lanes (fold chevron, marks) consume the press
    // themselves (see `handle_left_click`) — a chevron click must not
    // arm text-selection drag tracking.
    let is_editor_content = arms_content_drag(&target);
    let is_left_click = matches!(event.button, MouseButton::Left);

    // Dispatch based on target and button
    let result = dispatch_mouse_press(model, renderer, &target, &event, click_tracker);

    // Apply focus changes
    if let EventResult::Consumed {
        focus: Some(focus_target),
        ..
    } = &result
    {
        match focus_target {
            token::model::FocusTarget::Editor => model.ui.focus_editor(),
            token::model::FocusTarget::Dock(pos) => model.ui.focus_dock(*pos),
            token::model::FocusTarget::Modal => {}
        }
    }

    // Determine command - use explicit cmd if present, otherwise fallback to redraw
    let cmd = match &result {
        EventResult::Consumed { cmd: Some(c), .. } => Some(c.clone()),
        EventResult::Consumed { redraw: true, .. } => Some(Cmd::Redraw),
        EventResult::Consumed { redraw: false, .. } if dismissed_cursor_overlay => {
            Some(Cmd::Redraw)
        }
        EventResult::Consumed { redraw: false, .. } => None,
        EventResult::Bubble if dismissed_cursor_overlay => Some(Cmd::Redraw),
        EventResult::Bubble => None,
    };

    MousePressResult {
        cmd,
        start_drag_tracking: is_editor_content && is_left_click,
    }
}

/// Whether a press on `target` should arm text-selection drag tracking:
/// editor content, or a gutter click outside any interactive lane. A press
/// on an interactive lane (fold chevron, marks — editor-decorations.md)
/// consumes itself in `handle_left_click`/`handle_middle_click` and must
/// not also start a text selection.
fn arms_content_drag(target: &HitTarget) -> bool {
    matches!(
        target,
        HitTarget::EditorContent { .. } | HitTarget::ImageContent { .. }
    ) || matches!(
        target,
        HitTarget::EditorGutter { lane, .. } if !lane.is_some_and(|lane| lane.is_interactive())
    )
}

/// A press on an interactive gutter lane (fold chevron, marks) consumes
/// itself as a no-op rather than falling through to the default
/// focus/rectangle-selection gutter behavior — no lane owner has shipped
/// yet, but `handle_left_click`/`handle_middle_click` must actively
/// suppress it rather than let it fall through (editor-decorations.md).
fn interactive_gutter_lane_click(
    lane: Option<token::view::geometry::LaneId>,
) -> Option<EventResult> {
    lane.is_some_and(|lane| lane.is_interactive())
        .then(EventResult::consumed_no_redraw)
}

/// Dispatch a mouse press to the appropriate handler based on target and button
fn dispatch_mouse_press(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    match event.button {
        MouseButton::Left => handle_left_click(model, renderer, target, event, click_tracker),
        MouseButton::Middle => handle_middle_click(model, renderer, target, event),
        MouseButton::Right => handle_right_click(model, target, event),
        _ => EventResult::Bubble,
    }
}

/// Click on a cursor-anchored popup row: update selection, and — for the
/// Completion popup specifically — accept the clicked item (the same
/// message `Enter` sends).
fn handle_cursor_overlay_click(model: &mut AppModel, flat_index: Option<usize>) -> EventResult {
    let Some(idx) = flat_index else {
        return EventResult::consumed_redraw();
    };
    let kind = model.ui.cursor_overlay.map(|state| state.kind);
    if let Some(state) = &mut model.ui.cursor_overlay {
        state.selected = idx;
    }
    // The activation message may return a Cmd (e.g. CopyToClipboard) that
    // must actually run — same as the keyboard Enter path in
    // `handle_cursor_overlay_key`, which returns `update()`'s result
    // directly instead of discarding it.
    let cmd = match kind {
        Some(token::model::CursorOverlayKind::Completion) => {
            update(model, Msg::Completion(CompletionMsg::AcceptMenuItem))
        }
        // A row click sets selection and activates in one step
        // (overlay-surface.md Pointer) — same as Enter.
        Some(token::model::CursorOverlayKind::References) => update(
            model,
            Msg::Lsp(token::messages::LspMsg::ActivateReference { index: idx }),
        ),
        Some(token::model::CursorOverlayKind::ContextMenu) => update(
            model,
            Msg::ContextMenu(token::messages::ContextMenuMsg::ActivateItem { index: idx }),
        ),
        _ => None,
    };
    match cmd {
        Some(cmd) => EventResult::Consumed {
            redraw: true,
            focus: None,
            cmd: Some(cmd),
        },
        None => EventResult::consumed_redraw(),
    }
}

/// Handle left mouse button clicks
fn handle_left_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    use token::model::FocusTarget;

    match target {
        // Modal handling
        HitTarget::Modal { inside } => {
            if *inside {
                // Click inside modal (header/footer/padding) - consume but
                // don't close or act.
                EventResult::consumed_redraw()
            } else {
                // Click outside modal - close it
                update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
                EventResult::consumed_redraw()
            }
        }

        // Row click: select and activate in one step (overlay-surface.md
        // Pointer: "a click sets selection and activates in one step").
        HitTarget::ModalRow { flat_index } => {
            update(
                model,
                Msg::Ui(UiMsg::Modal(ModalMsg::ActivateRow(*flat_index))),
            );
            EventResult::consumed_redraw()
        }

        // Tab click: switch the Search Everywhere tab (overlay-surface.md
        // Pointer: "Tab click switches tabs").
        HitTarget::ModalTab { index } => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::ActivateTab(*index))));
            EventResult::consumed_redraw()
        }

        // Cursor-anchored popup: consume the click without dismissing the
        // popup or moving the text cursor (overlay-surface.md Phase 5). Row
        // clicks update the popup's own selection; the Completion popup
        // additionally accepts on click (the mouse-driven equivalent of
        // Enter — otherwise the real popup could only ever be used with the
        // keyboard).
        HitTarget::CursorOverlay { flat_index } => handle_cursor_overlay_click(model, *flat_index),

        // Status bar - consume but do nothing
        HitTarget::StatusBar => EventResult::consumed_no_redraw(),

        // Sidebar resize handle
        HitTarget::SidebarResize => {
            update(
                model,
                Msg::Workspace(WorkspaceMsg::StartSidebarResize {
                    initial_x: event.pos.x,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Sidebar empty area
        HitTarget::SidebarEmpty => {
            EventResult::consumed_with_focus(FocusTarget::Dock(DockPosition::Left))
        }

        // Sidebar item
        HitTarget::SidebarItem {
            path,
            row,
            is_dir,
            clicked_on_chevron,
        } => {
            // Track clicks for double-click detection
            let click_count = click_tracker.track_click(ClickRegion::Sidebar { row: *row });

            // Always select the item
            update(
                model,
                Msg::Workspace(WorkspaceMsg::SelectItem(path.clone())),
            );

            // Chevron click immediately toggles folder
            if *clicked_on_chevron {
                update(
                    model,
                    Msg::Workspace(WorkspaceMsg::ToggleFolder(path.clone())),
                );
                return EventResult::consumed_with_focus(FocusTarget::Dock(DockPosition::Left));
            }

            // Double-click opens file or toggles folder
            if click_count >= 2 {
                let cmd = if *is_dir {
                    update(
                        model,
                        Msg::Workspace(WorkspaceMsg::ToggleFolder(path.clone())),
                    )
                } else {
                    update(
                        model,
                        Msg::Workspace(WorkspaceMsg::OpenFile {
                            path: path.clone(),
                            preview: false,
                        }),
                    )
                };
                // Return the command from opening the file (includes syntax parse)
                return EventResult::consumed_with_cmd(cmd, FocusTarget::Dock(DockPosition::Left));
            }

            EventResult::consumed_with_focus(FocusTarget::Dock(DockPosition::Left))
        }

        // Splitter drag
        HitTarget::Splitter { index, .. } => {
            update(
                model,
                Msg::Layout(LayoutMsg::BeginSplitterDrag {
                    splitter_index: *index,
                    position: (event.pos.x as f32, event.pos.y as f32),
                }),
            );
            EventResult::consumed_redraw()
        }

        // Preview pane header - consume, keep editor focus
        HitTarget::PreviewHeader { .. } => {
            // Just consume - middle-click closes
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Preview pane content - consume, keep editor focus for keyboard
        HitTarget::PreviewContent { .. } => {
            // Webview handles its own clicks; just keep editor focus
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Tab click
        HitTarget::GroupTab {
            group_id,
            tab_id,
            tab_index,
        } => {
            // Focus group if not already focused
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(model, Msg::Layout(LayoutMsg::SwitchToTab(*tab_index)));
            // Arm a potential tab drag (activates past the move threshold)
            model.ui.tab_drag = Some(token::model::ui::TabDragState {
                tab_id: *tab_id,
                press: (event.pos.x, event.pos.y),
                current: (event.pos.x, event.pos.y),
                active: false,
            });
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Empty tab bar area
        HitTarget::GroupTabBarEmpty { group_id } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Editor gutter: interactive lanes (fold/marks) dispatch to their
        // owning feature instead of falling through to the default
        // focus/drag-select behavior (editor-decorations.md). No lane owner
        // has shipped yet, so those consume the press as a no-op.
        HitTarget::EditorGutter { group_id, lane, .. } => {
            match interactive_gutter_lane_click(*lane) {
                Some(result) => result,
                None => {
                    if *group_id != model.editor_area.focused_group_id {
                        update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
                    }
                    // For now, treat like editor content click
                    // Future: could select entire line
                    EventResult::consumed_with_focus(FocusTarget::Editor)
                }
            }
        }

        // Editor content - handled specially due to complex selection logic
        HitTarget::EditorContent { group_id, .. } => {
            handle_editor_content_click(model, renderer, *group_id, event, click_tracker)
        }

        // CSV cell click - use renderer to find actual cell
        HitTarget::CsvCell { group_id, .. } => {
            use token::messages::CsvMsg;

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            // Use renderer to find the actual cell at this position
            if let Some(cell) = renderer.pixel_to_csv_cell(event.pos.x, event.pos.y, model) {
                update(
                    model,
                    Msg::Csv(CsvMsg::SelectCell {
                        row: cell.row,
                        col: cell.col,
                    }),
                );
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Image content - start panning
        HitTarget::ImageContent { group_id, .. } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(
                model,
                Msg::Image(ImageMsg::StartPan {
                    x: event.pos.x,
                    y: event.pos.y,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Binary placeholder "Open with Default Application" button
        // Scrollbar thumb: begin drag
        HitTarget::ScrollbarThumbVertical {
            editor_id,
            grab_offset,
            track_y,
            track_h,
            thumb_h,
            max_scroll,
            ..
        } => {
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarThumbPressedVertical {
                    editor_id: *editor_id,
                    grab_offset: *grab_offset,
                    track_start: *track_y,
                    track_size: *track_h,
                    thumb_size: *thumb_h,
                    max_scroll: *max_scroll,
                }),
            );
            EventResult::consumed_redraw()
        }

        HitTarget::ScrollbarThumbHorizontal {
            editor_id,
            grab_offset,
            track_x,
            track_w,
            thumb_w,
            max_scroll,
            ..
        } => {
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarThumbPressedHorizontal {
                    editor_id: *editor_id,
                    grab_offset: *grab_offset,
                    track_start: *track_x,
                    track_size: *track_w,
                    thumb_size: *thumb_w,
                    max_scroll: *max_scroll,
                }),
            );
            EventResult::consumed_redraw()
        }

        // Scrollbar track: click to jump
        HitTarget::ScrollbarTrackVertical {
            editor_id,
            coord,
            track_y,
            track_h,
            thumb_h,
            max_scroll,
            ..
        } => {
            let new_position = token::view::scrollbar::position_from_track_click(
                *coord,
                *track_y,
                *track_h,
                *thumb_h,
                *max_scroll,
            );
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarTrackClickedVertical {
                    editor_id: *editor_id,
                    new_position,
                }),
            );
            EventResult::consumed_redraw()
        }

        HitTarget::ScrollbarTrackHorizontal {
            editor_id,
            coord,
            track_x,
            track_w,
            thumb_w,
            max_scroll,
            ..
        } => {
            let new_position = token::view::scrollbar::position_from_track_click(
                *coord,
                *track_x,
                *track_w,
                *thumb_w,
                *max_scroll,
            );
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarTrackClickedHorizontal {
                    editor_id: *editor_id,
                    new_position,
                }),
            );
            EventResult::consumed_redraw()
        }

        HitTarget::BinaryPlaceholderButton { group_id } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            if let Some(editor) = model.editor_area.focused_editor() {
                if let token::model::TabContent::BinaryPlaceholder(ref state) = editor.tab_content {
                    let path = state.path.clone();
                    update(model, Msg::Layout(LayoutMsg::OpenWithDefaultApp(path)));
                }
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Dock resize handle
        HitTarget::DockResize { position } => {
            let initial_coord = match position {
                token::panel::DockPosition::Bottom => event.pos.y,
                _ => event.pos.x,
            };
            update(
                model,
                Msg::Dock(token::messages::DockMsg::StartResize {
                    position: *position,
                    initial_coord,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Dock tab click - activate panel (never toggles the dock closed)
        HitTarget::DockTab { panel_id, .. } => {
            update(
                model,
                Msg::Dock(token::messages::DockMsg::ActivatePanel(*panel_id)),
            );
            EventResult::consumed_redraw()
        }

        // Dock tab bar empty area
        HitTarget::DockTabBarEmpty { position } => {
            // Focus the dock
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusDock(*position)),
            );
            EventResult::consumed_redraw()
        }

        // Dock content area - handle panel-specific interactions
        HitTarget::DockContent {
            position,
            active_panel_id,
        } => {
            // Focus the dock first
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusDock(*position)),
            );

            // Handle outline panel clicks — row geometry from the same
            // solved chrome the renderer painted, wherever the panel is
            // docked.
            if *active_panel_id == token::panel::PanelId::Outline {
                use token::layout::UiKey;
                use token::messages::OutlineMsg;

                let chrome = token::layout::chrome::chrome(model);
                let Some(rows) = chrome.row_list(UiKey::PanelRows(token::panel::PanelId::Outline))
                else {
                    return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
                };

                if let Some(clicked_index) = rows.row_at_y(event.pos.y as f32) {
                    let outline = model
                        .editor_area
                        .focused_document()
                        .and_then(|doc| doc.outline.as_ref());

                    if let Some(outline) = outline {
                        if let Some(row) = visible_tree_row_at_index(
                            &outline.roots,
                            clicked_index,
                            |node: &token::outline::OutlineNode| {
                                node.is_collapsible() && !model.outline_panel.is_collapsed(node)
                            },
                        ) {
                            let tree = token::view::geometry::TreeListLayout::outline_from_metrics(
                                &model.metrics,
                            );
                            let on_chevron = row.node.is_collapsible()
                                && tree.is_on_chevron(rows.rect().x, row.depth, event.pos.x as f32);

                            let click_count = click_tracker
                                .track_click(ClickRegion::Outline { row: clicked_index });

                            update(
                                model,
                                Msg::Outline(OutlineMsg::ClickRow {
                                    index: clicked_index,
                                    click_count,
                                    on_chevron,
                                }),
                            );
                        }
                    }
                }

                return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
            }

            // Handle problems panel clicks — same solved-chrome geometry.
            if *active_panel_id == token::panel::PanelId::Problems {
                use token::layout::UiKey;
                use token::messages::ProblemsMsg;
                use token::update::problems::problems_rows;

                let chrome = token::layout::chrome::chrome(model);
                let Some(rows_view) =
                    chrome.row_list(UiKey::PanelRows(token::panel::PanelId::Problems))
                else {
                    return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
                };

                if let Some(clicked_index) = rows_view.row_at_y(event.pos.y as f32) {
                    let rows = problems_rows(model);
                    if let Some(row) = rows.get(clicked_index) {
                        // Only File rows (depth 0) have a chevron.
                        let tree = token::view::geometry::TreeListLayout::outline_from_metrics(
                            &model.metrics,
                        );
                        let on_chevron =
                            matches!(row, token::update::problems::ProblemsRow::File { .. })
                                && tree.is_on_chevron(rows_view.rect().x, 0, event.pos.x as f32);

                        let click_count =
                            click_tracker.track_click(ClickRegion::Problems { row: clicked_index });
                        update(
                            model,
                            Msg::Problems(ProblemsMsg::ClickRow {
                                index: clicked_index,
                                click_count,
                                on_chevron,
                            }),
                        );
                    }
                }

                return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
            }

            // The left dock hosts the file explorer; other dock content handled
            // above has already returned with its dock focus.
            match position {
                token::panel::DockPosition::Left => {
                    EventResult::consumed_with_focus(FocusTarget::Dock(*position))
                }
                _ => EventResult::consumed_redraw(),
            }
        }
    }
}

/// Handle editor content click with full selection logic
fn handle_editor_content_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    group_id: token::model::editor_area::GroupId,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    use token::messages::EditorMsg;
    use token::model::FocusTarget;

    // Focus group if needed
    if group_id != model.editor_area.focused_group_id {
        update(model, Msg::Layout(LayoutMsg::FocusGroup(group_id)));
    }

    // Non-text tabs: double-click opens binary placeholder with default app, ignore other clicks
    if let Some(editor) = model.editor_area.focused_editor() {
        match &editor.tab_content {
            token::model::TabContent::BinaryPlaceholder(state) => {
                let click_count =
                    click_tracker.track_click(ClickRegion::BinaryPlaceholder { group: group_id });
                if click_count >= 2 {
                    let path = state.path.clone();
                    update(model, Msg::Layout(LayoutMsg::OpenWithDefaultApp(path)));
                }
                return EventResult::consumed_with_focus(FocusTarget::Editor);
            }
            token::model::TabContent::Text => {}
        }
    }

    // Convert pixel to cursor position
    let (line, column) = renderer.pixel_to_cursor(event.pos.x, event.pos.y, model);

    // Track clicks for double/triple detection
    let click_count = click_tracker.track_click(ClickRegion::Editor {
        group: group_id,
        line,
        column,
    });

    // Handle modifiers
    if event.cmd() {
        // Cmd+Click = go to definition at the clicked position (JetBrains /
        // VS Code convention: the caret moves to the click first).
        update(
            model,
            Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
        );
        update(model, Msg::Lsp(token::messages::LspMsg::GotoDefinition));
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    if event.shift() {
        update(
            model,
            Msg::Editor(EditorMsg::ExtendSelectionToPosition { line, column }),
        );
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    if event.alt() {
        update(
            model,
            Msg::Editor(EditorMsg::ToggleCursorAtPosition { line, column }),
        );
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    // Handle click count
    match click_count {
        2 => {
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
            update(model, Msg::Editor(EditorMsg::SelectWord));
        }
        3 => {
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
            update(model, Msg::Editor(EditorMsg::SelectLine));
        }
        _ => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
        }
    }

    EventResult::consumed_with_focus(FocusTarget::Editor)
}

/// Handle middle mouse button clicks
fn handle_middle_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
) -> EventResult {
    match target {
        // Status bar - ignore
        HitTarget::StatusBar => EventResult::consumed_no_redraw(),

        // Preview header - middle click closes preview
        HitTarget::PreviewHeader { .. } => {
            update(model, Msg::Preview(PreviewMsg::Close));
            EventResult::consumed_redraw()
        }

        // Preview content - consume but no action (webview handles its own)
        HitTarget::PreviewContent { .. } => EventResult::consumed_no_redraw(),

        // Tab - middle click closes tab
        HitTarget::GroupTab {
            group_id, tab_id, ..
        } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(model, Msg::Layout(LayoutMsg::CloseTab(*tab_id)));
            EventResult::consumed_redraw()
        }

        // Empty tab bar area - consume but no action
        HitTarget::GroupTabBarEmpty { .. } => EventResult::consumed_no_redraw(),

        // Editor gutter - treat like editor content for rectangle selection,
        // unless an interactive lane (fold/marks) owns the click.
        HitTarget::EditorGutter { group_id, lane, .. } => {
            use token::messages::EditorMsg;

            if let Some(result) = interactive_gutter_lane_click(*lane) {
                return result;
            }

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            let (line, visual_col) =
                renderer.pixel_to_line_and_visual_column(event.pos.x, event.pos.y, model);
            update(
                model,
                Msg::Editor(EditorMsg::StartRectangleSelection { line, visual_col }),
            );
            EventResult::consumed_redraw()
        }

        // Editor content - start rectangle selection
        HitTarget::EditorContent { group_id, .. } => {
            use token::messages::EditorMsg;

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            let (line, visual_col) =
                renderer.pixel_to_line_and_visual_column(event.pos.x, event.pos.y, model);
            update(
                model,
                Msg::Editor(EditorMsg::StartRectangleSelection { line, visual_col }),
            );
            EventResult::consumed_redraw()
        }

        // CSV cell - no middle-click behavior
        HitTarget::CsvCell { .. } => EventResult::consumed_no_redraw(),

        // Modal - consume, no action
        HitTarget::Modal { .. } | HitTarget::ModalRow { .. } | HitTarget::ModalTab { .. } => {
            EventResult::consumed_no_redraw()
        }

        // Sidebar targets - consume, no action for middle-click
        HitTarget::SidebarEmpty | HitTarget::SidebarItem { .. } => {
            EventResult::consumed_no_redraw()
        }

        // Sidebar resize and splitters - consume, no action
        HitTarget::SidebarResize | HitTarget::Splitter { .. } => EventResult::consumed_no_redraw(),

        // Dock targets - consume, no special middle-click action
        HitTarget::DockResize { .. }
        | HitTarget::DockTab { .. }
        | HitTarget::DockTabBarEmpty { .. }
        | HitTarget::DockContent { .. } => EventResult::consumed_no_redraw(),

        // Binary placeholder button - no middle-click action
        HitTarget::BinaryPlaceholderButton { .. } => EventResult::consumed_no_redraw(),

        // Image content and scrollbars - no middle-click action
        HitTarget::ImageContent { .. }
        | HitTarget::ScrollbarThumbVertical { .. }
        | HitTarget::ScrollbarTrackVertical { .. }
        | HitTarget::ScrollbarThumbHorizontal { .. }
        | HitTarget::ScrollbarTrackHorizontal { .. } => EventResult::consumed_no_redraw(),

        // Cursor overlay - no middle-click action
        HitTarget::CursorOverlay { .. } => EventResult::consumed_no_redraw(),
    }
}

/// Handle right mouse button clicks (context menus - future)
/// Synchronous clipboard-content check for the context menu's Paste-
/// enablement gate (context-menu.md's `ContextMenuTarget::Editor::
/// clipboard_has_content`). Unlike `Cmd::RequestClipboardPaste` (which
/// round-trips through a worker thread — see `App`'s `Cmd` executor), this
/// reads inline on the UI thread: a menu builder needs the answer before
/// it can render a single frame, and a local clipboard read is a fast
/// syscall, not a blocking one.
/// ponytail: main-thread clipboard read; move off-thread if a slow/huge
/// clipboard payload is ever observed to jank right-click.
fn clipboard_has_content() -> bool {
    arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .is_ok_and(|text| !text.is_empty())
}

/// Resolve `target`/`event` into a `ContextMenuTarget` for the three V1
/// regions (editor content, tab, file-tree item) and open the menu.
/// Regions with no V1 menu (status bar, dock, ...) keep bubbling
/// (context-menu.md "Phase 2: Hit-Test Wiring & Open/Close").
fn handle_right_click(model: &mut AppModel, target: &HitTarget, event: &MouseEvent) -> EventResult {
    use token::context_menu::ContextMenuTarget;
    use token::messages::ContextMenuMsg;

    let menu_target = match target {
        HitTarget::EditorContent {
            group_id,
            editor_id,
            ..
        } => {
            // The menu is built (enablement) and later activated against
            // the clicked split, not whatever happened to be focused
            // before the click — focus it first, same as a left click
            // (`handle_editor_content_click`). Per JetBrains/VS Code
            // convention, also move the caret to the click point, unless
            // the click landed inside the existing selection (which must
            // survive so Cut/Copy still act on it).
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            let caret_target = model
                .editor_area
                .groups
                .get(group_id)
                .zip(model.editor_area.editors.get(editor_id))
                .and_then(|(group, editor)| {
                    let document = editor
                        .document_id
                        .and_then(|id| model.editor_area.documents.get(&id))?;
                    let (line, column) = token::view::geometry::pixel_to_cursor_in_group(
                        event.pos.x,
                        event.pos.y,
                        model.char_width,
                        model.line_height as f64,
                        &group.rect,
                        model,
                        editor,
                        document,
                    );
                    let pos = token::model::editor::Position::new(line, column);
                    (!editor.active_selection().contains(pos)).then_some((line, column))
                });
            if let Some((line, column)) = caret_target {
                update(
                    model,
                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                );
            }

            let has_selection = model
                .editor_area
                .editors
                .get(editor_id)
                .is_some_and(|e| !e.active_selection().is_empty());
            ContextMenuTarget::Editor {
                group_id: *group_id,
                has_selection,
                clipboard_has_content: clipboard_has_content(),
            }
        }
        HitTarget::GroupTab {
            group_id, tab_id, ..
        } => ContextMenuTarget::Tab {
            group_id: *group_id,
            tab_id: *tab_id,
            file_path: tab_file_path(model, *group_id, *tab_id),
        },
        HitTarget::SidebarItem { path, is_dir, .. } => ContextMenuTarget::FileTreeItem {
            path: path.clone(),
            is_dir: *is_dir,
        },
        _ => return EventResult::Bubble,
    };

    let anchor = (event.pos.x as usize, event.pos.y as usize, 0);
    let cmd = update(
        model,
        Msg::ContextMenu(ContextMenuMsg::Open {
            target: menu_target,
            anchor,
        }),
    );
    match cmd {
        Some(cmd) => EventResult::Consumed {
            redraw: false,
            focus: None,
            cmd: Some(cmd),
        },
        // `has_modal()` guard tripped inside `update_context_menu` — still
        // consume the click (no menu opened, nothing else should act on
        // it either).
        None => EventResult::consumed_no_redraw(),
    }
}

/// The file path backing `tab_id` in `group_id`, if any — `None` covers
/// both "untitled buffer" and "tab not found" (defensive; the tab that was
/// just right-clicked should always resolve).
fn tab_file_path(
    model: &AppModel,
    group_id: GroupId,
    tab_id: token::model::TabId,
) -> Option<std::path::PathBuf> {
    let group = model.editor_area.groups.get(&group_id)?;
    let tab = group.tabs.iter().find(|t| t.id == tab_id)?;
    let editor = model.editor_area.editors.get(&tab.editor_id)?;
    let document_id = editor.document_id?;
    model
        .editor_area
        .documents
        .get(&document_id)
        .and_then(|d| d.file_path.clone())
}

/// Horizontal `delta_px` for scrolling the editor tab strip from a wheel event,
/// or `None` when neither axis moved.
///
/// A horizontal gesture maps directly: positive `h_delta` reveals tabs further
/// right, matching editor horizontal scrolling (and `ScrollTabBar`, which
/// increases `tab_scroll` for positive `delta_px`). A vertical-only wheel (a
/// plain mouse with no X axis) is repurposed to scroll the strip, keeping the
/// legacy inverted sign so mouse-wheel behavior over the tabs is unchanged.
fn tab_bar_scroll_delta_px(h_delta: i32, v_delta: i32, scroll_step: i32) -> Option<i32> {
    if h_delta != 0 {
        Some(h_delta * scroll_step)
    } else if v_delta != 0 {
        Some(-v_delta * scroll_step)
    } else {
        None
    }
}

/// Handle mouse wheel scroll events, routing to the appropriate target
/// based on the current hover region.
pub fn handle_mouse_wheel(
    model: &mut AppModel,
    mouse_position: Option<(f64, f64)>,
    h_delta: i32,
    v_delta: i32,
) -> Option<Cmd> {
    use token::model::HoverRegion;

    match model.ui.hover {
        // Sidebar: scroll the file tree
        HoverRegion::Sidebar => {
            if v_delta != 0 {
                update(
                    model,
                    Msg::Workspace(WorkspaceMsg::Scroll { lines: v_delta }),
                )
            } else {
                None
            }
        }

        // Dock panels: route to panel-specific scroll handlers
        HoverRegion::Dock(position) => {
            let active_panel = match position {
                token::panel::DockPosition::Left => model.dock_layout.left.active_panel(),
                token::panel::DockPosition::Right => model.dock_layout.right.active_panel(),
                token::panel::DockPosition::Bottom => model.dock_layout.bottom.active_panel(),
            };
            if active_panel == Some(token::panel::PanelId::Outline) && v_delta != 0 {
                update(model, Msg::Outline(OutlineMsg::Scroll { lines: v_delta }))
            } else if active_panel == Some(token::panel::PanelId::PROBLEMS) && v_delta != 0 {
                update(
                    model,
                    Msg::Problems(token::messages::ProblemsMsg::Scroll { lines: v_delta }),
                )
            } else if active_panel == Some(token::panel::PanelId::TERMINAL) && v_delta != 0 {
                let lines = v_delta.unsigned_abs() as usize;
                let msg = if v_delta < 0 {
                    TerminalMsg::ScrollUp(lines)
                } else {
                    TerminalMsg::ScrollDown(lines)
                };
                update(model, Msg::Terminal(msg))
            } else {
                None
            }
        }

        // Preview panes: webview handles its own scrolling
        HoverRegion::Preview => None,

        // Editor tab bar: scroll the tabs horizontally
        HoverRegion::EditorTabBar => {
            let scroll_step = (model.line_height as i32).max(1);
            let delta_px = tab_bar_scroll_delta_px(h_delta, v_delta, scroll_step)?;
            // Find which group's tab bar is under the cursor
            let (x, y) = mouse_position?;
            let pt = token::view::hit_test::Point::new(x, y);
            let group_id = model.editor_area.groups.values().find_map(|group| {
                let layout =
                    token::view::geometry::TabBarLayout::new(group, model, model.char_width);
                layout.contains(pt.x, pt.y).then_some(group.id)
            })?;
            update(
                model,
                Msg::Layout(LayoutMsg::ScrollTabBar { group_id, delta_px }),
            )
        }

        // Modal: scroll the visible window by 3 rows, selection unchanged
        // (overlay-surface.md Pointer: "Scroll wheel moves the viewport by
        // 3 rows without moving selection").
        HoverRegion::Modal => {
            if v_delta == 0 {
                return None;
            }
            let rows = (v_delta.signum() * 3) as isize;
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Scroll(rows))))
        }

        // Cursor overlay: scroll its own window, same 3-rows-per-notch
        // convention as modals (overlay-surface.md Phase 5).
        HoverRegion::CursorOverlay => {
            if v_delta == 0 {
                return None;
            }
            let completion_rows = model
                .ui
                .completion_menu
                .as_ref()
                .map(|m| m.filtered.len())
                .unwrap_or(0);
            let reference_rows = model.ui.reference_list.as_ref().map_or(0, Vec::len);
            let Some(state) = &mut model.ui.cursor_overlay else {
                return None;
            };
            let max_scroll = match state.kind {
                token::model::CursorOverlayKind::DebugCompletion => {
                    token::view::modal::debug_completion_row_count()
                        .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION)
                }
                token::model::CursorOverlayKind::DebugHover
                | token::model::CursorOverlayKind::Hover => 0,
                token::model::CursorOverlayKind::Completion => completion_rows
                    .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION),
                token::model::CursorOverlayKind::References => reference_rows
                    .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION),
                // No scroll behavior needed for V1 (menus fit without
                // scrolling) — inert, same as Hover (context-menu.md
                // "Mouse: click-away consumes").
                token::model::CursorOverlayKind::ContextMenu => 0,
            };
            let delta = v_delta.signum() * 3;
            state.scroll = if delta < 0 {
                state.scroll.saturating_sub(delta.unsigned_abs() as usize)
            } else {
                state.scroll.saturating_add(delta as usize).min(max_scroll)
            };
            Some(Cmd::Redraw)
        }

        // StatusBar/Splitter/DockResize/Button: ignore scroll
        HoverRegion::StatusBar
        | HoverRegion::Splitter
        | HoverRegion::SidebarResize
        | HoverRegion::DockResize(_)
        | HoverRegion::Button(_)
        | HoverRegion::None => None,

        // Editor text area: scroll the editor or delegate to specialized modes.
        HoverRegion::EditorText => {
            let in_image_mode = model
                .editor_area
                .focused_editor()
                .map(|e| e.view_mode.is_image())
                .unwrap_or(false);

            if in_image_mode {
                if v_delta != 0 {
                    let (mouse_x, mouse_y) = mouse_position.unwrap_or((0.0, 0.0));
                    return update(
                        model,
                        Msg::Image(ImageMsg::Zoom {
                            delta: v_delta as f64,
                            mouse_x,
                            mouse_y,
                        }),
                    );
                }
                return None;
            }

            let in_csv_mode = model
                .editor_area
                .focused_editor()
                .map(|e| e.view_mode.is_csv())
                .unwrap_or(false);

            if in_csv_mode {
                let v_cmd = if v_delta != 0 {
                    update(model, Msg::Csv(CsvMsg::ScrollVertical(v_delta)))
                } else {
                    None
                };
                let h_cmd = if h_delta != 0 {
                    update(model, Msg::Csv(CsvMsg::ScrollHorizontal(h_delta)))
                } else {
                    None
                };
                return v_cmd.or(h_cmd);
            }

            let v_cmd = if v_delta != 0 {
                update(model, Msg::Editor(EditorMsg::Scroll(v_delta)))
            } else {
                None
            };
            let h_cmd = if h_delta != 0 {
                update(model, Msg::Editor(EditorMsg::ScrollHorizontal(h_delta)))
            } else {
                None
            };
            v_cmd.or(h_cmd)
        }
    }
}
