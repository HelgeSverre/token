# LSP Servers and Formatters Overview

This document provides a comprehensive overview of popular Language Server Protocol (LSP) servers and code formatters across various programming languages.

## Language Server Protocol (LSP) Servers

### JavaScript/TypeScript
- **TypeScript Language Server** - Community LSP wrapper around Microsoft's tsserver
  - Repository: https://github.com/typescript-language-server/typescript-language-server
  - Features: Full TypeScript/JavaScript intelligence, completion, hover, go-to-definition
  - Extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`

- **Biome** - Fast JavaScript/TypeScript toolchain (linter, formatter, bundler)
  - Provides LSP diagnostics for TypeScript/JavaScript
  - Extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.json`, `.jsonc`

### Python
- **Pyright** - Microsoft's static type checker for Python
  - Repository: https://github.com/microsoft/pyright
  - Features: Type checking, completion, hover, go-to-definition, find references
  - Extensions: `.py`, `.pyi`

- **Python LSP Server (pylsp)** - Python LSP implementation by Spyder IDE team
  - Features: IDE integration, plugin system, code completion, linting
  - Extensions: `.py`

- **BasedPyright** - Fork of Pyright with additional features
  - Enhanced version of Pyright with extra capabilities

### Go
- **gopls** - Official Go language server
  - Repository: https://github.com/golang/tools/tree/master/gopls
  - Features: Completion, hover, go-to-definition, find references, diagnostics
  - Extensions: `.go`, `.mod`, `.sum`

### Rust
- **rust-analyzer** - Official Rust language server
  - Features: Completion, hover, go-to-definition, find references, diagnostics, macro expansion
  - Extensions: `.rs`
  - Project markers: `Cargo.toml`, `rust-analyzer.toml`

### Java
- **Eclipse JDT LS** - Eclipse Java development tools language server
  - Features: Full Java IDE capabilities, refactoring, debugging
  - Extensions: `.java`

- **Java Language Server** by Red Hat
  - Alternative Java LSP implementation

### C/C++
- **clangd** - LLVM-based C/C++ language server
  - Features: Completion, diagnostics, go-to-definition, cross-references
  - Extensions: `.c`, `.cpp`, `.h`, `.hpp`

- **ccls** - C/C++/Objective-C language server (older, less active)
  - Repository: https://github.com/MaskRay/ccls

### C#
- **OmniSharp** - C# language server
  - Repository: https://github.com/OmniSharp/csharp-language-server-protocol
  - Features: Completion, diagnostics, refactoring, debugging
  - Extensions: `.cs`

### Ruby
- **Solargraph** - Ruby language server
  - Features: Completion, hover, go-to-definition, diagnostics
  - Extensions: `.rb`

- **Ruby LSP** by Shopify
  - Modern Ruby language server with enhanced features

### PHP
- **Intelephense** - Commercial PHP language server
  - Features: Advanced PHP intelligence, completion, refactoring
  - Extensions: `.php`

- **phpactor** - Open source PHP language server
  - Repository: https://github.com/phpactor/phpactor
  - Features: Completion, refactoring, code transformation

### Swift
- **SourceKit-LSP** - Official Swift language server by Apple
  - Features: Completion, diagnostics, cross-language support (Swift/C/C++)
  - Extensions: `.swift`

### Kotlin
- **Kotlin Language Server** - Official Kotlin LSP
  - Repository: https://github.com/fwcd/kotlin-language-server
  - Features: Completion, hover, go-to-definition
  - Extensions: `.kt`, `.kts`

### Dart
- **Dart Analysis Server** - Official Dart language server
  - Command: `dart language-server`
  - Features: Completion, diagnostics, code actions, refactoring
  - Extensions: `.dart`

### Elixir
- **elixir-ls** - Elixir language server
  - Repository: https://github.com/elixir-lsp/elixir-ls
  - Features: Completion, hover, go-to-definition, Dialyzer analysis, debugging
  - Extensions: `.ex`, `.exs`

### Clojure
- **clojure-lsp** - Clojure language server
  - Features: Completion, hover, go-to-definition, find references, code actions
  - Extensions: `.clj`, `.cljs`, `.cljc`

### Haskell
- **haskell-language-server** - Haskell language server
  - Repository: https://github.com/haskell/haskell-language-server
  - Features: Completion, hover, go-to-definition, type information
  - Extensions: `.hs`, `.lhs`

### Scala
- **Metals** - Scala language server
  - Features: Completion, hover, go-to-definition, build server integration
  - Extensions: `.scala`, `.sbt`

### Lua
- **lua-language-server** (sumneko)
  - Repository: https://github.com/sumneko/lua-language-server
  - Features: Completion, hover, go-to-definition, diagnostics
  - Extensions: `.lua`

### Elm
- **elm-language-server** - Elm language server
  - Repository: https://github.com/elm-tooling/elm-language-server
  - Features: Completion, diagnostics, elm-analyzer integration
  - Extensions: `.elm`

### Shell/Bash
- **bash-language-server** - Bash language server
  - Repository: https://github.com/bash-lsp/bash-language-server
  - Features: Completion, hover, diagnostics
  - Extensions: `.sh`, `.bash`

### Terraform
- **terraform-ls** - Terraform language server by HashiCorp
  - Features: Completion, hover, diagnostics, module navigation
  - Extensions: `.tf`, `.tfvars`

### Nix
- **nil** - Nix language server
  - Repository: https://github.com/oxalica/nil
  - Features: Completion, hover, diagnostics
  - Extensions: `.nix`

### OCaml
- **ocaml-lsp** - OCaml language server
  - Repository: https://github.com/ocaml/ocaml-lsp
  - Features: Completion, hover, go-to-definition
  - Extensions: `.ml`, `.mli`

### Other Notable LSP Servers
- **CSS/SCSS/Less** - VSCode CSS Language Service
- **HTML** - VSCode HTML Language Service
- **JSON** - VSCode JSON Language Service
- **YAML** - YAML Language Server
- **GraphQL** - GraphQL Language Service
- **SQL** - SQL Language Server
- **Markdown** - Marksman (Markdown language server)
- **Dockerfile** - Dockerfile Language Server
- **Angular** - Angular Language Server

## Code Formatters

### Multi-Language Formatters

#### Prettier
- **Languages**: JavaScript, TypeScript, Flow, JSX, JSON, CSS, SCSS, Less, HTML, Vue, Angular, GraphQL, Markdown, YAML
- **Repository**: https://github.com/prettier/prettier
- **Features**: Opinionated formatter, minimal configuration, consistent output
- **Website**: https://prettier.io

#### clang-format
- **Languages**: C, C++, Java, JavaScript, Objective-C, Protobuf, C#
- **Repository**: Part of LLVM project
- **Features**: Configurable style rules, multiple preset styles (LLVM, Google, Chromium, Mozilla, WebKit)
- **Documentation**: https://clang.llvm.org/docs/ClangFormat.html

### Language-Specific Formatters

#### Python
- **Black** - The uncompromising Python code formatter
  - Repository: https://github.com/psf/black
  - Features: PEP 8 compliant, opinionated, deterministic, fast
  - Command: `black`

- **YAPF** - Yet Another Python Formatter
  - Features: Configurable, Google/PEP8 styles

- **Autopep8** - PEP 8 compliance tool
  - Features: Fixes PEP 8 style violations

#### Rust
- **rustfmt** - Official Rust formatter
  - Repository: https://github.com/rust-lang/rustfmt
  - Features: Enforces Rust style guidelines, configurable via `rustfmt.toml`
  - Command: `rustfmt` or `cargo fmt`

#### Go
- **gofmt** - Official Go formatter
  - Features: Built into Go toolchain, no configuration, canonical Go style
  - Command: `gofmt` or `go fmt`

- **gofumpt** - Stricter gofmt
  - Features: Enforces more formatting rules than gofmt

#### JavaScript/TypeScript
- **Prettier** (see above)
- **ESLint** with `--fix` - Linter with auto-fix capabilities
- **Biome** - Fast formatter/linter replacement for ESLint/Prettier

#### Ruby
- **RuboCop** - Ruby static code analyzer and formatter
  - Features: Style guide enforcement, auto-correction, highly configurable
  - Command: `rubocop --auto-correct`

- **Standard** - Ruby's bikeshed-proof linter and formatter
  - Repository: https://github.com/standardrb/standard
  - Features: Built on RuboCop, unconfigurable, opinionated

#### Swift
- **SwiftFormat** - Swift code formatter
  - Repository: https://github.com/nicklockwood/SwiftFormat
  - Features: Removes redundant `self`, fixes idioms, highly configurable
  - Command: `swiftformat`

#### Java
- **Google Java Format** - Google's Java style guide formatter
  - Features: Enforces Google Java Style, minimal configuration

- **Spotless** - Code formatting plugin
  - Features: Multi-language support including Java

#### Kotlin
- **ktlint** - Kotlin linter and formatter
  - Features: Enforces Kotlin coding conventions, auto-formatting

- **Kotlin Format** - Official Kotlin formatter

#### Dart
- **dart format** - Official Dart formatter
  - Features: Built into Dart SDK, follows Dart style guide
  - Command: `dart format`

#### C/C++
- **clang-format** (see above)
- **astyle** - Artistic Style formatter
  - Features: Configurable C/C++/Objective-C/C# formatter

#### C#
- **dotnet format** - Official .NET formatter
  - Features: Built into .NET CLI, uses .editorconfig settings

#### PHP
- **PHP CS Fixer** - PHP coding standards fixer
  - Features: PSR standards, highly configurable, Symfony rules

- **Pint** - Laravel PHP code style fixer
  - Features: Opinionated, based on PHP CS Fixer

#### Scala
- **scalafmt** - Scala code formatter
  - Features: Configurable, IDE integration, community-driven style
  - Repository: https://github.com/scalameta/scalafmt

#### Haskell
- **ormolu** - Haskell code formatter
  - Features: Opinionated, no configuration, GHC-compatible

- **stylish-haskell** - Haskell code formatter
  - Features: Configurable, preserves comments

#### HTML/CSS
- **Prettier** (see above)
- **Stylelint** - CSS linter with auto-fix
  - Features: Modern CSS, SCSS, Less support

#### Shell
- **shfmt** - Shell script formatter
  - Features: Bash, POSIX shell, mksh support
  - Command: `shfmt`

#### Terraform
- **terraform fmt** - Official Terraform formatter
  - Features: Built into Terraform CLI, canonical format
  - Command: `terraform fmt`

#### Nix
- **nixpkgs-fmt** - Nix code formatter
  - Features: Opinionated, used by nixpkgs

- **alejandra** - Nix formatter
  - Features: Configurable, well-maintained

#### Elm
- **elm-format** - Elm code formatter
  - Features: Opinionated, follows Elm style guide

#### OCaml
- **ocamlformat** - OCaml code formatter
  - Features: Auto-formatter for OCaml, configurable

#### SQL
- **SQLFluff** - SQL linter and formatter
  - Features: Multi-dialect support, configurable

#### TOML
- **taplo** - TOML formatter
  - Features: Opinionated, validates TOML syntax

#### YAML
- **prettier** (see above)
- **yamlfmt** - YAML formatter
  - Features: Configurable, preserves comments

#### Markdown
- **prettier** (see above)
- **markdownlint** - Markdown linter with auto-fix

## Additional Resources

- **Langserver.org** - Comprehensive LSP server catalog: https://langserver.org/
- **Microsoft LSP Implementors List** - Official server list: https://github.com/microsoft/language-server-protocol/blob/gh-pages/_implementors/servers.md
- **Awesome LSP Servers** - Curated list: https://github.com/Hexlet/awesome-lsp-servers
- **Are We Formatting Yet** - Formatter comparison: https://areweformattingyet.com/
- **treefmt** - Multi-language formatter runner: https://treefmt.com/

## Install Commands by Platform

Package manager legend: `brew` (Homebrew), `npm` (Node/npm), `pip`/`pipx` (Python), `cargo` (Rust), `go install` (Go toolchain), `gem` (RubyGems), `apt` (Debian/Ubuntu), `winget`/`scoop`/`choco` (Windows).

### LSP Servers

| Tool | macOS | Linux | Windows | Not available on |
|---|---|---|---|---|
| typescript-language-server | `npm i -g typescript-language-server typescript` | same as macOS | same as macOS | — (Node, cross-platform) |
| Biome | `brew install biome` | `npm i -g @biomejs/biome` or curl install script | `npm i -g @biomejs/biome` or `scoop install biome` | — |
| Pyright | `pip install pyright` or `npm i -g pyright` | same | same | — |
| pylsp (python-lsp-server) | `pip install python-lsp-server` | same | same | — |
| BasedPyright | `pip install basedpyright` | same | same | — |
| gopls | `go install golang.org/x/tools/gopls@latest` | same | same | — |
| rust-analyzer | `rustup component add rust-analyzer` | same | same | — |
| Eclipse JDT LS | `brew install jdtls` | download tarball from eclipse.org, or distro package | download zip, run via `jdtls` script (needs JDK) | no single npm/pip equivalent — JDK required everywhere |
| clangd | `brew install llvm` (bundles clangd) or Xcode Command Line Tools | `apt install clangd` | `winget install LLVM.LLVM` or install via Visual Studio/LLVM installer | — |
| ccls | `brew install ccls` | `apt install ccls` (or build from source) | not packaged; build from source with MSVC/clang | Windows (no official binaries) |
| OmniSharp | `brew install omnisharp-roslyn` (or use via .NET) | download release tarball, or `dotnet tool install -g omnisharp` (unofficial) | `dotnet tool install`/download release zip | needs .NET SDK everywhere |
| Solargraph | `gem install solargraph` | same | same (needs Ruby+DevKit) | — |
| Ruby LSP | `gem install ruby-lsp` | same | same | — |
| Intelephense | `npm i -g intelephense` | same | same | — |
| phpactor | `brew install phpactor` | download phar, or `composer global require phpactor/phpactor` | download `.phar`, run via PHP | — |
| SourceKit-LSP | bundled with Xcode / Swift toolchain | bundled with Swift toolchain (`apt` via swift.org install) | bundled with Swift for Windows toolchain | requires Swift toolchain install everywhere |
| Kotlin Language Server | `brew install kotlin-language-server` | download release tarball | download release zip (needs JDK) | no package-manager install on Windows |
| Dart Analysis Server | bundled with Dart SDK (`brew install dart`) | bundled with Dart SDK (`apt`/snap) | bundled with Dart SDK (`choco install dart-sdk`) | — |
| elixir-ls | `brew install elixir-ls` | download release zip, or via `mix escript.install github elixir-lsp/elixir-ls` | same as Linux (needs Elixir/Erlang) | — |
| clojure-lsp | `brew install clojure-lsp/brew/clojure-lsp-native` | install script from GitHub releases | download `.exe` from GitHub releases | — |
| haskell-language-server | `ghcup install hls` (or `brew install haskell-language-server`) | `ghcup install hls` | `ghcup install hls` | — |
| Metals (Scala) | `brew install coursier/formulas/coursier && cs install metals` | `cs install metals` (via Coursier) | `cs install metals` (via Coursier) | — |
| lua-language-server | `brew install lua-language-server` | download release tarball, or distro package | download release zip, or `scoop install lua-language-server` | — |
| elm-language-server | `npm i -g @elm-tooling/elm-language-server` | same | same | — |
| bash-language-server | `npm i -g bash-language-server` | same | same | — |
| terraform-ls | `brew install hashicorp/tap/terraform-ls` | `apt` via HashiCorp repo, or download binary | `choco install terraform-ls` or download binary | — |
| nil (Nix) | `nix profile install github:oxalica/nil` | same | not supported (Nix is Linux/macOS only) | Windows (Nix itself unsupported) |
| ocaml-lsp | `opam install ocaml-lsp-server` | same | same (via WSL/Cygwin OPAM, native Windows OPAM support is limited) | native Windows is best-effort |
| Marksman | `brew install marksman` | download binary, or distro package | `scoop install marksman` or download `.exe` | — |

### Formatters

| Tool | macOS | Linux | Windows | Not available on |
|---|---|---|---|---|
| Prettier | `npm i -g prettier` | same | same | — |
| clang-format | `brew install clang-format` | `apt install clang-format` | `winget install LLVM.LLVM` (includes clang-format) | — |
| Black | `pip install black` or `pipx install black` | same | same | — |
| YAPF | `pip install yapf` | same | same | — |
| Autopep8 | `pip install autopep8` | same | same | — |
| rustfmt | `rustup component add rustfmt` | same | same | — |
| gofmt | bundled with Go install (`brew install go`) | bundled with Go install | bundled with Go install | — |
| gofumpt | `go install mvdan.cc/gofumpt@latest` | same | same | — |
| ESLint --fix | `npm i -g eslint` | same | same | — |
| RuboCop | `gem install rubocop` | same | same | — |
| Standard (Ruby) | `gem install standard` | same | same | — |
| SwiftFormat | `brew install swiftformat` | build from source (SwiftPM), no official binary | build from source via SwiftPM | no prebuilt Linux/Windows package |
| Google Java Format | `brew install google-java-format` | download jar, run with `java -jar` | download jar, run with `java -jar` | — |
| Spotless | used via Gradle/Maven plugin, no standalone install | same | same | — |
| ktlint | `brew install ktlint` | download shell installer / binary | download `.jar`, run with `java -jar` (no native binary) | — |
| dart format | bundled with Dart SDK | bundled with Dart SDK | bundled with Dart SDK | — |
| astyle | `brew install astyle` | `apt install astyle` | download binary from sourceforge, or `choco install astyle` | — |
| dotnet format | bundled with .NET SDK (`brew install dotnet-sdk`) | bundled with .NET SDK | bundled with .NET SDK | — |
| PHP CS Fixer | `composer global require friendsofphp/php-cs-fixer` | same | same | — |
| Pint | `composer global require laravel/pint` (per-project via composer) | same | same | — |
| scalafmt | `brew install scalafmt` (or `cs install scalafmt`) | `cs install scalafmt` | `cs install scalafmt` | — |
| ormolu | `brew install ormolu` (or `ghcup install ormolu`) | `ghcup install ormolu` | `ghcup install ormolu` | — |
| stylish-haskell | `cabal install stylish-haskell` (or `ghcup`) | same | same | — |
| Stylelint | `npm i -g stylelint` | same | same | — |
| shfmt | `brew install shfmt` | `apt install shfmt` (or `go install mvdan.cc/sh/v3/cmd/shfmt@latest`) | `scoop install shfmt` or `go install` | — |
| terraform fmt | bundled with Terraform CLI | bundled with Terraform CLI | bundled with Terraform CLI | — |
| nixpkgs-fmt | `nix profile install nixpkgs#nixpkgs-fmt` | same | not supported (Nix unsupported) | Windows |
| alejandra | `nix profile install github:kamadorueda/alejandra` (or `brew install alejandra`) | `nix profile install` | not supported natively | Windows (without WSL) |
| elm-format | `npm i -g elm-format` (or `brew install elm-format`) | `npm i -g elm-format` | `npm i -g elm-format` | — |
| ocamlformat | `opam install ocamlformat` | same | limited native OPAM support | native Windows is best-effort |
| SQLFluff | `pip install sqlfluff` | same | same | — |
| taplo | `cargo install taplo-cli --locked` (or `brew install taplo`) | `cargo install taplo-cli` | `cargo install taplo-cli` or `scoop install taplo` | — |
| yamlfmt | `brew install yamlfmt` (or `go install github.com/google/yamlfmt/cmd/yamlfmt@latest`) | `go install` | `go install` | — |
| markdownlint | `npm i -g markdownlint-cli` | same | same | — |

### Notes for one-click install in Token

- Most tools resolve through one of: `npm i -g`, `pip`/`pipx install`, `cargo install`, `go install`, `gem install`, or `brew` — a per-tool "installer kind" enum (npm/pip/cargo/go/gem/brew/manual) plus package name covers ~90% of this table with no special-casing.
- Anything requiring a JDK (JDT LS, Kotlin LS, ktlint, Google Java Format via jar) needs a JDK-present check first — don't attempt to auto-install a JVM.
- Nix-based tools (nil, nixpkgs-fmt, alejandra) are Linux/macOS-only; hide or disable their install button on Windows.
- ccls and SwiftFormat have no official Windows binaries — surface a "build from source" note instead of a button on Windows.

## Integration Notes

Many editors and IDEs support these tools out of the box or through extensions:
- **VS Code** - Extensive LSP and formatter support via extensions
- **Neovim** - nvim-lspconfig, null-ls, conform.nvim
- **Emacs** - lsp-mode, eglot, format-all
- **Sublime Text** - LSP package
- **Helix** - Built-in LSP support
- **Zed** - Built-in LSP support

When choosing tools, consider:
- Project language stack
- Team preferences and existing configurations
- CI/CD integration requirements
- Editor/IDE compatibility
- Performance and resource usage

## Implementation: shared tooling catalog

The runtime-independent catalog lives in `src/tooling/`. `ToolDefinition` holds
identity and installation guidance; `ToolPreset` connects it to a typed
`Template::Lsp` or `Template::Formatter`. Installation options carry platform
applicability, prerequisites, ordered explanatory/copyable steps, and an official
source URL. These commands are display text, never executable jobs.

`src/tooling/presets.rs` owns LSP templates and the explicit six-server default
list; `default_formatters()` independently seeds Python/Ruff. The available
catalog is deliberately separate from these defaults. The larger tool list in
this document is a research backlog: verify runnable templates and current
upstream instructions before adding tools to the catalog.

`src/settings/forms/tooling.rs` projects the catalog into both form types and
copies presets into ordinary drafts. Optional persisted `preset_id` values supply
installation guidance only. Runtime LSP resolution and formatting use saved
configuration without catalog inheritance. Future one-click installation should
use a separate structured execution model rather than executing display strings.
