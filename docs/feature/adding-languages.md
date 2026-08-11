# Adding Tree-sitter Languages

Token keeps language detection, parser construction, highlighting, embedded
language parsing, structural selection, and outlines as composable concerns
owned by one authoritative `LanguageDefinition`. A language is complete only
when each relevant concern is covered by its descriptor.

## Integration checklist

1. Select a maintained grammar whose generated ABI works with Tree-sitter
   0.25. Prefer a released crate exposing `tree_sitter_language::LanguageFn`.
   Otherwise pin an exact Git revision. Vendor generated sources only when an
   older binding would introduce a second `tree_sitter::Language` type.
2. Add the `LanguageId`, then create its complete descriptor module in
   `src/syntax/registry.rs`. Add that descriptor to `ALL_LANGUAGES`.
3. Put display name, extensions, fence aliases, exact filenames, and compound
   suffixes in that descriptor. Do not add detection branches elsewhere.
4. Register the grammar factory and highlight source in the descriptor.
   Queries may come from an exported grammar constant or an attributed local
   file under `queries/<language>/highlights.scm`.
5. Add a representative fixture under `samples/syntax`. The
   `every_syntax_sample_has_a_registered_language` test prevents fixtures from
   silently falling back to plain text.
6. Add the fixture to `extended_language_samples_parse_and_highlight` and add
   a declaration/expression case to
   `extended_languages_provide_structural_candidates`.
7. Add an explicit outline implementation for declaration-bearing grammars.
   Languages without meaningful document symbols compose `NoOutline`; node
   kinds and name extraction are never guessed globally.
8. If the format embeds other languages, implement document-relative
   included-range discovery in its `InjectionBehavior` and test the resulting
   snapshot. Query registration alone is not enough for expand selection.
9. Run `just fmt`, `just lint`, the syntax tests, and `just test`.

## Metadata and detection

Language support is declarative and behavior-bearing:

```rust
language!(
    example, Example, "Example",
    ["example", "ex"], ["example", "ex"], [], [],
    tree_sitter_example::LANGUAGE.into(),
    static_query!(p::EXAMPLE_HIGHLIGHTS),
    CODE_SELECTION, NO_OUTLINE, NO_INJECTIONS
);
```

Extensions and fenced-code aliases must be unique. Ambiguous extensions should
not be claimed until `from_path()` has a reliable filename or content policy.
For example, `.h` remains C, `.vsh` is V rather than GLSL, and ambiguous Forth
extensions such as `.f` and `.fs` are omitted.

Do not register binary or bundle formats as text solely because their suffix is
associated with a language. AppleScript therefore recognizes `.applescript`,
but not compiled `.scpt` files or `.scptd` bundles.

## Parser and query registration

`ParserState` constructs parsers and compiles queries lazily on first use from
the descriptor's `ParserDefinition`. There is no parser match to update and no
language should eagerly initialize itself in `ParserState::new()`.

Use one of these dependency strategies:

- Released, compatible crate: preferred.
- Exact Git revision: for maintained grammars without a release.
- Local generated-source adapter: only for a grammar whose old Rust binding
  cannot coexist with Tree-sitter 0.25. Record its source, revision, and license
  in `vendor/TREE_SITTER_COMPAT.md`.

Never link a legacy grammar crate merely to obtain its native parser symbol:
doing so can link multiple Tree-sitter runtimes and cause duplicate symbols or
process crashes.

Highlight queries are an independent completion gate. If upstream ships a
query but does not export it, copy it under `queries/` with its provenance
recorded. If no query exists, author a minimal query and expand it from the
fixture. Parser-only registration is not considered syntax support.

## Structural selection

Every descriptor owns a `&dyn SelectionBehavior`. `src/syntax/selection.rs`
provides shared structural implementations that languages compose or replace.
The shared strategies are:

- `CodeSelection` walks expressions, statements, declarations, blocks, strings,
  and delimiter interiors.
- markup profiles add opening/closing-tag and element-interior boundaries;
- YAML, INI, and Rust profiles normalize their grammar-specific ranges; and
- document selection preserves document-oriented behavior.

Language profiles may provide a boundary profile, node-range normalization,
or extra semantic range callback. Add specialization only when a focused
candidate-chain test demonstrates that the generic fallback is wrong. Existing
Rust attached-doc/attribute, YAML comment, INI whitespace, and HTML-family
boundary behavior must remain intact.

Injected trees are evaluated before their host tree. While the selection is
inside a valid embedded region, host candidates are admitted only when they
contain the whole region. This prevents malformed host AST fragments from
interleaving with embedded-language declarations, as covered by the Svelte
adjacent-function regression test.

## Embedded languages

`SyntaxTreeSnapshot` stores a host tree and zero or more `InjectedSyntaxTree`
values. Each injected parser receives a Tree-sitter included range over the
original document, so all byte positions remain document-relative.

When adding an injected format:

1. Locate the content node and determine the embedded `LanguageId`.
2. Exclude delimiters when the embedded grammar does not accept them.
3. Implement or extend the descriptor's `InjectionBehavior`.
4. Test both its language and source range.
5. Add an expand-selection regression covering an embedded declaration.

Current discovery covers Markdown fences, HTML/Vue/Svelte/Astro scripts and
styles, Astro frontmatter, Dockerfile and Make shell commands, Tera
frontmatter, Hurl JSON/XML bodies, and language-tagged Typst raw blocks.

## Outlines

Each declaration-bearing language owns an outline extractor or an explicit
query/rule implementation. Shared traversal, range construction, and
containment nesting are reusable components, but node-kind and name semantics
belong to the language implementation. Formats without meaningful declarations
compose `NoOutline` explicitly.

Outline tests assert symbol kind and name, and include nesting when the language
supports members.

## Verification commands

```bash
cargo test every_syntax_sample_has_a_registered_language --lib
cargo test extended_language_samples_parse_and_highlight --lib
cargo test test_all_query_files_compile --lib
cargo test extended_languages_provide_structural_candidates --lib
cargo test syntax_snapshot_contains_ --lib
just fmt
just lint
just test
```
