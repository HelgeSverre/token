//! File-policy generations, live reload, and save continuations. No filesystem I/O.
use crate::{
    commands::Cmd,
    editorconfig::{PolicyRequest, ResolvedFilePolicy},
    model::{AppModel, DocumentId, SaveIntent, SaveReason},
};
use std::path::PathBuf;

fn source_path(document: &crate::model::Document) -> Option<PathBuf> {
    document
        .file_identity()
        .map(|id| id.path().to_path_buf())
        .or_else(|| document.file_path.clone())
}

pub(super) fn changed(model: &mut AppModel, paths: &[PathBuf]) {
    if !model.config.editorconfig {
        return;
    }
    for doc in model.editor_area.documents.values_mut() {
        // Native events may use an alias spelling (e.g. /var vs /private/var
        // on macOS). Re-resolving through the physical file remains authoritative.
        let source_ancestor_changed = doc.file_path.as_ref().is_some_and(|source| {
            source.ancestors().skip(1).any(|parent| {
                paths
                    .iter()
                    .any(|path| parent.join(".editorconfig") == *path || parent.starts_with(path))
            })
        });
        let affected = |policy: &ResolvedFilePolicy| {
            paths.is_empty()
                || source_ancestor_changed
                || policy
                    .dependencies
                    .iter()
                    .any(|config| paths.iter().any(|path| config.starts_with(path)))
        };
        if doc.pending_save_policy().is_some_and(affected) {
            if let Some(save) = &mut doc.pending_save {
                save.destination_policy = None;
            }
            doc.file_policy.pending = None;
        }
        if doc.file_policy.resolved.as_deref().is_some_and(affected) {
            doc.file_policy.invalidated = true;
            doc.file_policy.generation = doc.file_policy.generation.wrapping_add(1);
            doc.file_policy.pending = None;
        }
    }
}

pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    let mut commands = Vec::new();
    let mut resume = Vec::new();
    let enabled = model.config.editorconfig;
    for (&id, doc) in &mut model.editor_area.documents {
        // Worker-prepared paths opt into tracking. Synthetic/untitled buffers
        // have no discovery dependency until their first Save As.
        let managed = doc.file_policy.path.is_some()
            || doc.file_policy.enabled
            || doc.file_policy.pending.is_some();
        let path = source_path(doc);
        if !managed {
            continue;
        }
        if doc.file_policy.enabled != enabled || (!enabled && doc.file_policy.pending.is_some()) {
            let was_waiting = doc.file_policy.pending.is_some();
            doc.file_policy.enabled = enabled;
            doc.file_policy.generation = doc.file_policy.generation.wrapping_add(1);
            doc.file_policy.pending = None;
            doc.file_policy.invalidated = enabled;
            if !enabled {
                doc.file_policy.resolved = None;
                doc.file_text_preferences = Default::default();
                doc.resolve_text_settings(model.config.text);
                if was_waiting {
                    if let Some(mut save) = doc.pending_save.clone() {
                        save.settings = doc.text_settings;
                        save.destination_policy = None;
                        save.text_policy_generation = doc.text_policy_generation;
                        doc.pending_save = Some(save.clone());
                        resume.push(save);
                    }
                }
            }
        }
        if !enabled {
            continue;
        }
        if let Some(save) = doc
            .pending_save
            .clone()
            .filter(|save| save.reason == SaveReason::SaveAs && save.destination_policy.is_none())
        {
            if doc.file_policy.pending.is_none() {
                resume.push(save);
            }
            continue;
        }
        if path.is_none() {
            continue;
        }
        if doc.file_policy.path != path {
            doc.file_policy.invalidated = true;
            if doc.file_policy.pending.as_ref().is_some_and(|request| {
                request.save.is_none() && Some(&request.path) != path.as_ref()
            }) {
                doc.file_policy.pending = None;
                doc.file_policy.generation = doc.file_policy.generation.wrapping_add(1);
            }
        }
        if doc.file_policy.invalidated && doc.file_policy.pending.is_none() {
            let Some(path) = path else {
                continue;
            };
            doc.file_policy.generation = doc.file_policy.generation.wrapping_add(1);
            let request = PolicyRequest {
                document_id: id,
                source_path: doc.file_path.clone(),
                path,
                generation: doc.file_policy.generation,
                save: None,
            };
            doc.file_policy.pending = Some(request.clone());
            commands.push(Cmd::ResolveFilePolicy(request));
        }
    }
    for save in resume {
        if let Some(command) = prepare_save(model, &save) {
            commands.push(command);
        } else {
            commands.extend(super::app::prepare_resolved_save(model, save));
        }
    }
    (!commands.is_empty()).then_some(Cmd::Batch(commands))
}

/// Return a policy request when preparation must wait. The save token remains
/// on its document, so a reply can resume it even after focus moves elsewhere.
pub(super) fn prepare_save(model: &mut AppModel, intent: &SaveIntent) -> Option<Cmd> {
    if !model.config.editorconfig {
        return None;
    }
    let doc = model.editor_area.documents.get_mut(&intent.document_id)?;
    if intent.reason == SaveReason::SaveAs {
        doc.file_policy.enabled = true;
        if intent.destination_policy.is_some() {
            return None;
        }
        if doc
            .file_policy
            .pending
            .as_ref()
            .and_then(|p| p.save.as_ref())
            .is_some_and(|s| s.is_current(doc))
        {
            return Some(Cmd::None);
        }
        doc.file_policy.generation = doc.file_policy.generation.wrapping_add(1);
        let request = PolicyRequest {
            document_id: intent.document_id,
            source_path: doc.file_path.clone(),
            path: intent.path.clone(),
            generation: doc.file_policy.generation,
            save: Some(intent.clone()),
        };
        doc.file_policy.pending = Some(request.clone());
        return Some(Cmd::ResolveFilePolicy(request));
    }
    if doc
        .file_policy
        .pending
        .as_ref()
        .and_then(|request| request.save.as_ref())
        .is_some_and(|save| !save.is_current(doc))
    {
        doc.file_policy.pending = None;
        doc.file_policy.generation = doc.file_policy.generation.wrapping_add(1);
    }
    if doc.file_policy.pending.is_some() {
        return Some(Cmd::None);
    }
    if doc.file_policy.invalidated {
        return reconcile(model).or(Some(Cmd::None));
    }
    None
}

pub(super) fn resolved(
    model: &mut AppModel,
    request: PolicyRequest,
    result: Result<ResolvedFilePolicy, String>,
) -> Option<Cmd> {
    let doc = model.editor_area.documents.get_mut(&request.document_id)?;
    if !model.config.editorconfig
        || doc.file_path != request.source_path
        || !doc
            .file_policy
            .pending
            .as_ref()
            .is_some_and(|p| p.generation == request.generation)
    {
        return None;
    }
    doc.file_policy.pending = None;
    let policy = match result {
        Ok(policy) => policy,
        Err(error) => ResolvedFilePolicy {
            path: request.path.clone(),
            diagnostics: vec![error],
            incomplete: true,
            ..Default::default()
        },
    };
    if let Some(diagnostic) = policy.diagnostics.first() {
        model.ui.set_status(format!("EditorConfig: {diagnostic}"));
    }
    if let Some(mut save) = request.save {
        if !save.is_current(doc) {
            return None;
        }
        if policy.incomplete {
            doc.pending_save = None;
            doc.save_error = Some((
                doc.revision,
                "Could not resolve destination EditorConfig; save cancelled".into(),
            ));
            return Some(Cmd::Redraw);
        }
        save.settings = policy.settings(model.config.text);
        save.resolution_generation = request.generation;
        save.destination_policy = Some(std::sync::Arc::new(policy));
        doc.pending_save = Some(save.clone());
        return super::app::prepare_resolved_save(model, save);
    }
    doc.file_policy.install(request.path, policy);
    doc.file_text_preferences = doc.file_policy.resolved.as_ref()?.preferences;
    doc.resolve_text_settings(model.config.text);
    let save = doc
        .pending_save
        .clone()
        .filter(|intent| intent.reason != SaveReason::SaveAs);
    let effects = save.and_then(|mut intent| {
        let doc = model.editor_area.documents.get_mut(&intent.document_id)?;
        intent.settings = doc.text_settings;
        intent.resolution_generation = request.generation;
        intent.text_policy_generation = doc.text_policy_generation;
        doc.pending_save = Some(intent.clone());
        super::app::prepare_resolved_save(model, intent)
    });
    super::merge_cmds(Some(Cmd::Redraw), effects)
}

pub(super) fn saving_is_blocked(model: &AppModel, id: DocumentId) -> bool {
    model.config.editorconfig
        && model
            .editor_area
            .documents
            .get(&id)
            .is_some_and(|doc| doc.file_policy.invalidated || doc.file_policy.pending.is_some())
}

/// A copyable snapshot in an ordinary untitled tab; no config files are changed.
pub(super) fn show_details(model: &mut AppModel) -> Option<Cmd> {
    if !model.editor().is_plain_text_mode() {
        return None;
    }
    let document = model.document();
    let settings = document.text_settings;
    let name = document.display_name();
    let indent = match settings.indent_style {
        crate::model::IndentStyle::Tab => "tabs",
        crate::model::IndentStyle::Space => "spaces",
    };
    let ending = match document.line_ending() {
        crate::model::LineEnding::Lf => "LF",
        crate::model::LineEnding::Crlf => "CRLF",
        crate::model::LineEnding::Cr => "CR",
    };
    let trim = if settings.trim_trailing_whitespace == Some(true) {
        "remove"
    } else {
        "preserve"
    };
    let final_newline = match settings.insert_final_newline {
        Some(true) => "ensure",
        Some(false) => "remove",
        None => "preserve",
    };
    let mut text = format!("Text settings for {name}\n\nIndent using: {indent}\nIndent step: {} columns\nTab width: {} columns\nLine ending: {ending}\nTrailing whitespace: {trim}\nFinal newline: {final_newline}\nEditorConfig enabled: {}\n\n", settings.indent_size, settings.tabs.width(), model.config.editorconfig);
    if let Some(policy) = &document.file_policy.resolved {
        text.push_str(&format!(
            "Rules resolved for: {}\n\n",
            policy.path.display()
        ));
        for property in &policy.properties {
            text.push_str(&format!("{} = {}", property.name, property.value));
            if let Some((source, line)) = &property.source {
                text.push_str(&format!("\n  {}:{line}", source.display()));
            }
            text.push('\n');
        }
        if policy.properties.is_empty() {
            text.push_str("No matching project rules. User defaults apply.\n");
        }
        if !policy.diagnostics.is_empty() {
            text.push_str("\nDiagnostics\n");
        }
        for diagnostic in &policy.diagnostics {
            text.push_str(diagnostic);
            text.push('\n');
        }
        text.push_str("\nConsulted config locations (including missing files)\n");
        for path in &policy.dependencies {
            text.push_str(&format!("{}\n", path.display()));
        }
    } else {
        text.push_str("User defaults apply. No project rules are loaded.\n");
    }
    text.push_str("\nThis is a snapshot. Change defaults in Settings → Editor or edit the source .editorconfig.\n");
    let command = super::layout::update_layout(model, crate::messages::LayoutMsg::NewTab);
    let doc = model.document_mut();
    let id = doc.id;
    let mut report = crate::model::Document::with_text(&text);
    report.id = id;
    report.untitled_name = Some(format!("Text settings · {name}"));
    *doc = report;
    model.resync_viewports();
    command
}
