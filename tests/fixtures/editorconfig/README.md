# EditorConfig fixtures

`core/` contains unchanged `.in` fixtures and the BSD-2-Clause license from
[editorconfig-core-test](https://github.com/editorconfig/editorconfig-core-test),
revision `895b3a65d0d823dbd0acf2bc402376381995d1b1` (retrieved 2026-09-09).
`cases.json` is a generated transcription of the CTest arguments and expected
regular expressions for 198 current-spec parser, glob, property and traversal
cases. Absolute fixture paths are replaced with `@fixtures`.
CLI-only, pre-0.9 compatibility, and CMake's own sorting self-test are excluded.
The adapter test preserves the upstream output order, sorting only cases whose
upstream harness sorts. This is core compatibility evidence, not a claim of
complete editor/plugin conformance.

The dependency spike used ec4rs 1.2.0 (Apache-2.0; declared Rust 1.56 minimum),
with `track-source` and `allow-empty-values`. Standard property values must be
lowercased by the adapter; extension property values retain their case. The
spike passed all 199 selected CTest checks (including the CMake self-test) on
macOS. Legacy version switches and general encoding conversion are unsupported.

Editor-side indentation/whitespace expectations also refer to
[editorconfig-plugin-tests](https://github.com/editorconfig/editorconfig-plugin-tests),
revision `3178744a0d0df294579d1554bd28658456680e27` (CC-BY-4.0).
Tests in this repository are new Rust implementations of those behaviors; no
plugin-test source or fixture files are copied here.
