//! Token's dock-chrome layout: the one place dock geometry (header, tabs,
//! panel content, panel rows) is declared and solved.
//!
//! `chrome(model)` is a pure function over the app model — render (via
//! `RenderPlan`), hit-testing, mouse handling, and update-layer capacity
//! queries all call it and read the same solved snapshot, replacing the
//! per-call-site `WindowLayout → DockHeaderLayout → OutlinePanelLayout`
//! rebuild chains. Solving is a few microseconds for this tree size, so
//! correctness never depends on caching; `RenderPlan` holds one per frame
//! as an optimization.
//!
//! Chrome layout uses [`CellMeasure`] (monospace cells) everywhere, so all
//! consumers agree by construction and the function stays painter-free —
//! callable from `update/`.

use crate::layout::anchor::{FloatAnchor, FloatDecl};
use crate::layout::keys::UiKey;
use crate::layout::sizing::{Dir, Padding, Sizing, SizingAxes};
use crate::layout::snapshot::LayoutSnapshot;
use crate::layout::text::{CellMeasure, TextStyle};
use crate::layout::tree::{Content, ElementDecl, RowListDecl, UiTree};
use crate::model::editor_area::Rect;
use crate::model::AppModel;
use crate::panel::{DockPosition, PanelId};
use crate::view::geometry::WindowLayout;

/// Solve the dock chrome (right + bottom docks) for the current model
/// state. Keys present in the snapshot:
/// - `Dock(pos)`, `DockHeader(pos)`, `DockTab(pos, id)` per registered tab
/// - `PanelContent(active)` and, for row panels, `PanelRows(active)`
///
/// A key is absent when its dock is closed or its panel isn't the active
/// one — callers treat `None` as "not visible".
pub fn chrome(model: &AppModel) -> LayoutSnapshot {
    let window_layout = WindowLayout::compute(model);
    let mut tree = UiTree::new();
    let root = Rect::new(
        0.0,
        0.0,
        model.window_size.0 as f32,
        model.window_size.1 as f32,
    );

    tree.node(ElementDecl::default(), |t| {
        for (position, rect) in [
            (DockPosition::Right, window_layout.right_dock_rect),
            (DockPosition::Bottom, window_layout.bottom_dock_rect),
        ] {
            let Some(rect) = rect else { continue };
            let dock = model.dock_layout.dock(position);
            if !dock.is_open || dock.panel_ids.is_empty() {
                continue;
            }
            declare_dock(t, model, position, rect);
        }
    });

    let mut measure = CellMeasure {
        char_width: model.char_width,
        line_height: model.line_height as f32,
    };
    tree.solve(root, model.metrics.scale_factor, &mut measure)
}

fn declare_dock(t: &mut UiTree, model: &AppModel, position: DockPosition, rect: Rect) {
    let metrics = &model.metrics;
    let dock = model.dock_layout.dock(position);
    let active_panel = dock.active_panel();
    let row_style = TextStyle::sized(0.0); // CellMeasure ignores style

    t.node(
        ElementDecl {
            key: Some(UiKey::Dock(position)),
            dir: Dir::Column,
            sizing: SizingAxes::fixed(rect.width, rect.height),
            float: Some(FloatDecl {
                anchor: FloatAnchor::At {
                    x: rect.x,
                    y: rect.y,
                },
                z: 0,
                width: None,
            }),
            ..Default::default()
        },
        |t| {
            // Header: tab strip. Geometry mirrors the retired
            // `DockHeaderLayout`: tabs start `padding_medium` in, sit
            // `padding_small` below the top, and are `tab_bar_height -
            // padding_medium` tall.
            t.node(
                ElementDecl {
                    key: Some(UiKey::DockHeader(position)),
                    dir: Dir::Row,
                    sizing: SizingAxes::new(
                        Sizing::GROW,
                        Sizing::Fixed(metrics.tab_bar_height as f32),
                    ),
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
        },
    );
}
