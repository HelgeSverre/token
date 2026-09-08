//! FIM HTTP transports with native prefix/suffix or explicit raw prompt formats.
//! One async HTTP/TLS client owns framing, deadlines and bounded body reads.

use std::time::Duration;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};

use super::inline::{InlineRequest, MAX_ALTERNATIVES};
use super::prompt::{PromptFormat, PromptTemplate};
use super::provider::{InlineProvider, ProviderError, SuggestionFuture};
use crate::config::{ProviderConfig, TransportKind};
use crate::util::byte_size::ByteSize;

const RESPONSE_LIMIT: ByteSize = ByteSize::mebibytes(1);
const CREDENTIAL_LIMIT: ByteSize = ByteSize::kibibytes(4);

pub fn client() -> Result<Client, ProviderError> {
    Ok(Client::builder()
        // Never follow a redirect carrying document context to another endpoint.
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()?)
}

pub struct FimProvider {
    client: Client,
    config: ProviderConfig,
    url: Url,
    authorization: Option<HeaderValue>,
    prompt: Option<PromptTemplate>,
}

impl FimProvider {
    pub fn new(client: Client, config: ProviderConfig) -> Result<Self, ProviderError> {
        Self::with_environment(client, config, |name| std::env::var(name).ok())
    }

    /// Lookup injection keeps credential tests deterministic without changing
    /// process-global environment variables while other tests run.
    fn with_environment(
        client: Client,
        config: ProviderConfig,
        lookup: impl FnOnce(&str) -> Option<String>,
    ) -> Result<Self, ProviderError> {
        config.context.limits()?;
        if config.n == 0 || usize::from(config.n) > MAX_ALTERNATIVES {
            return Err(ProviderError::Configuration("n must be between 1 and 8"));
        }
        if config.prompt_format != PromptFormat::Native
            && !matches!(
                config.transport,
                TransportKind::Ollama | TransportKind::OpenAiCompat
            )
        {
            return Err(ProviderError::Configuration(
                "raw prompt_format requires ollama or open_ai_compat",
            ));
        }
        let prompt = config.prompt_format.resolve(config.model.as_deref())?;
        if config.n > 1 && config.transport != TransportKind::OpenAiCompat {
            return Err(ProviderError::Configuration(
                "n > 1 requires open_ai_compat",
            ));
        }
        if config.timeout_ms == 0 || config.max_tokens == 0 {
            return Err(ProviderError::Configuration(
                "timeout_ms and max_tokens must be positive",
            ));
        }
        if !matches!(
            config.transport,
            TransportKind::LlamaCpp | TransportKind::Tabby
        ) && config
            .model
            .as_deref()
            .is_none_or(|model| model.trim().is_empty())
        {
            return Err(ProviderError::Configuration(
                "this transport requires a model",
            ));
        }
        let mut url = Url::parse(&config.url)
            .map_err(|_| ProviderError::Configuration("expected an HTTP(S) base URL"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ProviderError::Configuration(
                "use an HTTP(S) base URL without credentials, query, or fragment",
            ));
        }
        let authorization = authorization(&config, &url, lookup)?;
        let base = url.path().trim_end_matches('/');
        let route = match config.transport {
            TransportKind::LlamaCpp => "infill",
            TransportKind::Ollama => "api/generate",
            TransportKind::OpenAiCompat | TransportKind::Tabby if base.ends_with("/v1") => {
                "completions"
            }
            TransportKind::OpenAiCompat | TransportKind::Tabby => "v1/completions",
            TransportKind::MistralFim if base.ends_with("/v1") => "fim/completions",
            TransportKind::MistralFim => "v1/fim/completions",
        };
        url.set_path(&format!("{base}/{route}"));
        Ok(Self {
            client,
            config,
            url,
            authorization,
            prompt,
        })
    }

    fn body(&self, request: &InlineRequest) -> Result<Value, ProviderError> {
        super::recency::validate(&request.extra_context, self.config.context)?;
        let prefix = if self.config.transport == TransportKind::LlamaCpp || self.prompt.is_some() {
            std::borrow::Cow::Borrowed(request.prefix.as_str())
        } else {
            super::recency::commented_prefix(request)?
        };
        let mut body = match self.config.transport {
            TransportKind::LlamaCpp => json!({
                "input_prefix": request.prefix,
                "input_suffix": request.suffix,
                "n_predict": self.config.max_tokens,
                "temperature": 0.1,
                "cache_prompt": true,
                "stream": false,
                "t_max_predict_ms": self.config.timeout_ms,
            }),
            TransportKind::Ollama => json!({
                "model": self.config.model,
                "prompt": prefix,
                "suffix": request.suffix,
                "stream": false,
                "keep_alive": self.config.keep_alive,
                "options": { "num_predict": self.config.max_tokens, "temperature": 0.1 },
            }),
            TransportKind::Tabby => {
                // Tabby owns model selection and generation limits. Its optional
                // filepath is workspace-relative; InlineRequest has no workspace
                // root, so do not send its absolute local file_path instead.
                let mut body = json!({
                    "segments": { "prefix": prefix, "suffix": request.suffix },
                    "temperature": 0.1,
                });
                if let Some(language) = request.language.as_deref() {
                    // Reuse LSP's conventional IDs (notably JSX and TSX).
                    let language = crate::syntax::LanguageId::from_code_fence_info(language)
                        .and_then(crate::lsp::sync::language_id_str)
                        .unwrap_or(match language {
                            "bash" => "shellscript",
                            other => other,
                        });
                    body["language"] = json!(language);
                }
                body
            }
            TransportKind::OpenAiCompat | TransportKind::MistralFim => {
                let mut body = json!({
                    "model": self.config.model,
                    "prompt": prefix,
                    "suffix": request.suffix,
                    "max_tokens": self.config.max_tokens,
                    "temperature": 0.1,
                    "stream": false,
                });
                // Mistral FIM has no `n` request parameter.
                if self.config.transport == TransportKind::OpenAiCompat {
                    body["n"] = json!(self.config.n);
                }
                body
            }
        };
        if self.config.transport == TransportKind::LlamaCpp && !request.extra_context.is_empty() {
            body["input_extra"] = json!(request.extra_context);
        }
        if let Some(template) = &self.prompt {
            body["prompt"] = json!(template.render(request)?);
            if let Some(object) = body.as_object_mut() {
                object.remove("suffix");
            }
            if self.config.transport == TransportKind::Ollama {
                body["raw"] = json!(true);
                body["options"]["stop"] = json!(template.stop);
            } else {
                body["stop"] = json!(template.stop);
            }
        }
        Ok(body)
    }

    async fn complete(&self, request: &InlineRequest) -> Result<Vec<String>, ProviderError> {
        let mut builder = self
            .client
            .post(self.url.clone())
            .timeout(Duration::from_millis(self.config.timeout_ms))
            .json(&self.body(request)?);
        if let Some(value) = &self.authorization {
            builder = builder.header(AUTHORIZATION, value.clone());
        }
        let mut response = builder.send().await?;
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|len| len > RESPONSE_LIMIT.as_u64())
        {
            return Err(ProviderError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if chunk.len() > RESPONSE_LIMIT.as_usize().saturating_sub(bytes.len()) {
                return Err(ProviderError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            // Inspect only enough to classify a known capability error; never
            // display backend text, which can echo prompts or credentials.
            if self.config.transport == TransportKind::Ollama && status.as_u16() == 400 {
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    if value
                        .get("error")
                        .and_then(Value::as_str)
                        .is_some_and(|error| {
                            let error = error.to_ascii_lowercase();
                            error.contains("does not support insert")
                                || error.contains("does not support suffix")
                        })
                    {
                        return Err(ProviderError::UnsupportedSuffix);
                    }
                }
            }
            return Err(ProviderError::Status(status.as_u16()));
        }
        match self.config.transport {
            TransportKind::LlamaCpp => {
                #[derive(Deserialize)]
                struct Reply {
                    content: String,
                }
                serde_json::from_slice::<Reply>(&bytes)
                    .map(|reply| vec![reply.content])
                    .map_err(|_| ProviderError::InvalidResponse)
            }
            TransportKind::Ollama => {
                #[derive(Deserialize)]
                struct Reply {
                    response: String,
                    done: bool,
                }
                let reply: Reply =
                    serde_json::from_slice(&bytes).map_err(|_| ProviderError::InvalidResponse)?;
                if !reply.done {
                    return Err(ProviderError::InvalidResponse);
                }
                Ok(vec![reply.response])
            }
            TransportKind::OpenAiCompat | TransportKind::Tabby => {
                #[derive(Deserialize)]
                struct Choice {
                    text: String,
                }
                choices::<Choice>(&bytes, self.config.transport == TransportKind::Tabby)
                    .map(|choices| choices.into_iter().map(|choice| choice.text).collect())
            }
            TransportKind::MistralFim => {
                #[derive(Deserialize)]
                struct Choice {
                    message: Message,
                }
                #[derive(Deserialize)]
                struct Message {
                    content: Content,
                }
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Content {
                    Text(String),
                    Chunks(Vec<TextChunk>),
                }
                #[derive(Deserialize)]
                #[serde(tag = "type", rename_all = "snake_case")]
                enum TextChunk {
                    Text { text: String },
                }
                choices::<Choice>(&bytes, false).map(|choices| {
                    choices
                        .into_iter()
                        .map(|choice| match choice.message.content {
                            Content::Text(text) => text,
                            Content::Chunks(chunks) => chunks
                                .into_iter()
                                .map(|TextChunk::Text { text }| text)
                                .collect(),
                        })
                        .collect()
                })
            }
        }
    }
}

fn choices<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    allow_empty: bool,
) -> Result<Vec<T>, ProviderError> {
    #[derive(Deserialize)]
    struct Reply<T> {
        choices: Vec<T>,
    }
    let choices = serde_json::from_slice::<Reply<T>>(bytes)
        .map_err(|_| ProviderError::InvalidResponse)?
        .choices;
    // Tabby's schema permits an empty list: no suggestion is not a backend
    // failure and must not contribute to automatic-request backoff.
    if choices.is_empty() && !allow_empty {
        return Err(ProviderError::InvalidResponse);
    }
    Ok(choices.into_iter().take(MAX_ALTERNATIVES).collect())
}

fn authorization(
    config: &ProviderConfig,
    url: &Url,
    lookup: impl FnOnce(&str) -> Option<String>,
) -> Result<Option<HeaderValue>, ProviderError> {
    let Some(name) = config.api_key_env.as_deref() else {
        return if config.transport == TransportKind::MistralFim {
            Err(ProviderError::Configuration(
                "Mistral FIM requires api_key_env",
            ))
        } else {
            Ok(None)
        };
    };
    if !name
        .as_bytes()
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(ProviderError::Configuration(
            "api_key_env must name an environment variable",
        ));
    }
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if url.scheme() != "https" && !loopback {
        return Err(ProviderError::Configuration(
            "credentials require HTTPS except on loopback",
        ));
    }
    let key = lookup(name)
        .filter(|key| !key.is_empty())
        .ok_or(ProviderError::Configuration(
            "api_key_env is unset, empty, or not Unicode",
        ))?;
    if key.len() > CREDENTIAL_LIMIT.as_usize() || !key.bytes().all(|c| c.is_ascii_graphic()) {
        return Err(ProviderError::Configuration(
            "api_key_env contains an invalid bearer token",
        ));
    }
    let mut header = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
        ProviderError::Configuration("api_key_env contains an invalid bearer token")
    })?;
    header.set_sensitive(true);
    Ok(Some(header))
}

impl InlineProvider for FimProvider {
    fn suggest<'a>(&'a mut self, request: &'a InlineRequest) -> SuggestionFuture<'a> {
        Box::pin(self.complete(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::inline::RequestSnapshot;
    use crate::model::DocumentId;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn request() -> InlineRequest {
        InlineRequest {
            snapshot: RequestSnapshot {
                document_id: DocumentId(1),
                revision: 1,
                line: 0,
                column: 8,
                request_id: 1,
            },
            prefix: "let a = ".into(),
            suffix: "\n".into(),
            language: Some("rust".into()),
            file_path: None,
            extra_context: Vec::new(),
            explicit: false,
        }
    }

    fn server(response: String) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                headers.push_str(&line);
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            headers.push_str(std::str::from_utf8(&body).unwrap());
            let _ = tx.send(headers);
            let _ = stream.write_all(response.as_bytes());
        });
        (url, rx)
    }

    fn response(status: &str, body: &str) -> String {
        format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
    }

    fn hosted_config(transport: TransportKind, url: String) -> ProviderConfig {
        ProviderConfig {
            transport,
            url,
            model: Some("fixture-fim-model".into()),
            api_key_env: Some("TOKEN_TEST_FIM_KEY".into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn tabby_segments_credentials_and_gateway_paths_round_trip() {
        for base_path in ["", "/v1/", "/gateway/", "/gateway/v1"] {
            for authenticated in [false, true] {
                let (url, seen) = server(response(
                    "200 OK",
                    r#"{"id":"fixture","choices":[{"index":0,"text":"first();"},{"index":1,"text":"second();"}]}"#,
                ));
                let mut provider = FimProvider::with_environment(
                    client().unwrap(),
                    ProviderConfig {
                        transport: TransportKind::Tabby,
                        url: format!("{url}{base_path}"),
                        api_key_env: authenticated.then(|| "TOKEN_TABBY_KEY".into()),
                        ..Default::default()
                    },
                    |name| {
                        assert!(authenticated, "no ambient credential discovery");
                        assert_eq!(name, "TOKEN_TABBY_KEY");
                        Some("synthetic-tabby-fixture".into())
                    },
                )
                .unwrap();
                let mut request = request();
                request.prefix = "// café 🦀\nlet value = ".into();
                request.file_path = Some("/private/workspace/secret.rs".into());
                assert_eq!(
                    provider.suggest(&request).await.unwrap(),
                    ["first();", "second();"]
                );
                let wire = seen.recv_timeout(Duration::from_secs(3)).unwrap();
                let base = if base_path.starts_with("/gateway") {
                    "/gateway"
                } else {
                    ""
                };
                assert!(wire.starts_with(&format!("POST {base}/v1/completions HTTP/1.1\r\n")));
                let (headers, body) = wire.split_once("\r\n\r\n").unwrap();
                assert_eq!(
                    headers
                        .to_ascii_lowercase()
                        .contains("authorization: bearer synthetic-tabby-fixture"),
                    authenticated
                );
                assert_eq!(
                    serde_json::from_str::<Value>(body).unwrap(),
                    json!({
                        "language": "rust",
                        "segments": { "prefix": request.prefix, "suffix": "\n" },
                        "temperature": 0.1,
                    })
                );
                assert!(!body.contains("private/workspace"));
            }
        }
    }

    #[test]
    fn tabby_native_configuration_and_language_ids() {
        let config: ProviderConfig =
            serde_yaml::from_str("transport: tabby\nurl: http://localhost:8080\n").unwrap();
        assert!(config.model.is_none());
        assert_eq!(
            serde_yaml::from_str::<ProviderConfig>(&serde_yaml::to_string(&config).unwrap())
                .unwrap(),
            config
        );
        let provider = FimProvider::new(client().unwrap(), config.clone()).unwrap();
        for (language, expected) in [
            (Some("rust"), Some("rust")),
            (Some("tsx"), Some("typescriptreact")),
            (Some("jsx"), Some("javascriptreact")),
            (Some("bash"), Some("shellscript")),
            (Some("plaintext"), Some("plaintext")),
            (None, None),
        ] {
            let mut request = request();
            request.language = language.map(str::to_owned);
            assert_eq!(
                provider
                    .body(&request)
                    .unwrap()
                    .get("language")
                    .and_then(Value::as_str),
                expected
            );
        }
        for prompt_format in [PromptFormat::Infer, PromptFormat::Qwen] {
            assert!(matches!(
                FimProvider::new(
                    client().unwrap(),
                    ProviderConfig {
                        prompt_format,
                        ..config.clone()
                    }
                ),
                Err(ProviderError::Configuration(_))
            ));
        }
    }

    #[tokio::test]
    async fn tabby_response_alternatives_are_bounded() {
        let body = json!({ "choices": (0..20).map(|index| json!({ "index": index, "text": format!("choice{index}") })).collect::<Vec<_>>() });
        let (url, _) = server(response("200 OK", &body.to_string()));
        let mut provider = FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                transport: TransportKind::Tabby,
                url,
                ..Default::default()
            },
        )
        .unwrap();
        let choices = provider.suggest(&request()).await.unwrap();
        assert_eq!(choices.len(), MAX_ALTERNATIVES);
        assert_eq!(choices.last().unwrap(), "choice7");
    }

    #[tokio::test]
    async fn raw_fim_transports_send_rendered_prompt_without_native_suffix() {
        for transport in [TransportKind::Ollama, TransportKind::OpenAiCompat] {
            let reply = if transport == TransportKind::Ollama {
                r#"{"response":"answer();","done":true}"#
            } else {
                r#"{"choices":[{"text":"answer();"}]}"#
            };
            let (url, seen) = server(response("200 OK", reply));
            let config = ProviderConfig {
                transport,
                url,
                model: Some("Qwen/Qwen2.5-Coder-7B".into()),
                prompt_format: PromptFormat::Infer,
                ..Default::default()
            };
            let mut provider = FimProvider::new(client().unwrap(), config).unwrap();
            assert_eq!(provider.suggest(&request()).await.unwrap(), ["answer();"]);
            let wire = seen.recv_timeout(Duration::from_secs(3)).unwrap();
            let body: Value = serde_json::from_str(wire.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(
                body["prompt"],
                "<|fim_prefix|>let a = <|fim_suffix|>\n<|fim_middle|>"
            );
            assert!(body.get("suffix").is_none());
            assert_eq!(body["stream"], false);
            if transport == TransportKind::Ollama {
                assert_eq!(body["raw"], true);
                assert_eq!(body["options"]["stop"][0], "<|endoftext|>");
            } else {
                assert!(body.get("raw").is_none());
                assert_eq!(body["stop"][0], "<|endoftext|>");
            }
        }
    }

    #[test]
    fn raw_fim_invalid_modes_fail_before_credential_lookup() {
        for (transport, format, model) in [
            (
                TransportKind::LlamaCpp,
                PromptFormat::Qwen,
                "qwen2.5-coder:7b",
            ),
            (
                TransportKind::MistralFim,
                PromptFormat::Codestral,
                "codestral",
            ),
            (TransportKind::OpenAiCompat, PromptFormat::Infer, "unknown"),
            (
                TransportKind::Ollama,
                PromptFormat::Infer,
                "Qwen2.5-Coder-7B-Instruct",
            ),
        ] {
            let config = ProviderConfig {
                transport,
                prompt_format: format,
                model: Some(model.into()),
                ..Default::default()
            };
            assert!(
                FimProvider::with_environment(client().unwrap(), config, |_| panic!(
                    "must fail before credentials"
                ))
                .is_err()
            );
        }
    }

    #[tokio::test]
    async fn extra_context_uses_native_extra_or_commented_prefix_on_the_wire() {
        use crate::completion::recency::{ContextChunk, ContextStrategy};
        for (transport, format) in [
            (TransportKind::LlamaCpp, PromptFormat::Native),
            (TransportKind::Ollama, PromptFormat::Native),
            (TransportKind::OpenAiCompat, PromptFormat::Native),
            (TransportKind::MistralFim, PromptFormat::Native),
            (TransportKind::Tabby, PromptFormat::Native),
            (TransportKind::Ollama, PromptFormat::Qwen),
            (TransportKind::OpenAiCompat, PromptFormat::Codestral),
        ] {
            let reply = match transport {
                TransportKind::LlamaCpp => r#"{"content":"answer();"}"#,
                TransportKind::Ollama => r#"{"response":"answer();","done":true}"#,
                TransportKind::OpenAiCompat | TransportKind::Tabby => {
                    r#"{"choices":[{"text":"answer();"}]}"#
                }
                TransportKind::MistralFim => r#"{"choices":[{"message":{"content":"answer();"}}]}"#,
            };
            let (url, seen) = server(response("200 OK", reply));
            let config = ProviderConfig {
                context: ContextStrategy::WorkspaceRetrieval {
                    max_chunks: 8,
                    chunk_lines: 64,
                },
                prompt_format: format,
                ..hosted_config(transport, url)
            };
            let mut provider = FimProvider::with_environment(client().unwrap(), config, |_| {
                Some("fixture-key".into())
            })
            .unwrap();
            let mut request = request();
            request.extra_context.push(ContextChunk {
                filename: "helpers.rs".into(),
                text: "fn answer() -> u8 { 42 }\n".into(),
            });
            assert_eq!(provider.suggest(&request).await.unwrap(), ["answer();"]);
            let wire = seen.recv_timeout(Duration::from_secs(3)).unwrap();
            let body: Value = serde_json::from_str(wire.split_once("\r\n\r\n").unwrap().1).unwrap();
            if transport == TransportKind::LlamaCpp {
                assert_eq!(body["input_extra"], json!(request.extra_context));
                assert_eq!(body["input_prefix"], request.prefix);
                assert_eq!(body["input_suffix"], request.suffix);
            } else {
                assert!(body.get("input_extra").is_none());
                let prompt = if transport == TransportKind::Tabby {
                    assert_eq!(body["segments"]["suffix"], request.suffix);
                    assert!(body.get("prompt").is_none());
                    body["segments"]["prefix"].as_str().unwrap()
                } else {
                    body["prompt"].as_str().unwrap()
                };
                assert!(prompt.contains("// Path: helpers.rs\n// fn answer() -> u8 { 42 }\n"));
                assert!(prompt.contains(&request.prefix));
                if format == PromptFormat::Native && transport != TransportKind::Tabby {
                    assert_eq!(body["suffix"], request.suffix);
                } else {
                    assert!(body.get("suffix").is_none());
                }
            }
            request.extra_context[0].text = "<|fim_middle|>".into();
            if format == PromptFormat::Qwen {
                assert!(
                    provider.body(&request).is_err(),
                    "raw markers in extra context are checked too"
                );
            }
            request.extra_context[0].filename = "bad\nheader".into();
            assert!(provider.body(&request).is_err());
            provider.config.context = ContextStrategy::None;
            request.extra_context[0].filename = "valid.rs".into();
            assert!(
                provider.body(&request).is_err(),
                "extra context cannot bypass opt-in"
            );
        }
    }

    #[test]
    fn raw_fim_configuration_round_trips_without_changing_native_defaults() {
        let old: ProviderConfig =
            serde_yaml::from_str("transport: ollama\nmodel: fixture\n").unwrap();
        assert_eq!(old.prompt_format, PromptFormat::Native);
        assert!(FimProvider::new(client().unwrap(), old)
            .unwrap()
            .body(&request())
            .unwrap()
            .get("suffix")
            .is_some());
        let raw: ProviderConfig = serde_yaml::from_str(
            "transport: open_ai_compat\nmodel: alias\nprompt_format: deep_seek\n",
        )
        .unwrap();
        assert_eq!(raw.prompt_format, PromptFormat::DeepSeek);
        assert_eq!(
            serde_yaml::from_str::<ProviderConfig>(&serde_yaml::to_string(&raw).unwrap()).unwrap(),
            raw
        );
        assert!(serde_yaml::from_str::<ProviderConfig>("prompt_format: typo").is_err());
    }

    #[tokio::test]
    async fn hosted_transports_send_native_suffix_and_parse_their_own_shape() {
        for (transport, route, reply) in [
            (
                TransportKind::OpenAiCompat,
                "completions",
                r#"{"choices":[{"text":"answer();"}]}"#,
            ),
            (
                TransportKind::MistralFim,
                "fim/completions",
                r#"{"choices":[{"message":{"content":"answer();"}}]}"#,
            ),
        ] {
            for base_path in ["", "/v1/", "/gateway/v1", "/gateway/"] {
                let (url, seen) = server(response("200 OK", reply));
                let config = hosted_config(transport, format!("{url}{base_path}"));
                let mut provider =
                    FimProvider::with_environment(client().unwrap(), config, |name| {
                        assert_eq!(name, "TOKEN_TEST_FIM_KEY");
                        Some("fixture-not-a-real-key".into())
                    })
                    .unwrap();
                assert!(provider.authorization.as_ref().unwrap().is_sensitive());
                assert!(!format!("{:?}", provider.authorization).contains("fixture-not-a-real-key"));
                assert_eq!(
                    provider.suggest(&request()).await.unwrap(),
                    vec!["answer();"]
                );
                let seen = seen.recv_timeout(Duration::from_secs(3)).unwrap();
                let prefix = if base_path.starts_with("/gateway") {
                    "/gateway"
                } else {
                    ""
                };
                assert!(seen.starts_with(&format!("POST {prefix}/v1/{route} HTTP/1.1\r\n")));
                let (headers, body) = seen.split_once("\r\n\r\n").unwrap();
                assert!(headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer fixture-not-a-real-key"));
                let body: Value = serde_json::from_str(body).unwrap();
                assert_eq!(body["prompt"], "let a = ");
                assert_eq!(body["suffix"], "\n");
                assert_eq!(body["model"], "fixture-fim-model");
                assert_eq!(body["temperature"], 0.1);
                assert_eq!(body["stream"], false);
                assert_eq!(body["max_tokens"], 128);
                assert_eq!(
                    body.get("n"),
                    (transport == TransportKind::OpenAiCompat).then_some(&json!(1))
                );
                assert!(!body.to_string().contains("fixture-not-a-real-key"));
                assert!(body.get("keep_alive").is_none());
            }
        }
    }

    #[tokio::test]
    async fn openai_compatible_local_endpoint_does_not_require_or_discover_a_key() {
        let (url, seen) = server(response("200 OK", r#"{"choices":[{"text":"local();"}]}"#));
        let mut config = hosted_config(TransportKind::OpenAiCompat, url);
        config.api_key_env = None;
        let mut provider = FimProvider::with_environment(client().unwrap(), config, |_| {
            panic!("no ambient credential discovery")
        })
        .unwrap();
        assert_eq!(
            provider.suggest(&request()).await.unwrap(),
            vec!["local();"]
        );
        assert!(!seen
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .to_ascii_lowercase()
            .contains("authorization:"));
    }

    #[tokio::test]
    async fn openai_multiple_choices_are_requested_and_preserved_in_response_order() {
        let (url, seen) = server(response(
            "200 OK",
            r#"{"choices":[{"text":"first"},{"text":"second"},{"text":"third"}]}"#,
        ));
        let mut config = hosted_config(TransportKind::OpenAiCompat, url);
        config.n = 3;
        config.api_key_env = None;
        let mut provider = FimProvider::new(client().unwrap(), config).unwrap();
        assert_eq!(
            provider.suggest(&request()).await.unwrap(),
            vec!["first", "second", "third"]
        );
        let seen = seen.recv_timeout(Duration::from_secs(3)).unwrap();
        let body: Value = serde_json::from_str(seen.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["n"], 3);
        assert_eq!(body["suffix"], "\n");
    }

    #[tokio::test]
    async fn alternative_request_counts_are_bounded_and_capability_checked() {
        for transport in [
            TransportKind::LlamaCpp,
            TransportKind::Ollama,
            TransportKind::MistralFim,
            TransportKind::OpenAiCompat,
            TransportKind::Tabby,
        ] {
            for n in [0, 1, 2, 8, 9, u8::MAX] {
                let mut config = hosted_config(transport, "https://example.invalid".into());
                config.n = n;
                let valid =
                    n == 1 || (transport == TransportKind::OpenAiCompat && (2..=8).contains(&n));
                let result = FimProvider::with_environment(client().unwrap(), config, |_| {
                    assert!(valid, "invalid count must fail before credential lookup");
                    Some("fixture".into())
                });
                assert_eq!(result.is_ok(), valid, "{transport:?}, n={n}");
            }
        }
        let raw = json!({ "choices": (0..20).map(|i| json!({"text": format!("answer{i}")})).collect::<Vec<_>>() });
        let (url, _) = server(response("200 OK", &raw.to_string()));
        let mut config = hosted_config(TransportKind::OpenAiCompat, url);
        config.n = 8;
        config.api_key_env = None;
        let mut provider = FimProvider::new(client().unwrap(), config).unwrap();
        let result = provider.suggest(&request()).await.unwrap();
        assert_eq!(result.len(), MAX_ALTERNATIVES);
        assert_eq!(result.last().unwrap(), "answer7");
    }

    #[tokio::test]
    async fn hosted_response_validation_and_safe_errors() {
        for transport in [
            TransportKind::OpenAiCompat,
            TransportKind::MistralFim,
            TransportKind::Tabby,
        ] {
            for body in [
                "{}",
                r#"{"choices":[{}]}"#,
                r#"{"choices":[{"text":null,"message":{"content":null}}]}"#,
            ] {
                let (url, _) = server(response("200 OK", body));
                let mut provider = FimProvider::with_environment(
                    client().unwrap(),
                    hosted_config(transport, url),
                    |_| Some("fixture".into()),
                )
                .unwrap();
                assert!(matches!(
                    provider.suggest(&request()).await,
                    Err(ProviderError::InvalidResponse)
                ));
            }
            for status in ["401 Unauthorized", "429 Too Many Requests", "500 Error"] {
                let (url, _) = server(response(status, "secret prompt and credential"));
                let mut provider = FimProvider::with_environment(
                    client().unwrap(),
                    hosted_config(transport, url),
                    |_| Some("fixture".into()),
                )
                .unwrap();
                let error = provider.suggest(&request()).await.unwrap_err();
                assert!(matches!(error, ProviderError::Status(_)));
                assert!(!error.to_string().contains("secret"));
            }
        }
    }

    #[tokio::test]
    async fn tabby_empty_choices_are_valid_but_other_transports_keep_validation() {
        for transport in [
            TransportKind::Tabby,
            TransportKind::OpenAiCompat,
            TransportKind::MistralFim,
        ] {
            let (url, _) = server(response("200 OK", r#"{"id":"empty","choices":[]}"#));
            let mut provider = FimProvider::with_environment(
                client().unwrap(),
                hosted_config(transport, url),
                |_| Some("fixture".into()),
            )
            .unwrap();
            let result = provider.suggest(&request()).await;
            if transport == TransportKind::Tabby {
                assert!(result.unwrap().is_empty());
            } else {
                assert!(matches!(result, Err(ProviderError::InvalidResponse)));
            }
        }
    }

    #[tokio::test]
    async fn mistral_text_chunks_are_joined_but_nontext_content_is_rejected() {
        for (body, expected) in [
            (
                r#"{"choices":[{"message":{"content":[{"type":"text","text":"héllo"},{"type":"text","text":"();"}]}}]}"#,
                Some("héllo();"),
            ),
            (
                r#"{"choices":[{"message":{"content":[{"type":"image_url","image_url":"private"}]}}]}"#,
                None,
            ),
        ] {
            let (url, _) = server(response("200 OK", body));
            let mut provider = FimProvider::with_environment(
                client().unwrap(),
                hosted_config(TransportKind::MistralFim, url),
                |_| Some("fixture".into()),
            )
            .unwrap();
            let result = provider.suggest(&request()).await;
            match expected {
                Some(text) => assert_eq!(result.unwrap(), vec![text]),
                None => assert!(matches!(result, Err(ProviderError::InvalidResponse))),
            }
        }
    }

    #[test]
    fn credentials_are_validated_before_network_and_never_echoed() {
        let config = hosted_config(TransportKind::MistralFim, "https://example.invalid".into());
        let url = Url::parse(&config.url).unwrap();
        for key in [
            None,
            Some(String::new()),
            Some("private\r\nInjected: value".into()),
            Some("private with spaces".into()),
            Some("é-secret".into()),
            Some("x".repeat(CREDENTIAL_LIMIT.as_usize() + 1)),
        ] {
            let error = authorization(&config, &url, |_| key).unwrap_err();
            assert!(matches!(error, ProviderError::Configuration(_)));
            assert!(!error.to_string().contains("private"));
            assert!(!error.to_string().contains("é-secret"));
        }
        for name in ["", "1KEY", "KEY=secret", "KEY\0secret", "KEY-secret"] {
            let mut config = config.clone();
            config.api_key_env = Some(name.into());
            assert!(authorization(&config, &url, |_| panic!(
                "invalid name must not be looked up"
            ))
            .is_err());
        }
        let mut config = config;
        config.api_key_env = None;
        assert!(authorization(&config, &url, |_| panic!("missing name")).is_err());
    }

    #[test]
    fn credentials_require_https_or_exact_loopback() {
        let config = hosted_config(TransportKind::OpenAiCompat, String::new());
        for url in [
            "http://example.invalid",
            "http://localhost.example.invalid",
            "http://192.0.2.1",
        ] {
            assert!(
                authorization(&config, &Url::parse(url).unwrap(), |_| panic!(
                    "reject before looking up credentials"
                ))
                .is_err()
            );
        }
        for url in [
            "https://example.invalid",
            "http://localhost:1234",
            "http://127.0.0.1:1234",
            "http://[::1]:1234",
        ] {
            assert!(authorization(&config, &Url::parse(url).unwrap(), |_| Some(
                "fixture".into()
            ))
            .unwrap()
            .is_some());
        }
    }

    #[tokio::test]
    async fn authenticated_redirect_does_not_forward_credentials_or_context() {
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let redirect = format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", destination.local_addr().unwrap());
        let (url, _) = server(redirect);
        let mut provider = FimProvider::with_environment(
            client().unwrap(),
            hosted_config(TransportKind::MistralFim, url),
            |_| Some("fixture".into()),
        )
        .unwrap();
        assert!(matches!(
            provider.suggest(&request()).await,
            Err(ProviderError::Status(307))
        ));
        assert!(
            matches!(destination.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[tokio::test]
    async fn llama_prefix_suffix_and_base_path_round_trip() {
        let (url, seen) = server(response("200 OK", r#"{"content":"1 + 2;"}"#));
        let mut provider = FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                url: format!("{url}/llama/"),
                max_tokens: 32,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(provider.suggest(&request()).await.unwrap(), vec!["1 + 2;"]);
        let seen = seen.recv().unwrap();
        assert!(seen.starts_with("POST /llama/infill HTTP/1.1\r\n"));
        let body: Value = serde_json::from_str(seen.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["input_prefix"], "let a = ");
        assert_eq!(body["input_suffix"], "\n");
        assert_eq!(body["n_predict"], 32);
    }

    #[tokio::test]
    async fn ollama_sends_model_suffix_and_residency() {
        let (url, seen) = server(response("200 OK", r#"{"response":"hello();","done":true}"#));
        let mut provider = FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                url,
                transport: TransportKind::Ollama,
                model: Some("code-model".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            provider.suggest(&request()).await.unwrap(),
            vec!["hello();"]
        );
        let seen = seen.recv().unwrap();
        assert!(seen.starts_with("POST /api/generate HTTP/1.1\r\n"));
        let body: Value = serde_json::from_str(seen.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["model"], "code-model");
        assert_eq!(body["prompt"], "let a = ");
        assert_eq!(body["suffix"], "\n");
        assert_eq!(body["keep_alive"], -1);
        assert_eq!(body["options"]["num_predict"], 128);
        assert_eq!(body["stream"], false);
    }

    #[tokio::test]
    async fn http_framing_limits_and_errors() {
        for (wire, expected) in [
            ("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nb\r\n{\"content\":\r\n8\r\n\"chunk\"}\r\n0\r\n\r\n".into(), "chunk"),
            (response("503 Unavailable", "private code"), "HTTP 503"),
            (response("302 Found", "private code"), "HTTP 302"),
            (response("200 OK", "{}"), "invalid inline backend response"),
            ("HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\n".into(), "size limit"),
        ] {
            let (url, _seen) = server(wire);
            let mut provider = FimProvider::new(client().unwrap(), ProviderConfig { url, ..Default::default() }).unwrap();
            let result = provider.suggest(&request()).await;
            let text = result.map(|texts| texts.join("")).unwrap_or_else(|error| error.to_string());
            assert!(text.contains(expected), "{text}");
            assert!(!text.contains("private code"));
        }
    }

    #[tokio::test]
    async fn ollama_capability_errors_are_actionable_without_echoing_the_body() {
        let (url, _) = server(response(
            "400 Bad Request",
            r#"{"error":"private-model does not support insert; secret prompt"}"#,
        ));
        let mut provider = FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                url,
                transport: TransportKind::Ollama,
                model: Some("code-model".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            provider.suggest(&request()).await,
            Err(ProviderError::UnsupportedSuffix)
        ));
    }

    #[tokio::test]
    async fn validates_urls_and_model_without_echoing_secrets() {
        for url in [
            "file:///tmp/x",
            "http://user:secret@localhost",
            "https://host/?key=secret",
            "https://host/#secret",
        ] {
            let error = FimProvider::new(
                client().unwrap(),
                ProviderConfig {
                    url: url.into(),
                    ..Default::default()
                },
            )
            .err()
            .unwrap();
            assert!(!error.to_string().contains("secret"));
        }
        for url in ["https://example.invalid", "http://[::1]:8012"] {
            assert!(FimProvider::new(
                client().unwrap(),
                ProviderConfig {
                    url: url.into(),
                    ..Default::default()
                }
            )
            .is_ok());
        }
        assert!(FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                transport: TransportKind::Ollama,
                ..Default::default()
            }
        )
        .is_err());
    }

    #[tokio::test]
    async fn total_deadline_covers_an_unresponsive_backend() {
        assert_total_deadline(TransportKind::LlamaCpp).await;
    }

    #[tokio::test]
    async fn tabby_http_deadline_and_chunked_body_limit() {
        assert_total_deadline(TransportKind::Tabby).await;
        assert_chunked_body_limit(TransportKind::Tabby).await;
    }

    async fn assert_total_deadline(transport: TransportKind) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(3));
        });
        let mut provider = FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                url,
                timeout_ms: 100,
                transport,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            provider.suggest(&request()).await,
            Err(ProviderError::Timeout)
        ));
        let _ = release.send(());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn chunked_responses_cannot_bypass_the_body_limit() {
        assert_chunked_body_limit(TransportKind::LlamaCpp).await;
    }

    async fn assert_chunked_body_limit(transport: TransportKind) {
        let too_large = "a".repeat(RESPONSE_LIMIT.as_usize() + 1);
        let wire = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{too_large}\r\n0\r\n\r\n",
            too_large.len()
        );
        let (url, _) = server(wire);
        let mut provider = FimProvider::new(
            client().unwrap(),
            ProviderConfig {
                url,
                transport,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            provider.suggest(&request()).await,
            Err(ProviderError::ResponseTooLarge)
        ));
    }
}
