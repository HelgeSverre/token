//! Stateless pane chrome. Hosts own titles, actions, placement and input state.
//! The gallery and future dock/floating hosts consume the same measured geometry.
use super::button::{render_button, ButtonState, ButtonStyle};
use super::helpers::EllipsisSide;
use super::{FontRole, Frame, TextPainter};
use crate::layout::snapshot::snap;
use crate::model::Rect;
use crate::theme::Theme;

fn snapped(rect: Rect) -> Rect {
    let (x, y, w, h) = snap(rect);
    Rect::new(x as f32, y as f32, w as f32, h as f32)
}

/// Partition a host's solved border box. An absent footer reserves nothing;
/// a short pane loses its footer before its header and body overlap.
pub struct PaneLayout {
    pub header: Rect,
    pub content: Rect,
    pub footer: Option<Rect>,
}

impl PaneLayout {
    pub fn new(bounds: Rect, header_height: f32, footer_height: Option<f32>) -> Self {
        let bounds = snapped(bounds);
        let header_height = header_height.round().clamp(0.0, bounds.height);
        let footer_height = footer_height
            .map(|height| height.round().max(0.0))
            .filter(|&height| height > 0.0 && height <= bounds.height - header_height);
        let footer_y = bounds.y + bounds.height - footer_height.unwrap_or(0.0);
        Self {
            header: Rect::new(bounds.x, bounds.y, bounds.width, header_height),
            content: Rect::new(
                bounds.x,
                bounds.y + header_height,
                bounds.width,
                footer_y - bounds.y - header_height,
            ),
            footer: footer_height.map(|height| Rect::new(bounds.x, footer_y, bounds.width, height)),
        }
    }
}

/// Uses the existing icon painter and its explicit missing-glyph fallback.
#[derive(Clone, Copy)]
pub struct PaneIcon {
    pub glyph: char,
    pub fallback: char,
}

pub struct PaneAction<'a, Id> {
    pub id: Id,
    pub label: &'a str,
    pub style: ButtonStyle,
}

pub struct PaneHeader<'a, Id> {
    pub title: &'a str,
    pub icon: Option<PaneIcon>,
    pub actions: &'a [PaneAction<'a, Id>],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderTarget<Id> {
    Action(Id),
    /// The host opens its menu with the complete declared action order.
    Overflow,
}

pub struct HeaderActionLayout<Id> {
    pub target: HeaderTarget<Id>,
    pub rect: Rect,
    pub enabled: bool,
}

pub struct HeaderLayout<Id> {
    pub bounds: Rect,
    pub title: Rect,
    pub icon: Option<Rect>,
    pub actions: Vec<HeaderActionLayout<Id>>,
}

impl<Id: Copy> HeaderLayout<Id> {
    /// Disabled and omitted actions have no pointer target. Capture and keyboard
    /// routing remain with the host, not with this presentation helper.
    pub fn action_at(&self, x: f32, y: f32) -> Option<HeaderTarget<Id>> {
        self.actions
            .iter()
            .find(|action| action.enabled && action.rect.contains(x, y))
            .map(|action| action.target)
    }
}

impl<Id: Copy + Eq> PaneHeader<'_, Id> {
    pub fn height(&self, painter: &mut TextPainter, scale: f64) -> f32 {
        let code = painter.with_font(FontRole::Code);
        (code.line_height_for_size((12.0 * scale) as f32) as f32).max((22.0 * scale) as f32)
            + (8.0 * scale) as f32
    }

    pub fn layout(&self, painter: &mut TextPainter, bounds: Rect, scale: f64) -> HeaderLayout<Id> {
        let bounds = snapped(bounds);
        let px = |value: f64| (value * scale).round() as f32;
        let pad = px(8.0).min(bounds.width / 2.0);
        let gap = px(6.0);
        let available = (bounds.width - 2.0 * pad).max(0.0);
        let height = px(22.0);
        let mut ui = painter.with_font(FontRole::Ui);
        let widths: Vec<_> = self
            .actions
            .iter()
            .map(|action| {
                (ui.measure_sized(action.label, (12.0 * scale) as f32, 0.0)
                    .ceil()
                    + px(12.0))
                .max(height)
            })
            .collect();
        let total = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
        // Leave a readable title fragment. On narrow hosts, offer the entire
        // action list through overflow rather than a misleading partial subset.
        let title_min = px(48.0).min(available);
        let mut actions = Vec::new();
        let mut title_right = bounds.x + bounds.width - pad;
        if !widths.is_empty() && bounds.height >= height {
            if total + gap + title_min <= available {
                let mut x = title_right - total;
                title_right = x - gap;
                for (action, width) in self.actions.iter().zip(widths) {
                    actions.push(HeaderActionLayout {
                        target: HeaderTarget::Action(action.id),
                        rect: Rect::new(
                            x,
                            bounds.y + ((bounds.height - height) / 2.0).floor(),
                            width,
                            height,
                        ),
                        enabled: action.style.state != ButtonState::Disabled,
                    });
                    x += width + gap;
                }
            } else if height <= available {
                title_right -= height;
                actions.push(HeaderActionLayout {
                    target: HeaderTarget::Overflow,
                    rect: Rect::new(
                        title_right,
                        bounds.y + ((bounds.height - height) / 2.0).floor(),
                        height,
                        height,
                    ),
                    enabled: true,
                });
                title_right -= gap;
            }
        }
        let mut title_x = bounds.x + pad;
        let icon_size = px(14.0);
        let icon = self
            .icon
            .filter(|_| {
                bounds.height >= icon_size && title_right - title_x >= icon_size + gap + title_min
            })
            .map(|_| {
                let rect = Rect::new(
                    title_x,
                    bounds.y + ((bounds.height - icon_size) / 2.0).floor(),
                    icon_size,
                    icon_size,
                );
                title_x += icon_size + gap;
                rect
            });
        HeaderLayout {
            bounds,
            title: Rect::new(
                title_x,
                bounds.y,
                (title_right - title_x).max(0.0),
                bounds.height,
            ),
            icon,
            actions,
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        theme: &Theme,
        layout: &HeaderLayout<Id>,
        scale: f64,
    ) {
        paint_band(frame, layout.bounds, theme, false);
        frame.push_clip(layout.bounds);
        let size = (12.0 * scale) as f32;
        {
            let mut code = painter.with_font(FontRole::Code);
            draw_label(
                frame,
                &mut code,
                layout.title,
                self.title,
                size,
                theme.sidebar.foreground.to_argb_u32(),
            );
        }
        if let (Some(icon), Some(rect)) = (self.icon, layout.icon) {
            painter.draw_icon(
                frame,
                rect,
                icon.glyph,
                icon.fallback,
                theme.sidebar.foreground.to_argb_u32(),
            );
        }
        let mut ui = painter.with_font(FontRole::Ui);
        for cell in &layout.actions {
            let (label, mut style) = match cell.target {
                HeaderTarget::Action(id) => {
                    let Some(action) = self.actions.iter().find(|action| action.id == id) else {
                        continue;
                    };
                    (action.label, action.style)
                }
                HeaderTarget::Overflow => ("…", ButtonStyle::default()),
            };
            style.text_size = Some(size);
            render_button(frame, &mut ui, theme, cell.rect, label, style);
        }
        frame.pop_clip();
    }
}

#[derive(Clone, Copy)]
pub enum FooterEmphasis {
    Quiet,
    Normal,
    Accent,
}

pub enum PaneFooterRun<'a> {
    Dot(FooterEmphasis),
    Icon(PaneIcon, FooterEmphasis),
    Text(&'a str, FooterEmphasis),
}

/// The owner explicitly chooses which slot survives when both cannot be read.
#[derive(Clone, Copy)]
pub enum FooterPriority {
    Leading,
    Trailing,
}

pub struct PaneFooter<'a> {
    pub leading: &'a [PaneFooterRun<'a>],
    pub trailing: &'a [PaneFooterRun<'a>],
    pub priority: FooterPriority,
}

pub struct FooterLayout {
    pub bounds: Rect,
    pub leading: Option<Rect>,
    pub trailing: Option<Rect>,
}

impl PaneFooter<'_> {
    pub fn height(&self, painter: &mut TextPainter, scale: f64) -> Option<f32> {
        if !has_text(self.leading) && !has_text(self.trailing) {
            return None;
        }
        let ui = painter.with_font(FontRole::Ui);
        Some(ui.line_height_for_size((11.0 * scale) as f32) as f32 + (12.0 * scale) as f32)
    }

    pub fn layout(&self, painter: &mut TextPainter, bounds: Rect, scale: f64) -> FooterLayout {
        let bounds = snapped(bounds);
        let pad = (8.0 * scale).round() as f32;
        let gap = (12.0 * scale).round() as f32;
        let x = bounds.x + pad.min(bounds.width / 2.0);
        let width = (bounds.width - 2.0 * pad).max(0.0);
        let mut ui = painter.with_font(FontRole::Ui);
        let measure = |runs: &[PaneFooterRun], painter: &mut TextPainter| {
            if !has_text(runs) {
                return 0.0;
            }
            let mut width = 0.0;
            for run in runs {
                let next = run_width(run, painter, scale);
                if next > 0.0 {
                    if width > 0.0 {
                        width += (4.0 * scale).round() as f32;
                    }
                    width += next;
                }
            }
            width
        };
        let leading = measure(self.leading, &mut ui);
        let trailing = measure(self.trailing, &mut ui);
        let half = ((width - gap).max(0.0) / 2.0).floor();
        let both_fit = leading.min((64.0 * scale) as f32) <= half
            && trailing.min((64.0 * scale) as f32) <= half;
        let (leading_width, trailing_width) = match (leading > 0.0, trailing > 0.0) {
            (true, true) if both_fit => (leading.min(half), trailing.min(half)),
            (true, true) => match self.priority {
                FooterPriority::Leading => (leading.min(width), 0.0),
                FooterPriority::Trailing => (0.0, trailing.min(width)),
            },
            (true, false) => (leading.min(width), 0.0),
            (false, true) => (0.0, trailing.min(width)),
            (false, false) => (0.0, 0.0),
        };
        FooterLayout {
            bounds,
            leading: (leading_width > 0.0 && leading_width >= leading.min((64.0 * scale) as f32))
                .then(|| Rect::new(x, bounds.y, leading_width, bounds.height)),
            trailing: (trailing_width > 0.0
                && trailing_width >= trailing.min((64.0 * scale) as f32))
            .then(|| {
                Rect::new(
                    x + width - trailing_width,
                    bounds.y,
                    trailing_width,
                    bounds.height,
                )
            }),
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        theme: &Theme,
        layout: &FooterLayout,
        scale: f64,
    ) {
        paint_band(frame, layout.bounds, theme, true);
        let mut ui = painter.with_font(FontRole::Ui);
        for (runs, slot) in [
            (self.leading, layout.leading),
            (self.trailing, layout.trailing),
        ] {
            let Some(slot) = slot else { continue };
            frame.push_clip(slot);
            let mut x = slot.x;
            for run in runs {
                let width = run_width(run, &mut ui, scale);
                if width == 0.0 {
                    continue;
                }
                let width = width.min((slot.x + slot.width - x).max(0.0));
                if width <= 0.0 {
                    break;
                }
                let rect = Rect::new(x, slot.y, width, slot.height);
                match run {
                    PaneFooterRun::Text(text, emphasis) => draw_label(
                        frame,
                        &mut ui,
                        rect,
                        text,
                        (11.0 * scale) as f32,
                        emphasis.color(theme),
                    ),
                    PaneFooterRun::Dot(emphasis) => {
                        let size = (4.0 * scale).round() as f32;
                        ui.draw_icon(
                            frame,
                            Rect::new(x, slot.y + ((slot.height - size) / 2.0).floor(), size, size),
                            '●',
                            '.',
                            emphasis.color(theme),
                        );
                    }
                    PaneFooterRun::Icon(icon, emphasis) => {
                        let size = (12.0 * scale).round() as f32;
                        ui.draw_icon(
                            frame,
                            Rect::new(x, slot.y + ((slot.height - size) / 2.0).floor(), size, size),
                            icon.glyph,
                            icon.fallback,
                            emphasis.color(theme),
                        );
                    }
                }
                x += width + (4.0 * scale).round() as f32;
            }
            frame.pop_clip();
        }
    }
}

fn has_text(runs: &[PaneFooterRun]) -> bool {
    runs.iter()
        .any(|run| matches!(run, PaneFooterRun::Text(text, _) if !text.is_empty()))
}

fn run_width(run: &PaneFooterRun, painter: &mut TextPainter, scale: f64) -> f32 {
    match run {
        PaneFooterRun::Text(text, _) => painter
            .measure_sized(text, (11.0 * scale) as f32, 0.0)
            .ceil(),
        PaneFooterRun::Dot(_) => (4.0 * scale).round() as f32,
        PaneFooterRun::Icon(..) => (12.0 * scale).round() as f32,
    }
}

impl FooterEmphasis {
    fn color(self, theme: &Theme) -> u32 {
        match self {
            Self::Quiet => theme.overlay.text_dim,
            Self::Normal => theme.sidebar.foreground,
            Self::Accent => theme.overlay.accent,
        }
        .to_argb_u32()
    }
}

fn paint_band(frame: &mut Frame, rect: Rect, theme: &Theme, top: bool) {
    let (x, y, w, h) = snap(rect);
    if h == 0 || w == 0 {
        return;
    }
    frame.fill_rect(rect, theme.sidebar.background.to_argb_u32());
    frame.fill_rect_px(
        x,
        if top { y } else { y + h - 1 },
        w,
        1,
        theme.sidebar.border.to_argb_u32(),
    );
}

fn draw_label(
    frame: &mut Frame,
    painter: &mut TextPainter,
    rect: Rect,
    text: &str,
    size: f32,
    color: u32,
) {
    let height = painter.line_height_for_size(size) as f32;
    let text = painter.truncate_sized(text, size, rect.width, EllipsisSide::End);
    frame.push_clip(rect);
    painter.draw_sized(
        frame,
        rect.x as usize,
        (rect.y + ((rect.height - height).max(0.0) / 2.0).floor()) as usize,
        &text,
        size,
        0.0,
        color,
    );
    frame.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_partition_shares_edges_and_drops_footer_before_overlapping() {
        for scale in [1.0, 1.5, 2.0] {
            let bounds = Rect::new(13.3 * scale, 7.7 * scale, 241.4 * scale, 173.2 * scale);
            let layout = PaneLayout::new(bounds, 30.0 * scale, Some(25.0 * scale));
            let (_, y, _, h) = snap(bounds);
            let footer = layout.footer.unwrap();
            assert_eq!(layout.header.y + layout.header.height, layout.content.y);
            assert_eq!(layout.content.y + layout.content.height, footer.y);
            assert_eq!(footer.y + footer.height, (y + h) as f32);
            let absent = PaneLayout::new(bounds, 30.0 * scale, None);
            assert!(absent.footer.is_none());
            assert_eq!(absent.content.y + absent.content.height, (y + h) as f32);
            for height in [20.0, 54.0, 55.0] {
                let short = PaneLayout::new(Rect::new(0.0, 0.0, 120.0, height), 30.0, Some(25.0));
                assert_eq!(short.footer.is_some(), height >= 55.0);
                assert!(short.content.height >= 0.0);
            }
        }
    }

    #[test]
    fn header_packing_preserves_action_ids_and_omits_unusable_targets() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/Inter-Regular.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut cache = super::super::GlyphCache::default();
        let mut painter = TextPainter::new(&font, &mut cache, 14.0, 11.0, 8.0, 18);
        let actions = [
            PaneAction {
                id: 42,
                label: "Dock",
                style: ButtonStyle::default(),
            },
            PaneAction {
                id: 7,
                label: "Close",
                style: ButtonStyle {
                    state: ButtonState::Disabled,
                    ..Default::default()
                },
            },
        ];
        let header = PaneHeader {
            title: "Long inspector title",
            icon: None,
            actions: &actions,
        };
        for scale in [1.0, 1.5, 2.0] {
            let layout = header.layout(
                &mut painter,
                Rect::new(17.0, 9.0, 400.0 * scale as f32, 30.0 * scale as f32),
                scale,
            );
            assert_eq!(layout.actions.len(), 2);
            assert_eq!(layout.actions[0].target, HeaderTarget::Action(42));
            assert_eq!(layout.actions[1].target, HeaderTarget::Action(7));
            for cell in &layout.actions {
                let hit = layout.action_at(
                    cell.rect.x + cell.rect.width / 2.0,
                    cell.rect.y + cell.rect.height / 2.0,
                );
                assert_eq!(hit, cell.enabled.then_some(cell.target));
                assert!(layout.title.x + layout.title.width <= cell.rect.x);
                assert!(cell.rect.x + cell.rect.width <= layout.bounds.x + layout.bounds.width);
            }
            let narrow = header.layout(
                &mut painter,
                Rect::new(0.0, 0.0, 120.0 * scale as f32, 30.0 * scale as f32),
                scale,
            );
            assert_eq!(narrow.actions.len(), 1);
            assert_eq!(narrow.actions[0].target, HeaderTarget::Overflow);
            // One physical pixel below each minimum must remove the target.
            for (width, height) in [
                (400.0 * scale as f32, (22.0 * scale) as f32 - 1.0),
                ((38.0 * scale) as f32 - 1.0, 30.0 * scale as f32),
            ] {
                let tiny = header.layout(&mut painter, Rect::new(0.0, 0.0, width, height), scale);
                assert!(tiny.actions.is_empty());
            }
        }
    }

    #[test]
    fn footer_normalizes_empty_content_and_respects_owner_priority() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/Inter-Regular.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut cache = super::super::GlyphCache::default();
        let mut painter = TextPainter::new(&font, &mut cache, 14.0, 11.0, 8.0, 18);
        let mut footer = PaneFooter {
            leading: &[
                PaneFooterRun::Dot(FooterEmphasis::Accent),
                PaneFooterRun::Text("", FooterEmphasis::Normal),
            ],
            trailing: &[],
            priority: FooterPriority::Leading,
        };
        assert!(footer.height(&mut painter, 1.0).is_none());
        footer.leading = &[PaneFooterRun::Text(
            "Connected to workspace",
            FooterEmphasis::Normal,
        )];
        footer.trailing = &[PaneFooterRun::Text("43 samples", FooterEmphasis::Quiet)];
        for scale in [1.0, 1.5, 2.0] {
            let height = footer.height(&mut painter, scale).unwrap();
            let wide = footer.layout(
                &mut painter,
                Rect::new(17.0, 9.0, 400.0 * scale as f32, height),
                scale,
            );
            assert!(
                wide.leading.unwrap().x + wide.leading.unwrap().width < wide.trailing.unwrap().x
            );
            let narrow = Rect::new(17.0, 9.0, 100.0 * scale as f32, height);
            footer.priority = FooterPriority::Leading;
            let layout = footer.layout(&mut painter, narrow, scale);
            assert!(layout.leading.is_some() && layout.trailing.is_none());
            footer.priority = FooterPriority::Trailing;
            let layout = footer.layout(&mut painter, narrow, scale);
            assert!(layout.leading.is_none() && layout.trailing.is_some());
            let tiny = footer.layout(&mut painter, Rect::new(0.0, 0.0, 20.0, height), scale);
            assert!(tiny.leading.is_none() && tiny.trailing.is_none());
        }
    }
}
