//! Token's window-chrome layout: the one place shell and dock geometry
//! (sidebar, editor area, status bar, dock headers/tabs/content/rows) is
//! declared and solved.
//!
//! `chrome(model)` is a pure function over the app model — render (via
//! `RenderPlan`), hit-testing, mouse handling, and update-layer capacity
//! queries all call it and read the same solved snapshot. Solving is a few
//! microseconds for this tree size, so correctness never depends on caching;
//! `RenderPlan` holds one per frame as an optimization.
//!
//! Chrome layout uses [`CellMeasure`] (monospace cells) everywhere, so all
//! consumers agree by construction and the function stays painter-free —
//! callable from `update/`.

use crate::layout::keys::UiKey;
use crate::layout::sizing::{Dir, Padding, Sizing, SizingAxes};
use crate::layout::snapshot::LayoutSnapshot;
use crate::layout::text::{CellMeasure, TextStyle};
use crate::layout::tree::{Content, ElementDecl, RowListDecl, UiTree};
use crate::model::editor_area::Rect;
use crate::model::AppModel;
use crate::panel::{DockPosition, PanelId};

/// Solve the complete window shell and dock chrome for the current model
/// state. Keys present in the snapshot:
/// - `Sidebar` when the workspace sidebar is visible
/// - `EditorArea` and `StatusBar` always
/// - `Dock(pos)`, `DockHeader(pos)`, `DockTab(pos, id)` per registered tab
/// - `PanelContent(active)` and, for row panels, `PanelRows(active)`
///
/// A key is absent when its dock is closed or its panel isn't the active
/// one — callers treat `None` as "not visible".
pub fn chrome(model: &AppModel) -> LayoutSnapshot {
    solve_chrome(model, true)
}

/// Solve only the top-level window shell.
///
/// This is the cheap state/update-layer path: it emits sidebar, editor,
/// status-bar, and outer dock rectangles without inspecting active panel
/// content or computing virtual row counts.
pub fn shell(model: &AppModel) -> LayoutSnapshot {
    solve_chrome(model, false)
}

fn solve_chrome(model: &AppModel, include_dock_contents: bool) -> LayoutSnapshot {
    let mut tree = UiTree::new();
    let root = Rect::new(
        0.0,
        0.0,
        model.window_size.0 as f32,
        model.window_size.1 as f32,
    );
    let status_bar_height = model.status_bar_height as f32;
    let sidebar_width = model
        .workspace
        .as_ref()
        .filter(|workspace| workspace.sidebar_visible)
        .map(|workspace| workspace.sidebar_width(model.metrics.scale_factor));
    let right_dock_width = visible_dock_size(model, DockPosition::Right);
    let bottom_dock_height = visible_dock_size(model, DockPosition::Bottom);

    tree.node(
        ElementDecl {
            dir: Dir::Column,
            ..Default::default()
        },
        |t| {
            // Everything above the status bar. The sidebar spans both the
            // upper editor/right-dock row and the bottom dock.
            t.node(
                ElementDecl {
                    dir: Dir::Row,
                    sizing: SizingAxes::grow(),
                    ..Default::default()
                },
                |t| {
                    if let Some(width) = sidebar_width {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::Sidebar),
                            sizing: SizingAxes::new(Sizing::Fixed(width), Sizing::GROW),
                            ..Default::default()
                        });
                    }

                    // Work area to the right of the sidebar: editor/right
                    // dock above, bottom dock below.
                    t.node(
                        ElementDecl {
                            dir: Dir::Column,
                            sizing: SizingAxes::grow(),
                            ..Default::default()
                        },
                        |t| {
                            t.node(
                                ElementDecl {
                                    dir: Dir::Row,
                                    sizing: SizingAxes::grow(),
                                    ..Default::default()
                                },
                                |t| {
                                    t.leaf(ElementDecl {
                                        key: Some(UiKey::EditorArea),
                                        sizing: SizingAxes::grow(),
                                        ..Default::default()
                                    });
                                    if let Some(width) = right_dock_width {
                                        declare_dock(
                                            t,
                                            model,
                                            DockPosition::Right,
                                            SizingAxes::new(Sizing::Fixed(width), Sizing::GROW),
                                            include_dock_contents,
                                        );
                                    }
                                },
                            );
                            if let Some(height) = bottom_dock_height {
                                declare_dock(
                                    t,
                                    model,
                                    DockPosition::Bottom,
                                    SizingAxes::new(Sizing::GROW, Sizing::Fixed(height)),
                                    include_dock_contents,
                                );
                            }
                        },
                    );
                },
            );

            t.leaf(ElementDecl {
                key: Some(UiKey::StatusBar),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(status_bar_height)),
                ..Default::default()
            });
        },
    );

    let mut measure = CellMeasure {
        char_width: model.char_width,
        line_height: model.line_height as f32,
    };
    tree.solve(root, model.metrics.scale_factor, &mut measure)
}

fn visible_dock_size(model: &AppModel, position: DockPosition) -> Option<f32> {
    let dock = model.dock_layout.dock(position);
    (dock.is_open && !dock.panel_ids.is_empty())
        .then(|| dock.size(model.metrics.scale_factor))
        .filter(|size| *size > 0.0)
}

fn declare_dock(
    t: &mut UiTree,
    model: &AppModel,
    position: DockPosition,
    sizing: SizingAxes,
    include_contents: bool,
) {
    let metrics = &model.metrics;
    let dock = model.dock_layout.dock(position);
    let active_panel = dock.active_panel();
    let row_style = TextStyle::sized(0.0); // CellMeasure ignores style

    let declaration = ElementDecl {
        key: Some(UiKey::Dock(position)),
        dir: Dir::Column,
        sizing,
        ..Default::default()
    };
    if !include_contents {
        t.leaf(declaration);
        return;
    }

    t.node(declaration, |t| {
        // Header: tabs start `padding_medium` in, sit `padding_small`
        // below the top, and are `tab_bar_height - padding_medium` tall.
        t.node(
            ElementDecl {
                key: Some(UiKey::DockHeader(position)),
                dir: Dir::Row,
                sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(metrics.tab_bar_height as f32)),
                padding: Padding {
                    l: metrics.padding_medium as f32,
                    r: metrics.padding_medium as f32,
                    t: metrics.padding_small as f32,
                    b: metrics.padding_medium.saturating_sub(metrics.padding_small) as f32,
                },
                gap: metrics.padding_small as f32,
                clip: true,
                ..Default::default()
            },
            |t| {
                for panel_id in dock.panel_ids.iter().copied() {
                    t.node(
                        ElementDecl {
                            key: Some(UiKey::DockTab(position, panel_id)),
                            dir: Dir::Row,
                            sizing: SizingAxes::new(Sizing::FIT, Sizing::GROW),
                            padding: Padding::xy(
                                metrics.padding_large as f32,
                                metrics.padding_medium as f32,
                            ),
                            ..Default::default()
                        },
                        |t| {
                            t.text(None, panel_id.display_name(), row_style);
                        },
                    );
                }
            },
        );

        // Content area of the active panel.
        let Some(active) = active_panel else { return };
        t.node(
            ElementDecl {
                key: Some(UiKey::PanelContent(active)),
                dir: Dir::Column,
                sizing: SizingAxes::grow(),
                clip: true,
                ..Default::default()
            },
            |t| {
                let rows = match active {
                    PanelId::Problems => Some(RowListDecl {
                        row_height: metrics.file_tree_row_height as f32,
                        count: crate::update::problems::problems_row_count(model),
                        scroll_offset: model.problems_panel.scroll_offset,
                    }),
                    PanelId::Outline => Some(RowListDecl {
                        row_height: metrics.file_tree_row_height as f32,
                        count: crate::update::outline::outline_row_count(model),
                        scroll_offset: model.outline_panel.scroll_offset,
                    }),
                    _ => None,
                };
                if let Some(list) = rows {
                    t.leaf(ElementDecl {
                        key: Some(UiKey::PanelRows(active)),
                        sizing: SizingAxes::grow(),
                        content: Content::RowList(list),
                        ..Default::default()
                    });
                }
            },
        );
    });
}
