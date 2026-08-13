//! Tests for the solved dock chrome (`layout::chrome`): the discipline
//! invariant (render, hit-test, and update queries read identical
//! geometry), dock-position independence, and the regressions the engine
//! migration fixed (tab advance, sliver rows, one clamp formula).

use token::layout::chrome::chrome;
use token::layout::{snapshot::snap, UiKey};
use token::model::AppModel;
use token::panel::{DockPosition, PanelId};
use token::view::geometry::WindowLayout;

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

/// Transition guarantee from the old `DockHeaderLayout`: the content area
/// is the dock rect minus a `tab_bar_height` header, full width.
#[test]
fn panel_content_matches_legacy_dock_content_rect() {
    let model = model_with_problems(DockPosition::Bottom);
    let snap_ = chrome(&model);
    let dock_rect = WindowLayout::compute(&model).bottom_dock_rect.unwrap();

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
    let right_rect = WindowLayout::compute(&model).right_dock_rect.unwrap();

    let rows = snap_
        .row_list(UiKey::PanelRows(PanelId::Problems))
        .expect("problems rows must resolve in the right dock");
    assert_eq!(rows.rect().x, right_rect.x);
    assert!(rows.visible_capacity() > 0);
    // A y inside the right dock maps to a row.
    let y = rows.rect().y + 1.0;
    assert_eq!(rows.row_at_y(y), Some(0));
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
