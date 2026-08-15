//! Tests for the solved dock chrome (`layout::chrome`): the discipline
//! invariant (render, hit-test, and update queries read identical
//! geometry), dock-position independence, and the regressions the engine
//! migration fixed (tab advance, sliver rows, one clamp formula).

use token::layout::chrome::{chrome, shell, sidebar_rows};
use token::layout::{snapshot::snap, UiKey};
use token::model::{AppModel, Workspace};
use token::panel::{DockPosition, PanelId};
use token::view::hit_test::{hit_test_sidebar, HitTarget, Point};

fn assert_rect(
    snapshot: &token::layout::LayoutSnapshot,
    key: UiKey,
    expected: (usize, usize, usize, usize),
) {
    let rect = snapshot
        .rect(key)
        .unwrap_or_else(|| panic!("{key:?} should be present"));
    assert_eq!(snap(rect), expected, "unexpected rect for {key:?}");
}

fn model_with_problems(dock: DockPosition) -> AppModel {
    let mut model = AppModel::new(1000, 700, 1.0, vec![]);
    model.document_mut().file_path = Some(std::path::PathBuf::from("/proj/a.rs"));
    // Move Problems into the requested dock and activate it.
    if dock != DockPosition::Bottom {
        model
            .dock_layout
            .bottom
            .panel_ids
            .retain(|&id| id != PanelId::Problems);
        model
            .dock_layout
            .dock_mut(dock)
            .register_panel(PanelId::Problems);
    }
    model.dock_layout.dock_mut(dock).activate(PanelId::Problems);

    for file in ["/proj/a.rs", "/proj/b.rs"] {
        model.lsp.diagnostics.insert(
            std::path::PathBuf::from(file),
            (0..20)
                .map(|line| lsp_types::Diagnostic {
                    range: lsp_types::Range {
                        start: lsp_types::Position { line, character: 0 },
                        end: lsp_types::Position { line, character: 3 },
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    message: format!("error {line}"),
                    ..Default::default()
                })
                .collect(),
        );
    }
    model
}

/// The discipline invariant: two solves of the same model produce
/// identical geometry, so the renderer's snapshot (computed in
/// `build_render_plan`) and hit-testing's on-demand solve can never
/// disagree.
#[test]
fn chrome_is_deterministic_across_solves() {
    let model = model_with_problems(DockPosition::Bottom);
    let a = chrome(&model);
    let b = chrome(&model);

    for key in [
        UiKey::Dock(DockPosition::Bottom),
        UiKey::DockHeader(DockPosition::Bottom),
        UiKey::DockTab(DockPosition::Bottom, PanelId::Terminal),
        UiKey::DockTab(DockPosition::Bottom, PanelId::Problems),
        UiKey::PanelContent(PanelId::Problems),
        UiKey::PanelRows(PanelId::Problems),
    ] {
        let ra = a.rect(key).unwrap_or_else(|| panic!("{key:?} missing"));
        let rb = b.rect(key).unwrap();
        assert_eq!(snap(ra), snap(rb), "{key:?} diverged between solves");
    }
}

/// The content area tiles the dock below its `tab_bar_height` header.
#[test]
fn panel_content_tiles_the_dock_below_its_header() {
    let model = model_with_problems(DockPosition::Bottom);
    let snap_ = chrome(&model);
    let dock_rect = snap_.rect(UiKey::Dock(DockPosition::Bottom)).unwrap();

    let content = snap_.rect(UiKey::PanelContent(PanelId::Problems)).unwrap();
    let tab_bar_h = model.metrics.tab_bar_height as f32;
    assert_eq!(content.x, dock_rect.x);
    assert_eq!(content.y, dock_rect.y + tab_bar_h);
    assert_eq!(content.width, dock_rect.width);
    assert_eq!(content.height, dock_rect.height - tab_bar_h);
}

/// Pain B regression: capacity and row hit-mapping come from whichever
/// dock actually hosts the panel — moving Problems to the right dock must
/// produce right-dock geometry, not bottom-dock (or none).
#[test]
fn problems_in_right_dock_gets_right_dock_geometry() {
    let model = model_with_problems(DockPosition::Right);
    let snap_ = chrome(&model);
    let right_rect = snap_.rect(UiKey::Dock(DockPosition::Right)).unwrap();

    let rows = snap_
        .row_list(UiKey::PanelRows(PanelId::Problems))
        .expect("problems rows must resolve in the right dock");
    assert_eq!(rows.rect().x, right_rect.x);
    assert!(rows.visible_capacity() > 0);
    // A y inside the right dock maps to a row.
    let y = rows.rect().y + 1.0;
    assert_eq!(rows.row_at_y(y), Some(0));
}

#[test]
fn window_shell_accounts_for_status_bar_and_both_docks() {
    let mut model = AppModel::new(1000, 700, 1.0, vec![]);
    model.status_bar_height = 20;
    model.dock_layout.right.is_open = true;
    model.dock_layout.right.size_logical = 180.0;
    model.dock_layout.bottom.is_open = true;
    model.dock_layout.bottom.size_logical = 140.0;

    let chrome = chrome(&model);

    assert_rect(&chrome, UiKey::StatusBar, (0, 680, 1000, 20));
    assert_rect(&chrome, UiKey::EditorArea, (0, 0, 820, 540));
    assert_rect(
        &chrome,
        UiKey::Dock(DockPosition::Right),
        (820, 0, 180, 540),
    );
    assert_rect(
        &chrome,
        UiKey::Dock(DockPosition::Bottom),
        (0, 540, 1000, 140),
    );
}

#[test]
fn sidebar_spans_the_content_height_and_bottom_dock_starts_after_it() {
    let dir = tempfile::tempdir().expect("temporary workspace should be created");
    let mut model = AppModel::new(1000, 700, 1.0, vec![]);
    model.status_bar_height = 20;
    let mut workspace =
        Workspace::new(dir.path().to_path_buf(), &model.metrics).expect("workspace should load");
    workspace.sidebar_width_logical = 200.0;
    model.workspace = Some(workspace);
    model.dock_layout.right.is_open = true;
    model.dock_layout.right.size_logical = 180.0;
    model.dock_layout.bottom.is_open = true;
    model.dock_layout.bottom.size_logical = 140.0;

    let chrome = chrome(&model);

    assert_rect(&chrome, UiKey::Sidebar, (0, 0, 200, 680));
    assert_rect(&chrome, UiKey::EditorArea, (200, 0, 620, 540));
    assert_rect(
        &chrome,
        UiKey::Dock(DockPosition::Bottom),
        (200, 540, 800, 140),
    );
}

#[test]
fn shell_only_solve_matches_full_chrome_outer_rects() {
    let dir = tempfile::tempdir().expect("temporary workspace should be created");
    let mut model = AppModel::new(1000, 700, 1.0, vec![]);
    model.workspace = Some(
        Workspace::new(dir.path().to_path_buf(), &model.metrics).expect("workspace should load"),
    );
    model.dock_layout.right.activate(PanelId::Outline);
    model.dock_layout.bottom.activate(PanelId::Terminal);

    let full = chrome(&model);
    let outer = shell(&model);

    for key in [
        UiKey::Sidebar,
        UiKey::EditorArea,
        UiKey::StatusBar,
        UiKey::Dock(DockPosition::Right),
        UiKey::Dock(DockPosition::Bottom),
    ] {
        assert_eq!(
            snap(full.rect(key).unwrap()),
            snap(outer.rect(key).unwrap()),
            "shell-only geometry diverged for {key:?}"
        );
    }
    assert!(outer.rect(UiKey::PanelContent(PanelId::Terminal)).is_none());
    assert!(outer
        .rect(UiKey::DockHeader(DockPosition::Bottom))
        .is_none());
    assert!(outer.row_list(UiKey::Sidebar).is_none());
}

#[test]
fn sidebar_rows_share_one_viewport_for_render_hit_and_scroll() {
    let dir = tempfile::tempdir().expect("temporary workspace should be created");
    for index in 0..12 {
        std::fs::write(dir.path().join(format!("file-{index}.rs")), "")
            .expect("test file should be created");
    }
    let mut model = AppModel::new(800, 180, 1.0, vec![]);
    model.workspace = Some(
        Workspace::new(dir.path().to_path_buf(), &model.metrics).expect("workspace should load"),
    );
    model.workspace.as_mut().unwrap().scroll_offset = 3;
    let expected_count = model.workspace.as_ref().unwrap().visible_item_count();

    let full = chrome(&model);
    let rows = full
        .row_list(UiKey::Sidebar)
        .expect("full chrome declares sidebar rows");
    assert_eq!(rows.count(), expected_count);
    assert_eq!(rows.scroll_offset(), 3);
    assert_eq!(snap(rows.rect()), snap(full.rect(UiKey::Sidebar).unwrap()));
    assert_eq!(rows.row_at_y(rows.rect().y + 1.0), Some(3));
    assert_eq!(
        rows.max_scroll(),
        expected_count.saturating_sub(rows.visible_capacity())
    );

    let sidebar_only = sidebar_rows(&model);
    assert_eq!(
        snap(sidebar_only.rect(UiKey::Sidebar).unwrap()),
        snap(rows.rect())
    );
    assert!(sidebar_only.row_list(UiKey::Sidebar).is_some());
    assert!(sidebar_only
        .rect(UiKey::PanelContent(PanelId::Terminal))
        .is_none());

    let hit = hit_test_sidebar(
        &model,
        Point::new(rows.rect().x as f64 + 100.0, rows.rect().y as f64 + 1.0),
    );
    assert!(matches!(hit, Some(HitTarget::SidebarItem { row: 3, .. })));
}

/// A panel that isn't the active panel of any dock resolves to no row
/// geometry — the "not visible" contract callers rely on for clamping.
#[test]
fn inactive_panel_has_no_row_geometry() {
    let mut model = model_with_problems(DockPosition::Bottom);
    model.dock_layout.bottom.activate(PanelId::Terminal);
    let snap_ = chrome(&model);
    assert!(snap_
        .row_list(UiKey::PanelRows(PanelId::Problems))
        .is_none());
    assert!(snap_.rect(UiKey::PanelContent(PanelId::Terminal)).is_some());
}

/// Pain K regression: tabs advance by their *solved* widths — each tab
/// starts exactly one gap after the previous tab's right edge, with no
/// phantom gap from a clamped-vs-ideal width mismatch.
#[test]
fn dock_tabs_advance_by_solved_widths() {
    let model = model_with_problems(DockPosition::Bottom);
    let snap_ = chrome(&model);

    let terminal = snap_
        .rect(UiKey::DockTab(DockPosition::Bottom, PanelId::Terminal))
        .unwrap();
    let problems = snap_
        .rect(UiKey::DockTab(DockPosition::Bottom, PanelId::Problems))
        .unwrap();
    let gap = model.metrics.padding_small as f32;
    assert_eq!(problems.x, terminal.x + terminal.width + gap);
}

/// Pain D regression: the partial bottom row is drawn (and therefore
/// clickable), while scroll capacity still floors — the two can no longer
/// disagree because both derive from the same `RowListView`.
#[test]
fn sliver_row_is_drawn_and_capacity_floors() {
    let mut model = model_with_problems(DockPosition::Bottom);
    // Pick a dock height that is not a row multiple.
    model.dock_layout.bottom.size_logical = 205.0;
    let snap_ = chrome(&model);
    let rows = snap_.row_list(UiKey::PanelRows(PanelId::Problems)).unwrap();

    let exact_rows = rows.rect().height / rows.row_height();
    assert_ne!(
        exact_rows.fract(),
        0.0,
        "test setup: height must not be a row multiple"
    );
    assert_eq!(rows.visible_capacity(), exact_rows.floor() as usize);
    assert_eq!(
        rows.drawn_range().len(),
        (exact_rows.ceil() as usize).min(rows.count())
    );
    // The sliver row maps from a click near the bottom edge.
    let sliver_y = rows.rect().y + rows.rect().height - 1.0;
    assert_eq!(
        rows.row_at_y(sliver_y),
        Some(rows.scroll_offset() + rows.visible_capacity())
    );
}

/// Wheel-scroll clamping uses the one formula: `count - visible_capacity`.
#[test]
fn scroll_clamps_to_count_minus_capacity() {
    use token::messages::{Msg, ProblemsMsg};
    use token::update::update;

    let mut model = model_with_problems(DockPosition::Bottom);
    let rows_total = token::update::problems::problems_rows(&model).len();
    let capacity = chrome(&model)
        .row_list(UiKey::PanelRows(PanelId::Problems))
        .unwrap()
        .visible_capacity();

    update(
        &mut model,
        Msg::Problems(ProblemsMsg::Scroll { lines: 10_000 }),
    );
    assert_eq!(
        model.problems_panel.scroll_offset,
        rows_total.saturating_sub(capacity)
    );
}

/// `problems_row_count` (the layout-side count) always equals the
/// materialized ordering authority `problems_rows().len()`.
#[test]
fn problems_row_count_matches_problems_rows() {
    let mut model = model_with_problems(DockPosition::Bottom);
    assert_eq!(
        token::update::problems::problems_row_count(&model),
        token::update::problems::problems_rows(&model).len()
    );
    model
        .problems_panel
        .collapsed
        .insert(std::path::PathBuf::from("/proj/a.rs"));
    assert_eq!(
        token::update::problems::problems_row_count(&model),
        token::update::problems::problems_rows(&model).len()
    );

    model.document_mut().file_path = Some(std::path::PathBuf::from("/proj/b.rs"));
    assert_eq!(
        token::update::problems::problems_row_count(&model),
        token::update::problems::problems_rows(&model).len()
    );

    model.document_mut().file_path = None;
    assert_eq!(token::update::problems::problems_row_count(&model), 0);
}
