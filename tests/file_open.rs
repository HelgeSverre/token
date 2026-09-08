//! Deterministic open-intent tests: disk replies are supplied explicitly.
mod common;

use std::path::{Path, PathBuf};
use token::commands::ConfigResource;
use token::messages::{LayoutMsg, Msg};
use token::model::{Document, FileOpenRequest, PreparedFile, SplitDirection, TabContent, ViewMode};
use token::update::update;
use token::{AppModel, Cmd};

fn request(model: &mut AppModel, path: &str) -> FileOpenRequest {
    let cmd = update(model, Msg::Layout(LayoutMsg::OpenFileInNewTab(path.into()))).unwrap();
    let Cmd::PrepareFileOpen(request) = cmd else {
        panic!("expected deferred open: {cmd:?}")
    };
    request
}

fn loaded(request: &FileOpenRequest, text: &str, canonical: Option<&str>) -> PreparedFile {
    loaded_path(request.source.path().unwrap(), text, canonical)
}

fn loaded_path(path: &Path, text: &str, canonical: Option<&str>) -> PreparedFile {
    let mut document = Document::with_text(text);
    document.file_path = Some(path.to_path_buf());
    document.set_file_identity(canonical.map(|canonical| {
        token::util::FileIdentity::from_resolved(path.to_path_buf(), Path::new(canonical))
    }));
    PreparedFile::Loaded {
        document: Box::new(document),
        view_mode: ViewMode::Text,
        tab_content: TabContent::Text,
    }
}

fn config_request(model: &mut AppModel, resource: ConfigResource) -> FileOpenRequest {
    let Cmd::PrepareFileOpen(request) = update(
        model,
        Msg::App(token::messages::AppMsg::OpenConfigResource(resource)),
    )
    .unwrap() else {
        panic!("configuration preparation request")
    };
    request
}

#[test]
fn config_resource_open_keeps_the_group_captured_before_preparation() {
    let mut model = common::test_model("origin", 0, 2);
    let group = model.editor_area.focused_group_id;
    let request = config_request(&mut model, ConfigResource::Keybindings);
    assert!(model.ui.is_loading);
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let focused = model.editor_area.focused_group_id;
    let path = Path::new("/fixture/keymap.yaml");
    finish(&mut model, request, loaded_path(path, "bindings: []", None));
    assert_eq!(model.editor_area.focused_group_id, focused);
    assert_eq!(model.document().buffer.to_string(), "origin");
    let editor_id = model.editor_area.groups[&group].active_editor_id().unwrap();
    let id = model.editor_area.editors[&editor_id].document_id.unwrap();
    assert_eq!(
        model.editor_area.documents[&id].file_path.as_deref(),
        Some(path)
    );
    assert!(!model.ui.is_loading);
}

#[test]
fn config_resource_preparation_does_not_reset_the_original_stale_input_guard() {
    for resource in [ConfigResource::Keybindings, ConfigResource::Log] {
        let mut model = common::test_model("origin", 0, 2);
        let request = config_request(&mut model, resource);
        update(&mut model, Msg::Layout(LayoutMsg::NewTab));
        let selected = model.document().id;
        finish(
            &mut model,
            request,
            loaded_path(Path::new("/fixture/config.txt"), "ready", None),
        );
        assert_eq!(model.document().id, selected);
        assert_eq!(model.editor_area.documents.len(), 3);
        assert!(!model.ui.is_loading);
    }
}

#[test]
fn config_resource_directory_does_not_supersede_a_pending_tab_open() {
    let mut model = common::test_model("origin", 0, 2);
    let tab = request(&mut model, "/fixture/next.txt");
    let directory = config_request(&mut model, ConfigResource::Directory);
    let result = PreparedFile::Directory {
        path: "/fixture/config".into(),
    };
    let cmd = finish(&mut model, directory.clone(), result.clone()).unwrap();
    assert!(find_command(&cmd, &|cmd| matches!(cmd, Cmd::OpenInExplorer { .. })).is_some());
    assert!(model.ui.is_loading, "the ordinary open is still pending");
    assert!(
        finish(&mut model, directory, result).is_none(),
        "duplicate must not reveal twice"
    );
    let prepared = loaded(&tab, "next", None);
    finish(&mut model, tab, prepared);
    assert_eq!(model.document().buffer.to_string(), "next");
    assert!(!model.ui.is_loading);
}

#[test]
fn config_resource_closed_group_rejects_directory_reveal() {
    let mut model = common::test_model("origin", 0, 0);
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let group = model.editor_area.focused_group_id;
    let request = config_request(&mut model, ConfigResource::Directory);
    update(&mut model, Msg::Layout(LayoutMsg::CloseGroup(group)));
    let cmd = finish(
        &mut model,
        request,
        PreparedFile::Directory {
            path: "/fixture/config".into(),
        },
    )
    .unwrap();
    assert!(find_command(&cmd, &|cmd| matches!(cmd, Cmd::OpenInExplorer { .. })).is_none());
    assert!(find_command(&cmd, &|cmd| matches!(
        cmd,
        Cmd::FileOpenFinished {
            document_id: None,
            ..
        }
    ))
    .is_some());
    assert!(!model.ui.is_loading);
    assert_eq!(model.document().buffer.to_string(), "origin");
}

fn finish(model: &mut AppModel, request: FileOpenRequest, prepared: PreparedFile) -> Option<Cmd> {
    update(
        model,
        Msg::Layout(LayoutMsg::FilePrepared {
            request,
            result: Ok(Box::new(prepared)),
        }),
    )
}

#[test]
fn file_open_request_does_not_read_or_replace_the_current_buffer() {
    let mut model = common::test_model("unsaved", 0, 3);
    let original = model.document().id;
    let request = request(&mut model, "/not-a-real-directory/new.rs");
    assert_eq!(model.document().id, original);
    assert_eq!(model.document().buffer.to_string(), "unsaved");
    assert!(model.ui.is_loading);
    let prepared = loaded(&request, "ready", None);
    finish(&mut model, request, prepared);
    assert_eq!(model.document().buffer.to_string(), "ready");
    assert!(!model.ui.is_loading);
}

#[test]
fn file_open_finishes_in_its_original_split_without_stealing_focus() {
    let mut model = common::test_model("origin", 0, 2);
    let origin_group = model.editor_area.focused_group_id;
    let request = request(&mut model, "/pending.rs");
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let focused_group = model.editor_area.focused_group_id;
    let focused_editor = model.editor_area.focused_editor_id();
    let prepared = loaded(&request, "target", None);
    finish(&mut model, request, prepared);
    assert_eq!(model.editor_area.focused_group_id, focused_group);
    assert_eq!(model.editor_area.focused_editor_id(), focused_editor);
    assert_eq!(model.document().buffer.to_string(), "origin");
    let group = &model.editor_area.groups[&origin_group];
    assert_eq!(
        model
            .editor_area
            .document_for_group(group)
            .unwrap()
            .buffer
            .to_string(),
        "target"
    );
}

#[test]
fn file_open_keeps_newer_tab_choice_and_consumes_duplicate_reply_once() {
    let mut model = common::test_model("origin", 0, 0);
    let request = request(&mut model, "/pending.rs");
    update(&mut model, Msg::Layout(LayoutMsg::NewTab));
    let selected = model.editor_area.focused_editor_id();
    let prepared = loaded(&request, "background", None);
    finish(&mut model, request.clone(), prepared.clone());
    let count = model.editor_area.documents.len();
    assert!(finish(&mut model, request, prepared).is_none());
    assert_eq!(model.editor_area.documents.len(), count);
    assert_eq!(model.editor_area.focused_editor_id(), selected);
    assert!(!model.ui.is_loading);
}

#[test]
fn file_open_does_not_interrupt_typing_or_cursor_movement_during_loading() {
    for edit in [false, true] {
        let mut model = common::test_model("origin", 0, 0);
        let selected = model.editor_area.focused_editor_id();
        let request = request(&mut model, "/pending.rs");
        let msg = if edit {
            Msg::Document(token::messages::DocumentMsg::InsertChar('X'))
        } else {
            Msg::Editor(token::messages::EditorMsg::MoveCursor(
                token::messages::Direction::Right,
            ))
        };
        update(&mut model, msg);
        let cursor = *model.editor().active_cursor();
        let prepared = loaded(&request, "target", None);
        finish(&mut model, request, prepared);
        assert_eq!(model.editor_area.focused_editor_id(), selected);
        assert_eq!(*model.editor().active_cursor(), cursor);
        assert_eq!(
            model.document().buffer.to_string(),
            if edit { "Xorigin" } else { "origin" }
        );
    }
}

#[test]
fn file_open_reverse_replies_activate_only_the_latest_request() {
    let mut model = common::test_model("origin", 0, 0);
    let first = request(&mut model, "/first.rs");
    let second = request(&mut model, "/second.rs");
    let prepared = loaded(&second, "second", None);
    finish(&mut model, second, prepared);
    assert!(model.ui.is_loading);
    let selected = model.editor_area.focused_editor_id();
    let prepared = loaded(&first, "first", None);
    finish(&mut model, first, prepared);
    assert_eq!(model.editor_area.documents.len(), 3);
    assert_eq!(model.editor_area.focused_editor_id(), selected);
    assert_eq!(model.document().buffer.to_string(), "second");
    assert!(!model.ui.is_loading);
}

#[test]
fn file_open_closed_group_rejects_reply_without_leaking_a_document() {
    let mut model = common::test_model("origin", 0, 0);
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let closing = model.editor_area.focused_group_id;
    let request = request(&mut model, "/pending.rs");
    update(&mut model, Msg::Layout(LayoutMsg::CloseGroup(closing)));
    let count = model.editor_area.documents.len();
    let prepared = loaded(&request, "unused", None);
    let cmd = finish(&mut model, request, prepared);
    assert!(find_command(cmd.as_ref().unwrap(), &|cmd| matches!(
        cmd,
        Cmd::FileOpenFinished {
            document_id: None,
            ..
        }
    ))
    .is_some());
    assert!(cmd.unwrap().needs_redraw());
    assert_eq!(model.editor_area.documents.len(), count);
    assert!(!model.ui.is_loading);
}

#[test]
fn file_open_overlapping_aliases_reuse_the_live_buffer_not_the_disk_reply() {
    let mut model = common::test_model("origin", 0, 0);
    let first = request(&mut model, "/real.rs");
    let second = request(&mut model, "/alias.rs");
    let prepared = loaded(&first, "disk", Some("/real.rs"));
    finish(&mut model, first, prepared);
    let doc_id = *model
        .editor_area
        .documents
        .iter()
        .find(|(_, doc)| doc.file_path.as_deref() == Some(std::path::Path::new("/real.rs")))
        .unwrap()
        .0;
    model.editor_area.documents.get_mut(&doc_id).unwrap().buffer = "unsaved edits".into();
    let prepared = loaded(&second, "obsolete disk bytes", Some("/real.rs"));
    finish(&mut model, second, prepared);
    assert_eq!(model.editor_area.documents.len(), 2);
    assert_eq!(model.document().id, Some(doc_id));
    assert_eq!(model.document().buffer.to_string(), "unsaved edits");
}

#[test]
fn file_open_retries_a_closed_or_renamed_worker_document_snapshot() {
    let mut model = common::test_model("origin", 0, 0);
    model.document_mut().file_path = Some("/real.rs".into());
    let original = model.document().id.unwrap();
    let request = request(&mut model, "/alias.rs");
    let id = request.id();
    model.document_mut().file_path = Some("/renamed.rs".into());
    let cmd = finish(
        &mut model,
        request,
        PreparedFile::Existing {
            document_id: original,
            path: "/real.rs".into(),
        },
    )
    .unwrap();
    let Cmd::PrepareFileOpen(retry) = cmd else {
        panic!("retry")
    };
    assert_eq!(retry.id(), id);
    assert_eq!(
        retry
            .known_documents
            .iter()
            .map(|known| (known.document_id, known.path.clone()))
            .collect::<Vec<_>>(),
        vec![(original, PathBuf::from("/renamed.rs"))]
    );
    let prepared = loaded(&retry, "actual target", None);
    finish(&mut model, retry, prepared);
    assert_eq!(model.document().buffer.to_string(), "actual target");
}

#[test]
fn file_identity_open_retries_when_a_same_path_snapshot_has_changed_identity() {
    let mut model = common::test_model("origin", 0, 0);
    model.document_mut().file_path = Some("/link.rs".into());
    model
        .document_mut()
        .set_file_identity(Some(token::util::FileIdentity::from_resolved(
            "/link.rs".into(),
            Path::new("/old-target.rs"),
        )));
    let doc_id = model.document().id.unwrap();
    let request = request(&mut model, "/unknown-alias.rs");
    let id = request.id();
    model
        .document_mut()
        .set_file_identity(Some(token::util::FileIdentity::from_resolved(
            "/link.rs".into(),
            Path::new("/new-target.rs"),
        )));
    let cmd = finish(
        &mut model,
        request,
        PreparedFile::Existing {
            document_id: doc_id,
            path: "/link.rs".into(),
        },
    )
    .unwrap();
    let Cmd::PrepareFileOpen(retry) = cmd else {
        panic!("stale identity must retry")
    };
    assert_eq!(retry.id(), id);
    assert_eq!(
        retry.known_documents[0].identity.as_ref().unwrap().path(),
        Path::new("/new-target.rs")
    );
    assert_eq!(model.document().buffer.to_string(), "origin");
}

#[test]
fn file_open_reusing_special_tabs_preserves_mode_in_another_split() {
    for image in [false, true] {
        let mut model = common::test_model("origin", 0, 0);
        let request = request(&mut model, "/special.bin");
        let mut prepared = loaded(&request, "", None);
        if let PreparedFile::Loaded {
            view_mode,
            tab_content,
            ..
        } = &mut prepared
        {
            if image {
                *view_mode = ViewMode::Image(Box::new(token::image::ImageState::new(
                    vec![255; 4],
                    1,
                    1,
                    4,
                    "PNG".into(),
                    800,
                    600,
                )));
            } else {
                *tab_content =
                    TabContent::BinaryPlaceholder(token::model::editor::BinaryPlaceholderState {
                        path: request.source.path().unwrap().to_path_buf(),
                        size_bytes: 50,
                    });
            }
        }
        finish(&mut model, request, prepared);
        let doc_id = model.document().id;
        let pixels = match &model.editor().view_mode {
            ViewMode::Image(image) => Some(image.pixels.clone()),
            _ => None,
        };
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        update(&mut model, Msg::Layout(LayoutMsg::NewTab));
        update(
            &mut model,
            Msg::Layout(LayoutMsg::OpenFileInNewTab("/special.bin".into())),
        );
        assert_eq!(model.document().id, doc_id);
        assert!(!model.editor().is_plain_text_mode());
        assert_eq!(model.editor().view_mode.is_image(), image);
        if let (Some(pixels), ViewMode::Image(shared)) = (pixels, &model.editor().view_mode) {
            assert!(std::sync::Arc::ptr_eq(&pixels, &shared.pixels));
        }
        assert_eq!(
            matches!(model.editor().tab_content, TabContent::BinaryPlaceholder(_)),
            !image
        );
    }
}

#[test]
fn file_open_deferred_cli_position_is_clamped_against_the_destination() {
    let mut model = common::test_model("origin", 0, 0);
    let cmd =
        token::update::navigation::open_path_at(&mut model, "/target.rs".into(), Some(2), Some(99))
            .unwrap();
    let Cmd::PrepareFileOpen(request) = cmd else {
        panic!("pending")
    };
    assert_eq!(model.editor().active_cursor().line, 0);
    let prepared = loaded(&request, "first\na🎉b\n", None);
    finish(&mut model, request, prepared);
    assert_eq!(
        (
            model.editor().active_cursor().line,
            model.editor().active_cursor().column
        ),
        (1, 3)
    );
}

fn find_command<'a>(cmd: &'a Cmd, predicate: &impl Fn(&Cmd) -> bool) -> Option<&'a Cmd> {
    if predicate(cmd) {
        return Some(cmd);
    }
    if let Cmd::Batch(commands) = cmd {
        return commands.iter().find_map(|cmd| find_command(cmd, predicate));
    }
    None
}

fn edit_fixture() -> (tempfile::TempDir, AppModel, lsp_types::WorkspaceEdit) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.rs");
    let other = dir.path().join("other.rs");
    std::fs::write(&source, "old\n").unwrap();
    std::fs::write(&other, "old\n").unwrap();
    let mut model = common::test_model("old\n", 0, 0);
    model.document_mut().file_path = Some(source.clone());
    model
        .document_mut()
        .set_file_identity(Some(token::util::FileIdentity::resolve(source.clone())));
    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(0, 3),
        ),
        new_text: "new".into(),
    };
    #[allow(clippy::mutable_key_type)]
    let changes = std::collections::HashMap::from([
        (token::lsp::path_to_uri(&source), vec![edit.clone()]),
        (token::lsp::path_to_uri(&other), vec![edit]),
    ]);
    (dir, model, lsp_types::WorkspaceEdit::new(changes))
}

fn start_server_edit(
    model: &mut AppModel,
    edit: lsp_types::WorkspaceEdit,
) -> (Cmd, FileOpenRequest) {
    let cmd = update(
        model,
        Msg::Lsp(token::messages::LspMsg::ApplyEditRequested {
            server_id: "test-server".into(),
            root: "/workspace".into(),
            request_id: serde_json::json!(17),
            edit: Box::new(edit),
            label: None,
        }),
    )
    .unwrap();
    assert!(find_command(&cmd, &|cmd| matches!(cmd, Cmd::LspRespondToServer { .. })).is_none());
    let Cmd::PrepareFileOpen(request) =
        find_command(&cmd, &|cmd| matches!(cmd, Cmd::PrepareFileOpen(_))).unwrap()
    else {
        unreachable!()
    };
    (cmd.clone(), request.clone())
}

#[test]
fn file_open_workspace_edit_waits_before_mutating_or_acknowledging() {
    let (_dir, mut model, edit) = edit_fixture();
    let original_editor = model.editor_area.focused_editor_id();
    let (_, request) = start_server_edit(&mut model, edit);
    assert_eq!(request.policy, token::model::FileOpenPolicy::ExistingText);
    assert_eq!(model.document().buffer.to_string(), "old\n");
    let prepared = loaded(&request, "old\n", None);
    let cmd = finish(&mut model, request, prepared).unwrap();
    assert_eq!(model.editor_area.focused_editor_id(), original_editor);
    for document in model.editor_area.documents.values() {
        assert_eq!(document.buffer.to_string(), "new\n");
        assert_eq!(document.undo_stack.len(), 1);
        assert!(document.is_modified);
    }
    let response =
        find_command(&cmd, &|cmd| matches!(cmd, Cmd::LspRespondToServer { .. })).unwrap();
    assert!(
        matches!(response, Cmd::LspRespondToServer { result, .. } if result["applied"] == true)
    );
}

#[test]
fn file_open_workspace_edit_rejects_changes_during_preparation() {
    let (_dir, mut model, edit) = edit_fixture();
    let (_, request) = start_server_edit(&mut model, edit);
    update(
        &mut model,
        Msg::Document(token::messages::DocumentMsg::InsertChar('X')),
    );
    let prepared = loaded(&request, "old\n", None);
    let cmd = finish(&mut model, request, prepared).unwrap();
    assert_eq!(model.document().buffer.to_string(), "Xold\n");
    let response =
        find_command(&cmd, &|cmd| matches!(cmd, Cmd::LspRespondToServer { .. })).unwrap();
    assert!(
        matches!(response, Cmd::LspRespondToServer { result, .. } if result["applied"] == false)
    );
    assert!(model
        .editor_area
        .documents
        .values()
        .any(|doc| doc.buffer == "old\n" && !doc.is_modified));
}

#[test]
fn file_open_workspace_edit_failure_does_not_apply_other_files() {
    let (_dir, mut model, edit) = edit_fixture();
    let (_, request) = start_server_edit(&mut model, edit);
    let cmd = update(
        &mut model,
        Msg::Layout(LayoutMsg::FilePrepared {
            request,
            result: Err("permission denied".into()),
        }),
    )
    .unwrap();
    assert_eq!(model.document().buffer.to_string(), "old\n");
    assert!(model.document().undo_stack.is_empty());
    assert!(!model.ui.is_loading);
    let response =
        find_command(&cmd, &|cmd| matches!(cmd, Cmd::LspRespondToServer { .. })).unwrap();
    assert!(
        matches!(response, Cmd::LspRespondToServer { result, .. } if result["applied"] == false)
    );
}

#[test]
fn file_open_workspace_edit_rejects_a_target_opened_and_edited_during_loading() {
    let (_dir, mut model, edit) = edit_fixture();
    let (_, edit_request) = start_server_edit(&mut model, edit);
    let normal = request(
        &mut model,
        edit_request.source.path().unwrap().to_str().unwrap(),
    );
    let prepared = loaded(&normal, "old\n", None);
    finish(&mut model, normal, prepared);
    update(
        &mut model,
        Msg::Document(token::messages::DocumentMsg::InsertChar('X')),
    );
    let prepared = loaded(&edit_request, "old\n", None);
    let cmd = finish(&mut model, edit_request, prepared).unwrap();
    assert_eq!(model.document().buffer.to_string(), "Xold\n");
    assert!(find_command(
        &cmd,
        &|cmd| matches!(cmd, Cmd::LspRespondToServer { result, .. } if result["applied"] == false)
    )
    .is_some());
    assert!(model
        .editor_area
        .documents
        .values()
        .any(|doc| doc.buffer == "old\n" && doc.undo_stack.is_empty()));
}

#[test]
fn file_open_code_action_command_runs_only_after_its_edits_finish() {
    let (_dir, mut model, edit) = edit_fixture();
    model.ui.code_action_list = Some(vec![token::model::ui::CodeActionItem {
        title: "Fix both files".into(),
        kind: None,
        is_preferred: false,
        edit: Some(Box::new(edit)),
        command: Some(lsp_types::Command {
            title: "after".into(),
            command: "after-edit".into(),
            arguments: None,
        }),
    }]);
    let cmd = update(
        &mut model,
        Msg::Lsp(token::messages::LspMsg::ActivateCodeAction { index: 0 }),
    )
    .unwrap();
    assert!(find_command(&cmd, &|cmd| matches!(cmd, Cmd::LspExecuteCommand { .. })).is_none());
    let Cmd::PrepareFileOpen(request) =
        find_command(&cmd, &|cmd| matches!(cmd, Cmd::PrepareFileOpen(_))).unwrap()
    else {
        unreachable!()
    };
    let prepared = loaded(request, "old\n", None);
    let cmd = finish(&mut model, request.clone(), prepared).unwrap();
    assert!(find_command(
        &cmd,
        &|cmd| matches!(cmd, Cmd::LspExecuteCommand { command, .. } if command == "after-edit")
    )
    .is_some());
    assert_eq!(model.document().buffer.to_string(), "new\n");
}

#[test]
fn file_open_native_dialog_reply_targets_the_original_group() {
    use token::messages::AppMsg;
    let mut model = common::test_model("origin", 0, 0);
    let Cmd::ShowOpenFileDialog { group_id, .. } =
        update(&mut model, Msg::App(AppMsg::OpenFileDialog)).unwrap()
    else {
        panic!("dialog")
    };
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let focused_group = model.editor_area.focused_group_id;
    let cmd = update(
        &mut model,
        Msg::App(AppMsg::OpenFileDialogResult {
            group_id,
            paths: vec!["/dialog.rs".into()],
        }),
    )
    .unwrap();
    let Cmd::PrepareFileOpen(request) =
        find_command(&cmd, &|cmd| matches!(cmd, Cmd::PrepareFileOpen(_))).unwrap()
    else {
        unreachable!()
    };
    let prepared = loaded(request, "dialog target", None);
    finish(&mut model, request.clone(), prepared);
    assert_eq!(model.editor_area.focused_group_id, focused_group);
    assert_eq!(model.document().buffer.to_string(), "origin");
    assert_eq!(
        model
            .editor_area
            .document_for_group(&model.editor_area.groups[&group_id])
            .unwrap()
            .buffer
            .to_string(),
        "dialog target"
    );
}
