//! Docked Find/Replace geometry shared by painting, pointer input and caret services.

use crate::messages::{ModalMsg, UiMsg};
use crate::model::{AppModel, FindReplaceField, FocusTarget, Rect, ScaledMetrics};

use super::button::{render_button, ButtonState};
use super::geometry::WidgetRect;
use super::{Frame, TextFieldOptions, TextFieldRenderer, TextPainter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Field(FindReplaceField),
    Expand,
    Close,
    Case,
    Word,
    Regex,
    Selection,
    Previous,
    Next,
    ReplaceNext,
    ReplaceAll,
}

impl Control {
    pub fn message(self) -> UiMsg {
        match self {
            Self::Field(field) => UiMsg::FocusFindField(field),
            Self::Expand => UiMsg::ToggleFindReplaceMode,
            Self::Close => UiMsg::CloseFind,
            Self::Case => UiMsg::Modal(ModalMsg::ToggleFindReplaceCaseSensitive),
            Self::Word => UiMsg::Modal(ModalMsg::ToggleFindReplaceWholeWord),
            Self::Regex => UiMsg::Modal(ModalMsg::ToggleFindReplaceRegex),
            Self::Selection => UiMsg::Modal(ModalMsg::ToggleFindReplaceSelectionOnly),
            Self::Previous => UiMsg::Modal(ModalMsg::FindPrevious),
            Self::Next => UiMsg::Modal(ModalMsg::FindNext),
            Self::ReplaceNext => UiMsg::Modal(ModalMsg::ReplaceAndFindNext),
            Self::ReplaceAll => UiMsg::Modal(ModalMsg::ReplaceAll),
        }
    }
}

fn compact(width: f32, line_height: usize, metrics: &ScaledMetrics) -> bool {
    width < (640.0 * metrics.scale_factor).max((line_height * 28) as f64) as f32
}

pub(crate) fn height(
    line_height: usize,
    metrics: &ScaledMetrics,
    width: f32,
    replace: bool,
) -> usize {
    let narrow = compact(width, line_height, metrics);
    let rows = (1 + usize::from(narrow)) * (1 + usize::from(replace));
    rows * (line_height + metrics.padding_small * 2)
        + (rows - 1) * metrics.padding_small
        + metrics.padding_medium * 2
        + metrics.border_width
}

#[derive(Debug)]
pub struct FindBarLayout {
    pub rect: Rect,
    pub controls: Vec<(Control, WidgetRect)>,
    pub status: WidgetRect,
}

impl FindBarLayout {
    pub fn new(model: &AppModel) -> Option<Self> {
        let (_, bar_height) = model.find_bar_inset()?;
        let group = model.editor_area.focused_group()?;
        let state = model.ui.find_bar.as_ref()?;
        let metrics = &model.metrics;
        let pad = metrics.padding_medium;
        let gap = metrics.padding_small;
        let row_h = model.line_height + gap * 2;
        let rect = Rect::new(
            group.rect.x,
            group.rect.y + metrics.tab_bar_height as f32,
            group.rect.width,
            (bar_height as f32).min((group.rect.height - metrics.tab_bar_height as f32).max(0.0)),
        );
        let x = rect.x.round() as usize + pad;
        let y = rect.y.round() as usize + pad;
        let right = (rect.x + rect.width).round() as usize;
        let right = right.saturating_sub(pad).max(x);
        let narrow = compact(rect.width, model.line_height, metrics);
        let button_w = if narrow {
            row_h.min(right.saturating_sub(x + gap * 6) / 6)
        } else {
            row_h
        };
        let close_x = right.saturating_sub(button_w).max(x);
        let control_y = if narrow { y + row_h + gap } else { y };
        // A dedicated control row on narrow panes leaves the query usable.
        let controls_right = if narrow {
            right
        } else {
            close_x.saturating_sub(gap)
        };
        let status_w = (96.0 * metrics.scale_factor) as usize;
        let controls_w = button_w * 6 + gap * 6 + status_w;
        let controls_x = if narrow {
            x
        } else {
            controls_right.saturating_sub(controls_w).max(x)
        };
        let field_x = x + button_w + gap;
        let field_right = if narrow {
            close_x.saturating_sub(gap)
        } else {
            controls_x.saturating_sub(gap)
        };
        let field = WidgetRect {
            x: field_x,
            y,
            w: field_right.saturating_sub(field_x),
            h: row_h,
        };
        let mut controls = vec![
            (
                Control::Expand,
                WidgetRect {
                    x,
                    y,
                    w: button_w,
                    h: row_h,
                },
            ),
            (Control::Field(FindReplaceField::Query), field),
            (
                Control::Close,
                WidgetRect {
                    x: close_x,
                    y,
                    w: button_w,
                    h: row_h,
                },
            ),
        ];
        let mut cx = controls_x;
        for control in [
            Control::Case,
            Control::Word,
            Control::Regex,
            Control::Selection,
        ] {
            controls.push((
                control,
                WidgetRect {
                    x: cx,
                    y: control_y,
                    w: button_w,
                    h: row_h,
                },
            ));
            cx += button_w + gap;
        }
        let nav_x = controls_right.saturating_sub(button_w * 2 + gap).max(cx);
        let status = WidgetRect {
            x: cx,
            y: control_y,
            w: nav_x.saturating_sub(cx + gap),
            h: row_h,
        };
        for (index, control) in [Control::Previous, Control::Next].into_iter().enumerate() {
            controls.push((
                control,
                WidgetRect {
                    x: nav_x + index * (button_w + gap),
                    y: control_y,
                    w: button_w,
                    h: row_h,
                },
            ));
        }
        if state.replace_mode {
            let y = control_y + row_h + gap;
            let action_w = ((90.0 * metrics.scale_factor) as usize)
                .max(row_h * 3)
                .min(right.saturating_sub(field_x + gap) / 2);
            let actions_x = right.saturating_sub(action_w * 2 + gap).max(field_x);
            controls.push((
                Control::Field(FindReplaceField::Replace),
                WidgetRect {
                    x: field_x,
                    y,
                    w: if narrow {
                        right.saturating_sub(field_x)
                    } else {
                        actions_x.saturating_sub(field_x + gap)
                    },
                    h: row_h,
                },
            ));
            for (index, control) in [Control::ReplaceNext, Control::ReplaceAll]
                .into_iter()
                .enumerate()
            {
                controls.push((
                    control,
                    WidgetRect {
                        x: actions_x + index * (action_w + gap),
                        y: if narrow { y + row_h + gap } else { y },
                        w: action_w,
                        h: row_h,
                    },
                ));
            }
        }
        Some(Self {
            rect,
            controls,
            status,
        })
    }

    pub fn hit(&self, x: f64, y: f64) -> Option<Control> {
        self.controls
            .iter()
            .find(|(_, rect)| contains(*rect, x, y))
            .map(|(control, _)| *control)
    }

    pub fn field(&self, field: FindReplaceField) -> Option<WidgetRect> {
        self.controls
            .iter()
            .find(|(control, _)| *control == Control::Field(field))
            .map(|(_, rect)| *rect)
    }
}

fn contains(rect: WidgetRect, x: f64, y: f64) -> bool {
    x >= rect.x as f64
        && x < (rect.x + rect.w) as f64
        && y >= rect.y as f64
        && y < (rect.y + rect.h) as f64
}

pub fn field_options(model: &AppModel, field: FindReplaceField) -> Option<TextFieldOptions> {
    let layout = FindBarLayout::new(model)?;
    let state = model.ui.find_bar.as_ref()?;
    let input = match field {
        FindReplaceField::Query => &state.query_editable,
        FindReplaceField::Replace => &state.replace_editable,
    };
    Some(TextFieldOptions::for_modal(
        input,
        &layout.field(field)?,
        model.line_height,
        model.char_width,
        model.metrics.scale_factor,
    ))
}

pub fn column_at(model: &AppModel, field: FindReplaceField, x: f64) -> Option<usize> {
    let options = field_options(model, field)?;
    let offset = ((x - options.x as f64) / options.char_width as f64).round() as isize;
    Some(options.scroll_x.saturating_add_signed(offset))
}

pub(crate) fn render(frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
    let Some(layout) = FindBarLayout::new(model) else {
        return;
    };
    let Some(state) = model.ui.find_bar.as_ref() else {
        return;
    };
    let previous_font = painter.use_ui_font(true);
    frame.push_clip(layout.rect);
    let (x, y, w, h) = crate::layout::snapshot::snap(layout.rect);
    frame.fill_rect_px(
        x,
        y,
        w,
        h,
        model.theme.tab_bar.background.to_argb_u32() | 0xFF000000,
    );
    frame.fill_rect_px(
        x,
        y + h.saturating_sub(1),
        w,
        1,
        model.theme.tab_bar.border.to_argb_u32(),
    );
    for (control, rect) in &layout.controls {
        let hovered = model.ui.hover == crate::model::HoverRegion::FindBar(Some(*control));
        if let Control::Field(field) = control {
            let input = match field {
                FindReplaceField::Query => &state.query_editable,
                FindReplaceField::Replace => &state.replace_editable,
            };
            let focused = model.ui.focus == FocusTarget::FindBar
                && state.focused_field == *field
                && !model.ui.has_modal();
            let border = if focused {
                model.theme.button.focus_ring.to_argb_u32()
            } else {
                model.theme.tab_bar.border.to_argb_u32()
            };
            frame.draw_bordered_rect(
                rect.x.saturating_sub(1),
                rect.y.saturating_sub(1),
                rect.w + 2,
                rect.h + 2,
                model.theme.overlay.input_background.to_argb_u32(),
                border,
            );
            painter.use_ui_font(false);
            TextFieldRenderer::render_modal_input(
                frame,
                painter,
                input,
                rect,
                model.line_height,
                model.char_width,
                model.theme.overlay.input_background.to_argb_u32() | 0xFF000000,
                model.theme.overlay.foreground.to_argb_u32(),
                model.theme.overlay.highlight.to_argb_u32(),
                model.theme.overlay.selection_background.to_argb_u32(),
                focused && model.ui.cursor_visible,
                model.metrics.scale_factor,
            );
            if input.buffer.as_str().is_empty() && !focused {
                let opts = TextFieldOptions::for_modal(
                    input,
                    rect,
                    model.line_height,
                    model.char_width,
                    model.metrics.scale_factor,
                );
                frame.push_clip(Rect::new(
                    rect.x as f32,
                    rect.y as f32,
                    rect.w as f32,
                    rect.h as f32,
                ));
                painter.draw(
                    frame,
                    opts.x,
                    opts.y,
                    if *field == FindReplaceField::Query {
                        "Find"
                    } else {
                        "Replace"
                    },
                    model.theme.overlay.foreground.to_argb_u32() & 0x80FFFFFF,
                );
                frame.pop_clip();
            }
            painter.use_ui_font(true);
            continue;
        }
        let (label, active) = match control {
            Control::Expand => (if state.replace_mode { "⌄" } else { "›" }, false),
            Control::Close => ("×", false),
            Control::Case => ("Aa", state.case_sensitive),
            Control::Word => ("W", state.whole_word),
            Control::Regex => (".*", state.use_regex),
            Control::Selection => ("≡", state.selection_only),
            Control::Previous => ("↑", false),
            Control::Next => ("↓", false),
            Control::ReplaceNext => ("Replace", false),
            Control::ReplaceAll => ("Replace All", false),
            Control::Field(_) => continue,
        };
        let rect = Rect::new(rect.x as f32, rect.y as f32, rect.w as f32, rect.h as f32);
        frame.push_clip(rect);
        render_button(
            frame,
            painter,
            &model.theme,
            rect,
            label,
            if active {
                ButtonState::Pressed
            } else if hovered {
                ButtonState::Hovered
            } else {
                ButtonState::Normal
            },
            false,
        );
        frame.pop_clip();
    }
    if let Some(status) = state.status(model.document(), &model.editor().selections[0]) {
        let label = status.label();
        let rect = layout.status;
        frame.push_clip(Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.w as f32,
            rect.h as f32,
        ));
        let label = painter.truncate_to_width(&label, rect.w as f32);
        let color = if status.is_error() {
            model.theme.overlay.error
        } else {
            model.theme.overlay.foreground
        };
        painter.draw(
            frame,
            rect.x,
            rect.y + rect.h.saturating_sub(painter.line_height()) / 2,
            &label,
            color.to_argb_u32(),
        );
        frame.pop_clip();
    }
    frame.pop_clip();
    painter.use_ui_font(previous_font);
}
