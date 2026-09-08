//! Opt-in extra source context. Selection and idle scheduling live in runtime;
//! this module owns the provider-neutral payload and its serialization policy.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::inline::InlineRequest;
use super::provider::ProviderError;
use crate::util::ByteSize;

pub const MAX_CHUNKS: usize = 32;
pub const CHUNK_BYTES: ByteSize = ByteSize::kibibytes(8);
pub const FILENAME_BYTES: ByteSize = ByteSize::kibibytes(1);

/// Extra context is disabled unless explicitly enabled for this provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum ContextStrategy {
    #[default]
    None,
    RecencyRing {
        #[serde(default = "default_max_chunks")]
        max_chunks: usize,
        #[serde(default = "default_chunk_lines")]
        chunk_lines: usize,
    },
}

fn default_max_chunks() -> usize {
    8
}

fn default_chunk_lines() -> usize {
    64
}

impl ContextStrategy {
    /// Validated bounds, shared by runtime collection and provider validation.
    pub fn limits(self) -> Result<Option<(usize, usize)>, ProviderError> {
        match self {
            Self::None => Ok(None),
            Self::RecencyRing {
                max_chunks,
                chunk_lines,
            } if (1..=MAX_CHUNKS).contains(&max_chunks) && (1..=256).contains(&chunk_lines) => {
                Ok(Some((max_chunks, chunk_lines)))
            }
            Self::RecencyRing { .. } => Err(ProviderError::Configuration(
                "recency_ring requires max_chunks: 1..32 and chunk_lines: 1..256",
            )),
        }
    }
}

/// One bounded, ordered snippet. No absolute paths or source text in Debug.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ContextChunk {
    pub filename: String,
    pub text: String,
}

impl std::fmt::Debug for ContextChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextChunk")
            .field("filename_bytes", &self.filename.len())
            .field("text_bytes", &self.text.len())
            .finish()
    }
}

pub(super) fn validate(
    chunks: &[ContextChunk],
    strategy: ContextStrategy,
) -> Result<(), ProviderError> {
    let limit = strategy.limits()?.map_or(0, |(count, _)| count);
    if chunks.len() > limit
        || chunks.iter().any(|chunk| {
            chunk.text.len() > CHUNK_BYTES.as_usize()
                || chunk.filename.len() > FILENAME_BYTES.as_usize()
                || chunk
                    .filename
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '<' | '>'))
        })
    {
        return Err(ProviderError::Configuration(
            "extra context exceeds configured bounds or has an invalid filename",
        ));
    }
    Ok(())
}

/// Comment each physical line, including CR-only boundaries. Unsupported
/// languages fail explicitly instead of silently inserting non-comment code.
pub(super) fn commented_prefix(request: &InlineRequest) -> Result<Cow<'_, str>, ProviderError> {
    if request.extra_context.is_empty() {
        return Ok(Cow::Borrowed(&request.prefix));
    }
    let comment = match request.language.as_deref() {
        Some("rust" | "go" | "javascript" | "typescript" | "tsx" | "jsx" | "c" | "cpp" | "java" | "csharp" | "swift" | "kotlin" | "dart" | "solidity" | "gleam") => "//",
        Some("python" | "ruby" | "bash" | "toml" | "yaml" | "r" | "julia" | "elixir" | "just" | "dockerfile" | "make") => "#",
        Some("lua" | "sql" | "haskell" | "applescript") => "--",
        Some("scheme" | "fennel" | "clojure" | "sema" | "janet" | "ini") => ";",
        _ => return Err(ProviderError::Configuration("extra context comment fallback is unsupported for this language; use llama_cpp or context strategy none")),
    };
    let mut prefix = String::new();
    for chunk in &request.extra_context {
        prefix.push_str(comment);
        prefix.push_str(" Path: ");
        prefix.push_str(&chunk.filename);
        prefix.push('\n');
        for line in chunk.text.split(['\r', '\n', '\u{2028}', '\u{2029}']) {
            prefix.push_str(comment);
            prefix.push(' ');
            prefix.push_str(line);
            prefix.push('\n');
        }
        // A C-family line ending in backslash continues onto the following
        // physical line. An empty separator keeps the active prefix outside it.
        prefix.push('\n');
    }
    prefix.push_str(&request.prefix);
    Ok(Cow::Owned(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recency_config_is_opt_in_defaulted_and_bounded() {
        let provider: crate::config::ProviderConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(provider.context, ContextStrategy::None);
        let provider: crate::config::ProviderConfig =
            serde_yaml::from_str("context: { strategy: recency_ring }").unwrap();
        assert_eq!(provider.context.limits().unwrap(), Some((8, 64)));
        let encoded = serde_yaml::to_string(&provider).unwrap();
        assert_eq!(
            serde_yaml::from_str::<crate::config::ProviderConfig>(&encoded).unwrap(),
            provider
        );
        for (max_chunks, chunk_lines) in [
            (0, 64),
            (33, 64),
            (8, 0),
            (8, 257),
            (usize::MAX, usize::MAX),
        ] {
            assert!(ContextStrategy::RecencyRing {
                max_chunks,
                chunk_lines
            }
            .limits()
            .is_err());
        }
    }

    #[test]
    fn recency_comment_fallback_covers_physical_lines_without_changing_active_text() {
        let mut document = crate::model::Document::with_text("α\r\n    ");
        document.id = Some(crate::model::DocumentId(1));
        let mut request =
            super::super::inline::build_request(&document, (1, 4), 1, Some("rust".into()), false)
                .unwrap();
        request.extra_context.push(ContextChunk {
            filename: "helper.rs".into(),
            text: "one\rtwo\r\nthree\n".into(),
        });
        let result = commented_prefix(&request).unwrap();
        assert_eq!(
            result,
            "// Path: helper.rs\n// one\n// two\n// \n// three\n// \n\nα\r\n    "
        );
        request.extra_context[0].text = "one\u{2028}two\\".into();
        assert_eq!(
            commented_prefix(&request).unwrap(),
            "// Path: helper.rs\n// one\n// two\\\n\nα\r\n    "
        );
        request.language = Some("python".into());
        assert!(commented_prefix(&request).unwrap().starts_with("# Path:"));
        request.language = Some("json".into());
        assert!(
            commented_prefix(&request).is_err(),
            "JSON has no comment syntax"
        );
        request.extra_context.clear();
        assert_eq!(commented_prefix(&request).unwrap(), request.prefix);
    }
}
