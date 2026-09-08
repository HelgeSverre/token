mod common;

use token::lsp::workspace_symbols::{
    SymbolItem, SymbolProvider, SymbolResults, SymbolSearchRequest,
};
use token::messages::{LayoutMsg, LspMsg, ModalMsg, Msg, UiMsg};
use token::model::{
    CommandPaletteState, Document, ModalState, PreparedFile, SearchTab, TabContent, ViewMode,
};
use token::update::update;
use token::{AppModel, Cmd};

fn modal(model: &mut AppModel, message: ModalMsg) -> Option<Cmd> {
    update(model, Msg::Ui(UiMsg::Modal(message)))
}

fn palette(model: &AppModel) -> &CommandPaletteState {
    let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal else {
        panic!("palette")
    };
    state
}

fn fixture() -> (AppModel, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let mut model = common::test_model("origin", 0, 2);
    model.open_workspace(directory.path().to_path_buf());
    let provider = SymbolProvider {
        server_id: "rust-analyzer".into(),
        root: model.workspace_root().unwrap().clone(),
        generation: 7,
    };
    update(
        &mut model,
        Msg::Lsp(LspMsg::WorkspaceSymbolProviders(vec![provider])),
    );
    modal(&mut model, ModalMsg::OpenCommandPalette);
    (model, directory)
}

fn reply(model: &mut AppModel, request: SymbolSearchRequest, count: usize) {
    let provider = request.providers[0].clone();
    let items = (0..count)
        .map(|index| SymbolItem {
            name: format!("symbol{index:02}"),
            detail: "symbols.rs".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            location: lsp_types::Location {
                uri: token::lsp::path_to_uri(&provider.root.join("symbols.rs")),
                range: lsp_types::Range::new(
                    lsp_types::Position::new(index as u32, 3),
                    lsp_types::Position::new(index as u32, 4),
                ),
            },
            provider: provider.clone(),
        })
        .collect();
    update(
        model,
        Msg::Lsp(LspMsg::WorkspaceSymbolsReady {
            request,
            results: SymbolResults {
                items,
                ..Default::default()
            },
        }),
    );
}

#[test]
fn workspace_symbols_queries_reject_aba_stale_replies_and_cancel_on_close() {
    let (mut model, _directory) = fixture();
    let initial = model.ui.workspace_symbol_request.clone().unwrap();
    assert!(palette(&model).symbols.searching);
    modal(&mut model, ModalMsg::SetInput("alpha".into()));
    let alpha = model.ui.workspace_symbol_request.clone().unwrap();
    assert_ne!(alpha.id, initial.id);
    reply(&mut model, initial, 2);
    assert!(palette(&model).symbols.results.items.is_empty());
    modal(&mut model, ModalMsg::Close);
    assert!(model.ui.workspace_symbol_request.is_none());
    modal(&mut model, ModalMsg::OpenCommandPalette);
    modal(&mut model, ModalMsg::SetInput("alpha".into()));
    let fresh = model.ui.workspace_symbol_request.clone().unwrap();
    assert_ne!(fresh.id, alpha.id);
    reply(&mut model, alpha, 2);
    assert!(palette(&model).symbols.results.items.is_empty());
    reply(&mut model, fresh, 3);
    assert_eq!(palette(&model).symbols.results.items.len(), 3);
    assert!(!palette(&model).symbols.searching);
}

#[test]
fn workspace_symbols_tab_prefix_navigation_and_provider_restart() {
    let (mut model, _directory) = fixture();
    modal(&mut model, ModalMsg::InsertChar('@'));
    assert_eq!(palette(&model).active_tab, SearchTab::Symbols);
    let request = model.ui.workspace_symbol_request.clone().unwrap();
    reply(&mut model, request.clone(), 30);
    modal(&mut model, ModalMsg::SelectNext);
    assert_eq!(palette(&model).symbols.selected_index, 1);
    modal(&mut model, ModalMsg::PageDown);
    assert!(palette(&model).symbols.selected_index > 1);
    let selected = palette(&model).symbols.selected_index;
    modal(&mut model, ModalMsg::Scroll(5));
    assert_eq!(palette(&model).symbols.selected_index, selected);
    assert!(palette(&model).symbols.scroll_offset > 0);
    modal(&mut model, ModalMsg::ActivateTab(1));
    assert!(model.ui.workspace_symbol_request.is_none());
    modal(&mut model, ModalMsg::ActivateTab(3));
    let mut providers = request.providers.clone();
    providers[0].generation += 1;
    update(
        &mut model,
        Msg::Lsp(LspMsg::WorkspaceSymbolProviders(providers)),
    );
    let restarted = model.ui.workspace_symbol_request.clone().unwrap();
    assert_ne!(restarted.id, request.id);
    reply(&mut model, request, 2);
    assert!(palette(&model).symbols.results.items.is_empty());
    update(
        &mut model,
        Msg::Lsp(LspMsg::WorkspaceSymbolProviders(Vec::new())),
    );
    assert!(!palette(&model).symbols.available);
    assert!(model.ui.workspace_symbol_request.is_none());
}

#[test]
fn workspace_symbols_long_queries_and_disabled_servers_do_not_request() {
    let (mut model, _directory) = fixture();
    modal(&mut model, ModalMsg::SetInput("é".repeat(257)));
    assert!(model.ui.workspace_symbol_request.is_none());
    assert!(palette(&model).symbols.query_too_long);
    assert!(palette(&model).symbols.status().unwrap().contains("256"));
    modal(&mut model, ModalMsg::SetInput("é".repeat(256)));
    assert!(model.ui.workspace_symbol_request.is_some());
    model.config.lsp.enabled = false;
    modal(&mut model, ModalMsg::SelectNext);
    assert!(model.ui.workspace_symbol_request.is_none());
    assert!(!palette(&model).symbols.available);
}

#[test]
fn workspace_symbols_follow_workspace_scope_and_per_server_enablement() {
    let (mut model, _directory) = fixture();
    let initial = model.ui.workspace_symbol_request.clone().unwrap();
    let mut unrelated = initial.providers[0].clone();
    unrelated.server_id = "unrelated".into();
    unrelated.root = "/not-this-workspace".into();
    let mut providers = initial.providers.clone();
    providers.push(unrelated);
    update(
        &mut model,
        Msg::Lsp(LspMsg::WorkspaceSymbolProviders(providers)),
    );
    assert_eq!(model.ui.workspace_symbol_request, Some(initial.clone()));
    model
        .config
        .lsp
        .servers
        .entry("rust-analyzer".into())
        .or_default()
        .enabled = Some(false);
    modal(&mut model, ModalMsg::SelectNext);
    assert!(model.ui.workspace_symbol_request.is_none());
    model.config.lsp.servers.clear();
    modal(&mut model, ModalMsg::SelectNext);
    assert!(model.ui.workspace_symbol_request.is_some());
    let other = tempfile::tempdir().unwrap();
    model.open_workspace(other.path().to_path_buf());
    modal(&mut model, ModalMsg::SelectNext);
    assert!(model.ui.workspace_symbol_request.is_none());
    reply(&mut model, initial, 2);
    assert!(palette(&model).symbols.results.items.is_empty());
    assert!(!palette(&model).symbols.available);
}

fn file_request(command: Cmd) -> Option<token::model::FileOpenRequest> {
    match command {
        Cmd::PrepareFileOpen(request) => Some(request),
        Cmd::Batch(commands) => commands.into_iter().find_map(file_request),
        _ => None,
    }
}

#[test]
fn workspace_symbols_all_and_symbols_click_open_with_utf16_and_history() {
    for tab in [0, 3] {
        let (mut model, _directory) = fixture();
        model.document_mut().file_path = Some(model.workspace_root().unwrap().join("origin.rs"));
        modal(&mut model, ModalMsg::ActivateTab(tab));
        let request = model.ui.workspace_symbol_request.clone().unwrap();
        reply(&mut model, request, 1);
        let state = palette(&model);
        let row = if tab == 0 {
            token::update::search_everywhere_sections(state)
                .iter()
                .take_while(|(title, _)| *title != Some("Symbols"))
                .map(|(_, count)| count)
                .sum()
        } else {
            0
        };
        let command = modal(&mut model, ModalMsg::ActivateRow(row)).unwrap();
        assert!(model.ui.active_modal.is_none());
        assert!(model.ui.workspace_symbol_request.is_none());
        assert!(
            model.lsp.route_hint.is_none(),
            "route belongs to the pending file open"
        );
        let request = file_request(command).expect("deferred file open");
        assert_eq!(model.document().buffer.to_string(), "origin");
        let mut document = Document::with_text("a😀target");
        document.file_path = request.source.path().map(std::path::Path::to_path_buf);
        update(
            &mut model,
            Msg::Layout(LayoutMsg::FilePrepared {
                request,
                result: Ok(Box::new(PreparedFile::Loaded {
                    document: Box::new(document),
                    view_mode: ViewMode::Text,
                    tab_content: TabContent::Text,
                })),
            }),
        );
        assert_eq!(model.editor().active_cursor().column, 2);
        assert_eq!(model.jump_history.len(), 1);
        assert_eq!(model.document().buffer.to_string(), "a😀target");
        assert!(!model.document().is_modified);
    }
}
