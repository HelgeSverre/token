//! Table-driven tests for the layout engine (`src/layout/`): sizing passes,
//! grow/shrink distribution, wrapping, clip chains, row-list math, floats.

use token::layout::{
    snapshot, AlignX, AlignY, CellMeasure, Content, Dir, ElementDecl, FloatAnchor, FloatDecl,
    LayoutSnapshot, Padding, RowListDecl, Sizing, SizingAxes, TextDecl, TextStyle, UiKey, UiTree,
    WidthRule, Wrap,
};
use token::model::editor_area::Rect;
use token::panel::DockPosition;

const SF: f64 = 1.0;

fn measure() -> CellMeasure {
    CellMeasure {
        char_width: 8.0,
        line_height: 16.0,
    }
}

fn solve(tree: UiTree, w: f32, h: f32) -> LayoutSnapshot {
    let mut m = measure();
    tree.solve(Rect::new(0.0, 0.0, w, h), SF, &mut m)
}

/// Keys for tests that need to look up solved rects; the semantic meaning
/// of the dock variants is irrelevant here — they're just distinct keys.
const K_A: UiKey = UiKey::Dock(DockPosition::Left);
const K_B: UiKey = UiKey::Dock(DockPosition::Right);
const K_C: UiKey = UiKey::Dock(DockPosition::Bottom);
const K_ROWS: UiKey = UiKey::StatusBar;

fn rect_of(snap: &LayoutSnapshot, key: UiKey) -> Rect {
    snap.rect(key).expect("key not solved")
}

// --- Fit sizing ---

#[test]
fn fit_row_sums_children_and_gaps() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            key: Some(K_A),
            dir: Dir::Row,
            gap: 4.0,
            padding: Padding::all(10.0),
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                sizing: SizingAxes::fixed(50.0, 20.0),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                sizing: SizingAxes::fixed(30.0, 40.0),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 800.0, 600.0);
    let r = rect_of(&snap, K_A);
    // Root is forced to the root box; wrap in a root to test fit itself.
    assert_eq!((r.width, r.height), (800.0, 600.0));

    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.node(
            ElementDecl {
                key: Some(K_A),
                dir: Dir::Row,
                gap: 4.0,
                padding: Padding::all(10.0),
                ..Default::default()
            },
            |t| {
                t.leaf(ElementDecl {
                    sizing: SizingAxes::fixed(50.0, 20.0),
                    ..Default::default()
                });
                t.leaf(ElementDecl {
                    sizing: SizingAxes::fixed(30.0, 40.0),
                    ..Default::default()
                });
            },
        );
    });
    let snap = solve(t, 800.0, 600.0);
    let r = rect_of(&snap, K_A);
    // Width: 50 + 4 + 30 + 20 padding = 104. Height: max(20, 40) + 20 = 60.
    assert_eq!((r.width, r.height), (104.0, 60.0));
}

#[test]
fn fit_column_takes_max_width_and_sum_height() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.node(
            ElementDecl {
                key: Some(K_A),
                dir: Dir::Column,
                gap: 2.0,
                ..Default::default()
            },
            |t| {
                t.leaf(ElementDecl {
                    sizing: SizingAxes::fixed(50.0, 20.0),
                    ..Default::default()
                });
                t.leaf(ElementDecl {
                    sizing: SizingAxes::fixed(30.0, 40.0),
                    ..Default::default()
                });
            },
        );
    });
    let snap = solve(t, 800.0, 600.0);
    let r = rect_of(&snap, K_A);
    assert_eq!((r.width, r.height), (50.0, 62.0));
}

#[test]
fn fit_clamps_to_min_max() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.node(
            ElementDecl {
                key: Some(K_A),
                dir: Dir::Row,
                sizing: SizingAxes::new(
                    Sizing::Fit {
                        min: 200.0,
                        max: f32::INFINITY,
                    },
                    Sizing::Fit {
                        min: 0.0,
                        max: 25.0,
                    },
                ),
                ..Default::default()
            },
            |t| {
                t.leaf(ElementDecl {
                    sizing: SizingAxes::fixed(50.0, 40.0),
                    ..Default::default()
                });
            },
        );
    });
    let snap = solve(t, 800.0, 600.0);
    let r = rect_of(&snap, K_A);
    assert_eq!((r.width, r.height), (200.0, 25.0));
}

#[test]
fn text_fit_measures_widest_source_line() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.leaf(ElementDecl {
            key: Some(K_A),
            content: Content::Text(TextDecl {
                text: "ab\nabcd\nx".into(),
                style: TextStyle::sized(13.0),
                wrap: Wrap::None,
            }),
            ..Default::default()
        });
    });
    let snap = solve(t, 800.0, 600.0);
    let r = rect_of(&snap, K_A);
    // Widest line "abcd" = 4 * 8. Three lines * 16.
    assert_eq!((r.width, r.height), (32.0, 48.0));
}

#[test]
fn text_fit_counts_the_empty_line_after_a_trailing_newline() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.leaf(ElementDecl {
            key: Some(K_A),
            content: Content::Text(TextDecl {
                text: "ab\n".into(),
                style: TextStyle::sized(13.0),
                wrap: Wrap::None,
            }),
            ..Default::default()
        });
    });
    let snap = solve(t, 800.0, 600.0);
    assert_eq!(rect_of(&snap, K_A).height, 32.0);
}

// --- Grow distribution ---

#[test]
fn grow_splits_leftover_evenly() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_B),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 100.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).width, 50.0);
    assert_eq!(rect_of(&snap, K_B).width, 50.0);
}

#[test]
fn grow_equalizes_smallest_first() {
    // Clay's canonical behavior: a grow child with a larger min keeps its
    // head start until the smaller one catches up.
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::grow_clamped(60.0, f32::INFINITY), Sizing::GROW),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_B),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                ..Default::default()
            });
        },
    );
    // Space 100: A starts at 60, B at 0. B grows to 40; A stays 60.
    let snap = solve(t, 100.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).width, 60.0);
    assert_eq!(rect_of(&snap, K_B).width, 40.0);

    // Space 200: B catches A at 60, then both grow to 100 wait — total
    // 200 = 60 + 140; B reaches 60 (spent 60), remaining 80 splits evenly:
    // both end at 100.
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::grow_clamped(60.0, f32::INFINITY), Sizing::GROW),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_B),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 200.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).width, 100.0);
    assert_eq!(rect_of(&snap, K_B).width, 100.0);
}

#[test]
fn grow_respects_max_clamp() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::grow_clamped(0.0, 30.0), Sizing::GROW),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_B),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 100.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).width, 30.0);
    assert_eq!(rect_of(&snap, K_B).width, 70.0);
}

#[test]
fn shrink_recovers_overflow_from_grow_children() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::grow_clamped(80.0, f32::INFINITY), Sizing::GROW),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_B),
                sizing: SizingAxes::fixed(50.0, 10.0),
                ..Default::default()
            });
        },
    );
    // Content 100: A min 80 + fixed 50 = 130 over-full; A cannot shrink
    // below its min, so it stays 80 and the row overflows (clip handles it).
    let snap = solve(t, 100.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).width, 80.0);
    assert_eq!(rect_of(&snap, K_B).width, 50.0);
}

#[test]
fn percent_resolves_against_parent_content() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            padding: Padding::all(10.0),
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::Percent(0.25), Sizing::Percent(0.5)),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 120.0, 60.0);
    let r = rect_of(&snap, K_A);
    // Content box: 100 x 40.
    assert_eq!((r.width, r.height), (25.0, 20.0));
}

// --- Alignment & positioning ---

#[test]
fn alignment_centers_and_right_aligns() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            align: (AlignX::Right, AlignY::Center),
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::fixed(20.0, 10.0),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 100.0, 50.0);
    let r = rect_of(&snap, K_A);
    assert_eq!((r.x, r.y), (80.0, 20.0));
}

#[test]
fn row_positions_advance_with_gap() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            gap: 5.0,
            padding: Padding::xy(3.0, 2.0),
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::fixed(20.0, 10.0),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_B),
                sizing: SizingAxes::fixed(20.0, 10.0),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 100.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).x, 3.0);
    assert_eq!(rect_of(&snap, K_B).x, 28.0);
    assert_eq!(rect_of(&snap, K_A).y, 2.0);
}

#[test]
fn scroll_offsets_shift_children() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Column,
            scroll: Some(token::layout::tree::ScrollDecl {
                offset_x: 0.0,
                offset_y: 30.0,
            }),
            clip: true,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::fixed(20.0, 100.0),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 100.0, 50.0);
    assert_eq!(rect_of(&snap, K_A).y, -30.0);
}

// --- Clip chains ---

#[test]
fn clip_chains_intersect() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            clip: true,
            padding: Padding::all(10.0),
            ..Default::default()
        },
        |t| {
            t.node(
                ElementDecl {
                    key: Some(K_A),
                    clip: true,
                    sizing: SizingAxes::fixed(200.0, 200.0),
                    padding: Padding::all(5.0),
                    ..Default::default()
                },
                |t| {
                    t.leaf(ElementDecl {
                        key: Some(K_B),
                        sizing: SizingAxes::fixed(500.0, 500.0),
                        ..Default::default()
                    });
                },
            );
        },
    );
    let snap = solve(t, 100.0, 80.0);
    // A is clipped by root's content box (10..90 x 10..70).
    let a = snap.node(K_A).unwrap();
    let ac = a.clip.unwrap();
    assert_eq!((ac.x, ac.y, ac.width, ac.height), (10.0, 10.0, 80.0, 60.0));
    // B is clipped by the intersection of root's content box and A's
    // content box (A spans 10..210 with 5 padding => 15..205, capped by
    // root clip to 15..90 x 15..70).
    let b = snap.node(K_B).unwrap();
    let bc = b.clip.unwrap();
    assert_eq!((bc.x, bc.y), (15.0, 15.0));
    assert_eq!((bc.width, bc.height), (75.0, 55.0));
}

// --- Degenerate space ---

#[test]
fn zero_and_negative_space_never_panics_or_goes_negative() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            padding: Padding::all(50.0),
            gap: 10.0,
            ..Default::default()
        },
        |t| {
            for key in [Some(K_A), Some(K_B), None] {
                t.leaf(ElementDecl {
                    key,
                    sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                    ..Default::default()
                });
            }
        },
    );
    let snap = solve(t, 20.0, 10.0);
    for node in [snap.node(K_A).unwrap(), snap.node(K_B).unwrap()] {
        assert!(node.rect.width >= 0.0 && node.rect.height >= 0.0);
        assert!(node.content_rect.width >= 0.0 && node.content_rect.height >= 0.0);
    }
}

#[test]
fn empty_tree_solves() {
    let t = UiTree::new();
    let snap = solve(t, 100.0, 100.0);
    assert!(snap.hit(10.0, 10.0).is_none());
}

// --- RowList ---

fn row_list_tree(height: f32, count: usize, scroll_offset: usize) -> LayoutSnapshot {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Column,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_ROWS),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height)),
                content: Content::RowList(RowListDecl {
                    row_height: 22.0,
                    count,
                    scroll_offset,
                }),
                ..Default::default()
            });
        },
    );
    solve(t, 300.0, height.max(1.0))
}

#[test]
fn row_list_capacity_floors_and_drawn_ceils() {
    // 100px / 22px rows: 4 fully visible, 5 drawn (partial sliver).
    let snap = row_list_tree(100.0, 50, 0);
    let rows = snap.row_list(K_ROWS).unwrap();
    assert_eq!(rows.visible_capacity(), 4);
    assert_eq!(rows.drawn_range(), 0..5);
    // Exact multiple: no sliver.
    let snap = row_list_tree(88.0, 50, 0);
    let rows = snap.row_list(K_ROWS).unwrap();
    assert_eq!(rows.visible_capacity(), 4);
    assert_eq!(rows.drawn_range(), 0..4);
}

#[test]
fn row_list_row_at_y_maps_drawn_rows_only() {
    let snap = row_list_tree(100.0, 50, 10);
    let rows = snap.row_list(K_ROWS).unwrap();
    assert_eq!(rows.row_at_y(0.0), Some(10));
    assert_eq!(rows.row_at_y(21.9), Some(10));
    assert_eq!(rows.row_at_y(22.0), Some(11));
    // Sliver row (5th, partial) is drawn => hittable.
    assert_eq!(rows.row_at_y(99.0), Some(14));
    // Outside the box.
    assert_eq!(rows.row_at_y(101.0), None);
    assert_eq!(rows.row_at_y(-1.0), None);
}

#[test]
fn row_list_row_at_y_rejects_rows_past_count() {
    // 3 items in a 100px box: y below the last row maps to nothing.
    let snap = row_list_tree(100.0, 3, 0);
    let rows = snap.row_list(K_ROWS).unwrap();
    assert_eq!(rows.row_at_y(22.0 * 2.5), Some(2));
    assert_eq!(rows.row_at_y(22.0 * 3.5), None);
}

#[test]
fn row_list_scroll_clamp_is_count_minus_capacity() {
    let snap = row_list_tree(100.0, 50, 0);
    let rows = snap.row_list(K_ROWS).unwrap();
    assert_eq!(rows.max_scroll(), 46);
    assert_eq!(rows.clamp_scroll(100), 46);
    assert_eq!(rows.clamp_scroll(3), 3);
    // Fewer items than capacity: never scrolls.
    let snap = row_list_tree(100.0, 2, 0);
    let rows = snap.row_list(K_ROWS).unwrap();
    assert_eq!(rows.max_scroll(), 0);
    assert_eq!(rows.clamp_scroll(5), 0);
}

#[test]
fn row_list_row_rect_matches_row_at_y() {
    let snap = row_list_tree(100.0, 50, 10);
    let rows = snap.row_list(K_ROWS).unwrap();
    let r = rows.row_rect(12).unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (0.0, 44.0, 300.0, 22.0));
    assert_eq!(rows.row_at_y(r.y), Some(12));
    assert!(rows.row_rect(9).is_none()); // above scroll window
    assert!(rows.row_rect(16).is_none()); // below drawn range
}

// --- Text wrapping ---

fn wrapped_lines(text: &str, width: f32) -> Vec<String> {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Column,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::Fixed(width), Sizing::FIT),
                content: Content::Text(TextDecl {
                    text: text.into(),
                    style: TextStyle::sized(13.0),
                    wrap: Wrap::Words,
                }),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 400.0, 400.0);
    let node = snap.node(K_A).unwrap();
    match &node.content {
        token::layout::SolvedContent::Text { text, lines, .. } => lines
            .iter()
            .map(|l| text[l.range.clone()].to_string())
            .collect(),
        other => panic!("expected text content, got {other:?}"),
    }
}

#[test]
fn wrap_prefers_last_whitespace() {
    // 10 cells of 8px = 80px.
    let lines = wrapped_lines("hello brave new world", 80.0);
    assert_eq!(lines, vec!["hello", "brave new", "world"]);
}

#[test]
fn wrap_hard_breaks_oversize_tokens() {
    let lines = wrapped_lines("abcdefghijklmnop", 64.0); // 8 cells
    assert_eq!(lines, vec!["abcdefgh", "ijklmnop"]);
}

#[test]
fn wrap_preserves_source_newlines() {
    let lines = wrapped_lines("ab\ncd", 800.0);
    assert_eq!(lines, vec!["ab", "cd"]);
}

#[test]
fn wrap_preserves_the_empty_line_after_a_trailing_newline() {
    let lines = wrapped_lines("ab\n", 800.0);
    assert_eq!(lines, vec!["ab", ""]);
}

#[test]
fn wrapped_text_height_is_line_count() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Column,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::Fixed(80.0), Sizing::FIT),
                content: Content::Text(TextDecl {
                    text: "hello brave new world".into(),
                    style: TextStyle::sized(13.0),
                    wrap: Wrap::Words,
                }),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 400.0, 400.0);
    assert_eq!(rect_of(&snap, K_A).height, 48.0); // 3 lines * 16
}

// --- Floating elements ---

#[test]
fn caret_float_flips_above_when_below_is_short() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.leaf(ElementDecl {
            key: Some(K_C),
            sizing: SizingAxes::new(Sizing::Fixed(100.0), Sizing::Fixed(100.0)),
            float: Some(FloatDecl {
                anchor: FloatAnchor::Caret {
                    x: 50.0,
                    y: 200.0,
                    line_h: 20.0,
                    prefer_below: true,
                },
                z: 10,
                width: None,
            }),
            ..Default::default()
        });
    });
    let snap = solve(t, 1000.0, 250.0);
    let r = rect_of(&snap, K_C);
    assert_eq!((r.x, r.y), (50.0, 98.0)); // flipped above: 200 - 2 - 100
}

#[test]
fn float_width_rule_overrides_sizing() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.leaf(ElementDecl {
            key: Some(K_C),
            sizing: SizingAxes::new(Sizing::FIT, Sizing::Fixed(50.0)),
            float: Some(FloatDecl {
                anchor: FloatAnchor::Caret {
                    x: 0.0,
                    y: 0.0,
                    line_h: 20.0,
                    prefer_below: true,
                },
                z: 10,
                width: Some(WidthRule {
                    pct: 0.0,
                    min: 280.0,
                    max: 420.0,
                }),
            }),
            ..Default::default()
        });
    });
    let snap = solve(t, 1000.0, 500.0);
    assert_eq!(rect_of(&snap, K_C).width, 280.0);
}

#[test]
fn float_does_not_affect_flow_siblings() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_C),
                sizing: SizingAxes::fixed(300.0, 40.0),
                float: Some(FloatDecl {
                    anchor: FloatAnchor::WindowCentered,
                    z: 5,
                    width: None,
                }),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 100.0, 400.0);
    // The grow sibling takes the full row as if the float weren't there.
    assert_eq!(rect_of(&snap, K_A).width, 100.0);
}

#[test]
fn float_escapes_parent_clip_and_wins_hit_test() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            clip: true,
            sizing: SizingAxes::grow(),
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::grow(),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_C),
                sizing: SizingAxes::fixed(100.0, 100.0),
                float: Some(FloatDecl {
                    anchor: FloatAnchor::WindowCentered,
                    z: 10,
                    width: None,
                }),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 400.0, 400.0);
    let float_node = snap.node(K_C).unwrap();
    assert!(float_node.clip.is_none());
    assert_eq!(float_node.z, 10);
    // Centered at x=150; a point inside the float hits it, not the
    // underlying grow element.
    let r = rect_of(&snap, K_C);
    assert_eq!(snap.hit(r.x + 5.0, r.y + 5.0), Some(K_C));
    // A point outside the float hits the flow element.
    assert_eq!(snap.hit(5.0, 395.0), Some(K_A));
}

#[test]
fn element_attached_float_positions_below_target() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            dir: Dir::Row,
            ..Default::default()
        },
        |t| {
            t.leaf(ElementDecl {
                key: Some(K_A),
                sizing: SizingAxes::fixed(80.0, 30.0),
                ..Default::default()
            });
            t.leaf(ElementDecl {
                key: Some(K_C),
                sizing: SizingAxes::fixed(120.0, 60.0),
                float: Some(FloatDecl {
                    anchor: FloatAnchor::Element {
                        target: K_A,
                        attach: token::layout::AttachPoint::BelowLeft,
                    },
                    z: 1,
                    width: None,
                }),
                ..Default::default()
            });
        },
    );
    let snap = solve(t, 400.0, 400.0);
    let r = rect_of(&snap, K_C);
    assert_eq!((r.x, r.y), (0.0, 30.0));
}

// --- Hit testing ---

#[test]
fn hit_resolves_nearest_keyed_ancestor() {
    let mut t = UiTree::new();
    t.node(
        ElementDecl {
            key: Some(K_A),
            padding: Padding::all(10.0),
            ..Default::default()
        },
        |t| {
            // Unkeyed wrapper.
            t.node(
                ElementDecl {
                    sizing: SizingAxes::grow(),
                    ..Default::default()
                },
                |t| {
                    t.leaf(ElementDecl {
                        key: Some(K_B),
                        sizing: SizingAxes::fixed(20.0, 20.0),
                        ..Default::default()
                    });
                },
            );
        },
    );
    let snap = solve(t, 100.0, 100.0);
    assert_eq!(snap.hit(15.0, 15.0), Some(K_B));
    // Inside the unkeyed wrapper but outside K_B: nearest keyed ancestor.
    assert_eq!(snap.hit(50.0, 50.0), Some(K_A));
    assert_eq!(snap.hit(200.0, 50.0), None);
}

#[test]
fn hit_honors_clip() {
    let mut t = UiTree::new();
    t.node(ElementDecl::default(), |t| {
        t.node(
            ElementDecl {
                clip: true,
                sizing: SizingAxes::fixed(50.0, 50.0),
                ..Default::default()
            },
            |t| {
                t.leaf(ElementDecl {
                    key: Some(K_B),
                    sizing: SizingAxes::fixed(200.0, 200.0),
                    ..Default::default()
                });
            },
        );
    });
    let snap = solve(t, 300.0, 300.0);
    assert_eq!(snap.hit(40.0, 40.0), Some(K_B));
    // Inside K_B's rect but clipped away: no hit.
    assert_eq!(snap.hit(100.0, 100.0), None);
}

// --- Pixel snapping ---

#[test]
fn snap_keeps_adjacent_rects_gap_free() {
    let a = Rect::new(0.0, 0.0, 33.4, 10.0);
    let b = Rect::new(33.4, 0.0, 33.4, 10.0);
    let (ax, _, aw, _) = snapshot::snap(a);
    let (bx, _, _, _) = snapshot::snap(b);
    assert_eq!(ax + aw, bx);
}
