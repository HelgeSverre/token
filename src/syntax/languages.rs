//! Language identification and detection
//!
//! Maps file extensions to language IDs and provides language metadata.

use std::path::Path;

/// Supported language identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LanguageId {
    #[default]
    PlainText,
    // Phase 1 languages
    Yaml,
    Markdown,
    Rust,
    // Phase 2 languages (web stack)
    Html,
    Css,
    JavaScript,
    // Future phases
    // Php,
    // TypeScript,
    // Python,
    // Go,
    // C,
    // Cpp,
    // Json,
    // Toml,
}

impl LanguageId {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // Phase 1
            "yaml" | "yml" => LanguageId::Yaml,
            "md" | "markdown" => LanguageId::Markdown,
            "rs" => LanguageId::Rust,
            // Phase 2 (web stack)
            "html" | "htm" => LanguageId::Html,
            "css" => LanguageId::Css,
            "js" | "mjs" | "cjs" => LanguageId::JavaScript,
            // Future phases will add more
            _ => LanguageId::PlainText,
        }
    }

    /// Detect language from file path
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(Self::from_extension)
            .unwrap_or(LanguageId::PlainText)
    }

    /// Get display name for the language
    pub fn display_name(&self) -> &'static str {
        match self {
            LanguageId::PlainText => "Plain Text",
            LanguageId::Yaml => "YAML",
            LanguageId::Markdown => "Markdown",
            LanguageId::Rust => "Rust",
            LanguageId::Html => "HTML",
            LanguageId::Css => "CSS",
            LanguageId::JavaScript => "JavaScript",
        }
    }

    /// Check if this language has syntax highlighting support
    pub fn has_highlighting(&self) -> bool {
        !matches!(self, LanguageId::PlainText)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension() {
        assert_eq!(LanguageId::from_extension("yaml"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("yml"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("YML"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("md"), LanguageId::Markdown);
        assert_eq!(LanguageId::from_extension("rs"), LanguageId::Rust);
        assert_eq!(LanguageId::from_extension("txt"), LanguageId::PlainText);
        assert_eq!(LanguageId::from_extension("unknown"), LanguageId::PlainText);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(
            LanguageId::from_path(Path::new("config.yaml")),
            LanguageId::Yaml
        );
        assert_eq!(
            LanguageId::from_path(Path::new("/path/to/README.md")),
            LanguageId::Markdown
        );
        assert_eq!(
            LanguageId::from_path(Path::new("main.rs")),
            LanguageId::Rust
        );
        assert_eq!(
            LanguageId::from_path(Path::new("no_extension")),
            LanguageId::PlainText
        );
    }
}
