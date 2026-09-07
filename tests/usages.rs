mod common;

use std::sync::Arc;
use token::messages::{DockMsg, LayoutMsg, LspMsg, Msg, ReferencesOutcome, UsagesMsg};
use token::model::usages::{ReferencesTarget, UsagesRow, MAX_REFERENCE_LOCATIONS};
use token::model::{DocumentId, FocusTarget, Position};
use token::panel::{DockPosition, PanelId};
use token::update::navigation::LocationItem;
use token::update::update;
use token::{AppModel, Cmd};

fn fixture() -> AppModel {
    let mut model = common::test_model("fn symbol() {}\n", 0, 3);
    model.document_mut().file_path = Some("/source.rs".into());
    model
}

fn capture(cmd: Cmd) -> Option<(ReferencesTarget, DocumentId, u64, Position)> {
    match cmd {
        Cmd::LspRequestReferences {
            target,
            document_id,
            revision,
            cursor,
            ..
        } => Some((target, document_id, revision, cursor)),
        Cmd::Batch(commands) => commands.into_iter().find_map(capture),
        _ => None,
    }
}

fn start(model: &mut AppModel) -> (ReferencesTarget, DocumentId, u64, Position) {
    capture(update(model, Msg::Lsp(LspMsg::FindUsagesInPanel)).unwrap()).unwrap()
}

fn item(path: &str, line: u32, col: u32) -> LocationItem {
    LocationItem {
        path: path.into(),
        position: lsp_types::Position::new(line, col),
        preview: "symbol()".into(),
        route_hint: None,
    }
}

fn respond(
    model: &mut AppModel,
    request: (ReferencesTarget, DocumentId, u64, Position),
    items: Vec<LocationItem>,
    outcome: ReferencesOutcome,
) {
    let (target, document_id, revision, cursor) = request;
    update(
        model,
        Msg::Lsp(LspMsg::ReferencesResolved {
            target,
            document_id,
            revision,
            cursor,
            items,
            outcome,
        }),
    );
}

fn populated() -> AppModel {
    let mut model = fixture();
    let request = start(&mut model);
    respond(
        &mut model,
        request,
        vec![
            item("/b.rs", 0, 3),
            item("/a.rs", 1, 0),
            item("/a.rs", 0, 7),
            item("/a.rs", 0, 3),
            item("/a.rs", 0, 3),
        ],
        ReferencesOutcome::Found,
    );
    model
}

#[test]
fn usages_panel_and_popup_commands_capture_distinct_destinations() {
    let mut model = fixture();
    for message in token::keymap::Command::FindUsages.to_msgs() {
        assert!(matches!(message, Msg::Lsp(LspMsg::FindUsagesInPanel)));
    }
    let request = start(&mut model);
    assert!(matches!(request.0, ReferencesTarget::Panel(_)));
    assert_eq!(model.ui.focus, FocusTarget::Dock(DockPosition::Bottom));
    assert!(model.usages_panel.is_loading());
    assert!(model.ui.cursor_overlay.is_none());
    let popup = capture(update(&mut model, Msg::Lsp(LspMsg::FindReferences)).unwrap()).unwrap();
    assert!(matches!(popup.0, ReferencesTarget::Popup));
    assert!(!model.usages_panel.is_loading());
    assert!(model.usages_panel.status.contains("cancelled"));
}

#[test]
fn usages_panel_groups_deduplicates_and_orders_positions_without_auto_navigation() {
    let model = populated();
    assert_eq!(model.usages_panel.items.len(), 4);
    assert_eq!(model.usages_panel.items[0].position.character, 3);
    assert_eq!(model.usages_panel.items[1].position.character, 7);
    assert_eq!(
        model.usages_panel.rows(),
        vec![
            UsagesRow::Summary,
            UsagesRow::File {
                first: 0,
                count: 3,
                collapsed: false
            },
            UsagesRow::Location(0),
            UsagesRow::Location(1),
            UsagesRow::Location(2),
            UsagesRow::File {
                first: 3,
                count: 1,
                collapsed: false
            },
            UsagesRow::Location(3),
        ]
    );
    assert_eq!(
        model.document().file_path.as_deref(),
        Some(std::path::Path::new("/source.rs"))
    );
    assert_eq!(model.editor().active_cursor().column, 3);
    assert!(model.ui.cursor_overlay.is_none());
}

#[test]
fn usages_panel_receives_results_after_focus_change_without_reopening_a_closed_dock() {
    let mut model = fixture();
    let request = start(&mut model);
    update(&mut model, Msg::Dock(DockMsg::CloseFocusedDock));
    update(&mut model, Msg::Layout(LayoutMsg::NewTab));
    respond(
        &mut model,
        request,
        vec![item("/source.rs", 0, 3)],
        ReferencesOutcome::Found,
    );
    assert_eq!(model.usages_panel.items.len(), 1);
    assert_eq!(model.ui.focus, FocusTarget::Editor);
    assert!(model
        .dock_layout
        .active_panel_position(PanelId::Usages)
        .is_none());
    update(
        &mut model,
        Msg::Dock(DockMsg::ActivatePanel(PanelId::Usages)),
    );
    assert_eq!(model.usages_panel.items.len(), 1);
    assert_eq!(model.usages_panel.selected_index, Some(1));
}

#[test]
fn usages_panel_rejects_older_query_and_duplicate_reply_even_at_same_cursor() {
    let mut model = fixture();
    let first = start(&mut model);
    let latest = start(&mut model);
    let duplicate = latest.clone();
    respond(
        &mut model,
        first,
        vec![item("/old.rs", 0, 0)],
        ReferencesOutcome::Found,
    );
    assert!(model.usages_panel.is_loading());
    respond(
        &mut model,
        latest,
        vec![item("/current.rs", 0, 0)],
        ReferencesOutcome::Found,
    );
    respond(
        &mut model,
        duplicate,
        vec![item("/duplicate.rs", 0, 0)],
        ReferencesOutcome::Found,
    );
    assert_eq!(
        model.usages_panel.items[0].path,
        std::path::Path::new("/current.rs")
    );
}

#[test]
fn usages_panel_rejects_changed_source_but_not_caret_movement() {
    let mut model = fixture();
    let request = start(&mut model);
    model.editor_mut().cursors[0].column = 1;
    model.editor_mut().clear_selection();
    respond(
        &mut model,
        request,
        vec![item("/source.rs", 0, 3)],
        ReferencesOutcome::Found,
    );
    assert_eq!(model.usages_panel.items.len(), 1);
    let request = start(&mut model);
    model.document_mut().revision += 1;
    respond(
        &mut model,
        request,
        vec![item("/source.rs", 0, 3)],
        ReferencesOutcome::Found,
    );
    assert!(model.usages_panel.items.is_empty());
    assert!(!model.usages_panel.is_loading());
    assert!(model.usages_panel.status.contains("changed or closed"));
}

#[test]
fn usages_panel_reports_empty_unsupported_indexing_timeout_and_untitled_states() {
    for (outcome, label) in [
        (ReferencesOutcome::NoResult, "No usages"),
        (ReferencesOutcome::NotSupported, "not supported"),
        (ReferencesOutcome::StillIndexing, "indexing"),
        (ReferencesOutcome::TimedOut, "timed out"),
    ] {
        let mut model = fixture();
        let request = start(&mut model);
        respond(&mut model, request, vec![], outcome);
        assert!(!model.usages_panel.is_loading());
        assert!(model.usages_panel.status.contains(label));
        assert_eq!(model.usages_panel.rows(), [UsagesRow::Summary]);
    }
    let mut model = common::test_model("untitled", 0, 0);
    assert!(capture(update(&mut model, Msg::Lsp(LspMsg::FindUsagesInPanel)).unwrap()).is_none());
    assert!(!model.usages_panel.is_loading());
    assert!(model.usages_panel.status.contains("saved text file"));
}

#[test]
fn usages_panel_displays_the_result_limit() {
    let mut model = fixture();
    let request = start(&mut model);
    respond(
        &mut model,
        request,
        (0..250).map(|n| item("/a.rs", n, 0)).collect(),
        ReferencesOutcome::Found,
    );
    assert_eq!(model.usages_panel.items.len(), MAX_REFERENCE_LOCATIONS);
    assert!(model.usages_panel.status.contains("limit 200"));
}

#[test]
fn usages_panel_collapse_and_expand_are_directional_and_preserve_other_groups() {
    let mut model = populated();
    model.usages_panel.selected_index = Some(3);
    update(&mut model, Msg::Usages(UsagesMsg::SetExpanded(false)));
    assert_eq!(model.usages_panel.selected_index, Some(1));
    assert_eq!(model.usages_panel.rows().len(), 4);
    update(&mut model, Msg::Usages(UsagesMsg::SetExpanded(false)));
    assert_eq!(model.usages_panel.rows().len(), 4);
    update(&mut model, Msg::Usages(UsagesMsg::SetExpanded(true)));
    update(&mut model, Msg::Usages(UsagesMsg::SetExpanded(true)));
    assert_eq!(model.usages_panel.rows().len(), 7);
}

#[test]
fn usages_panel_mouse_selection_chevron_and_double_click_share_navigation() {
    let mut model = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.rs");
    std::fs::write(&path, model.document().buffer.to_string()).unwrap();
    model.document_mut().file_path = Some(path.clone());
    let request = start(&mut model);
    respond(
        &mut model,
        request,
        vec![LocationItem {
            path,
            ..item("/source.rs", 0, 7)
        }],
        ReferencesOutcome::Found,
    );
    update(
        &mut model,
        Msg::Usages(UsagesMsg::ClickRow {
            index: 1,
            click_count: 1,
            on_chevron: false,
        }),
    );
    assert_eq!(model.usages_panel.rows().len(), 3);
    update(
        &mut model,
        Msg::Usages(UsagesMsg::ClickRow {
            index: 1,
            click_count: 1,
            on_chevron: true,
        }),
    );
    assert_eq!(model.usages_panel.rows().len(), 2);
    update(&mut model, Msg::Usages(UsagesMsg::OpenSelected));
    update(
        &mut model,
        Msg::Usages(UsagesMsg::ClickRow {
            index: 2,
            click_count: 1,
            on_chevron: false,
        }),
    );
    assert_eq!(model.editor().active_cursor().column, 3);
    update(
        &mut model,
        Msg::Usages(UsagesMsg::ClickRow {
            index: 2,
            click_count: 2,
            on_chevron: false,
        }),
    );
    assert_eq!(model.editor().active_cursor().column, 7);
    assert_eq!(model.ui.focus, FocusTarget::Editor);
    assert_eq!(model.usages_panel.items.len(), 1);
    update(
        &mut model,
        Msg::Usages(UsagesMsg::ClickRow {
            index: usize::MAX,
            click_count: 2,
            on_chevron: true,
        }),
    );
    assert_eq!(model.usages_panel.selected_index, Some(2));
}

#[test]
fn usages_panel_scroll_and_page_selection_follow_relocated_chrome() {
    let mut model = fixture();
    let request = start(&mut model);
    respond(
        &mut model,
        request,
        (0..100).map(|n| item("/a.rs", n, 0)).collect(),
        ReferencesOutcome::Found,
    );
    // Generic panel chrome currently exists in the bottom and right docks;
    // the left side is the separate workspace sidebar.
    for position in [DockPosition::Bottom, DockPosition::Right] {
        for old in DockPosition::ALL {
            let dock = model.dock_layout.dock_mut(old);
            dock.panel_ids.retain(|panel| *panel != PanelId::Usages);
            dock.active_index = Some(0);
            dock.close();
        }
        model
            .dock_layout
            .dock_mut(position)
            .register_panel(PanelId::Usages);
        update(
            &mut model,
            Msg::Dock(DockMsg::ActivatePanel(PanelId::Usages)),
        );
        let key = token::layout::UiKey::PanelRows(PanelId::Usages);
        let view = token::layout::chrome::chrome(&model).row_list(key).unwrap();
        model.usages_panel.selected_index = Some(1);
        update(
            &mut model,
            Msg::Usages(UsagesMsg::Select {
                delta: 1,
                page: true,
            }),
        );
        assert_eq!(
            model.usages_panel.selected_index,
            Some(1 + view.visible_capacity())
        );
        update(
            &mut model,
            Msg::Usages(UsagesMsg::Scroll { lines: i32::MAX }),
        );
        let view = token::layout::chrome::chrome(&model).row_list(key).unwrap();
        assert_eq!(model.usages_panel.scroll_offset, view.max_scroll());
        for index in view.drawn_range() {
            let rect = view.row_rect(index).unwrap();
            assert_eq!(view.row_at_y(rect.y + 0.1), Some(index));
        }
        update(
            &mut model,
            Msg::Usages(UsagesMsg::Scroll { lines: i32::MIN }),
        );
        assert_eq!(model.usages_panel.scroll_offset, 0);
    }
}

#[test]
fn usages_panel_foreign_token_cannot_replace_active_results() {
    let mut model = fixture();
    let mut request = start(&mut model);
    request.0 = ReferencesTarget::Panel(Arc::new(()));
    respond(
        &mut model,
        request,
        vec![item("/wrong.rs", 0, 0)],
        ReferencesOutcome::Found,
    );
    assert!(model.usages_panel.is_loading());
    assert!(model.usages_panel.items.is_empty());
}

#[test]
fn usages_panel_settles_loading_when_source_closes_without_a_server_reply() {
    let mut model = fixture();
    let source_tab = model.editor_area.groups[&model.editor_area.focused_group_id].tabs[0].id;
    let _request = start(&mut model);
    update(&mut model, Msg::Layout(LayoutMsg::NewTab));
    update(&mut model, Msg::Layout(LayoutMsg::CloseTab(source_tab)));
    assert!(!model.usages_panel.is_loading());
    assert!(model.usages_panel.status.contains("changed or closed"));
    assert!(model.usages_panel.items.is_empty());
}
