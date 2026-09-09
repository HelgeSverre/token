//! Documentation intent and lifetime, independent of pointer timing/rendering.

use crate::commands::Cmd;
use crate::messages::HoverOutcome;
use crate::model::hover::{mouse_hover_allowed, HoverAnchor, HoverOrigin, HoverRequest};
use crate::model::{AppModel, CursorOverlayKind, CursorOverlayState, HoverCardState, Position};

pub(super) fn show(model: &mut AppModel, position: Position, origin: HoverOrigin) -> Option<Cmd> {
    if origin == HoverOrigin::Mouse && !mouse_hover_allowed(model) {
        return None;
    }
    let anchor = HoverAnchor::capture(model)?;
    let document = model.try_document()?;
    document.file_path.as_ref()?;
    let lsp_position = crate::lsp::position_to_lsp(document, position);
    let dismissed = dismiss(model);
    let completion = if origin == HoverOrigin::Keyboard {
        model.ui.signature_help = None;
        super::completion::update_completion(model, crate::messages::CompletionMsg::Dismiss)
    } else {
        None
    };
    model.ui.hover_request = Some(HoverRequest {
        anchor,
        position,
        origin,
    });
    let request = super::merge_cmds(
        completion,
        Some(Cmd::LspRequestHover {
            document_id: anchor.document_id,
            revision: anchor.revision,
            cursor: position,
            position: lsp_position,
        }),
    );
    super::merge_cmds(dismissed, request)
}

pub(super) fn dismiss(model: &mut AppModel) -> Option<Cmd> {
    let request = model.ui.hover_request;
    model.ui.dismiss_hover().then(|| match request {
        Some(request) => Cmd::Batch(vec![
            Cmd::Redraw,
            Cmd::LspCancelHover {
                document_id: request.anchor.document_id,
            },
        ]),
        None => Cmd::Redraw,
    })
}

pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    let request = model.ui.hover_request?;
    let competing_overlay = model
        .ui
        .cursor_overlay
        .is_some_and(|overlay| overlay.kind != CursorOverlayKind::Hover);
    if HoverAnchor::capture(model) != Some(request.anchor)
        || competing_overlay
        || (request.origin == HoverOrigin::Mouse
            && (!model.config.hover_on_mouse || model.ui.signature_help.is_some()))
    {
        return dismiss(model);
    }
    None
}

pub(super) fn resolved(
    model: &mut AppModel,
    document_id: crate::model::DocumentId,
    revision: u64,
    cursor: Position,
    outcome: HoverOutcome,
) -> Option<Cmd> {
    let request = model.ui.hover_request?;
    if request.anchor.document_id != document_id
        || request.anchor.revision != revision
        || request.position != cursor
    {
        return None;
    }
    if let Some(cmd) = reconcile(model) {
        return Some(cmd);
    }
    let mouse = request.origin == HoverOrigin::Mouse;
    let message = match outcome {
        HoverOutcome::Content(content) => {
            let has_diagnostics =
                !crate::model::decorations::diagnostics_at_position(model.document(), cursor)
                    .is_empty();
            if content.is_some() || has_diagnostics {
                model.ui.hover_card = Some(HoverCardState {
                    content,
                    anchor: mouse.then_some((cursor.line, cursor.column)),
                });
                model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Hover));
                return Some(Cmd::Redraw);
            }
            "No hover information"
        }
        HoverOutcome::StillIndexing => "Language server still indexing…",
        HoverOutcome::NotSupported => "Hover not supported by this server",
    };
    // Empty/unsupported automatic hovers are silent. Keep their target until
    // movement so a resting pointer cannot repeatedly request the same result.
    if mouse {
        None
    } else {
        // There is no keyboard card left to preserve. Do not let an empty
        // explicit request suppress subsequent mouse documentation indefinitely.
        model.ui.hover_request = None;
        model.ui.set_status(message);
        Some(Cmd::redraw_status_bar())
    }
}
