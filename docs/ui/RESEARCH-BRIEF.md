# Token UI component research brief

Research scope established 2026-09-12. This work specifies components; it does
not authorize replacing Settings or implementing a new widget framework.

## Objective

Establish a precise component vocabulary and implementation contracts for Token,
informed by IntelliJ's UI guidelines and grounded in Token's actual Elm-style
model/update/command/render architecture. Document existing overlap, missing
behavior, gallery coverage, and the next useful implementation slices.

## Design questions

1. Which names distinguish actions, value selection, navigation, and containers?
2. Which production painters, state models, and geometry helpers already provide
   reusable components, and which remain feature-local?
3. What state, event, focus, keyboard, pointer, and dismissal contracts must each
   component expose without owning application effects?
4. Which subcomponents should be shared, and which domain-specific behavior must
   remain separate (especially editor, terminal, and panel tabs)?
5. How should theme roles, font roles, scaling, clipping, and accessibility work?
6. Which gallery specimens prove useful visual and interaction coverage?
7. Which missing components have concrete consumers and should be added next?

## Sources and confidence

- IntelliJ: [UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)
  and linked official component guidelines. Primary evidence for IntelliJ UX,
  not an instruction to adopt Swing or Kotlin infrastructure.
- Local [IntelliJ SDK UI reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md): user-provided secondary SDK
  guidance; cross-check relevant claims against official documentation.
- Token: current `src/`, tests, gallery catalog, and
  [editor reference](../EDITOR_UI_REFERENCE.md). Code is authoritative over plans.

Mark verified implementation and primary-source guidance **high confidence**;
mark proposed Token contracts explicitly **proposed**, not implemented. Unknown
or unverified behavior must stay visible. Link concrete source files and URLs.

## Component document contract

Each `COMPONENT-NAME.md` covers purpose and naming; current implementation and
consumers; anatomy/subcomponents; data model and ownership; event transitions;
keyboard, pointer, focus and accessibility; layout, font and theme contracts;
edge cases; gallery states; implementation/reuse guidance; gaps and acceptance
criteria; evidence. Separate present behavior from recommendations throughout.

## Work split and synthesis

Research input/action controls, navigation/collections, and surfaces/feedback in
parallel. Independently inventory Token's architectural foundations. Consolidate
names, cross-links, coverage and prioritized next slices in `README.md`; retain
explicit exclusions so this does not become an unbounded widget wishlist.
