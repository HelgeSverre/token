# Technical reference standard

This document defines the acceptance bar for the rewrite requested 2026-09-13.
The earlier catalog was an inventory, not an implementation manual. Adding more
adjectives, source links, or acceptance bullet points does not address that gap.

## Required treatment in every component chapter

1. **Concrete representation:** show existing Rust data structures or an accurate
   relevant excerpt, explain every field/unit/identity/lifetime, then give a
   separately labelled proposed representation where needed. Define dependencies
   used in sketches. Do not pass a bag of unspecified `state` to an algorithm.
2. **Ownership and invariants:** distinguish durable value, transient input,
   borrowed presentation and derived layout/cache. Explain invalid states and
   exactly who repairs them after mutation/removal/resize. Use equations or
   assertions where useful, not just “keep selection valid.”
3. **Transitions:** specify state × event → new state + intent/effect, including
   input preconditions, disabled/read-only, release/capture/cancel/focus loss,
   empty collections and changing identity. Passive components explicitly have
   no input machine; document their owner's invalidation and projection instead.
4. **Algorithms:** derive sizing, placement, clipping, index mapping, selection,
   scrolling and hit testing as applicable. Name units and rounding boundaries.
   Include executable-looking pseudocode/Rust that actually calculates results.
5. **Worked traces:** walk through concrete input values and resulting state,
   rectangles or indices. Include at least a normal case and a pathological
   case. “Test narrow widths” is not a worked example.
6. **Integration:** show the real Token update/render/input path and the proposed
   function boundaries, including how a consumer assembles subcomponents. Do not
   invent a component framework or imply proposed types already exist.
7. **Invalidation and cost:** list actual inputs of cached/derived state and when
   they change; explain traversal/measurement cost and avoid unsupported timing
   claims. Async consumers require generation/owner checks and stale results.
8. **Verification cases:** concrete initial state, action and expected output,
   including numeric geometry when relevant. Distinguish existing tests from
   proposed tests and static gallery rendering from interaction coverage.

Use `EDITOR_UI_REFERENCE.md` as the model for derivation and explanation, not
as a word-count quota. Simple passive primitives need less text than forms or
editor surfaces, but no chapter is complete as a glossary card. Cross-reference
shared foundations without omitting component-specific mechanics.

## Code sample conventions

- Label blocks **current excerpt**, **proposed API**, or **algorithm sketch**.
- Rust blocks should have coherent field/type definitions and ownership. Name
  omitted dependencies; do not present pseudocode as a compiling integration.
- No production Rust changes are authorized by this documentation rewrite.
- Describe current font roles accurately: inputs/editor/explorer/document/dock/
  terminal tabs use Code; other UI, including overlay tabs, uses explicit UI
  scopes. The 2026-09-16 [visual-polish plan](../feature/editor-visual-polish.md)
  proposes a later measured UI-font trial for non-editable chrome. Mark that
  policy change as proposed; preserve source/input geometry and do not imply
  that writing a component chapter has already changed a native font role.
- Source links support factual claims but are not a substitute for explaining
  the implementation in the document itself.

## Completion review

Review every family document against all eight requirements. A filename, heading
or code-fence count alone is not proof. Verify representative computations and
API excerpts against current source, reconcile cross-document concepts, and
fix errors before declaring the collection complete.
