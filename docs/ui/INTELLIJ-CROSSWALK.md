# IntelliJ terminology → Token component families

Source inventory checked 2026-09-12 against the official
[component index](https://plugins.jetbrains.com/docs/intellij/components.html)
and [application overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).
This is a coverage map, not a claim that Token implements every entry. Detailed
family documents describe implementation status and proposed contracts.

## Component index coverage

| IntelliJ name          | Token family / canonical term         | Boundary                                                                   |
| ---------------------- | ------------------------------------- | -------------------------------------------------------------------------- |
| Button                 | Button                                | Immediate action                                                           |
| Built-In Button        | Text field accessory action           | Part of an input, not a new independent input engine                       |
| Checkbox               | Checkbox                              | Boolean or mixed selection; mixed support must be explicit                 |
| Combo Box              | Combo box                             | Editable text plus suggested choices, not the existing non-editable Select |
| Context Help           | Tooltip, empty state, label/help text | Umbrella purpose, not one component                                        |
| Description Text       | Label / form description              | Non-interactive explanatory content                                        |
| Drop-Down List         | Select                                | Existing-value selection                                                   |
| Got It Tooltip         | Tooltip: onboarding variant           | Defer unsolicited onboarding; not ordinary hover help                      |
| Group Header           | Group header / disclosure             | Static or collapsible section heading                                      |
| Icon Button            | Button with icon content              | Same action semantics, accessible name required                            |
| Input Field            | Text field                            | Shared editable state and renderer                                         |
| Link                   | Link                                  | Navigation, distinguished from commands                                    |
| Notifications          | Notification                          | Feedback family, not a positioning primitive                               |
| Notification Balloon   | Notification: toast variant           | Transient nonmodal feedback; proposed unless implemented                   |
| Banner                 | Notification: contextual banner       | Persistent context-bound feedback                                          |
| Progress Indicators    | Progress                              | Operation state presentation                                               |
| Loader                 | Progress: indeterminate               | No fabricated completion percentage                                        |
| Progress bar           | Progress: determinate                 | Known completed/total work                                                 |
| Progress text          | Progress: text                        | Works without animation or a bar                                           |
| Radio Button           | Radio group                           | Mutually exclusive labelled alternatives                                   |
| Scrollbar              | Scroll area: scrollbar                | Indicator and input mapped to the owner's viewport                         |
| Search Field           | Search field                          | Text field with search semantics and result lifecycle                      |
| Split Button           | Split button                          | Primary action plus separate action-menu target                            |
| Split Icon Button      | Split button: icon content            | Same two-target semantics                                                  |
| Table                  | Table                                 | Column relationships, unlike a free-form result list                       |
| Tabs                   | Tabs                                  | Content navigation, preserving Token's four families                       |
| Text Area              | Text field: multiline                 | Reuse editable engine; distinct Enter and wrapping policies                |
| Toggle Button          | Button: toggle variant                | Retained selected state, not pointer press                                 |
| Toolbar                | Toolbar                               | Ordered action/control composition                                         |
| Toolbar Drop-Down List | Select in toolbar                     | Density/layout variation, not a second selection model                     |
| Tool Window            | Panel                                 | Existing Token dock/panel vocabulary                                       |

## Application overview coverage

| Overview surface             | Token contract or disposition                                                                             |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- |
| Main toolbar                 | Toolbar; adopting an always-visible main toolbar is a separate product decision                           |
| Project/VCS/run widgets      | Compositions of select, menu, button and status; do not invent these workflows just to populate a gallery |
| Tool window stripes          | Potential panel-navigation composition; defer until there is a concrete navigation need                   |
| Editor area and tabs         | Editor surface, tabs and splitter                                                                         |
| Gutter and inlays            | Editor-surface subcomponents, backed by the existing visual-row geometry                                  |
| Inspection widget            | Diagnostic/status composition; not a second diagnostics state store                                       |
| Floating toolbar             | Toolbar inside anchored popup; avoid automatic display without an explicit interaction design             |
| Status bar/widgets           | Status bar                                                                                                |
| Navigation bar               | Breadcrumb navigation is a possible later component; no automatic replacement of Token's explorer         |
| Dialogs                      | Dialog; keep Settings as its separate preferences page                                                    |
| Popups                       | Popup, specialized into menu, completion and documentation content                                        |
| Notifications/alerts/banners | Notification or dialog according to whether interaction must block                                        |
| Context menus                | Menu with context captured by its owner                                                                   |

## Interpretation

**High confidence:** the source indexes establish these names and purposes.
**Proposed synthesis:** Token's family grouping above deliberately combines
variants that can share a contract while retaining domain distinctions. It does
not imply that JetBrains uses Token's architecture or that all corresponding
Rust types already exist.

The references are UX precedents. Swing classes, UI DSL bindings, service
lifetimes and extension-point registration are not portable component APIs for
Token. The local SDK notes are useful for locating those distinctions, but Token's
state/effect boundary remains authoritative.
