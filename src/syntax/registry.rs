//! Authoritative compile-time registry of supported languages.
//!
//! Each inline language module owns one complete definition. The registry
//! slice only makes those definitions discoverable; it contains no behavior.

use std::borrow::Cow;

use tree_sitter::Language;

use super::parser as p;
use super::parser::{
    InjectionBehavior, ASTRO_INJECTIONS, COMPONENT_INJECTIONS, DOCKER_INJECTIONS, HTML_INJECTIONS,
    HURL_INJECTIONS, MAKE_INJECTIONS, MARKDOWN_INJECTIONS, NO_INJECTIONS, TERA_INJECTIONS,
    TYPST_INJECTIONS,
};
use super::selection::{
    SelectionProfile, CODE_SELECTION, HTML_SELECTION, INI_SELECTION, JSX_SELECTION, RUST_SELECTION,
    XML_SELECTION, YAML_SELECTION,
};
use crate::outline::extract::{
    OutlineBehavior, APPLESCRIPT_OUTLINE, BLADE_OUTLINE, COMPONENT_OUTLINE, CPP_OUTLINE,
    CSHARP_OUTLINE, C_OUTLINE, DART_RULE_OUTLINE, ELIXIR_OUTLINE, GLEAM_OUTLINE, GO_OUTLINE,
    HTML_OUTLINE, JAVASCRIPT_OUTLINE, JAVA_OUTLINE, KOTLIN_RULE_OUTLINE, LUA_OUTLINE,
    MARKDOWN_OUTLINE, NIM_RULE_OUTLINE, NO_OUTLINE, PHP_OUTLINE, PKL_RULE_OUTLINE,
    PONY_RULE_OUTLINE, PROTOBUF_RULE_OUTLINE, PYTHON_OUTLINE, RUBY_OUTLINE, RUST_OUTLINE,
    R_OUTLINE, SOLIDITY_OUTLINE, SWIFT_OUTLINE, VHDL_RULE_OUTLINE, V_RULE_OUTLINE,
    WGSL_RULE_OUTLINE, WIT_RULE_OUTLINE, YAML_OUTLINE,
};

pub(crate) type GrammarFactory = fn() -> Language;

#[derive(Clone, Copy)]
pub(crate) enum HighlightSource {
    Static(&'static str),
    Combined(&'static str, &'static str),
}

impl HighlightSource {
    pub(crate) fn query(self) -> Cow<'static, str> {
        match self {
            Self::Static(query) => Cow::Borrowed(query),
            Self::Combined(base, extension) => Cow::Owned(format!("{base}\n{extension}")),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ParserDefinition {
    pub grammar: GrammarFactory,
    pub highlights: HighlightSource,
}

pub(crate) struct LanguageDefinition {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub fence_aliases: &'static [&'static str],
    pub filenames: &'static [&'static str],
    pub compound_suffixes: &'static [&'static str],
    pub parser: Option<ParserDefinition>,
    pub selection: SelectionProfile,
    pub outline: &'static dyn OutlineBehavior,
    pub injections: &'static dyn InjectionBehavior,
}

macro_rules! parser_language_module {
    ($module:ident, $id:ident, $name:literal, $extensions:expr, $aliases:expr,
     $filenames:expr, $suffixes:expr, $grammar:expr, $highlights:expr,
     $selection:expr, $outline:expr, $injections:expr) => {
        pub(crate) mod $module {
            use super::*;

            fn grammar() -> Language {
                $grammar
            }

            pub(crate) static DEFINITION: LanguageDefinition = LanguageDefinition {
                id: LanguageId::$id,
                display_name: $name,
                extensions: &$extensions,
                fence_aliases: &$aliases,
                filenames: &$filenames,
                compound_suffixes: &$suffixes,
                parser: Some(ParserDefinition {
                    grammar,
                    highlights: $highlights,
                }),
                selection: $selection,
                outline: $outline,
                injections: $injections,
            };
        }
    };
}

macro_rules! plain_language_module {
    ($module:ident, $id:ident, $name:literal, $extensions:expr, $aliases:expr,
     $filenames:expr, $suffixes:expr, $selection:expr, $outline:expr, $injections:expr) => {
        pub(crate) mod $module {
            use super::*;

            pub(crate) static DEFINITION: LanguageDefinition = LanguageDefinition {
                id: LanguageId::$id,
                display_name: $name,
                extensions: &$extensions,
                fence_aliases: &$aliases,
                filenames: &$filenames,
                compound_suffixes: &$suffixes,
                parser: None,
                selection: $selection,
                outline: $outline,
                injections: $injections,
            };
        }
    };
}

macro_rules! language_registry {
    (
        plain_language!($plain_module:ident, $plain_id:ident, $($plain_definition:tt)*);
        $(language!($module:ident, $id:ident, $($definition:tt)*);)*
    ) => {
        /// Supported language identifiers.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub enum LanguageId {
            #[default]
            $plain_id,
            $($id),*
        }

        plain_language_module!($plain_module, $plain_id, $($plain_definition)*);
        $(parser_language_module!($module, $id, $($definition)*);)*

        pub(crate) static ALL_LANGUAGES: &[&LanguageDefinition] = &[
            &$plain_module::DEFINITION,
            $(&$module::DEFINITION),*
        ];

        pub(crate) fn language(id: LanguageId) -> &'static LanguageDefinition {
            match id {
                LanguageId::$plain_id => &$plain_module::DEFINITION,
                $(LanguageId::$id => &$module::DEFINITION),*
            }
        }
    };
}

macro_rules! static_query {
    ($query:expr) => {
        HighlightSource::Static($query)
    };
}

language_registry! {
plain_language!(
    plain_text,
    PlainText,
    "Plain Text",
    [],
    [],
    [],
    [],
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    yaml,
    Yaml,
    "YAML",
    ["yaml", "yml"],
    ["yaml", "yml"],
    [],
    [],
    tree_sitter_yaml::language(),
    static_query!(p::YAML_HIGHLIGHTS),
    YAML_SELECTION,
    YAML_OUTLINE,
    NO_INJECTIONS
);
language!(
    markdown,
    Markdown,
    "Markdown",
    ["md", "markdown"],
    [],
    [],
    [],
    tree_sitter_md::LANGUAGE.into(),
    static_query!(p::MARKDOWN_HIGHLIGHTS),
    CODE_SELECTION,
    MARKDOWN_OUTLINE,
    MARKDOWN_INJECTIONS
);
language!(
    rust,
    Rust,
    "Rust",
    ["rs"],
    ["rust", "rs"],
    [],
    [],
    tree_sitter_rust::LANGUAGE.into(),
    static_query!(p::RUST_HIGHLIGHTS),
    RUST_SELECTION,
    RUST_OUTLINE,
    NO_INJECTIONS
);
language!(
    html,
    Html,
    "HTML",
    ["html", "htm"],
    ["html"],
    [],
    [],
    tree_sitter_html::LANGUAGE.into(),
    static_query!(p::HTML_HIGHLIGHTS),
    HTML_SELECTION,
    HTML_OUTLINE,
    HTML_INJECTIONS
);
language!(
    css,
    Css,
    "CSS",
    ["css"],
    ["css"],
    [],
    [],
    tree_sitter_css::LANGUAGE.into(),
    static_query!(p::CSS_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    javascript,
    JavaScript,
    "JavaScript",
    ["js", "mjs", "cjs"],
    ["javascript", "js"],
    [],
    [],
    tree_sitter_javascript::LANGUAGE.into(),
    static_query!(p::JAVASCRIPT_HIGHLIGHTS),
    CODE_SELECTION,
    JAVASCRIPT_OUTLINE,
    NO_INJECTIONS
);
language!(
    typescript,
    TypeScript,
    "TypeScript",
    ["ts", "mts", "cts"],
    ["typescript", "ts"],
    [],
    [],
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    static_query!(p::TYPESCRIPT_HIGHLIGHTS),
    CODE_SELECTION,
    JAVASCRIPT_OUTLINE,
    NO_INJECTIONS
);
language!(
    tsx,
    Tsx,
    "TSX",
    ["tsx"],
    ["tsx"],
    [],
    [],
    tree_sitter_typescript::LANGUAGE_TSX.into(),
    static_query!(p::TSX_HIGHLIGHTS),
    JSX_SELECTION,
    JAVASCRIPT_OUTLINE,
    NO_INJECTIONS
);
language!(
    jsx,
    Jsx,
    "JSX",
    ["jsx"],
    ["jsx"],
    [],
    [],
    tree_sitter_javascript::LANGUAGE.into(),
    HighlightSource::Combined(p::JAVASCRIPT_HIGHLIGHTS, p::JSX_HIGHLIGHTS),
    JSX_SELECTION,
    JAVASCRIPT_OUTLINE,
    NO_INJECTIONS
);
language!(
    json,
    Json,
    "JSON",
    ["json", "jsonc"],
    ["json", "jsonc"],
    ["package-lock.json", "composer.lock", "Pipfile.lock"],
    [],
    tree_sitter_json::LANGUAGE.into(),
    static_query!(p::JSON_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    toml,
    Toml,
    "TOML",
    ["toml"],
    ["toml"],
    ["Cargo.lock", "poetry.lock", "pdm.lock"],
    [],
    tree_sitter_toml_ng::LANGUAGE.into(),
    static_query!(p::TOML_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    python,
    Python,
    "Python",
    ["py", "pyw", "pyi"],
    ["python", "py"],
    [],
    [],
    tree_sitter_python::LANGUAGE.into(),
    static_query!(p::PYTHON_HIGHLIGHTS),
    CODE_SELECTION,
    PYTHON_OUTLINE,
    NO_INJECTIONS
);
language!(
    go,
    Go,
    "Go",
    ["go"],
    ["go", "golang"],
    [],
    [],
    tree_sitter_go::LANGUAGE.into(),
    static_query!(p::GO_HIGHLIGHTS),
    CODE_SELECTION,
    GO_OUTLINE,
    NO_INJECTIONS
);
language!(
    php,
    Php,
    "PHP",
    ["php", "phtml", "php3", "php4", "php5", "phps"],
    ["php"],
    [],
    [],
    tree_sitter_php::LANGUAGE_PHP.into(),
    static_query!(p::PHP_HIGHLIGHTS),
    CODE_SELECTION,
    PHP_OUTLINE,
    NO_INJECTIONS
);
language!(
    c,
    C,
    "C",
    ["c", "h"],
    ["c"],
    [],
    [],
    tree_sitter_c::LANGUAGE.into(),
    static_query!(p::C_HIGHLIGHTS),
    CODE_SELECTION,
    C_OUTLINE,
    NO_INJECTIONS
);
language!(
    cpp,
    Cpp,
    "C++",
    ["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++"],
    ["cpp", "c++", "cxx"],
    [],
    [],
    tree_sitter_cpp::LANGUAGE.into(),
    static_query!(p::CPP_HIGHLIGHTS),
    CODE_SELECTION,
    CPP_OUTLINE,
    NO_INJECTIONS
);
language!(
    java,
    Java,
    "Java",
    ["java"],
    ["java"],
    [],
    [],
    tree_sitter_java::LANGUAGE.into(),
    static_query!(p::JAVA_HIGHLIGHTS),
    CODE_SELECTION,
    JAVA_OUTLINE,
    NO_INJECTIONS
);
language!(
    bash,
    Bash,
    "Bash",
    ["sh", "bash", "zsh", "ksh"],
    ["bash", "sh", "shell", "zsh"],
    [".bashrc", ".bash_profile", ".zshrc", ".profile"],
    [],
    tree_sitter_bash::LANGUAGE.into(),
    static_query!(p::BASH_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    scheme,
    Scheme,
    "Scheme",
    ["scm", "rkt", "ss"],
    ["scheme", "scm", "racket", "rkt"],
    [],
    [],
    tree_sitter_racket::LANGUAGE.into(),
    static_query!(p::SCHEME_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    ini,
    Ini,
    "INI",
    ["ini", "cfg", "conf"],
    ["ini", "conf"],
    [".editorconfig", ".gitconfig", ".npmrc", ".pylintrc"],
    [],
    tree_sitter_ini::LANGUAGE.into(),
    static_query!(p::INI_HIGHLIGHTS),
    INI_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    xml,
    Xml,
    "XML",
    ["xml", "xsd", "xsl", "xslt", "svg", "plist"],
    ["xml", "svg"],
    [],
    [],
    tree_sitter_xml::LANGUAGE_XML.into(),
    static_query!(p::XML_HIGHLIGHTS),
    XML_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    sema,
    Sema,
    "Sema",
    ["sema"],
    ["sema"],
    [],
    [],
    tree_sitter_sema::LANGUAGE.into(),
    static_query!(p::SEMA_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    blade,
    Blade,
    "Blade",
    [],
    ["blade"],
    [],
    [".blade.php"],
    tree_sitter_blade::LANGUAGE.into(),
    static_query!(p::BLADE_HIGHLIGHTS),
    CODE_SELECTION,
    BLADE_OUTLINE,
    NO_INJECTIONS
);
language!(
    vue,
    Vue,
    "Vue",
    ["vue"],
    ["vue"],
    [],
    [],
    tree_sitter_html::LANGUAGE.into(),
    static_query!(p::HTML_HIGHLIGHTS),
    HTML_SELECTION,
    COMPONENT_OUTLINE,
    COMPONENT_INJECTIONS
);
language!(
    svelte,
    Svelte,
    "Svelte",
    ["svelte"],
    ["svelte"],
    [],
    [],
    tree_sitter_svelte_ng::LANGUAGE.into(),
    HighlightSource::Combined(p::HTML_HIGHLIGHTS, p::SVELTE_HIGHLIGHTS),
    HTML_SELECTION,
    COMPONENT_OUTLINE,
    COMPONENT_INJECTIONS
);
language!(
    just,
    Just,
    "Just",
    ["just"],
    ["just", "justfile"],
    ["justfile", ".justfile"],
    [],
    tree_sitter_just::LANGUAGE.into(),
    static_query!(p::JUST_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    applescript,
    AppleScript,
    "AppleScript",
    ["applescript"],
    ["applescript"],
    [],
    [],
    tree_sitter_applescript::language(),
    static_query!(p::APPLESCRIPT_HIGHLIGHTS),
    CODE_SELECTION,
    APPLESCRIPT_OUTLINE,
    NO_INJECTIONS
);
language!(
    csharp,
    CSharp,
    "C#",
    ["cs", "csx"],
    ["csharp", "cs", "c#"],
    [],
    [],
    tree_sitter_c_sharp::LANGUAGE.into(),
    static_query!(p::CSHARP_HIGHLIGHTS),
    CODE_SELECTION,
    CSHARP_OUTLINE,
    NO_INJECTIONS
);
language!(
    ruby,
    Ruby,
    "Ruby",
    ["rb", "rake", "gemspec"],
    ["ruby", "rb"],
    ["Gemfile", "Rakefile"],
    [],
    tree_sitter_ruby::LANGUAGE.into(),
    static_query!(p::RUBY_HIGHLIGHTS),
    CODE_SELECTION,
    RUBY_OUTLINE,
    NO_INJECTIONS
);
language!(
    lua,
    Lua,
    "Lua",
    ["lua"],
    ["lua"],
    [],
    [],
    tree_sitter_lua::LANGUAGE.into(),
    static_query!(p::LUA_HIGHLIGHTS),
    CODE_SELECTION,
    LUA_OUTLINE,
    NO_INJECTIONS
);
language!(
    r,
    R,
    "R",
    ["r"],
    ["r"],
    [".Rprofile"],
    [],
    tree_sitter_r::LANGUAGE.into(),
    static_query!(p::R_HIGHLIGHTS),
    CODE_SELECTION,
    R_OUTLINE,
    NO_INJECTIONS
);
language!(
    swift,
    Swift,
    "Swift",
    ["swift"],
    ["swift"],
    [],
    [],
    tree_sitter_swift::LANGUAGE.into(),
    static_query!(p::SWIFT_HIGHLIGHTS),
    CODE_SELECTION,
    SWIFT_OUTLINE,
    NO_INJECTIONS
);
language!(
    elixir,
    Elixir,
    "Elixir",
    ["ex", "exs"],
    ["elixir", "ex"],
    [],
    [],
    tree_sitter_elixir::LANGUAGE.into(),
    static_query!(p::ELIXIR_HIGHLIGHTS),
    CODE_SELECTION,
    ELIXIR_OUTLINE,
    NO_INJECTIONS
);
language!(
    gleam,
    Gleam,
    "Gleam",
    ["gleam"],
    ["gleam"],
    [],
    [],
    tree_sitter_gleam::LANGUAGE.into(),
    static_query!(p::GLEAM_HIGHLIGHTS),
    CODE_SELECTION,
    GLEAM_OUTLINE,
    NO_INJECTIONS
);
language!(
    solidity,
    Solidity,
    "Solidity",
    ["sol"],
    ["solidity", "sol"],
    [],
    [],
    tree_sitter_solidity::LANGUAGE.into(),
    static_query!(p::SOLIDITY_HIGHLIGHTS),
    CODE_SELECTION,
    SOLIDITY_OUTLINE,
    NO_INJECTIONS
);
language!(
    kotlin,
    Kotlin,
    "Kotlin",
    ["kt", "kts"],
    ["kotlin", "kt"],
    [],
    [],
    tree_sitter_kotlin_ng::LANGUAGE.into(),
    static_query!(p::KOTLIN_HIGHLIGHTS),
    CODE_SELECTION,
    &KOTLIN_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    dart,
    Dart,
    "Dart",
    ["dart"],
    ["dart"],
    [],
    [],
    tree_sitter_dart::LANGUAGE.into(),
    static_query!(p::DART_HIGHLIGHTS),
    CODE_SELECTION,
    &DART_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    julia,
    Julia,
    "Julia",
    ["jl"],
    ["julia", "jl"],
    [],
    [],
    tree_sitter_julia::LANGUAGE.into(),
    static_query!(p::JULIA_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    haskell,
    Haskell,
    "Haskell",
    ["hs"],
    ["haskell", "hs"],
    [],
    [],
    tree_sitter_haskell::LANGUAGE.into(),
    static_query!(p::HASKELL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    ocaml,
    Ocaml,
    "OCaml",
    ["ml"],
    ["ocaml", "ml"],
    [],
    [],
    tree_sitter_ocaml::LANGUAGE_OCAML.into(),
    static_query!(p::OCAML_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    ocaml_interface,
    OcamlInterface,
    "OCaml Interface",
    ["mli"],
    ["ocaml-interface", "mli"],
    [],
    [],
    tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into(),
    static_query!(p::OCAML_INTERFACE_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    d,
    D,
    "D",
    ["d", "di"],
    ["dlang"],
    [],
    [],
    tree_sitter_d::LANGUAGE.into(),
    static_query!(p::D_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    objective_c,
    ObjectiveC,
    "Objective-C",
    ["m", "mm"],
    ["objective-c", "objc"],
    [],
    [],
    tree_sitter_objc::LANGUAGE.into(),
    static_query!(p::OBJC_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    vhdl,
    Vhdl,
    "VHDL",
    ["vhd", "vhdl"],
    ["vhdl"],
    [],
    [],
    tree_sitter_vhdl::LANGUAGE.into(),
    static_query!(p::VHDL_HIGHLIGHTS),
    CODE_SELECTION,
    &VHDL_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    odin,
    Odin,
    "Odin",
    ["odin"],
    ["odin"],
    [],
    [],
    tree_sitter_odin::LANGUAGE.into(),
    static_query!(p::ODIN_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    fish,
    Fish,
    "Fish",
    ["fish"],
    ["fish"],
    [],
    [],
    tree_sitter_fish::language(),
    static_query!(p::FISH_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    assembly,
    Assembly,
    "Assembly",
    ["asm", "s", "assembly"],
    ["asm", "assembly"],
    [],
    [],
    tree_sitter_asm::LANGUAGE.into(),
    static_query!(p::ASM_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    scss,
    Scss,
    "SCSS",
    ["scss"],
    ["scss"],
    [],
    [],
    tree_sitter_scss::language(),
    static_query!(p::SCSS_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    cmake,
    Cmake,
    "CMake",
    ["cmake"],
    ["cmake"],
    ["CMakeLists.txt"],
    [],
    tree_sitter_cmake::LANGUAGE.into(),
    static_query!(p::CMAKE_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    make,
    Make,
    "Make",
    ["mk", "mak"],
    ["make", "makefile"],
    ["Makefile", "GNUmakefile"],
    [],
    tree_sitter_make::LANGUAGE.into(),
    static_query!(p::MAKE_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    MAKE_INJECTIONS
);
language!(
    common_lisp,
    CommonLisp,
    "Common Lisp",
    ["lisp", "cl"],
    ["common-lisp", "commonlisp"],
    [],
    [],
    tree_sitter_commonlisp::LANGUAGE_COMMONLISP.into(),
    static_query!(p::COMMONLISP_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    zig,
    Zig,
    "Zig",
    ["zig"],
    ["zig"],
    [],
    [],
    tree_sitter_zig::LANGUAGE.into(),
    static_query!(p::ZIG_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    glsl,
    Glsl,
    "GLSL",
    ["glsl", "vert", "frag", "fsh", "geom", "tesc", "tese", "comp"],
    ["glsl"],
    [],
    [],
    tree_sitter_glsl::LANGUAGE_GLSL.into(),
    static_query!(p::GLSL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    graphql,
    Graphql,
    "GraphQL",
    ["graphql", "gql"],
    ["graphql", "gql"],
    [],
    [],
    tree_sitter_graphql::LANGUAGE.into(),
    static_query!(p::GRAPHQL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    hcl,
    Hcl,
    "HCL",
    ["hcl", "tf", "tfvars"],
    ["hcl", "terraform", "tf"],
    [],
    [],
    tree_sitter_hcl::LANGUAGE.into(),
    static_query!(p::HCL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    nix,
    Nix,
    "Nix",
    ["nix"],
    ["nix"],
    [],
    [],
    tree_sitter_nix::LANGUAGE.into(),
    static_query!(p::NIX_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    ada,
    Ada,
    "Ada",
    ["adb", "ads", "ada"],
    ["ada"],
    [],
    [],
    tree_sitter_ada::LANGUAGE.into(),
    static_query!(p::ADA_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    erlang,
    Erlang,
    "Erlang",
    ["erl", "hrl", "escript"],
    ["erlang", "erl"],
    [],
    [],
    tree_sitter_erlang::LANGUAGE.into(),
    static_query!(p::ERLANG_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    clojure,
    Clojure,
    "Clojure",
    ["clj", "cljs", "cljc", "edn"],
    ["clojure", "clj", "cljs", "edn"],
    [],
    [],
    tree_sitter_clojure::LANGUAGE.into(),
    static_query!(p::CLOJURE_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    nushell,
    Nushell,
    "Nushell",
    ["nu"],
    ["nu", "nushell"],
    [],
    [],
    tree_sitter_nu::LANGUAGE.into(),
    static_query!(p::NU_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    sed,
    Sed,
    "sed",
    ["sed"],
    ["sed"],
    [],
    [],
    tree_sitter_sed::LANGUAGE.into(),
    static_query!(p::SED_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    tcl,
    Tcl,
    "Tcl",
    ["tcl", "tk", "tm"],
    ["tcl", "tk"],
    [],
    [],
    tree_sitter_tcl::LANGUAGE.into(),
    static_query!(p::TCL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    roc,
    Roc,
    "Roc",
    ["roc"],
    ["roc"],
    [],
    [],
    tree_sitter_roc::LANGUAGE.into(),
    static_query!(p::ROC_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    janet,
    Janet,
    "Janet",
    ["janet"],
    ["janet"],
    [],
    [],
    tree_sitter_janet::language(),
    static_query!(p::JANET_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    forth,
    Forth,
    "Forth",
    ["forth", "fth", "4th"],
    ["forth"],
    [],
    [],
    tree_sitter_forth::LANGUAGE.into(),
    static_query!(p::FORTH_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    protobuf,
    Protobuf,
    "Protocol Buffers",
    ["proto"],
    ["protobuf", "proto"],
    [],
    [],
    tree_sitter_proto::LANGUAGE.into(),
    static_query!(p::PROTO_HIGHLIGHTS),
    CODE_SELECTION,
    &PROTOBUF_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    dhall,
    Dhall,
    "Dhall",
    ["dhall"],
    ["dhall"],
    [],
    [],
    tree_sitter_dhall::LANGUAGE.into(),
    static_query!(p::DHALL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    pkl,
    Pkl,
    "Pkl",
    ["pkl"],
    ["pkl"],
    [],
    [],
    tree_sitter_pkl::LANGUAGE.into(),
    static_query!(p::PKL_HIGHLIGHTS),
    CODE_SELECTION,
    &PKL_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    hurl,
    Hurl,
    "Hurl",
    ["hurl"],
    ["hurl"],
    [],
    [],
    tree_sitter_hurl::LANGUAGE.into(),
    static_query!(p::HURL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    HURL_INJECTIONS
);
language!(
    wit,
    Wit,
    "WIT",
    ["wit"],
    ["wit"],
    [],
    [],
    tree_sitter_wit::LANGUAGE.into(),
    static_query!(p::WIT_HIGHLIGHTS),
    CODE_SELECTION,
    &WIT_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    standard_ml,
    StandardMl,
    "Standard ML",
    ["sml", "sig", "fun", "mlb"],
    ["sml", "standard-ml"],
    [],
    [],
    tree_sitter_sml::LANGUAGE.into(),
    static_query!(p::SML_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    nim,
    Nim,
    "Nim",
    ["nim", "nims", "nimble"],
    ["nim"],
    [],
    [],
    tree_sitter_nim::language(),
    static_query!(p::NIM_HIGHLIGHTS),
    CODE_SELECTION,
    &NIM_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    astro,
    Astro,
    "Astro",
    ["astro"],
    ["astro"],
    [],
    [],
    tree_sitter_astro_next::LANGUAGE.into(),
    static_query!(p::ASTRO_HIGHLIGHTS),
    HTML_SELECTION,
    NO_OUTLINE,
    ASTRO_INJECTIONS
);
language!(
    awk,
    Awk,
    "AWK",
    ["awk"],
    ["awk"],
    [],
    [],
    arborium_awk::language().into(),
    static_query!(p::AWK_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    kdl,
    Kdl,
    "KDL",
    ["kdl"],
    ["kdl"],
    [],
    [],
    arborium_kdl::language().into(),
    static_query!(p::KDL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    tera,
    Tera,
    "Tera",
    ["tera"],
    ["tera"],
    [],
    [],
    tree_sitter_tera::LANGUAGE.into(),
    static_query!(p::TERA_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    TERA_INJECTIONS
);
language!(
    typst,
    Typst,
    "Typst",
    ["typ"],
    ["typst"],
    [],
    [],
    codebook_tree_sitter_typst::LANGUAGE.into(),
    static_query!(p::TYPST_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    TYPST_INJECTIONS
);
language!(
    dockerfile,
    Dockerfile,
    "Dockerfile",
    ["dockerfile", "containerfile"],
    ["dockerfile", "containerfile"],
    ["Dockerfile", "Containerfile"],
    [],
    tree_sitter_containerfile::LANGUAGE.into(),
    static_query!(p::DOCKERFILE_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    DOCKER_INJECTIONS
);
language!(
    wgsl,
    Wgsl,
    "WGSL",
    ["wgsl"],
    ["wgsl"],
    [],
    [],
    tree_sitter_wgsl_bevy::LANGUAGE.into(),
    static_query!(p::WGSL_HIGHLIGHTS),
    CODE_SELECTION,
    &WGSL_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    sql,
    Sql,
    "SQL",
    ["sql"],
    ["sql"],
    [],
    [],
    tree_sitter_sequel::LANGUAGE.into(),
    static_query!(p::SQL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    v,
    V,
    "V",
    ["v", "vsh"],
    ["vlang"],
    [],
    [],
    tree_sitter_v::LANGUAGE.into(),
    static_query!(p::V_HIGHLIGHTS),
    CODE_SELECTION,
    &V_RULE_OUTLINE,
    NO_INJECTIONS
);
language!(
    cue,
    Cue,
    "CUE",
    ["cue"],
    ["cue"],
    [],
    [],
    crate::syntax::compat::cue_language(),
    static_query!(p::CUE_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    fennel,
    Fennel,
    "Fennel",
    ["fnl"],
    ["fennel", "fnl"],
    [],
    [],
    crate::syntax::compat::fennel_language(),
    static_query!(p::FENNEL_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    pest,
    Pest,
    "Pest",
    ["pest"],
    ["pest"],
    [],
    [],
    crate::syntax::compat::pest_language(),
    static_query!(p::PEST_HIGHLIGHTS),
    CODE_SELECTION,
    NO_OUTLINE,
    NO_INJECTIONS
);
language!(
    pony,
    Pony,
    "Pony",
    ["pony"],
    ["pony"],
    [],
    [],
    crate::syntax::compat::pony_language(),
    static_query!(p::PONY_HIGHLIGHTS),
    CODE_SELECTION,
    &PONY_RULE_OUTLINE,
    NO_INJECTIONS
);

}

pub(crate) fn from_extension(extension: &str) -> Option<&'static LanguageDefinition> {
    ALL_LANGUAGES.iter().copied().find(|definition| {
        definition
            .extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    })
}

pub(crate) fn from_filename(filename: &str) -> Option<&'static LanguageDefinition> {
    ALL_LANGUAGES.iter().copied().find(|definition| {
        definition
            .filenames
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(filename))
            || definition.compound_suffixes.iter().any(|suffix| {
                filename
                    .to_ascii_lowercase()
                    .ends_with(&suffix.to_ascii_lowercase())
            })
    })
}

pub(crate) fn from_fence(info: &str) -> Option<&'static LanguageDefinition> {
    ALL_LANGUAGES.iter().copied().find(|definition| {
        definition
            .fence_aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(info))
    })
}
