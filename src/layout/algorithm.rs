//! The layout solver: Clay's two-pass model expanded into explicit linear
//! passes over the declaration-ordered node `Vec` (parents always precede
//! descendants, so bottom-up = reverse iteration and top-down = forward).
//!
//! Pass order:
//! 1. text preferred widths (via the measure callback)
//! 2. fit widths, bottom-up
//! 3. final widths, top-down (Percent resolution + Clay's
//!    equalize-smallest-first grow distribution / equalize-largest shrink)
//! 4. text wrapping at final widths
//! 5. fit heights, bottom-up
//! 6. final heights, top-down
//! 7. positions + clip chains, top-down (scroll offsets subtracted)
//! 8. floating elements (anchors resolved against solved rects, subtree
//!    translated)
//! 9. snapshot emission (key map + z-sorted draw order)
//!
//! Divergences from Clay, chosen for Token's chrome: only `Grow` children
//! shrink when a row over-flows (fit/fixed content keeps its size and the
//! clip stack handles overflow), and a `Fit` child's cross-axis size is
//! capped at the parent's content box (chrome never wants cross-axis
//! overflow, and wrapping text needs the bound).

use std::collections::HashMap;

use crate::layout::anchor::{self, AttachPoint, FloatAnchor};
use crate::layout::sizing::{AlignX, AlignY, Dir, Sizing};
use crate::layout::snapshot::{LayoutSnapshot, RowListSolved, SolvedContent, SolvedNode, TextLine};
use crate::layout::text::TextMeasure;
use crate::layout::tree::{Content, Node, Wrap};
use crate::model::editor_area::Rect;

pub(crate) fn solve(
    mut nodes: Vec<Node>,
    root: Rect,
    scale_factor: f64,
    measure: &mut dyn TextMeasure,
) -> LayoutSnapshot {
    let n = nodes.len();
    if n == 0 {
        return LayoutSnapshot {
            nodes: Vec::new(),
            by_key: HashMap::new(),
            draw_order: Vec::new(),
        };
    }

    // --- Float pre-pass: nearest floating ancestor (inclusive) + z, and
    // WidthRule resolution (a float's width rule overrides its sizing).
    let mut float_root: Vec<Option<u32>> = vec![None; n];
    let mut z: Vec<i16> = vec![0; n];
    for i in 0..n {
        if let Some(parent) = nodes[i].parent {
            float_root[i] = float_root[parent as usize];
            z[i] = z[parent as usize];
        }
        if let Some(float) = nodes[i].decl.float {
            debug_assert!(
                float_root[i].is_none(),
                "nested floating elements are not supported"
            );
            float_root[i] = Some(i as u32);
            z[i] = float.z;
            if let Some(rule) = float.width {
                let cursor_anchored = matches!(float.anchor, FloatAnchor::Caret { .. });
                let w = anchor::resolve_width(
                    &rule,
                    cursor_anchored,
                    root.width as usize,
                    scale_factor,
                );
                nodes[i].decl.sizing.w = Sizing::Fixed(w as f32);
            }
        }
    }

    // --- Pass 1: text preferred widths (widest source line, unwrapped).
    let mut text_pref_w: Vec<f32> = vec![0.0; n];
    let mut text_line_h: Vec<f32> = vec![0.0; n];
    for i in 0..n {
        if let Content::Text(ref t) = nodes[i].decl.content {
            let style = t.style;
            let mut widest: f32 = 0.0;
            for line in split_source_lines(&t.text) {
                widest = widest.max(measure.width(line, style));
            }
            text_pref_w[i] = widest;
            text_line_h[i] = measure.line_height(style);
        }
    }

    // --- Pass 2: fit widths, bottom-up.
    let mut w: Vec<f32> = vec![0.0; n];
    for i in (0..n).rev() {
        w[i] = fit_size(&nodes, i, Axis::X, &w, text_pref_w[i]);
    }

    // --- Pass 3: final widths, top-down.
    w[0] = root.width;
    for i in 0..n {
        distribute_axis(&nodes, i, Axis::X, &mut w, root.width);
    }

    // --- Pass 4: wrap text at final widths.
    let mut text_lines: Vec<Option<Vec<TextLine>>> = vec![None; n];
    for i in 0..n {
        if let Content::Text(ref t) = nodes[i].decl.content {
            let avail = (w[i] - nodes[i].decl.padding.x()).max(0.0);
            let lines = match t.wrap {
                Wrap::None => crate::layout::text::measure_lines(&t.text, t.style, measure),
                Wrap::Words => crate::layout::text::wrap_to_width(&t.text, t.style, avail, measure),
            };
            text_lines[i] = Some(lines);
        }
    }

    // --- Pass 5: fit heights, bottom-up.
    let mut h: Vec<f32> = vec![0.0; n];
    for i in (0..n).rev() {
        let text_h = match text_lines[i] {
            Some(ref lines) => lines.len() as f32 * text_line_h[i],
            None => 0.0,
        };
        h[i] = fit_size(&nodes, i, Axis::Y, &h, text_h);
    }

    // --- Pass 6: final heights, top-down.
    h[0] = root.height;
    for i in 0..n {
        distribute_axis(&nodes, i, Axis::Y, &mut h, root.height);
    }

    // --- Pass 7: positions + clip chains, top-down.
    let mut x: Vec<f32> = vec![0.0; n];
    let mut y: Vec<f32> = vec![0.0; n];
    let mut clip: Vec<Option<Rect>> = vec![None; n];
    x[0] = root.x;
    y[0] = root.y;
    for i in 0..n {
        position_children(&nodes, i, &w, &h, &mut x, &mut y, &mut clip);
    }

    // --- Pass 8: floating elements. Anchors may target solved flow rects
    // (or earlier floats, in declaration order); the whole subtree —
    // positions and subtree-derived clips — translates by the same delta.
    fn solved_rect_of_key(
        nodes: &[Node],
        x: &[f32],
        y: &[f32],
        w: &[f32],
        h: &[f32],
        key: crate::layout::keys::UiKey,
    ) -> Option<Rect> {
        nodes
            .iter()
            .position(|node| node.decl.key == Some(key))
            .map(|i| Rect::new(x[i], y[i], w[i], h[i]))
    }
    for f in 0..n {
        let Some(float) = nodes[f].decl.float else {
            continue;
        };
        let (fx, fy) = match float.anchor {
            FloatAnchor::At { x, y } => (x, y),
            FloatAnchor::WindowCentered => {
                let (px, py) = anchor::position_centered(
                    root.width as usize,
                    root.height as usize,
                    w[f] as usize,
                    scale_factor,
                );
                (root.x + px as f32, root.y + py as f32)
            }
            FloatAnchor::Caret {
                x: ax,
                y: ay,
                line_h,
                prefer_below,
            } => {
                let (px, py) = anchor::position_at_caret(
                    (ax - root.x).max(0.0) as usize,
                    (ay - root.y).max(0.0) as usize,
                    line_h as usize,
                    prefer_below,
                    root.width as usize,
                    root.height as usize,
                    w[f] as usize,
                    h[f] as usize,
                    scale_factor,
                );
                (root.x + px as f32, root.y + py as f32)
            }
            FloatAnchor::Element { target, attach } => {
                let target_rect = solved_rect_of_key(&nodes, &x, &y, &w, &h, target)
                    .unwrap_or(Rect::new(root.x, root.y, 0.0, 0.0));
                let (ax, ay) = match attach {
                    AttachPoint::BelowLeft => (target_rect.x, target_rect.y + target_rect.height),
                    AttachPoint::BelowRight => (
                        target_rect.x + target_rect.width - w[f],
                        target_rect.y + target_rect.height,
                    ),
                    AttachPoint::AboveLeft => (target_rect.x, target_rect.y - h[f]),
                    AttachPoint::RightTop => (target_rect.x + target_rect.width, target_rect.y),
                };
                // Edge-clamp into the root box.
                let cx = ax.clamp(root.x, (root.x + root.width - w[f]).max(root.x));
                let cy = ay.clamp(root.y, (root.y + root.height - h[f]).max(root.y));
                (cx, cy)
            }
        };
        let (dx, dy) = (fx - x[f], fy - y[f]);
        if dx != 0.0 || dy != 0.0 {
            for i in f..n {
                if float_root[i] == Some(f as u32) {
                    x[i] += dx;
                    y[i] += dy;
                    if let Some(ref mut c) = clip[i] {
                        c.x += dx;
                        c.y += dy;
                    }
                }
            }
        }
    }

    // --- Pass 9: emit the snapshot.
    let mut solved = Vec::with_capacity(n);
    let mut by_key = HashMap::new();
    for i in 0..n {
        let decl = &nodes[i].decl;
        let rect = Rect::new(x[i], y[i], w[i].max(0.0), h[i].max(0.0));
        let content_rect = Rect::new(
            rect.x + decl.padding.l,
            rect.y + decl.padding.t,
            (rect.width - decl.padding.x()).max(0.0),
            (rect.height - decl.padding.y()).max(0.0),
        );
        let content = match decl.content {
            Content::None => SolvedContent::None,
            Content::Text(ref t) => SolvedContent::Text {
                text: t.text.clone(),
                style: t.style,
                line_height: text_line_h[i],
                lines: text_lines[i].take().unwrap_or_default(),
            },
            Content::RowList(list) => SolvedContent::RowList(RowListSolved {
                row_height: list.row_height,
                count: list.count,
                scroll_offset: list.scroll_offset,
            }),
        };
        if let Some(key) = decl.key {
            let prev = by_key.insert(key, i as u32);
            debug_assert!(prev.is_none(), "duplicate UiKey in layout tree: {key:?}");
        }
        solved.push(SolvedNode {
            key: decl.key,
            rect,
            content_rect,
            clip: clip[i],
            z: z[i],
            parent: nodes[i].parent,
            content,
        });
    }

    // Draw order: flow first, then floating subtrees, sorted by
    // `(z, in-float, declaration order)` — stable, parents before children.
    let mut draw_order: Vec<u32> = (0..n as u32).collect();
    draw_order.sort_by_key(|&i| {
        let idx = i as usize;
        (z[idx], u8::from(float_root[idx].is_some()), i)
    });

    LayoutSnapshot {
        nodes: solved,
        by_key,
        draw_order,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn sizing(self, node: &Node) -> Sizing {
        match self {
            Axis::X => node.decl.sizing.w,
            Axis::Y => node.decl.sizing.h,
        }
    }

    fn padding(self, node: &Node) -> f32 {
        match self {
            Axis::X => node.decl.padding.x(),
            Axis::Y => node.decl.padding.y(),
        }
    }

    /// Is this axis the one `dir` stacks children along?
    fn is_along(self, dir: Dir) -> bool {
        matches!((self, dir), (Axis::X, Dir::Row) | (Axis::Y, Dir::Column))
    }
}

/// Flow (non-floating) children of `i`.
fn flow_children<'a>(nodes: &'a [Node], i: usize) -> impl Iterator<Item = u32> + 'a {
    nodes[i]
        .children
        .iter()
        .copied()
        .filter(move |&c| nodes[c as usize].decl.float.is_none())
}

/// Bottom-up fit size of node `i` on `axis`; children sizes are already in
/// `sizes`. `content_size` is the node's own measured content (text) on
/// this axis.
fn fit_size(nodes: &[Node], i: usize, axis: Axis, sizes: &[f32], content_size: f32) -> f32 {
    let node = &nodes[i];
    match axis.sizing(node) {
        Sizing::Fixed(v) => v.max(0.0),
        // Percent resolves against the parent in the top-down pass; a
        // percent child contributes nothing to its parent's fit size.
        Sizing::Percent(_) => 0.0,
        // Grow contributes its minimum to the parent's fit size.
        Sizing::Grow { min, .. } => min.max(0.0),
        Sizing::Fit { min, max } => {
            let children_size = if node.children.is_empty() {
                content_size
            } else if axis.is_along(node.decl.dir) {
                let mut sum = 0.0;
                let mut count = 0usize;
                for c in flow_children(nodes, i) {
                    sum += sizes[c as usize];
                    count += 1;
                }
                sum + node.decl.gap * count.saturating_sub(1) as f32
            } else {
                let mut max_c: f32 = 0.0;
                for c in flow_children(nodes, i) {
                    max_c = max_c.max(sizes[c as usize]);
                }
                max_c
            };
            (children_size + axis.padding(node)).clamp(min.max(0.0), max)
        }
    }
}

/// Top-down width/height assignment for the children of `i`. Resolves
/// `Percent`, caps cross-axis children at the content box, and runs the
/// grow/shrink distribution along the layout axis. Floating children are
/// sized against the root box instead of participating in flow.
fn distribute_axis(nodes: &[Node], i: usize, axis: Axis, sizes: &mut [f32], root_size: f32) {
    if nodes[i].children.is_empty() {
        return;
    }
    let node = &nodes[i];
    let content = (sizes[i] - axis.padding(node)).max(0.0);
    let along = axis.is_along(node.decl.dir);

    // Floating children: size against the root, independent of flow.
    for &c in &node.children {
        let child = &nodes[c as usize];
        if child.decl.float.is_some() {
            match axis.sizing(child) {
                Sizing::Fixed(v) => sizes[c as usize] = v.max(0.0),
                Sizing::Percent(p) => sizes[c as usize] = (p * root_size).max(0.0),
                // Fit keeps its bottom-up size; Grow means "as much as the
                // window allows" for a float.
                Sizing::Fit { .. } => {}
                Sizing::Grow { min, max } => sizes[c as usize] = root_size.clamp(min.max(0.0), max),
            }
        }
    }

    if !along {
        // Cross axis: each flow child sizes independently against the
        // content box.
        for c in flow_children(nodes, i) {
            let ci = c as usize;
            let child = &nodes[ci];
            sizes[ci] = match axis.sizing(child) {
                Sizing::Fixed(v) => v.max(0.0),
                Sizing::Percent(p) => (p * content).max(0.0),
                Sizing::Grow { min, max } => content.clamp(min.max(0.0), max),
                // Divergence from Clay: cap fit at the content box so
                // wrapping text is bounded and chrome never overflows on
                // the cross axis.
                Sizing::Fit { .. } => sizes[ci].min(content),
            };
        }
        return;
    }

    // Along axis: resolve Percent, then distribute leftover to Grow
    // children (equalize-smallest-first) or shrink them when over-full.
    let mut used = 0.0;
    let mut count = 0usize;
    let mut grow: Vec<u32> = Vec::new();
    for c in flow_children(nodes, i) {
        let ci = c as usize;
        let child = &nodes[ci];
        match axis.sizing(child) {
            Sizing::Percent(p) => sizes[ci] = (p * content).max(0.0),
            Sizing::Grow { .. } => grow.push(c),
            Sizing::Fixed(_) | Sizing::Fit { .. } => {}
        }
        used += sizes[ci];
        count += 1;
    }
    used += node.decl.gap * count.saturating_sub(1) as f32;

    let leftover = content - used;
    if grow.is_empty() || leftover == 0.0 {
        return;
    }
    if leftover > 0.0 {
        grow_children(nodes, axis, sizes, &grow, leftover);
    } else {
        shrink_children(nodes, axis, sizes, &grow, -leftover);
    }
}

/// Clay's grow algorithm: repeatedly raise the smallest grow children
/// toward the next-smallest size until the leftover is spent or every
/// child hits its max clamp.
fn grow_children(nodes: &[Node], axis: Axis, sizes: &mut [f32], grow: &[u32], mut leftover: f32) {
    let max_of = |c: u32| match axis.sizing(&nodes[c as usize]) {
        Sizing::Grow { max, .. } => max,
        _ => f32::INFINITY,
    };
    let mut open: Vec<u32> = grow
        .iter()
        .copied()
        .filter(|&c| sizes[c as usize] < max_of(c))
        .collect();
    while leftover > f32::EPSILON && !open.is_empty() {
        let smallest = open
            .iter()
            .map(|&c| sizes[c as usize])
            .fold(f32::INFINITY, f32::min);
        // The size tier to raise the smallest children to: the
        // next-smallest child size, capped by the tightest max clamp among
        // the smallest, and by an even split of the leftover.
        let next_smallest = open
            .iter()
            .map(|&c| sizes[c as usize])
            .filter(|&s| s > smallest)
            .fold(f32::INFINITY, f32::min);
        let smallest_children: Vec<u32> = open
            .iter()
            .copied()
            .filter(|&c| sizes[c as usize] <= smallest)
            .collect();
        let tightest_max = smallest_children
            .iter()
            .map(|&c| max_of(c))
            .fold(f32::INFINITY, f32::min);
        let share = leftover / smallest_children.len() as f32;
        let target = (smallest + share).min(next_smallest).min(tightest_max);
        let raise = target - smallest;
        if raise <= 0.0 {
            break;
        }
        for &c in &smallest_children {
            sizes[c as usize] += raise;
            leftover -= raise;
        }
        open.retain(|&c| sizes[c as usize] < max_of(c));
    }
}

/// Symmetric shrink: repeatedly lower the largest grow children toward the
/// next-largest size until the overflow is recovered or every child hits
/// its min clamp. Fixed/fit children never shrink (overflow clips).
fn shrink_children(nodes: &[Node], axis: Axis, sizes: &mut [f32], grow: &[u32], mut overflow: f32) {
    let min_of = |c: u32| match axis.sizing(&nodes[c as usize]) {
        Sizing::Grow { min, .. } => min.max(0.0),
        _ => 0.0,
    };
    let mut open: Vec<u32> = grow
        .iter()
        .copied()
        .filter(|&c| sizes[c as usize] > min_of(c))
        .collect();
    while overflow > f32::EPSILON && !open.is_empty() {
        let largest = open
            .iter()
            .map(|&c| sizes[c as usize])
            .fold(f32::NEG_INFINITY, f32::max);
        let next_largest = open
            .iter()
            .map(|&c| sizes[c as usize])
            .filter(|&s| s < largest)
            .fold(0.0f32, f32::max);
        let largest_children: Vec<u32> = open
            .iter()
            .copied()
            .filter(|&c| sizes[c as usize] >= largest)
            .collect();
        let loosest_min = largest_children
            .iter()
            .map(|&c| min_of(c))
            .fold(0.0f32, f32::max);
        let share = overflow / largest_children.len() as f32;
        let target = (largest - share).max(next_largest).max(loosest_min);
        let lower = largest - target;
        if lower <= 0.0 {
            break;
        }
        for &c in &largest_children {
            sizes[c as usize] -= lower;
            overflow -= lower;
        }
        open.retain(|&c| sizes[c as usize] > min_of(c));
    }
}

/// Position the flow children of `i` (its own position is already set) and
/// record each child's effective clip. Floating children are provisionally
/// placed at the parent's origin; pass 8 translates them.
fn position_children(
    nodes: &[Node],
    i: usize,
    w: &[f32],
    h: &[f32],
    x: &mut [f32],
    y: &mut [f32],
    clip: &mut [Option<Rect>],
) {
    if nodes[i].children.is_empty() {
        return;
    }
    let node = &nodes[i];
    let content = Rect::new(
        x[i] + node.decl.padding.l,
        y[i] + node.decl.padding.t,
        (w[i] - node.decl.padding.x()).max(0.0),
        (h[i] - node.decl.padding.y()).max(0.0),
    );
    let child_clip = if node.decl.clip {
        Some(match clip[i] {
            Some(outer) => intersect(outer, content),
            None => content,
        })
    } else {
        clip[i]
    };

    let (scroll_x, scroll_y) = node
        .decl
        .scroll
        .map(|s| (s.offset_x, s.offset_y))
        .unwrap_or((0.0, 0.0));

    // Leftover along the layout axis, for alignment.
    let mut used = 0.0;
    let mut count = 0usize;
    for c in flow_children(nodes, i) {
        used += match node.decl.dir {
            Dir::Row => w[c as usize],
            Dir::Column => h[c as usize],
        };
        count += 1;
    }
    used += node.decl.gap * count.saturating_sub(1) as f32;
    let along_extent = match node.decl.dir {
        Dir::Row => content.width,
        Dir::Column => content.height,
    };
    let leftover = (along_extent - used).max(0.0);
    let (align_x, align_y) = node.decl.align;
    let along_offset = match node.decl.dir {
        Dir::Row => match align_x {
            AlignX::Left => 0.0,
            AlignX::Center => leftover / 2.0,
            AlignX::Right => leftover,
        },
        Dir::Column => match align_y {
            AlignY::Top => 0.0,
            AlignY::Center => leftover / 2.0,
            AlignY::Bottom => leftover,
        },
    };

    let mut cursor = along_offset;
    for &c in &node.children {
        let ci = c as usize;
        // Floats: provisional origin at the parent content origin (pass 8
        // translates the subtree); they escape the parent's clip.
        if nodes[ci].decl.float.is_some() {
            x[ci] = content.x;
            y[ci] = content.y;
            clip[ci] = None;
            continue;
        }
        match node.decl.dir {
            Dir::Row => {
                x[ci] = content.x + cursor;
                y[ci] = content.y
                    + match align_y {
                        AlignY::Top => 0.0,
                        AlignY::Center => (content.height - h[ci]).max(0.0) / 2.0,
                        AlignY::Bottom => (content.height - h[ci]).max(0.0),
                    };
                cursor += w[ci] + node.decl.gap;
            }
            Dir::Column => {
                y[ci] = content.y + cursor;
                x[ci] = content.x
                    + match align_x {
                        AlignX::Left => 0.0,
                        AlignX::Center => (content.width - w[ci]).max(0.0) / 2.0,
                        AlignX::Right => (content.width - w[ci]).max(0.0),
                    };
                cursor += h[ci] + node.decl.gap;
            }
        }
        x[ci] -= scroll_x;
        y[ci] -= scroll_y;
        clip[ci] = child_clip;
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width).max(x0);
    let y1 = (a.y + a.height).min(b.y + b.height).max(y0);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// `str::lines`, but a trailing newline still yields its (empty) final
/// line and `\r\n` is tolerated.
fn split_source_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}
