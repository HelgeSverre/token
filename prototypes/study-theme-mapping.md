# Study theme mapping

`themes/study.yaml` turns the original performance-panel study into Token's
built-in **Study** theme. It keeps the cool graphite ground and restrained teal
accent that made the prototype feel quieter than Default Dark, while assigning
every native theme role a deliberate color.

The prototype still exposes two intentional comparison modes:

- **Original study (custom)** (`?theme=study`) is the hand-tuned CSS palette
  from the first visual experiment.
- **Study (built-in theme)** (`?theme=builtin-study`) resolves the registered
  `study` theme from `themes/study.yaml` through the same generated data used by
  every other prototype theme.

The alias is necessary because the original prototype already reserved `study`
for its custom state. The registered theme's actual stable ID remains `study`.

## Source inventory and mapping

“Exact” means the original custom-property value could populate the native role
without changing its meaning. “Adapted” maps an existing prototype surface to
the closest native role. “Derived” fills a role that the original prototype did
not have, using a neighboring Study color and preserving its contrast ramp.

| Native field | Value | Source | Mapping and reason |
| --- | --- | --- | --- |
| `editor.background` | `#181B1E` | `--editor` | Exact editor ground. |
| `editor.foreground` | `#BCC8CF` | `--code-fg` | Exact code foreground. |
| `editor.current_line_background` | `#232E32` | `--current-line` | Exact active-line fill. |
| `editor.cursor_color` | `#79D8BD` | `--teal` / `--focus` | Adapted: the literal Study accent is reused for a visible cursor. |
| `editor.selection_background` | `#34423E` | `--control-selected` | Adapted: the study's selected-control wash is its only sustained selection fill. |
| `editor.secondary_cursor_color` | `#79D8BD80` | teal with 50% alpha | Derived secondary cursor from the exact cursor accent. |
| `editor.indent_guide` | `#2A3034` | `--soft-line` | Adapted quiet interior guide. |
| `gutter.background` | `#181B1E` | transparent gutter matched to `--editor` | Adapted visual-match choice: the native role supports alpha, but the opaque editor value reproduces the study's transparent-on-editor composition. |
| `gutter.foreground`, `gutter.foreground_active` | `#8E9AA2` | `--gutter-fg`, `--gutter-active` | Exact. |
| `gutter.border_color` | `#2A3034` | `--soft-line` | Exact quiet editor boundary. |
| `status_bar.background`, `foreground`, `border` | `#24292D`, `#A1AEB7`, `#32383C` | `--status-*` | Exact. |
| `sidebar.background`, `foreground` | `#1D2124`, `#A1ABB3` | `--sidebar-*` | Exact. |
| `sidebar.selection_background`, `selection_foreground` | `#30393F`, `#E0E6E9` | `--sidebar-selected*` | Exact. |
| `sidebar.hover_background` | `#343B40` | `--hover` | Exact row hover fill. |
| `sidebar.folder_icon`, `file_icon` | `#8D9CA7`, `#BC9B81` | `--folder-icon`, `--file-icon` | Exact. |
| `sidebar.border` | `#2A3034` | `--soft-line` | Exact sidebar/editor divider. |
| `splitter.background` | `#2A3034` | `--soft-line` | Adapted quiet internal divider. The study also used `#32383C` for stronger panel/status boundaries, which stay on those explicit roles. |
| `tab_bar.background`, `active_background`, `active_foreground` | `#1D2124`, `#181B1E`, `#CFD7DD` | `--tabs-*` | Exact. |
| `tab_bar.inactive_background`, `inactive_foreground`, `border` | `#1D2124`, `#8E999F`, `#2A3034` | `--tabs-*` | Exact. |
| `tab_bar.modified_indicator` | `#79D8BD` | active-tab teal border | Adapted accent for unsaved state. |
| `overlay.border`, `background`, `foreground` | `#51616A`, `#12191FED`, `#E5EEEF` | `--tooltip-*` | Exact tooltip/popup treatment. |
| `overlay.input_background` | `#181B1E` | `--editor` | Adapted recessed input ground. |
| `overlay.selection_background`, `highlight`, `warning`, `error` | `#34423E`, `#79D8BD`, `#E6BB7C`, `#ED918B` | `--control-selected`, `--teal`, `--amber`, `--red` | Exact prototype state/diagnostic anchors. |
| `overlay.accent`, `accent_bright` | `#79D8BD`, `#BBF4E2` | `--focus`, `--control-selected-fg` | Exact focus and selected-label colors. |
| `overlay.panel_background`, `panel_secondary`, `recessed_wash` | `#1C2023`, `#202427`, `#181B1E` | `--panel`, `--chrome`, `--editor` | Exact Study tiers. Native overlay schema has no separate `chrome` field, so `panel_secondary` carries it. |
| `overlay.hairline`, `selection_wash`, `match_on_selection` | `#30393D`, `#34423E`, `#E0FFF4` | `--chart-grid`, `--control-selected`, `--chart-selected` | Exact. |
| `overlay.text_primary`, `text_bright`, `text_secondary`, `text_dim` | `#D4DFE3`, `#E8F2EF`, `#A0A9AF`, `#858F97` | `--stat-fg`, `--bright`, `--muted`, `--faint` | Exact hierarchy. |
| `overlay.keycap_bg`, `keycap_border`, `keycap_fg` | `#262C30`, `#3A4247`, `#C0CBD0` | `--control-bg`, `--window-border`, `--stage-fg` | Adapted control/keycap hierarchy. |
| `overlay.severity_{error,warning,info,hint}` | `#ED918B`, `#E6BB7C`, `#8EAFF0`, `#929FA7` | `--red`, `--amber`, `--blue`, `--chart-label` | Exact first three; hint adapted from chart labels. |
| `overlay.severity_*_text` | `#FFC3BF`, `#F5CF94`, `#BFCEFC`, `#CAD4D9` | severity values lifted for banner contrast | Derived to retain the native 4.5:1 contrast floor. |
| `csv.header_background`, `header_foreground`, `grid_line` | `#202427`, `#D4DFE3`, `#30393D` | chrome/stat/chart tokens | Adapted table chrome. |
| `csv.selected_cell_background`, `selected_cell_border`, `number_foreground` | `#34423E80`, `#79D8BD`, `#B3A0E1` | selected/teal/violet | Adapted selection; exact accent/violet. |
| `button.background`, `background_hover`, `background_pressed`, `background_selected` | `#262C30`, `#30373D`, `#34423E`, `#34423E` | `--control-*` | Exact original control states. |
| `button.foreground`, `foreground_disabled`, `border`, `focus_ring` | `#D4DFE3`, `#858F97`, `#32383C`, `#79D8BD` | stat/faint/line/focus | Exact or direct semantic reuse. |
| `image_preview.*` | `#24292D`, `#1C2023`, `8` | status/panel/default size | Derived neutral checkerboard. |
| `scrollbar.track`, `thumb`, `thumb_hover` | `#181B1E`, `#39434B`, `#4B5760` | transparent track matched to editor; `--scrollbar`; lifted thumb | Exact thumb, with a visual-match editor track and hover adaptation. |
| `syntax.keyword`, `function`, `type`, `comment` | `#BE9ACB`, `#A9BE90`, `#8AB7C4`, `#809383` | `--syntax-*` | Exact study highlighting. |
| remaining `syntax.*` | graphite, teal, blue, amber, violet and muted values shown in `study.yaml` | stage-series palette and text ramp | Derived only where the prototype had no syntax role, keeping existing series colors semantically consistent. |

## Schema choices

The browser study had four broad dark surfaces: desktop `#111315`, editor
`#181B1E`, dock panel `#1C2023`, and chrome `#202427`, plus sidebar
`#1D2124`. Token's native schema purposefully does not expose a desktop color
or a generic chrome color. The native theme therefore uses the editor for the
window ground, keeps sidebar and tab bar exact, and assigns panel/chrome to the
overlay panel tiers. This preserves the study's contrast between document,
sidebar, dock, and popup without inventing new theme fields.

The prototype palette adapter also intentionally couples a few browser
properties to native roles: generic panels use `sidebar.background`, controls
use overlay keycap roles, and charts use syntax colors. The built-in Study
preview therefore follows the same repository adapter as every other theme; it
does not promise a pixel-identical reproduction of the custom CSS study. The
two selector entries remain available precisely to make that comparison clear.

Terminal ANSI colors are not present in the current `ThemeData` schema. Study
therefore styles terminal-adjacent chrome through the shared sidebar, tab, and
editor roles only; it does not introduce an unsupported terminal palette.
