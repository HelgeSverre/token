//! Save requests must retain document identity, history and newer edits.
mod common;

use token::{
    commands::Cmd,
    config::{AutoSaveConfig, AutoSaveMode},
    messages::{AppMsg, AutoSaveRequest, DocumentMsg, LayoutMsg, LspMsg, Msg},
    model::{AppModel, SaveReason},
    update::update,
};

fn model() -> AppModel {
    let mut m = common::test_model("text", 0, 4);
    m.document_mut().file_path = Some("/fixture/a.txt".into());
    m.config.auto_save.mode = AutoSaveMode::AfterDelay;
    update(&mut m, Msg::Document(DocumentMsg::InsertChar('!')));
    m
}

fn request(m: &AppModel, reason: SaveReason) -> AutoSaveRequest {
    AutoSaveRequest {
        document_id: m.document().id.unwrap(),
        revision: m.document().revision,
        path: m.document().file_path.clone().unwrap(),
        policy: m.config.auto_save.clone(),
        reason,
    }
}

fn find_command(cmd: &Cmd, pred: fn(&Cmd) -> bool) -> Option<Cmd> {
    if pred(cmd) {
        Some(cmd.clone())
    } else if let Cmd::Batch(cmds) = cmd {
        cmds.iter().find_map(|cmd| find_command(cmd, pred))
    } else {
        None
    }
}

fn write(cmd: Option<Cmd>) -> Option<Cmd> {
    cmd.as_ref()
        .and_then(|cmd| find_command(cmd, |cmd| matches!(cmd, Cmd::SaveFile { .. })))
}

fn complete(m: &mut AppModel, cmd: Cmd, result: Result<(), String>) {
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = cmd
    else {
        panic!("expected write")
    };
    update(
        m,
        Msg::App(AppMsg::SaveCompleted {
            target,
            path,
            content,
            result,
            identity: None,
        }),
    );
}

#[test]
fn auto_save_config_defaults_custom_values_and_bounds() {
    let config: token::config::EditorConfig = serde_yaml::from_str("{}").unwrap();
    assert_eq!(config.auto_save, AutoSaveConfig::default());
    for mode in [
        "off",
        "on_focus_loss",
        "after_delay",
        "on_focus_loss_and_delay",
    ] {
        let text =
            format!("auto_save:\n  mode: {mode}\n  delay_ms: 1375\n  format_on_save: true\n");
        let config: token::config::EditorConfig = serde_yaml::from_str(&text).unwrap();
        assert_eq!(config.auto_save.delay().as_millis(), 1375);
        assert!(config.auto_save.format_on_save);
        let roundtrip: token::config::EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert_eq!(config.auto_save, roundtrip.auto_save);
    }
    let mut config = AutoSaveConfig {
        delay_ms: 0,
        ..Default::default()
    };
    assert_eq!(config.delay().as_millis(), 100);
    config.delay_ms = u64::MAX;
    assert_eq!(config.delay().as_secs(), 86400);
}

#[test]
fn auto_save_preserves_newer_edits_and_history_after_completion() {
    let mut m = model();
    let trigger = request(&m, SaveReason::Idle);
    let cmd = write(update(&mut m, Msg::App(AppMsg::AutoSave(trigger)))).unwrap();
    update(&mut m, Msg::Document(DocumentMsg::InsertChar('?')));
    let history = m.document().undo_stack.len();
    complete(&mut m, cmd, Ok(()));
    assert_eq!(m.document().buffer.to_string(), "text!?");
    assert!(m.document().is_modified);
    assert_eq!(m.document().undo_stack.len(), history);
    update(&mut m, Msg::Document(DocumentMsg::Undo));
    assert!(!m.document().is_modified);
}

#[test]
fn auto_save_rejects_stale_revision_path_policy_and_ineligible_content() {
    for change in 0..7 {
        let mut m = model();
        let trigger = request(&m, SaveReason::Idle);
        match change {
            0 => {
                update(&mut m, Msg::Document(DocumentMsg::InsertChar('?')));
            }
            1 => m.document_mut().file_path = Some("/fixture/b.txt".into()),
            2 => m.config.auto_save.mode = AutoSaveMode::Off,
            3 => m.document_mut().file_path = None,
            4 => m.document_mut().is_modified = false,
            5 => m.document_mut().save_error = Some((m.document().revision, "disk full".into())),
            6 => {
                update(&mut m, Msg::App(AppMsg::SaveFileAs));
            }
            _ => unreachable!(),
        }
        assert!(
            write(update(&mut m, Msg::App(AppMsg::AutoSave(trigger)))).is_none(),
            "case {change}"
        );
    }
}

#[test]
fn auto_save_coalesces_pending_write_and_rearms_after_new_edit_on_failure() {
    let mut m = model();
    let trigger = request(&m, SaveReason::Idle);
    let cmd = write(update(&mut m, Msg::App(AppMsg::AutoSave(trigger.clone())))).unwrap();
    assert!(write(update(&mut m, Msg::App(AppMsg::AutoSave(trigger.clone())))).is_none());
    complete(&mut m, cmd, Err("disk full".into()));
    assert!(m.document().is_modified);
    assert!(m.document().save_error.is_some());
    assert!(write(update(&mut m, Msg::App(AppMsg::AutoSave(trigger)))).is_none());
    update(&mut m, Msg::Document(DocumentMsg::InsertChar('?')));
    let trigger = request(&m, SaveReason::Idle);
    let cmd = write(update(&mut m, Msg::App(AppMsg::AutoSave(trigger)))).unwrap();
    complete(&mut m, cmd, Ok(()));
    assert!(!m.document().is_modified);
    assert!(m.document().save_error.is_none());
}

fn formatting(m: &mut AppModel, automatic: bool) -> Cmd {
    m.config.format_on_save = !automatic;
    m.config.auto_save.format_on_save = automatic;
    let msg = if automatic {
        AppMsg::AutoSave(request(m, SaveReason::Idle))
    } else {
        AppMsg::SaveFile
    };
    let cmd = update(m, Msg::App(msg)).unwrap();
    find_command(&cmd, |cmd| matches!(cmd, Cmd::LspRequestFormatting { .. })).unwrap()
}

fn formatted(m: &mut AppModel, cmd: Cmd, new_text: Option<&str>) -> Option<Cmd> {
    let Cmd::LspRequestFormatting {
        document_id,
        revision,
        save,
        ..
    } = cmd
    else {
        panic!("formatter")
    };
    let edits = new_text.map(|text| {
        vec![(
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 4),
            ),
            text.into(),
        )]
    });
    update(
        m,
        Msg::Lsp(LspMsg::FormattingResolved {
            document_id,
            revision,
            save,
            edits,
        }),
    )
}

#[test]
fn save_formatting_follows_document_across_focus_changes() {
    for automatic in [false, true] {
        let mut m = model();
        let id = m.document().id.unwrap();
        let cmd = formatting(&mut m, automatic);
        update(&mut m, Msg::Layout(LayoutMsg::NewTab));
        let other = m.document().id.unwrap();
        assert_ne!(id, other);
        let saved = write(formatted(&mut m, cmd, Some("TEXT"))).unwrap();
        let Cmd::SaveFile {
            target, content, ..
        } = saved
        else {
            unreachable!()
        };
        assert_eq!(target.document_id, id);
        assert_eq!(content.to_string(), "TEXT!");
        assert_eq!(m.document().id, Some(other));
        assert_eq!(m.document().buffer.to_string(), "");
    }
}

#[test]
fn save_formatting_tokens_reject_superseded_save_and_dialog() {
    for dialog in [false, true] {
        let mut m = model();
        let old = formatting(&mut m, true);
        if dialog {
            update(&mut m, Msg::App(AppMsg::SaveFileAs));
        } else {
            formatting(&mut m, false);
        }
        assert!(write(formatted(&mut m, old, Some("BAD"))).is_none());
        assert_eq!(m.document().buffer.to_string(), "text!");
    }
}

#[test]
fn automatic_formatter_waits_for_new_idle_but_manual_save_keeps_latest_text() {
    for automatic in [false, true] {
        let mut m = model();
        let old = formatting(&mut m, automatic);
        update(&mut m, Msg::Document(DocumentMsg::InsertChar('?')));
        let saved = write(formatted(&mut m, old, Some("BAD")));
        assert_eq!(saved.is_none(), automatic);
        if let Some(Cmd::SaveFile { content, .. }) = saved {
            assert_eq!(content.to_string(), "text!?");
        }
        assert_eq!(m.document().buffer.to_string(), "text!?");
    }
}

#[test]
fn auto_save_disabled_during_formatting_does_not_apply_edits() {
    let mut m = model();
    let old = formatting(&mut m, true);
    m.config.auto_save.mode = AutoSaveMode::Off;
    assert!(write(formatted(&mut m, old, Some("BAD"))).is_none());
    assert_eq!(m.document().buffer.to_string(), "text!");
    assert!(!m.document().save_busy());
}

#[test]
fn interactive_formatting_settles_an_earlier_save_before_replacing_its_request() {
    let mut m = model();
    let old = formatting(&mut m, false);
    let next = update(
        &mut m,
        Msg::Lsp(LspMsg::FormatDocument {
            selection_only: false,
        }),
    )
    .unwrap();
    assert!(write(Some(next.clone())).is_some());
    assert!(find_command(&next, |cmd| matches!(
        cmd,
        Cmd::LspRequestFormatting { save: None, .. }
    ))
    .is_some());
    assert!(write(formatted(&mut m, old, Some("BAD"))).is_none());
    assert_eq!(m.document().buffer.to_string(), "text!");
}

#[test]
fn save_formatting_does_not_apply_an_old_language_result() {
    let mut m = model();
    let cmd = formatting(&mut m, false);
    m.document_mut().language = token::syntax::LanguageId::Rust;
    let saved = write(formatted(&mut m, cmd, Some("BAD"))).unwrap();
    let Cmd::SaveFile { content, .. } = saved else {
        unreachable!()
    };
    assert_eq!(content.to_string(), "text!");
}
