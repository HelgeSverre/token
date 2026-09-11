//! AI-provider drafts reuse the Settings field, choice, validation and save paths.
use super::{FormChoice, FormError, FormField, FormKind, SettingsChange, SettingsForm};
use crate::completion::{
    fim,
    prompt::PromptFormat,
    recency::{self, ContextStrategy},
};
use crate::config::{EditorConfig, LocalServerConfig, ProviderConfig, TransportKind};
use crate::settings::{RowKind, SettingRow};
use std::{str::FromStr, sync::Arc};

const TRANSPORTS: &[TransportKind] = &[
    TransportKind::LlamaCpp,
    TransportKind::Ollama,
    TransportKind::OpenAiCompat,
    TransportKind::MistralFim,
    TransportKind::Tabby,
];
const PROMPTS: &[PromptFormat] = &[
    PromptFormat::Native,
    PromptFormat::Infer,
    PromptFormat::Qwen,
    PromptFormat::StarCoder,
    PromptFormat::CodeLlama,
    PromptFormat::DeepSeek,
    PromptFormat::Codestral,
    PromptFormat::Mellum,
];
const TRANSPORT: usize = 0;
const PROMPT: usize = 1;
const CONTEXT: usize = 2;
const MANAGED: usize = 3;

#[derive(Clone, Copy)]
#[repr(usize)]
enum Field {
    Id,
    Url,
    Model,
    KeyEnv,
    MaxTokens,
    Timeout,
    Alternatives,
    KeepAlive,
    Chunks,
    Lines,
    Executable,
    ModelPath,
    Startup,
    ContextSize,
    GpuLayers,
}

impl SettingsForm {
    pub fn inline_provider(id: Option<&str>, config: &EditorConfig) -> Self {
        let value = id
            .and_then(|id| config.completion.providers.get(id))
            .cloned()
            .unwrap_or_default();
        let local = value.local_server.clone().unwrap_or_default();
        let (strategy, chunks, lines) = match value.context {
            ContextStrategy::None => (
                0,
                recency::default_max_chunks(),
                recency::default_chunk_lines(),
            ),
            ContextStrategy::RecencyRing {
                max_chunks,
                chunk_lines,
            } => (1, max_chunks, chunk_lines),
            ContextStrategy::WorkspaceRetrieval {
                max_chunks,
                chunk_lines,
            } => (2, max_chunks, chunk_lines),
        };
        let field = |label, help, value: &str| FormField::new(label, help, value, false);
        let mut fields = vec![
            field(
                "Provider ID",
                "Unique name used by completion.inline.provider",
                id.unwrap_or(""),
            ),
            field(
                "Base URL",
                "HTTP(S) endpoint; Token adds the transport route. No credentials in URLs",
                &value.url,
            ),
            field(
                "Model",
                "Model name required except for llama.cpp and Tabby, which choose it server-side",
                value.model.as_deref().unwrap_or(""),
            ),
            field(
                "API key environment variable",
                "Variable name only, e.g. MISTRAL_API_KEY; never paste the key itself",
                value.api_key_env.as_deref().unwrap_or(""),
            ),
            field(
                "Maximum output tokens",
                "Positive limit for each generated completion",
                &value.max_tokens.to_string(),
            ),
            field(
                "Request timeout (ms)",
                "Positive time limit for a completion request",
                &value.timeout_ms.to_string(),
            ),
            field(
                "Alternatives",
                "1–8 for OpenAI-compatible endpoints; all other transports require 1",
                &value.n.to_string(),
            ),
            field(
                "Keep alive (seconds)",
                "Ollama only: negative keeps the model loaded; 0 unloads after the request",
                &value.keep_alive.to_string(),
            ),
            field(
                "Maximum context chunks",
                "1–32 snippets; extra context is sent to the configured provider",
                &chunks.to_string(),
            ),
            field(
                "Lines per context chunk",
                "1–256 lines per snippet",
                &lines.to_string(),
            ),
            field(
                "llama-server executable",
                "Installed executable; absolute path, no shell expansion",
                &local.executable.to_string_lossy(),
            ),
            field(
                "GGUF model file",
                "Existing local model; absolute path. Token does not download models",
                &local.model_path.to_string_lossy(),
            ),
            field(
                "Startup timeout (ms)",
                "1–600000; includes loading the local model",
                &local.startup_timeout_ms.to_string(),
            ),
            field(
                "Model context size",
                "Positive llama-server context window (--ctx-size)",
                &local.context_size.to_string(),
            ),
            field(
                "GPU layers",
                "Empty lets llama-server choose; 0 uses CPU only",
                &local.gpu_layers.map(|v| v.to_string()).unwrap_or_default(),
            ),
        ];
        fields[Field::Executable as usize].browse = true;
        fields[Field::ModelPath as usize].browse = true;
        Self {
            session: Arc::new(()), kind: FormKind::InlineProvider(id.map(str::to_owned)), fields,
            choices: vec![
                FormChoice { label: "Transport", help: "Choose the API protocol supported by your server", labels: &["llama.cpp", "Ollama", "OpenAI-compatible", "Mistral FIM", "Tabby"], active: TRANSPORTS.iter().position(|v| *v == value.transport).unwrap_or(0) },
                FormChoice { label: "Prompt format", help: "Native uses prefix/suffix fields; raw FIM formats require Ollama or OpenAI-compatible", labels: &["Native", "Infer", "Qwen", "StarCoder", "CodeLlama", "DeepSeek", "Codestral", "Mellum"], active: PROMPTS.iter().position(|v| *v == value.prompt_format).unwrap_or(0) },
                FormChoice { label: "Extra source context", help: "Opt-in: send snippets from other open files or workspace retrieval to this provider", labels: &["None", "Recent files", "Workspace"], active: strategy },
                FormChoice { label: "Manage local llama-server", help: "Launch the executable on demand; requires llama.cpp at http://127.0.0.1:PORT", labels: &["Off", "On"], active: usize::from(value.local_server.is_some()) },
            ],
            enabled: false, focused: Some(if id.is_some() { Field::Url as usize } else { Field::Id as usize }), dragging: false, saving: false,
            status: "Draft · Save & Use selects this provider without enabling inline suggestions".into(), executable_status: String::new(), remove_pending: false, records_scroll: 0, advanced: false, dirty: false, open_select: None, select_cursor: 0, preset: None,
        }
    }

    pub(super) fn provider_entries(&self) -> Vec<SettingRow> {
        let section = "AI provider";
        let field = |index: Field| RowKind::FormField(index as usize);
        let mut kinds = Vec::new();
        kinds.push(field(Field::Id));
        kinds.push(RowKind::FormChoice(TRANSPORT));
        kinds.extend([Field::Url, Field::Model, Field::KeyEnv].map(field));
        kinds.push(RowKind::FormAdvanced);
        if self.advanced {
            kinds.push(RowKind::FormChoice(PROMPT));
            kinds.extend(
                [
                    Field::MaxTokens,
                    Field::Timeout,
                    Field::Alternatives,
                    Field::KeepAlive,
                ]
                .map(field),
            );
            kinds.push(RowKind::FormChoice(CONTEXT));
            if self.choices[CONTEXT].active != 0 {
                kinds.extend([Field::Chunks, Field::Lines].map(field));
            }
            kinds.push(RowKind::FormChoice(MANAGED));
            if self.choices[MANAGED].active != 0 {
                kinds.extend(
                    [
                        Field::Executable,
                        Field::ModelPath,
                        Field::Startup,
                        Field::ContextSize,
                        Field::GpuLayers,
                    ]
                    .map(field),
                );
            }
        }
        let mut rows: Vec<_> = kinds
            .into_iter()
            .filter_map(|kind| {
                let (label, help) = match kind {
                    RowKind::FormAdvanced => (
                        "Advanced",
                        "Generation limits, prompt format, context and local process settings",
                    ),
                    RowKind::FormField(index) => {
                        (self.fields[index].label, self.fields[index].help)
                    }
                    RowKind::FormChoice(index) => {
                        (self.choices[index].label, self.choices[index].help)
                    }
                    _ => return None,
                };
                Some(SettingRow {
                    kind,
                    section,
                    name: label.into(),
                    description: help.into(),
                })
            })
            .collect();
        rows.push(SettingRow { kind: RowKind::FormActions, section, name: "Configuration".into(), description: "Save keeps the current provider selection; Save & Use selects this provider. Inline remains opt-in".into() });
        rows
    }

    pub(super) fn provider_change(
        &self,
        id: Option<&str>,
        config: &EditorConfig,
    ) -> Result<SettingsChange, FormError> {
        let text = |field: Field| self.fields[field as usize].input.text().trim().to_owned();
        let optional = |field| {
            let value = text(field);
            (!value.is_empty()).then_some(value)
        };
        let name = text(Field::Id);
        if !name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
            || !name
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || b"-_.".contains(&ch))
            || (id != Some(name.as_str()) && config.completion.providers.contains_key(&name))
        {
            return Err(FormError::ProviderId);
        }
        let context = match self.choices[CONTEXT].active {
            0 => ContextStrategy::None,
            1 => ContextStrategy::RecencyRing {
                max_chunks: self.number(Field::Chunks)?,
                chunk_lines: self.number(Field::Lines)?,
            },
            _ => ContextStrategy::WorkspaceRetrieval {
                max_chunks: self.number(Field::Chunks)?,
                chunk_lines: self.number(Field::Lines)?,
            },
        };
        let local_server = if self.choices[MANAGED].active == 1 {
            Some(LocalServerConfig {
                executable: text(Field::Executable).into(),
                model_path: text(Field::ModelPath).into(),
                startup_timeout_ms: self.number(Field::Startup)?,
                context_size: self.number(Field::ContextSize)?,
                gpu_layers: optional(Field::GpuLayers)
                    .map(|_| self.number(Field::GpuLayers))
                    .transpose()?,
            })
        } else {
            None
        };
        let value = ProviderConfig {
            transport: TRANSPORTS[self.choices[TRANSPORT].active],
            prompt_format: PROMPTS[self.choices[PROMPT].active],
            context,
            local_server,
            url: text(Field::Url),
            model: optional(Field::Model),
            api_key_env: optional(Field::KeyEnv),
            max_tokens: self.number(Field::MaxTokens)?,
            timeout_ms: self.number(Field::Timeout)?,
            n: self.number(Field::Alternatives)?,
            keep_alive: self.number(Field::KeepAlive)?,
        };
        fim::validate_config(&value)?;
        Ok(SettingsChange::InlineProvider {
            previous_id: id.map(str::to_owned),
            id: name,
            value,
            select: false,
        })
    }

    fn number<T: FromStr>(&self, field: Field) -> Result<T, FormError> {
        let field = &self.fields[field as usize];
        field
            .input
            .text()
            .trim()
            .parse()
            .map_err(|_| FormError::Number(field.label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_forms_round_trip_all_transports_and_managed_options() {
        let mut config = EditorConfig::default();
        for yaml in [
            "{}",
            "transport: ollama\nmodel: qwen2.5-coder\nprompt_format: qwen\nkeep_alive: 45",
            "transport: open_ai_compat\nmodel: custom\nn: 4\napi_key_env: TEST_TOKEN_VARIABLE\ncontext: {strategy: recency_ring, max_chunks: 2, chunk_lines: 24}",
            "transport: mistral_fim\nurl: https://example.invalid\nmodel: codestral\napi_key_env: TEST_TOKEN_VARIABLE",
            "transport: tabby\ncontext: {strategy: workspace_retrieval, max_chunks: 3, chunk_lines: 40}",
        ] {
            let value: ProviderConfig = serde_yaml::from_str(yaml).unwrap();
            config.completion.providers.insert("existing".into(), value.clone());
            let draft = SettingsForm::inline_provider(Some("existing"), &config);
            let SettingsChange::InlineProvider { value: actual, select, .. } = draft.change(&config).unwrap() else { panic!("provider change") };
            assert_eq!(actual, value);
            assert!(!select);
        }
        let mut value = ProviderConfig::default();
        let root = std::env::current_dir().unwrap();
        value.local_server = Some(LocalServerConfig {
            executable: root.join("llama-server"),
            model_path: root.join("models/code.gguf"),
            startup_timeout_ms: 65000,
            context_size: 4096,
            gpu_layers: Some(0),
        });
        config
            .completion
            .providers
            .insert("managed".into(), value.clone());
        let draft = SettingsForm::inline_provider(Some("managed"), &config);
        let SettingsChange::InlineProvider { value: actual, .. } = draft.change(&config).unwrap()
        else {
            panic!("provider change")
        };
        assert_eq!(actual, value);

        let mut draft = SettingsForm::inline_provider(None, &config);
        draft.fields[Field::Id as usize]
            .input
            .set_content("managed");
        assert!(matches!(draft.change(&config), Err(FormError::ProviderId)));
        draft.fields[Field::Id as usize]
            .input
            .set_content("new-provider");
        draft.fields[Field::Timeout as usize]
            .input
            .set_content("-1");
        assert!(matches!(draft.change(&config), Err(FormError::Number(_))));
        draft.fields[Field::Timeout as usize]
            .input
            .set_content("5000");
        draft.choices[TRANSPORT].active = 1;
        assert!(
            matches!(draft.change(&config), Err(FormError::Provider(_))),
            "Ollama requires a model"
        );
    }
}
