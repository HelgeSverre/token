//! Identity consumers must use the same snapshot, without consulting disk.

use std::path::Path;
#[cfg(unix)]
use token::messages::LspMsg;
use token::messages::{LayoutMsg, Msg};
use token::{update::update, AppModel};

#[cfg(unix)]
#[test]
fn file_identity_renaming_symlink_agrees_across_tabs_lsp_and_problems_without_disk() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real.rs");
    let alias = dir.path().join("different-name.rs");
    std::fs::write(&real, "fn café() {}\n").unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let mut model = AppModel::with_document(
        800,
        600,
        1.0,
        token::model::Document::from_file(alias.clone()).unwrap(),
    );
    let id = model.document().id.unwrap();
    let identity = model.document().file_identity().unwrap().clone();
    let diagnostic = lsp_types::Diagnostic::new_simple(
        lsp_types::Range::default(),
        "identity regression".into(),
    );
    // Both spellings are now unavailable on disk. The open buffer still owns
    // its original identity; lookups must not silently drop its diagnostics.
    std::fs::rename(&real, dir.path().join("moved.rs")).unwrap();
    assert!(!alias.exists());
    assert_eq!(
        model.editor_area.find_open_file(identity.path()).unwrap().0,
        id
    );
    update(
        &mut model,
        Msg::Lsp(LspMsg::DiagnosticsPublished {
            uri: identity.uri().clone(),
            version: None,
            diagnostics: vec![diagnostic.clone()],
        }),
    );
    assert_eq!(model.document().diagnostics, vec![diagnostic]);
    assert_eq!(token::update::problems::problems_row_count(&model), 2);
    update(&mut model, Msg::Layout(LayoutMsg::NewTab));
    let cmd = update(
        &mut model,
        Msg::Layout(LayoutMsg::OpenFileInNewTab(identity.path().into())),
    )
    .unwrap();
    assert!(
        !matches!(cmd, token::Cmd::PrepareFileOpen(_)),
        "known aliases reuse the buffer directly"
    );
    assert_eq!(model.document().id, Some(id));
    assert_eq!(model.document().file_path.as_deref(), Some(alias.as_path()));
    assert!(!model.ui.is_loading);
}

#[test]
fn file_identity_known_path_reuse_prefers_the_focused_group() {
    let path = Path::new("/fixture/missing.rs");
    let mut model = AppModel::with_document(
        800,
        600,
        1.0,
        token::model::Document::new_with_path(path.into()),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(
            token::model::SplitDirection::Vertical,
        )),
    );
    assert_eq!(
        model.editor_area.find_open_file(path).unwrap().1,
        model.editor_area.focused_group_id
    );
}
