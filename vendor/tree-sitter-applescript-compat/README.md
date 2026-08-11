# Tree-sitter AppleScript compatibility sources

These files are copied unchanged from
`HelgeSverre/tree-sitter-applescript` revision
`1676f5fe99eee6b6532ab6d13559323c48d26190` under its MIT license.

The pinned crate's `build.rs` compiles `parser.c` but not its required external
`scanner.c`. Token compiles the scanner separately until the upstream Rust
package includes it. `tree_sitter/parser.h` is the generated Tree-sitter parser
API header shipped at the same revision.
