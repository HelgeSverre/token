//! `ContextMenuMsg` handlers (context-menu.md).
//!
//! Menus are built here (region builders in `context_menu::builders`), not
//! in `App` — no `Keymap` needs threading through (item hints come from the
//! `commands::COMMANDS` registry, per the doc's "Adjustment 2"). Anything
//! requiring I/O (clipboard content for `Editor` targets, the caret's pixel
//! rect for Shift+F10) is already resolved by the caller
//! (`runtime/mouse.rs::handle_right_click`, `App::open_context_menu_at_caret`)
//! before the `Open` message reaches here.

use crate::commands::Cmd;
use crate::context_menu::{self, ContextMenuTarget, MenuAction};
use crate::messages::ContextMenuMsg;
use crate::model::{AppModel, ContextMenuState, CursorOverlayKind, CursorOverlayState};

pub fn update_context_menu(model: &mut AppModel, msg: ContextMenuMsg) -> Option<Cmd> {
    match msg {
        ContextMenuMsg::Open { target, anchor } => open_menu(model, target, anchor),
        ContextMenuMsg::ActivateItem { index } => activate_item(model, index),
        ContextMenuMsg::RevealPathInFinder(path) => Some(Cmd::Batch(vec![
            Cmd::RevealFileInFinder { path },
            Cmd::Redraw,
        ])),
        ContextMenuMsg::CopyPath { path, relative } => copy_path(model, path, relative),
    }
}

/// Guard: don't open while a modal is active, or another cursor overlay is
/// already open (close the old one first, matching Completion's re-open
/// behavior) — Phase 2's checklist.
fn open_menu(
    model: &mut AppModel,
    target: ContextMenuTarget,
    anchor: (usize, usize, usize),
) -> Option<Cmd> {
    if model.ui.has_modal() {
        return None;
    }
    let region = target.region();
    let items = context_menu::build_menu(model, &target);
    let selected = context_menu::first_enabled_index(&items);
    // Another cursor overlay already open -> close it first (Phase 2's
    // guard, matching the mouse click-away path at
    // `runtime/mouse.rs::handle_mouse_press`): a sibling left populated
    // but unrendered would otherwise be mistaken for still-open by
    // whatever reads it next (e.g. completion re-triggering on the next
    // edit via `sync_after_document_edit`'s `menu_open` check).
    model.ui.completion_menu = None;
    model.ui.hover_card = None;
    model.ui.reference_list = None;
    let mut overlay = CursorOverlayState::new(CursorOverlayKind::ContextMenu);
    overlay.selected = selected;
    model.ui.cursor_overlay = Some(overlay);
    model.ui.context_menu = Some(ContextMenuState {
        items,
        anchor,
        region,
    });
    Some(Cmd::Redraw)
}

/// Shift+F10 / `CommandId::ShowContextMenu`: open the editor menu for the
/// live cursor/selection state, anchored at the text caret rect (the same
/// geometry the caret and completion popup use) instead of a click point —
/// context-menu.md's "Keyboard Trigger".
///
/// `clipboard_has_content` is resolved by the caller: `App::
/// open_context_menu_at_caret` (the real Shift+F10 path, via
/// `App::dispatch_command`) does a live clipboard read before calling in,
/// since `update()` must not do I/O; `execute_command`'s
/// `CommandId::ShowContextMenu` arm (the palette-invocation path) has no
/// such runtime access and passes `false` — Paste shows disabled when the
/// command is run from the palette rather than the keyboard shortcut, a
/// minor cosmetic gap in an already-unusual invocation path.
pub fn open_editor_menu_at_caret(model: &mut AppModel, clipboard_has_content: bool) -> Option<Cmd> {
    let group_id = model.editor_area.focused_group_id;
    let has_selection = !model.editor().active_selection().is_empty();
    let rect =
        crate::view::caret::active_text_input_rect(model, model.char_width, model.line_height)?;
    let anchor = (rect.x, rect.y, rect.h);
    let target = ContextMenuTarget::Editor {
        group_id,
        has_selection,
        clipboard_has_content,
    };
    open_menu(model, target, anchor)
}

fn close_menu(model: &mut AppModel) {
    model.ui.cursor_overlay = None;
    model.ui.context_menu = None;
}

/// Run the selectable-space item at `index` (see
/// `context_menu::selectable_items`) and dismiss. A disabled item (or an
/// out-of-range index) is a no-op that leaves the menu open — matches the
/// "disabled items don't activate" testing-strategy rule.
fn activate_item(model: &mut AppModel, index: usize) -> Option<Cmd> {
    let action = model
        .ui
        .context_menu
        .as_ref()
        .and_then(|state| context_menu::selectable_items(&state.items).nth(index))
        .filter(|item| item.enabled)
        .map(|item| item.action.clone());

    let Some(action) = action else {
        return Some(Cmd::Redraw);
    };
    close_menu(model);

    match action {
        MenuAction::Command(id) => super::execute_command(model, id),
        MenuAction::Messages(msgs) => {
            let mut result = None;
            for m in msgs {
                result = super::merge_cmds(result, super::update(model, m));
            }
            result.or(Some(Cmd::Redraw))
        }
        MenuAction::None => Some(Cmd::Redraw),
    }
}

/// Copy an absolute or workspace-relative path — the same logic as
/// `execute_command`'s `CopyAbsolutePath`/`CopyRelativePath` arms, targeted
/// at an explicit path instead of `model.document()`.
fn copy_path(model: &mut AppModel, path: std::path::PathBuf, relative: bool) -> Option<Cmd> {
    let text = if relative {
        model
            .workspace_root()
            .and_then(|root| path.strip_prefix(root).ok())
            .map(|rel| rel.display().to_string())
            .unwrap_or_else(|| path.display().to_string())
    } else {
        path.display().to_string()
    };
    model.ui.set_status(format!("Copied: {}", text));
    Some(Cmd::Batch(vec![Cmd::CopyToClipboard(text), Cmd::Redraw]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_menu::MenuItem;
    use crate::messages::Msg;
    use crate::model::editor_area::TabId;

    fn model_with_menu(items: Vec<MenuItem>) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let selected = context_menu::first_enabled_index(&items);
        let mut overlay = CursorOverlayState::new(CursorOverlayKind::ContextMenu);
        overlay.selected = selected;
        model.ui.cursor_overlay = Some(overlay);
        model.ui.context_menu = Some(ContextMenuState {
            items,
            anchor: (10, 20, 0),
            region: crate::context_menu::ContextMenuRegion::Editor,
        });
        model
    }

    #[test]
    fn activate_item_on_a_disabled_row_is_a_no_op_and_leaves_the_menu_open() {
        let mut model = model_with_menu(vec![
            MenuItem::custom("Close", false, vec![]),
            MenuItem::custom(
                "Other",
                true,
                vec![Msg::Layout(crate::messages::LayoutMsg::CloseFocusedTab)],
            ),
        ]);
        activate_item(&mut model, 0);
        assert!(
            model.ui.context_menu.is_some(),
            "disabled item must not close the menu"
        );
        assert!(model.ui.cursor_overlay.is_some());
    }

    #[test]
    fn activate_item_runs_a_command_action_and_closes_the_menu() {
        let mut model = model_with_menu(vec![MenuItem::from_command(
            crate::commands::CommandId::CloseTab,
            "Close Tab",
            true,
        )]);
        activate_item(&mut model, 0);
        assert!(model.ui.context_menu.is_none(), "activation dismisses");
        assert!(model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn activate_item_skips_separators_when_indexing() {
        let group_id = crate::model::GroupId(0);
        let items = vec![
            MenuItem::custom("Close", true, vec![]),
            MenuItem::separator(),
            MenuItem::custom(
                "Close Others",
                true,
                vec![Msg::Layout(crate::messages::LayoutMsg::CloseOtherTabs {
                    group_id,
                    keep: TabId(1),
                })],
            ),
        ];
        // index 1 in *selectable* space (separators excluded) is "Close
        // Others", not the separator that sits at items[1].
        let mut model = model_with_menu(items);
        activate_item(&mut model, 1);
        assert!(model.ui.context_menu.is_none());
    }

    #[test]
    fn open_menu_closes_a_sibling_completion_popup_left_open() {
        // Phase 2's guard: opening the context menu while another cursor
        // overlay (e.g. completion, reachable via Shift+F10 passthrough)
        // is already open must close the sibling's state too, not just
        // overwrite `cursor_overlay` — otherwise `completion_menu` stays
        // populated but unrendered and `sync_after_document_edit` treats
        // completion as still open on the next edit.
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let document_id = model.document().id.unwrap();
        model.ui.completion_menu = Some(crate::completion::menu::CompletionMenuState {
            document_id,
            revision: 0,
            query_start: crate::model::Cursor::at(0, 0),
            items: vec![],
            filtered: vec![],
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        let target = ContextMenuTarget::FileTreeItem {
            path: "/tmp/x".into(),
            is_dir: false,
        };
        open_menu(&mut model, target, (0, 0, 0));

        assert!(
            model.ui.completion_menu.is_none(),
            "the sibling completion popup's state must be cleared, not just overwritten"
        );
        assert_eq!(
            model.ui.cursor_overlay.map(|s| s.kind),
            Some(CursorOverlayKind::ContextMenu)
        );
    }

    #[test]
    fn open_menu_is_a_no_op_while_a_modal_is_active() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.ui.active_modal = Some(crate::model::ModalState::GotoLine(Default::default()));
        let target = ContextMenuTarget::FileTreeItem {
            path: "/tmp/x".into(),
            is_dir: false,
        };
        let cmd = open_menu(&mut model, target, (0, 0, 0));
        assert!(cmd.is_none());
        assert!(model.ui.context_menu.is_none());
    }
}
