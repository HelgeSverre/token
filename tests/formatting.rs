//! External and LSP formatting share document/save continuations.
mod common;
use std::sync::Arc;
use token::commands::Cmd;
use token::messages::{AppMsg, AutoSaveRequest, DocumentMsg, FormattingMsg, Msg};
use token::model::{AppModel, Position, SaveReason, Selection};
use token::syntax::LanguageId;
use token::update::update;

fn model() -> AppModel {
    let mut model = common::test_model("🐍=1\n", 0, 3);
    model.document_mut().language = LanguageId::Python;
    model.config.editorconfig = false;
    model
}

fn find(cmd: Option<Cmd>, predicate: fn(&Cmd) -> bool) -> Option<Cmd> {
    let cmd = cmd?;
    if predicate(&cmd) {
        return Some(cmd);
    }
    if let Cmd::Batch(commands) = cmd {
        return commands
            .into_iter()
            .find_map(|cmd| find(Some(cmd), predicate));
    }
    None
}

fn request(model: &mut AppModel, selection_only: bool) -> Option<Cmd> {
    update(
        model,
        Msg::Formatting(FormattingMsg::FormatDocument { selection_only }),
    )
}

fn complete(model: &mut AppModel, command: Cmd, result: Result<&str, &str>) -> Option<Cmd> {
    let Cmd::RunFormatter {
        document_id,
        revision,
        language,
        save,
        ..
    } = command
    else {
        panic!("external formatter request");
    };
    update(
        model,
        Msg::Formatting(FormattingMsg::ExternalResolved {
            document_id,
            revision,
            language,
            request: Arc::new(()),
            save,
            result: result.map(str::to_owned).map_err(str::to_owned),
        }),
    )
}

fn save_request(model: &mut AppModel, automatic: bool) -> Cmd {
    model.config.format_on_save = !automatic;
    model.config.auto_save.format_on_save = automatic;
    model.document_mut().file_path = Some("/fixture/file.py".into());
    update(model, Msg::Document(DocumentMsg::InsertChar(' ')));
    let msg = if automatic {
        model.config.auto_save.mode = token::config::AutoSaveMode::AfterDelay;
        AppMsg::AutoSave(AutoSaveRequest {
            document_id: model.document().id.unwrap(),
            revision: model.document().revision,
            path: model.document().file_path.clone().unwrap(),
            policy: model.config.auto_save.clone(),
            reason: SaveReason::Idle,
        })
    } else {
        AppMsg::SaveFile
    };
    find(update(model, Msg::App(msg)), |cmd| {
        matches!(cmd, Cmd::RunFormatter { .. })
    })
    .expect("save uses external formatter")
}

#[test]
fn formatting_prefers_command_with_or_without_lsp_and_for_untitled_buffers() {
    for enabled in [false, true] {
        let mut model = model();
        model.config.lsp.enabled = enabled;
        let command = request(&mut model, false).unwrap();
        let Cmd::RunFormatter {
            formatter,
            text,
            file,
            ..
        } = command
        else {
            panic!("external request");
        };
        assert_eq!(formatter.command, "ruff");
        assert_eq!(text, "🐍=1\n");
        assert!(file.is_none());
    }
}

#[test]
fn formatting_disabled_or_removed_command_uses_lsp_and_selection_always_uses_lsp() {
    for remove in [false, true] {
        let mut model = model();
        if remove {
            model.config.formatters.clear();
        } else {
            model
                .config
                .formatters
                .get_mut(&LanguageId::Python)
                .unwrap()
                .enabled = false;
        }
        assert!(matches!(
            request(&mut model, false),
            Some(Cmd::LspRequestFormatting { range: None, .. })
        ));
    }
    let mut model = model();
    *model.editor_mut().active_selection_mut() =
        Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 3));
    assert!(matches!(
        request(&mut model, true),
        Some(Cmd::LspRequestFormatting { range: Some(_), .. })
    ));
}

#[test]
fn formatting_command_applies_unicode_as_one_undo_step_and_tracks_unchanged_suffix() {
    let mut model = model();
    let before = model.document().buffer.to_string();
    let command = request(&mut model, false).unwrap();
    complete(&mut model, command, Ok("🐍 = 1\n"));
    assert_eq!(model.document().buffer.to_string(), "🐍 = 1\n");
    assert_eq!(model.editor().active_cursor().column, 5);
    assert_eq!(model.document().undo_stack.len(), 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), before);
    assert_eq!(model.editor().active_cursor().column, 3);
}

#[test]
fn formatting_unchanged_output_and_failure_never_create_undo_or_fallback() {
    for result in [Ok("🐍=1\n"), Err("ruff is missing")] {
        let mut model = model();
        let command = request(&mut model, false).unwrap();
        let effects = complete(&mut model, command, result);
        assert_eq!(model.document().buffer.to_string(), "🐍=1\n");
        assert!(model.document().undo_stack.is_empty());
        assert!(find(effects, |cmd| matches!(
            cmd,
            Cmd::LspRequestFormatting { .. }
        ))
        .is_none());
        if result.is_err() {
            assert!(model
                .ui
                .transient_message
                .as_ref()
                .unwrap()
                .text
                .contains("ruff is missing"));
        }
    }
}

#[test]
fn formatting_drops_command_output_when_buffer_has_changed() {
    let mut model = model();
    let command = request(&mut model, false).unwrap();
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    let changed = model.document().buffer.to_string();
    complete(&mut model, command, Ok("old output"));
    assert_eq!(model.document().buffer.to_string(), changed);
}

#[test]
fn formatting_manual_and_automatic_saves_work_with_lsp_disabled_and_warn_on_failure() {
    for automatic in [false, true] {
        for success in [false, true] {
            let mut model = model();
            model.config.lsp.enabled = false;
            let command = save_request(&mut model, automatic);
            let original = model.document().buffer.to_string();
            let effects = complete(
                &mut model,
                command,
                if success {
                    Ok("formatted\n")
                } else {
                    Err("formatter failed")
                },
            );
            let write =
                find(effects, |cmd| matches!(cmd, Cmd::SaveFile { .. })).expect("save continues");
            let Cmd::SaveFile { content, .. } = write else {
                unreachable!()
            };
            assert_eq!(
                content.to_string(),
                if success { "formatted\n" } else { &original }
            );
            if !success {
                assert!(model
                    .ui
                    .transient_message
                    .as_ref()
                    .unwrap()
                    .text
                    .contains("saved unformatted"));
            }
        }
    }
}

#[test]
fn formatting_save_as_uses_destination_filename_and_skips_language_changes() {
    for extension in ["py", "txt"] {
        let mut model = model();
        model.config.format_on_save = true;
        let dialog = find(update(&mut model, Msg::App(AppMsg::SaveFileAs)), |cmd| {
            matches!(cmd, Cmd::ShowSaveFileDialog { .. })
        })
        .unwrap();
        let Cmd::ShowSaveFileDialog { target, .. } = dialog else {
            unreachable!()
        };
        let destination = std::path::PathBuf::from(format!("/fixture/destination.{extension}"));
        let effects = update(
            &mut model,
            Msg::App(AppMsg::SaveFileAsDialogResult {
                target,
                path: Some(destination.clone()),
            }),
        );
        if extension == "py" {
            let Cmd::RunFormatter { file, .. } =
                find(effects, |cmd| matches!(cmd, Cmd::RunFormatter { .. })).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(file, Some(destination));
        } else {
            assert!(find(effects, |cmd| matches!(cmd, Cmd::SaveFile { .. })).is_some());
        }
    }
}

#[test]
fn formatting_preserves_exact_output_across_line_endings_and_empty_documents() {
    for (before, after) in [
        ("x\r\n", "x\n"),
        ("x\n", "x\r\n"),
        ("x\r\ny", "x\ny"),
        ("x\ry", "x\ny"),
        ("x", "x\n"),
        ("x\n", "x"),
        ("", "🐍\n"),
        ("🐍\n", ""),
        ("prefix\n🐍=1\nsuffix\n", "prefix\n🐍 = 1\nsuffix\n"),
    ] {
        let mut model = common::test_model(before, 0, 0);
        model.document_mut().language = LanguageId::Python;
        let command = request(&mut model, false).unwrap();
        complete(&mut model, command, Ok(after));
        assert_eq!(
            model.document().buffer.to_string(),
            after,
            "input {before:?}"
        );
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), before);
    }
}

#[test]
fn formatting_drops_command_output_after_language_changes() {
    let mut model = model();
    let command = request(&mut model, false).unwrap();
    model.document_mut().language = LanguageId::Rust;
    assert!(complete(&mut model, command, Ok("stale Python output")).is_none());
    assert_eq!(model.document().buffer.to_string(), "🐍=1\n");
}

#[test]
fn formatting_settings_toggles_use_shared_persistence_and_lock_while_applying_drafts() {
    use token::messages::{ModalMsg, SettingsMsg, UiMsg};
    let mut model = model();
    update(
        &mut model,
        Msg::Ui(UiMsg::ToggleModal(token::model::ModalId::Settings)),
    );
    let category = token::settings::CategoryId::Formatting.index();
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::ActivateTab(category))),
    );
    let row = |model: &AppModel, label: &str| {
        let Some(token::model::ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!("settings");
        };
        state
            .filtered_rows()
            .position(|(name, _)| name == label)
            .unwrap()
    };
    let toggle = row(&model, "Format on save");
    let choose = |row, choice| Msg::Ui(UiMsg::Modal(ModalMsg::ChooseSetting { row, choice }));
    let effects = update(&mut model, choose(toggle, 1));
    assert!(model.config.format_on_save);
    assert!(find(effects, |cmd| matches!(cmd, Cmd::SaveConfiguration { .. })).is_some());
    let enabled = row(&model, "Use external formatter");
    update(&mut model, choose(enabled, 0));
    assert!(
        model.config.formatters[&LanguageId::Python].enabled,
        "draft is not live yet"
    );
    let save = row(&model, "Configuration");
    let effects = update(&mut model, choose(save, 0));
    let Cmd::ApplySettingsForm { session, change } =
        find(effects, |cmd| matches!(cmd, Cmd::ApplySettingsForm { .. })).unwrap()
    else {
        unreachable!()
    };
    update(&mut model, choose(toggle, 0));
    assert!(
        model.config.format_on_save,
        "controls must remain locked while persisting the draft"
    );
    update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::FormApplied {
            session,
            change,
            result: Ok(()),
        })),
    );
    assert!(!model.config.formatters[&LanguageId::Python].enabled);
    assert!(model.config.format_on_save);
}
