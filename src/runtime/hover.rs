//! Pointer timing for documentation; ownership/invalidation lives in update.

use std::time::{Duration, Instant};
use token::commands::Cmd;
use token::messages::{LspMsg, Msg};
use token::model::hover::{mouse_hover_allowed, same_hover_target, HoverAnchor, HoverOrigin};
use token::model::{CursorOverlayKind, HoverRegion, Position};
use token::update::update;
use winit::event::{ElementState, WindowEvent};

use super::App;

const HOVER_HIDE_DELAY: Duration = Duration::from_millis(300);

#[derive(Clone, Copy)]
pub(super) struct HoverDwell {
    pub anchor: HoverAnchor,
    pub position: Position,
    pub started: Instant,
}

impl App {
    fn pointer_in_hover_card(&self) -> bool {
        self.model.ui.hover == HoverRegion::CursorOverlay
            && self.model.ui.cursor_overlay.is_some_and(|overlay| {
                matches!(
                    overlay.kind,
                    CursorOverlayKind::Hover | CursorOverlayKind::DebugHover
                )
            })
    }

    fn mouse_documentation(&self) -> bool {
        self.model
            .ui
            .hover_request
            .is_some_and(|request| request.origin == HoverOrigin::Mouse)
    }

    /// Clear timers along with the model intent. Returning the command keeps
    /// cancellation/redraw effects intact even when the event handler returns early.
    fn dismiss_documentation(&mut self) -> Option<Cmd> {
        self.hover_dwell = None;
        self.hover_hide_at = None;
        if !self.model.ui.has_hover() {
            return None;
        }
        update(&mut self.model, Msg::Lsp(LspMsg::DismissHover))
    }

    pub(super) fn prepare_hover_event(&mut self, event: &WindowEvent) -> Option<Cmd> {
        let dismiss = match event {
            WindowEvent::KeyboardInput { event, .. } => {
                event.state == ElementState::Pressed
                    && !crate::runtime::input::is_modifier_key(&event.logical_key)
            }
            WindowEvent::Focused(false)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. } => true,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            }
            | WindowEvent::MouseWheel { .. } => !self.pointer_in_hover_card(),
            WindowEvent::CursorLeft { .. } => {
                self.hover_dwell = None;
                self.mouse_documentation()
            }
            _ => false,
        };
        dismiss.then(|| self.dismiss_documentation()).flatten()
    }

    fn pointer_target(&self) -> Option<Position> {
        if self.model.ui.hover != HoverRegion::EditorText
            || self.drag.is_down()
            || !self.modifiers.is_empty()
            || self
                .window
                .as_ref()
                .is_some_and(|window| !window.has_focus())
        {
            return None;
        }
        let (x, y) = self.mouse_position?;
        token::view::geometry::hover_position(x, y, &self.model)
    }

    /// Called after shared hit testing. Moving inside a symbol preserves its
    /// dwell; leaving a shown card starts a short grace period for entering it.
    pub(super) fn update_hover_dwell(&mut self) -> Option<Cmd> {
        if self
            .model
            .ui
            .hover_request
            .is_some_and(|request| request.origin == HoverOrigin::Keyboard)
        {
            self.hover_dwell = None;
            self.hover_hide_at = None;
            return None;
        }
        if self.pointer_in_hover_card() {
            self.hover_hide_at = None;
            self.hover_dwell = None;
            return None;
        }
        let anchor = HoverAnchor::capture(&self.model);
        let target = self.pointer_target();
        if let Some(request) = self.model.ui.hover_request {
            if anchor == Some(request.anchor)
                && target.is_some_and(|target| {
                    same_hover_target(self.model.document(), request.position, target)
                })
            {
                self.hover_hide_at = None;
                return None;
            }
            if self.model.ui.hover_card.is_some() {
                self.hover_hide_at
                    .get_or_insert(Instant::now() + HOVER_HIDE_DELAY);
                return None;
            }
        }
        let dismissed = self
            .model
            .ui
            .hover_request
            .is_some()
            .then(|| self.dismiss_documentation())
            .flatten();
        if let (true, Some(anchor), Some(position)) =
            (mouse_hover_allowed(&self.model), anchor, target)
        {
            if !self.hover_dwell.is_some_and(|dwell| {
                dwell.anchor == anchor
                    && same_hover_target(self.model.document(), dwell.position, position)
            }) {
                self.hover_dwell = Some(HoverDwell {
                    anchor,
                    position,
                    started: Instant::now(),
                });
            }
        } else {
            self.hover_dwell = None;
        }
        dismissed
    }

    pub(super) fn check_hover_dwell(&mut self) -> bool {
        let anchor = HoverAnchor::capture(&self.model);
        if !self.mouse_documentation() {
            self.hover_hide_at = None;
        }
        if self
            .model
            .ui
            .hover_request
            .is_some_and(|request| anchor != Some(request.anchor))
        {
            let cmd = self.dismiss_documentation();
            return self.apply_hover_command(cmd);
        }
        if self
            .hover_hide_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            let cmd = self.dismiss_documentation();
            let redraw = self.apply_hover_command(cmd);
            // A pointer that settled on a different word should not need an
            // extra wiggle after the old card's grace period ends.
            let cmd = self.update_hover_dwell();
            return self.apply_hover_command(cmd) || redraw;
        }
        let Some(dwell) = self.hover_dwell else {
            return false;
        };
        if anchor != Some(dwell.anchor)
            || !mouse_hover_allowed(&self.model)
            || !self.pointer_target().is_some_and(|target| {
                same_hover_target(self.model.document(), dwell.position, target)
            })
        {
            self.hover_dwell = None;
            return false;
        }
        if dwell.started.elapsed() < Duration::from_millis(self.model.config.hover_delay_ms) {
            return false;
        }
        self.hover_dwell = None;
        let cmd = update(
            &mut self.model,
            Msg::Lsp(LspMsg::ShowHoverAt {
                line: dwell.position.line,
                col: dwell.position.column,
            }),
        );
        self.apply_hover_command(cmd)
    }

    fn apply_hover_command(&mut self, cmd: Option<Cmd>) -> bool {
        let Some(cmd) = cmd else {
            return false;
        };
        let redraw = cmd.needs_redraw();
        self.pending_damage.merge(cmd.damage());
        self.process_cmd(cmd);
        redraw
    }
}
