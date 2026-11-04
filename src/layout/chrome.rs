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

use crate::layout::anchor::{FloatAnchor, FloatDecl};
use crate::layout::keys::UiKey;
use crate::layout::sizing::{Dir, Padding, Sizing, SizingAxes};
use crate::layout::snapshot::LayoutSnapshot;
use crate::layout::text::{CellMeasure, TextStyle};
use crate::layout::tree::{Content, ElementDecl, RowListDecl, UiTree};
use crate::model::editor_area::Rect;
use crate::model::AppModel;
use crate::panel::{DockPosition, NonDockedPanel, PanelHost, PanelId};

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
    solve_chrome(model, true, true)
}

/// Solve only the top-level window shell.
///
/// This is the cheap state/update-layer path: it emits sidebar, editor,
/// status-bar, and outer dock rectangles without inspecting active panel
/// content or computing virtual row counts.
pub fn shell(model: &AppModel) -> LayoutSnapshot {
    solve_chrome(model, false, false)
}

/// Solve the shell with file-tree row geometry but without inspecting dock
/// panel contents. Pointer and update paths use this only when they actually
/// need sidebar rows, keeping the common editor-pointer path cheap.
pub fn sidebar_rows(model: &AppModel) -> LayoutSnapshot {
    solve_chrome(model, true, false)
}

fn solve_chrome(
    model: &AppModel,
    include_sidebar_rows: bool,
    include_dock_contents: bool,
) -> LayoutSnapshot {
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
                            content: model
                                .workspace
                                .as_ref()
                                .filter(|_| include_sidebar_rows)
                                .map(|workspace| {
                                    Content::RowList(RowListDecl {
                                        row_height: model.metrics.file_tree_row_height as f32,
                                        count: workspace.visible_item_count(),
                                        scroll_offset: workspace.scroll_offset,
                                    })
                                })
                                .unwrap_or_default(),
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
                                            // The chosen width yields once the editor is
                                            // gone, so a narrow window never overflows.
                                            SizingAxes::new(
                                                Sizing::Prefer {
                                                    size: width,
                                                    min: 0.0,
                                                },
                                                Sizing::GROW,
                                            ),
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
                                    // Keeps its header; the rest yields in short
                                    // windows instead of covering the status bar.
                                    SizingAxes::new(
                                        Sizing::GROW,
                                        Sizing::Prefer {
                                            size: height,
                                            min: model.metrics.tab_bar_height as f32,
                                        },
                                    ),
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

            if include_dock_contents {
                declare_floating_panels(t, model, sidebar_width.unwrap_or(0.0));
            }
        },
    );

    let mut measure = CellMeasure {
        char_width: model.char_width,
        line_height: model.line_height as f32,
    };
    tree.solve(root, model.metrics.scale_factor, &mut measure)
}

/// Declare each floating panel as a window-level float: a framed column with
/// a draggable header (title and actions) over a clipped content box. The size
/// is the panel's chosen extent, capped at the window.
fn declare_floating_panels(t: &mut UiTree, model: &AppModel, work_x: f32) {
    let scale = model.metrics.scale_factor as f32;
    for (&panel, &placement) in &model.non_docked_panels.by_id {
        let NonDockedPanel::Floating { rect, .. } = placement else {
            continue;
        };
        let width = rect.width * scale;
        let height = rect.height * scale;
        t.node(
            ElementDecl {
                key: Some(UiKey::FloatingPanel(panel)),
                dir: Dir::Column,
                // Capped at the window so a short window never cuts the
                // panel off below the status bar.
                sizing: SizingAxes::new(
                    Sizing::Prefer {
                        size: width,
                        min: 0.0,
                    },
                    Sizing::Prefer {
                        size: height,
                        min: model.metrics.tab_bar_height as f32,
                    },
                ),
                clip: true,
                float: Some(FloatDecl {
                    anchor: FloatAnchor::WindowPoint {
                        x: work_x + rect.x * scale,
                        y: rect.y * scale,
                    },
                    z: 50,
                    width: None,
                }),
                ..Default::default()
            },
            |t| {
                t.node(
                    ElementDecl {
                        key: Some(UiKey::FloatingPanelHeader(panel)),
                        dir: Dir::Row,
                        sizing: SizingAxes::new(
                            Sizing::GROW,
                            Sizing::Fixed(model.metrics.tab_bar_height as f32),
                        ),
                        padding: Padding::xy(
                            model.metrics.padding_large as f32,
                            model.metrics.padding_medium as f32,
                        ),
                        gap: model.metrics.padding_small as f32,
                        ..Default::default()
                    },
                    |t| {
                        t.text(None, panel.display_name(), TextStyle::sized(0.0));
                        t.leaf(ElementDecl {
                            sizing: SizingAxes::grow(),
                            ..Default::default()
                        });
                        for &action in panel.header_actions(PanelHost::Floating) {
                            t.leaf(ElementDecl {
                                key: Some(UiKey::FloatingPanelAction(panel, action)),
                                sizing: SizingAxes::new(Sizing::Fixed(22.0 * scale), Sizing::GROW),
                                ..Default::default()
                            });
                        }
                    },
                );
                t.leaf(ElementDecl {
                    key: Some(UiKey::FloatingPanelContent(panel)),
                    sizing: SizingAxes::grow(),
                    clip: true,
                    ..Default::default()
                });
            },
        );
    }
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
                            t.text(
                                None,
                                crate::view::panels::dock_tab_title(model, panel_id),
                                row_style,
                            );
                        },
                    );
                }
                if let Some((panel, actions)) = active_panel
                    .map(|panel| (panel, panel.header_actions(PanelHost::Docked)))
                    .filter(|(_, actions)| !actions.is_empty())
                {
                    t.leaf(ElementDecl {
                        sizing: SizingAxes::grow(),
                        ..Default::default()
                    });
                    for &action in actions {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::DockPanelAction(panel, action)),
                            sizing: SizingAxes::new(
                                Sizing::Fixed(22.0 * model.metrics.scale_factor as f32),
                                Sizing::GROW,
                            ),
                            ..Default::default()
                        });
                    }
                }
            },
        );

        // Content area of the active panel.
        let Some(active) = active_panel else { return };
        if active == PanelId::Terminal {
            crate::panels::terminal::declare_tabs(t, model);
        }
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
                    PanelId::Usages => Some(RowListDecl {
                        row_height: metrics.file_tree_row_height as f32,
                        count: model.usages_panel.rows().len(),
                        scroll_offset: model.usages_panel.scroll_offset,
                    }),
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

#[cfg(test)]
mod tests {
    use crate::layout::UiKey;
    use crate::model::AppModel;
    use crate::panel::{DockPosition, PanelId};

    #[test]
    fn short_windows_shrink_docks_instead_of_overflowing() {
        for scale in [1.0, 2.0] {
            let mut model = AppModel::new((800.0 * scale) as u32, (260.0 * scale) as u32, scale);
            model.dock_layout.bottom.size_logical = 500.0;
            model.dock_layout.bottom.activate(PanelId::Terminal);
            let chrome = super::chrome(&model);
            let status = chrome.rect(UiKey::StatusBar).unwrap();
            let dock = chrome.rect(UiKey::Dock(DockPosition::Bottom)).unwrap();
            assert!(
                dock.y + dock.height <= status.y + 0.5,
                "the bottom dock ends above the status bar at {scale}x"
            );
            assert!(
                dock.height >= model.metrics.tab_bar_height as f32,
                "keeps its header"
            );
        }
    }

    #[test]
    fn narrow_windows_shrink_the_right_dock_instead_of_overflowing() {
        let mut model = AppModel::new(300, 600, 1.0);
        model.dock_layout.right.size_logical = 900.0;
        model.dock_layout.right.activate(PanelId::Outline);
        model.dock_layout.right.is_open = true;
        let chrome = super::chrome(&model);
        let dock = chrome.rect(UiKey::Dock(DockPosition::Right)).unwrap();
        assert!(
            dock.x + dock.width <= model.window_size.0 as f32 + 0.5,
            "the right dock stays inside the window"
        );
    }
}
