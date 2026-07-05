<div align="center">

<img src="https://sema-lang.com/logo.svg" alt="Sema" height="64">

# tree-sitter-sema

**[Sema](https://sema-lang.com) tree-sitter grammar** — a Lisp with first-class LLM primitives.

[![CI](https://img.shields.io/github/actions/workflow/status/sema-lisp/tree-sitter-sema/ci.yml?branch=main&label=CI&logo=github)](https://github.com/sema-lisp/tree-sitter-sema/actions)
[![License](https://img.shields.io/github/license/sema-lisp/tree-sitter-sema?color=c8a855)](LICENSE)
[![Website](https://img.shields.io/badge/website-sema--lang.com-c8a855)](https://sema-lang.com)

</div>

This is the canonical home for the grammar. Editor integrations (Zed, Helix, Neovim) consume it by pinning a tagged commit; the generated `src/` (parser + scanner) is committed so consumers compile it directly without running `tree-sitter generate`.

## Usage

```sh
tree-sitter generate && tree-sitter test
```

Editing `grammar.js` requires regenerating and committing `src/`:

```sh
npm install
tree-sitter generate    # updates src/parser.c etc.
tree-sitter test
```

## Node Types

The grammar produces the following node types:

- `list` — parenthesized list `(...)` 
- `vector` — square-bracket vector `[...]`
- `hash_map` — curly-brace map `{...}`
- `symbol` — identifiers and operators
- `keyword` — colon-prefixed keywords `:foo`
- `number` — integer and floating-point literals
- `string` — double-quoted string literals
- `boolean` — `#t` and `#f`
- `character` — character literals `#\a`
- `byte_vector` — byte vector literals `#u8(...)`
- `comment` — line comments `;`
- `block_comment` — block comments `#| ... |#`
- `quote` — `'expr`
- `quasiquote` — `` `expr ``
- `unquote` — `,expr`
- `unquote_splicing` — `,@expr`
- `short_lambda` — anonymous-function shorthand `#(...)`
- `regex` — regex literals `#"..."`
- `deref` — dereference `@expr`
- `shebang` — leading `#!...` line

## Consumers

Editor integrations pin a tagged commit of this grammar:

- [zed-sema](https://github.com/sema-lisp/zed-sema) — Zed extension
- [helix-sema](https://github.com/sema-lisp/helix-sema) — Helix language support
- [sema.nvim](https://github.com/sema-lisp/sema.nvim) — Neovim plugin

## Releases

Releases are tagged (`vX.Y.Z`); consumers pin a specific tag rather than tracking `main`. Current release: **v0.2.0**.

## Links

- **Website** — [sema-lang.com](https://sema-lang.com)
- **Playground** — [sema.run](https://sema.run)
- **Documentation** — [sema-lang.com/docs](https://sema-lang.com/docs/)
- **Consumers** — [zed-sema](https://github.com/sema-lisp/zed-sema) · [helix-sema](https://github.com/sema-lisp/helix-sema) · [sema.nvim](https://github.com/sema-lisp/sema.nvim)
- **Repository** — [sema-lisp/tree-sitter-sema](https://github.com/sema-lisp/tree-sitter-sema)

## License

[MIT](LICENSE) © [Helge Sverre](https://github.com/HelgeSverre)
