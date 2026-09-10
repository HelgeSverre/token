//! Mouse event handling using the unified hit-test system
//!
//! This module provides centralized mouse event dispatch that:
//! - Uses `hit_test_ui()` to determine the target under the cursor
//! - Dispatches behavior based on (target, button, click_count)
//! - Handles focus changes consistently
//! - Shares hit-testing logic across left/middle/right clicks

use std::time::{Duration, Instant};

use winit::event::MouseButton;
use winit::keyboard::ModifiersState;

use token::commands::Cmd;
use token::messages::{
    CompletionMsg, CsvMsg, EditorMsg, ImageMsg, LayoutMsg, ModalMsg, Msg, OutlineMsg, PreviewMsg,
    TerminalMsg, UiMsg, WorkspaceMsg,
};
use token::model::ui::{ScrollbarDragAxis, ScrollbarDragState, ScrollbarTarget};
use token::model::AppModel;
use token::panel::DockPosition;
use token::update::update;
use token::util::visible_tree_row_at_index;

use token::layout::editor::EditorTabBarLayout;
use token::model::editor_area::GroupId;
use token::view::hit_test::{hit_test_ui, EventResult, HitTarget, MouseEvent};
use token::view::Renderer;

pub(super) fn terminal_link_modifier(modifiers: ModifiersState) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.super_key() && !modifiers.control_key() && !modifiers.alt_key()
    } else {
        modifiers.control_key() && !modifiers.super_key() && !modifiers.alt_key()
    }
}

fn terminal_link_at_pointer(
    model: &AppModel,
    target: Option<&HitTarget>,
    x: f64,
    y: f64,
    modifiers: ModifiersState,
) -> Option<(usize, token::terminal::TerminalLink)> {
    if !terminal_link_modifier(modifiers)
        || model.terminal.selection_drag.is_some()
        || !matches!(
            target,
            Some(HitTarget::DockContent {
                active_panel_id: token::panel::PanelId::Terminal,
                ..
            })
        )
    {
        return None;
    }
    let viewport = token::panels::terminal::TerminalViewport::for_model(model)?;
    let session = model.terminal.active_session()?;
    session
        .link_at(viewport.point_inside(x, y)?)
        .map(|link| (session.id, link))
}

pub(super) fn update_terminal_link_hover(
    model: &mut AppModel,
    target: Option<&HitTarget>,
    x: f64,
    y: f64,
    modifiers: ModifiersState,
) -> bool {
    let hovered = terminal_link_at_pointer(model, target, x, y, modifiers);
    if model.terminal.hovered_link == hovered {
        return false;
    }
    model.terminal.hovered_link = hovered;
    true
}

/// Track pointer rows using the same flat indices as painting and activation.
/// Returns whether row highlights changed, so idle popup hover requests repaint.
pub(super) fn update_hover_target(model: &mut AppModel, target: Option<&HitTarget>) -> bool {
    let previous_find = match model.ui.hover {
        token::model::HoverRegion::FindBar(control) => control,
        _ => None,
    };
    let previous_modal = model.ui.modal_hover_row;
    let previous_terminal = model.terminal.hovered_tab;
    model.terminal.hovered_tab = match target {
        Some(HitTarget::TerminalAction { action, .. }) => Some(*action),
        _ => None,
    };
    let previous_popup = model
        .ui
        .cursor_overlay
        .and_then(|overlay| overlay.hover_row);
    model.ui.hover = target.map_or(token::model::HoverRegion::None, HitTarget::hover_region);
    model.ui.modal_hover_row = match target {
        Some(HitTarget::ModalRow { flat_index } | HitTarget::ModalChoice { flat_index, .. }) => {
            Some(*flat_index)
        }
        _ => None,
    };
    if let Some(overlay) = &mut model.ui.cursor_overlay {
        overlay.hover_row = match target {
            Some(HitTarget::CursorOverlay { flat_index }) => *flat_index,
            _ => None,
        };
    }
    previous_find
        != match model.ui.hover {
            token::model::HoverRegion::FindBar(control) => control,
            _ => None,
        }
        || previous_terminal != model.terminal.hovered_tab
        || previous_modal != model.ui.modal_hover_row
        || previous_popup
            != model
                .ui
                .cursor_overlay
                .and_then(|overlay| overlay.hover_row)
}

/// Identifies what was clicked, so rapid clicks on unrelated targets
/// (e.g. a sidebar row then an editor line) never count as double-clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickRegion {
    FindField(token::model::FindReplaceField),
    Terminal {
        session: usize,
        point: alacritty_terminal::index::Point,
    },
    Editor {
        group: token::model::editor_area::GroupId,
        line: usize,
        column: usize,
    },
    Sidebar {
        row: usize,
    },
    Outline {
        row: usize,
    },
    Problems {
        row: usize,
    },
    Usages {
        row: usize,
    },
    BinaryPlaceholder {
        group: token::model::editor_area::GroupId,
    },
    /// A CSV data cell — repeat presses on the same cell count up for
    /// double-click-to-edit / word select.
    CsvCell {
        group: token::model::editor_area::GroupId,
        row: usize,
        col: usize,
    },
}

/// Click tracking state for double/triple click detection
pub struct ClickTracker {
    pub last_click_time: Instant,
    pub last_click_region: Option<ClickRegion>,
    pub click_count: u32,
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self {
            last_click_time: Instant::now() - Duration::from_secs(10),
            last_click_region: None,
            click_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    fn settings_scrollbar(model: &AppModel) -> token::view::scrollbar::ScrollbarGeometry {
        let pt = token::view::hit_test::Point::new(
            model.window_size.0 as f64 - 22.0,
            model.window_size.1 as f64 / 2.0,
        );
        let Some(HitTarget::ModalScrollbar { geometry }) =
            token::view::hit_test::hit_test_modal(model, pt)
        else {
            panic!("Settings must expose its painted track as a scrollbar target");
        };
        geometry
    }

    fn grab_settings_scrollbar(model: &mut AppModel) -> MouseEvent {
        let bar = settings_scrollbar(model);
        let event = MouseEvent::new(
            (bar.thumb_rect.x + 6.0) as f64,
            (bar.thumb_rect.y + bar.thumb_rect.height / 3.0) as f64,
            MouseButton::Left,
            ModifiersState::empty(),
        );
        assert!(matches!(
            modal_scrollbar_press(model, &bar, &event),
            EventResult::Consumed { focus: None, .. }
        ));
        assert!(model.ui.scrollbar_drag.is_some());
        event
    }

    #[test]
    fn settings_scrollbar_pointer_drag_and_track_click_reach_both_ends() {
        use token::model::{ModalId, ModalState};
        for width in [400, 800] {
            let mut model = AppModel::new(width, 750, 1.0);
            model
                .ui
                .open_modal(ModalState::Settings(Default::default()));
            let first = settings_scrollbar(&model);
            let event = grab_settings_scrollbar(&mut model);
            assert!(matches!(
                model.ui.scrollbar_drag.as_ref().unwrap().target,
                ScrollbarTarget::Modal(ModalId::Settings)
            ));
            update(
                &mut model,
                Msg::Ui(UiMsg::ScrollbarDragUpdate {
                    mouse_coord: event.pos.y as f32,
                }),
            );
            assert_eq!(
                settings_scrollbar(&model).state.position,
                0,
                "grabbing must not jump"
            );
            update(
                &mut model,
                Msg::Ui(UiMsg::ScrollbarDragUpdate {
                    mouse_coord: 5000.0,
                }),
            );
            let bottom = settings_scrollbar(&model);
            assert_eq!(bottom.state.position, bottom.state.max_position());
            update(
                &mut model,
                Msg::Ui(UiMsg::ScrollbarDragUpdate {
                    mouse_coord: -500.0,
                }),
            );
            assert_eq!(settings_scrollbar(&model).state.position, 0);
            update(&mut model, Msg::Ui(UiMsg::ScrollbarDragEnd));
            assert!(model.ui.scrollbar_drag.is_none());
            let event = MouseEvent::new(
                (first.track_rect.x + 6.0) as f64,
                (first.track_rect.y + first.track_rect.height - 1.0) as f64,
                MouseButton::Left,
                ModifiersState::empty(),
            );
            modal_scrollbar_press(&mut model, &first, &event);
            let bottom = settings_scrollbar(&model);
            assert_eq!(bottom.state.position, bottom.state.max_position());
            let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
                panic!("page closed");
            };
            assert_eq!(
                state.selected_index(),
                0,
                "scrolling must not change selection"
            );
            assert_eq!(
                model.editor().viewport.top_line,
                0,
                "scrolling Settings must not scroll the editor"
            );
        }
    }

    #[test]
    fn settings_scrollbar_capture_is_cancelled_by_page_changes_and_resize() {
        use token::model::ModalState;
        for action in [
            ModalMsg::SetInput("theme".into()),
            ModalMsg::NextTab,
            ModalMsg::Close,
        ] {
            let mut model = AppModel::new(800, 750, 1.0);
            model
                .ui
                .open_modal(ModalState::Settings(Default::default()));
            grab_settings_scrollbar(&mut model);
            update(&mut model, Msg::Ui(UiMsg::Modal(action)));
            assert!(model.ui.scrollbar_drag.is_none());
            assert!(update(
                &mut model,
                Msg::Ui(UiMsg::ScrollbarDragUpdate {
                    mouse_coord: 5000.0
                })
            )
            .is_none());
        }
        let mut model = AppModel::new(800, 750, 1.0);
        model
            .ui
            .open_modal(ModalState::Settings(Default::default()));
        grab_settings_scrollbar(&mut model);
        model.resize(400, 750);
        assert!(model.ui.scrollbar_drag.is_none());
    }

    #[test]
    fn settings_scrollbar_wheel_preserves_scroll_delta_magnitude() {
        let mut model = AppModel::new(800, 750, 1.0);
        model
            .ui
            .open_modal(token::model::ModalState::Settings(Default::default()));
        model.ui.hover = token::model::HoverRegion::Modal;
        scroll_hovered_region(&mut model, None, (0, 1).into());
        let one = settings_scrollbar(&model).state.position;
        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput(String::new()))),
        );
        scroll_hovered_region(&mut model, None, (0, 5).into());
        assert_eq!(one, 1);
        assert_eq!(settings_scrollbar(&model).state.position, 5);
    }

    #[test]
    fn settings_scrollbar_does_not_snap_on_drag_or_visible_selection() {
        let mut model = AppModel::new(800, 750, 1.0);
        model
            .ui
            .open_modal(token::model::ModalState::Settings(Default::default()));
        update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Scroll(13))));
        assert_eq!(settings_scrollbar(&model).state.position, 13);
        update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
        assert_eq!(settings_scrollbar(&model).state.position, 13);
        let event = grab_settings_scrollbar(&mut model);
        update(
            &mut model,
            Msg::Ui(UiMsg::ScrollbarDragUpdate {
                mouse_coord: event.pos.y as f32,
            }),
        );
        assert_eq!(settings_scrollbar(&model).state.position, 13);
        let coord = event.pos.y as f32 + 1.0;
        let expected = model
            .ui
            .scrollbar_drag
            .as_ref()
            .unwrap()
            .position_from_mouse(coord);
        update(
            &mut model,
            Msg::Ui(UiMsg::ScrollbarDragUpdate { mouse_coord: coord }),
        );
        assert_eq!(settings_scrollbar(&model).state.position, expected);
        assert!(
            expected > 13 && expected < 72,
            "a one-pixel drag must not jump a section"
        );
        update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::PageDown)));
        assert!(settings_scrollbar(&model).state.position > expected);
    }

    #[test]
    fn modal_pointer_row_preserves_palette_command_effect() {
        use token::model::ModalId;
        let mut model = AppModel::new(800, 600, 1.0);
        update(
            &mut model,
            Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)),
        );
        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("Quit".into()))),
        );
        let EventResult::Consumed {
            cmd: Some(Cmd::Batch(commands)),
            focus: None,
            ..
        } = modal_press(&mut model, ModalMsg::ActivateRow(0))
        else {
            panic!("palette action effect lost")
        };
        assert!(commands.iter().any(|command| matches!(command, Cmd::Quit)));
        assert!(commands
            .iter()
            .any(|command| matches!(command, Cmd::SaveCommandHistory { .. })));
        assert!(model.ui.active_modal.is_none());
    }

    #[test]
    fn settings_keymap_modal_pointer_preserves_load_and_save_effects() {
        use token::keymap::preferences::{parse_sequence, KeymapSnapshot};
        use token::messages::SettingsMsg;
        use token::model::{ModalId, ModalState};
        fn keymap_effect(command: &Cmd, saving: bool) -> bool {
            match command {
                Cmd::PrepareKeymap { save, .. } => save.is_some() == saving,
                Cmd::Batch(commands) => commands
                    .iter()
                    .any(|command| keymap_effect(command, saving)),
                _ => false,
            }
        }
        let mut model = AppModel::new(800, 600, 1.0);
        update(&mut model, Msg::Ui(UiMsg::ToggleModal(ModalId::Settings)));
        let EventResult::Consumed {
            cmd: Some(command), ..
        } = modal_press(
            &mut model,
            ModalMsg::ActivateTab(token::settings::categories().len() - 1),
        )
        else {
            panic!("load effect lost")
        };
        assert!(keymap_effect(&command, false));
        let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
            panic!("settings")
        };
        let session = state.keymap.session.clone();
        update(
            &mut model,
            Msg::Ui(UiMsg::Settings(SettingsMsg::KeymapResult {
                session,
                saved: false,
                result: Ok(Box::new(KeymapSnapshot::parse(None).unwrap())),
            })),
        );
        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("SaveFile".into()))),
        );
        modal_press(&mut model, ModalMsg::ActivateRow(0));
        update(
            &mut model,
            Msg::Ui(UiMsg::Settings(SettingsMsg::CaptureKey(
                parse_sequence("ctrl+f24").unwrap()[0],
            ))),
        );
        let EventResult::Consumed {
            cmd: Some(command), ..
        } = modal_press(&mut model, ModalMsg::ChooseSetting { row: 0, choice: 0 })
        else {
            panic!("save effect lost")
        };
        assert!(keymap_effect(&command, true));
    }

    #[test]
    fn modal_pointer_close_preserves_theme_restore_effect() {
        use token::model::ModalId;
        let mut model = AppModel::new(800, 600, 1.0);
        update(
            &mut model,
            Msg::Ui(UiMsg::ToggleModal(ModalId::ThemePicker)),
        );
        let EventResult::Consumed {
            cmd: Some(Cmd::Batch(commands)),
            focus: None,
            ..
        } = modal_press(&mut model, ModalMsg::Close)
        else {
            panic!("restore effect lost")
        };
        assert!(commands
            .iter()
            .any(|command| matches!(command, Cmd::LoadTheme { persist: false, .. })));
        assert!(model.ui.active_modal.is_none());
    }

    fn documentation_model() -> AppModel {
        use token::completion::menu::CompletionMenuState;
        use token::model::{Cursor, CursorOverlayKind, CursorOverlayState};
        let mut model = AppModel::new(1000, 600, 1.0);
        model.config.lsp.enabled = false;
        model.document_mut().buffer = "va\n".into();
        model.editor_mut().cursors[0] = Cursor::at(0, 2);
        model.editor_mut().clear_selection();
        model.resize(1000, 600);
        let docs = (0..60)
            .map(|i| format!("documentation line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let items = token::completion::lsp::items_to_menu_items(
            ["value_a", "value_b"]
                .into_iter()
                .map(|label| lsp_types::CompletionItem {
                    label: label.into(),
                    documentation: Some(lsp_types::Documentation::String(docs.clone())),
                    ..Default::default()
                })
                .collect(),
            &token::lsp::LspServerId::from("fixture"),
            std::path::Path::new("/tmp/fixture"),
            None,
        );
        model.ui.completion_menu = Some(CompletionMenuState {
            document_id: model.document().id.unwrap(),
            revision: model.document().revision,
            query_start: Cursor::at(0, 0),
            query: "va".into(),
            items,
            filtered: vec![(0, 0, vec![]), (0, 1, vec![])],
            context: Default::default(),
            selection_changed: false,
            is_incomplete: false,
            pending_resolve: None,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));
        model
    }

    #[test]
    fn documentation_wheel_scrollbar_and_expansion_do_not_accept_or_move_selection() {
        let mut model = documentation_model();
        let mut measure = token::view::overlay_surface::cell_measure(1.0);
        let layout = token::view::modal::with_cursor_overlay_spec(&model, |spec| {
            token::view::overlay_surface::layout_measured(spec, 1000, 600, 1.0, &mut measure)
        })
        .unwrap();
        let text = layout.docs_text.unwrap();
        let bar = layout.docs_scrollbar.unwrap();
        let cursors = model.editor().cursors.clone();
        // Deliberately stale hover: hit-test the actual card on this event.
        model.ui.hover = HoverRegion::EditorText;
        assert!(handle_mouse_wheel(
            &mut model,
            Some(((text.x + 1) as f64, (text.y + 1) as f64)),
            (0, 1).into(),
            Some(&mut measure)
        )
        .is_some());
        let state = model.ui.cursor_overlay.unwrap();
        assert_eq!(state.documentation.scroll, 3);
        assert_eq!((state.scroll, state.selected), (0, 0));
        assert_eq!(model.editor().cursors, cursors);
        assert_eq!(model.document().buffer.to_string(), "va\n");
        let target = token::view::hit_test::hit_test_cursor_overlay(
            &model,
            token::view::hit_test::Point::new(
                (bar.track_rect.x + 1.0) as f64,
                (bar.track_rect.y + 1.0) as f64,
            ),
            &mut measure,
        )
        .unwrap();
        assert!(matches!(
            target,
            HitTarget::CursorOverlayDocumentation {
                scrollbar: Some(_),
                ..
            }
        ));
        let dismissal = dismiss_overlay_for_press(&mut model, &target, MouseButton::Left);
        assert!(!dismissal.dismissed);
        let HitTarget::CursorOverlayDocumentation {
            scrollbar: Some(bar),
            ..
        } = target
        else {
            panic!("documentation track must expose the painted scrollbar");
        };
        let event = MouseEvent::new(
            (bar.thumb_rect.x + 1.0) as f64,
            (bar.thumb_rect.y + bar.thumb_rect.height / 2.0) as f64,
            MouseButton::Left,
            ModifiersState::empty(),
        );
        let target = ScrollbarTarget::Documentation {
            kind: state.kind,
            selected: state.selected,
        };
        overlay_scrollbar_press(&mut model, target, &bar, &event);
        assert!(model.ui.scrollbar_drag.is_some());
        update(
            &mut model,
            Msg::Ui(UiMsg::ScrollbarDragUpdate {
                mouse_coord: event.pos.y as f32,
            }),
        );
        assert_eq!(
            model.ui.cursor_overlay.unwrap().documentation.scroll,
            3,
            "grabbing must not jump"
        );
        for (mouse_coord, expected) in [(5000.0, bar.state.max_position()), (-500.0, 0)] {
            update(
                &mut model,
                Msg::Ui(UiMsg::ScrollbarDragUpdate { mouse_coord }),
            );
            assert_eq!(
                model.ui.cursor_overlay.unwrap().documentation.scroll,
                expected
            );
        }
        update(&mut model, Msg::Ui(UiMsg::ScrollbarDragEnd));
        assert!(model.ui.scrollbar_drag.is_none());
        let track_click = MouseEvent::new(
            (bar.track_rect.x + 1.0) as f64,
            (bar.track_rect.y + bar.track_rect.height - 1.0) as f64,
            MouseButton::Left,
            ModifiersState::empty(),
        );
        overlay_scrollbar_press(&mut model, target, &bar, &track_click);
        assert_eq!(
            model.ui.cursor_overlay.unwrap().documentation.scroll,
            bar.state.max_position()
        );
        assert!(model.ui.scrollbar_drag.is_none());
        assert_eq!(model.editor().viewport.top_line, 0);
        assert_eq!(model.editor().cursors, cursors);
        assert_eq!(model.document().buffer.to_string(), "va\n");
        update(&mut model, Msg::Ui(UiMsg::ToggleDocumentation));
        assert!(model.ui.cursor_overlay.unwrap().documentation.expanded);
        update(&mut model, Msg::Completion(CompletionMsg::MenuNext));
        let state = model.ui.cursor_overlay.unwrap();
        assert_eq!(state.selected, 1);
        assert_eq!(state.documentation.scroll, 0);
        assert!(!state.documentation.expanded);
        update(&mut model, Msg::Completion(CompletionMsg::Dismiss));
        assert!(update(&mut model, Msg::Ui(UiMsg::DocumentationScrolled(100))).is_none());
        assert!(model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn documentation_viewport_rechecks_hover_when_the_pointer_is_outside() {
        let mut model = documentation_model();
        model.ui.hover = HoverRegion::CursorOverlay;
        let mut measure = token::view::overlay_surface::cell_measure(1.0);
        let point = (50..1000)
            .step_by(50)
            .flat_map(|x| (100..500).step_by(100).map(move |y| (x as f64, y as f64)))
            .find(|&(x, y)| {
                matches!(
                    token::view::hit_test::hit_test_ui(
                        &model,
                        token::view::hit_test::Point::new(x, y),
                        model.char_width,
                        &mut measure
                    ),
                    Some(HitTarget::EditorContent { .. })
                )
            })
            .expect("editor content outside the card");
        handle_mouse_wheel(&mut model, Some(point), (0, 1).into(), Some(&mut measure));
        assert!(
            model.ui.completion_menu.is_none(),
            "scrolling the editor dismisses the attached menu"
        );
        assert_eq!(model.document().buffer.to_string(), "va\n");
    }

    #[test]
    fn documentation_viewport_noop_selection_and_focus_guards() {
        let mut model = documentation_model();
        model
            .ui
            .completion_menu
            .as_mut()
            .unwrap()
            .filtered
            .truncate(1);
        let overlay = model.ui.cursor_overlay.as_mut().unwrap();
        overlay.documentation.scroll = 8;
        overlay.documentation.expanded = true;
        update(&mut model, Msg::Completion(CompletionMsg::MenuNext));
        assert_eq!(model.ui.cursor_overlay.unwrap().documentation.scroll, 8);
        assert!(model.ui.cursor_overlay.unwrap().documentation.expanded);
        model
            .ui
            .open_modal(token::model::ModalState::GotoLine(Default::default()));
        // The outer update dismisses the hidden completion and may redraw.
        // Later documentation actions must not reopen it behind the modal.
        update(&mut model, Msg::Ui(UiMsg::ToggleDocumentation));
        assert!(model.ui.cursor_overlay.is_none());
        update(&mut model, Msg::Ui(UiMsg::DocumentationScrolled(0)));
        assert!(model.ui.cursor_overlay.is_none());
        assert!(model.ui.has_modal());
    }

    #[test]
    fn documentation_viewport_keyboard_routes_without_changing_the_buffer() {
        use winit::keyboard::{Key, NamedKey};
        let mut model = documentation_model();
        let cmd = crate::runtime::input::handle_cursor_overlay_key(
            &mut model,
            &Key::Named(NamedKey::PageDown),
            crate::runtime::input::KeyModifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert!(matches!(
            cmd,
            Some(Some(Cmd::PageDocumentation { forward: true }))
        ));
        crate::runtime::input::handle_cursor_overlay_key(
            &mut model,
            &Key::Named(NamedKey::F1),
            Default::default(),
        );
        assert!(model.ui.cursor_overlay.unwrap().documentation.expanded);
        assert_eq!(model.document().buffer.to_string(), "va\n");
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 0);
    }

    #[test]
    fn hover_documentation_controls_preserve_the_card_and_editor() {
        use token::model::{CursorOverlayKind, CursorOverlayState, HoverCardState};
        use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
        let mut model = documentation_model();
        let content = model
            .ui
            .completion_menu
            .as_ref()
            .unwrap()
            .selected_documentation(0)
            .unwrap()
            .clone();
        model.ui.completion_menu = None;
        model.ui.hover_card = Some(HoverCardState {
            content: Some(content),
            ..Default::default()
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Hover));
        let cursors = model.editor().cursors.clone();
        let mut measure = token::view::overlay_surface::cell_measure(1.0);
        let layout = token::view::modal::with_cursor_overlay_spec(&model, |spec| {
            token::view::overlay_surface::layout_measured(spec, 1000, 600, 1.0, &mut measure)
        })
        .unwrap();
        let text = layout.zones_text.unwrap();
        model.ui.hover = HoverRegion::EditorText;
        handle_mouse_wheel(
            &mut model,
            Some(((text.x + 1) as f64, (text.y + 1) as f64)),
            (0, 1).into(),
            Some(&mut measure),
        );
        assert_eq!(model.ui.cursor_overlay.unwrap().documentation.scroll, 3);
        crate::runtime::input::handle_key(
            &mut model,
            Key::Named(NamedKey::F1),
            PhysicalKey::Code(KeyCode::F1),
            Default::default(),
            false,
        );
        assert!(model.ui.cursor_overlay.unwrap().documentation.expanded);
        let cmd = crate::runtime::input::handle_key(
            &mut model,
            Key::Named(NamedKey::PageDown),
            PhysicalKey::Code(KeyCode::PageDown),
            crate::runtime::input::KeyModifiers {
                alt: true,
                ..Default::default()
            },
            false,
        );
        assert!(matches!(
            cmd,
            Some(Cmd::PageDocumentation { forward: true })
        ));
        let layout = token::view::modal::with_cursor_overlay_spec(&model, |spec| {
            token::view::overlay_surface::layout_measured(spec, 1000, 600, 1.0, &mut measure)
        })
        .unwrap();
        let bar = layout.docs_scrollbar.unwrap();
        let target = token::view::hit_test::hit_test_cursor_overlay(
            &model,
            token::view::hit_test::Point::new(
                (bar.track_rect.x + 1.0) as f64,
                (bar.track_rect.y + 1.0) as f64,
            ),
            &mut measure,
        )
        .unwrap();
        assert!(matches!(
            target,
            HitTarget::CursorOverlayDocumentation {
                scrollbar: Some(_),
                ..
            }
        ));
        assert!(!dismiss_overlay_for_press(&mut model, &target, MouseButton::Left).dismissed);
        update(&mut model, Msg::Ui(UiMsg::ToggleDocumentation));
        assert!(!model.ui.cursor_overlay.unwrap().documentation.expanded);
        assert_eq!(model.editor().cursors, cursors);
        assert_eq!(model.document().buffer.to_string(), "va\n");
        crate::runtime::input::handle_key(
            &mut model,
            Key::Named(NamedKey::Escape),
            PhysicalKey::Code(KeyCode::Escape),
            Default::default(),
            false,
        );
        assert!(model.ui.cursor_overlay.is_none());
        assert!(model.ui.hover_card.is_none());
    }

    #[test]
    fn popup_hover_requests_repaint_only_on_row_changes_and_preserves_selection() {
        use token::model::{CursorOverlayKind, CursorOverlayState};
        let mut model = AppModel::new(800, 600, 1.0);
        for kind in [
            CursorOverlayKind::Completion,
            CursorOverlayKind::ContextMenu,
            CursorOverlayKind::DebugCompletion,
            CursorOverlayKind::CodeActions,
            CursorOverlayKind::References,
        ] {
            model.ui.cursor_overlay = Some(CursorOverlayState::new(kind));
            let row = HitTarget::CursorOverlay {
                flat_index: Some(1),
            };
            assert!(update_hover_target(&mut model, Some(&row)));
            assert_eq!(model.ui.cursor_overlay.unwrap().hover_row, Some(1));
            assert_eq!(model.ui.cursor_overlay.unwrap().selected, 0);
            assert!(!update_hover_target(&mut model, Some(&row)));
            let separator = HitTarget::CursorOverlay { flat_index: None };
            assert!(update_hover_target(&mut model, Some(&separator)));
            assert_eq!(model.ui.cursor_overlay.unwrap().hover_row, None);
            assert!(update_hover_target(&mut model, Some(&row)));
            assert!(update_hover_target(&mut model, None));
            assert_eq!(model.ui.cursor_overlay.unwrap().hover_row, None);
            assert!(!update_hover_target(&mut model, None));
        }
    }

    use std::sync::mpsc;

    use super::*;
    use token::model::HoverRegion;
    use token::panel::{DockPosition, PanelId};
    use token::terminal::{PtyHandle, TerminalSession};

    fn terminal_model_with_history() -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0);
        model.dock_layout.bottom.activate(PanelId::TERMINAL);
        model.ui.hover = HoverRegion::Dock(DockPosition::Bottom);

        let (pty, _pty_rx) = PtyHandle::new_for_test();
        let (msg_tx, _msg_rx) = mpsc::channel();
        let mut session = TerminalSession::new(7, 4, 20, pty, msg_tx);
        session.apply_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\n");
        model.terminal.sessions.push(session);
        model
    }

    #[test]
    fn terminal_link_hover_requires_modifier_and_yields_to_selection() {
        let mut model = terminal_model_with_history();
        let session = model.terminal.active_session_mut().unwrap();
        session.clear();
        session.apply_bytes(b"\x1b[Hhttps://example.com");
        let viewport = token::panels::terminal::TerminalViewport::for_model(&model).unwrap();
        let (x, y) = (
            (viewport.rect.x + 2.0) as f64,
            (viewport.rect.y + 2.0) as f64,
        );
        let target = HitTarget::DockContent {
            position: DockPosition::Bottom,
            active_panel_id: PanelId::Terminal,
        };
        let modifier = if cfg!(target_os = "macos") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        };
        assert!(
            terminal_link_at_pointer(&model, Some(&target), x, y, ModifiersState::empty())
                .is_none()
        );
        assert!(update_terminal_link_hover(
            &mut model,
            Some(&target),
            x,
            y,
            modifier
        ));
        assert_eq!(
            model.terminal.hovered_link.as_ref().unwrap().1.uri,
            "https://example.com"
        );
        assert!(update_terminal_link_hover(
            &mut model,
            Some(&target),
            x,
            y,
            ModifiersState::empty()
        ));
        model.terminal.selection_drag = Some(7);
        assert!(terminal_link_at_pointer(&model, Some(&target), x, y, modifier).is_none());
        model.terminal.selection_drag = None;
        assert!(terminal_link_at_pointer(
            &model,
            Some(&HitTarget::Modal { inside: true }),
            x,
            y,
            modifier
        )
        .is_none());
    }

    // ========================================================================
    // Interactive gutter lane suppression (editor-decorations.md)
    // ========================================================================

    fn gutter_target(lane: Option<token::view::geometry::LaneId>) -> HitTarget {
        HitTarget::EditorGutter {
            group_id: token::model::editor_area::GroupId(0),
            editor_id: token::model::editor_area::EditorId(0),
            line: 0,
            lane,
        }
    }

    #[test]
    fn interactive_lane_press_does_not_arm_content_drag() {
        use token::view::geometry::LaneId;

        assert!(
            !arms_content_drag(&gutter_target(Some(LaneId::Fold))),
            "a chevron click must not arm text-selection drag"
        );
    }

    #[test]
    fn non_interactive_gutter_press_arms_content_drag() {
        assert!(
            arms_content_drag(&gutter_target(None)),
            "line-number gutter clicks must keep arming drag, same as today"
        );
    }

    #[test]
    fn editor_content_press_arms_content_drag() {
        let target = HitTarget::EditorContent {
            group_id: token::model::editor_area::GroupId(0),
            editor_id: token::model::editor_area::EditorId(0),
            document_id: token::model::editor_area::DocumentId(0),
        };
        assert!(arms_content_drag(&target));
    }

    #[test]
    fn interactive_lane_click_is_suppressed() {
        use token::view::geometry::LaneId;

        let result = interactive_gutter_lane_click(Some(LaneId::Fold));
        assert!(matches!(
            result,
            Some(EventResult::Consumed { redraw: false, .. })
        ));
    }

    #[test]
    fn non_interactive_lane_click_falls_through() {
        assert!(interactive_gutter_lane_click(None).is_none());
    }

    #[test]
    fn cursor_overlay_row_click_accepts_a_completion_item() {
        use token::messages::DocumentMsg;
        use token::model::{Cursor, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str("value_one\n\n");
        model.editor_mut().cursors[0] = Cursor::at(1, 0);
        model.editor_mut().clear_selection();
        for ch in "val".chars() {
            update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        }
        assert!(model.ui.completion_menu.is_some(), "menu should be open");
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        let result = handle_cursor_overlay_click(&mut model, Some(0));

        assert!(matches!(result, EventResult::Consumed { redraw: true, .. }));
        assert!(
            model.ui.completion_menu.is_none(),
            "a row click must accept and close the menu, not just select the row"
        );
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "value_one");
    }

    #[test]
    fn cursor_overlay_row_click_activates_a_context_menu_item() {
        use token::context_menu::MenuItem;
        use token::messages::LayoutMsg;
        use token::model::{ContextMenuState, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0);
        // A second tab so "Close" (targeting tab 0) has something to close
        // without hitting the "can't close the last tab" guard.
        update(&mut model, Msg::Layout(LayoutMsg::NewTab));
        let group_id = model.editor_area.focused_group_id;
        let first_tab = model.editor_area.groups[&group_id].tabs[0].id;

        let items = vec![MenuItem::custom(
            "Close",
            true,
            vec![Msg::Layout(LayoutMsg::CloseTab(first_tab))],
        )];
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::ContextMenu));
        model.ui.context_menu = Some(ContextMenuState {
            items,
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::EditorTabBar,
        });

        let result = handle_cursor_overlay_click(&mut model, Some(0));

        assert!(matches!(result, EventResult::Consumed { redraw: true, .. }));
        assert!(
            model.ui.context_menu.is_none(),
            "activation closes the menu"
        );
        assert_eq!(model.editor_area.groups[&group_id].tabs.len(), 1);
        assert!(!model.editor_area.groups[&group_id]
            .tabs
            .iter()
            .any(|t| t.id == first_tab));
    }

    #[test]
    fn cursor_overlay_row_click_runs_the_activated_items_cmd() {
        // Regression: `handle_cursor_overlay_click` used to discard
        // `update()`'s returned Cmd and return `consumed_redraw()`
        // (cmd: None) unconditionally, so a mouse click on e.g. "Copy
        // Absolute Path" closed the menu but never ran the
        // `CopyToClipboard` command the keyboard Enter path produces.
        use token::context_menu::MenuItem;
        use token::messages::ContextMenuMsg;
        use token::model::{ContextMenuState, CursorOverlayKind, CursorOverlayState};

        let mut model = AppModel::new(800, 600, 1.0);
        let path = std::path::PathBuf::from("/tmp/x.rs");
        let items = vec![MenuItem::custom(
            "Copy Absolute Path",
            true,
            vec![Msg::ContextMenu(ContextMenuMsg::CopyPath {
                path,
                relative: false,
            })],
        )];
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::ContextMenu));
        model.ui.context_menu = Some(ContextMenuState {
            items,
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::Editor,
        });

        let result = handle_cursor_overlay_click(&mut model, Some(0));

        match result {
            EventResult::Consumed { cmd: Some(cmd), .. } => {
                assert!(
                    matches!(&cmd, Cmd::Batch(cmds) if cmds.iter().any(|c| matches!(c, Cmd::CopyToClipboard(_)))),
                    "expected the CopyToClipboard cmd to reach the caller, got {cmd:?}"
                );
            }
            other => {
                panic!("expected a Consumed result carrying the activation's Cmd, got {other:?}")
            }
        }
    }

    #[test]
    fn pixel_wheel_targets_hovered_split_without_changing_keyboard_focus() {
        let mut model = AppModel::new(1000, 600, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str(&"long text line\n".repeat(200));
        let original = model.editor_area.focused_editor_id().unwrap();
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(
                token::model::SplitDirection::Horizontal,
            )),
        );
        let focused = model.editor_area.focused_editor_id().unwrap();
        assert_ne!(original, focused);
        model.resize(1000, 600);
        let group = model
            .editor_area
            .groups
            .values()
            .find(|g| g.active_editor_id() == Some(original))
            .unwrap();
        let point = (group.rect.x as f64 + 100.0, group.rect.y as f64 + 100.0);
        model.ui.hover = HoverRegion::EditorText;
        handle_mouse_wheel(
            &mut model,
            Some(point),
            WheelScroll {
                rows: (0, 0),
                pixels: Some((0.0, 7.25)),
                animated: false,
            },
            None,
        );
        assert_eq!(model.editor_area.focused_editor_id(), Some(focused));
        assert_eq!(
            model.editor_area.editors[&original]
                .pixel_scroll_position()
                .1,
            7.25
        );
        assert_eq!(model.editor().pixel_scroll_position().1, 0.0);
        // A special tab owning keyboard focus must not block the explicit
        // plain-text target or accidentally enter its text editing path.
        model.editor_mut().tab_content =
            token::model::TabContent::BinaryPlaceholder(token::model::BinaryPlaceholderState {
                path: "fixture.bin".into(),
                size_bytes: 1,
            });
        handle_mouse_wheel(
            &mut model,
            Some(point),
            WheelScroll {
                rows: (0, 0),
                pixels: Some((0.0, 7.25)),
                animated: false,
            },
            None,
        );
        assert_eq!(
            model.editor_area.editors[&original]
                .pixel_scroll_position()
                .1,
            14.5
        );
        assert_eq!(model.editor_area.focused_editor_id(), Some(focused));
    }

    #[test]
    fn mouse_wheel_up_over_terminal_dock_scrolls_scrollback() {
        let mut model = terminal_model_with_history();

        let cmd = handle_mouse_wheel(&mut model, Some((0.0, 0.0)), (0, -3).into(), None);

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 3);
    }

    #[test]
    fn mouse_wheel_down_over_terminal_dock_scrolls_toward_bottom() {
        let mut model = terminal_model_with_history();
        model.terminal.active_session_mut().unwrap().scroll_offset = 4;

        let cmd = handle_mouse_wheel(&mut model, Some((0.0, 0.0)), (0, 2).into(), None);

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 2);
    }

    #[test]
    fn cursor_overlay_wheel_scroll_clamps_to_row_count() {
        // The debug Completion demo has exactly `MAX_VISIBLE_COMPLETION`
        // rows, so its whole list is always visible and `max_scroll` is 0 —
        // any number of downward notches must leave `scroll` at 0 rather
        // than accumulating unboundedly (regression: previously required
        // as many upward notches to "unwind" before the (already-fully-
        // visible) window would move again).
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.hover = HoverRegion::CursorOverlay;
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::DebugCompletion,
        ));
        assert_eq!(
            token::view::modal::debug_completion_row_count(),
            token::view::overlay_surface::MAX_VISIBLE_COMPLETION
        );

        for _ in 0..20 {
            handle_mouse_wheel(&mut model, None, (0, 3).into(), None);
        }
        assert_eq!(model.ui.cursor_overlay.unwrap().scroll, 0);

        handle_mouse_wheel(&mut model, None, (0, -3).into(), None);
        assert_eq!(model.ui.cursor_overlay.unwrap().scroll, 0);
    }

    #[test]
    fn tab_bar_horizontal_scroll_matches_content_direction() {
        // Regression: horizontal tab scrolling was inverted once trackpad
        // horizontal deltas stopped truncating to zero. Positive `h_delta`
        // must reveal tabs further right (positive `delta_px`), matching
        // editor horizontal scrolling.
        assert_eq!(tab_bar_scroll_delta_px(2, 0, 10), Some(20));
        assert_eq!(tab_bar_scroll_delta_px(-2, 0, 10), Some(-20));
    }

    #[test]
    fn tab_bar_horizontal_takes_precedence_over_vertical() {
        assert_eq!(tab_bar_scroll_delta_px(1, 5, 10), Some(10));
    }

    #[test]
    fn tab_bar_vertical_wheel_falls_back_with_inverted_sign() {
        // Plain mouse wheel (no X axis) keeps its legacy repurposed sign.
        assert_eq!(tab_bar_scroll_delta_px(0, 3, 10), Some(-30));
    }

    #[test]
    fn tab_bar_no_scroll_when_both_axes_are_zero() {
        assert_eq!(tab_bar_scroll_delta_px(0, 0, 10), None);
    }

    // ========================================================================
    // Context menu (context-menu.md)
    // ========================================================================

    fn right_click_event() -> MouseEvent {
        MouseEvent::new(50.0, 60.0, MouseButton::Right, ModifiersState::empty())
    }

    #[test]
    fn right_click_on_editor_content_opens_the_editor_menu() {
        let mut model = AppModel::new(800, 600, 1.0);
        let group_id = model.editor_area.focused_group_id;
        let editor_id = model.editor_area.focused_editor_id().unwrap();
        let document_id = model.editor_area.focused_document_id().unwrap();
        let target = HitTarget::EditorContent {
            group_id,
            editor_id,
            document_id,
        };

        let result = handle_right_click(&mut model, &target, &right_click_event());

        assert!(matches!(result, EventResult::Consumed { .. }));
        assert!(model.ui.context_menu.is_some(), "the editor menu opened");
        assert_eq!(
            model.ui.context_menu.unwrap().region,
            token::context_menu::ContextMenuRegion::Editor
        );
    }

    #[test]
    fn editor_click_uses_the_layout_before_find_bar_moves_between_splits() {
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            let mut model = AppModel::new(1000, 800, 1.0);
            model.document_mut().buffer = ropey::Rope::from_str(&"alpha beta gamma\n".repeat(80));
            let group_id = model.editor_area.focused_group_id;
            update(
                &mut model,
                Msg::Layout(LayoutMsg::SplitFocused(
                    token::model::SplitDirection::Vertical,
                )),
            );
            model.resize(1000, 800);
            update(&mut model, Msg::Ui(UiMsg::OpenFind { replace: true }));
            let group = &model.editor_area.groups[&group_id];
            let layout = token::view::geometry::GroupLayout::new(group, &model, model.char_width);
            let event = MouseEvent::new(
                layout.text_start_x as f64 + model.char_width as f64 * 3.0,
                layout.content_y() as f64 + model.line_height as f64 * 5.25,
                button,
                ModifiersState::empty(),
            );
            let target = HitTarget::EditorContent {
                group_id,
                editor_id: group.active_editor_id().unwrap(),
                document_id: model
                    .editor_area
                    .document_for_group(group)
                    .unwrap()
                    .id
                    .unwrap(),
            };
            match button {
                MouseButton::Left => {
                    handle_editor_content_click(
                        &mut model,
                        group_id,
                        &event,
                        &mut ClickTracker::default(),
                    );
                }
                MouseButton::Right => {
                    handle_right_click(&mut model, &target, &event);
                }
                MouseButton::Middle => {
                    handle_middle_click(&mut model, &target, &event);
                    assert!(model.editor().rectangle_selection.active);
                }
                _ => unreachable!(),
            }
            assert_eq!(model.editor_area.focused_group_id, group_id);
            let position = if button == MouseButton::Middle {
                let selection = &model.editor().rectangle_selection;
                (selection.start_line, selection.start_visual_col)
            } else {
                let cursor = model.editor().active_cursor();
                (cursor.line, cursor.column)
            };
            assert_eq!(position, (5, 3), "{button:?}");
            let moved = token::view::find_bar::FindBarLayout::new(&model).unwrap();
            assert!(moved.rect.height > 0.0);
            assert_eq!(
                moved.rect.x,
                model.editor_area.focused_group().unwrap().rect.x
            );
        }
    }

    #[test]
    fn right_click_on_a_sidebar_item_opens_the_file_tree_menu() {
        let mut model = AppModel::new(800, 600, 1.0);
        let target = HitTarget::SidebarItem {
            path: std::path::PathBuf::from("/tmp/foo.rs"),
            row: 0,
            is_dir: false,
            clicked_on_chevron: false,
        };

        handle_right_click(&mut model, &target, &right_click_event());

        assert_eq!(
            model.ui.context_menu.unwrap().region,
            token::context_menu::ContextMenuRegion::FileTree
        );
    }

    #[test]
    fn right_click_on_a_region_with_no_v1_menu_bubbles() {
        let mut model = AppModel::new(800, 600, 1.0);
        let result = handle_right_click(&mut model, &HitTarget::StatusBar, &right_click_event());
        assert!(matches!(result, EventResult::Bubble));
        assert!(model.ui.context_menu.is_none());
    }

    #[test]
    fn right_click_is_a_no_op_while_a_modal_is_open() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.active_modal = Some(token::model::ModalState::GotoLine(Default::default()));
        let target = HitTarget::SidebarItem {
            path: std::path::PathBuf::from("/tmp/foo.rs"),
            row: 0,
            is_dir: false,
            clicked_on_chevron: false,
        };

        let result = handle_right_click(&mut model, &target, &right_click_event());

        assert!(model.ui.context_menu.is_none());
        assert!(
            matches!(result, EventResult::Consumed { .. }),
            "still consumed — no menu opened, but the click shouldn't fall through either"
        );
    }

    // ========================================================================
    // Mouse-press preamble policy (context-menu.md §Mouse): click-away
    // consumes, and the right-click-reopen exception (075f957).
    // ========================================================================

    #[test]
    fn click_away_from_a_context_menu_dismisses_and_swallows_the_click() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::ContextMenu,
        ));
        model.ui.context_menu = Some(token::model::ContextMenuState {
            items: vec![],
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::Editor,
        });

        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Left);

        assert!(dismissal.dismissed);
        assert!(
            dismissal.swallow,
            "a left-click that dismisses a context menu must not also act on what it landed on"
        );
        assert!(model.ui.cursor_overlay.is_none());
        assert!(model.ui.context_menu.is_none());
    }

    #[test]
    fn a_right_click_dismisses_a_context_menu_without_swallowing_so_it_can_reopen() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::ContextMenu,
        ));
        model.ui.context_menu = Some(token::model::ContextMenuState {
            items: vec![],
            anchor: (0, 0, 0),
            region: token::context_menu::ContextMenuRegion::Editor,
        });

        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Right);

        assert!(dismissal.dismissed);
        assert!(
            !dismissal.swallow,
            "a right-click on a second target must still reach handle_right_click to reopen"
        );
    }

    #[test]
    fn click_away_from_a_non_context_menu_overlay_dismisses_without_swallowing() {
        // Completion/hover/references are non-blocking (overlay-surface.md
        // Phase 5): click-away dismisses but falls through to whatever's
        // under it — only the context menu swallows.
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::Completion,
        ));

        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Left);

        assert!(dismissal.dismissed);
        assert!(!dismissal.swallow);
    }

    #[test]
    fn click_on_the_cursor_overlay_itself_never_dismisses() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
            token::model::CursorOverlayKind::ContextMenu,
        ));

        let dismissal = dismiss_overlay_for_press(
            &mut model,
            &HitTarget::CursorOverlay {
                flat_index: Some(0),
            },
            MouseButton::Left,
        );

        assert!(!dismissal.dismissed);
        assert!(!dismissal.swallow);
        assert!(model.ui.cursor_overlay.is_some());
    }

    #[test]
    fn no_open_overlay_is_a_no_op() {
        let mut model = AppModel::new(800, 600, 1.0);
        let dismissal =
            dismiss_overlay_for_press(&mut model, &HitTarget::StatusBar, MouseButton::Left);
        assert!(!dismissal.dismissed);
        assert!(!dismissal.swallow);
    }

    #[test]
    fn tab_file_path_resolves_the_tabs_document_not_the_focused_one() {
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
        // An untitled buffer has no path.
        assert_eq!(tab_file_path(&model, group_id, tab_id), None);
    }
}

impl ClickTracker {
    /// Update click count based on timing and click target
    ///
    /// Returns the new click count (1, 2, or 3)
    pub fn track_click(&mut self, region: ClickRegion) -> u8 {
        let now = Instant::now();
        let double_click_time = Duration::from_millis(300);

        let is_rapid_click = now.duration_since(self.last_click_time) < double_click_time;
        let is_same_target = self.last_click_region == Some(region);

        if is_rapid_click && is_same_target {
            self.click_count += 1;
            if self.click_count > 3 {
                self.click_count = 1;
            }
        } else {
            self.click_count = 1;
        }

        self.last_click_time = now;
        self.last_click_region = Some(region);

        self.click_count as u8
    }
}

/// Tracks drag state for text selection (left mouse button drag).
///
/// Encapsulates the state machine: idle → mouse down → threshold exceeded → dragging.
/// Also handles auto-scroll throttling during drag.
#[derive(Default)]
pub struct DragState {
    left_mouse_down: bool,
    start_position: Option<(f64, f64)>,
    active: bool,
    last_auto_scroll: Option<Instant>,
}

impl DragState {
    /// Whether the left mouse button is currently held down.
    pub fn is_down(&self) -> bool {
        self.left_mouse_down
    }

    /// Start tracking a potential drag from the given position.
    pub fn begin(&mut self, x: f64, y: f64) {
        self.left_mouse_down = true;
        self.start_position = Some((x, y));
        self.active = false;
    }

    /// End the drag (mouse released).
    pub fn end(&mut self) {
        self.left_mouse_down = false;
        self.start_position = None;
        self.active = false;
        self.last_auto_scroll = None;
    }

    /// Whether a drag is currently active (threshold exceeded).
    pub fn is_active(&self) -> bool {
        self.left_mouse_down && self.active
    }

    /// Check if mouse movement exceeds the drag threshold (4px).
    /// Returns the start position if the threshold was just crossed, None otherwise.
    pub fn check_threshold(&mut self, x: f64, y: f64) -> Option<(f64, f64)> {
        const DRAG_THRESHOLD_PIXELS: f64 = 4.0;

        if self.active || !self.left_mouse_down {
            return None;
        }

        if let Some((start_x, start_y)) = self.start_position {
            let dx = x - start_x;
            let dy = y - start_y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance >= DRAG_THRESHOLD_PIXELS {
                self.active = true;
                return Some((start_x, start_y));
            }
        }
        None
    }

    /// Attempt auto-scroll during drag. Returns a scroll direction (+1 or -1)
    /// if the cursor is outside the visible area and enough time has passed.
    pub fn try_auto_scroll(&mut self, y: f64, status_bar_top: f64) -> Option<i32> {
        const AUTO_SCROLL_INTERVAL_MS: u64 = 80;

        let direction = if y < 0.0 {
            Some(-1)
        } else if y >= status_bar_top {
            Some(1)
        } else {
            None
        };

        let direction = direction?;

        let now = Instant::now();
        if let Some(last) = self.last_auto_scroll {
            if now.duration_since(last) < Duration::from_millis(AUTO_SCROLL_INTERVAL_MS) {
                return None;
            }
        }

        self.last_auto_scroll = Some(now);
        Some(direction)
    }
}

/// Distance in pixels before an armed tab drag becomes active
const TAB_DRAG_THRESHOLD_PIXELS: f64 = 4.0;

/// Find the group whose tab bar contains the point, along with the tab index
/// under the cursor (or the last index when over the empty tail of the bar).
fn tab_bar_target_at(model: &AppModel, x: f64, y: f64) -> Option<(GroupId, usize)> {
    model.editor_area.groups.values().find_map(|group| {
        let layout = EditorTabBarLayout::new(group, model, model.char_width);
        if !layout.contains(x, y) {
            return None;
        }
        let index = layout
            .tab_at(x, y)
            .and_then(|tab_id| group.tabs.iter().position(|tab| tab.id == tab_id))
            .unwrap_or_else(|| group.tabs.len().saturating_sub(1));
        Some((group.id, index))
    })
}

/// Find the group that currently owns a tab.
fn tab_owning_group(model: &AppModel, tab_id: token::model::editor_area::TabId) -> Option<GroupId> {
    model
        .editor_area
        .groups
        .iter()
        .find_map(|(id, g)| g.tabs.iter().any(|t| t.id == tab_id).then_some(*id))
}

/// Update an armed/active tab drag on mouse move.
///
/// The drag is fully live: hovering a tab bar reorders the tab into that
/// slot (moving it between groups first if needed), and hovering another
/// pane's content area moves the tab into that pane. Releasing simply drops
/// the tab where it already is (`end_tab_drag`).
pub fn update_tab_drag(model: &mut AppModel, x: f64, y: f64) -> Option<Cmd> {
    let drag = model.ui.tab_drag.as_mut()?;
    drag.current = (x, y);

    if !drag.active {
        let dx = x - drag.press.0;
        let dy = y - drag.press.1;
        if (dx * dx + dy * dy).sqrt() < TAB_DRAG_THRESHOLD_PIXELS {
            return None;
        }
        drag.active = true;
    }

    let tab_id = drag.tab_id;
    let owning_group = tab_owning_group(model, tab_id)?;

    // Tab bars take priority; a pane's content area targets that pane's
    // tab end, but only for *other* panes (dragging into your own pane's
    // text area must not reorder anything).
    let target = tab_bar_target_at(model, x, y).or_else(|| {
        model.editor_area.groups.values().find_map(|g| {
            (g.id != owning_group && g.rect.contains(x as f32, y as f32))
                .then_some((g.id, usize::MAX))
        })
    });

    if let Some((group_id, index)) = target {
        if group_id != owning_group {
            update(
                model,
                Msg::Layout(LayoutMsg::MoveTab {
                    tab_id,
                    to_group: group_id,
                }),
            );
            update(model, Msg::Layout(LayoutMsg::FocusGroup(group_id)));
        }
        // ReorderTab clamps the index, so usize::MAX means "keep at end"
        update(
            model,
            Msg::Layout(LayoutMsg::ReorderTab {
                tab_id,
                to_index: index,
            }),
        );
    }

    // Full redraw every move: the drag ghost follows the cursor anywhere
    Some(Cmd::Redraw)
}

/// Finish a tab drag on mouse release.
///
/// Moves/reorders happen live during the drag, so this only clears the
/// drag state and repaints to remove the ghost.
pub fn end_tab_drag(model: &mut AppModel) -> Option<Cmd> {
    let drag = model.ui.tab_drag.take()?;
    if !drag.active {
        return None; // plain click, no drag happened
    }
    Some(Cmd::Redraw)
}

/// Construct a MouseEvent from raw input data
pub fn make_mouse_event(
    x: f64,
    y: f64,
    button: MouseButton,
    modifiers: ModifiersState,
) -> MouseEvent {
    MouseEvent::new(x, y, button, modifiers)
}

/// Result of mouse press handling, including state changes for the App
#[derive(Debug, Clone)]
pub struct MousePressResult {
    /// Command to execute (usually Redraw or None)
    pub cmd: Option<Cmd>,
    /// Whether to start tracking left mouse drag (for text selection)
    pub start_drag_tracking: bool,
}

/// Outcome of `dismiss_overlay_for_press`: whether an open cursor-anchored
/// popup was dismissed by this press, and whether the press must be
/// swallowed outright (consumed, not click-through) rather than falling
/// through to whatever's under it.
struct OverlayDismissal {
    dismissed: bool,
    swallow: bool,
}

/// The mouse-press preamble's overlay policy (context-menu.md §Mouse,
/// 075f957) — factored out of `handle_mouse_press` so it's unit-testable
/// without a `Renderer`/real `Window`.
///
/// Cursor-anchored popups are non-blocking (overlay-surface.md Phase 5): a
/// click that lands outside the popup dismisses it but still falls through
/// to whatever's actually under the cursor. The context menu is the one
/// exception (context-menu.md "Mouse: click-away consumes, not
/// click-through" — JetBrains behavior, not VS Code's) — except a
/// right-click while a context menu is open must still reach
/// `dispatch_mouse_press`/`handle_right_click` so a new menu can open at
/// the new target (context-menu.md Phase 2: "another cursor overlay
/// already open -> close the old one first and re-open").
fn dismiss_overlay_for_press(
    model: &mut AppModel,
    target: &HitTarget,
    button: MouseButton,
) -> OverlayDismissal {
    let dismissed_kind = model.ui.cursor_overlay.map(|s| s.kind);
    let dismissed = model.ui.cursor_overlay.is_some()
        && !matches!(
            target,
            HitTarget::CursorOverlay { .. } | HitTarget::CursorOverlayDocumentation { .. }
        );
    if dismissed {
        model.ui.cursor_overlay = None;
        model.ui.completion_menu = None;
        model.ui.hover_card = None;
        model.ui.reference_list = None;
        model.ui.code_action_list = None;
        model.ui.context_menu = None;
    }
    let swallow = dismissed
        && dismissed_kind == Some(token::model::CursorOverlayKind::ContextMenu)
        && button != MouseButton::Right;
    OverlayDismissal { dismissed, swallow }
}

/// Handle a mouse press event using the unified hit-test system
///
/// This is the main entry point for mouse click handling. It:
/// 1. Performs hit-testing to find the target
/// 2. Dispatches to the appropriate handler based on (target, button)
/// 3. Applies focus changes from EventResult
/// 4. Returns MousePressResult with command and state changes
pub fn handle_mouse_press(
    model: &mut AppModel,
    renderer: &mut Renderer,
    event: MouseEvent,
    click_tracker: &mut ClickTracker,
) -> MousePressResult {
    let char_width = renderer.char_width();
    let pt = event.pos;

    // Perform hit-testing, measuring text through the renderer's glyph
    // cache so overlay geometry matches what was painted.
    let target = {
        let mut painter = renderer.text_painter();
        let mut measure = token::layout::PainterMeasure::new(&mut painter);
        hit_test_ui(model, pt, char_width, &mut measure)
    };
    let Some(target) = target else {
        return MousePressResult {
            cmd: None,
            start_drag_tracking: false,
        };
    };

    let dismissal = dismiss_overlay_for_press(model, &target, event.button);
    let dismissed_cursor_overlay = dismissal.dismissed;
    if dismissal.swallow {
        // Consumed, not click-through: the dismissing click must not also
        // act on whatever it landed on.
        return MousePressResult {
            cmd: Some(Cmd::Redraw),
            start_drag_tracking: false,
        };
    }

    // Track if we're clicking on editor content (for drag tracking).
    // Interactive gutter lanes (fold chevron, marks) consume the press
    // themselves (see `handle_left_click`) — a chevron click must not
    // arm text-selection drag tracking.
    let is_selectable_content = arms_content_drag(&target);
    let is_left_click = matches!(event.button, MouseButton::Left);

    // Dispatch based on target and button
    let result = dispatch_mouse_press(model, renderer, &target, &event, click_tracker);

    // Apply focus changes
    if let EventResult::Consumed {
        focus: Some(focus_target),
        ..
    } = &result
    {
        match focus_target {
            token::model::FocusTarget::Editor => model.ui.focus_editor(),
            token::model::FocusTarget::Dock(pos) => model.ui.focus_dock(*pos),
            token::model::FocusTarget::Modal => {}
            token::model::FocusTarget::FindBar => {
                model.ui.focus = token::model::FocusTarget::FindBar
            }
        }
    }

    // Determine command - use explicit cmd if present, otherwise fallback to redraw
    let cmd = match &result {
        EventResult::Consumed { cmd: Some(c), .. } => Some(c.clone()),
        EventResult::Consumed { redraw: true, .. } => Some(Cmd::Redraw),
        EventResult::Consumed { redraw: false, .. } if dismissed_cursor_overlay => {
            Some(Cmd::Redraw)
        }
        EventResult::Consumed { redraw: false, .. } => None,
        EventResult::Bubble if dismissed_cursor_overlay => Some(Cmd::Redraw),
        EventResult::Bubble => None,
    };

    MousePressResult {
        cmd,
        start_drag_tracking: is_selectable_content
            && is_left_click
            && (!matches!(
                target,
                HitTarget::DockContent {
                    active_panel_id: token::panel::PanelId::Terminal,
                    ..
                }
            ) || model.terminal.selection_drag.is_some()),
    }
}

/// Whether a press on `target` should arm text-selection drag tracking:
/// editor content, or a gutter click outside any interactive lane. A press
/// on an interactive lane (fold chevron, marks — editor-decorations.md)
/// consumes itself in `handle_left_click`/`handle_middle_click` and must
/// not also start a text selection.
fn arms_content_drag(target: &HitTarget) -> bool {
    matches!(
        target,
        HitTarget::EditorContent { .. }
            | HitTarget::ModalField { .. }
            | HitTarget::FindBar {
                control: Some(token::view::find_bar::Control::Field(_))
            }
            | HitTarget::ImageContent { .. }
            | HitTarget::DockContent {
                active_panel_id: token::panel::PanelId::Terminal,
                ..
            }
    ) || matches!(
        target,
        HitTarget::EditorGutter { lane, .. } if !lane.is_some_and(|lane| lane.is_interactive())
    )
}

/// A press on an interactive gutter lane (fold chevron, marks) consumes
/// itself as a no-op rather than falling through to the default
/// focus/rectangle-selection gutter behavior — no lane owner has shipped
/// yet, but `handle_left_click`/`handle_middle_click` must actively
/// suppress it rather than let it fall through (editor-decorations.md).
fn interactive_gutter_lane_click(
    lane: Option<token::view::geometry::LaneId>,
) -> Option<EventResult> {
    lane.is_some_and(|lane| lane.is_interactive())
        .then(EventResult::consumed_no_redraw)
}

/// Dispatch a mouse press to the appropriate handler based on target and button
fn dispatch_mouse_press(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    match event.button {
        MouseButton::Left => handle_left_click(model, renderer, target, event, click_tracker),
        MouseButton::Middle => handle_middle_click(model, target, event),
        MouseButton::Right => handle_right_click(model, target, event),
        _ => EventResult::Bubble,
    }
}

/// Click on a cursor-anchored popup row: update selection, and — for the
/// Completion popup specifically — accept the clicked item (the same
/// message `Enter` sends).
fn handle_cursor_overlay_click(model: &mut AppModel, flat_index: Option<usize>) -> EventResult {
    let Some(idx) = flat_index else {
        return EventResult::consumed_redraw();
    };
    let kind = model.ui.cursor_overlay.map(|state| state.kind);
    if let Some(state) = &mut model.ui.cursor_overlay {
        if state.selected != idx {
            state.reset_documentation();
        }
        state.selected = idx;
    }
    // The activation message may return a Cmd (e.g. CopyToClipboard) that
    // must actually run — same as the keyboard Enter path in
    // `handle_cursor_overlay_key`, which returns `update()`'s result
    // directly instead of discarding it.
    let cmd = match kind {
        Some(token::model::CursorOverlayKind::Completion) => {
            update(model, Msg::Completion(CompletionMsg::AcceptMenuItem))
        }
        // A row click sets selection and activates in one step
        // (overlay-surface.md Pointer) — same as Enter.
        Some(token::model::CursorOverlayKind::References) => update(
            model,
            Msg::Lsp(token::messages::LspMsg::ActivateReference { index: idx }),
        ),
        Some(token::model::CursorOverlayKind::CodeActions) => update(
            model,
            Msg::Lsp(token::messages::LspMsg::ActivateCodeAction { index: idx }),
        ),
        Some(token::model::CursorOverlayKind::ContextMenu) => update(
            model,
            Msg::ContextMenu(token::messages::ContextMenuMsg::ActivateItem { index: idx }),
        ),
        _ => None,
    };
    match cmd {
        Some(cmd) => EventResult::Consumed {
            redraw: true,
            focus: None,
            cmd: Some(cmd),
        },
        None => EventResult::consumed_redraw(),
    }
}

/// Modal pointer actions must carry effects back to the runtime, not only
/// mutate state. Dropping these commands can leave a save/loading state pending.
fn modal_press(model: &mut AppModel, message: ModalMsg) -> EventResult {
    EventResult::Consumed {
        redraw: true,
        focus: None,
        cmd: update(model, Msg::Ui(UiMsg::Modal(message))),
    }
}

/// Capture a modal thumb or jump along its track without changing selection.
fn modal_scrollbar_press(
    model: &mut AppModel,
    geometry: &token::view::scrollbar::ScrollbarGeometry,
    event: &MouseEvent,
) -> EventResult {
    let Some(modal) = &model.ui.active_modal else {
        return EventResult::consumed_no_redraw();
    };
    let target = ScrollbarTarget::Modal(modal.id());
    overlay_scrollbar_press(model, target, geometry, event)
}

/// Modal lists and documentation share vertical track clicks and thumb capture.
fn overlay_scrollbar_press(
    model: &mut AppModel,
    target: ScrollbarTarget,
    geometry: &token::view::scrollbar::ScrollbarGeometry,
    event: &MouseEvent,
) -> EventResult {
    let message = if geometry.hits_thumb(event.pos.x as f32, event.pos.y as f32) {
        UiMsg::ScrollbarThumbPressed(ScrollbarDragState {
            target,
            axis: ScrollbarDragAxis::Vertical,
            grab_offset: event.pos.y as f32 - geometry.thumb_rect.y,
            track_start: geometry.track_rect.y,
            track_size: geometry.track_rect.height,
            thumb_size: geometry.thumb_rect.height,
            max_scroll: geometry.state.max_position(),
        })
    } else {
        UiMsg::ScrollbarTrackClicked {
            target,
            axis: ScrollbarDragAxis::Vertical,
            new_position: geometry.position_from_track_click(event.pos.y as f32),
        }
    };
    EventResult::Consumed {
        redraw: true,
        focus: None,
        cmd: update(model, Msg::Ui(message)),
    }
}

/// Handle left mouse button clicks
fn handle_left_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    use token::model::FocusTarget;

    match target {
        HitTarget::FindBar { control } => {
            let clicks = if let Some(token::view::find_bar::Control::Field(field)) = control {
                click_tracker.track_click(ClickRegion::FindField(*field))
            } else {
                0
            };
            let cmd = if let Some(token::view::find_bar::Control::Field(field)) = control {
                token::view::find_bar::column_at(model, *field, event.pos.x).and_then(|column| {
                    update(
                        model,
                        Msg::Ui(UiMsg::FindFieldPointer {
                            field: *field,
                            column,
                            extend: event.shift(),
                            clicks,
                        }),
                    )
                })
            } else {
                control.and_then(|control| update(model, Msg::Ui(control.message())))
            };
            EventResult::Consumed {
                redraw: true,
                focus: None,
                cmd,
            }
        }
        // Modal handling
        HitTarget::ModalField { row, position } => EventResult::Consumed {
            redraw: true,
            focus: None,
            cmd: update(
                model,
                Msg::Ui(UiMsg::Settings(
                    token::messages::SettingsMsg::FieldPointer {
                        row: *row,
                        position: *position,
                        extend: event.shift(),
                    },
                )),
            ),
        },
        HitTarget::ModalScrollbar { geometry } => modal_scrollbar_press(model, geometry, event),
        HitTarget::Modal { inside } => {
            if *inside {
                // Click inside modal (header/footer/padding) - consume but
                // don't close or act.
                EventResult::consumed_redraw()
            } else {
                // Click outside modal - close it
                modal_press(model, ModalMsg::Close)
            }
        }

        // Row click: select and activate in one step (overlay-surface.md
        // Pointer: "a click sets selection and activates in one step").
        HitTarget::ModalRow { flat_index } => {
            modal_press(model, ModalMsg::ActivateRow(*flat_index))
        }
        HitTarget::ModalChoice { flat_index, choice } => modal_press(
            model,
            ModalMsg::ChooseSetting {
                row: *flat_index,
                choice: *choice,
            },
        ),

        // Tab click: switch the Search Everywhere tab (overlay-surface.md
        // Pointer: "Tab click switches tabs").
        HitTarget::ModalTab { index } => modal_press(model, ModalMsg::ActivateTab(*index)),

        // Cursor-anchored popup: consume the click without dismissing the
        // popup or moving the text cursor (overlay-surface.md Phase 5). Row
        // clicks update the popup's own selection; the Completion popup
        // additionally accepts on click (the mouse-driven equivalent of
        // Enter — otherwise the real popup could only ever be used with the
        // keyboard).
        HitTarget::CursorOverlay { flat_index } => handle_cursor_overlay_click(model, *flat_index),
        HitTarget::CursorOverlayDocumentation { scrollbar, .. } => {
            if let (Some(geometry), Some(overlay)) = (scrollbar, model.ui.cursor_overlay) {
                overlay_scrollbar_press(
                    model,
                    ScrollbarTarget::Documentation {
                        kind: overlay.kind,
                        selected: overlay.selected,
                    },
                    geometry,
                    event,
                )
            } else {
                EventResult::consumed_no_redraw()
            }
        }

        // Status bar - consume but do nothing
        HitTarget::StatusBar => EventResult::consumed_no_redraw(),

        // Sidebar resize handle
        HitTarget::SidebarResize => {
            update(
                model,
                Msg::Workspace(WorkspaceMsg::StartSidebarResize {
                    initial_x: event.pos.x,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Sidebar empty area
        HitTarget::SidebarEmpty => {
            EventResult::consumed_with_focus(FocusTarget::Dock(DockPosition::Left))
        }

        // Sidebar item
        HitTarget::SidebarItem {
            path,
            row,
            is_dir,
            clicked_on_chevron,
        } => {
            // Track clicks for double-click detection
            let click_count = click_tracker.track_click(ClickRegion::Sidebar { row: *row });

            // Always select the item
            update(
                model,
                Msg::Workspace(WorkspaceMsg::SelectItem(path.clone())),
            );

            // Chevron click immediately toggles folder
            if *clicked_on_chevron {
                update(
                    model,
                    Msg::Workspace(WorkspaceMsg::ToggleFolder(path.clone())),
                );
                return EventResult::consumed_with_focus(FocusTarget::Dock(DockPosition::Left));
            }

            // Double-click opens file or toggles folder
            if click_count >= 2 {
                let cmd = if *is_dir {
                    update(
                        model,
                        Msg::Workspace(WorkspaceMsg::ToggleFolder(path.clone())),
                    )
                } else {
                    update(
                        model,
                        Msg::Workspace(WorkspaceMsg::OpenFile {
                            path: path.clone(),
                            preview: false,
                        }),
                    )
                };
                // Return the command from opening the file (includes syntax parse)
                return EventResult::consumed_with_cmd(cmd, FocusTarget::Dock(DockPosition::Left));
            }

            EventResult::consumed_with_focus(FocusTarget::Dock(DockPosition::Left))
        }

        // Splitter drag
        HitTarget::Splitter { index, .. } => {
            update(
                model,
                Msg::Layout(LayoutMsg::BeginSplitterDrag {
                    splitter_index: *index,
                    position: (event.pos.x as f32, event.pos.y as f32),
                }),
            );
            EventResult::consumed_redraw()
        }

        // Preview pane header - consume, keep editor focus
        HitTarget::PreviewHeader { .. } => {
            // Just consume - middle-click closes
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Preview pane content - consume, keep editor focus for keyboard
        HitTarget::PreviewContent { .. } => {
            // Webview handles its own clicks; just keep editor focus
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Tab click
        HitTarget::GroupTab {
            group_id,
            tab_id,
            tab_index,
        } => {
            // Focus group if not already focused
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(model, Msg::Layout(LayoutMsg::SwitchToTab(*tab_index)));
            // Arm a potential tab drag (activates past the move threshold)
            model.ui.tab_drag = Some(token::model::ui::TabDragState {
                tab_id: *tab_id,
                press: (event.pos.x, event.pos.y),
                current: (event.pos.x, event.pos.y),
                active: false,
            });
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Empty tab bar area
        HitTarget::GroupTabBarEmpty { group_id } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Editor gutter: interactive lanes (fold/marks) dispatch to their
        // owning feature instead of falling through to the default
        // focus/drag-select behavior (editor-decorations.md). Folding handles
        // its disclosure lane; other interactive lanes consume the press.
        HitTarget::EditorGutter {
            group_id,
            editor_id,
            line,
            lane,
        } => {
            if *lane == Some(token::view::geometry::LaneId::Fold) {
                let mut commands = Vec::new();
                if *group_id != model.editor_area.focused_group_id {
                    commands.extend(update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id))));
                }
                commands.extend(update(
                    model,
                    Msg::Editor(token::messages::EditorMsg::Fold {
                        editor_id: Some(*editor_id),
                        header: Some(*line),
                        action: token::folding::FoldAction::Toggle,
                    }),
                ));
                EventResult::consumed_with_cmd(
                    Some(token::commands::Cmd::Batch(commands)),
                    FocusTarget::Editor,
                )
            } else {
                match interactive_gutter_lane_click(*lane) {
                    Some(result) => result,
                    None => {
                        if *group_id != model.editor_area.focused_group_id {
                            update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
                        }
                        EventResult::consumed_with_focus(FocusTarget::Editor)
                    }
                }
            }
        }

        // Editor content - handled specially due to complex selection logic
        HitTarget::EditorContent { group_id, .. } => {
            handle_editor_content_click(model, *group_id, event, click_tracker)
        }

        // CSV cell click - use renderer to find actual cell
        HitTarget::CsvCell { group_id, .. } => {
            use token::messages::CsvMsg;

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            // Use renderer to find the actual cell at this position; all
            // select / caret / commit / edit branching lives in
            // `update::csv::click_cell`.
            if let Some(hit) = renderer.pixel_to_csv_cell(event.pos.x, event.pos.y, model) {
                let click_count = click_tracker.track_click(ClickRegion::CsvCell {
                    group: *group_id,
                    row: hit.position.row,
                    col: hit.position.col,
                });
                update(
                    model,
                    Msg::Csv(CsvMsg::ClickCell {
                        row: hit.position.row,
                        col: hit.position.col,
                        x_in_cell: hit.x_in_cell,
                        click_count,
                        extend_selection: event.shift(),
                    }),
                );
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Image content - start panning
        HitTarget::ImageContent { group_id, .. } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(
                model,
                Msg::Image(ImageMsg::StartPan {
                    x: event.pos.x,
                    y: event.pos.y,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Binary placeholder "Open with Default Application" button
        // Scrollbar thumb: begin drag
        HitTarget::ScrollbarThumbVertical {
            editor_id,
            grab_offset,
            track_y,
            track_h,
            thumb_h,
            max_scroll,
            ..
        } => {
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarThumbPressed(ScrollbarDragState {
                    target: ScrollbarTarget::Editor(*editor_id),
                    axis: ScrollbarDragAxis::Vertical,
                    grab_offset: *grab_offset,
                    track_start: *track_y,
                    track_size: *track_h,
                    thumb_size: *thumb_h,
                    max_scroll: *max_scroll,
                })),
            );
            EventResult::consumed_redraw()
        }

        HitTarget::ScrollbarThumbHorizontal {
            editor_id,
            grab_offset,
            track_x,
            track_w,
            thumb_w,
            max_scroll,
            ..
        } => {
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarThumbPressed(ScrollbarDragState {
                    target: ScrollbarTarget::Editor(*editor_id),
                    axis: ScrollbarDragAxis::Horizontal,
                    grab_offset: *grab_offset,
                    track_start: *track_x,
                    track_size: *track_w,
                    thumb_size: *thumb_w,
                    max_scroll: *max_scroll,
                })),
            );
            EventResult::consumed_redraw()
        }

        // Scrollbar track: click to jump
        HitTarget::ScrollbarTrackVertical {
            editor_id,
            coord,
            track_y,
            track_h,
            thumb_h,
            max_scroll,
            ..
        } => {
            let new_position = token::view::scrollbar::position_from_track_click(
                *coord,
                *track_y,
                *track_h,
                *thumb_h,
                *max_scroll,
            );
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarTrackClicked {
                    target: ScrollbarTarget::Editor(*editor_id),
                    axis: ScrollbarDragAxis::Vertical,
                    new_position,
                }),
            );
            EventResult::consumed_redraw()
        }

        HitTarget::ScrollbarTrackHorizontal {
            editor_id,
            coord,
            track_x,
            track_w,
            thumb_w,
            max_scroll,
            ..
        } => {
            let new_position = token::view::scrollbar::position_from_track_click(
                *coord,
                *track_x,
                *track_w,
                *thumb_w,
                *max_scroll,
            );
            update(
                model,
                Msg::Ui(UiMsg::ScrollbarTrackClicked {
                    target: ScrollbarTarget::Editor(*editor_id),
                    axis: ScrollbarDragAxis::Horizontal,
                    new_position,
                }),
            );
            EventResult::consumed_redraw()
        }

        HitTarget::BinaryPlaceholderButton { group_id } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            if let Some(editor) = model.editor_area.focused_editor() {
                if let token::model::TabContent::BinaryPlaceholder(ref state) = editor.tab_content {
                    let path = state.path.clone();
                    update(model, Msg::Layout(LayoutMsg::OpenWithDefaultApp(path)));
                }
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Dock resize handle
        HitTarget::DockResize { position } => {
            let initial_coord = match position {
                token::panel::DockPosition::Bottom => event.pos.y,
                _ => event.pos.x,
            };
            update(
                model,
                Msg::Dock(token::messages::DockMsg::StartResize {
                    position: *position,
                    initial_coord,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Dock tab click - activate panel (never toggles the dock closed)
        HitTarget::TerminalAction { action, position } => EventResult::Consumed {
            redraw: true,
            focus: Some(FocusTarget::Dock(*position)),
            cmd: update(model, Msg::Terminal(TerminalMsg::Tab(*action))),
        },
        HitTarget::DockTab { panel_id, .. } => {
            update(
                model,
                Msg::Dock(token::messages::DockMsg::ActivatePanel(*panel_id)),
            );
            EventResult::consumed_redraw()
        }

        // Dock tab bar empty area
        HitTarget::DockTabBarEmpty { position } => {
            // Focus the dock
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusDock(*position)),
            );
            EventResult::consumed_redraw()
        }

        // Dock content area - handle panel-specific interactions
        HitTarget::DockContent {
            position,
            active_panel_id,
        } => {
            // Focus the dock first
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusDock(*position)),
            );

            if *active_panel_id == token::panel::PanelId::Terminal {
                // Resolve again at the click, never open a cached hover URL.
                if let Some((_, link)) = terminal_link_at_pointer(
                    model,
                    Some(target),
                    event.pos.x,
                    event.pos.y,
                    event.modifiers,
                ) {
                    return EventResult::consumed_with_cmd(
                        Some(Cmd::OpenWebUrl(link.uri)),
                        FocusTarget::Dock(*position),
                    );
                }
                use alacritty_terminal::selection::SelectionType;
                let Some(viewport) = token::panels::terminal::TerminalViewport::for_model(model)
                else {
                    return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
                };
                let (point, side) = viewport.point_at(event.pos.x, event.pos.y);
                let Some(session) = model.terminal.active_session() else {
                    return EventResult::consumed_no_redraw();
                };
                let clicks = click_tracker.track_click(ClickRegion::Terminal {
                    session: session.id,
                    point,
                });
                let kind = match clicks {
                    2 => SelectionType::Semantic,
                    3 => SelectionType::Lines,
                    _ => SelectionType::Simple,
                };
                return EventResult::Consumed {
                    redraw: true,
                    focus: Some(FocusTarget::Dock(*position)),
                    cmd: update(
                        model,
                        Msg::Terminal(TerminalMsg::SelectionStart { point, side, kind }),
                    ),
                };
            }

            // Handle outline panel clicks — row geometry from the same
            // solved chrome the renderer painted, wherever the panel is
            // docked.
            if *active_panel_id == token::panel::PanelId::Outline {
                use token::layout::UiKey;
                use token::messages::OutlineMsg;

                let chrome = token::layout::chrome::chrome(model);
                let Some(rows) = chrome.row_list(UiKey::PanelRows(token::panel::PanelId::Outline))
                else {
                    return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
                };

                if let Some(clicked_index) = rows.row_at_y(event.pos.y as f32) {
                    let outline = model
                        .editor_area
                        .focused_document()
                        .and_then(|doc| doc.outline.as_ref());

                    if let Some(outline) = outline {
                        if let Some(row) = visible_tree_row_at_index(
                            &outline.roots,
                            clicked_index,
                            |node: &token::outline::OutlineNode| {
                                node.is_collapsible() && !model.outline_panel.is_collapsed(node)
                            },
                        ) {
                            let tree = token::view::geometry::TreeRowLayout::outline_from_metrics(
                                &model.metrics,
                            );
                            let on_chevron = row.node.is_collapsible()
                                && tree.is_on_chevron(rows.rect().x, row.depth, event.pos.x as f32);

                            let click_count = click_tracker
                                .track_click(ClickRegion::Outline { row: clicked_index });

                            update(
                                model,
                                Msg::Outline(OutlineMsg::ClickRow {
                                    index: clicked_index,
                                    click_count,
                                    on_chevron,
                                }),
                            );
                        }
                    }
                }

                return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
            }

            // Handle problems panel clicks — same solved-chrome geometry.
            if *active_panel_id == token::panel::PanelId::Problems {
                use token::layout::UiKey;
                use token::messages::ProblemsMsg;
                use token::update::problems::problems_rows;

                let chrome = token::layout::chrome::chrome(model);
                let Some(rows_view) =
                    chrome.row_list(UiKey::PanelRows(token::panel::PanelId::Problems))
                else {
                    return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
                };

                if let Some(clicked_index) = rows_view.row_at_y(event.pos.y as f32) {
                    let rows = problems_rows(model);
                    if let Some(row) = rows.get(clicked_index) {
                        // Only File rows (depth 0) have a chevron.
                        let tree = token::view::geometry::TreeRowLayout::outline_from_metrics(
                            &model.metrics,
                        );
                        let on_chevron =
                            matches!(row, token::update::problems::ProblemsRow::File { .. })
                                && tree.is_on_chevron(rows_view.rect().x, 0, event.pos.x as f32);

                        let click_count =
                            click_tracker.track_click(ClickRegion::Problems { row: clicked_index });
                        update(
                            model,
                            Msg::Problems(ProblemsMsg::ClickRow {
                                index: clicked_index,
                                click_count,
                                on_chevron,
                            }),
                        );
                    }
                }

                return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
            }

            if *active_panel_id == token::panel::PanelId::Usages {
                let chrome = token::layout::chrome::chrome(model);
                if let Some(view) = chrome.row_list(token::layout::UiKey::PanelRows(
                    token::panel::PanelId::Usages,
                )) {
                    if let Some(index) = view.row_at_y(event.pos.y as f32) {
                        let rows = model.usages_panel.rows();
                        let tree = token::view::geometry::TreeRowLayout::outline_from_metrics(
                            &model.metrics,
                        );
                        let on_chevron =
                            matches!(
                                rows.get(index),
                                Some(token::model::usages::UsagesRow::File { .. })
                            ) && tree.is_on_chevron(view.rect().x, 0, event.pos.x as f32);
                        let click_count =
                            click_tracker.track_click(ClickRegion::Usages { row: index });
                        model.ui.focus = FocusTarget::Dock(*position);
                        let cmd = update(
                            model,
                            Msg::Usages(token::messages::UsagesMsg::ClickRow {
                                index,
                                click_count,
                                on_chevron,
                            }),
                        );
                        return EventResult::consumed_with_cmd(cmd, model.ui.focus);
                    }
                }
                return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
            }

            // The left dock hosts the file explorer; other dock content handled
            // above has already returned with its dock focus.
            match position {
                token::panel::DockPosition::Left => {
                    EventResult::consumed_with_focus(FocusTarget::Dock(*position))
                }
                _ => EventResult::consumed_redraw(),
            }
        }
    }
}

/// Resolve the click against the visible layout before focus changes move
/// docked controls or resize wrapped text. Middle clicks use visual columns for
/// rectangle selection; left/right clicks use document character columns.
fn focus_editor_at_point(
    model: &mut AppModel,
    group_id: GroupId,
    event: &MouseEvent,
) -> Option<(usize, usize)> {
    let position = model.editor_area.groups.get(&group_id).and_then(|group| {
        let editor = model.editor_area.editors.get(&group.active_editor_id()?)?;
        if !editor.is_plain_text_mode() {
            return None;
        }
        let document = model.editor_area.document_for_group(group)?;
        let layout = token::view::geometry::GroupLayout::new(group, model, model.char_width);
        Some(if event.button == MouseButton::Middle {
            layout.pixel_to_line_and_visual_column(
                event.pos.x,
                event.pos.y,
                model.char_width,
                model.line_height as f64,
                editor,
                document,
            )
        } else {
            layout.pixel_to_cursor(
                event.pos.x,
                event.pos.y,
                model.char_width,
                model.line_height as f64,
                editor,
                document,
            )
        })
    });
    if group_id != model.editor_area.focused_group_id {
        update(model, Msg::Layout(LayoutMsg::FocusGroup(group_id)));
    }
    position
}

/// Handle editor content click with full selection logic
fn handle_editor_content_click(
    model: &mut AppModel,
    group_id: GroupId,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    use token::messages::EditorMsg;
    use token::model::FocusTarget;

    let position = focus_editor_at_point(model, group_id, event);

    // Non-text tabs: double-click opens binary placeholder with default app, ignore other clicks
    if let Some(editor) = model.editor_area.focused_editor() {
        match &editor.tab_content {
            token::model::TabContent::BinaryPlaceholder(state) => {
                let click_count =
                    click_tracker.track_click(ClickRegion::BinaryPlaceholder { group: group_id });
                if click_count >= 2 {
                    let path = state.path.clone();
                    update(model, Msg::Layout(LayoutMsg::OpenWithDefaultApp(path)));
                }
                return EventResult::consumed_with_focus(FocusTarget::Editor);
            }
            token::model::TabContent::Text => {}
        }
    }

    let Some((line, column)) = position else {
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    };

    // Track clicks for double/triple detection
    let click_count = click_tracker.track_click(ClickRegion::Editor {
        group: group_id,
        line,
        column,
    });

    // Handle modifiers
    if event.cmd() {
        // Cmd+Click = go to definition at the clicked position (JetBrains /
        // VS Code convention: the caret moves to the click first).
        update(
            model,
            Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
        );
        update(model, Msg::Lsp(token::messages::LspMsg::GotoDefinition));
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    if event.shift() {
        update(
            model,
            Msg::Editor(EditorMsg::ExtendSelectionToPosition { line, column }),
        );
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    if event.alt() {
        update(
            model,
            Msg::Editor(EditorMsg::ToggleCursorAtPosition { line, column }),
        );
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    // Handle click count
    match click_count {
        2 => {
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
            update(model, Msg::Editor(EditorMsg::SelectWord));
        }
        3 => {
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
            update(model, Msg::Editor(EditorMsg::SelectLine));
        }
        _ => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
        }
    }

    EventResult::consumed_with_focus(FocusTarget::Editor)
}

/// Handle middle mouse button clicks
fn handle_middle_click(
    model: &mut AppModel,
    target: &HitTarget,
    event: &MouseEvent,
) -> EventResult {
    match target {
        // Status bar - ignore
        HitTarget::StatusBar => EventResult::consumed_no_redraw(),

        // Preview header - middle click closes preview
        HitTarget::PreviewHeader { .. } => {
            update(model, Msg::Preview(PreviewMsg::Close));
            EventResult::consumed_redraw()
        }

        // Preview content - consume but no action (webview handles its own)
        HitTarget::PreviewContent { .. } => EventResult::consumed_no_redraw(),

        // Tab - middle click closes tab
        HitTarget::GroupTab {
            group_id, tab_id, ..
        } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(model, Msg::Layout(LayoutMsg::CloseTab(*tab_id)));
            EventResult::consumed_redraw()
        }

        // Empty tab bar area - consume but no action
        HitTarget::GroupTabBarEmpty { .. } => EventResult::consumed_no_redraw(),

        // Editor gutter - treat like editor content for rectangle selection,
        // unless an interactive lane (fold/marks) owns the click.
        HitTarget::EditorGutter { group_id, .. } | HitTarget::EditorContent { group_id, .. } => {
            if let HitTarget::EditorGutter { lane, .. } = target {
                if let Some(result) = interactive_gutter_lane_click(*lane) {
                    return result;
                }
            }
            let Some((line, visual_col)) = focus_editor_at_point(model, *group_id, event) else {
                return EventResult::consumed_no_redraw();
            };
            update(
                model,
                Msg::Editor(EditorMsg::StartRectangleSelection { line, visual_col }),
            );
            EventResult::consumed_redraw()
        }

        // CSV cell - no middle-click behavior
        HitTarget::CsvCell { .. } => EventResult::consumed_no_redraw(),

        // Modal - consume, no action
        HitTarget::FindBar { .. }
        | HitTarget::Modal { .. }
        | HitTarget::ModalScrollbar { .. }
        | HitTarget::ModalRow { .. }
        | HitTarget::ModalChoice { .. }
        | HitTarget::ModalField { .. }
        | HitTarget::ModalTab { .. } => EventResult::consumed_no_redraw(),

        // Sidebar targets - consume, no action for middle-click
        HitTarget::SidebarEmpty | HitTarget::SidebarItem { .. } => {
            EventResult::consumed_no_redraw()
        }

        // Sidebar resize and splitters - consume, no action
        HitTarget::SidebarResize | HitTarget::Splitter { .. } => EventResult::consumed_no_redraw(),

        // Dock targets - consume, no special middle-click action
        HitTarget::DockResize { .. }
        | HitTarget::DockTab { .. }
        | HitTarget::TerminalAction { .. }
        | HitTarget::DockTabBarEmpty { .. }
        | HitTarget::DockContent { .. } => EventResult::consumed_no_redraw(),

        // Binary placeholder button - no middle-click action
        HitTarget::BinaryPlaceholderButton { .. } => EventResult::consumed_no_redraw(),

        // Image content and scrollbars - no middle-click action
        HitTarget::ImageContent { .. }
        | HitTarget::ScrollbarThumbVertical { .. }
        | HitTarget::ScrollbarTrackVertical { .. }
        | HitTarget::ScrollbarThumbHorizontal { .. }
        | HitTarget::ScrollbarTrackHorizontal { .. } => EventResult::consumed_no_redraw(),

        // Cursor overlay - no middle-click action
        HitTarget::CursorOverlay { .. } | HitTarget::CursorOverlayDocumentation { .. } => {
            EventResult::consumed_no_redraw()
        }
    }
}

/// Handle right mouse button clicks (context menus - future)
/// Synchronous clipboard-content check for the context menu's Paste-
/// enablement gate (context-menu.md's `ContextMenuTarget::Editor::
/// clipboard_has_content`). Unlike `Cmd::RequestClipboardPaste` (which
/// round-trips through a worker thread — see `App`'s `Cmd` executor), this
/// reads inline on the UI thread: a menu builder needs the answer before
/// it can render a single frame, and a local clipboard read is a fast
/// syscall, not a blocking one.
/// ponytail: main-thread clipboard read; move off-thread if a slow/huge
/// clipboard payload is ever observed to jank right-click.
fn clipboard_has_content() -> bool {
    arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .is_ok_and(|text| !text.is_empty())
}

/// Resolve `target`/`event` into a `ContextMenuTarget` for the three V1
/// regions (editor content, tab, file-tree item) and open the menu.
/// Regions with no V1 menu (status bar, dock, ...) keep bubbling
/// (context-menu.md "Phase 2: Hit-Test Wiring & Open/Close").
fn handle_right_click(model: &mut AppModel, target: &HitTarget, event: &MouseEvent) -> EventResult {
    use token::context_menu::ContextMenuTarget;
    use token::messages::ContextMenuMsg;

    let menu_target = match target {
        HitTarget::EditorContent {
            group_id,
            editor_id,
            ..
        } => {
            // The menu is built (enablement) and later activated against
            // the clicked split, not whatever happened to be focused
            // before the click — focus it first, same as a left click
            // (`handle_editor_content_click`). Per JetBrains/VS Code
            // convention, also move the caret to the click point, unless
            // the click landed inside the existing selection (which must
            // survive so Cut/Copy still act on it).
            let caret_target =
                focus_editor_at_point(model, *group_id, event).filter(|&(line, column)| {
                    let pos = token::model::editor::Position::new(line, column);
                    model
                        .editor_area
                        .editors
                        .get(editor_id)
                        .is_some_and(|editor| !editor.active_selection().contains(pos))
                });
            if let Some((line, column)) = caret_target {
                update(
                    model,
                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                );
            }

            let has_selection = model
                .editor_area
                .editors
                .get(editor_id)
                .is_some_and(|e| !e.active_selection().is_empty());
            ContextMenuTarget::Editor {
                group_id: *group_id,
                has_selection,
                clipboard_has_content: clipboard_has_content(),
            }
        }
        HitTarget::GroupTab {
            group_id, tab_id, ..
        } => ContextMenuTarget::Tab {
            group_id: *group_id,
            tab_id: *tab_id,
            file_path: tab_file_path(model, *group_id, *tab_id),
        },
        HitTarget::SidebarItem { path, is_dir, .. } => ContextMenuTarget::FileTreeItem {
            path: path.clone(),
            is_dir: *is_dir,
        },
        _ => return EventResult::Bubble,
    };

    let anchor = (event.pos.x as usize, event.pos.y as usize, 0);
    let cmd = update(
        model,
        Msg::ContextMenu(ContextMenuMsg::Open {
            target: menu_target,
            anchor,
        }),
    );
    match cmd {
        Some(cmd) => EventResult::Consumed {
            redraw: false,
            focus: None,
            cmd: Some(cmd),
        },
        // `has_modal()` guard tripped inside `update_context_menu` — still
        // consume the click (no menu opened, nothing else should act on
        // it either).
        None => EventResult::consumed_no_redraw(),
    }
}

/// The file path backing `tab_id` in `group_id`, if any — `None` covers
/// both "untitled buffer" and "tab not found" (defensive; the tab that was
/// just right-clicked should always resolve).
fn tab_file_path(
    model: &AppModel,
    group_id: GroupId,
    tab_id: token::model::TabId,
) -> Option<std::path::PathBuf> {
    let group = model.editor_area.groups.get(&group_id)?;
    let tab = group.tabs.iter().find(|t| t.id == tab_id)?;
    let editor = model.editor_area.editors.get(&tab.editor_id)?;
    let document_id = editor.document_id?;
    model
        .editor_area
        .documents
        .get(&document_id)
        .and_then(|d| d.file_path.clone())
}

/// Horizontal `delta_px` for scrolling the editor tab strip from a wheel event,
/// or `None` when neither axis moved.
///
/// A horizontal gesture maps directly: positive `h_delta` reveals tabs further
/// right, matching editor horizontal scrolling (and `ScrollTabBar`, which
/// increases `tab_scroll` for positive `delta_px`). A vertical-only wheel (a
/// plain mouse with no X axis) is repurposed to scroll the strip, keeping the
/// legacy inverted sign so mouse-wheel behavior over the tabs is unchanged.
fn tab_bar_scroll_delta_px(h_delta: i32, v_delta: i32, scroll_step: i32) -> Option<i32> {
    if h_delta != 0 {
        Some(h_delta * scroll_step)
    } else if v_delta != 0 {
        Some(-v_delta * scroll_step)
    } else {
        None
    }
}

/// Handle mouse wheel scroll events, routing to the appropriate target
/// based on the current hover region.
/// Row-oriented widgets keep accumulated row deltas. Text panes consume the
/// original pixel displacement, without rounding it to a row or column first.
#[derive(Clone, Copy)]
pub(super) struct WheelScroll {
    pub rows: (i32, i32),
    pub pixels: Option<(f64, f64)>,
    pub animated: bool,
}

impl From<(i32, i32)> for WheelScroll {
    fn from(rows: (i32, i32)) -> Self {
        Self {
            rows,
            pixels: None,
            animated: false,
        }
    }
}

pub(super) fn handle_mouse_wheel(
    model: &mut AppModel,
    mouse_position: Option<(f64, f64)>,
    scroll: WheelScroll,
    measure: Option<&mut dyn token::layout::TextMeasure>,
) -> Option<Cmd> {
    let (_, v_delta) = scroll.rows;
    if model.terminal.selection_drag.is_some() {
        let scroll = if v_delta < 0 {
            TerminalMsg::ScrollUp(v_delta.unsigned_abs() as usize)
        } else {
            TerminalMsg::ScrollDown(v_delta as usize)
        };
        let cmd = update(model, Msg::Terminal(scroll));
        let selection = mouse_position.and_then(|(x, y)| update_terminal_selection(model, x, y));
        return merge(cmd, selection);
    }
    // Re-hit-test with current font/window geometry: a resize or a new reply
    // can move the card without moving the pointer. Never use stale row bounds.
    let mut hover_changed = false;
    if model.ui.has_visible_completion() || model.ui.has_documentation() {
        if let (Some(measure), Some((x, y))) = (measure, mouse_position) {
            let target = token::view::hit_test::hit_test_ui(
                model,
                token::view::hit_test::Point::new(x, y),
                model.char_width,
                measure,
            );
            hover_changed = update_hover_target(model, target.as_ref());
            if let Some(HitTarget::CursorOverlayDocumentation { viewport, .. }) = target {
                let scroll = viewport.scrolled((v_delta.signum() * 3) as isize);
                let cmd = update(model, Msg::Ui(UiMsg::DocumentationScrolled(scroll)));
                return merge(hover_changed.then_some(Cmd::Redraw), cmd);
            }
        }
    }
    let cmd = scroll_hovered_region(model, mouse_position, scroll);
    merge(hover_changed.then_some(Cmd::Redraw), cmd)
}

/// Extend a captured terminal selection through the same viewport that paints it.
pub(super) fn update_terminal_selection(model: &mut AppModel, x: f64, y: f64) -> Option<Cmd> {
    model.terminal.selection_drag?;
    let Some(viewport) = token::panels::terminal::TerminalViewport::for_model(model) else {
        model.terminal.selection_drag = None;
        return None;
    };
    let (point, side) = viewport.point_at(x, y);
    update(
        model,
        Msg::Terminal(TerminalMsg::SelectionUpdate { point, side }),
    )
}

fn merge(a: Option<Cmd>, b: Option<Cmd>) -> Option<Cmd> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Cmd::Batch(vec![a, b])),
        (a, b) => a.or(b),
    }
}

fn scroll_hovered_region(
    model: &mut AppModel,
    mouse_position: Option<(f64, f64)>,
    scroll: WheelScroll,
) -> Option<Cmd> {
    use token::model::HoverRegion;
    let (h_delta, v_delta) = scroll.rows;
    match model.ui.hover {
        // Sidebar: scroll the file tree
        HoverRegion::Sidebar => {
            if v_delta != 0 {
                update(
                    model,
                    Msg::Workspace(WorkspaceMsg::Scroll { lines: v_delta }),
                )
            } else {
                None
            }
        }

        // Dock panels: route to panel-specific scroll handlers
        HoverRegion::Dock(position) => {
            let active_panel = match position {
                token::panel::DockPosition::Left => model.dock_layout.left.active_panel(),
                token::panel::DockPosition::Right => model.dock_layout.right.active_panel(),
                token::panel::DockPosition::Bottom => model.dock_layout.bottom.active_panel(),
            };
            if active_panel == Some(token::panel::PanelId::Outline) && v_delta != 0 {
                update(model, Msg::Outline(OutlineMsg::Scroll { lines: v_delta }))
            } else if active_panel == Some(token::panel::PanelId::PROBLEMS) && v_delta != 0 {
                update(
                    model,
                    Msg::Problems(token::messages::ProblemsMsg::Scroll { lines: v_delta }),
                )
            } else if active_panel == Some(token::panel::PanelId::TERMINAL) && v_delta != 0 {
                let lines = v_delta.unsigned_abs() as usize;
                let msg = if v_delta < 0 {
                    TerminalMsg::ScrollUp(lines)
                } else {
                    TerminalMsg::ScrollDown(lines)
                };
                update(model, Msg::Terminal(msg))
            } else if active_panel == Some(token::panel::PanelId::Usages) && v_delta != 0 {
                update(
                    model,
                    Msg::Usages(token::messages::UsagesMsg::Scroll { lines: v_delta }),
                )
            } else {
                None
            }
        }
        // Preview panes: webview handles its own scrolling
        HoverRegion::Preview => None,

        // Editor tab bar: scroll the tabs horizontally
        HoverRegion::EditorTabBar => {
            let scroll_step = (model.line_height as i32).max(1);
            let delta_px = tab_bar_scroll_delta_px(h_delta, v_delta, scroll_step)?;
            // Find which group's tab bar is under the cursor
            let (x, y) = mouse_position?;
            let pt = token::view::hit_test::Point::new(x, y);
            let group_id = model.editor_area.groups.values().find_map(|group| {
                let layout =
                    token::layout::editor::EditorTabBarLayout::new(group, model, model.char_width);
                layout.contains(pt.x, pt.y).then_some(group.id)
            })?;
            update(
                model,
                Msg::Layout(LayoutMsg::ScrollTabBar { group_id, delta_px }),
            )
        }

        // Modal: scroll the visible window by 3 rows, selection unchanged
        // (overlay-surface.md Pointer: "Scroll wheel moves the viewport by
        // 3 rows without moving selection").
        HoverRegion::Modal => {
            if v_delta == 0 {
                return None;
            }
            let rows = if matches!(
                model.ui.active_modal,
                Some(token::model::ModalState::Settings(_))
            ) {
                v_delta as isize
            } else {
                (v_delta.signum() * 3) as isize
            };
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Scroll(rows))))
        }

        // Cursor overlay: scroll its own window, same 3-rows-per-notch
        // convention as modals (overlay-surface.md Phase 5).
        HoverRegion::CursorOverlay => {
            if v_delta == 0 {
                return None;
            }
            let completion_rows = model
                .ui
                .completion_menu
                .as_ref()
                .map(|m| m.filtered.len())
                .unwrap_or(0);
            let reference_rows = model.ui.reference_list.as_ref().map_or(0, Vec::len);
            let code_action_rows = model.ui.code_action_list.as_ref().map_or(0, Vec::len);
            let Some(state) = &mut model.ui.cursor_overlay else {
                return None;
            };
            let max_scroll = match state.kind {
                token::model::CursorOverlayKind::DebugCompletion => {
                    token::view::modal::debug_completion_row_count()
                        .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION)
                }
                token::model::CursorOverlayKind::DebugHover
                | token::model::CursorOverlayKind::Hover => 0,
                token::model::CursorOverlayKind::Completion => completion_rows
                    .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION),
                token::model::CursorOverlayKind::References => reference_rows
                    .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION),
                token::model::CursorOverlayKind::CodeActions => code_action_rows
                    .saturating_sub(token::view::overlay_surface::MAX_VISIBLE_COMPLETION),
                // No scroll behavior needed for V1 (menus fit without
                // scrolling) — inert, same as Hover (context-menu.md
                // "Mouse: click-away consumes").
                token::model::CursorOverlayKind::ContextMenu => 0,
            };
            let delta = v_delta.signum() * 3;
            state.scroll = if delta < 0 {
                state.scroll.saturating_sub(delta.unsigned_abs() as usize)
            } else {
                state.scroll.saturating_add(delta as usize).min(max_scroll)
            };
            Some(Cmd::Redraw)
        }

        // StatusBar/Splitter/DockResize/Button: ignore scroll
        HoverRegion::StatusBar
        | HoverRegion::FindBar(_)
        | HoverRegion::Splitter
        | HoverRegion::SidebarResize
        | HoverRegion::DockResize(_)
        | HoverRegion::Button(_)
        | HoverRegion::None => None,

        // Editor text area: scroll the editor or delegate to specialized modes.
        HoverRegion::EditorText => {
            // Scrolling the editor moves the text out from under the
            // completion popup's anchor; the popup would visually detach
            // from its word. Dismiss it instead (the anchor clamps to the
            // content edge, so keeping it open reads as a stale artifact).
            // The dismissal's own Cmd is merged into whichever scroll
            // command this arm returns, so a dismissal with no scroll
            // still repaints.
            let completion_dismiss = if model.ui.completion_menu.is_some() {
                update(model, Msg::Completion(CompletionMsg::Dismiss))
            } else {
                None
            };

            if let Some((delta_x, delta_y)) = scroll.pixels {
                let target = mouse_position
                    .and_then(|(x, y)| {
                        model
                            .editor_area
                            .groups
                            .values()
                            .find(|group| group.rect.contains(x as f32, y as f32))
                            .and_then(|group| group.active_editor_id())
                    })
                    .or_else(|| model.editor_area.focused_editor_id());
                if let Some(editor_id) = target.filter(|id| {
                    model
                        .editor_area
                        .editors
                        .get(id)
                        .is_some_and(|editor| editor.is_plain_text_mode())
                }) {
                    return merge(
                        completion_dismiss,
                        update(
                            model,
                            Msg::Editor(EditorMsg::ScrollPixels {
                                editor_id,
                                delta_x,
                                delta_y,
                                animated: scroll.animated,
                            }),
                        ),
                    );
                }
            }

            let in_image_mode = model
                .editor_area
                .focused_editor()
                .map(|e| e.view_mode.is_image())
                .unwrap_or(false);

            if in_image_mode {
                if v_delta != 0 {
                    let (mouse_x, mouse_y) = mouse_position.unwrap_or((0.0, 0.0));
                    return merge(
                        completion_dismiss,
                        update(
                            model,
                            Msg::Image(ImageMsg::Zoom {
                                delta: v_delta as f64,
                                mouse_x,
                                mouse_y,
                            }),
                        ),
                    );
                }
                return completion_dismiss;
            }

            let in_csv_mode = model
                .editor_area
                .focused_editor()
                .map(|e| e.view_mode.is_csv())
                .unwrap_or(false);

            if in_csv_mode {
                let v_cmd = if v_delta != 0 {
                    update(model, Msg::Csv(CsvMsg::ScrollVertical(v_delta)))
                } else {
                    None
                };
                let h_cmd = if h_delta != 0 {
                    update(model, Msg::Csv(CsvMsg::ScrollHorizontal(h_delta)))
                } else {
                    None
                };
                return merge(completion_dismiss, v_cmd.or(h_cmd));
            }

            let v_cmd = if v_delta != 0 {
                update(model, Msg::Editor(EditorMsg::Scroll(v_delta)))
            } else {
                None
            };
            let h_cmd = if h_delta != 0 {
                update(model, Msg::Editor(EditorMsg::ScrollHorizontal(h_delta)))
            } else {
                None
            };
            merge(completion_dismiss, v_cmd.or(h_cmd))
        }
    }
}
