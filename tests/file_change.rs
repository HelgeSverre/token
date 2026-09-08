//! External observations must never discard unsaved edits or target another tab.
use token::commands::Cmd;
use token::messages::{AppMsg, CsvMsg, DocumentMsg, LayoutMsg, ModalMsg, Msg, UiMsg};
use token::model::{
    AppModel, DiskContent, Document, FileRequest, ModalState, ObservedFile, SplitDirection,
};
use token::update::update;

fn model(text: &str) -> AppModel {
    let identity = token::util::FileIdentity::from_resolved(
        "/fixture/document.csv".into(),
        std::path::Path::new("/fixture/document.csv"),
    );
    let mut model =
        AppModel::with_document(1000, 700, 1.0, Document::from_loaded_text(text, identity));
    model.config.lsp.enabled = false;
    model.config.format_on_save = false;
    model
}

fn find_effect(command: Cmd, predicate: fn(&Cmd) -> bool) -> Option<Cmd> {
    if predicate(&command) {
        return Some(command);
    }
    if let Cmd::Batch(commands) = command {
        return commands
            .into_iter()
            .find_map(|cmd| find_effect(cmd, predicate));
    }
    None
}

fn request(model: &mut AppModel) -> FileRequest {
    let command = update(model, Msg::App(AppMsg::FilesChanged(Vec::new()))).unwrap();
    let Cmd::ObserveFile(target) =
        find_effect(command, |cmd| matches!(cmd, Cmd::ObserveFile(_))).unwrap()
    else {
        panic!("observation")
    };
    target
}

fn deliver(model: &mut AppModel, target: FileRequest, content: DiskContent) {
    let identity = target.source_identity.clone();
    update(
        model,
        Msg::App(AppMsg::FileObserved {
            target,
            observed: ObservedFile { content, identity },
        }),
    );
}

fn observe(model: &mut AppModel, content: DiskContent) {
    let target = request(model);
    deliver(model, target, content);
}

#[test]
fn external_change_reloads_clean_shared_panes_without_moving_focus_or_selection() {
    let mut m = model("one\ntwo\nthree\nfour\n");
    m.editor_mut().cursors[0].line = 2;
    m.editor_mut().cursors[0].column = 3;
    m.editor_mut().collapse_selections_to_cursors();
    update(
        &mut m,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    // Splitting creates a fresh pane, rather than copying the original caret.
    for editor in m.editor_area.editors.values_mut() {
        editor.cursors[0].line = 2;
        editor.cursors[0].column = 3;
        editor.collapse_selections_to_cursors();
    }
    let focus = m.editor_area.focused_group_id;
    observe(
        &mut m,
        DiskContent::Text("new\ncontent\nthree\nfour\n".into()),
    );
    assert_eq!(
        m.document().buffer.to_string(),
        "new\ncontent\nthree\nfour\n"
    );
    assert!(!m.document().is_modified);
    assert!(m.ui.active_modal.is_none());
    assert_eq!(m.editor_area.focused_group_id, focus);
    for editor in m.editor_area.editors.values() {
        assert_eq!((editor.cursors[0].line, editor.cursors[0].column), (2, 3));
        assert_eq!(editor.selections[0].head.line, 2);
    }
    observe(&mut m, DiskContent::Text("x".into()));
    for editor in m.editor_area.editors.values() {
        assert_eq!((editor.cursors[0].line, editor.cursors[0].column), (0, 1));
    }
}

#[test]
fn external_change_dirty_conflict_defaults_to_keep_and_rechecks_approved_overwrite() {
    let mut m = model("original");
    update(&mut m, Msg::Document(DocumentMsg::InsertChar('!')));
    let local = m.document().buffer.clone();
    observe(&mut m, DiskContent::Text("outside".into()));
    assert!(
        matches!(&m.ui.active_modal, Some(ModalState::FileConflict(state)) if state.selected_index == 0)
    );
    update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));
    assert_eq!(m.document().buffer, local);
    assert!(m.document().external_change.is_some());
    assert!(m.ui.active_modal.is_none());
    // Identical notifications do not nag after Keep Editing.
    observe(&mut m, DiskContent::Text("outside".into()));
    assert!(m.ui.active_modal.is_none());
    update(&mut m, Msg::App(AppMsg::SaveFile));
    update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
    update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
    let command = update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm))).unwrap();
    let Cmd::SaveFile {
        target, content, ..
    } = find_effect(command, |cmd| matches!(cmd, Cmd::SaveFile { .. })).unwrap()
    else {
        panic!("save")
    };
    assert_eq!(content, local);
    assert_eq!(target.write_guard.saved.unwrap().to_string(), "outside");
    assert!(target.write_guard.queued.is_none());
    assert!(!target.write_guard.save_as);
}

#[test]
fn external_change_manual_reload_rejects_intervening_edits_and_missing_files_offer_recreation() {
    let mut m = model("original");
    m.config.auto_reload = false;
    observe(&mut m, DiskContent::Text("outside".into()));
    update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
    let command = update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm))).unwrap();
    let Cmd::LoadFile { target, path } =
        find_effect(command, |cmd| matches!(cmd, Cmd::LoadFile { .. })).unwrap()
    else {
        panic!("reload")
    };
    assert!(target.external_reload);
    update(&mut m, Msg::Document(DocumentMsg::InsertChar('!')));
    update(
        &mut m,
        Msg::App(AppMsg::FileLoaded {
            target,
            path,
            identity: None,
            result: Ok("outside".into()),
        }),
    );
    assert_eq!(m.document().buffer.to_string(), "!original");

    let mut m = model("original");
    observe(&mut m, DiskContent::Missing);
    update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
    let command = update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm))).unwrap();
    let Cmd::SaveFile { target, .. } =
        find_effect(command, |cmd| matches!(cmd, Cmd::SaveFile { .. })).unwrap()
    else {
        panic!("recreate")
    };
    assert!(target.write_guard.saved.is_none());
    assert!(target.write_guard.queued.is_none());
}

#[test]
fn external_change_csv_reload_preserves_grid_and_waits_for_cell_edits() {
    let mut m = model("a,b\n1,2\n");
    update(&mut m, Msg::Csv(CsvMsg::Toggle));
    observe(&mut m, DiskContent::Text("a,b\n3,4\n".into()));
    assert!(m.editor().view_mode.is_csv());
    assert_eq!(m.document().buffer.to_string(), "a,b\n3,4\n");
    update(&mut m, Msg::Csv(CsvMsg::StartEditing));
    update(&mut m, Msg::Csv(CsvMsg::EditInsertChar('z')));
    observe(&mut m, DiskContent::Text("a,b\n5,6\n".into()));
    assert_eq!(m.document().buffer.to_string(), "a,b\n3,4\n");
    assert!(m.editor().view_mode.as_csv().unwrap().is_editing());
    assert!(m.ui.active_modal.is_none());
    assert!(m.document().external_change.is_some());
    update(&mut m, Msg::Csv(CsvMsg::ConfirmEdit));
    assert!(matches!(
        m.ui.active_modal,
        Some(ModalState::FileConflict(_))
    ));
}

#[test]
fn external_change_observation_captured_before_save_cannot_replace_saved_buffer() {
    let mut m = model("original");
    let observation = request(&mut m);
    let command = update(&mut m, Msg::App(AppMsg::SaveFile)).unwrap();
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = find_effect(command, |cmd| matches!(cmd, Cmd::SaveFile { .. })).unwrap()
    else {
        panic!("save")
    };
    deliver(
        &mut m,
        observation,
        DiskContent::Text("stale disk version".into()),
    );
    assert_eq!(m.document().buffer.to_string(), "original");
    let command = update(
        &mut m,
        Msg::App(AppMsg::SaveCompleted {
            target,
            path,
            content,
            identity: None,
            result: Ok(()),
        }),
    )
    .unwrap();
    assert!(find_effect(command, |cmd| matches!(cmd, Cmd::ObserveFile(_))).is_some());
}

#[test]
fn external_change_tracks_a_retargeted_alias_even_when_contents_are_identical() {
    let mut m = model("original");
    let target = request(&mut m);
    let new_path = std::path::PathBuf::from("/another-directory/target.csv");
    let identity =
        token::util::FileIdentity::from_resolved(target.source_path.clone().unwrap(), &new_path);
    update(
        &mut m,
        Msg::App(AppMsg::FileObserved {
            target,
            observed: ObservedFile {
                content: DiskContent::Text("original".into()),
                identity: Some(identity.clone()),
            },
        }),
    );
    assert_eq!(m.document().file_identity(), Some(&identity));
    assert!(!m.document().is_modified);
    let command = update(&mut m, Msg::App(AppMsg::FilesChanged(vec![new_path]))).unwrap();
    assert!(find_effect(command, |cmd| matches!(cmd, Cmd::ObserveFile(_))).is_some());
}
