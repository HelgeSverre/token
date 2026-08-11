# Vendored Tree-sitter compatibility sources

These directories contain generated parser or external-scanner sources for
grammars whose published Rust bindings cannot be linked directly with Token's
Tree-sitter 0.25 runtime. `build.rs` gives every parser its own native archive,
and `src/syntax/compat.rs` exposes the resulting language functions through
`tree-sitter-language`.

| Directory | Upstream | Source revision | License | Local contents |
| --- | --- | --- | --- | --- |
| `tree-sitter-applescript-compat` | [HelgeSverre/tree-sitter-applescript](https://github.com/HelgeSverre/tree-sitter-applescript) | `1676f5fe99eee6b6532ab6d13559323c48d26190` | MIT | External scanner compatibility source; license and details are retained in the directory. |
| `tree-sitter-cue-compat` | [eonpatapon/tree-sitter-cue](https://github.com/eonpatapon/tree-sitter-cue) | `d4f98c1c236d25a1fce7ffc207b3809b521e6e7b` | MIT | Generated parser and external scanner from crate `tree-sitter-cue` 0.0.1. |
| `tree-sitter-fennel-compat` | [alexmozaidze/tree-sitter-fennel](https://github.com/alexmozaidze/tree-sitter-fennel) | `3f0f6b2` | CC0-1.0 | Generated parser and external scanner. |
| `tree-sitter-janet-compat` | [GrayJack/tree-sitter-janet](https://github.com/GrayJack/tree-sitter-janet) | `64db751b233ba44ce06fa6c729701bdf87779011` | BSD-3-Clause | External scanner compiled separately from the otherwise current Rust binding. |
| `tree-sitter-pest-compat` | [tree-sitter/tree-sitter-pest](https://github.com/tree-sitter/tree-sitter-pest) | `a8a98a824452b1ec4da7f508386a187a2f234b85` | MIT | Generated parser from crate `tree-sitter-pest` 0.0.2. |
| `tree-sitter-pony-compat` | [amaanq/tree-sitter-pony](https://github.com/amaanq/tree-sitter-pony) | `16f930b250433cfcd4fb4144df92bb98ad344c20` | MIT | Generated parser and external scanner from crate `tree-sitter-pony` 1.0.0. |

The generated sources remain attributable to their upstream projects and are
not hand-maintained. When an upstream publishes a Tree-sitter 0.25-compatible
Rust binding, replace its compatibility directory with the normal dependency
and remove the corresponding adapter.
