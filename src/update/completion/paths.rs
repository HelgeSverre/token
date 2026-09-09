use std::sync::Arc;

use super::*;
use crate::completion::menu::{MenuItem, MenuItemKind};
use crate::completion::path::{PathContext, PathRequest, PathResults};
use crate::model::FocusTarget;

fn eligible(model: &AppModel) -> bool {
    model.editor_area.focused_document_id().is_some()
        && model.editor_area.focused_editor_id().is_some()
        && completion_enabled(model)
        && model.ui.focus == FocusTarget::Editor
        && model.ui.active_modal.is_none()
        && model.ui.context_menu.is_none()
        && model.editor().is_plain_text_mode()
        && !model.editor().rectangle_selection.active
        && !model.editor().cursors.is_empty()
        && model.editor().selections.len() == model.editor().cursors.len()
        && model
            .editor()
            .selections
            .iter()
            .all(|selection| selection.is_empty())
}

pub(super) fn valid(model: &AppModel, request: &PathRequest) -> bool {
    eligible(model)
        && model.document().id == Some(request.document_id)
        && model.editor_area.focused_editor_id() == Some(request.editor_id)
        && model.document().revision == request.revision
        && model.document().language == request.language
        && model.document().file_path == request.file_path
        && model.workspace.as_ref().map(|workspace| &workspace.root)
            == request.workspace_root.as_ref()
        && model.editor().cursors == request.cursors
        && model.editor().active_cursor_index == request.active_cursor_index
        && model.ui.completion_menu.as_ref().is_some_and(|menu| {
            menu.context == CompletionContext::Path
                && menu.document_id == request.document_id
                && menu.revision == request.revision
        })
        && model
            .ui
            .cursor_overlay
            .is_none_or(|overlay| overlay.kind == CursorOverlayKind::Completion)
}

pub(in crate::update) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    // Deferred commit acceptance owns the staged character and older snapshot.
    if model.ui.completion_commit.is_some() {
        return None;
    }
    let request = model.ui.completion_path.as_ref()?;
    if valid(model, request) {
        None
    } else {
        dismiss_with_cleanup(model)
    }
}

pub(super) fn open(model: &mut AppModel, explicit: bool) -> Option<Cmd> {
    if !eligible(model) {
        return None;
    }
    let workspace_root = model
        .workspace
        .as_ref()
        .map(|workspace| workspace.root.as_path());
    let context = PathContext::at(
        model.document(),
        *model.editor().active_cursor(),
        workspace_root,
        explicit,
    )?;
    // Absolute path edits are not a sensible fallback for unrelated secondary
    // carets. Offer the source only when every caret has the same path query.
    for &cursor in &model.editor().cursors {
        let peer = PathContext::at(model.document(), cursor, workspace_root, explicit)?;
        if !peer.compatible_with(&context) {
            return None;
        }
    }
    let request = Arc::new(PathRequest {
        explicit,
        document_id: model.document().id?,
        editor_id: model.editor_area.focused_editor_id()?,
        revision: model.document().revision,
        language: model.document().language,
        cursors: model.editor().cursors.clone(),
        active_cursor_index: model.editor().active_cursor_index,
        file_path: model.document().file_path.clone(),
        workspace_root: workspace_root.map(std::path::Path::to_path_buf),
        context,
    });
    let cancel = pending_cancel_cmd(model);
    let carry = model.ui.completion_path.as_ref().is_some_and(|previous| {
        previous.document_id == request.document_id
            && previous.editor_id == request.editor_id
            && previous.file_path == request.file_path
            && previous.language == request.language
            && previous.context.directory == request.context.directory
            && previous.context.start == request.context.start
            && (request.context.query.starts_with(&previous.context.query)
                || previous.context.query.starts_with(&request.context.query))
    });
    let mut items = if carry {
        model
            .ui
            .completion_menu
            .as_mut()
            .map(|menu| std::mem::take(&mut menu.items))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    items.retain(|item| item.source == MenuSourceId::Lsp || request.context.ready);
    let filtered = filter_and_sort(&items, &request.context.query);
    dismiss(model);
    model.ui.completion_menu = Some(CompletionMenuState {
        context: CompletionContext::Path,
        selection_changed: false,
        document_id: request.document_id,
        revision: request.revision,
        query_start: request.context.start,
        query: request.context.query.clone(),
        items,
        filtered,
        is_incomplete: false,
        pending_resolve: None,
    });
    model.ui.cursor_overlay = model
        .ui
        .completion_menu
        .as_ref()
        .filter(|menu| !menu.filtered.is_empty())
        .map(|menu| {
            let mut overlay = CursorOverlayState::new(CursorOverlayKind::Completion);
            overlay.selected = menu.preferred_index();
            overlay
        });
    model.ui.completion_path = Some(Arc::clone(&request));
    let mut cmds = vec![Cmd::Redraw, Cmd::CancelPathCompletion];
    cmds.extend(cancel);
    if request.context.ready {
        cmds.push(Cmd::CompletePaths(Arc::clone(&request)));
    }
    if lsp_capable(model) {
        cmds.push(Cmd::LspScheduleCompletion {
            document_id: request.document_id,
            revision: request.revision,
            position: crate::lsp::position_to_lsp(
                model.document(),
                model.editor().active_cursor().to_position(),
            ),
            trigger_character: None,
        });
    }
    Some(batch_redraw(cmds))
}

pub(super) fn refresh_after_syntax(
    model: &mut AppModel,
    document_id: crate::model::DocumentId,
) -> Option<Cmd> {
    let request = model.ui.completion_path.as_ref()?;
    if request.document_id != document_id
        || request.context.ready
        || !valid(model, request)
        || model
            .ui
            .completion_menu
            .as_ref()
            .is_some_and(|menu| menu.pending_resolve.is_some())
    {
        return None;
    }
    let explicit = request.explicit;
    open(model, explicit).or_else(|| dismiss_with_cleanup(model))
}

pub(super) fn ready(
    model: &mut AppModel,
    request: Arc<PathRequest>,
    result: Result<PathResults, String>,
) -> Option<Cmd> {
    if !request.context.ready {
        return None;
    }
    if !model
        .ui
        .completion_path
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, &request))
        || !valid(model, &request)
    {
        return None;
    }
    let results = match result {
        Ok(results) => results,
        Err(_) => {
            if request.explicit {
                model
                    .ui
                    .set_status("Path completion could not read this directory");
            }
            // A language server can resolve import aliases that do not name a
            // real directory. Failure of the local source must not discard it.
            if !lsp_capable(model)
                && !model.ui.completion_menu.as_ref().is_some_and(|menu| {
                    menu.items
                        .iter()
                        .any(|item| item.source == MenuSourceId::Lsp)
                })
            {
                return dismiss_with_cleanup(model);
            }
            PathResults::default()
        }
    };
    let items: Vec<_> = results
        .entries
        .into_iter()
        .filter_map(|entry| {
            let text = request.context.insertion(&entry)?;
            Some(MenuItem {
                label: if entry.is_directory {
                    format!("{}/", entry.name)
                } else {
                    entry.name.clone()
                },
                filter_text: entry.name,
                insert: MenuInsert::Text(text),
                kind: if entry.is_directory {
                    MenuItemKind::Folder
                } else {
                    MenuItemKind::File
                },
                source: MenuSourceId::Paths,
                detail: Some(
                    if entry.is_directory {
                        "Directory"
                    } else {
                        "File"
                    }
                    .into(),
                ),
                sort_text: None,
                preselect: false,
            })
        })
        .collect();
    let menu = model.ui.completion_menu.as_mut()?;
    if menu.pending_resolve.is_some() {
        return None;
    }
    let previous_overlay = model.ui.cursor_overlay;
    let selected = model
        .ui
        .cursor_overlay
        .and_then(|overlay| menu.selected_item(overlay.selected))
        .map(|item| (item.source, item.label.clone()));
    menu.items.retain(|item| item.source != MenuSourceId::Paths);
    menu.items.extend(items);
    menu.filtered = filter_and_sort(&menu.items, &menu.query);
    model.ui.cursor_overlay = (!menu.filtered.is_empty()).then(|| {
        let mut overlay = previous_overlay
            .unwrap_or_else(|| CursorOverlayState::new(CursorOverlayKind::Completion));
        let preserved = selected.and_then(|(source, label)| {
            menu.filtered.iter().position(|(_, index, _)| {
                menu.items[*index].source == source && menu.items[*index].label == label
            })
        });
        overlay.selected = preserved.unwrap_or_else(|| menu.preferred_index());
        if preserved.is_none() {
            overlay.reset_documentation();
        }
        overlay.scroll = SelectableListViewport::compute_from(
            menu.filtered.len(),
            overlay.selected,
            MAX_VISIBLE_COMPLETION,
            0,
        )
        .scroll_offset;
        overlay
    });
    if results.truncated {
        model
            .ui
            .set_status("Path suggestions limited; type a longer prefix");
    }
    Some(Cmd::Redraw)
}

pub(super) fn accept(model: &mut AppModel, text: &str) -> Option<Cmd> {
    let request = model.ui.completion_path.as_ref()?;
    if !valid(model, request) {
        return dismiss_with_cleanup(model);
    }
    apply(model, text)
}

/// Also used after a validated LSP deferred accept retracts its staged literal.
/// Its old coordinates are restored, though the internal revision has advanced.
pub(super) fn apply(model: &mut AppModel, text: &str) -> Option<Cmd> {
    let request = model.ui.completion_path.as_ref()?;
    let explicit = request.explicit;
    let mut edits = Vec::new();
    for &cursor in &request.cursors {
        let context = PathContext::at(
            model.document(),
            cursor,
            request.workspace_root.as_deref(),
            request.explicit,
        )?;
        if !context.compatible_with(&request.context) {
            return dismiss_with_cleanup(model);
        }
        let start = model
            .document()
            .cursor_to_offset(context.start.line, context.start.column);
        let end = model
            .document()
            .cursor_to_offset(context.end.line, context.end.column);
        edits.push((start, end));
    }
    edits.sort_unstable();
    edits.dedup();
    if edits.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return dismiss_with_cleanup(model);
    }
    let planned: Vec<_> = edits
        .iter()
        .rev()
        .map(|&(start, end)| PlannedEdit {
            start,
            deleted: model.document().buffer.slice(start..end).to_string(),
            inserted: text.to_owned(),
        })
        .collect();
    let map = EditOffsetMap::new(&planned);
    let inserted_len = text.chars().count();
    let offsets: Vec<_> = request
        .cursors
        .iter()
        .map(|cursor| {
            let offset = model
                .document()
                .cursor_to_offset(cursor.line, cursor.column);
            let span = edits
                .partition_point(|&(start, _)| start <= offset)
                .saturating_sub(1);
            map.inserted_offset(planned.len() - 1 - span, inserted_len)
        })
        .collect();
    let directory = text.ends_with('/');
    let applied = finish_accept(model, &planned, &offsets);
    if directory {
        super::super::merge_cmds(applied, open(model, explicit))
    } else {
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::path::PathEntry;
    use crate::messages::{DocumentMsg, Msg, SyntaxMsg};
    use crate::syntax::LanguageId;
    use crate::update::update;

    fn fixture(marked: &str, language: LanguageId) -> AppModel {
        let byte = marked.find('|').unwrap();
        let offset = marked[..byte].chars().count();
        let mut model = AppModel::new(1000, 700, 1.0);
        model.config.lsp.enabled = false;
        model.document_mut().buffer = marked.replace('|', "").into();
        model.document_mut().file_path = Some("/project/main.rs".into());
        model.document_mut().language = language;
        let (line, column) = model.document().offset_to_cursor(offset);
        model.editor_mut().cursors = vec![Cursor::at(line, column)];
        model.editor_mut().clear_selection();
        model
    }

    fn extract(command: Option<Cmd>) -> Option<Arc<PathRequest>> {
        match command? {
            Cmd::CompletePaths(request) => Some(request),
            Cmd::Batch(commands) => commands.into_iter().find_map(|cmd| extract(Some(cmd))),
            _ => None,
        }
    }

    fn trigger(model: &mut AppModel) -> Option<Arc<PathRequest>> {
        extract(update(model, Msg::Completion(CompletionMsg::TriggerMenu)))
    }

    #[test]
    fn completion_menu_config_auto_disabled_preserves_explicit_path_sessions() {
        let mut model = fixture("./|", LanguageId::PlainText);
        model.config.completion.menu.enabled = false;
        let automatic = update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
        assert!(extract(automatic).is_none());
        assert!(model.ui.completion_path.is_none());
        let request = trigger(&mut model).expect("manual path request");
        reply(&mut model, request, "assets", true);
        assert!(model.ui.has_visible_completion());
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('s')));
        let request = model.ui.completion_path.clone().unwrap();
        assert_eq!(request.context.query, "as");
        reply(&mut model, request, "assets", true);
        let next = extract(update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptMenuItem),
        ))
        .expect("manual folder completion continues into its children");
        assert!(next.explicit);
        assert_eq!(model.document().buffer.to_string(), "./assets/");
    }

    fn parse(model: &mut AppModel) -> Option<Arc<PathRequest>> {
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let highlights = crate::syntax::ParserState::new().parse_and_highlight(
            &model.document().buffer.to_string(),
            model.document().language,
            document_id,
            revision,
        );
        extract(update(
            model,
            Msg::Syntax(SyntaxMsg::ParseCompleted {
                document_id,
                revision,
                highlights,
                syntax_tree: None,
                outline: None,
                folds: None,
                timing: Box::default(),
                replace_line_ranges: None,
            }),
        ))
    }

    fn reply(model: &mut AppModel, request: Arc<PathRequest>, name: &str, is_directory: bool) {
        update(
            model,
            Msg::Completion(CompletionMsg::PathsReady {
                request,
                result: Ok(PathResults {
                    entries: vec![PathEntry {
                        name: name.into(),
                        is_directory,
                    }],
                    truncated: false,
                }),
            }),
        );
    }

    fn server_reply(model: &mut AppModel, label: &str, resolve: bool) {
        let items = crate::completion::lsp::items_to_menu_items(
            vec![lsp_types::CompletionItem {
                label: label.into(),
                kind: Some(lsp_types::CompletionItemKind::FILE),
                commit_characters: Some(vec!["!".into()]),
                ..Default::default()
            }],
            &crate::lsp::LspServerId::from("rust-analyzer"),
            std::path::Path::new("/project"),
            Some(&lsp_types::CompletionOptions {
                resolve_provider: Some(resolve),
                ..Default::default()
            }),
        );
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        update(
            model,
            Msg::Lsp(crate::messages::LspMsg::CompletionResolved {
                document_id,
                revision,
                items,
                is_incomplete: false,
            }),
        );
    }

    #[test]
    fn documentation_viewport_survives_local_path_arrival() {
        let mut model = fixture("let s = \"./as|\";", LanguageId::Rust);
        model.config.lsp.enabled = true;
        trigger(&mut model);
        let request = parse(&mut model).unwrap();
        server_reply(&mut model, "asset.rs", false);
        let menu = model.ui.completion_menu.as_mut().unwrap();
        let MenuInsert::Lsp(data) = &mut menu.items[0].insert else {
            panic!("server item");
        };
        data.documentation = Some("documentation\n".repeat(60).into());
        update(
            &mut model,
            Msg::Ui(crate::messages::UiMsg::DocumentationScrolled(7)),
        );
        update(
            &mut model,
            Msg::Ui(crate::messages::UiMsg::ToggleDocumentation),
        );
        reply(&mut model, request, "assets", true);
        let overlay = model.ui.cursor_overlay.unwrap();
        assert_eq!(overlay.documentation.scroll, 7);
        assert!(overlay.documentation.expanded);
        assert_eq!(
            model
                .ui
                .completion_menu
                .as_ref()
                .unwrap()
                .selected_item(overlay.selected)
                .unwrap()
                .label,
            "asset.rs"
        );
    }

    #[test]
    fn path_completion_keeps_server_items_and_full_component_fallback() {
        let mut model = fixture("let s = \"./as-set|old\";", LanguageId::Rust);
        model.config.lsp.enabled = true;
        trigger(&mut model);
        let request = parse(&mut model).unwrap();
        server_reply(&mut model, "as-set-new.rs", false);
        reply(&mut model, Arc::clone(&request), "as-set-local.rs", false);
        let menu = model.ui.completion_menu.as_ref().unwrap();
        assert_eq!(menu.selected_item(0).unwrap().source, MenuSourceId::Lsp);
        assert_eq!(menu.filtered.len(), 2);
        update(
            &mut model,
            Msg::Completion(CompletionMsg::PathsReady {
                request,
                result: Err("directory missing".into()),
            }),
        );
        assert!(
            model.ui.cursor_overlay.is_some(),
            "directory failure cannot discard server aliases"
        );
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert_eq!(
            model.document().buffer.to_string(),
            "let s = \"./as-set-new.rs\";"
        );
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(
            model.document().buffer.to_string(),
            "let s = \"./as-setold\";"
        );
    }

    #[test]
    fn path_completion_server_commit_characters_keep_resolve_and_undo_guards() {
        for resolve in [false, true] {
            let mut model = fixture("let s = \"./as|old\";", LanguageId::Rust);
            model.config.lsp.enabled = true;
            trigger(&mut model);
            let request = parse(&mut model).unwrap();
            server_reply(&mut model, "asset.rs", resolve);
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
            if resolve {
                assert_eq!(model.document().buffer.to_string(), "let s = \"./as!old\";");
                assert!(model.ui.completion_commit.is_some());
                reply(&mut model, Arc::clone(&request), "asset-local.rs", false);
                assert!(
                    model.ui.completion_commit.is_some(),
                    "late directory reply cannot cancel acceptance"
                );
                update(
                    &mut model,
                    Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                        document_id: request.document_id,
                        revision: request.revision,
                        selected: 0,
                        detail: None,
                        documentation: None,
                        additional_text_edits: Vec::new(),
                    }),
                );
            }
            assert_eq!(
                model.document().buffer.to_string(),
                "let s = \"./asset.rs!\";"
            );
            assert!(model.ui.completion_commit.is_none());
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(model.document().buffer.to_string(), "let s = \"./asold\";");
        }
    }

    #[test]
    fn path_completion_failed_directory_keeps_late_server_alias_results() {
        let mut model = fixture("let s = \"alias/as|\";", LanguageId::Rust);
        model.config.lsp.enabled = true;
        trigger(&mut model);
        let request = parse(&mut model).unwrap();
        update(
            &mut model,
            Msg::Completion(CompletionMsg::PathsReady {
                request,
                result: Err("No such directory".into()),
            }),
        );
        assert!(model.ui.completion_menu.is_some());
        assert!(model.ui.cursor_overlay.is_none());
        server_reply(&mut model, "asset.rs", false);
        assert!(model.ui.cursor_overlay.is_some());
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert_eq!(
            model.document().buffer.to_string(),
            "let s = \"alias/asset.rs\";"
        );
    }

    #[test]
    fn path_completion_parse_cannot_withdraw_pending_server_acceptance() {
        let mut model = fixture("let s = \"./as|\";", LanguageId::Rust);
        model.config.lsp.enabled = true;
        trigger(&mut model);
        let request = Arc::clone(model.ui.completion_path.as_ref().unwrap());
        server_reply(&mut model, "asset.rs", true);
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert!(model
            .ui
            .completion_menu
            .as_ref()
            .unwrap()
            .pending_resolve
            .is_some());
        parse(&mut model);
        assert!(model
            .ui
            .completion_menu
            .as_ref()
            .unwrap()
            .pending_resolve
            .is_some());
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: request.document_id,
                revision: request.revision,
                selected: 0,
                detail: None,
                documentation: None,
                additional_text_edits: Vec::new(),
            }),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "let s = \"./asset.rs\";"
        );
    }

    #[test]
    fn path_completion_server_utf16_edit_preserves_suffix_and_import_undo() {
        let mut model = fixture("let s = \"🙂\"; let p = \"./as|old\";", LanguageId::Rust);
        model.config.lsp.enabled = true;
        let before = model.document().buffer.to_string();
        trigger(&mut model);
        let request = parse(&mut model).unwrap();
        let range = lsp_types::Range::new(
            crate::lsp::position_to_lsp(model.document(), request.context.start.to_position()),
            crate::lsp::position_to_lsp(model.document(), request.context.end.to_position()),
        );
        let items = crate::completion::lsp::items_to_menu_items(
            vec![lsp_types::CompletionItem {
                label: "asset.rs".into(),
                text_edit: Some(lsp_types::CompletionTextEdit::Edit(
                    lsp_types::TextEdit::new(range, "asset.rs".into()),
                )),
                additional_text_edits: Some(vec![lsp_types::TextEdit::new(
                    lsp_types::Range::default(),
                    "// import\n".into(),
                )]),
                ..Default::default()
            }],
            &crate::lsp::LspServerId::from("rust-analyzer"),
            std::path::Path::new("/project"),
            None,
        );
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionResolved {
                document_id: request.document_id,
                revision: request.revision,
                items,
                is_incomplete: false,
            }),
        );
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert_eq!(
            model.document().buffer.to_string(),
            "// import\nlet s = \"🙂\"; let p = \"./asset.rs\";"
        );
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), before);
    }

    #[test]
    fn path_completion_multi_cursor_deferred_server_commit_is_one_undo() {
        let mut model = fixture("let a = \"./as\"; let b = \"./as|\";", LanguageId::Rust);
        model.config.lsp.enabled = true;
        let before = model.document().buffer.to_string();
        let cursors: Vec<_> = before
            .match_indices("./as")
            .map(|(start, _)| Cursor::at(0, start + 4))
            .collect();
        model.editor_mut().selections = cursors
            .iter()
            .map(|cursor| crate::model::Selection::new(cursor.to_position()))
            .collect();
        model.editor_mut().cursors = cursors.clone();
        model.editor_mut().active_cursor_index = 1;
        trigger(&mut model);
        let request = parse(&mut model).unwrap();
        server_reply(&mut model, "asset.rs", true);
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
        assert!(model.ui.completion_commit.is_some());
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: request.document_id,
                revision: request.revision,
                selected: 0,
                detail: None,
                documentation: None,
                additional_text_edits: Vec::new(),
            }),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            before.replace("./as", "./asset.rs!")
        );
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), before);
        assert_eq!(model.editor().cursors, cursors);
    }

    #[test]
    fn path_completion_waits_for_syntax_and_drops_comments() {
        for (marked, allowed) in [("let s = \"./as|\";", true), ("// see \"./as|\"", false)] {
            let mut model = fixture(marked, LanguageId::Rust);
            assert!(
                trigger(&mut model).is_none(),
                "no filesystem request before syntax"
            );
            assert!(model.ui.cursor_overlay.is_none());
            assert_eq!(parse(&mut model).is_some(), allowed);
            assert_eq!(model.ui.completion_path.is_some(), allowed);
        }
    }

    #[test]
    fn path_completion_replaces_suffix_and_undo_restores_caret_and_status() {
        let mut model = fixture("[x](./hé|old)", LanguageId::Markdown);
        let before = model.document().buffer.to_string();
        let cursors = model.editor().cursors.clone();
        let request = trigger(&mut model).unwrap();
        reply(&mut model, request, "hé llo.md", false);
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert_eq!(model.document().buffer.to_string(), "[x](./hé%20llo.md)");
        assert_eq!(model.editor().active_cursor().column, 17);
        assert_eq!(model.document().undo_stack.len(), 1);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), before);
        assert_eq!(model.editor().cursors, cursors);
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(model.document().buffer.to_string(), "[x](./hé%20llo.md)");
        assert_eq!(model.editor().active_cursor().column, 17);
    }

    #[test]
    fn path_completion_directory_accept_reopens_and_does_not_double_separator() {
        for marked in ["./as|", "./as|/child"] {
            let mut model = fixture(marked, LanguageId::PlainText);
            let request = trigger(&mut model).unwrap();
            reply(&mut model, request, "assets", true);
            let next = extract(update(
                &mut model,
                Msg::Completion(CompletionMsg::AcceptMenuItem),
            ))
            .unwrap();
            assert!(model.document().buffer.to_string().starts_with("./assets/"));
            assert!(!model.document().buffer.to_string().contains("//"));
            assert!(next.context.query.is_empty());
            assert_eq!(model.editor().active_cursor().column, 9);
            assert_eq!(model.document().undo_stack.len(), 1);
        }
    }

    #[test]
    fn path_completion_rejects_superseded_dismissed_and_reopened_requests() {
        let mut model = fixture("./as|", LanguageId::PlainText);
        let first = trigger(&mut model).unwrap();
        update(&mut model, Msg::Completion(CompletionMsg::Dismiss));
        let second = trigger(&mut model).unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        reply(&mut model, first, "assets", true);
        assert!(model.ui.cursor_overlay.is_none());
        let newer = extract(update(
            &mut model,
            Msg::Document(DocumentMsg::InsertChar('s')),
        ))
        .unwrap();
        reply(&mut model, second, "assets", true);
        assert!(model.ui.cursor_overlay.is_none());
        reply(&mut model, newer, "assets", true);
        assert!(model.ui.cursor_overlay.is_some());
    }

    #[test]
    fn path_completion_guards_focus_file_language_selection_and_configuration() {
        for change in 0..5 {
            let mut model = fixture("./as|", LanguageId::PlainText);
            let request = trigger(&mut model).unwrap();
            match change {
                0 => model.ui.focus = FocusTarget::Modal,
                1 => model.document_mut().file_path = Some("/other/main.rs".into()),
                2 => model.document_mut().language = LanguageId::Rust,
                3 => model.editor_mut().selections[0].anchor.column = 0,
                4 => model.config.completion.enabled = false,
                _ => unreachable!(),
            }
            reply(&mut model, request, "assets", true);
            assert!(model.ui.completion_path.is_none());
            assert!(model.ui.cursor_overlay.is_none());
            assert_eq!(model.document().buffer.to_string(), "./as");
        }
    }

    #[test]
    fn path_completion_multi_cursor_and_peer_pane_undo_share_the_transaction() {
        let mut model = fixture("./as ./as|", LanguageId::PlainText);
        update(
            &mut model,
            Msg::Layout(crate::messages::LayoutMsg::SplitFocused(
                crate::model::SplitDirection::Vertical,
            )),
        );
        model.editor_mut().cursors = vec![Cursor::at(0, 4), Cursor::at(0, 9)];
        model.editor_mut().selections = vec![
            crate::model::Selection::new(Cursor::at(0, 4).to_position()),
            crate::model::Selection::new(Cursor::at(0, 9).to_position()),
        ];
        let before: Vec<_> = model
            .editor_area
            .editors
            .iter()
            .map(|(&id, editor)| (id, editor.cursors.clone(), editor.selections.clone()))
            .collect();
        let request = trigger(&mut model).unwrap();
        reply(&mut model, request, "asset.txt", false);
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert_eq!(
            model.document().buffer.to_string(),
            "./asset.txt ./asset.txt"
        );
        assert_eq!(
            model.editor().cursors,
            vec![Cursor::at(0, 11), Cursor::at(0, 23)]
        );
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "./as ./as");
        for (id, cursors, selections) in before {
            assert_eq!(model.editor_area.editors[&id].cursors, cursors);
            assert_eq!(model.editor_area.editors[&id].selections, selections);
        }
    }
}
