//! Per-region menu builders (context-menu.md "V1 Scope: Regions & Menus").
//! Each builder is `fn(&AppModel, &ContextMenuTarget) -> Vec<MenuItem>` —
//! pure, no I/O (the target already carries anything that needed I/O to
//! resolve, e.g. `clipboard_has_content`).

use std::path::{Path, PathBuf};

use crate::commands::CommandId;
use crate::messages::{ContextMenuMsg, LayoutMsg, Msg, WorkspaceMsg};
use crate::model::editor_area::{GroupId, TabId};
use crate::model::AppModel;

use super::{ContextMenuTarget, MenuAction, MenuItem};

/// Build the menu for `target` — dispatches to the matching per-region
/// builder (context-menu.md "V1 Scope").
pub(super) fn build_menu(model: &AppModel, target: &ContextMenuTarget) -> Vec<MenuItem> {
    let mut context = crate::keymap::KeyContext::for_command_hints(model);
    let mut items = match target {
        ContextMenuTarget::Editor {
            group_id,
            clipboard_has_content,
        } => {
            let group = model.editor_area.groups.get(group_id);
            let editor = group
                .and_then(|group| group.active_editor_id())
                .and_then(|id| model.editor_area.editors.get(&id));
            let doc = group.and_then(|group| model.editor_area.document_for_group(group));
            context.editor_focused = true;
            context.sidebar_focused = false;
            context.has_selection = editor.is_some_and(|e| !e.active_selection().is_empty());
            context.has_multiple_cursors = editor.is_some_and(|e| e.has_multiple_cursors());
            build_editor_menu(model, doc, context.has_selection, *clipboard_has_content)
        }
        ContextMenuTarget::Tab { group_id, tab_id } => build_tab_menu(model, *group_id, *tab_id),
        ContextMenuTarget::FileTreeItem { path, is_dir } => {
            context.editor_focused = false;
            context.sidebar_focused = true;
            build_file_tree_menu(model, path, *is_dir)
        }
    };
    for item in &mut items {
        if let MenuAction::Command(id) = item.action {
            item.shortcut_hint =
                crate::commands::keybinding_for_command(id, &model.ui.keymap, &context);
        }
    }
    items
}

/// Whether an LSP server is registered for `language` and tracked in the
/// model mirror at all — an approximation of "LSP available at cursor"
/// (no per-document capability snapshot reaches the model; see
/// `LspUiState::servers`'s doc comment). Good enough to gate a menu item;
/// the request itself still falls back to a status transient
/// (`NotSupported`/`StillIndexing`) if this is too optimistic.
fn lsp_available_for(model: &AppModel, language: crate::syntax::LanguageId) -> bool {
    crate::lsp::server_id_for_language(language, &model.config.lsp).is_some_and(|id| {
        model
            .lsp
            .servers
            .contains_key(&crate::lsp::LspServerId::from(id))
    })
}

fn build_editor_menu(
    model: &AppModel,
    doc: Option<&crate::model::Document>,
    has_selection: bool,
    clipboard_has_content: bool,
) -> Vec<MenuItem> {
    let lsp_available = doc.is_some_and(|d| lsp_available_for(model, d.language));
    let has_path = doc.is_some_and(|d| d.file_path.is_some());

    vec![
        MenuItem::from_command(CommandId::Cut, "Cut", has_selection),
        MenuItem::from_command(CommandId::Copy, "Copy", has_selection),
        MenuItem::from_command(CommandId::Paste, "Paste", clipboard_has_content),
        MenuItem::separator(),
        MenuItem::from_command(CommandId::GotoDefinition, "Go to Definition", lsp_available),
        MenuItem::from_command(CommandId::ShowUsages, "Show Usages", lsp_available),
        MenuItem::from_command(CommandId::ShowHover, "Show Hover", true),
        MenuItem::separator(),
        MenuItem::from_command(
            CommandId::RevealInSidebar,
            "Reveal in File Explorer",
            has_path,
        ),
    ]
}

fn build_tab_menu(model: &AppModel, group_id: GroupId, tab_id: TabId) -> Vec<MenuItem> {
    let group = model.editor_area.groups.get(&group_id);
    let other_tabs = group.is_some_and(|g| g.tabs.len() > 1);
    let file_path = group
        .and_then(|g| g.tabs.iter().find(|tab| tab.id == tab_id))
        .and_then(|tab| model.editor_area.editors.get(&tab.editor_id))
        .and_then(|editor| editor.document_id)
        .and_then(|id| model.editor_area.documents.get(&id))
        .and_then(|doc| doc.file_path.as_deref());
    let has_path = file_path.is_some();

    let mut items = vec![
        MenuItem::custom(
            "Close",
            true,
            vec![Msg::Layout(LayoutMsg::CloseTab(tab_id))],
        ),
        MenuItem::custom(
            "Close Others",
            other_tabs,
            vec![Msg::Layout(LayoutMsg::CloseOtherTabs {
                group_id,
                keep: tab_id,
            })],
        ),
        MenuItem::custom(
            "Close All",
            true,
            vec![Msg::Layout(LayoutMsg::CloseAllTabs { group_id })],
        ),
        MenuItem::separator(),
    ];
    items.push(MenuItem::custom(
        "Reveal in File Explorer",
        has_path,
        vec![Msg::Workspace(WorkspaceMsg::RevealPath(
            file_path.map(Path::to_path_buf).unwrap_or_default(),
        ))],
    ));
    items.extend(path_action_items(model, file_path));
    items
}

fn build_file_tree_menu(model: &AppModel, path: &Path, is_dir: bool) -> Vec<MenuItem> {
    let open_action = if is_dir {
        Msg::Workspace(WorkspaceMsg::ToggleFolder(path.to_path_buf()))
    } else {
        Msg::Workspace(WorkspaceMsg::OpenFile {
            path: path.to_path_buf(),
            preview: false,
        })
    };
    // context-menu.md's File Tree table lists only Reveal in Finder / Copy
    // Absolute Path / Copy Relative Path — no "Reveal in File Explorer"
    // row (that's the Tab menu's item, which reveals a *different* file
    // than the one already showing in the tree). `has_workspace` reflects
    // the real workspace state rather than being hardcoded true, so "Copy
    // Relative Path" is correctly disabled if `copy_path` would otherwise
    // silently fall back to the absolute path.
    let mut items = vec![
        MenuItem::custom("Open", true, vec![open_action]),
        MenuItem::separator(),
    ];
    items.extend(path_action_items(model, Some(path)));
    items.push(MenuItem::separator());
    items.push(MenuItem::custom(
        "Refresh Tree",
        true,
        vec![Msg::Workspace(WorkspaceMsg::Refresh)],
    ));
    items
}

/// The three path-targeted rows shared by the Tab and File Tree menus
/// (context-menu.md's tables list them identically): Reveal in Finder,
/// Copy Absolute Path, Copy Relative Path.
fn path_action_items(model: &AppModel, file_path: Option<&Path>) -> Vec<MenuItem> {
    let has_path = file_path.is_some();
    let has_workspace = model.workspace_root().is_some();
    let path: PathBuf = file_path.map(Path::to_path_buf).unwrap_or_default();
    vec![
        MenuItem::custom(
            "Reveal in Finder",
            has_path,
            vec![Msg::ContextMenu(ContextMenuMsg::RevealPathInFinder(
                path.clone(),
            ))],
        ),
        MenuItem::custom(
            "Copy Absolute Path",
            has_path,
            vec![Msg::ContextMenu(ContextMenuMsg::CopyPath {
                path: path.clone(),
                relative: false,
            })],
        ),
        MenuItem::custom(
            "Copy Relative Path",
            has_path && has_workspace,
            vec![Msg::ContextMenu(ContextMenuMsg::CopyPath {
                path,
                relative: true,
            })],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Position, Selection};

    #[test]
    fn shortcut_hints_use_live_bindings_and_the_target_selection_context() {
        use crate::keymap::{
            Command, Condition, KeyCode, Keybinding, Keymap, Keystroke, Modifiers,
        };
        let mut model = AppModel::new(800, 600, 1.0);
        let stroke = Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL);
        model.ui.keymap = Keymap::with_bindings(vec![
            Keybinding::new(stroke, Command::Copy).when_single(Condition::HasSelection)
        ]);
        model.document_mut().buffer = ropey::Rope::from_str("text");
        for has_selection in [false, true] {
            model.editor_mut().selections[0] = Selection::from_positions(
                Position::new(0, 0),
                Position::new(0, if has_selection { 4 } else { 0 }),
            );
            let target = ContextMenuTarget::Editor {
                group_id: model.editor_area.focused_group_id,
                clipboard_has_content: false,
            };
            let items = build_menu(&model, &target);
            assert_eq!(
                items
                    .iter()
                    .find(|item| item.label == "Copy")
                    .unwrap()
                    .shortcut_hint,
                has_selection.then(|| stroke.display_string())
            );
        }
        model.ui.keymap = Keymap::new();
        let target = ContextMenuTarget::Editor {
            group_id: model.editor_area.focused_group_id,
            clipboard_has_content: false,
        };
        assert!(build_menu(&model, &target)
            .iter()
            .all(|item| item.shortcut_hint.is_none()));
    }

    fn label(items: &[MenuItem]) -> Vec<&str> {
        items
            .iter()
            .map(|i| {
                if i.is_separator {
                    "---"
                } else {
                    i.label.as_str()
                }
            })
            .collect()
    }

    #[test]
    fn editor_menu_disables_cut_copy_without_selection_and_paste_without_clipboard() {
        let model = AppModel::new(800, 600, 1.0);
        let group_id = model.editor_area.focused_group_id;
        let target = ContextMenuTarget::Editor {
            group_id,
            clipboard_has_content: false,
        };
        let items = build_menu(&model, &target);
        assert_eq!(
            label(&items),
            vec![
                "Cut",
                "Copy",
                "Paste",
                "---",
                "Go to Definition",
                "Show Usages",
                "Show Hover",
                "---",
                "Reveal in File Explorer",
            ]
        );
        assert!(!items[0].enabled, "Cut needs a selection");
        assert!(!items[1].enabled, "Copy needs a selection");
        assert!(!items[2].enabled, "Paste needs clipboard content");
        assert!(items[6].enabled, "Show Hover is always enabled");
    }

    #[test]
    fn editor_menu_enables_cut_copy_paste_when_available() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str("text");
        model.editor_mut().selections[0] =
            Selection::from_positions(Position::new(0, 0), Position::new(0, 4));
        let group_id = model.editor_area.focused_group_id;
        crate::update::update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(
                crate::model::SplitDirection::Vertical,
            )),
        );
        model.editor_mut().selections[0] = Selection::new(Position::new(0, 0));
        assert_ne!(model.editor_area.focused_group_id, group_id);
        let target = ContextMenuTarget::Editor {
            group_id,
            clipboard_has_content: true,
        };
        let items = build_menu(&model, &target);
        assert!(items[0].enabled);
        assert!(items[1].enabled);
        assert!(items[2].enabled);
    }

    #[test]
    fn tab_menu_disables_close_others_with_a_single_tab() {
        let model = AppModel::new(800, 600, 1.0);
        let group_id = model.editor_area.focused_group_id;
        let tab_id = model
            .editor_area
            .groups
            .get(&group_id)
            .unwrap()
            .active_tab()
            .unwrap()
            .id;
        let items = build_tab_menu(&model, group_id, tab_id);
        let close_others = items.iter().find(|i| i.label == "Close Others").unwrap();
        assert!(
            !close_others.enabled,
            "Close Others is disabled with only one tab"
        );
        // No file path: every path-targeted row is disabled.
        for label in [
            "Reveal in File Explorer",
            "Reveal in Finder",
            "Copy Absolute Path",
            "Copy Relative Path",
        ] {
            assert!(
                !items.iter().find(|i| i.label == label).unwrap().enabled,
                "{label} needs a saved file path"
            );
        }
    }

    #[test]
    fn tab_menu_targets_the_clicked_tab_not_the_focused_one() {
        let mut model = AppModel::new(800, 600, 1.0);
        let group_id = model.editor_area.focused_group_id;
        let clicked_tab = model.editor_area.groups[&group_id].active_tab().unwrap().id;
        let clicked_path = PathBuf::from("/tmp/clicked.rs");
        model.document_mut().file_path = Some(clicked_path.clone());
        crate::update::update(&mut model, Msg::Layout(LayoutMsg::NewTab));
        model.document_mut().file_path = Some(PathBuf::from("/tmp/focused.rs"));
        let menu = super::super::ContextMenuState::new(
            &model,
            &ContextMenuTarget::Tab {
                group_id,
                tab_id: clicked_tab,
            },
            (0, 0, 0),
        );
        let items = &menu.items;
        let close = items.iter().find(|i| i.label == "Close").unwrap();
        match &close.action {
            crate::context_menu::MenuAction::Messages(msgs) => {
                assert!(matches!(
                    msgs.as_slice(),
                    [Msg::Layout(LayoutMsg::CloseTab(id))] if *id == clicked_tab
                ));
            }
            _ => panic!("expected a Messages action"),
        }
        let copy = items
            .iter()
            .find(|i| i.label == "Copy Absolute Path")
            .unwrap();
        assert!(copy.enabled);
        assert!(matches!(
            &copy.action,
            MenuAction::Messages(msgs) if matches!(
                msgs.as_slice(),
                [Msg::ContextMenu(ContextMenuMsg::CopyPath { path, relative: false })]
                    if path == &clicked_path
            )
        ));
    }

    #[test]
    fn file_tree_menu_opens_a_directory_via_toggle_folder() {
        let model = AppModel::new(800, 600, 1.0);
        let path = PathBuf::from("/tmp/some/dir");
        let items = build_file_tree_menu(&model, &path, true);
        let open = items.iter().find(|i| i.label == "Open").unwrap();
        match &open.action {
            crate::context_menu::MenuAction::Messages(msgs) => {
                assert!(matches!(
                    msgs.as_slice(),
                    [Msg::Workspace(WorkspaceMsg::ToggleFolder(p))] if p == &path
                ));
            }
            _ => panic!("expected a Messages action"),
        }
    }

    #[test]
    fn file_tree_menu_enables_all_path_rows_for_a_saved_file() {
        let model = AppModel::new(800, 600, 1.0);
        let path = PathBuf::from("/tmp/some/file.rs");
        let items = build_file_tree_menu(&model, &path, false);
        for label in ["Reveal in Finder", "Copy Absolute Path", "Refresh Tree"] {
            assert!(items.iter().find(|i| i.label == label).unwrap().enabled);
        }
    }

    #[test]
    fn file_tree_menu_has_no_reveal_in_file_explorer_row() {
        // context-menu.md's File Tree table doesn't list this row (it's
        // the Tab menu's item) — a right-click in the tree that is already
        // showing this file shouldn't offer to reveal it again.
        let model = AppModel::new(800, 600, 1.0);
        let path = PathBuf::from("/tmp/some/file.rs");
        let items = build_file_tree_menu(&model, &path, false);
        assert!(!items.iter().any(|i| i.label == "Reveal in File Explorer"));
    }

    #[test]
    fn file_tree_menu_copy_relative_path_needs_a_real_workspace() {
        let path = PathBuf::from("/tmp/some/file.rs");

        let no_workspace = AppModel::new(800, 600, 1.0);
        let items = build_file_tree_menu(&no_workspace, &path, false);
        assert!(
            !items
                .iter()
                .find(|i| i.label == "Copy Relative Path")
                .unwrap()
                .enabled,
            "no workspace open -> Copy Relative Path disabled, not hardcoded enabled"
        );

        let ws_dir = tempfile::tempdir().unwrap();
        let mut with_workspace = AppModel::new(800, 600, 1.0);
        with_workspace.workspace = crate::model::workspace::Workspace::new(
            ws_dir.path().to_path_buf(),
            &with_workspace.metrics,
        )
        .ok();
        let items = build_file_tree_menu(&with_workspace, &path, false);
        assert!(
            items
                .iter()
                .find(|i| i.label == "Copy Relative Path")
                .unwrap()
                .enabled
        );
    }
}
