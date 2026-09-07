//! Bounded, protocol-neutral workspace-symbol rows shared by runtime and palette.

use std::path::PathBuf;

use lsp_types::{Location, OneOf, SymbolKind, WorkspaceSymbol};

use super::LspServerId;

pub const MAX_SYMBOLS: usize = 2_000;
pub const MAX_QUERY_CHARS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolProvider {
    pub server_id: LspServerId,
    pub root: PathBuf,
    pub generation: u64,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SymbolResponseError {
    #[error("Language server rejected symbol search")]
    Rejected,
    #[error("Invalid workspace symbol response")]
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSearchRequest {
    pub id: u64,
    pub query: String,
    pub workspace: PathBuf,
    pub providers: Vec<SymbolProvider>,
}

#[derive(Debug, Clone)]
pub struct SymbolItem {
    pub name: String,
    pub detail: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub provider: SymbolProvider,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolResults {
    pub items: Vec<SymbolItem>,
    pub truncated: bool,
    pub failures: usize,
}

/// Both SymbolInformation and complete WorkspaceSymbol objects share these
/// fields. We do not advertise resolveSupport, so URI-only locations are not
/// usable. Unknown URI schemes and malformed individual rows never navigate.
pub(crate) fn parse_response(
    message: &serde_json::Value,
    provider: SymbolProvider,
) -> Result<SymbolResults, SymbolResponseError> {
    if message.get("error").is_some() {
        return Err(SymbolResponseError::Rejected);
    }
    let mut results = SymbolResults::default();
    let value = message.get("result").ok_or(SymbolResponseError::Invalid)?;
    if value.is_null() {
        return Ok(results);
    }
    let values = value.as_array().ok_or(SymbolResponseError::Invalid)?;
    results.truncated = values.len() > MAX_SYMBOLS;
    for value in values.iter().take(MAX_SYMBOLS) {
        let Ok(symbol) = serde_json::from_value::<WorkspaceSymbol>(value.clone()) else {
            results.failures += 1;
            continue;
        };
        let OneOf::Left(location) = symbol.location else {
            results.failures += 1;
            continue;
        };
        if location.uri.as_str().len() > crate::util::ByteSize::kibibytes(8).as_usize() {
            results.truncated = true;
            continue;
        }
        let Some(path) = super::uri_to_path(&location.uri) else {
            continue;
        };
        if location.range.start > location.range.end || symbol.name.is_empty() {
            continue;
        }
        let clean = |text: &str| {
            text.chars()
                .filter(|ch| !ch.is_control())
                .take(256)
                .collect::<String>()
        };
        let name = clean(&symbol.name);
        if name.is_empty() {
            continue;
        }
        let path = path.strip_prefix(&provider.root).unwrap_or(&path);
        let mut detail = clean(&path.to_string_lossy());
        if let Some(container) = symbol.container_name {
            detail = format!("{} · {detail}", clean(&container));
        }
        results.items.push(SymbolItem {
            name,
            detail,
            kind: symbol.kind,
            location,
            provider: provider.clone(),
        });
    }
    Ok(results)
}

/// Arrival order must not affect ranking or duplicate selection across servers.
pub fn finish_results(query: &str, results: &mut SymbolResults) {
    use nucleo_matcher::{Config, Matcher, Utf32Str};
    let mut matcher = Matcher::new(Config::DEFAULT);
    let query = query.to_lowercase();
    let mut needle_buf = Vec::new();
    let needle = Utf32Str::new(&query, &mut needle_buf);
    let mut scored: Vec<_> = results
        .items
        .drain(..)
        .map(|item| {
            let lower = item.name.to_lowercase();
            let mut buf = Vec::new();
            let score = if query.is_empty() {
                0
            } else {
                matcher
                    .fuzzy_match(Utf32Str::new(&lower, &mut buf), needle)
                    .unwrap_or(0)
            };
            (score, item)
        })
        .collect();
    scored.sort_by(|(a_score, a), (b_score, b)| {
        b_score
            .cmp(a_score)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.location.uri.as_str().cmp(b.location.uri.as_str()))
            .then_with(|| a.location.range.start.cmp(&b.location.range.start))
            .then_with(|| a.location.range.end.cmp(&b.location.range.end))
            .then_with(|| a.provider.server_id.0.cmp(&b.provider.server_id.0))
            .then_with(|| a.provider.root.cmp(&b.provider.root))
    });
    scored.dedup_by(|(_, a), (_, b)| a.name == b.name && a.location == b.location);
    results.truncated |= scored.len() > MAX_SYMBOLS;
    results.items = scored
        .into_iter()
        .take(MAX_SYMBOLS)
        .map(|(_, item)| item)
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(id: &str) -> SymbolProvider {
        SymbolProvider {
            server_id: id.into(),
            root: "/workspace".into(),
            generation: 1,
        }
    }

    fn row(name: &str) -> serde_json::Value {
        json!({"name": name, "kind": 12, "containerName": "module",
            "location": {"uri": "file:///workspace/caf%C3%A9.rs",
                "range": {"start": {"line": 2, "character": 4},
                    "end": {"line": 2, "character": 8}}}})
    }

    #[test]
    fn workspace_symbols_accept_both_complete_wire_formats_and_unicode() {
        let mut legacy = row("café");
        legacy["deprecated"] = json!(false);
        let mut modern = row("méthode");
        modern["data"] = json!({"opaque": 42});
        let results =
            parse_response(&json!({"result": [legacy, modern]}), provider("rust")).unwrap();
        assert_eq!(results.items.len(), 2);
        assert_eq!(results.items[0].detail, "module · café.rs");
        assert_eq!(results.items[0].location.range.start.character, 4);
        assert_eq!(results.failures, 0);
    }

    #[test]
    fn workspace_symbols_reject_invalid_responses_and_skip_unusable_rows() {
        assert!(matches!(
            parse_response(&json!({}), provider("a")),
            Err(SymbolResponseError::Invalid)
        ));
        assert!(matches!(
            parse_response(&json!({"result": {}}), provider("a")),
            Err(SymbolResponseError::Invalid)
        ));
        assert!(matches!(
            parse_response(&json!({"error": {"code": -1}}), provider("a")),
            Err(SymbolResponseError::Rejected)
        ));
        assert!(parse_response(&json!({"result": null}), provider("a"))
            .unwrap()
            .items
            .is_empty());
        let mut unresolved = row("unresolved");
        unresolved["location"]
            .as_object_mut()
            .unwrap()
            .remove("range");
        let mut remote = row("remote");
        remote["location"]["uri"] = json!("https://example.invalid/file.rs");
        let mut reversed = row("reversed");
        reversed["location"]["range"]["end"]["line"] = json!(0);
        let results = parse_response(
            &json!({"result": [unresolved, remote, reversed, {}, row("good")]}),
            provider("a"),
        )
        .unwrap();
        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].name, "good");
        assert_eq!(results.failures, 2);
    }

    #[test]
    fn workspace_symbols_bound_rows_and_sanitize_display_only() {
        let results = parse_response(
            &json!({"result": vec![row(&format!("\n{}", "é".repeat(300))); MAX_SYMBOLS + 1]}),
            provider("a"),
        )
        .unwrap();
        assert_eq!(results.items.len(), MAX_SYMBOLS);
        assert!(results.truncated);
        assert_eq!(results.items[0].name.chars().count(), 256);
        assert!(!results.items[0].name.contains('\n'));
        assert_eq!(
            results.items[0].location.uri.as_str(),
            "file:///workspace/caf%C3%A9.rs"
        );
    }

    #[test]
    fn workspace_symbols_rank_and_deduplicate_independently_of_server_arrival() {
        let rows = json!({"result": [row("zebra"), row("alpha"), row("alternative")]});
        let mut first = parse_response(&rows, provider("b")).unwrap();
        first
            .items
            .extend(parse_response(&rows, provider("a")).unwrap().items);
        let mut reversed = first.clone();
        reversed.items.reverse();
        finish_results("alpha", &mut first);
        finish_results("alpha", &mut reversed);
        assert_eq!(first.items.len(), 3);
        assert_eq!(first.items[0].name, "alpha");
        let keys = |results: SymbolResults| {
            results
                .items
                .into_iter()
                .map(|item| (item.name, item.provider.server_id))
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(first), keys(reversed));
    }

    #[test]
    fn workspace_symbols_capability_respects_explicit_false() {
        use crate::lsp::client::supports_workspace_symbols;
        let mut caps = lsp_types::ServerCapabilities::default();
        assert!(!supports_workspace_symbols(&caps));
        caps.workspace_symbol_provider = Some(OneOf::Left(false));
        assert!(!supports_workspace_symbols(&caps));
        caps.workspace_symbol_provider = Some(OneOf::Left(true));
        assert!(supports_workspace_symbols(&caps));
        caps.workspace_symbol_provider = Some(OneOf::Right(lsp_types::WorkspaceSymbolOptions {
            work_done_progress_options: Default::default(),
            resolve_provider: None,
        }));
        assert!(supports_workspace_symbols(&caps));
    }
}
