//! EditorConfig adapter: parsing/merging is pure; runtime owns discovery and reads.
//! ec4rs supplies spec glob semantics, cascades, and indentation fallbacks.

use crate::model::{DocumentId, IndentStyle, LineEnding, SaveIntent, TextPreferences};
use ec4rs::{ConfigParser, Properties, PropertiesSource};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub source: Option<(PathBuf, usize)>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedFilePolicy {
    pub path: PathBuf,
    /// Includes missing candidates, so creation and root-boundary changes reload.
    pub dependencies: Vec<PathBuf>,
    pub properties: Vec<Property>,
    pub preferences: TextPreferences,
    pub indent_uses_tab_width: bool,
    pub diagnostics: Vec<String>,
    /// False for read/parse failures; retain the last successful values on reload.
    pub incomplete: bool,
}

impl ResolvedFilePolicy {
    pub fn settings(&self, user: TextPreferences) -> crate::model::DocumentTextSettings {
        let mut settings = crate::model::DocumentTextSettings::resolve(user, self.preferences);
        if self.indent_uses_tab_width {
            settings.indent_size = settings.tabs.width();
            settings.explicit_indent = true;
        }
        settings
    }
}

#[derive(Debug, Clone)]
pub struct PolicyRequest {
    pub document_id: DocumentId,
    pub source_path: Option<PathBuf>,
    pub path: PathBuf,
    pub generation: u64,
    pub save: Option<SaveIntent>,
}

#[derive(Debug, Clone, Default)]
pub struct FilePolicyState {
    pub resolved: Option<Arc<ResolvedFilePolicy>>,
    pub pending: Option<PolicyRequest>,
    pub path: Option<PathBuf>,
    pub generation: u64,
    pub invalidated: bool,
    pub enabled: bool,
}

impl FilePolicyState {
    pub fn install(&mut self, path: PathBuf, mut policy: ResolvedFilePolicy) {
        if policy.incomplete {
            if let Some(previous) = &self.resolved {
                policy.preferences = previous.preferences;
                policy.indent_uses_tab_width = previous.indent_uses_tab_width;
                policy.properties.clone_from(&previous.properties);
                policy
                    .dependencies
                    .extend(previous.dependencies.iter().cloned());
                policy.dependencies.sort();
                policy.dependencies.dedup();
            }
        }
        self.path = Some(path);
        self.resolved = Some(Arc::new(policy));
        self.pending = None;
        self.invalidated = false;
    }
}

pub struct ConfigLayer {
    pub root: bool,
    properties: Properties,
}

/// The document path is relative to this config's directory for glob matching.
pub fn parse_layer(path: &Path, text: &str, target: &Path) -> Result<ConfigLayer, String> {
    let mut parser = ConfigParser::new_with_path(text.as_bytes(), Some(Arc::<Path>::from(path)))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let mut properties = Properties::new();
    let relative = target
        .strip_prefix(path.parent().unwrap_or(path))
        .unwrap_or(target);
    let root = parser.is_root;
    for section in &mut parser {
        section
            .map_err(|error| format!("{}: {error}", path.display()))?
            .apply_to(&mut properties, relative)
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(ConfigLayer { root, properties })
}

/// Layers arrive nearest-first. An enclosing workspace is not a stopping point.
pub fn resolve_layers(path: PathBuf, layers: Vec<ConfigLayer>) -> ResolvedFilePolicy {
    let mut properties = Properties::new();
    for layer in layers.into_iter().rev() {
        for (key, value) in layer.properties.iter() {
            properties.insert_raw_for_key(key, value.clone());
        }
    }
    normalize_properties(&mut properties);
    let original = properties.clone();
    properties.use_fallbacks();
    let mut resolved = ResolvedFilePolicy {
        path,
        ..Default::default()
    };
    resolved.properties = properties
        .iter()
        .map(|(key, value)| {
            // Fallback values inherit the property that determined them.
            let origin = value
                .source()
                .or_else(|| original.get_raw_for_key(key).source())
                .or_else(|| {
                    let key = if key == "tab_width" {
                        "indent_size"
                    } else if key == "indent_size" {
                        "tab_width"
                    } else {
                        key
                    };
                    original.get_raw_for_key(key).source()
                });
            Property {
                name: key.into(),
                value: value.to_string(),
                source: origin.map(|(path, line)| (path.into(), line)),
            }
        })
        .collect();
    let get = |name| {
        properties
            .get_raw_for_key(name)
            .filter_unset()
            .into_option()
    };
    let mut diagnostics = Vec::new();
    let mut invalid = |name: &str, value: &str| {
        diagnostics.push(format!("Invalid {name}={value}; using the user default"))
    };
    let mut width = |name| {
        get(name).and_then(|value| match value.parse::<usize>() {
            Ok(value @ 1..=256) => Some(value),
            _ => {
                invalid(name, value);
                None
            }
        })
    };
    let tab_width = width("tab_width");
    let indent_uses_tab_width = get("indent_size") == Some("tab");
    let indent_size = if indent_uses_tab_width {
        None
    } else {
        width("indent_size")
    };
    let indent_style = get("indent_style").and_then(|value| match value {
        "tab" => Some(IndentStyle::Tab),
        "space" => Some(IndentStyle::Space),
        _ => {
            invalid("indent_style", value);
            None
        }
    });
    let end_of_line = get("end_of_line").and_then(|value| match value {
        "lf" => Some(LineEnding::Lf),
        "crlf" => Some(LineEnding::Crlf),
        "cr" => Some(LineEnding::Cr),
        _ => {
            invalid("end_of_line", value);
            None
        }
    });
    let mut boolean = |name| {
        get(name).and_then(|value| match value {
            "true" => Some(true),
            "false" => Some(false),
            _ => {
                invalid(name, value);
                None
            }
        })
    };
    resolved.preferences = TextPreferences {
        indent_style,
        indent_size,
        tab_width,
        end_of_line,
        trim_trailing_whitespace: boolean("trim_trailing_whitespace"),
        insert_final_newline: boolean("insert_final_newline"),
    };
    resolved.indent_uses_tab_width = indent_uses_tab_width;
    if let Some(charset) = get("charset").filter(|value| *value != "utf-8") {
        diagnostics.push(format!("charset={charset}: encoding conversion is not supported; preserving UTF-8 and existing BOM"));
    }
    resolved.diagnostics = diagnostics;
    resolved
}

/// Standard property values are case-insensitive; arbitrary extension values are not.
pub fn normalize_properties(properties: &mut Properties) {
    for (key, value) in properties.iter_mut() {
        if matches!(
            key,
            "indent_style"
                | "indent_size"
                | "tab_width"
                | "end_of_line"
                | "charset"
                | "trim_trailing_whitespace"
                | "insert_final_newline"
        ) {
            *value = value.to_lowercase();
        }
    }
}
