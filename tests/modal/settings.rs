use crate::common::test_model;
use token::messages::{ModalMsg, Msg, UiMsg};
use token::model::{AppModel, ModalId, ModalState};
use token::update::update;
use token::Cmd;

fn modal(model: &mut AppModel, msg: ModalMsg) -> Option<Cmd> {
    update(model, Msg::Ui(UiMsg::Modal(msg)))
}

fn saved(cmd: &Cmd) -> Option<&token::config::EditorConfig> {
    match cmd {
        Cmd::SaveConfiguration { config } => Some(config),
        Cmd::Batch(cmds) => cmds.iter().find_map(saved),
        _ => None,
    }
}

fn open(model: &mut AppModel) {
    let cmd = update(model, Msg::Ui(UiMsg::ToggleModal(ModalId::Settings))).unwrap();
    assert!(saved(&cmd).is_none());
}

#[test]
fn settings_page_category_navigation_filters_without_changing_preferences() {
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    let categories = token::settings::categories();
    for (index, category) in categories
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, category)| **category != Some("Keymap"))
    {
        let cmd = modal(&mut model, ModalMsg::ActivateTab(index)).unwrap();
        assert!(saved(&cmd).is_none());
        let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!("settings page");
        };
        assert_eq!(state.category, index);
        assert!(state
            .filtered_rows()
            .all(|(_, section)| Some(section) == *category));
    }
    modal(&mut model, ModalMsg::ActivateTab(0));
    modal(&mut model, ModalMsg::PrevTab);
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!("settings page");
    };
    assert_eq!(state.category, categories.len() - 1);
    modal(&mut model, ModalMsg::NextTab);
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!("settings page");
    };
    assert_eq!(state.category, 0);
}

#[test]
fn settings_open_close_preserves_off_preset_values_without_saving() {
    let mut model = test_model("text", 0, 0);
    model.config.cursor_blink_ms = 777;
    model.config.status_bar_font_size = 12.5;
    open(&mut model);
    modal(&mut model, ModalMsg::SetInput("cursor_blink_ms".into()));
    let cmd = modal(&mut model, ModalMsg::ActivateRow(0)).unwrap();
    assert!(
        saved(&cmd).is_none(),
        "clicking the label only selects a row"
    );
    assert_eq!(model.config.cursor_blink_ms, 777);
    let cmd = modal(&mut model, ModalMsg::Close).unwrap();
    assert!(saved(&cmd).is_none());
    assert_eq!(model.config.cursor_blink_ms, 777);
    assert_eq!(model.config.status_bar_font_size, 12.5);
}

#[test]
fn settings_filtered_choice_and_keyboard_commit_same_descriptor() {
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    modal(
        &mut model,
        ModalMsg::SetInput("status_bar_font_size".into()),
    );
    let cmd = modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 2 }).unwrap();
    assert_eq!(saved(&cmd).unwrap().status_bar_font_size, 13.0);
    assert!(matches!(
        model.ui.active_modal,
        Some(ModalState::Settings(_))
    ));
    assert!(
        matches!(cmd, Cmd::Batch(ref cmds) if cmds.iter().any(|cmd| matches!(cmd, Cmd::SyncFontMetrics)))
    );
    let cmd = modal(&mut model, ModalMsg::MoveCursorLeft).unwrap();
    assert_eq!(saved(&cmd).unwrap().status_bar_font_size, 12.0);
    let cmd = modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 1 }).unwrap();
    assert!(saved(&cmd).is_none(), "unchanged preset must not write");
    assert!(modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 99 }).is_none());
    assert!(modal(&mut model, ModalMsg::ChooseSetting { row: 99, choice: 0 }).is_none());
}

#[test]
fn settings_empty_search_does_not_mutate_and_theme_uses_existing_picker() {
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    modal(
        &mut model,
        ModalMsg::SetInput("no-such-setting-zzzz".into()),
    );
    assert!(modal(&mut model, ModalMsg::Confirm).is_none());
    modal(&mut model, ModalMsg::SetInput("color scheme".into()));
    let cmd = modal(&mut model, ModalMsg::Confirm).unwrap();
    assert!(saved(&cmd).is_none());
    assert!(matches!(
        model.ui.active_modal,
        Some(ModalState::ThemePicker(_))
    ));
}

#[test]
fn settings_blink_off_keeps_caret_visible() {
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    modal(&mut model, ModalMsg::SetInput("cursor_blink_ms".into()));
    let cmd = modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 0 }).unwrap();
    assert_eq!(saved(&cmd).unwrap().cursor_blink_ms, 0);
    model.ui.cursor_visible = false;
    update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
    assert!(model.ui.cursor_visible);
    assert!(update(&mut model, Msg::Ui(UiMsg::BlinkCursor)).is_none());
}

fn row(model: &AppModel, label: &str) -> usize {
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!("settings");
    };
    state
        .filtered_rows()
        .position(|(name, _)| name == label)
        .expect("visible settings row")
}

fn contains(cmd: &Cmd, predicate: fn(&Cmd) -> bool) -> bool {
    predicate(cmd)
        || matches!(cmd, Cmd::Batch(cmds) if cmds.iter().any(|cmd| contains(cmd, predicate)))
}

#[test]
fn lsp_settings_master_reuses_live_effect_and_noops_on_same_choice() {
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    let index = row(&model, "Language servers");
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: index,
            choice: 0,
        },
    )
    .unwrap();
    assert!(!saved(&cmd).unwrap().lsp.enabled);
    assert!(contains(&cmd, |cmd| matches!(
        cmd,
        Cmd::LspSetEnabled { enabled: false }
    )));
    assert!(contains(&cmd, |cmd| matches!(cmd, Cmd::Redraw)));
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: index,
            choice: 0,
        },
    )
    .unwrap();
    assert!(saved(&cmd).is_none());
    assert!(!contains(&cmd, |cmd| matches!(
        cmd,
        Cmd::LspSetEnabled { .. }
    )));
    let cmd = modal(&mut model, ModalMsg::MoveCursorRight).unwrap();
    assert!(saved(&cmd).unwrap().lsp.enabled);
    assert!(contains(&cmd, |cmd| matches!(
        cmd,
        Cmd::LspSetEnabled { enabled: true }
    )));
}

#[test]
fn lsp_settings_filtered_server_toggle_preserves_other_overrides() {
    let mut model = test_model("text", 0, 0);
    model.config.lsp.enabled = false;
    let override_config = token::config::LspServerOverride {
        command: Some("/custom/rust-analyzer".into()),
        args: Some(vec!["--quiet".into()]),
        initialization_options: Some(serde_json::json!({"test": true})),
        settings: Some(serde_json::json!({"cargo": {"features": "all"}})),
        ..Default::default()
    };
    model
        .config
        .lsp
        .servers
        .insert("rust-analyzer".into(), override_config.clone());
    open(&mut model);
    modal(&mut model, ModalMsg::SetInput("rust-analyzer".into()));
    let index = row(&model, "rust-analyzer enabled");
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: index,
            choice: 1,
        },
    )
    .unwrap();
    assert!(
        saved(&cmd).is_none(),
        "absent enabled override defaults to on, even with master off"
    );
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: index,
            choice: 0,
        },
    )
    .unwrap();
    let config = saved(&cmd).unwrap();
    assert!(!config.lsp.enabled);
    let server = &config.lsp.servers["rust-analyzer"];
    assert_eq!(server.enabled, Some(false));
    assert_eq!(server.command, override_config.command);
    assert_eq!(server.args, override_config.args);
    assert_eq!(
        server.initialization_options,
        override_config.initialization_options
    );
    assert_eq!(server.settings, override_config.settings);
    assert!(contains(
        &cmd,
        |cmd| matches!(cmd, Cmd::LspSetServerEnabled { server_id, enabled: false } if server_id.to_string() == "rust-analyzer")
    ));
}

#[test]
fn lsp_settings_status_is_read_only_and_configure_opens_a_draft_form() {
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    modal(&mut model, ModalMsg::SetInput("rust-analyzer".into()));
    let before = serde_yaml::to_value(&model.config).unwrap();
    {
        let index = row(&model, "rust-analyzer status");
        let cmd = modal(&mut model, ModalMsg::ActivateRow(index)).unwrap();
        assert!(matches!(cmd, Cmd::Redraw));
        for message in [
            ModalMsg::MoveCursorLeft,
            ModalMsg::MoveCursorRight,
            ModalMsg::Confirm,
            ModalMsg::ChooseSetting {
                row: index,
                choice: 0,
            },
        ] {
            assert!(modal(&mut model, message).is_none());
        }
    }
    assert_eq!(serde_yaml::to_value(&model.config).unwrap(), before);
    let index = row(&model, "rust-analyzer executable");
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: index,
            choice: 0,
        },
    )
    .unwrap();
    assert!(
        matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.editing_field())
    );
    assert!(contains(
        &cmd,
        |cmd| matches!(cmd, Cmd::InspectSettingsExecutable { command, .. } if command == "rust-analyzer")
    ));
    assert!(saved(&cmd).is_none());
    modal(&mut model, ModalMsg::SetInput("/draft/server".into()));
    let actions = row(&model, "Configuration");
    modal(&mut model, ModalMsg::ActivateRow(actions));
    for message in [ModalMsg::MoveCursorLeft, ModalMsg::MoveCursorRight] {
        let cmd = modal(&mut model, message).unwrap();
        assert!(pending_form(&cmd).is_none());
        assert!(matches!(
            &model.ui.active_modal,
            Some(ModalState::Settings(_))
        ));
    }
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: actions,
            choice: 2,
        },
    )
    .unwrap();
    assert!(
        model.ui.active_modal.is_none(),
        "the log must not open behind Settings"
    );
    assert!(contains(&cmd, |cmd| matches!(
        cmd,
        Cmd::PrepareFileOpen { .. }
    )));
    open(&mut model);
    let executable = row(&model, "Executable");
    modal(&mut model, ModalMsg::ActivateRow(executable));
    assert!(
        matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.input() == "/draft/server")
    );
    modal(&mut model, ModalMsg::Close);
    assert!(
        matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if !state.editing_field())
    );
    assert_eq!(serde_yaml::to_value(&model.config).unwrap(), before);
}

fn pending_form(
    cmd: &Cmd,
) -> Option<(
    std::sync::Arc<()>,
    Box<token::settings::forms::SettingsChange>,
)> {
    match cmd {
        Cmd::ApplySettingsForm { session, change } => Some((session.clone(), change.clone())),
        Cmd::Batch(commands) => commands.iter().find_map(pending_form),
        _ => None,
    }
}

fn form_input(model: &mut AppModel, name: &str, value: &str) {
    let index = row(model, name);
    modal(model, ModalMsg::ActivateRow(index));
    modal(model, ModalMsg::SetInput(value.into()));
}

#[test]
fn settings_adds_a_custom_server_without_overwriting_existing_configuration() {
    use token::messages::SettingsMsg;
    let mut model = test_model("text", 0, 0);
    open(&mut model);
    let add = row(&model, "Custom language server");
    modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: add,
            choice: 0,
        },
    );
    form_input(&mut model, "Server ID", "rust-analyzer");
    form_input(&mut model, "Executable", "/installed/clangd");
    form_input(
        &mut model,
        "Arguments (JSON / YAML list)",
        "[--background-index]",
    );
    form_input(&mut model, "Languages", "C, C++");
    form_input(
        &mut model,
        "Root markers (JSON / YAML list)",
        "[compile_commands.json, .git]",
    );
    let actions = row(&model, "Configuration");
    let apply = ModalMsg::ChooseSetting {
        row: actions,
        choice: 0,
    };
    assert!(
        pending_form(&modal(&mut model, apply.clone()).unwrap()).is_none(),
        "a built-in ID must not be overwritten"
    );
    form_input(&mut model, "Server ID", "clangd");
    form_input(&mut model, "Languages", "not-a-language");
    assert!(pending_form(&modal(&mut model, apply.clone()).unwrap()).is_none());
    form_input(&mut model, "Languages", "C, C++");
    let (session, change) = pending_form(&modal(&mut model, apply).unwrap()).unwrap();
    assert!(!model.config.lsp.servers.contains_key("clangd"));
    update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::FormApplied {
            session,
            change,
            result: Ok(()),
        })),
    );
    let server = &model.config.lsp.servers["clangd"];
    assert_eq!(server.command.as_deref(), Some("/installed/clangd"));
    assert_eq!(
        server.languages.as_deref(),
        Some(&[token::syntax::LanguageId::C, token::syntax::LanguageId::Cpp][..])
    );
    assert_eq!(
        server.root_markers.as_deref(),
        Some(&["compile_commands.json".into(), ".git".into()][..])
    );
    modal(&mut model, ModalMsg::Close);
    let configure = row(&model, "clangd executable");
    modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: configure,
            choice: 0,
        },
    );
    let executable = row(&model, "Executable");
    modal(&mut model, ModalMsg::ActivateRow(executable));
    assert!(
        matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.input() == "/installed/clangd")
    );
}

#[test]
fn settings_server_form_validates_and_applies_only_after_save_success() {
    use token::messages::SettingsMsg;
    let mut model = test_model("text", 0, 0);
    model
        .config
        .lsp
        .servers
        .entry("gopls".into())
        .or_default()
        .command = Some("/keep/gopls".into());
    let original = serde_yaml::to_value(&model.config).unwrap();
    open(&mut model);
    modal(
        &mut model,
        ModalMsg::SetInput("rust-analyzer executable".into()),
    );
    modal(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 0 });
    form_input(&mut model, "Executable", "/path with spaces/rust-analyzer");
    // Ordinary cursor movement must edit the field, not activate another setting.
    modal(&mut model, ModalMsg::MoveCursorLeft);
    form_input(
        &mut model,
        "Arguments (JSON / YAML list)",
        r#"["--quiet", "", "a\nb"]"#,
    );
    form_input(&mut model, "Initialization options (JSON / YAML)", "{");
    let actions = row(&model, "Configuration");
    let invalid = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: actions,
            choice: 0,
        },
    )
    .unwrap();
    assert!(pending_form(&invalid).is_none());
    assert_eq!(serde_yaml::to_value(&model.config).unwrap(), original);
    form_input(&mut model, "Initialization options (JSON / YAML)", "");
    modal(
        &mut model,
        ModalMsg::PasteText("check:\r\n  command: clippy\r\n".into()),
    );
    for message in [ModalMsg::PageUp, ModalMsg::PageDown] {
        modal(&mut model, message);
        assert!(
            matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.input() == "check:\n  command: clippy\n" && state.editing_field())
        );
    }
    form_input(&mut model, "Server settings (JSON / YAML object)", "[1, 2]");
    assert!(pending_form(
        &modal(
            &mut model,
            ModalMsg::ChooseSetting {
                row: actions,
                choice: 0
            }
        )
        .unwrap()
    )
    .is_none());
    form_input(
        &mut model,
        "Server settings (JSON / YAML object)",
        "cargo:\n  features: all\n",
    );
    let cmd = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: actions,
            choice: 0,
        },
    )
    .unwrap();
    let (session, change) = pending_form(&cmd).unwrap();
    assert_eq!(serde_yaml::to_value(&model.config).unwrap(), original);
    let failed = update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::FormApplied {
            session: session.clone(),
            change: change.clone(),
            result: Err("read-only configuration".into()),
        })),
    )
    .unwrap();
    assert!(!contains(&failed, |cmd| matches!(
        cmd,
        Cmd::LspApplyConfiguration { .. }
    )));
    assert_eq!(serde_yaml::to_value(&model.config).unwrap(), original);
    let retry = modal(
        &mut model,
        ModalMsg::ChooseSetting {
            row: actions,
            choice: 0,
        },
    )
    .unwrap();
    assert!(pending_form(&retry).is_some());
    let applied = update(
        &mut model,
        Msg::Ui(UiMsg::Settings(SettingsMsg::FormApplied {
            session,
            change,
            result: Ok(()),
        })),
    )
    .unwrap();
    assert!(contains(
        &applied,
        |cmd| matches!(cmd, Cmd::LspApplyConfiguration { server_id } if server_id.to_string() == "rust-analyzer")
    ));
    let server = &model.config.lsp.servers["rust-analyzer"];
    assert_eq!(
        server.command.as_deref(),
        Some("/path with spaces/rust-analyzer")
    );
    assert_eq!(server.args.as_ref().unwrap(), &["--quiet", "", "a\nb"]);
    assert_eq!(
        server.initialization_options,
        Some(serde_json::json!({"check":{"command":"clippy"}}))
    );
    assert_eq!(
        server.settings,
        Some(serde_json::json!({"cargo":{"features":"all"}}))
    );
    assert_eq!(
        model.config.lsp.servers["gopls"].command.as_deref(),
        Some("/keep/gopls")
    );
}
