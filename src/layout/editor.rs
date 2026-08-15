//! Clay declarations for the editor chrome that sits around specialized
//! editor and preview content.
//!
//! This module intentionally stops at the chrome boundary:
//! - [`EditorTabBarLayout`] owns tab flow, gaps, horizontal scrolling, and
//!   clipping, while the editor viewport/gutter remain in `view::geometry`.
//! - [`PreviewPaneLayout`] owns the pane/header/content split and padding,
//!   while native Markdown/HTML painting and hosted webviews remain their
//!   respective feature implementations.

use crate::layout::keys::UiKey;
use crate::layout::sizing::{Dir, Padding, Sizing, SizingAxes};
use crate::layout::snapshot::LayoutSnapshot;
use crate::layout::text::{CellMeasure, TextStyle};
use crate::layout::tree::{Content, ElementDecl, ScrollDecl, TextDecl, UiTree, Wrap};
use crate::model::editor_area::{EditorGroup, GroupId, PreviewId, Rect, Tab, TabId};
use crate::model::{AppModel, ScaledMetrics};

/// The intrinsic width of one editor tab in physical pixels.
///
/// Clay owns placement and overflow, but the monospace title-width policy is
/// also used by the tab-drag ghost, so it remains an explicit shared input.
pub fn tab_width(model: &AppModel, tab: &Tab, char_width: f32) -> usize {
    let title = model.editor_area.tab_display_name(tab);
    tab_width_for_title(&title, char_width, model.metrics.padding_large)
}

fn tab_width_for_title(title: &str, char_width: f32, horizontal_padding: usize) -> usize {
    (title.chars().count() as f32 * char_width).round() as usize + horizontal_padding * 2
}

/// Solved Clay snapshot for one editor group's tab strip.
pub struct EditorTabBarLayout {
    snapshot: LayoutSnapshot,
    group_id: GroupId,
    last_tab_id: Option<TabId>,
    scroll_offset: usize,
    trailing_padding: usize,
}

impl EditorTabBarLayout {
    pub fn new(group: &EditorGroup, model: &AppModel, char_width: f32) -> Self {
        let metrics = &model.metrics;
        let group_x = group.rect.x.round();
        let group_y = group.rect.y.round();
        let group_width = group.rect.width.round().max(0.0);
        let root = Rect::new(group_x, group_y, group_width, metrics.tab_bar_height as f32);
        let last_tab_id = group.tabs.last().map(|tab| tab.id);

        let mut tree = UiTree::new();
        tree.node(
            ElementDecl {
                key: Some(UiKey::EditorTabBar(group.id)),
                dir: Dir::Column,
                sizing: SizingAxes::grow(),
                padding: Padding {
                    t: metrics.padding_small as f32,
                    b: metrics.padding_medium.saturating_sub(metrics.padding_small) as f32,
                    ..Default::default()
                },
                // The full group width is the horizontal scissor. Vertical
                // padding positions the tabs without narrowing that scissor.
                clip: true,
                ..Default::default()
            },
            |tree| {
                tree.node(
                    ElementDecl {
                        dir: Dir::Row,
                        sizing: SizingAxes::grow(),
                        padding: Padding {
                            l: metrics.padding_medium as f32,
                            r: metrics.padding_medium as f32,
                            ..Default::default()
                        },
                        gap: metrics.padding_small as f32,
                        scroll: Some(ScrollDecl {
                            offset_x: group.tab_scroll as f32,
                            offset_y: 0.0,
                        }),
                        ..Default::default()
                    },
                    |tree| {
                        for tab in &group.tabs {
                            let title = model.editor_area.tab_display_name(tab);
                            tree.leaf(ElementDecl {
                                key: Some(UiKey::EditorTab(group.id, tab.id)),
                                sizing: SizingAxes::new(
                                    Sizing::Fixed(tab_width_for_title(
                                        &title,
                                        char_width,
                                        metrics.padding_large,
                                    ) as f32),
                                    Sizing::GROW,
                                ),
                                padding: Padding::xy(
                                    metrics.padding_large as f32,
                                    metrics.padding_medium as f32,
                                ),
                                content: Content::Text(TextDecl {
                                    text: title,
                                    style: TextStyle::sized(0.0),
                                    wrap: Wrap::None,
                                }),
                                ..Default::default()
                            });
                        }
                    },
                );
            },
        );

        let mut measure = CellMeasure {
            char_width,
            line_height: model.line_height as f32,
        };
        let snapshot = tree.solve(root, metrics.scale_factor, &mut measure);

        Self {
            snapshot,
            group_id: group.id,
            last_tab_id,
            scroll_offset: group.tab_scroll,
            trailing_padding: metrics.padding_medium,
        }
    }

    pub fn bar_rect(&self) -> Rect {
        self.snapshot
            .rect(UiKey::EditorTabBar(self.group_id))
            .expect("editor tab layout always declares its bar")
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.bar_rect().contains(x as f32, y as f32)
    }

    pub fn tab_at(&self, x: f64, y: f64) -> Option<TabId> {
        match self.snapshot.hit(x as f32, y as f32) {
            Some(UiKey::EditorTab(group_id, tab_id)) if group_id == self.group_id => Some(tab_id),
            _ => None,
        }
    }

    /// The visible portion of a tab after applying the Clay clip chain.
    pub fn tab_rect(&self, tab_id: TabId) -> Option<Rect> {
        let node = self
            .snapshot
            .node(UiKey::EditorTab(self.group_id, tab_id))?;
        node.clip
            .and_then(|clip| intersect(node.rect, clip))
            .or_else(|| (node.clip.is_none()).then_some(node.rect))
    }

    /// The un-clipped title origin. The painter applies [`Self::tab_rect`]
    /// as its clip, so a partially visible tab does not shift its glyphs.
    pub fn title_origin(&self, tab_id: TabId) -> Option<(usize, usize)> {
        let rect = self
            .snapshot
            .content_rect(UiKey::EditorTab(self.group_id, tab_id))?;
        Some((
            rect.x.round().max(0.0) as usize,
            rect.y.round().max(0.0) as usize,
        ))
    }

    /// One tab's unscrolled `(start, end)` x-span relative to the bar.
    pub fn tab_span(&self, tab_id: TabId) -> Option<(usize, usize)> {
        let bar = self.bar_rect();
        let tab = self
            .snapshot
            .rect(UiKey::EditorTab(self.group_id, tab_id))?;
        let start = (tab.x - bar.x + self.scroll_offset as f32).round().max(0.0) as usize;
        let end = (tab.x + tab.width - bar.x + self.scroll_offset as f32)
            .round()
            .max(0.0) as usize;
        Some((start, end))
    }

    /// Total unscrolled width including the trailing tab-bar padding.
    pub fn total_tabs_width(&self) -> usize {
        self.last_tab_id
            .and_then(|tab_id| self.tab_span(tab_id))
            .map(|(_, end)| end + self.trailing_padding)
            .unwrap_or(0)
    }
}

/// Solved Clay snapshot for one preview pane.
pub struct PreviewPaneLayout {
    snapshot: LayoutSnapshot,
    preview_id: PreviewId,
}

impl PreviewPaneLayout {
    pub fn new(preview_id: PreviewId, rect: Rect, metrics: &ScaledMetrics) -> Self {
        let inset = (metrics.padding_large + metrics.padding_medium) as f32;
        let mut tree = UiTree::new();
        tree.node(
            ElementDecl {
                key: Some(UiKey::PreviewPane(preview_id)),
                dir: Dir::Column,
                sizing: SizingAxes::grow(),
                clip: true,
                ..Default::default()
            },
            |tree| {
                tree.leaf(ElementDecl {
                    key: Some(UiKey::PreviewHeader(preview_id)),
                    sizing: SizingAxes::new(
                        Sizing::GROW,
                        Sizing::Fixed(metrics.tab_bar_height as f32),
                    ),
                    padding: Padding {
                        l: inset,
                        r: inset,
                        t: metrics.padding_medium as f32,
                        ..Default::default()
                    },
                    ..Default::default()
                });
                tree.leaf(ElementDecl {
                    key: Some(UiKey::PreviewContent(preview_id)),
                    sizing: SizingAxes::grow(),
                    padding: Padding::all(inset),
                    ..Default::default()
                });
            },
        );

        let mut measure = CellMeasure {
            char_width: 0.0,
            line_height: 0.0,
        };
        let snapshot = tree.solve(rect, metrics.scale_factor, &mut measure);
        Self {
            snapshot,
            preview_id,
        }
    }

    pub fn pane_rect(&self) -> Rect {
        self.required_rect(UiKey::PreviewPane(self.preview_id))
    }

    pub fn header_rect(&self) -> Rect {
        self.required_rect(UiKey::PreviewHeader(self.preview_id))
    }

    /// Full content box used to position the hosted webview.
    pub fn hosted_content_rect(&self) -> Rect {
        self.required_rect(UiKey::PreviewContent(self.preview_id))
    }

    /// Padded content box used by the native preview fallback.
    pub fn native_content_rect(&self) -> Rect {
        self.snapshot
            .content_rect(UiKey::PreviewContent(self.preview_id))
            .expect("preview layout always declares its content")
    }

    pub fn title_origin(&self) -> (usize, usize) {
        let rect = self
            .snapshot
            .content_rect(UiKey::PreviewHeader(self.preview_id))
            .expect("preview layout always declares its header");
        (
            rect.x.round().max(0.0) as usize,
            rect.y.round().max(0.0) as usize,
        )
    }

    pub fn is_in_header(&self, x: f64, y: f64) -> bool {
        self.snapshot.hit(x as f32, y as f32) == Some(UiKey::PreviewHeader(self.preview_id))
    }

    pub fn is_in_content(&self, x: f64, y: f64) -> bool {
        self.snapshot.hit(x as f32, y as f32) == Some(UiKey::PreviewContent(self.preview_id))
    }

    fn required_rect(&self, key: UiKey) -> Rect {
        self.snapshot
            .rect(key)
            .unwrap_or_else(|| panic!("preview layout must declare {key:?}"))
    }
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_bar_hits_tabs_and_empty_space() {
        let mut model = AppModel::new(400, 300, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;
        {
            let group = model.editor_area.groups.get_mut(&group_id).unwrap();
            group.rect = Rect::new(10.0, 20.0, 260.0, 120.0);
            let mut second_tab = group.tabs[0].clone();
            second_tab.id = TabId(999);
            group.tabs.push(second_tab);
        }

        let group = &model.editor_area.groups[&group_id];
        let layout = EditorTabBarLayout::new(group, &model, 8.0);
        let first_rect = layout.tab_rect(group.tabs[0].id).unwrap();
        let second_rect = layout.tab_rect(group.tabs[1].id).unwrap();

        assert!(layout.contains(11.0, 21.0));
        assert_eq!(layout.tab_at(11.0, 21.0), None);
        assert_eq!(
            layout.tab_at((first_rect.x + 1.0) as f64, (first_rect.y + 1.0) as f64),
            Some(group.tabs[0].id)
        );
        assert_eq!(
            layout.tab_at((second_rect.x + 1.0) as f64, (second_rect.y + 1.0) as f64),
            Some(group.tabs[1].id)
        );
    }

    #[test]
    fn tab_bar_clips_at_group_edge_without_changing_logical_span() {
        let mut model = AppModel::new(400, 300, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;
        {
            let group = model.editor_area.groups.get_mut(&group_id).unwrap();
            group.rect = Rect::new(10.0, 20.0, 70.0, 120.0);
            let mut second_tab = group.tabs[0].clone();
            second_tab.id = TabId(999);
            group.tabs.push(second_tab);
        }

        let group = &model.editor_area.groups[&group_id];
        let layout = EditorTabBarLayout::new(group, &model, 8.0);
        let first = layout.tab_rect(group.tabs[0].id).unwrap();
        let bar = layout.bar_rect();

        assert_eq!(first.x + first.width, bar.x + bar.width);
        assert!(layout.tab_rect(group.tabs[1].id).is_none());
        assert!(layout.total_tabs_width() > bar.width as usize);
    }

    #[test]
    fn tab_bar_scroll_moves_visible_rects_but_preserves_logical_spans() {
        let mut model = AppModel::new(400, 300, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;
        {
            let group = model.editor_area.groups.get_mut(&group_id).unwrap();
            group.rect = Rect::new(10.0, 20.0, 120.0, 120.0);
            for id in [TabId(998), TabId(999)] {
                let mut tab = group.tabs[0].clone();
                tab.id = id;
                group.tabs.push(tab);
            }
        }

        let unscrolled = EditorTabBarLayout::new(&model.editor_area.groups[&group_id], &model, 8.0);
        let tab_id = model.editor_area.groups[&group_id].tabs[1].id;
        let logical_span = unscrolled.tab_span(tab_id).unwrap();
        let visible_x = unscrolled.tab_rect(tab_id).unwrap().x;
        let total_width = unscrolled.total_tabs_width();

        model
            .editor_area
            .groups
            .get_mut(&group_id)
            .unwrap()
            .tab_scroll = 20;
        let scrolled = EditorTabBarLayout::new(&model.editor_area.groups[&group_id], &model, 8.0);

        assert_eq!(scrolled.tab_span(tab_id), Some(logical_span));
        assert_eq!(scrolled.total_tabs_width(), total_width);
        assert_eq!(scrolled.tab_rect(tab_id).unwrap().x, visible_x - 20.0);
    }

    #[test]
    fn preview_layout_splits_hosted_and_native_content() {
        let metrics = ScaledMetrics::new(1.0);
        let layout =
            PreviewPaneLayout::new(PreviewId(7), Rect::new(100.0, 40.0, 320.0, 240.0), &metrics);
        let hosted = layout.hosted_content_rect();
        let native = layout.native_content_rect();
        let inset = (metrics.padding_large + metrics.padding_medium) as f32;

        assert_eq!(hosted.x, 100.0);
        assert_eq!(hosted.y, 40.0 + metrics.tab_bar_height as f32);
        assert_eq!(hosted.width, 320.0);
        assert_eq!(hosted.height, 240.0 - metrics.tab_bar_height as f32);
        assert_eq!(native.x, hosted.x + inset);
        assert_eq!(native.y, hosted.y + inset);
        assert_eq!(native.width, hosted.width - inset * 2.0);
        assert_eq!(native.height, hosted.height - inset * 2.0);
    }

    #[test]
    fn preview_layout_hit_testing_uses_solved_regions() {
        let metrics = ScaledMetrics::new(1.0);
        let layout =
            PreviewPaneLayout::new(PreviewId(7), Rect::new(100.0, 40.0, 320.0, 240.0), &metrics);
        let header_y = 40.0 + metrics.tab_bar_height as f64 / 2.0;
        let content_y = 40.0 + metrics.tab_bar_height as f64 + 10.0;

        assert!(layout.is_in_header(120.0, header_y));
        assert!(!layout.is_in_content(120.0, header_y));
        assert!(!layout.is_in_header(120.0, content_y));
        assert!(layout.is_in_content(120.0, content_y));
    }
}
