//! Searchable settings metadata and row ordering.
pub mod descriptors;

use crate::lsp::{all_server_defs, LspServerDef};
use descriptors::DESCRIPTORS;
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Stable row identity shared by rendering and input handling.
#[derive(Debug, Clone, Copy)]
pub enum SettingsRow {
    Theme(&'static descriptors::SettingDescriptor),
    Preset(usize),
    ServerEnabled(&'static LspServerDef),
    ServerCommand(&'static LspServerDef),
    ServerStatus(&'static LspServerDef),
}

impl SettingsRow {
    pub fn section(self) -> &'static str {
        match self {
            Self::Theme(descriptor) => descriptor.section,
            Self::Preset(index) => DESCRIPTORS[index].section,
            Self::ServerEnabled(_) | Self::ServerCommand(_) | Self::ServerStatus(_) => "LSP",
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Theme(descriptor) => descriptor.name.into(),
            Self::Preset(index) => DESCRIPTORS[index].name.into(),
            Self::ServerEnabled(def) => def.id.into(),
            Self::ServerCommand(def) => format!("{} Command", def.id),
            Self::ServerStatus(def) => format!("{} Status", def.id),
        }
    }

    pub fn description(self) -> String {
        match self {
            Self::Theme(descriptor) => descriptor.description.into(),
            Self::Preset(index) => DESCRIPTORS[index].description.into(),
            Self::ServerEnabled(def) => format!("lsp.servers.{}.enabled", def.id),
            Self::ServerCommand(def) => {
                format!("Edit lsp.servers.{}.command in config.yaml", def.id)
            }
            Self::ServerStatus(_) => "Live language server status".into(),
        }
    }

    fn search_text(self) -> String {
        let keywords = match self {
            Self::Theme(descriptor) => descriptor.keywords,
            Self::Preset(index) => DESCRIPTORS[index].keywords,
            Self::ServerEnabled(_) => "language server enable disable",
            Self::ServerCommand(_) => "language server executable override path",
            Self::ServerStatus(_) => "language server lifecycle ready starting failed",
        };
        format!(
            "{} {} {} {keywords}",
            self.section(),
            self.label(),
            self.description()
        )
    }
}

/// The ordering authority for every settings consumer. Stable table order keeps
/// categories contiguous; empty categories disappear without placeholder rows.
pub fn resolve_settings_rows(query: &str) -> Vec<SettingsRow> {
    let rows = DESCRIPTORS
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            if descriptor.id == "theme" {
                SettingsRow::Theme(descriptor)
            } else {
                SettingsRow::Preset(index)
            }
        })
        .chain(all_server_defs().iter().flat_map(|&def| {
            [
                SettingsRow::ServerEnabled(def),
                SettingsRow::ServerCommand(def),
                SettingsRow::ServerStatus(def),
            ]
        }));
    let query = query.trim().to_lowercase();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut needle_buf = Vec::new();
    let needle = Utf32Str::new(&query, &mut needle_buf);
    rows.filter(|row| {
        if query.is_empty() {
            return true;
        }
        let text = row.search_text().to_lowercase();
        let mut buffer = Vec::new();
        matcher
            .fuzzy_match(Utf32Str::new(&text, &mut buffer), needle)
            .is_some()
    })
    .collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod search_tests {
    use super::*;

    #[test]
    fn search_matches_keywords_and_yaml_keys() {
        assert!(resolve_settings_rows("caret")
            .iter()
            .any(|row| matches!(row,
            SettingsRow::Preset(index) if DESCRIPTORS[*index].id == "cursor_blink_ms")));
        assert!(resolve_settings_rows("lsp.servers.pyright.command")
            .iter()
            .any(|row| matches!(row, SettingsRow::ServerCommand(def) if def.id == "pyright")));
        assert!(resolve_settings_rows("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").is_empty());
    }
}
