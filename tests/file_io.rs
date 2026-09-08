//! File replies must never act on whichever tab happens to be focused later.

mod common;

use std::path::PathBuf;
use token::commands::Cmd;
use token::messages::{AppMsg, DocumentMsg, LayoutMsg, Msg};
use token::model::{AppModel, FileRequest, SplitDirection};
use token::update::update;

fn model() -> AppModel {
    let mut model = common::test_model("abc", 0, 3);
    model.document_mut().file_path = Some("/fixture/original.txt".into());
    model.config.format_on_save = false;
    model
}

fn write_reply(cmd: Cmd, result: Result<(), String>) -> Msg {
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = cmd
    else {
        panic!("expected a write, got {cmd:?}")
    };
    Msg::App(AppMsg::SaveCompleted {
        identity: None,
        target,
        path,
        content,
        result,
    })
}

fn save(model: &mut AppModel) -> Msg {
    let cmd = update(model, Msg::App(AppMsg::SaveFile)).unwrap();
    write_reply(cmd, Ok(()))
}

fn load(model: &mut AppModel, path: &str, content: &str) -> Msg {
    let Cmd::LoadFile { target, path } =
        update(model, Msg::App(AppMsg::LoadFile(path.into()))).unwrap()
    else {
        panic!("expected read")
    };
    Msg::App(AppMsg::FileLoaded {
        identity: None,
        target,
        path,
        result: Ok(content.to_owned()),
    })
}

fn edit(model: &mut AppModel, ch: char) {
    update(model, Msg::Document(DocumentMsg::InsertChar(ch)));
}

fn save_dialog(model: &mut AppModel) -> FileRequest {
    let Cmd::ShowSaveFileDialog { target, .. } =
        update(model, Msg::App(AppMsg::SaveFileAs)).unwrap()
    else {
        panic!("expected dialog")
    };
    target
}

#[test]
fn file_identity_save_as_replaces_aliases_only_after_success() {
    use std::path::Path;
    use token::util::FileIdentity;
    for success in [false, true] {
        let mut m = model();
        let old = m.document().file_path.clone().unwrap();
        let original =
            FileIdentity::from_resolved(old.clone(), Path::new("/canonical/original.txt"));
        m.document_mut().set_file_identity(Some(original.clone()));
        let target = save_dialog(&mut m);
        let new = PathBuf::from("/fixture/renamed.txt");
        let cmd = update(
            &mut m,
            Msg::App(AppMsg::SaveFileAsDialogResult {
                target,
                path: Some(new.clone()),
            }),
        )
        .unwrap();
        let mut reply = write_reply(
            cmd,
            if success {
                Ok(())
            } else {
                Err("denied".into())
            },
        );
        let replacement = FileIdentity::from_resolved(new, Path::new("/canonical/renamed.txt"));
        if let Msg::App(AppMsg::SaveCompleted { identity, .. }) = &mut reply {
            *identity = Some(replacement.clone());
        }
        update(&mut m, reply.clone());
        let expected = if success { &replacement } else { &original };
        assert_eq!(m.document().file_identity(), Some(expected));
        assert!(m.document().matches_file_path(expected.path()));
        if success {
            assert!(!m.document().matches_file_path(original.path()));
        }
        update(&mut m, reply);
        assert_eq!(m.document().file_identity(), Some(expected));
    }
}

#[test]
fn file_identity_stale_reload_cannot_replace_current_identity() {
    use std::path::Path;
    use token::util::FileIdentity;
    let mut m = model();
    let source = m.document().file_path.clone().unwrap();
    let original = FileIdentity::from_resolved(source, Path::new("/canonical/original.txt"));
    m.document_mut().set_file_identity(Some(original.clone()));
    let mut reply = load(&mut m, "/fixture/other.txt", "stale text");
    if let Msg::App(AppMsg::FileLoaded { identity, .. }) = &mut reply {
        *identity = Some(FileIdentity::from_resolved(
            "/fixture/other.txt".into(),
            Path::new("/canonical/other.txt"),
        ));
    }
    edit(&mut m, '!');
    update(&mut m, reply);
    assert_eq!(m.document().file_identity(), Some(&original));
    assert!(!m
        .document()
        .matches_file_path(Path::new("/canonical/other.txt")));
}

#[test]
fn file_io_save_targets_origin_and_keeps_edits_made_during_write_dirty() {
    let mut m = model();
    let original = m.document().id.unwrap();
    edit(&mut m, '1');
    let reply = save(&mut m);
    edit(&mut m, '2');
    update(&mut m, Msg::Layout(LayoutMsg::NewTab));
    edit(&mut m, 'B');
    let focused = m.document().id;
    update(&mut m, reply.clone());
    assert_eq!(m.document().id, focused);
    assert_eq!(m.document().buffer.to_string(), "B");
    assert!(m.document().is_modified);
    assert_eq!(
        m.editor_area.documents[&original].buffer.to_string(),
        "abc12"
    );
    assert!(m.editor_area.documents[&original].is_modified);
    assert!(!m.ui.is_saving);
    update(&mut m, reply); // Duplicate replies are inert.
    assert!(m.document().is_modified);
    update(&mut m, Msg::Layout(LayoutMsg::PrevTab));
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    assert_eq!(m.document().buffer.to_string(), "abc1");
    assert!(
        !m.document().is_modified,
        "undo returns to the snapshot actually saved"
    );
}

#[test]
fn file_io_failed_newer_write_retains_older_success_and_busy_state() {
    let mut m = model();
    edit(&mut m, '1');
    let first = save(&mut m);
    edit(&mut m, '2');
    let second = write_reply(
        update(&mut m, Msg::App(AppMsg::SaveFile)).unwrap(),
        Err("disk full".to_owned()),
    );
    update(&mut m, first);
    assert!(m.ui.is_saving, "another write is still pending");
    assert!(m.document().is_modified);
    update(&mut m, second);
    assert!(!m.ui.is_saving);
    assert!(m.document().is_modified);
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    assert!(!m.document().is_modified);
}

#[test]
fn file_io_saved_state_is_content_not_reused_history_depth() {
    let mut m = model();
    edit(&mut m, 'x');
    let reply = save(&mut m);
    update(&mut m, reply);
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    edit(&mut m, 'y');
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    update(&mut m, Msg::Document(DocumentMsg::Redo));
    assert_eq!(m.document().buffer.to_string(), "abcy");
    assert!(m.document().is_modified);
}

#[test]
fn file_io_save_as_dialog_and_write_keep_original_document_across_focus_changes() {
    let mut m = model();
    let original = m.document().id.unwrap();
    let target = save_dialog(&mut m);
    edit(&mut m, '1'); // Save current origin content when the dialog confirms.
    update(&mut m, Msg::Layout(LayoutMsg::NewTab));
    edit(&mut m, 'B');
    let cmd = update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult {
            target,
            path: Some("/fixture/renamed.txt".into()),
        }),
    )
    .unwrap();
    let reply = write_reply(cmd, Ok(()));
    assert_eq!(
        m.editor_area.documents[&original].file_path,
        Some("/fixture/original.txt".into())
    );
    update(&mut m, reply);
    let doc = &m.editor_area.documents[&original];
    assert_eq!(doc.file_path, Some("/fixture/renamed.txt".into()));
    assert_eq!(doc.buffer.to_string(), "abc1");
    assert!(!doc.is_modified);
    assert_eq!(m.document().buffer.to_string(), "B");
    assert!(m.document().is_modified);
}

#[test]
fn file_io_load_targets_all_origin_panes_without_stealing_focus() {
    let mut m = model();
    let original = m.document().id.unwrap();
    update(
        &mut m,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    let reply = load(&mut m, "/fixture/replacement.txt", "z");
    update(&mut m, Msg::Layout(LayoutMsg::NewTab));
    edit(&mut m, 'B');
    let focused = m.document().id;
    update(&mut m, reply);
    assert_eq!(m.document().id, focused);
    assert_eq!(m.document().buffer.to_string(), "B");
    assert_eq!(m.editor_area.documents[&original].buffer.to_string(), "z");
    for editor in m
        .editor_area
        .editors
        .values()
        .filter(|e| e.document_id == Some(original))
    {
        assert!(editor.is_plain_text_mode());
        assert!(editor.cursors[0].column <= 1);
        assert_eq!(editor.cursors[0].line, 0);
        assert!(editor
            .selections
            .iter()
            .all(|selection| selection.is_empty()));
    }
}

#[test]
fn file_io_load_rejects_intervening_edits_and_superseded_or_duplicate_replies() {
    let mut m = model();
    let stale = load(&mut m, "/fixture/a.txt", "stale");
    edit(&mut m, 'x');
    update(&mut m, stale);
    assert_eq!(m.document().buffer.to_string(), "abcx");
    assert!(m.document().is_modified);
    assert!(!m.ui.is_loading);
    let older = load(&mut m, "/fixture/older.txt", "older");
    let newer = load(&mut m, "/fixture/newer.txt", "newer");
    update(&mut m, newer.clone());
    update(&mut m, older);
    edit(&mut m, 'x');
    let text = m.document().buffer.clone();
    update(&mut m, newer);
    assert_eq!(m.document().buffer, text);
    assert_eq!(m.document().file_path, Some("/fixture/newer.txt".into()));
}

#[test]
fn file_io_reload_invalidates_old_saves_and_dialogs() {
    let mut m = model();
    let write = save(&mut m);
    let dialog = save_dialog(&mut m);
    let read = load(&mut m, "/fixture/loaded.txt", "loaded");
    update(&mut m, read);
    update(&mut m, write);
    let cmd = update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult {
            target: dialog,
            path: Some(PathBuf::from("/fixture/obsolete.txt")),
        }),
    );
    assert!(cmd.is_none());
    assert_eq!(m.document().file_path, Some("/fixture/loaded.txt".into()));
    assert_eq!(m.document().buffer.to_string(), "loaded");
    assert!(!m.document().is_modified);
}

#[test]
fn file_io_write_after_pending_read_supersedes_the_read() {
    let mut m = model();
    let read = load(&mut m, "/fixture/original.txt", "different disk bytes");
    let write = save(&mut m);
    // The worker performs these in request order: read, then write. Applying
    // the old read here would leave a clean buffer disagreeing with that write.
    update(&mut m, read);
    assert_eq!(m.document().buffer.to_string(), "abc");
    update(&mut m, write);
    assert!(!m.document().is_modified);
    assert!(!m.ui.is_loading && !m.ui.is_saving);
    assert_eq!(m.document().buffer.to_string(), "abc");
}

#[test]
fn file_io_save_as_preserves_edits_made_after_dialog_confirmation() {
    let mut m = model();
    let target = save_dialog(&mut m);
    let cmd = update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult {
            target,
            path: Some("/fixture/renamed.txt".into()),
        }),
    )
    .unwrap();
    let reply = write_reply(cmd, Ok(()));
    edit(&mut m, 'x');
    update(&mut m, reply);
    assert_eq!(m.document().file_path, Some("/fixture/renamed.txt".into()));
    assert_eq!(m.document().buffer.to_string(), "abcx");
    assert!(m.document().is_modified);
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    assert!(!m.document().is_modified);
}

#[test]
fn file_io_older_save_reply_cannot_replace_a_newer_saved_snapshot() {
    let mut m = model();
    let older = save(&mut m);
    edit(&mut m, 'x');
    let newer = save(&mut m);
    update(&mut m, newer);
    update(&mut m, older);
    assert!(!m.document().is_modified);
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    assert!(m.document().is_modified);
}

#[test]
fn file_io_save_never_writes_placeholder_buffer_over_image_or_binary_files() {
    use token::model::editor::{BinaryPlaceholderState, TabContent, ViewMode};
    for image in [false, true] {
        let mut m = model();
        if image {
            m.editor_mut().view_mode = ViewMode::Image(Box::new(token::image::ImageState::new(
                vec![0; 4],
                1,
                1,
                4,
                "PNG".to_owned(),
                100,
                100,
            )));
        } else {
            m.editor_mut().tab_content = TabContent::BinaryPlaceholder(BinaryPlaceholderState {
                path: "/fixture/binary.bin".into(),
                size_bytes: 1234,
            });
        }
        for message in [AppMsg::SaveFile, AppMsg::SaveFileAs] {
            let cmd = update(&mut m, Msg::App(message)).unwrap();
            assert!(
                matches!(cmd, Cmd::RedrawAreas(_)),
                "no write or dialog: {cmd:?}"
            );
            assert!(!m.ui.is_saving);
            assert!(m
                .ui
                .transient_message
                .as_ref()
                .unwrap()
                .text
                .contains("not supported"));
        }
    }
    let mut m = model();
    m.document_mut().file_path = Some("/fixture/table.csv".into());
    m.document_mut().buffer = "name,value\na,1\n".into();
    update(&mut m, Msg::Csv(token::messages::CsvMsg::Toggle));
    assert!(m.editor().view_mode.as_csv().is_some());
    let cmd = update(&mut m, Msg::App(AppMsg::SaveFile)).unwrap();
    assert!(
        matches!(cmd, Cmd::SaveFile { .. }),
        "text-backed CSV remains savable"
    );
}

#[test]
fn file_io_closed_document_replies_do_not_affect_replacement_tab() {
    let mut m = model();
    let write = save(&mut m);
    let read = load(&mut m, "/fixture/closed.txt", "closed");
    update(&mut m, Msg::Layout(LayoutMsg::NewTab));
    let old_tab = m.editor_area.focused_group().unwrap().tabs[0].id;
    update(&mut m, Msg::Layout(LayoutMsg::CloseTab(old_tab)));
    edit(&mut m, 'B');
    update(&mut m, write);
    update(&mut m, read);
    assert_eq!(m.document().buffer.to_string(), "B");
    assert!(m.document().is_modified);
    assert!(!m.ui.is_saving && !m.ui.is_loading);
}
