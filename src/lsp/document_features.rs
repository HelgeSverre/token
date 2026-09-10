//! Revision-bound semantic colors, parameter annotations and code-lens actions.

use std::{borrow::Cow, collections::BTreeMap};

use lsp_types::{CodeLens, InlayHint, InlayHintLabel, SemanticTokensOptions, ServerCapabilities};

use crate::{
    model::{CodeActionItem, Document},
    syntax::{highlight_id_for_name, HighlightToken, SyntaxHighlights},
    util::ByteSize,
};

pub const MAX_DOCUMENT_BYTES: ByteSize = ByteSize::mebibytes(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    SemanticTokens,
    InlayHints,
    CodeLens,
}

impl Feature {
    pub const ALL: [Self; 3] = [Self::SemanticTokens, Self::InlayHints, Self::CodeLens];
    pub fn method(self) -> &'static str {
        match self {
            Self::SemanticTokens => "textDocument/semanticTokens/full",
            Self::InlayHints => "textDocument/inlayHint",
            Self::CodeLens => "textDocument/codeLens",
        }
    }
    pub fn supports(self, caps: &ServerCapabilities) -> bool {
        match self {
            Self::SemanticTokens => semantic_options(caps).is_some_and(|options| {
                !matches!(
                    options.full,
                    None | Some(lsp_types::SemanticTokensFullOptions::Bool(false))
                )
            }),
            Self::InlayHints => matches!(
                caps.inlay_hint_provider,
                Some(lsp_types::OneOf::Left(true) | lsp_types::OneOf::Right(_))
            ),
            Self::CodeLens => caps.code_lens_provider.is_some(),
        }
    }
}

pub fn semantic_options(caps: &ServerCapabilities) -> Option<&SemanticTokensOptions> {
    match caps.semantic_tokens_provider.as_ref()? {
        lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
            Some(options)
        }
        lsp_types::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
            Some(&options.semantic_tokens_options)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DocumentFeatures {
    pub revision: u64,
    pub semantic: Option<SyntaxHighlights>,
    pub hints: BTreeMap<usize, Vec<String>>,
    pub lenses: Vec<CodeLens>,
    pub eval_output: BTreeMap<usize, String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvalOutput {
    pub uri: lsp_types::Uri,
    pub range: lsp_types::Range,
    pub ok: bool,
    pub value: Option<String>,
    pub error: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

impl EvalOutput {
    pub fn summary(&self) -> String {
        let text = if self.ok {
            self.value
                .as_deref()
                .filter(|text| !text.is_empty())
                .unwrap_or(&self.stdout)
        } else {
            self.error.as_deref().unwrap_or(&self.stderr)
        };
        let prefix = if self.ok { "Sema: " } else { "Sema error: " };
        format!(
            "{prefix}{}",
            text.chars()
                .map(|ch| if ch.is_control() { ' ' } else { ch })
                .take(500)
                .collect::<String>()
        )
    }
}

impl DocumentFeatures {
    pub fn lens_actions(&self, document: &Document, line: usize) -> Vec<CodeActionItem> {
        if self.revision != document.revision {
            return Vec::new();
        }
        self.lenses
            .iter()
            .filter(|lens| {
                lens.range.start.line as usize <= line && line <= lens.range.end.line as usize
            })
            .filter_map(|lens| lens.command.clone())
            .map(|command| CodeActionItem {
                title: command.title.clone(),
                kind: None,
                is_preferred: false,
                edit: None,
                command: Some(command),
            })
            .collect()
    }

    /// Invalid server data is ignored as a whole, retaining lexical highlighting.
    pub fn apply(
        &mut self,
        document: &Document,
        feature: Feature,
        result: serde_json::Value,
        caps: &ServerCapabilities,
    ) {
        if self.revision != document.revision {
            *self = Self {
                revision: document.revision,
                ..Self::default()
            };
        }
        match feature {
            Feature::SemanticTokens => self.semantic = decode_semantic(document, &result, caps),
            Feature::InlayHints => {
                self.hints.clear();
                let Some(items) = result.as_array().filter(|items| items.len() <= 10_000) else {
                    return;
                };
                let mut hints: Vec<_> = items
                    .iter()
                    .filter_map(|item| serde_json::from_value::<InlayHint>(item.clone()).ok())
                    .collect();
                hints.sort_by_key(|hint| hint.position);
                for hint in hints {
                    if exact_column(document, hint.position).is_none() {
                        continue;
                    }
                    let label = match hint.label {
                        InlayHintLabel::String(text) => text,
                        InlayHintLabel::LabelParts(parts) => {
                            parts.into_iter().map(|part| part.value).collect()
                        }
                    };
                    let label: String = label
                        .chars()
                        .filter(|ch| !ch.is_control())
                        .take(160)
                        .collect();
                    let line = self.hints.entry(hint.position.line as usize).or_default();
                    if !label.is_empty() && line.len() < 16 {
                        line.push(label);
                    }
                }
            }
            Feature::CodeLens => {
                self.lenses = result
                    .as_array()
                    .filter(|items| items.len() <= 10_000)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| serde_json::from_value::<CodeLens>(item.clone()).ok())
                    .filter(|lens| lens.command.is_some() && valid_range(document, lens.range))
                    .collect();
            }
        }
    }
}

pub fn valid_range(document: &Document, range: lsp_types::Range) -> bool {
    range.start <= range.end
        && [range.start, range.end]
            .into_iter()
            .all(|position| exact_column(document, position).is_some())
}

/// Use Ropey's indexed conversions so a token batch does not repeatedly scan
/// long line prefixes. Reject line endings and partial surrogate pairs.
fn exact_column(document: &Document, position: lsp_types::Position) -> Option<usize> {
    let line = document.get_line_slice(position.line as usize)?;
    let column = line
        .try_utf16_cu_to_char(position.character as usize)
        .ok()?;
    (column <= document.line_length(position.line as usize)
        && line.char_to_utf16_cu(column) == position.character as usize)
        .then_some(column)
}

fn decode_semantic(
    document: &Document,
    result: &serde_json::Value,
    caps: &ServerCapabilities,
) -> Option<SyntaxHighlights> {
    let data = result.get("data")?.as_array()?;
    if data.len() > 250_000 || data.len() % 5 != 0 {
        return None;
    }
    let legend = &semantic_options(caps)?.legend;
    let mut highlights = SyntaxHighlights::new(document.language, document.revision);
    let (mut line, mut column) = (0u32, 0u32);
    let mut previous_end = lsp_types::Position::new(0, 0);
    for encoded in data.as_chunks::<5>().0 {
        let mut fields = [0u32; 5];
        for (field, value) in fields.iter_mut().zip(encoded) {
            *field = u32::try_from(value.as_u64()?).ok()?;
        }
        line = line.checked_add(fields[0])?;
        column = if fields[0] == 0 {
            column.checked_add(fields[1])?
        } else {
            fields[1]
        };
        let start = lsp_types::Position::new(line, column);
        let end = lsp_types::Position::new(line, column.checked_add(fields[2])?);
        if fields[2] == 0 || start < previous_end {
            return None;
        }
        previous_end = end;
        let start_col = exact_column(document, start)?;
        let end_col = exact_column(document, end)?;
        let kind = legend.token_types.get(fields[3] as usize)?.as_str();
        let builtin = legend
            .token_modifiers
            .iter()
            .position(|modifier| modifier.as_str() == "defaultLibrary")
            .is_some_and(|bit| bit < 32 && fields[4] & (1u32 << bit) != 0);
        let capture = match (kind, builtin) {
            ("function" | "method", true) => "function.builtin",
            ("variable", true) => "variable.builtin",
            ("parameter", _) => "variable.parameter",
            ("macro", _) => "function",
            ("class" | "struct" | "interface" | "enum" | "typeParameter", _) => "type",
            (other, _) => other,
        };
        if let Some(highlight) = highlight_id_for_name(capture) {
            highlights
                .lines
                .entry(line as usize)
                .or_default()
                .tokens
                .push(HighlightToken {
                    start_col,
                    end_col,
                    highlight,
                });
        }
    }
    Some(highlights)
}

pub fn overlay_tokens<'a>(
    lexical: &'a [HighlightToken],
    semantic: &[HighlightToken],
) -> Cow<'a, [HighlightToken]> {
    if semantic.is_empty() {
        return Cow::Borrowed(lexical);
    }
    let mut tokens = Vec::with_capacity(lexical.len() + semantic.len());
    let mut first = 0;
    for token in lexical {
        while first < semantic.len() && semantic[first].end_col <= token.start_col {
            first += 1;
        }
        let mut start = token.start_col;
        for overlay in semantic[first..]
            .iter()
            .take_while(|overlay| overlay.start_col < token.end_col)
        {
            if start < overlay.start_col {
                tokens.push(HighlightToken {
                    start_col: start,
                    end_col: overlay.start_col,
                    highlight: token.highlight,
                });
            }
            start = start.max(overlay.end_col);
        }
        if start < token.end_col {
            tokens.push(HighlightToken {
                start_col: start,
                ..token.clone()
            });
        }
    }
    tokens.extend_from_slice(semantic);
    tokens.sort_by_key(|token| token.start_col);
    Cow::Owned(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn caps() -> ServerCapabilities {
        serde_json::from_value(serde_json::json!({"semanticTokensProvider":{"legend":{"tokenTypes":["variable","function","parameter"],"tokenModifiers":["defaultLibrary"]},"full":true}})).unwrap()
    }
    #[test]
    fn sema_semantic_tokens_convert_utf16_and_builtin_modifiers() {
        let doc = Document::with_text("\"🙂\" map π");
        let highlights = decode_semantic(
            &doc,
            &serde_json::json!({"data":[0,5,3,1,1,0,4,1,2,0]}),
            &caps(),
        )
        .unwrap();
        assert_eq!(
            highlights.get_line_tokens(0),
            [
                HighlightToken {
                    start_col: 4,
                    end_col: 7,
                    highlight: highlight_id_for_name("function.builtin").unwrap()
                },
                HighlightToken {
                    start_col: 8,
                    end_col: 9,
                    highlight: highlight_id_for_name("variable.parameter").unwrap()
                }
            ]
        );
        for data in [
            serde_json::json!([0, 2, 1, 0, 0]),
            serde_json::json!([0, 5, 100, 1, 0]),
            serde_json::json!([0, 5, 1, 99, 0]),
            serde_json::json!([0, 5, 1]),
        ] {
            assert!(decode_semantic(&doc, &serde_json::json!({"data":data}), &caps()).is_none());
        }
    }
    #[test]
    fn semantic_colors_override_without_losing_lexical_punctuation() {
        let lexical = [HighlightToken {
            start_col: 0,
            end_col: 10,
            highlight: 1,
        }];
        let semantic = [HighlightToken {
            start_col: 2,
            end_col: 5,
            highlight: 2,
        }];
        let result = overlay_tokens(&lexical, &semantic);
        assert_eq!(
            result
                .iter()
                .map(|t| (t.start_col, t.end_col, t.highlight))
                .collect::<Vec<_>>(),
            [(0, 2, 1), (2, 5, 2), (5, 10, 1)]
        );
    }
    #[test]
    fn sema_annotations_reject_stale_lenses_and_invalid_positions() {
        let mut doc = Document::with_text("(f 42)\n");
        let mut features = DocumentFeatures::default();
        features.apply(&doc, Feature::InlayHints, serde_json::json!([{"position":{"line":0,"character":3},"label":"x:"},{"position":{"line":50,"character":0},"label":"bad"}]), &caps());
        assert_eq!(features.hints.len(), 1);
        features.apply(&doc, Feature::CodeLens, serde_json::json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":6}},"command":{"title":"Run","command":"sema.runTopLevel"}}]), &caps());
        assert_eq!(features.lens_actions(&doc, 0).len(), 1);
        doc.revision += 1;
        assert!(features.lens_actions(&doc, 0).is_empty());
    }

    #[test]
    fn sema_annotations_sort_hints_and_validate_unicode_ranges() {
        let doc = Document::with_text("\"🙂\" (f 1 2)\r\n");
        let mut features = DocumentFeatures::default();
        features.apply(
            &doc,
            Feature::InlayHints,
            serde_json::json!([
                {"position":{"line":0,"character":10},"label":"second:"},
                {"position":{"line":0,"character":8},"label":[{"value":"first:"}]},
                {"position":{"line":0,"character":2},"label":"partial surrogate"}
            ]),
            &caps(),
        );
        assert_eq!(features.hints[&0], ["first:", "second:"]);
        for (line, character) in [(0, 2), (0, 14), (20, 0)] {
            let position = lsp_types::Position::new(line, character);
            assert!(!valid_range(
                &doc,
                lsp_types::Range::new(position, position)
            ));
        }
        assert!(!Feature::SemanticTokens.supports(&ServerCapabilities::default()));
        assert!(!Feature::InlayHints.supports(&ServerCapabilities::default()));
        assert!(!Feature::CodeLens.supports(&ServerCapabilities::default()));
    }

    #[test]
    fn sema_semantic_positions_cross_rope_chunks_without_losing_columns() {
        let text = format!("{}🙂 name", "π ".repeat(2000));
        let doc = Document::with_text(&text);
        let highlights =
            decode_semantic(&doc, &serde_json::json!({"data":[0,4003,4,0,0]}), &caps()).unwrap();
        assert_eq!(highlights.get_line_tokens(0)[0].start_col, 4002);
        assert_eq!(highlights.get_line_tokens(0)[0].end_col, 4006);
    }
}
