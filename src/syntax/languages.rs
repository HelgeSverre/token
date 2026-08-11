//! Language identification and detection
//!
//! Maps file extensions to language IDs and provides language metadata.

use std::path::Path;

/// Supported language identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LanguageId {
    #[default]
    PlainText,
    Yaml,
    Markdown,
    Rust,
    Html,
    Css,
    JavaScript,
    TypeScript,
    Tsx,
    Jsx,
    Json,
    Toml,
    Python,
    Go,
    Php,
    C,
    Cpp,
    Java,
    Bash,
    Scheme,
    Ini,
    Xml,
    Sema,
    Blade,
    Vue,
    Svelte,
    Just,
    AppleScript,
    CSharp,
    Ruby,
    Lua,
    R,
    Swift,
    Elixir,
    Gleam,
    Solidity,
    Kotlin,
    Dart,
    Julia,
    Haskell,
    Ocaml,
    OcamlInterface,
    D,
    ObjectiveC,
    Vhdl,
    Odin,
    Fish,
    Assembly,
    Scss,
    Cmake,
    Make,
    CommonLisp,
    Zig,
    Glsl,
    Graphql,
    Hcl,
    Nix,
    Ada,
    Erlang,
    Clojure,
    Nushell,
    Sed,
    Tcl,
    Roc,
    Janet,
    Forth,
    Protobuf,
    Dhall,
    Pkl,
    Hurl,
    Wit,
    StandardMl,
    Nim,
    Astro,
    Awk,
    Kdl,
    Tera,
    Typst,
    Dockerfile,
    Wgsl,
    Sql,
    V,
    Cue,
    Fennel,
    Pest,
    Pony,
}

impl LanguageId {
    /// Detect language from file extension
    ///
    /// Note: `.blade.php` files are detected in `from_path()` before this is called,
    /// since `.blade.php` is a compound extension that would otherwise match `.php`.
    pub fn from_extension(ext: &str) -> Self {
        super::registry::from_extension(ext).map_or(LanguageId::PlainText, |language| language.id)
    }

    /// Detect language from file path
    pub fn from_path(path: &Path) -> Self {
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(language) = super::registry::from_filename(filename) {
                return language.id;
            }
        }

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(Self::from_extension)
            .unwrap_or(LanguageId::PlainText)
    }

    /// Get display name for the language
    pub fn display_name(&self) -> &'static str {
        super::registry::language(*self).display_name
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
        super::registry::from_fence(info).map(|language| language.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_unique_ids_extensions_and_fence_aliases() {
        let mut ids = HashSet::new();
        let mut extensions = HashSet::new();
        let mut aliases = HashSet::new();

        for definition in crate::syntax::registry::ALL_LANGUAGES {
            assert!(
                ids.insert(definition.id),
                "duplicate id: {:?}",
                definition.id
            );
            for extension in definition.extensions {
                assert!(
                    extensions.insert(extension.to_ascii_lowercase()),
                    "duplicate extension: {extension}"
                );
            }
            for alias in definition.fence_aliases {
                assert!(
                    aliases.insert(alias.to_ascii_lowercase()),
                    "duplicate fence alias: {alias}"
                );
            }
        }
    }

    #[test]
    fn test_from_extension() {
        assert_eq!(LanguageId::from_extension("yaml"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("yml"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("YML"), LanguageId::Yaml);
        assert_eq!(LanguageId::from_extension("md"), LanguageId::Markdown);
        assert_eq!(LanguageId::from_extension("rs"), LanguageId::Rust);
        assert_eq!(LanguageId::from_extension("html"), LanguageId::Html);
        assert_eq!(LanguageId::from_extension("css"), LanguageId::Css);
        assert_eq!(LanguageId::from_extension("js"), LanguageId::JavaScript);
        assert_eq!(LanguageId::from_extension("ts"), LanguageId::TypeScript);
        assert_eq!(LanguageId::from_extension("tsx"), LanguageId::Tsx);
        assert_eq!(LanguageId::from_extension("jsx"), LanguageId::Jsx);
        assert_eq!(LanguageId::from_extension("json"), LanguageId::Json);
        assert_eq!(LanguageId::from_extension("toml"), LanguageId::Toml);
        assert_eq!(LanguageId::from_extension("py"), LanguageId::Python);
        assert_eq!(LanguageId::from_extension("go"), LanguageId::Go);
        assert_eq!(LanguageId::from_extension("php"), LanguageId::Php);
        assert_eq!(LanguageId::from_extension("c"), LanguageId::C);
        assert_eq!(LanguageId::from_extension("cpp"), LanguageId::Cpp);
        assert_eq!(LanguageId::from_extension("java"), LanguageId::Java);
        assert_eq!(LanguageId::from_extension("sh"), LanguageId::Bash);
        assert_eq!(LanguageId::from_extension("bash"), LanguageId::Bash);
        assert_eq!(LanguageId::from_extension("just"), LanguageId::Just);
        assert_eq!(
            LanguageId::from_extension("applescript"),
            LanguageId::AppleScript
        );
        assert_eq!(LanguageId::from_extension("scm"), LanguageId::Scheme);
        assert_eq!(LanguageId::from_extension("rkt"), LanguageId::Scheme);
        assert_eq!(LanguageId::from_extension("ss"), LanguageId::Scheme);
        assert_eq!(LanguageId::from_extension("ini"), LanguageId::Ini);
        assert_eq!(LanguageId::from_extension("cfg"), LanguageId::Ini);
        assert_eq!(LanguageId::from_extension("conf"), LanguageId::Ini);
        assert_eq!(LanguageId::from_extension("xml"), LanguageId::Xml);
        assert_eq!(LanguageId::from_extension("plist"), LanguageId::Xml);
        assert_eq!(LanguageId::from_extension("svg"), LanguageId::Xml);
        assert_eq!(LanguageId::from_extension("vue"), LanguageId::Vue);
        assert_eq!(LanguageId::from_extension("svelte"), LanguageId::Svelte);
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
            LanguageId::Make
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
        assert_eq!(LanguageId::AppleScript.display_name(), "AppleScript");
    }

    #[test]
    fn every_syntax_sample_has_a_registered_language() {
        let sample_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/syntax");
        let mut unsupported = std::fs::read_dir(&sample_dir)
            .expect("syntax sample directory should exist")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| LanguageId::from_path(path) == LanguageId::PlainText)
            .filter_map(|path| path.file_name().map(|name| name.to_owned()))
            .collect::<Vec<_>>();
        unsupported.sort();

        assert!(
            unsupported.is_empty(),
            "syntax samples without a registered language: {unsupported:?}"
        );
    }
}
