//! Keymap preferences use the same typed sequences and merge rules as dispatch.
use token::keymap::preferences::{parse_sequence, BaseKeymap, KeymapSnapshot};
use token::keymap::{Command, Keybinding};
mod common;

use token::messages::{ModalMsg, Msg, SettingsMsg, UiMsg};
use token::model::{AppModel, ModalId, ModalState};
use token::settings::{keymap::SettingsTab, SettingsState};
use token::update::update;
use token::Cmd;

fn modal(model: &mut AppModel, message: ModalMsg) -> Option<Cmd> {
    update(model, Msg::Ui(UiMsg::Modal(message)))
}

fn state(model: &AppModel) -> &SettingsState {
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!("Settings must remain open")
    };
    state
}

fn request(cmd: &Cmd) -> Option<&token::keymap::preferences::KeymapSave> {
    match cmd {
        Cmd::PrepareKeymap { save, .. } => save.as_deref(),
        Cmd::Batch(cmds) => cmds.iter().find_map(request),
        _ => None,
    }
}

fn open_keymap(model: &mut AppModel) {
    update(model, Msg::Ui(UiMsg::ToggleModal(ModalId::Settings)));
    modal(
        model,
        ModalMsg::ActivateTab(token::settings::categories().len() - 1),
    );
    assert!(state(model).keymap.loading);
    let session = state(model).keymap.session.clone();
    update(
        model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::KeymapResult {
            session,
            saved: false,
            result: Ok(Box::new(KeymapSnapshot::parse(None).unwrap())),
        })),
    );
}

fn capture(model: &mut AppModel, key: &str) -> Option<Cmd> {
    update(
        model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::CaptureKey(
            parse_sequence(key).unwrap()[0],
        ))),
    )
}

#[test]
fn settings_keymap_filtered_capture_cancel_and_reserved_keys_are_explicit() {
    let mut model = common::test_model("unchanged", 0, 0);
    open_keymap(&mut model);
    assert_eq!(state(&model).tab, SettingsTab::Keymap);
    modal(&mut model, ModalMsg::SetInput("SaveFile".into()));
    let index = state(&model)
        .filtered_rows()
        .position(|(label, _)| label == "SaveFile")
        .unwrap();
    modal(&mut model, ModalMsg::ActivateRow(index));
    assert_eq!(
        state(&model).keymap.capture.as_ref().unwrap().command,
        Command::SaveFile
    );
    let empty = capture(&mut model, "ctrl+enter").unwrap();
    assert!(request(&empty).is_none());
    assert!(state(&model).keymap.status.contains("at least one"));
    capture(&mut model, "ctrl+k");
    capture(&mut model, "backspace");
    assert!(state(&model)
        .keymap
        .capture
        .as_ref()
        .unwrap()
        .strokes
        .is_empty());
    modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 2 });
    capture(&mut model, "escape");
    assert_eq!(
        state(&model).keymap.capture.as_ref().unwrap().strokes,
        parse_sequence("escape").unwrap()
    );
    // Query and tabs cannot move while a binding is being captured.
    modal(&mut model, ModalMsg::SetInput("Quit".into()));
    modal(&mut model, ModalMsg::NextTab);
    assert_eq!(state(&model).input(), "SaveFile");
    assert_eq!(state(&model).tab, SettingsTab::Keymap);
    let cancelled = capture(&mut model, "escape").unwrap();
    assert!(request(&cancelled).is_none());
    assert!(state(&model).keymap.capture.is_none());
    assert!(state(&model)
        .keymap
        .snapshot
        .as_ref()
        .unwrap()
        .source
        .is_none());
    assert_eq!(model.document().buffer.to_string(), "unchanged");
}

#[test]
fn settings_keymap_save_is_explicit_failure_retains_capture_and_success_applies() {
    let mut model = common::test_model("unchanged", 0, 0);
    open_keymap(&mut model);
    modal(&mut model, ModalMsg::SetInput("SaveFile".into()));
    let index = state(&model)
        .filtered_rows()
        .position(|(label, _)| label == "SaveFile")
        .unwrap();
    modal(&mut model, ModalMsg::ActivateRow(index));
    for key in ["ctrl+k", "ctrl+f24", "ctrl+plus", "alt+s", "ctrl+x"] {
        assert!(request(&capture(&mut model, key).unwrap()).is_none());
    }
    assert_eq!(
        state(&model).keymap.capture.as_ref().unwrap().strokes.len(),
        4
    );
    let cmd = capture(&mut model, "ctrl+enter").unwrap();
    let save = request(&cmd).unwrap().clone();
    assert!(save.expected.is_none());
    assert!(state(&model).keymap.saving);
    let session = state(&model).keymap.session.clone();
    update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::KeymapResult {
            session: session.clone(),
            saved: true,
            result: Err("disk failed".into()),
        })),
    );
    assert!(!state(&model).keymap.saving);
    assert_eq!(state(&model).keymap.status, "disk failed");
    assert!(state(&model).keymap.capture.is_some());
    let token::keymap::preferences::KeymapChange::Rebind {
        original,
        command,
        strokes,
    } = save.change
    else {
        panic!("rebind")
    };
    let text = state(&model)
        .keymap
        .snapshot
        .as_ref()
        .unwrap()
        .rebind(original.as_ref(), command, &strokes)
        .unwrap();
    update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::KeymapResult {
            session,
            saved: true,
            result: Ok(Box::new(KeymapSnapshot::parse(Some(text)).unwrap())),
        })),
    );
    assert!(state(&model).keymap.capture.is_none());
    assert!(state(&model)
        .keymap
        .snapshot
        .as_ref()
        .unwrap()
        .bindings
        .contains(&Keybinding::chord(strokes, Command::SaveFile)));
}

#[test]
fn settings_keymap_stale_load_is_ignored_and_base_choice_only_requests_keymap_save() {
    let mut model = common::test_model("", 0, 0);
    open_keymap(&mut model);
    let stale_session = state(&model).keymap.session.clone();
    modal(&mut model, ModalMsg::Close);
    open_keymap(&mut model);
    update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::KeymapResult {
            session: stale_session,
            saved: false,
            result: Err("stale error".into()),
        })),
    );
    assert!(!state(&model).keymap.status.contains("stale"));
    let cmd = modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 1 }).unwrap();
    assert!(matches!(
        request(&cmd).unwrap().change,
        token::keymap::preferences::KeymapChange::Base(BaseKeymap::Conventional)
    ));
    assert_eq!(
        state(&model).keymap.snapshot.as_ref().unwrap().base,
        BaseKeymap::Token
    );
}

#[test]
fn settings_keymap_background_reply_preserves_general_tab_selection() {
    let mut model = common::test_model("", 0, 0);
    open_keymap(&mut model);
    modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 1 });
    let session = state(&model).keymap.session.clone();
    modal(&mut model, ModalMsg::PrevTab);
    modal(&mut model, ModalMsg::SelectNext);
    assert_eq!(state(&model).selected_index(), 1);
    update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::KeymapResult {
            session,
            saved: true,
            result: Ok(Box::new(
                KeymapSnapshot::parse(Some("base: conventional\nbindings: []\n".into())).unwrap(),
            )),
        })),
    );
    assert_eq!(state(&model).tab, SettingsTab::General);
    assert_eq!(state(&model).selected_index(), 1);
    assert_eq!(
        state(&model).keymap.snapshot.as_ref().unwrap().base,
        BaseKeymap::Conventional
    );
}
