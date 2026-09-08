//! Raw FIM serialization, independent of HTTP transport. Model-card references
//! and tokenizer assumptions are recorded in the completion implementation audit.

use serde::{Deserialize, Serialize};

use super::inline::InlineRequest;
use super::provider::ProviderError;

/// Native prompting preserves the transport's prefix/suffix API. Other choices
/// explicitly opt into raw FIM; Infer recognizes names, not model capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptFormat {
    #[default]
    Native,
    Infer,
    Qwen,
    StarCoder,
    CodeLlama,
    DeepSeek,
    Codestral,
    Mellum,
}

#[derive(Clone, Copy)]
pub(super) struct PromptTemplate {
    markers: [&'static str; 3],
    suffix_first: bool,
    filename: bool,
    pub stop: &'static [&'static str],
}

const FILENAME: &str = "<filename>";
const TEMPLATES: &[(PromptFormat, PromptTemplate)] = &[
    (
        PromptFormat::Qwen,
        PromptTemplate {
            markers: ["<|fim_prefix|>", "<|fim_suffix|>", "<|fim_middle|>"],
            suffix_first: false,
            filename: false,
            stop: &[
                "<|endoftext|>",
                "<|fim_prefix|>",
                "<|fim_suffix|>",
                "<|fim_middle|>",
            ],
        },
    ),
    (
        PromptFormat::StarCoder,
        PromptTemplate {
            markers: ["<fim_prefix>", "<fim_suffix>", "<fim_middle>"],
            suffix_first: false,
            filename: false,
            stop: &[
                "<|endoftext|>",
                "<fim_prefix>",
                "<fim_suffix>",
                "<fim_middle>",
            ],
        },
    ),
    (
        PromptFormat::CodeLlama,
        PromptTemplate {
            // SentencePiece markers and ordinary prefix encoding need their
            // word-boundary spaces. BOS is supplied by the serving tokenizer.
            markers: ["<PRE> ", " <SUF>", " <MID>"],
            suffix_first: false,
            filename: false,
            stop: &["<EOT>", "</s>", "<PRE>", "<SUF>"],
        },
    ),
    (
        PromptFormat::DeepSeek,
        PromptTemplate {
            markers: ["<｜fim▁begin｜>", "<｜fim▁hole｜>", "<｜fim▁end｜>"],
            suffix_first: false,
            filename: false,
            stop: &[
                "<｜end▁of▁sentence｜>",
                "<｜fim▁begin｜>",
                "<｜fim▁hole｜>",
                "<｜fim▁end｜>",
            ],
        },
    ),
    (
        PromptFormat::Codestral,
        PromptTemplate {
            markers: ["[SUFFIX]", "[PREFIX]", ""],
            suffix_first: true,
            filename: false,
            stop: &["</s>", "[SUFFIX]", "[PREFIX]", "[MIDDLE]"],
        },
    ),
    (
        PromptFormat::Mellum,
        PromptTemplate {
            markers: ["<fim_suffix>", "<fim_prefix>", "<fim_middle>"],
            suffix_first: true,
            filename: true,
            stop: &["<|endoftext|>", "<fim_suffix>", "<fim_prefix>", FILENAME],
        },
    ),
];

impl PromptFormat {
    pub(super) fn resolve(
        self,
        model: Option<&str>,
    ) -> Result<Option<PromptTemplate>, ProviderError> {
        let format = match self {
            Self::Native => return Ok(None),
            Self::Infer => model.and_then(infer).ok_or(ProviderError::Configuration(
                "cannot infer a raw FIM format; select prompt_format explicitly for a supported FIM model",
            ))?,
            format => format,
        };
        TEMPLATES
            .iter()
            .find_map(|(kind, template)| (*kind == format).then_some(*template))
            .map(Some)
            .ok_or(ProviderError::Configuration("unsupported raw FIM format"))
    }
}

fn infer(model: &str) -> Option<PromptFormat> {
    let name = model.rsplit('/').next()?.to_ascii_lowercase();
    // Do not equate an arbitrary instruction/chat or Python-only derivative
    // with the base FIM model merely because its name contains a family name.
    if ["chat", "mamba"].iter().any(|tag| name.contains(tag))
        || (name.contains("instruct") && !name.starts_with("codellama"))
    {
        return None;
    }
    if name.starts_with("codellama")
        && (name.contains("python")
            || !name
                .split(['-', ':', '.'])
                .any(|part| matches!(part, "7b" | "13b")))
    {
        return None;
    }
    [
        ("qwen2.5-coder", PromptFormat::Qwen),
        ("starcoderbase", PromptFormat::StarCoder),
        ("starcoder2", PromptFormat::StarCoder),
        ("starcoder", PromptFormat::StarCoder),
        ("codellama", PromptFormat::CodeLlama),
        ("deepseek-coder", PromptFormat::DeepSeek),
        ("codestral", PromptFormat::Codestral),
        ("mellum", PromptFormat::Mellum),
    ]
    .into_iter()
    .find_map(|(family, format)| {
        name.strip_prefix(family)
            .filter(|tail| tail.is_empty() || tail.starts_with(['-', ':', '.']))
            .map(|_| format)
    })
}

impl PromptTemplate {
    pub(super) fn render(&self, request: &InlineRequest) -> Result<String, ProviderError> {
        let prefix = super::recency::commented_prefix(request)?;
        if self
            .markers
            .into_iter()
            .map(str::trim)
            .chain(self.stop.iter().copied())
            .filter(|token| !token.is_empty())
            .any(|token| prefix.contains(token) || request.suffix.contains(token))
            || (self.filename && (prefix.contains(FILENAME) || request.suffix.contains(FILENAME)))
        {
            return Err(ProviderError::Configuration(
                "source contains raw FIM control tokens; use native prompting",
            ));
        }
        let mut prompt = String::new();
        if self.filename {
            if let Some(name) = request
                .file_path
                .as_deref()
                .and_then(|p| p.file_name())
                .and_then(|name| name.to_str())
                .filter(|name| {
                    !name
                        .chars()
                        .any(|ch| ch.is_control() || matches!(ch, '<' | '>'))
                })
            {
                prompt.push_str(FILENAME);
                prompt.push_str(name);
                prompt.push('\n');
            }
        }
        let (first, second) = if self.suffix_first {
            (request.suffix.as_str(), prefix.as_ref())
        } else {
            (prefix.as_ref(), request.suffix.as_str())
        };
        prompt.push_str(self.markers[0]);
        prompt.push_str(first);
        prompt.push_str(self.markers[1]);
        prompt.push_str(second);
        prompt.push_str(self.markers[2]);
        Ok(prompt)
    }
}

/// Response cleanup and raw formatting share the vocabulary definitions.
pub(super) fn sentinels() -> impl Iterator<Item = &'static str> {
    TEMPLATES
        .iter()
        .flat_map(|(_, template)| {
            template
                .markers
                .into_iter()
                .flat_map(|marker| [marker, marker.trim()])
                .chain(template.stop.iter().copied())
        })
        .filter(|token| !token.is_empty())
        .chain([
            "<|file_sep|>",
            "<|repo_name|>",
            "<|fim_pad|>",
            "<fim_pad>",
            "<|im_start|>",
            "<|im_end|>",
            FILENAME,
        ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::inline::{postprocess, RequestSnapshot};
    use crate::model::DocumentId;

    fn request() -> InlineRequest {
        InlineRequest {
            snapshot: RequestSnapshot {
                document_id: DocumentId(1),
                revision: 1,
                line: 1,
                column: 2,
                request_id: 1,
            },
            prefix: "α\r\n  P ".into(),
            suffix: " S\nβ".into(),
            language: Some("rust".into()),
            file_path: Some("/private/project/probe.rs".into()),
            extra_context: Vec::new(),
            explicit: false,
        }
    }

    #[test]
    fn raw_fim_formats_have_byte_exact_golden_prompts() {
        for (format, expected) in [
            (
                PromptFormat::Qwen,
                "<|fim_prefix|>α\r\n  P <|fim_suffix|> S\nβ<|fim_middle|>",
            ),
            (
                PromptFormat::StarCoder,
                "<fim_prefix>α\r\n  P <fim_suffix> S\nβ<fim_middle>",
            ),
            (PromptFormat::CodeLlama, "<PRE> α\r\n  P  <SUF> S\nβ <MID>"),
            (
                PromptFormat::DeepSeek,
                "<｜fim▁begin｜>α\r\n  P <｜fim▁hole｜> S\nβ<｜fim▁end｜>",
            ),
            (PromptFormat::Codestral, "[SUFFIX] S\nβ[PREFIX]α\r\n  P "),
            (
                PromptFormat::Mellum,
                "<filename>probe.rs\n<fim_suffix> S\nβ<fim_prefix>α\r\n  P <fim_middle>",
            ),
        ] {
            let template = format.resolve(None).unwrap().unwrap();
            assert_eq!(
                template.render(&request()).unwrap().as_bytes(),
                expected.as_bytes(),
                "{format:?}"
            );
            assert!((1..=4).contains(&template.stop.len()));
            assert!(template.stop.iter().all(|stop| !stop.is_empty()));
        }
    }

    #[test]
    fn raw_fim_inference_is_family_bounded_and_explicit_formats_allow_aliases() {
        for (model, format) in [
            ("Qwen/Qwen2.5-Coder-7B", PromptFormat::Qwen),
            ("qwen2.5-coder:7b-base", PromptFormat::Qwen),
            ("bigcode/starcoder2-3b", PromptFormat::StarCoder),
            ("bigcode/starcoderbase", PromptFormat::StarCoder),
            ("CodeLlama-13b-hf.Q4_K_M.gguf", PromptFormat::CodeLlama),
            ("codellama:7b-code", PromptFormat::CodeLlama),
            ("CodeLlama-7b-Instruct-hf", PromptFormat::CodeLlama),
            (
                "deepseek-ai/DeepSeek-Coder-V2-Lite-Base",
                PromptFormat::DeepSeek,
            ),
            ("mistralai/Codestral-22B-v0.1", PromptFormat::Codestral),
            ("JetBrains/Mellum-4b-sft-python", PromptFormat::Mellum),
        ] {
            assert_eq!(infer(model), Some(format), "{model}");
            assert!(PromptFormat::Infer.resolve(Some(model)).unwrap().is_some());
        }
        for model in [
            "",
            "alias",
            "qwen2.5:7b",
            "qwen3",
            "not-qwen2.5-coder",
            "starcoderish",
            "Qwen2.5-Coder-7B-Instruct",
            "codellama:34b",
            "codellama:7b-python",
            "codestral-mamba",
        ] {
            assert!(PromptFormat::Infer.resolve(Some(model)).is_err(), "{model}");
            assert!(PromptFormat::Qwen.resolve(Some(model)).unwrap().is_some());
        }
        assert!(PromptFormat::Native.resolve(None).unwrap().is_none());
        assert!(PromptFormat::Infer.resolve(None).is_err());
    }

    #[test]
    fn raw_fim_filename_metadata_is_optional_and_never_a_full_path() {
        let template = PromptFormat::Mellum.resolve(None).unwrap().unwrap();
        let mut req = request();
        assert!(!template.render(&req).unwrap().contains("private"));
        for path in [
            None,
            Some("bad\nname.rs".into()),
            Some("<fim_prefix>.rs".into()),
        ] {
            req.file_path = path;
            assert!(template.render(&req).unwrap().starts_with("<fim_suffix>"));
        }
        req.prefix.clear();
        req.suffix.clear();
        assert_eq!(
            template.render(&req).unwrap(),
            "<fim_suffix><fim_prefix><fim_middle>"
        );
    }

    #[test]
    fn raw_fim_control_tokens_are_rejected_in_source_and_stripped_from_responses() {
        for (_, template) in TEMPLATES {
            for token in template
                .markers
                .into_iter()
                .chain(template.stop.iter().copied())
                .filter(|t| !t.is_empty())
            {
                let mut req = request();
                req.prefix.push_str(token);
                let error = template.render(&req).unwrap_err().to_string();
                assert!(!error.contains(&req.prefix));
                let mut req = request();
                req.suffix.push_str(token);
                assert!(template.render(&req).is_err());
                assert_eq!(
                    postprocess(&format!("answer(){token}garbage"), "").as_deref(),
                    Some("answer()")
                );
            }
        }
        for token in sentinels() {
            assert_eq!(
                postprocess(&format!("answer(){token}garbage"), "").as_deref(),
                Some("answer()")
            );
        }
    }
}
