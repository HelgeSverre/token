//! Editor configuration persistence
//!
//! Stores user preferences in `~/.config/token-editor/config.yaml`

use serde::{Deserialize, Serialize};

/// Result of reloading configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ReloadResult {
    /// Successfully loaded from file
    Loaded,
    /// File doesn't exist, using defaults
    FileNotFound,
    /// Parse error, using defaults
    ParseError(String),
    /// Read error (locked/no access), using defaults
    ReadError(String),
    /// No config directory available
    NoConfigDir,
}

/// Editor configuration that persists across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Fallbacks for text documents; matching .editorconfig rules take precedence.
    #[serde(default)]
    pub text: crate::model::TextPreferences,
    #[serde(default = "default_true")]
    pub editorconfig: bool,
    #[serde(default)]
    pub session: SessionConfig,
    /// Selected theme id (e.g., "default-dark", "fleet-dark")
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Monospaced family for code, terminal grids, explorer, tabs and all inputs.
    #[serde(default = "default_editor_font")]
    pub editor_font: String,

    /// Family for application chrome and UI labels.
    #[serde(default = "default_ui_font")]
    pub ui_font: String,

    /// Cursor blink interval in milliseconds (default: 600)
    #[serde(default = "default_cursor_blink_ms")]
    pub cursor_blink_ms: u64,

    /// Automatically surround selected text when typing brackets/quotes (default: true)
    #[serde(default = "default_true")]
    pub auto_surround: bool,

    /// Highlight matching bracket when cursor is adjacent to one (default: true)
    #[serde(default = "default_true")]
    pub bracket_matching: bool,

    /// Show scrollbars in editor panes (default: true)
    ///
    /// When false, no scrollbars are rendered and no space is reserved for them.
    #[serde(default = "default_true")]
    pub show_scrollbar: bool,

    /// Show vertical guides at inferred indentation steps (default: true).
    #[serde(default = "default_true")]
    pub indent_guides: bool,
    /// Reload unmodified text files when their contents change on disk.
    #[serde(default = "default_true")]
    pub auto_reload: bool,

    /// Status bar font size in logical px (default: 12, editor text is 14)
    #[serde(default = "default_status_bar_font_size")]
    pub status_bar_font_size: f32,

    /// Show the LSP hover card when the mouse dwells over editor text,
    /// Zed-style (default: true). The keyboard binding (Shift+Cmd+D,
    /// caret-anchored) is unaffected by this setting.
    #[serde(default = "default_true")]
    pub hover_on_mouse: bool,

    /// How long the mouse must stay still over editor text before
    /// `hover_on_mouse` triggers, in milliseconds (default: 300, mirrors
    /// Zed's `hover_popover_delay`).
    #[serde(default = "default_hover_delay_ms")]
    pub hover_delay_ms: u64,

    /// Language server settings (see `LspConfig`).
    #[serde(default)]
    pub lsp: LspConfig,

    /// Autocomplete settings (see `CompletionConfig`).
    #[serde(default)]
    pub completion: CompletionConfig,

    /// Run `textDocument/formatting` before every save (default: false).
    /// Saves unformatted when the server can't format within ~2 s.
    #[serde(default)]
    pub format_on_save: bool,
    /// Automatic saving of file-backed documents, off until enabled.
    #[serde(default)]
    pub auto_save: AutoSaveConfig,
}

/// Independent automatic save triggers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoSaveMode {
    #[default]
    Off,
    OnFocusLoss,
    AfterDelay,
    OnFocusLossAndDelay,
}

impl AutoSaveMode {
    pub fn on_focus_loss(self) -> bool {
        matches!(self, Self::OnFocusLoss | Self::OnFocusLossAndDelay)
    }

    pub fn after_delay(self) -> bool {
        matches!(self, Self::AfterDelay | Self::OnFocusLossAndDelay)
    }
}

/// Auto-save uses document idle time, independently of cursor movement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoSaveConfig {
    pub mode: AutoSaveMode,
    pub delay_ms: u64,
    pub format_on_save: bool,
}

impl Default for AutoSaveConfig {
    fn default() -> Self {
        Self {
            mode: AutoSaveMode::Off,
            delay_ms: 1000,
            format_on_save: false,
        }
    }
}

impl AutoSaveConfig {
    /// Keep invalid/extreme YAML values from spinning or overflowing deadlines.
    pub fn delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.delay_ms.clamp(100, 86_400_000))
    }
}

/// Session metadata never includes unsaved text or undo history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_true")]
    pub restore: bool,
    #[serde(default = "default_true")]
    pub save_on_exit: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            restore: true,
            save_on_exit: true,
        }
    }
}

/// Autocomplete settings, stored under `completion:` in `config.yaml`:
///
/// ```yaml
/// completion:
///   enabled: true
///   menu:
///     enabled: true
///     min_word_length: 3
///     words: fallback
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct CompletionConfig {
    /// Master switch for both menu and inline suggestions, including explicit requests.
    pub enabled: bool,
    /// Dropdown policy, independent of inline suggestions.
    pub menu: CompletionMenuConfig,
    /// Inline (ghost-text) suggestions — off until a provider is configured.
    pub inline: InlineConfig,
    /// Named backends `inline.provider` picks from.
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            menu: CompletionMenuConfig::default(),
            inline: InlineConfig::default(),
            providers: std::collections::HashMap::new(),
        }
    }
}

/// Dropdown settings. `enabled` controls automatic opening, not Ctrl+Space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompletionMenuConfig {
    pub enabled: bool,
    /// Minimum candidate identifier length in characters, with an effective floor of one.
    pub min_word_length: usize,
    pub words: WordsMode,
}

impl Default for CompletionMenuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_word_length: 3,
            words: WordsMode::default(),
        }
    }
}

impl<'de> Deserialize<'de> for CompletionConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Keep legacy input compatibility at the serialization boundary, never
        // as a second runtime setting. New menu.words wins when both are present.
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct MenuInput {
            enabled: Option<bool>,
            min_word_length: Option<usize>,
            words: Option<WordsMode>,
        }
        #[derive(Deserialize)]
        struct Input {
            #[serde(default = "default_true")]
            enabled: bool,
            #[serde(default)]
            menu: MenuInput,
            words: Option<WordsMode>,
            #[serde(default)]
            inline: InlineConfig,
            #[serde(default)]
            providers: std::collections::HashMap<String, ProviderConfig>,
        }
        let input = Input::deserialize(deserializer)?;
        let defaults = CompletionMenuConfig::default();
        Ok(Self {
            enabled: input.enabled,
            menu: CompletionMenuConfig {
                enabled: input.menu.enabled.unwrap_or(defaults.enabled),
                min_word_length: input
                    .menu
                    .min_word_length
                    .unwrap_or(defaults.min_word_length),
                words: input.menu.words.or(input.words).unwrap_or(defaults.words),
            },
            inline: input.inline,
            providers: input.providers,
        })
    }
}

/// `completion.inline`: ghost-text suggestions (autocomplete.md Phase 2).
///
/// ```yaml
/// completion:
///   inline:
///     enabled: true
///     provider: local
///     debounce_ms: 300
///     max_line_suffix: 8
///   providers:
///     local:
///       transport: llama_cpp
///       url: http://127.0.0.1:8012
///       max_tokens: 128
///       timeout_ms: 5000
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Store only aggregate response outcomes per provider on this machine.
    #[serde(default = "default_true")]
    pub statistics: bool,
    /// Key into `completion.providers`.
    #[serde(default = "default_inline_provider")]
    pub provider: String,
    /// Quiet time after the last keystroke before a request goes out;
    /// an explicit trigger skips it.
    #[serde(default = "default_inline_debounce_ms")]
    pub debounce_ms: u64,
    /// Auto-trigger only with at most this many non-closer chars right of
    /// the cursor.
    #[serde(default = "default_max_line_suffix")]
    pub max_line_suffix: usize,
}

impl Default for InlineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            statistics: true,
            provider: default_inline_provider(),
            debounce_ms: default_inline_debounce_ms(),
            max_line_suffix: default_max_line_suffix(),
        }
    }
}

fn default_inline_provider() -> String {
    "local".to_owned()
}
fn default_inline_debounce_ms() -> u64 {
    300
}
fn default_max_line_suffix() -> usize {
    8
}

/// One inline-suggestion backend. Credentials and network work stay runtime-side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Opt-in ownership of a local llama-server executable and model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_server: Option<LocalServerConfig>,
    #[serde(default)]
    pub transport: TransportKind,
    /// Native by default; an explicit format selects raw FIM on compatible transports.
    #[serde(default)]
    pub prompt_format: crate::completion::prompt::PromptFormat,
    /// Opt-in snippets from open buffers; collected and committed on idle.
    #[serde(default)]
    pub context: crate::completion::recency::ContextStrategy,
    #[serde(default = "default_provider_url")]
    pub url: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_provider_timeout_ms")]
    pub timeout_ms: u64,
    /// Required except for llama.cpp and Tabby, which select models server-side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Ollama residency in seconds. Negative keeps the model loaded.
    #[serde(default = "default_keep_alive")]
    pub keep_alive: i64,
    /// Environment variable name, never the credential itself. Resolved only by
    /// the worker when constructing a provider; required for Mistral FIM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Number of alternatives requested. Only OpenAI-compatible supports >1.
    #[serde(default = "default_provider_n")]
    pub n: u8,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            local_server: None,
            transport: TransportKind::default(),
            prompt_format: crate::completion::prompt::PromptFormat::default(),
            context: crate::completion::recency::ContextStrategy::default(),
            url: default_provider_url(),
            max_tokens: default_max_tokens(),
            timeout_ms: default_provider_timeout_ms(),
            model: None,
            keep_alive: default_keep_alive(),
            api_key_env: None,
            n: default_provider_n(),
        }
    }
}

/// Managed llama.cpp startup. Paths are literal absolute paths, not shell input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalServerConfig {
    pub executable: std::path::PathBuf,
    pub model_path: std::path::PathBuf,
    #[serde(default = "default_server_startup_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_server_context_size")]
    pub context_size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_layers: Option<u32>,
}

fn default_server_startup_ms() -> u64 {
    120_000
}

fn default_server_context_size() -> u32 {
    8192
}

fn default_provider_url() -> String {
    "http://127.0.0.1:8012".to_owned()
}
fn default_provider_n() -> u8 {
    1
}
fn default_keep_alive() -> i64 {
    -1
}
fn default_max_tokens() -> u32 {
    128
}
fn default_provider_timeout_ms() -> u64 {
    5000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    #[default]
    LlamaCpp,
    Ollama,
    OpenAiCompat,
    MistralFim,
    Tabby,
}

/// `completion.menu.words`: `enabled` always lists buffer words, `fallback`
/// (default) lists them only until the language server answers, `disabled`
/// never lists them. Snippets are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WordsMode {
    Enabled,
    #[default]
    Fallback,
    Disabled,
}

/// Language server settings, stored under `lsp:` in `config.yaml`:
///
/// ```yaml
/// lsp:
///   enabled: true
///   servers:
///     rust-analyzer:
///       command: /custom/path/rust-analyzer
///     pyright:
///       enabled: false
/// ```
///
/// Overrides are keyed by `LspServerDef::id`, not by language — to run
/// `laravel-lsp` instead of the default `phpantom`, override the
/// `phpantom` entry's `command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Master switch; `false` disables every server regardless of
    /// per-server settings.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub servers: std::collections::HashMap<String, LspServerOverride>,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspServerOverride {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub enabled: Option<bool>,
    /// Arbitrary server-specific options sent as `initializationOptions`
    /// in the `initialize` request (e.g. pyright's `python` config).
    /// Passed through verbatim — the editor doesn't interpret it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<serde_json::Value>,
    /// Server-specific settings returned from `workspace/configuration`
    /// requests (e.g. rust-analyzer's `cargo.features`). Each requested
    /// configuration `section` is looked up as a dotted path into this
    /// object; a missing section answers `null`, matching the previous
    /// all-null reply for servers with no configured settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
}

fn default_theme() -> String {
    "default-dark".to_string()
}

fn default_cursor_blink_ms() -> u64 {
    600
}

fn default_true() -> bool {
    true
}

fn default_status_bar_font_size() -> f32 {
    12.0
}

fn default_editor_font() -> String {
    "JetBrains Mono".into()
}

fn default_ui_font() -> String {
    "Inter".into()
}

fn default_hover_delay_ms() -> u64 {
    300
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            text: Default::default(),
            editorconfig: true,
            session: SessionConfig::default(),
            theme: default_theme(),
            editor_font: default_editor_font(),
            ui_font: default_ui_font(),
            cursor_blink_ms: default_cursor_blink_ms(),
            auto_surround: true,
            bracket_matching: true,
            show_scrollbar: true,
            indent_guides: true,
            auto_reload: true,
            status_bar_font_size: default_status_bar_font_size(),
            hover_on_mouse: true,
            hover_delay_ms: default_hover_delay_ms(),
            lsp: LspConfig::default(),
            completion: CompletionConfig::default(),
            format_on_save: false,
            auto_save: AutoSaveConfig::default(),
        }
    }
}

impl EditorConfig {
    /// Status bar font size clamped to a usable range, in logical px.
    pub fn status_bar_font_size_clamped(&self) -> f32 {
        self.status_bar_font_size.clamp(8.0, 24.0)
    }
}

impl EditorConfig {
    /// Load config from disk, or return defaults if not found
    pub fn load() -> Self {
        let Some(path) = crate::config_paths::config_file() else {
            tracing::debug!("No config directory available, using defaults");
            return Self::default();
        };

        if !path.exists() {
            tracing::debug!(
                "Config file not found at {}, using defaults",
                path.display()
            );
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str(&content) {
                Ok(config) => {
                    tracing::info!("Loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    tracing::warn!("Failed to parse config at {}: {}", path.display(), e);
                    Self::default()
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read config at {}: {}", path.display(), e);
                Self::default()
            }
        }
    }

    /// Reload config from disk with detailed status for user feedback
    pub fn reload() -> (Self, ReloadResult) {
        let Some(path) = crate::config_paths::config_file() else {
            return (Self::default(), ReloadResult::NoConfigDir);
        };

        if !path.exists() {
            return (Self::default(), ReloadResult::FileNotFound);
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str(&content) {
                Ok(config) => (config, ReloadResult::Loaded),
                Err(e) => (Self::default(), ReloadResult::ParseError(e.to_string())),
            },
            Err(e) => (Self::default(), ReloadResult::ReadError(e.to_string())),
        }
    }

    /// Save config to disk
    ///
    /// Creates the config directory if it doesn't exist. Unknown YAML keys are
    /// preserved; malformed existing files are left untouched. YAML comments
    /// and formatting are not retained by the serializer.
    pub fn save(&self) -> Result<(), String> {
        let path = crate::config_paths::config_file()
            .ok_or_else(|| "No config directory available".to_string())?;
        self.save_to(&path)
    }

    /// Save to an explicit path — factored out of `save()` so tests can
    /// exercise real file I/O against a scratch dir without mutating the
    /// process-global `XDG_CONFIG_HOME` (mirrors `CommandHistory::save_to`,
    /// which has the same race-under-parallel-tests concern).
    fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let mut value =
            serde_yaml::to_value(self).map_err(|e| format!("Failed to serialize config: {}", e))?;
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let mut old: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
                    format!(
                        "Refusing to overwrite invalid config at {}: {e}",
                        path.display()
                    )
                })?;
                if !old.is_null() {
                    // Serializing the old typed config tells us which keys are
                    // known, including omitted optional fields and dynamic map
                    // entries. Their removal must not be mistaken for an unknown
                    // key that should be copied back.
                    let known: Self = serde_yaml::from_value(old.clone()).map_err(|e| {
                        format!(
                            "Refusing to overwrite invalid config at {}: {e}",
                            path.display()
                        )
                    })?;
                    let known = serde_yaml::to_value(known)
                        .map_err(|e| format!("Failed to serialize existing config: {e}"))?;
                    // The legacy key is recognized on input but deliberately
                    // absent from canonical output, not an unknown user key.
                    if let Some(completion) = old
                        .get_mut("completion")
                        .and_then(serde_yaml::Value::as_mapping_mut)
                    {
                        completion.remove(serde_yaml::Value::String("words".into()));
                    }
                    keep_unknown(&mut value, old, &known);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to read config at {}: {error}",
                    path.display()
                ))
            }
        }
        let content = serde_yaml::to_string(&value)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;

        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write config to {}: {}", path.display(), e))?;

        tracing::info!("Saved config to {}", path.display());
        Ok(())
    }

    /// Update theme and save
    pub fn set_theme(&mut self, theme_id: &str) -> Result<(), String> {
        self.theme = theme_id.to_string();
        self.save()
    }
}

/// Copy unknown mapping keys recursively, but never resurrect removed known
/// settings or merge arbitrary user-owned values such as LSP settings objects.
fn keep_unknown(new: &mut serde_yaml::Value, old: serde_yaml::Value, known: &serde_yaml::Value) {
    let (serde_yaml::Value::Mapping(new), serde_yaml::Value::Mapping(old)) = (new, old) else {
        return;
    };
    for (key, value) in old {
        let known_value = known.as_mapping().and_then(|map| map.get(&key));
        if let Some(new_value) = new.get_mut(&key) {
            keep_unknown(
                new_value,
                value,
                known_value.unwrap_or(&serde_yaml::Value::Null),
            );
        } else if known_value.is_none() {
            new.insert(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_provider_configuration_roundtrips_only_a_credential_reference() {
        for transport in ["open_ai_compat", "mistral_fim"] {
            let provider: ProviderConfig = serde_yaml::from_str(&format!(
                "transport: {transport}\nurl: https://example.invalid/v1\nmodel: fixture\napi_key_env: TOKEN_FIM_KEY\n"
            )).unwrap();
            assert_eq!(provider.api_key_env.as_deref(), Some("TOKEN_FIM_KEY"));
            let yaml = serde_yaml::to_string(&provider).unwrap();
            assert_eq!(
                serde_yaml::from_str::<ProviderConfig>(&yaml).unwrap(),
                provider
            );
            assert!(!yaml.contains("api_key:"));
        }
        assert!(ProviderConfig::default().api_key_env.is_none());
        assert_eq!(ProviderConfig::default().n, 1);
        let config: ProviderConfig =
            serde_yaml::from_str("transport: open_ai_compat\nmodel: fixture\nn: 3\n").unwrap();
        assert_eq!(config.n, 3);
        assert_eq!(
            serde_yaml::from_str::<ProviderConfig>(&serde_yaml::to_string(&config).unwrap())
                .unwrap(),
            config
        );
    }

    #[test]
    fn ollama_inline_config_defaults_and_round_trip() {
        let provider: ProviderConfig = serde_yaml::from_str(
            "transport: ollama\nurl: http://localhost:11434\nmodel: code-model\n",
        )
        .unwrap();
        assert_eq!(provider.transport, TransportKind::Ollama);
        assert_eq!(provider.keep_alive, -1);
        assert_eq!(provider.model.as_deref(), Some("code-model"));
        let encoded = serde_yaml::to_string(&provider).unwrap();
        assert_eq!(
            serde_yaml::from_str::<ProviderConfig>(&encoded).unwrap(),
            provider
        );
        let legacy: ProviderConfig = serde_yaml::from_str("url: http://localhost:8012\n").unwrap();
        assert_eq!(legacy.transport, TransportKind::LlamaCpp);
        assert!(legacy.model.is_none());
    }

    #[test]
    fn save_preserves_unknown_keys_and_updates_known_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let original = "theme: old\nfuture: {nested: [one, two]}\nlsp:\n  servers:\n    rust-analyzer:\n      enabled: true\n      future_option: {answer: 42}\ncompletion:\n  inline:\n    future_setting: enabled\n";
        std::fs::write(&path, original).unwrap();
        let mut config: EditorConfig = serde_yaml::from_str(original).unwrap();
        config.theme = "new".into();
        config.lsp.servers.get_mut("rust-analyzer").unwrap().enabled = Some(false);
        config.save_to(&path).unwrap();
        let saved: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let old: serde_yaml::Value = serde_yaml::from_str(original).unwrap();
        assert_eq!(saved["theme"], "new");
        assert_eq!(saved["future"], old["future"]);
        assert_eq!(
            saved["lsp"]["servers"]["rust-analyzer"]["future_option"],
            old["lsp"]["servers"]["rust-analyzer"]["future_option"]
        );
        assert_eq!(saved["lsp"]["servers"]["rust-analyzer"]["enabled"], false);
        assert_eq!(saved["completion"]["inline"]["future_setting"], "enabled");
    }

    #[test]
    fn save_does_not_restore_removed_known_options_or_map_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let original = "lsp:\n  servers:\n    rust-analyzer:\n      initialization_options: {old: true}\n      settings: {removed: true, retained: false}\n    pyright:\n      enabled: false\n";
        std::fs::write(&path, original).unwrap();
        let mut config: EditorConfig = serde_yaml::from_str(original).unwrap();
        config.lsp.servers.remove("pyright");
        let server = config.lsp.servers.get_mut("rust-analyzer").unwrap();
        server.initialization_options = None;
        server.settings = Some(serde_json::json!({"retained": true}));
        config.save_to(&path).unwrap();
        let saved: EditorConfig =
            serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!saved.lsp.servers.contains_key("pyright"));
        let server = &saved.lsp.servers["rust-analyzer"];
        assert!(server.initialization_options.is_none());
        assert_eq!(server.settings, Some(serde_json::json!({"retained": true})));
    }

    #[test]
    fn save_leaves_invalid_and_unreadable_configs_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        for original in ["theme: [", "[not, a, mapping]", "cursor_blink_ms: invalid"] {
            std::fs::write(&path, original).unwrap();
            assert!(EditorConfig::default().save_to(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
        assert!(EditorConfig::default().save_to(dir.path()).is_err());
        std::fs::write(&path, "# Empty config\n").unwrap();
        EditorConfig::default().save_to(&path).unwrap();
        assert!(
            serde_yaml::from_str::<EditorConfig>(&std::fs::read_to_string(path).unwrap()).is_ok()
        );
    }

    #[test]
    fn hover_on_mouse_defaults_to_enabled_with_a_300ms_delay() {
        let config = EditorConfig::default();
        assert!(config.hover_on_mouse);
        assert_eq!(config.hover_delay_ms, 300);
    }

    #[test]
    fn format_on_save_parses_and_defaults_off() {
        let parsed: EditorConfig = serde_yaml::from_str("format_on_save: true\n").unwrap();
        assert!(parsed.format_on_save);
        let defaulted: EditorConfig = serde_yaml::from_str("theme: default-dark\n").unwrap();
        assert!(!defaulted.format_on_save);
    }

    #[test]
    fn completion_block_parses_and_defaults() {
        let parsed: EditorConfig =
            serde_yaml::from_str("completion:\n  enabled: false\n  words: disabled\n").unwrap();
        assert!(!parsed.completion.enabled);
        assert_eq!(parsed.completion.menu.words, WordsMode::Disabled);

        let defaulted: EditorConfig = serde_yaml::from_str("theme: default-dark\n").unwrap();
        assert!(defaulted.completion.enabled);
        assert_eq!(defaulted.completion.menu.words, WordsMode::Fallback);
    }

    #[test]
    fn completion_menu_config_migrates_legacy_words_with_fieldwise_precedence() {
        let parsed: CompletionConfig = serde_yaml::from_str(
            "enabled: false\nwords: disabled\nmenu:\n  enabled: false\n  min_word_length: 5\n",
        )
        .unwrap();
        assert!(!parsed.enabled);
        assert!(!parsed.menu.enabled);
        assert_eq!(parsed.menu.min_word_length, 5);
        assert_eq!(parsed.menu.words, WordsMode::Disabled);
        let both: CompletionConfig =
            serde_yaml::from_str("words: disabled\nmenu:\n  words: enabled\n").unwrap();
        assert_eq!(both.menu.words, WordsMode::Enabled);
        assert!(both.menu.enabled);
        assert_eq!(both.menu.min_word_length, 3);
        assert_eq!(
            serde_yaml::from_str::<CompletionConfig>("{}").unwrap().menu,
            CompletionMenuConfig::default()
        );
        let json = serde_json::to_value(&parsed).unwrap();
        assert!(json.get("words").is_none());
        assert_eq!(
            serde_json::from_value::<CompletionConfig>(json)
                .unwrap()
                .menu,
            parsed.menu
        );
    }

    #[test]
    fn completion_menu_config_save_removes_legacy_key_and_preserves_unknowns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let original = "completion:\n  enabled: false\n  words: disabled\n  future: preserved\n  menu:\n    min_word_length: 5\n    future: nested\n";
        std::fs::write(&path, original).unwrap();
        let config: EditorConfig = serde_yaml::from_str(original).unwrap();
        for _ in 0..2 {
            config.save_to(&path).unwrap();
            let content = std::fs::read_to_string(&path).unwrap();
            let value: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
            assert!(value["completion"].get("words").is_none());
            assert_eq!(value["completion"]["future"], "preserved");
            assert_eq!(value["completion"]["menu"]["future"], "nested");
            let reloaded: EditorConfig = serde_yaml::from_str(&content).unwrap();
            assert!(!reloaded.completion.enabled);
            assert_eq!(reloaded.completion.menu, config.completion.menu);
        }
    }

    #[test]
    fn completion_menu_config_invalid_values_are_not_overwritten_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        for setting in ["enabled: wrong", "min_word_length: -1", "words: wrong"] {
            let original = format!("completion:\n  menu:\n    {setting}\n");
            assert!(serde_yaml::from_str::<EditorConfig>(&original).is_err());
            std::fs::write(&path, &original).unwrap();
            assert!(EditorConfig::default().save_to(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
    }

    #[test]
    fn save_to_round_trips_the_lsp_master_switch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let config = EditorConfig {
            lsp: LspConfig {
                enabled: false,
                ..LspConfig::default()
            },
            ..EditorConfig::default()
        };
        config.save_to(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let reloaded: EditorConfig = serde_yaml::from_str(&content).unwrap();
        assert!(!reloaded.lsp.enabled);
    }

    #[test]
    fn save_to_round_trips_a_per_server_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut config = EditorConfig::default();
        config.lsp.servers.insert(
            "rust-analyzer".to_owned(),
            LspServerOverride {
                enabled: Some(false),
                ..Default::default()
            },
        );
        config.save_to(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let reloaded: EditorConfig = serde_yaml::from_str(&content).unwrap();
        assert_eq!(reloaded.lsp.servers["rust-analyzer"].enabled, Some(false));
    }
}
