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
    // Phase 3 languages (priority)
    TypeScript,
    Tsx,
    Jsx,
    Json,
    Toml,
    // Phase 4 languages (common)
    Python,
    Go,
    Php,
    // Phase 5 languages (extended)
    C,
    Cpp,
    Java,
    Bash,
    // Phase 6 languages (specialized)
    Scheme,
    Ini,
    Xml,
    Sema,
    // Phase 7 languages (template)
    Blade,
    // Phase 8 languages (framework)
    Vue,
    // Phase 8 languages (build tooling)
    Just,
}

impl LanguageId {
    /// Detect language from file extension
    ///
    /// Note: `.blade.php` files are detected in `from_path()` before this is called,
    /// since `.blade.php` is a compound extension that would otherwise match `.php`.
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
            // Phase 3 (priority)
            "ts" | "mts" | "cts" => LanguageId::TypeScript,
            "tsx" => LanguageId::Tsx,
            "jsx" => LanguageId::Jsx,
            "json" | "jsonc" => LanguageId::Json,
            "toml" => LanguageId::Toml,
            // Phase 4 (common)
            "py" | "pyw" | "pyi" => LanguageId::Python,
            "go" => LanguageId::Go,
            "php" | "phtml" | "php3" | "php4" | "php5" | "phps" => LanguageId::Php,
            // Phase 5 (extended)
            "c" | "h" => LanguageId::C,
            "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" => LanguageId::Cpp,
            "java" => LanguageId::Java,
            "sh" | "bash" | "zsh" | "ksh" => LanguageId::Bash,
            "just" => LanguageId::Just,
            // Phase 6 (specialized)
            "sema" => LanguageId::Sema,
            "scm" | "rkt" | "ss" => LanguageId::Scheme,
            "ini" | "cfg" | "conf" => LanguageId::Ini,
            "xml" | "xsd" | "xsl" | "xslt" | "svg" | "plist" => LanguageId::Xml,
            // Phase 8 (framework)
            "vue" => LanguageId::Vue,
            // Default
            _ => LanguageId::PlainText,
        }
    }

    /// Detect language from file path
    pub fn from_path(path: &Path) -> Self {
        // Check for special filenames first
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // Compound extensions (must be checked before simple extension matching)
            if filename.ends_with(".blade.php") {
                return LanguageId::Blade;
            }

            if filename.eq_ignore_ascii_case("justfile")
                || filename.eq_ignore_ascii_case(".justfile")
            {
                return LanguageId::Just;
            }

            match filename {
                "Makefile" | "makefile" | "GNUmakefile" => return LanguageId::Bash,
                "Dockerfile" => return LanguageId::Bash,
                ".bashrc" | ".bash_profile" | ".zshrc" | ".profile" => return LanguageId::Bash,
                // Lock files (TOML format)
                "Cargo.lock" | "poetry.lock" | "pdm.lock" => return LanguageId::Toml,
                // Lock files (JSON format)
                "package-lock.json" | "composer.lock" | "Pipfile.lock" => return LanguageId::Json,
                // Config dotfiles (INI format)
                ".editorconfig" | ".gitconfig" | ".npmrc" | ".pylintrc" => return LanguageId::Ini,
                _ => {}
            }
        }

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
            LanguageId::TypeScript => "TypeScript",
            LanguageId::Tsx => "TSX",
            LanguageId::Jsx => "JSX",
            LanguageId::Json => "JSON",
            LanguageId::Toml => "TOML",
            LanguageId::Python => "Python",
            LanguageId::Go => "Go",
            LanguageId::Php => "PHP",
            LanguageId::C => "C",
            LanguageId::Cpp => "C++",
            LanguageId::Java => "Java",
            LanguageId::Bash => "Bash",
            LanguageId::Scheme => "Scheme",
            LanguageId::Ini => "INI",
            LanguageId::Xml => "XML",
            LanguageId::Sema => "Sema",
            LanguageId::Blade => "Blade",
            LanguageId::Vue => "Vue",
            LanguageId::Just => "Just",
        }
    }

    /// Check if this language has syntax highlighting support
    pub fn has_highlighting(&self) -> bool {
        !matches!(self, LanguageId::PlainText)
    }

    /// Check if this language supports live preview
    pub fn supports_preview(&self) -> bool {
        matches!(self, LanguageId::Markdown | LanguageId::Html)
    }

    /// Detect language from fenced code block info string (e.g., "rust", "python", "js")
    /// Used for language injection in markdown code blocks.
    pub fn from_code_fence_info(info: &str) -> Option<Self> {
        // Fenced code block info strings are typically lowercase language names
        match info.to_lowercase().as_str() {
            // Common language names
            "rust" | "rs" => Some(LanguageId::Rust),
            "python" | "py" => Some(LanguageId::Python),
            "javascript" | "js" => Some(LanguageId::JavaScript),
            "typescript" | "ts" => Some(LanguageId::TypeScript),
            "tsx" => Some(LanguageId::Tsx),
            "jsx" => Some(LanguageId::Jsx),
            "html" => Some(LanguageId::Html),
            "css" => Some(LanguageId::Css),
            "json" | "jsonc" => Some(LanguageId::Json),
            "yaml" | "yml" => Some(LanguageId::Yaml),
            "toml" => Some(LanguageId::Toml),
            "go" | "golang" => Some(LanguageId::Go),
            "java" => Some(LanguageId::Java),
            "c" => Some(LanguageId::C),
            "cpp" | "c++" | "cxx" => Some(LanguageId::Cpp),
            "php" => Some(LanguageId::Php),
            "bash" | "sh" | "shell" | "zsh" => Some(LanguageId::Bash),
            "sema" => Some(LanguageId::Sema),
            "scheme" | "scm" | "racket" | "rkt" => Some(LanguageId::Scheme),
            "xml" | "svg" => Some(LanguageId::Xml),
            "ini" | "conf" => Some(LanguageId::Ini),
            "blade" => Some(LanguageId::Blade),
            "vue" => Some(LanguageId::Vue),
            "just" | "justfile" => Some(LanguageId::Just),
            // Don't inject markdown into markdown
            "markdown" | "md" => None,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension() {
        // Phase 1
        assert_eq!(LanguageId::from_extension("yaml"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("yml"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("YML"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("md"), LanguageId::Markdown);
        assert_eq!(LanguageId::from_extension("rs"), LanguageId::Rust);
        // Phase 2
        assert_eq!(LanguageId::from_extension("html"), LanguageId::Html);
        assert_eq!(LanguageId::from_extension("css"), LanguageId::Css);
        assert_eq!(LanguageId::from_extension("js"), LanguageId::JavaScript);
        // Phase 3
        assert_eq!(LanguageId::from_extension("ts"), LanguageId::TypeScript);
        assert_eq!(LanguageId::from_extension("tsx"), LanguageId::Tsx);
        assert_eq!(LanguageId::from_extension("jsx"), LanguageId::Jsx);
        assert_eq!(LanguageId::from_extension("json"), LanguageId::Json);
        assert_eq!(LanguageId::from_extension("toml"), LanguageId::Toml);
        // Phase 4
        assert_eq!(LanguageId::from_extension("py"), LanguageId::Python);
        assert_eq!(LanguageId::from_extension("go"), LanguageId::Go);
        assert_eq!(LanguageId::from_extension("php"), LanguageId::Php);
        // Phase 5
        assert_eq!(LanguageId::from_extension("c"), LanguageId::C);
        assert_eq!(LanguageId::from_extension("cpp"), LanguageId::Cpp);
        assert_eq!(LanguageId::from_extension("java"), LanguageId::Java);
        assert_eq!(LanguageId::from_extension("sh"), LanguageId::Bash);
        assert_eq!(LanguageId::from_extension("bash"), LanguageId::Bash);
        assert_eq!(LanguageId::from_extension("just"), LanguageId::Just);
        // Phase 6
        assert_eq!(LanguageId::from_extension("scm"), LanguageId::Scheme);
        assert_eq!(LanguageId::from_extension("rkt"), LanguageId::Scheme);
        assert_eq!(LanguageId::from_extension("ss"), LanguageId::Scheme);
        assert_eq!(LanguageId::from_extension("ini"), LanguageId::Ini);
        assert_eq!(LanguageId::from_extension("cfg"), LanguageId::Ini);
        assert_eq!(LanguageId::from_extension("conf"), LanguageId::Ini);
        assert_eq!(LanguageId::from_extension("xml"), LanguageId::Xml);
        assert_eq!(LanguageId::from_extension("plist"), LanguageId::Xml);
        assert_eq!(LanguageId::from_extension("svg"), LanguageId::Xml);
        // Phase 8
        assert_eq!(LanguageId::from_extension("vue"), LanguageId::Vue);
        // Note: Blade is detected via from_path() not from_extension()
        // Unknown
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
        // Special filenames
        assert_eq!(
            LanguageId::from_path(Path::new("Makefile")),
            LanguageId::Bash
        );
        assert_eq!(
            LanguageId::from_path(Path::new(".bashrc")),
            LanguageId::Bash
        );
        assert_eq!(
            LanguageId::from_path(Path::new("justfile")),
            LanguageId::Just
        );
        assert_eq!(
            LanguageId::from_path(Path::new("Justfile")),
            LanguageId::Just
        );
        assert_eq!(
            LanguageId::from_path(Path::new(".justfile")),
            LanguageId::Just
        );
        assert_eq!(
            LanguageId::from_path(Path::new(".Justfile")),
            LanguageId::Just
        );
        assert_eq!(
            LanguageId::from_path(Path::new("build.just")),
            LanguageId::Just
        );
        // Lock files (TOML)
        assert_eq!(
            LanguageId::from_path(Path::new("Cargo.lock")),
            LanguageId::Toml
        );
        assert_eq!(
            LanguageId::from_path(Path::new("poetry.lock")),
            LanguageId::Toml
        );
        assert_eq!(
            LanguageId::from_path(Path::new("pdm.lock")),
            LanguageId::Toml
        );
        // Lock files (JSON)
        assert_eq!(
            LanguageId::from_path(Path::new("package-lock.json")),
            LanguageId::Json
        );
        assert_eq!(
            LanguageId::from_path(Path::new("composer.lock")),
            LanguageId::Json
        );
        assert_eq!(
            LanguageId::from_path(Path::new("Pipfile.lock")),
            LanguageId::Json
        );
        // Config dotfiles (INI)
        assert_eq!(
            LanguageId::from_path(Path::new(".editorconfig")),
            LanguageId::Ini
        );
        assert_eq!(
            LanguageId::from_path(Path::new(".gitconfig")),
            LanguageId::Ini
        );
        // Blade templates (compound extension)
        assert_eq!(
            LanguageId::from_path(Path::new("welcome.blade.php")),
            LanguageId::Blade
        );
        assert_eq!(
            LanguageId::from_path(Path::new("/resources/views/layout.blade.php")),
            LanguageId::Blade
        );
        // Regular PHP files should still be PHP
        assert_eq!(LanguageId::from_path(Path::new("app.php")), LanguageId::Php);
    }

    #[test]
    fn test_display_names() {
        assert_eq!(LanguageId::TypeScript.display_name(), "TypeScript");
        assert_eq!(LanguageId::Tsx.display_name(), "TSX");
        assert_eq!(LanguageId::Jsx.display_name(), "JSX");
        assert_eq!(LanguageId::Json.display_name(), "JSON");
        assert_eq!(LanguageId::Toml.display_name(), "TOML");
        assert_eq!(LanguageId::Python.display_name(), "Python");
        assert_eq!(LanguageId::Go.display_name(), "Go");
        assert_eq!(LanguageId::Php.display_name(), "PHP");
        assert_eq!(LanguageId::C.display_name(), "C");
        assert_eq!(LanguageId::Cpp.display_name(), "C++");
        assert_eq!(LanguageId::Java.display_name(), "Java");
        assert_eq!(LanguageId::Bash.display_name(), "Bash");
        assert_eq!(LanguageId::Scheme.display_name(), "Scheme");
        assert_eq!(LanguageId::Ini.display_name(), "INI");
        assert_eq!(LanguageId::Xml.display_name(), "XML");
        assert_eq!(LanguageId::Blade.display_name(), "Blade");
        assert_eq!(LanguageId::Vue.display_name(), "Vue");
        assert_eq!(LanguageId::Just.display_name(), "Just");
    }
}
