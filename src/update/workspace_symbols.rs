//! One query lifecycle for Symbols and the All-tab summary.

use crate::commands::Cmd;
use crate::lsp::workspace_symbols::{SymbolItem, SymbolSearchRequest};
use crate::model::{AppModel, ModalState, SearchTab};

pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    if !matches!(model.ui.active_modal, Some(ModalState::CommandPalette(_))) {
        return model
            .ui
            .workspace_symbol_request
            .take()
            .map(|_| Cmd::WorkspaceSymbols(None));
    }
    let workspace = model.workspace_root().cloned();
    let providers: Vec<_> = model
        .lsp
        .workspace_symbol_providers
        .iter()
        .filter(|provider| {
            model.config.lsp.enabled
                && !model
                    .config
                    .lsp
                    .servers
                    .get(&provider.server_id.0)
                    .is_some_and(|settings| settings.enabled == Some(false))
                && workspace.as_ref().is_some_and(|root| {
                    provider.root.starts_with(root) || root.starts_with(&provider.root)
                })
        })
        .cloned()
        .collect();
    let desired = if let Some(ModalState::CommandPalette(state)) = &mut model.ui.active_modal {
        state.symbols.available = !providers.is_empty();
        let query = state.input();
        state.symbols.query_too_long =
            query.chars().count() > crate::lsp::workspace_symbols::MAX_QUERY_CHARS;
        if state.symbols.available
            && !state.symbols.query_too_long
            && matches!(state.active_tab, SearchTab::All | SearchTab::Symbols)
        {
            workspace.map(|workspace| (query, workspace))
        } else {
            None
        }
    } else {
        None
    };
    let Some((query, workspace)) = desired else {
        if let Some(ModalState::CommandPalette(state)) = &mut model.ui.active_modal {
            state.symbols.searching = false;
            if !state.symbols.available || state.symbols.query_too_long {
                state.symbols.results = Default::default();
            }
        }
        return model
            .ui
            .workspace_symbol_request
            .take()
            .map(|_| Cmd::WorkspaceSymbols(None));
    };
    if model
        .ui
        .workspace_symbol_request
        .as_ref()
        .is_some_and(|old| {
            old.query == query && old.workspace == workspace && old.providers == providers
        })
    {
        return None;
    }
    model.ui.next_workspace_symbol_request = model.ui.next_workspace_symbol_request.wrapping_add(1);
    let request = SymbolSearchRequest {
        id: model.ui.next_workspace_symbol_request,
        query,
        workspace,
        providers,
    };
    model.ui.workspace_symbol_request = Some(request.clone());
    if let Some(ModalState::CommandPalette(state)) = &mut model.ui.active_modal {
        state.symbols.results = Default::default();
        state.symbols.searching = true;
        state.symbols.selected_index = 0;
        state.symbols.scroll_offset = 0;
        state.all_selected = 0;
    }
    Some(Cmd::Batch(vec![
        Cmd::WorkspaceSymbols(Some(request)),
        Cmd::Redraw,
    ]))
}

pub(super) fn open(model: &mut AppModel, symbol: SymbolItem) -> Option<Cmd> {
    let path = crate::lsp::uri_to_path(&symbol.location.uri)?;
    let origin = super::navigation::current_jump_entry(model);
    model.ui.close_modal();
    model.lsp.route_hint = Some((
        path.clone(),
        symbol.provider.server_id,
        symbol.provider.root,
    ));
    super::navigation::jump_to_location(model, origin, &path, symbol.location.range.start)
}
