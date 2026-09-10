//! Closing must never turn a cancelled/failed save into lost edits.
mod common;

use token::{
    commands::Cmd,
    messages::{AppMsg, CsvMsg, DocumentMsg, LayoutMsg, ModalMsg, Msg, UiMsg},
    model::{AppModel, DocumentId, ModalState, SplitDirection, TabId},
    update::update,
};

fn model() -> AppModel {
    let mut model = common::test_model("text", 0, 4);
    model.document_mut().file_path = Some("/fixture/close.txt".into());
    model.config.editorconfig = false;
    model.config.format_on_save = false;
    model.config.lsp.enabled = false;
    model
}

fn edit(model: &mut AppModel) {
    update(model, Msg::Document(DocumentMsg::InsertChar('!')));
}

fn choose(model: &mut AppModel, action: usize) -> Option<Cmd> {
    update(model, Msg::Ui(UiMsg::Modal(ModalMsg::ActivateRow(action))))
}

fn confirmation(model: &AppModel) -> &token::model::UnsavedChangesState {
    let Some(ModalState::UnsavedChanges(state)) = &model.ui.active_modal else {
        panic!(
            "expected unsaved-change confirmation, got {:?}",
            model.ui.active_modal
        )
    };
    state
}

fn command(result: &Option<Cmd>, predicate: fn(&Cmd) -> bool) -> Option<Cmd> {
    fn find(cmd: &Cmd, predicate: fn(&Cmd) -> bool) -> Option<Cmd> {
        if predicate(cmd) {
            Some(cmd.clone())
        } else if let Cmd::Batch(commands) = cmd {
            commands.iter().find_map(|cmd| find(cmd, predicate))
        } else {
            None
        }
    }
    result.as_ref().and_then(|cmd| find(cmd, predicate))
}

fn write(result: &Option<Cmd>) -> Cmd {
    command(result, |cmd| matches!(cmd, Cmd::SaveFile { .. })).expect("expected save effect")
}

fn complete(model: &mut AppModel, command: Cmd, result: Result<(), String>) -> Option<Cmd> {
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = command
    else {
        panic!("expected write")
    };
    update(
        model,
        Msg::App(AppMsg::SaveCompleted {
            target,
            path,
            content,
            identity: None,
            result,
        }),
    )
}

fn background_dirty_tab(model: &mut AppModel) -> (TabId, DocumentId) {
    edit(model);
    let tab = model
        .editor_area
        .focused_group()
        .unwrap()
        .active_tab()
        .unwrap()
        .id;
    let doc = model.document().id.unwrap();
    update(model, Msg::Layout(LayoutMsg::NewTab));
    (tab, doc)
}

#[test]
fn closing_cancel_and_stale_discard_preserve_changes() {
    let mut m = model();
    edit(&mut m);
    let result = update(&mut m, Msg::App(AppMsg::Quit));
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_none());
    assert_eq!(confirmation(&m).selected_index, 0);
    choose(&mut m, 0);
    assert!(m.ui.active_modal.is_none());
    assert_eq!(m.document().buffer.to_string(), "text!");

    update(&mut m, Msg::App(AppMsg::Quit));
    // Model updates from other sources can still arrive while a modal is open.
    edit(&mut m);
    let result = choose(&mut m, 2);
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_none());
    assert_eq!(confirmation(&m).selected_index, 0);
    assert_eq!(m.document().buffer.to_string(), "text!!");
    let result = choose(&mut m, 2);
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_some());
}

#[test]
fn closing_background_tab_waits_for_save_success_and_preserves_failures_or_newer_edits() {
    for outcome in ["success", "failure", "newer edits", "cancel"] {
        let mut m = model();
        let (tab, doc) = background_dirty_tab(&mut m);
        let focused = m.document().id;
        update(&mut m, Msg::Layout(LayoutMsg::CloseTab(tab)));
        let saving = write(&choose(&mut m, 1));
        assert!(m.editor_area.documents.contains_key(&doc));
        assert_eq!(
            m.document().id,
            focused,
            "saving a background tab must not move focus"
        );
        assert_eq!(confirmation(&m).actions(), &["Cancel Closing"]);
        if outcome == "newer edits" {
            let document = m.editor_area.documents.get_mut(&doc).unwrap();
            document.buffer.insert(0, "new");
            document.revision += 1;
        } else if outcome == "cancel" {
            update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
        }
        let result = if outcome == "failure" {
            Err("disk full".into())
        } else {
            Ok(())
        };
        complete(&mut m, saving, result);
        assert_eq!(
            m.editor_area.documents.contains_key(&doc),
            outcome != "success",
            "{outcome}"
        );
        assert!(m.ui.active_modal.is_none());
        if outcome == "failure" {
            assert!(m
                .ui
                .transient_message
                .as_ref()
                .unwrap()
                .text
                .contains("disk full"));
        }
    }
}

#[test]
fn closing_quit_serializes_untitled_save_dialogs_and_cancellation_stops_closing() {
    let mut m = model();
    m.document_mut().file_path = None;
    let direct = update(&mut m, Msg::App(AppMsg::SaveFile));
    let Cmd::ShowSaveFileDialog { target, .. } =
        command(&direct, |cmd| matches!(cmd, Cmd::ShowSaveFileDialog { .. })).unwrap()
    else {
        unreachable!()
    };
    update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult { target, path: None }),
    );
    edit(&mut m);
    update(&mut m, Msg::Layout(LayoutMsg::NewTab));
    edit(&mut m);
    update(&mut m, Msg::App(AppMsg::Quit));
    assert_eq!(confirmation(&m).actions()[1], "Save All");
    let result = choose(&mut m, 1);
    let Cmd::ShowSaveFileDialog { target, .. } =
        command(&result, |cmd| matches!(cmd, Cmd::ShowSaveFileDialog { .. })).unwrap()
    else {
        unreachable!()
    };
    let result = update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult {
            target,
            path: Some("/fixture/first.txt".into()),
        }),
    );
    let result = complete(&mut m, write(&result), Ok(()));
    let Cmd::ShowSaveFileDialog { target, .. } =
        command(&result, |cmd| matches!(cmd, Cmd::ShowSaveFileDialog { .. })).unwrap()
    else {
        unreachable!()
    };
    let result = update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult { target, path: None }),
    );
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_none());
    assert_eq!(m.editor_area.documents.len(), 2);
    assert_eq!(
        m.editor_area
            .documents
            .values()
            .filter(|doc| doc.is_modified)
            .count(),
        1
    );
    assert!(m.ui.active_modal.is_none());

    // Retry the remaining document; Quit occurs only after its write completes.
    update(&mut m, Msg::App(AppMsg::Quit));
    let result = choose(&mut m, 1);
    let Cmd::ShowSaveFileDialog { target, .. } =
        command(&result, |cmd| matches!(cmd, Cmd::ShowSaveFileDialog { .. })).unwrap()
    else {
        unreachable!()
    };
    let result = update(
        &mut m,
        Msg::App(AppMsg::SaveFileAsDialogResult {
            target,
            path: Some("/fixture/second.txt".into()),
        }),
    );
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_none());
    let result = complete(&mut m, write(&result), Ok(()));
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_some());
}

#[test]
fn closing_shared_view_does_not_prompt_but_last_dirty_view_does() {
    let mut m = model();
    edit(&mut m);
    let doc = m.document().id.unwrap();
    update(
        &mut m,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    update(&mut m, Msg::Layout(LayoutMsg::CloseFocusedGroup));
    assert!(m.ui.active_modal.is_none());
    assert_eq!(m.editor_area.groups.len(), 1);
    assert!(m.editor_area.documents[&doc].is_modified);
    let tab = m
        .editor_area
        .focused_group()
        .unwrap()
        .active_tab()
        .unwrap()
        .id;
    update(&mut m, Msg::Layout(LayoutMsg::NewTab));
    update(&mut m, Msg::Layout(LayoutMsg::CloseTab(tab)));
    confirmation(&m);
    choose(&mut m, 2);
    assert!(!m.editor_area.documents.contains_key(&doc));
}

#[test]
fn closing_csv_cell_buffer_is_preserved_on_cancel_and_committed_before_save() {
    let mut m = common::test_model("name,value\na,1\n", 0, 0);
    m.config.editorconfig = false;
    m.config.format_on_save = false;
    m.document_mut().file_path = Some("/fixture/table.csv".into());
    update(&mut m, Msg::Csv(CsvMsg::Toggle));
    update(&mut m, Msg::Csv(CsvMsg::StartEditingWithChar('Z')));
    assert!(
        !m.document().is_modified,
        "cell edit is not in the document yet"
    );
    update(&mut m, Msg::App(AppMsg::Quit));
    choose(&mut m, 0);
    assert_eq!(
        m.editor()
            .view_mode
            .as_csv()
            .unwrap()
            .editing
            .as_ref()
            .unwrap()
            .buffer(),
        "Z"
    );
    update(&mut m, Msg::App(AppMsg::Quit));
    let saving = write(&choose(&mut m, 1));
    let Cmd::SaveFile { content, .. } = &saving else {
        unreachable!()
    };
    assert!(content.to_string().contains('Z'));
    assert!(m.editor().view_mode.as_csv().unwrap().editing.is_none());
    let result = complete(&mut m, saving, Ok(()));
    assert!(command(&result, |cmd| matches!(cmd, Cmd::Quit)).is_some());
}

#[test]
fn closing_csv_conflicting_views_and_missing_cells_do_not_lose_edit_buffers() {
    for missing_cell in [false, true] {
        let mut m = common::test_model("name,value\na,1\n", 0, 0);
        m.config.editorconfig = false;
        m.config.format_on_save = false;
        update(&mut m, Msg::Csv(CsvMsg::Toggle));
        update(&mut m, Msg::Csv(CsvMsg::StartEditingWithChar('Z')));
        if missing_cell {
            // Another view changed the source while this cell was being edited.
            m.document_mut().buffer = "".into();
            m.editor_area
                .focused_editor_mut()
                .unwrap()
                .view_mode
                .as_csv_mut()
                .unwrap()
                .editing
                .as_mut()
                .unwrap()
                .position
                .row = 5;
        } else {
            update(
                &mut m,
                Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
            );
            update(&mut m, Msg::Csv(CsvMsg::EditInsertChar('W')));
        }
        update(&mut m, Msg::App(AppMsg::Quit));
        let result = choose(&mut m, 1);
        assert!(command(&result, |cmd| matches!(
            cmd,
            Cmd::Quit | Cmd::SaveFile { .. } | Cmd::ShowSaveFileDialog { .. }
        ))
        .is_none());
        assert!(m.ui.active_modal.is_none());
        assert!(m.editor().view_mode.as_csv().unwrap().editing.is_some());
        assert!(m
            .ui
            .transient_message
            .as_ref()
            .unwrap()
            .text
            .contains("Closing cancelled"));
    }
}

#[test]
fn closing_all_tabs_keeps_last_tab_and_group_close_protects_unique_documents() {
    for close_group in [false, true] {
        let mut m = model();
        if close_group {
            update(
                &mut m,
                Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
            );
        }
        update(&mut m, Msg::Layout(LayoutMsg::NewTab));
        edit(&mut m);
        let dirty = m.document().id.unwrap();
        update(&mut m, Msg::Layout(LayoutMsg::NewTab));
        edit(&mut m);
        let survivor = m.document().id.unwrap();
        let group_id = m.editor_area.focused_group_id;
        update(
            &mut m,
            Msg::Layout(if close_group {
                LayoutMsg::CloseGroup(group_id)
            } else {
                LayoutMsg::CloseAllTabs { group_id }
            }),
        );
        confirmation(&m);
        assert!(m.editor_area.documents.contains_key(&dirty));
        choose(&mut m, 2);
        assert!(!m.editor_area.documents.contains_key(&dirty));
        assert_eq!(
            m.editor_area.documents.contains_key(&survivor),
            !close_group
        );
        assert_eq!(m.editor_area.groups.len(), 1);
    }
}

#[test]
fn closing_save_does_not_override_external_file_conflicts() {
    let mut m = model();
    let (tab, doc) = background_dirty_tab(&mut m);
    m.editor_area
        .documents
        .get_mut(&doc)
        .unwrap()
        .external_change = Some(token::model::ExternalFileChange {
        observed: token::model::ObservedFile {
            content: token::model::DiskContent::Text("external".into()),
            identity: None,
        },
        notified: false,
    });
    update(&mut m, Msg::Layout(LayoutMsg::CloseTab(tab)));
    let result = choose(&mut m, 1);
    assert!(command(&result, |cmd| matches!(cmd, Cmd::SaveFile { .. })).is_none());
    assert!(
        matches!(&m.ui.active_modal, Some(ModalState::FileConflict(state)) if state.document_id == doc)
    );
    assert!(m.editor_area.documents[&doc].is_modified);
}
