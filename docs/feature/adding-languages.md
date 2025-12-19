# Adding New Languages to Syntax Highlighting

**Status:** Ready for Implementation  
**Created:** 2025-12-15  
**Prerequisites:** Familiarity with tree-sitter and Scheme-like query syntax

---

## Overview

This document describes how to add support for new programming languages to the syntax highlighting system. The architecture is designed to make adding languages straightforward.

### Current Languages (20)

| Language | Tree-sitter Crate | Query Source |
|----------|-------------------|--------------|
| YAML | `tree-sitter-yaml` | Custom: `queries/yaml/highlights.scm` |
| Markdown | `tree-sitter-md` | Custom: `queries/markdown/highlights.scm` |
| Rust | `tree-sitter-rust` | Built-in: `HIGHLIGHTS_QUERY` |
| HTML | `tree-sitter-html` | Custom: `queries/html/highlights.scm` |
| CSS | `tree-sitter-css` | Custom: `queries/css/highlights.scm` |
| JavaScript | `tree-sitter-javascript` | Custom: `queries/javascript/highlights.scm` |
| TypeScript | `tree-sitter-typescript` | Custom: `queries/typescript/highlights.scm` |
| TSX | `tree-sitter-typescript` | Custom: `queries/typescript/highlights.scm` |
| JSON | `tree-sitter-json` | Custom: `queries/json/highlights.scm` |
| TOML | `tree-sitter-toml-ng` | Custom: `queries/toml/highlights.scm` |
| Python | `tree-sitter-python` | Built-in: `HIGHLIGHTS_QUERY` |
| Go | `tree-sitter-go` | Built-in: `HIGHLIGHTS_QUERY` |
| PHP | `tree-sitter-php` | Built-in: `HIGHLIGHTS_QUERY` |
| C | `tree-sitter-c` | Built-in: `HIGHLIGHT_QUERY` |
| C++ | `tree-sitter-cpp` | Built-in: `HIGHLIGHT_QUERY` |
| Java | `tree-sitter-java` | Built-in: `HIGHLIGHTS_QUERY` |
| Bash | `tree-sitter-bash` | Built-in: `HIGHLIGHT_QUERY` |
| Scheme | `tree-sitter-racket` | Built-in: `HIGHLIGHTS_QUERY` |
| INI | `tree-sitter-ini` | Built-in: `HIGHLIGHTS_QUERY` |
| XML | `tree-sitter-xml` | Built-in: `XML_HIGHLIGHT_QUERY` |

### Future Languages

**Potential additions:**
- Swift (`tree-sitter-swift`)
- Kotlin (`tree-sitter-kotlin`)
- Ruby (`tree-sitter-ruby`)
- SQL (`tree-sitter-sql`)
- Lua (`tree-sitter-lua`)

---

## Step-by-Step: Adding a New Language

### Step 1: Add the tree-sitter grammar dependency

Edit `Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...
tree-sitter-python = "0.23"  # Check crates.io for latest version
```

### Step 2: Add the LanguageId variant

Edit `src/syntax/languages.rs`:

```rust
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
    // Phase 3 languages -- ADD YOUR LANGUAGE HERE
    Python,  // NEW
}
```

Update `from_extension()`:

```rust
impl LanguageId {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // ... existing ...
            "py" | "pyw" => LanguageId::Python,  // NEW
            _ => LanguageId::PlainText,
        }
    }
}
```

Update `display_name()`:

```rust
pub fn display_name(&self) -> &'static str {
    match self {
        // ... existing ...
        LanguageId::Python => "Python",  // NEW
    }
}
```

Update `has_highlighting()` if needed (defaults to true for non-PlainText).

### Step 3: Create the highlights query file

Create `queries/python/highlights.scm`:

```scheme
; queries/python/highlights.scm
; Python syntax highlighting queries

; Keywords
[
  "and"
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "in"
  "is"
  "lambda"
  "nonlocal"
  "not"
  "or"
  "pass"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
] @keyword

; Function definitions
(function_definition
  name: (identifier) @function)

; Class definitions
(class_definition
  name: (identifier) @type)

; Function calls
(call
  function: (identifier) @function)

; Parameters
(parameters
  (identifier) @variable.parameter)

; Decorators
(decorator) @attribute

; Strings
(string) @string

; Comments
(comment) @comment

; Numbers
(integer) @number
(float) @number

; Boolean
(true) @boolean
(false) @boolean
(none) @constant.builtin

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "**"
  "//"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "="
  "+="
  "-="
  "*="
  "/="
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "."] @punctuation.delimiter
```

**Tips for writing queries:**
- Use nvim-treesitter queries as reference: https://github.com/nvim-treesitter/nvim-treesitter/tree/master/queries
- Run `tree-sitter parse <file>` to see the syntax tree structure
- Test queries with tree-sitter playground: https://tree-sitter.github.io/tree-sitter/playground

### Step 4: Register the language in ParserState

Edit `src/syntax/parser.rs`:

Add the constant for the query file:

```rust
// Embedded query files
const YAML_HIGHLIGHTS: &str = include_str!("../../queries/yaml/highlights.scm");
// ... existing ...
const PYTHON_HIGHLIGHTS: &str = include_str!("../../queries/python/highlights.scm");  // NEW
```

Update `ParserState::new()` to initialize the language:

```rust
impl ParserState {
    pub fn new() -> Self {
        let mut state = Self {
            parsers: HashMap::new(),
            queries: HashMap::new(),
            doc_cache: HashMap::new(),
        };

        // Initialize existing languages
        state.init_language(LanguageId::Yaml);
        state.init_language(LanguageId::Markdown);
        state.init_language(LanguageId::Rust);
        state.init_language(LanguageId::Html);
        state.init_language(LanguageId::Css);
        state.init_language(LanguageId::JavaScript);
        
        // Phase 3 languages -- ADD HERE
        state.init_language(LanguageId::Python);  // NEW

        state
    }
}
```

Update `init_language()` match arm:

```rust
fn init_language(&mut self, lang: LanguageId) {
    let (ts_lang, highlights_scm) = match lang {
        // ... existing ...
        LanguageId::Python => (
            tree_sitter_python::LANGUAGE.into(),
            PYTHON_HIGHLIGHTS,
        ),
        LanguageId::PlainText => return,
    };
    // ... rest of function ...
}
```

### Step 5: Add the query compilation test

Edit `src/syntax/parser.rs`, add to `query_compilation_tests` module:

```rust
#[test]
fn test_python_query_compiles() {
    assert_query_compiles(
        "Python",
        tree_sitter_python::LANGUAGE.into(),
        PYTHON_HIGHLIGHTS,
    );
}
```

Also update `test_all_query_files_compile()`:

```rust
let languages_with_queries = [
    LanguageId::Yaml,
    LanguageId::Markdown,
    LanguageId::Rust,
    LanguageId::Html,
    LanguageId::Css,
    LanguageId::JavaScript,
    LanguageId::Python,  // NEW
];
```

### Step 6: Add a parsing test

Add a test in `src/syntax/parser.rs`:

```rust
#[test]
fn test_python_parsing() {
    let mut state = ParserState::new();
    let source = r#"
def greet(name):
    """Say hello."""
    print(f"Hello, {name}!")

class Person:
    def __init__(self, name):
        self.name = name

if __name__ == "__main__":
    greet("World")
"#;
    let doc_id = DocumentId(100);
    let highlights = state.parse_and_highlight(source, LanguageId::Python, doc_id, 1);

    assert_eq!(highlights.language, LanguageId::Python);
    assert!(!highlights.lines.is_empty());
}
```

### Step 7: Run tests and verify

```bash
# Run all syntax tests
cargo test --lib syntax

# Run just the new language tests
cargo test --lib python

# Run benchmarks to verify performance
cargo bench --bench syntax -- parse_sample
```

---

## Supported Highlight Names

The editor maps tree-sitter capture names to theme colors. These are the supported captures in `HIGHLIGHT_NAMES`:

```
attribute             - @attribute (decorators, annotations)
boolean               - @boolean (true, false)
comment               - @comment
constant              - @constant
constant.builtin      - @constant.builtin (null, nil, None)
constructor           - @constructor (new Foo)
escape                - @escape (string escapes like \n)
function              - @function
function.builtin      - @function.builtin (print, len)
function.method       - @function.method
keyword               - @keyword
keyword.return        - @keyword.return
keyword.function      - @keyword.function (def, fn, function)
keyword.operator      - @keyword.operator (and, or, not)
label                 - @label (goto labels, YAML anchors)
number                - @number
operator              - @operator
property              - @property
punctuation           - @punctuation (general)
punctuation.bracket   - @punctuation.bracket
punctuation.delimiter - @punctuation.delimiter
punctuation.special   - @punctuation.special
string                - @string
string.special        - @string.special (regex, heredoc)
tag                   - @tag (HTML tags)
tag.attribute         - @tag.attribute
text                  - @text (plain text in markdown)
text.emphasis         - @text.emphasis (*italic*)
text.strong           - @text.strong (**bold**)
text.title            - @text.title (headings)
text.uri              - @text.uri (URLs)
type                  - @type
type.builtin          - @type.builtin (int, str, bool)
variable              - @variable
variable.builtin      - @variable.builtin (self, this)
variable.parameter    - @variable.parameter
```

If your query uses a capture name not in this list, it will be silently ignored. You can either:
1. Map it to an existing capture (e.g., `@keyword.control` → `@keyword`)
2. Add the new capture to `HIGHLIGHT_NAMES` in `src/syntax/highlights.rs`

---

## Query File Locations

Queries are embedded at compile time via `include_str!()`:

```
queries/
├── yaml/
│   └── highlights.scm
├── markdown/
│   └── highlights.scm
├── html/
│   └── highlights.scm
├── css/
│   └── highlights.scm
├── javascript/
│   └── highlights.scm
└── python/           # NEW
    └── highlights.scm
```

**Note:** Rust uses the built-in `tree_sitter_rust::HIGHLIGHTS_QUERY` constant instead of a separate file.

---

## Debugging Query Issues

### Query doesn't compile

Run the specific test:

```bash
cargo test --lib test_python_query_compiles
```

The test will show:
- Exact row and column of the error
- The problematic line
- Error type (invalid capture, syntax error, etc.)

### Highlights aren't appearing

1. **Check the language is detected:**
   ```rust
   let lang = LanguageId::from_extension("py");
   println!("{:?}", lang);  // Should be Python, not PlainText
   ```

2. **Check the query has captures:**
   Run the debug overlay (F8) and look at "Syntax Highlighting" section.

3. **Check capture names match:**
   Use `HIGHLIGHT_NAMES` captures only, or add new ones.

4. **Inspect the syntax tree:**
   Use `tree-sitter parse file.py` to see the actual tree structure.

### Performance issues

Run benchmarks:

```bash
cargo bench --bench syntax -- python
```

If parsing is slow:
- Reduce query complexity
- Use more specific patterns
- Consider lazy loading for rarely-used languages

---

## Resources

- [Tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Tree-sitter Query Syntax](https://tree-sitter.github.io/tree-sitter/using-parsers#pattern-matching-with-queries)
- [nvim-treesitter queries](https://github.com/nvim-treesitter/nvim-treesitter/tree/master/queries) - High-quality reference queries
- [Tree-sitter Playground](https://tree-sitter.github.io/tree-sitter/playground) - Test queries interactively
