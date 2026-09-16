//! Pure completion presentation and trigger policy.

use crate::model::{AppModel, FocusTarget};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompletionInteraction {
    pub menu_visible: bool,
    pub inline_visible: bool,
    pub menu_session_pending: bool,
    pub can_accept_menu: bool,
    pub can_accept_inline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerIntent {
    AutomaticMenu,
    ExplicitMenu,
    AutomaticInline,
    ExplicitInline,
}

fn editor_eligible(model: &AppModel) -> bool {
    model.ui.focus == FocusTarget::Editor
        && !model.ui.has_modal()
        && model.ui.context_menu.is_none()
        && model.editor().is_plain_text_mode()
        && !model.editor().rectangle_selection.active
        && model.editor().active_selection().is_empty()
}

pub fn interaction(model: &AppModel) -> CompletionInteraction {
    let eligible = editor_eligible(model);
    let has_menu = model.ui.completion.completion_menu.is_some();
    let menu_visible = eligible
        && model
            .ui
            .completion
            .completion_menu
            .as_ref()
            .is_some_and(|menu| !menu.filtered.is_empty());
    let inline_visible = eligible
        && !menu_visible
        && model.config.completion.enabled
        && model.config.completion.inline.enabled
        && model
            .ui
            .completion
            .inline_session
            .as_ref()
            .is_some_and(|session| {
                model.editor().id == session.editor_id
                    && model
                        .config
                        .completion
                        .providers
                        .get(&model.config.completion.inline.provider)
                        == Some(&session.provider)
            })
        && model
            .ui
            .completion
            .inline_suggestion
            .as_ref()
            .is_some_and(|proposal| {
                let cursor = model.editor().active_cursor();
                proposal.applies_to(model.document(), (cursor.line, cursor.column))
                    && !proposal.remaining().is_empty()
            });
    CompletionInteraction {
        menu_visible,
        inline_visible,
        menu_session_pending: has_menu && !menu_visible,
        can_accept_menu: menu_visible,
        can_accept_inline: inline_visible,
    }
}

pub fn may_trigger(model: &AppModel, intent: TriggerIntent) -> bool {
    if !model.config.completion.enabled || !editor_eligible(model) {
        return false;
    }
    match intent {
        TriggerIntent::AutomaticMenu => {
            model.config.completion.menu.enabled && !interaction(model).inline_visible
        }
        TriggerIntent::ExplicitMenu => true,
        TriggerIntent::AutomaticInline | TriggerIntent::ExplicitInline => {
            model.config.completion.inline.enabled
                && model
                    .config
                    .completion
                    .providers
                    .contains_key(&model.config.completion.inline.provider)
                && !interaction(model).menu_visible
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_menu_is_distinct_from_visible_menu() {
        let model = AppModel::new(800, 600, 1.0);
        let state = interaction(&model);
        assert!(!state.menu_visible);
        assert!(!state.menu_session_pending);
    }
}
