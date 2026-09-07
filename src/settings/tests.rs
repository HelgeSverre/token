use crate::commands::Cmd;
use crate::config::EditorConfig;
use crate::messages::{LspMsg, ModalMsg, Msg, UiMsg};
use crate::model::{AppModel, ModalId, ModalState};
use crate::settings::{descriptors::DESCRIPTORS, SettingsRow};
use crate::update::update;
use crate::view::overlay_surface::{self, Accessory, Body, OverlayHit};

fn model() -> AppModel {
    let mut model = AppModel::new(1000, 800, 1.0, vec![]);
    model.config = EditorConfig::default();
    update(&mut model, Msg::Ui(UiMsg::ToggleModal(ModalId::Settings)));
    model
}

#[test]
fn category_navigation_filters_the_form_without_writing_configuration() {
    let mut model = model();
    let index = crate::settings::categories()
        .iter()
        .position(|category| *category == Some("Editor"))
        .unwrap();
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    crate::view::modal::with_settings_spec(&model, state, |spec| {
        let layout = overlay_surface::layout(spec, 1000, 800, 1.0);
        let category = layout.tab_rects[index];
        assert_eq!(
            overlay_surface::hit_test(
                spec,
                &layout,
                category.x + category.w / 2,
                category.y + category.h / 2
            ),
            OverlayHit::Tab(index)
        );
        assert!(category.x < layout.rows[0].x);
    });
    let cmd = update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::ActivateTab(index))),
    )
    .unwrap();
    assert!(saved_config(&cmd).is_none());
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    assert!(!state.rows.is_empty());
    assert!(state.rows.iter().all(|row| row.section() == "Editor"));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::NextTab)));
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    assert_eq!(state.category, index + 1);
}

#[test]
fn keyboard_navigation_keeps_settings_visible_at_each_window_size() {
    for (width, height, scale) in [
        (1000, 800, 1.0),
        (1600, 1100, 2.0),
        (400, 750, 1.0),
        (1600, 600, 2.0),
        (800, 600, 2.0),
        (400, 300, 1.0),
    ] {
        let mut model = model();
        model.window_size = (width, height);
        model.metrics.scale_factor = scale;
        for action in [
            ModalMsg::ActivateTab(0),
            ModalMsg::SelectPrevious,
            ModalMsg::SelectNext,
            ModalMsg::PageDown,
            ModalMsg::PageUp,
        ]
        .into_iter()
        .chain(std::iter::repeat_with(|| ModalMsg::SelectNext).take(80))
        {
            update(&mut model, Msg::Ui(UiMsg::Modal(action)));
            let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
                panic!()
            };
            crate::view::modal::with_settings_spec(&model, state, |spec| {
                let layout = overlay_surface::layout(spec, width as usize, height as usize, scale);
                for (index, tab) in layout.tab_rects.iter().enumerate() {
                    assert!(tab.y + tab.h <= layout.footer.unwrap().y);
                    assert_eq!(
                        overlay_surface::hit_test(
                            spec,
                            &layout,
                            tab.x + tab.w / 2,
                            tab.y + tab.h / 2
                        ),
                        OverlayHit::Tab(index)
                    );
                }
                assert!(layout
                    .rows
                    .iter()
                    .all(|r| r.y + r.h <= layout.footer.unwrap().y));
                assert!(
                    layout.rows.iter().any(|r| overlay_surface::hit_test(
                        spec,
                        &layout,
                        r.x + 1,
                        r.y + 1
                    ) == OverlayHit::Row(overlay_surface::FlatIndex(
                        state.selected_index
                    ))),
                    "selected row {} is not visible",
                    state.selected_index
                );
            });
        }
    }
}

#[test]
fn boolean_switch_hit_commits_the_opposite_value() {
    let mut model = model();
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("Auto Surround".into()))),
    );
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    let hit = crate::view::modal::with_settings_spec(&model, state, |spec| {
        let layout = overlay_surface::layout(spec, 1000, 800, 1.0);
        layout
            .rows
            .iter()
            .find_map(|r| {
                let hit = overlay_surface::hit_test(spec, &layout, r.x + r.w - 17, r.y + 20);
                matches!(hit, OverlayHit::Choice { .. }).then_some(hit)
            })
            .expect("painted switch has a click target")
    });
    let OverlayHit::Choice { row, choice } = hit else {
        panic!()
    };
    let previous = model.config.auto_surround;
    let cmd = update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SelectSettingChoice {
            row: row.0,
            choice,
        })),
    )
    .unwrap();
    assert_eq!(model.config.auto_surround, !previous);
    assert_eq!(saved_config(&cmd).unwrap().auto_surround, !previous);
}

fn saved_config(cmd: &Cmd) -> Option<&EditorConfig> {
    match cmd {
        Cmd::SaveConfiguration { config } => Some(config),
        Cmd::Batch(cmds) => cmds.iter().find_map(saved_config),
        _ => None,
    }
}

#[test]
fn filtered_view_order_is_the_order_committed_by_chip_input() {
    let mut model = model();
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("on".into()))),
    );
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    assert!(state.sections().len() > 1);
    let rows = state.rows.clone();
    crate::view::modal::with_settings_spec(&model, state, |spec| {
        let Body::List { sections, .. } = &spec.body else {
            panic!()
        };
        assert_eq!(
            sections
                .iter()
                .flat_map(|section| section.rows.iter())
                .map(|row| row.label)
                .collect::<Vec<_>>(),
            rows.iter().map(|row| row.label()).collect::<Vec<_>>()
        );
        assert!(sections.iter().all(|section| !section.rows.is_empty()));
    });
    for (row, setting) in rows.iter().enumerate() {
        let SettingsRow::Preset(index) = setting else {
            continue;
        };
        let descriptor = &DESCRIPTORS[*index];
        let choice =
            (descriptor.active_choice(&model.config).unwrap_or(0) + 1) % descriptor.choices.len();
        let cmd = update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SelectSettingChoice { row, choice })),
        )
        .unwrap();
        assert_eq!(
            (descriptor.read)(&model.config),
            descriptor.choices[choice].1
        );
        assert_eq!(
            (descriptor.read)(saved_config(&cmd).expect("immediate save command")),
            descriptor.choices[choice].1
        );
        assert!(matches!(
            model.ui.active_modal,
            Some(ModalState::Settings(_))
        ));
    }
}

#[test]
fn opening_searching_and_closing_preserve_off_preset_values() {
    let mut model = model();
    model.config.cursor_blink_ms = 777;
    model.config.status_bar_font_size = 12.5;
    let before = serde_yaml::to_string(&model.config).unwrap();
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("blink".into()))),
    );
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    crate::view::modal::with_settings_spec(&model, state, |spec| {
        let Body::List { sections, .. } = &spec.body else {
            panic!()
        };
        assert!(matches!(
            sections[0].rows[0].accessory,
            Accessory::Choices { active: None, .. }
        ));
    });
    let cmd = update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Close))).unwrap();
    assert!(saved_config(&cmd).is_none());
    assert_eq!(serde_yaml::to_string(&model.config).unwrap(), before);
}

#[test]
fn visible_chips_are_clickable_at_multiple_scales_and_widths() {
    for (width, scale) in [(1000, 1.0), (1600, 2.0), (360, 1.0)] {
        let mut model = model();
        model.window_size = (width, 800);
        model.metrics.scale_factor = scale;
        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("blink".into()))),
        );
        let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!()
        };
        crate::view::modal::with_settings_spec(&model, state, |spec| {
            let layout = overlay_surface::layout(spec, width as usize, 800, scale);
            let mut found = std::collections::BTreeSet::new();
            for row in &layout.rows {
                for y in (row.y..row.y + row.h).step_by(4) {
                    for x in (row.x..row.x + row.w).step_by(2) {
                        if let OverlayHit::Choice { row, choice } =
                            overlay_surface::hit_test(spec, &layout, x, y)
                        {
                            if row.0 == 0 {
                                found.insert(choice);
                            }
                        }
                    }
                }
            }
            assert_eq!(found, [0, 1, 2, 3].into_iter().collect());
        });
    }
}

#[test]
fn lsp_status_transition_redraws_the_open_settings_modal() {
    let mut model = model();
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput(
            "rust-analyzer status".into(),
        ))),
    );
    for (status, label) in [
        (crate::lsp::ServerState::Starting, "Starting"),
        (crate::lsp::ServerState::Ready, "Ready"),
    ] {
        let cmd = update(
            &mut model,
            Msg::Lsp(LspMsg::ServerStateChanged {
                server_id: "rust-analyzer".into(),
                root: "/workspace".into(),
                state: status,
            }),
        )
        .unwrap();
        assert!(matches!(cmd.damage(), crate::commands::Damage::Full));
        let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!()
        };
        crate::view::modal::with_settings_spec(&model, state, |spec| {
            let Body::List { sections, .. } = &spec.body else {
                panic!()
            };
            assert!(sections
                .iter()
                .flat_map(|s| s.rows.iter())
                .any(|row| matches!(row.accessory, Accessory::DimText(text) if text == label)));
        });
    }
}

#[test]
fn server_overrides_preserve_commands_and_read_only_rows_never_save() {
    let mut model = model();
    model.config.lsp.servers.insert(
        "pyright".into(),
        crate::config::LspServerOverride {
            command: Some("/custom/bin/pyright".into()),
            ..Default::default()
        },
    );
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("pyright".into()))),
    );
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        panic!()
    };
    let rows = state.rows.clone();
    let enabled = rows
        .iter()
        .position(|row| matches!(row, SettingsRow::ServerEnabled(def) if def.id == "pyright"))
        .unwrap();
    let cmd = update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SelectSettingChoice {
            row: enabled,
            choice: 0,
        })),
    )
    .unwrap();
    let saved = saved_config(&cmd).unwrap();
    assert_eq!(saved.lsp.servers["pyright"].enabled, Some(false));
    assert_eq!(
        saved.lsp.servers["pyright"].command.as_deref(),
        Some("/custom/bin/pyright")
    );
    for (row, setting) in rows.iter().enumerate() {
        if matches!(
            setting,
            SettingsRow::ServerCommand(_) | SettingsRow::ServerStatus(_)
        ) {
            let cmd = update(
                &mut model,
                Msg::Ui(UiMsg::Modal(ModalMsg::SelectSettingChoice {
                    row,
                    choice: 1,
                })),
            )
            .unwrap();
            assert!(saved_config(&cmd).is_none());
        }
    }
}

#[test]
fn settings_keybinding_and_palette_command_are_registered() {
    let bindings =
        crate::keymap::parse_keymap_yaml(crate::keymap::get_default_keymap_yaml()).unwrap();
    assert!(bindings
        .iter()
        .any(|binding| binding.command == crate::keymap::Command::OpenSettings));
    let def = crate::commands::all_commands()
        .into_iter()
        .find(|def| def.id == crate::commands::CommandId::OpenSettings)
        .unwrap();
    assert_eq!(def.label, "Open Settings");
    assert!(crate::keymap::Command::OpenSettings.is_global());
    let mut model = AppModel::new(1000, 800, 1.0, vec![]);
    for message in crate::keymap::Command::OpenSettings.to_msgs() {
        update(&mut model, message);
    }
    assert!(matches!(
        model.ui.active_modal,
        Some(ModalState::Settings(_))
    ));
}
