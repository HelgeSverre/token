//! Isolated development fixtures. Nothing here reads or writes editor settings.
use crate::editable::{EditConstraints, EditableState, StringBuffer};

pub const CATEGORIES: &[&str] = &[
    "All",
    "Buttons",
    "Inputs",
    "Selection",
    "Forms",
    "Menus & rows",
    "Structure",
    "Tabs & panels",
    "Editor",
];

#[derive(Clone, Copy)]
pub enum ChromePreview {
    DocumentTabs,
    DocumentOverflow,
    DocumentDrag,
    DockTabs,
    TerminalTabs,
    TerminalOverflow,
    TerminalExited,
    BottomPanel,
    ProblemsPopulated,
    UsagesPopulated,
    TerminalContent,
    RightPanel,
}

#[derive(Clone, Copy)]
pub enum EditorPreview {
    Selection,
    IndentGuides,
    Folding,
    Diagnostics,
    Inlays,
    Find,
}

#[derive(Clone, Copy)]
pub enum SearchCollectionPreview {
    Grouped,
    Loading,
    Empty,
}

#[derive(Clone, Copy)]
pub enum SettingsRecordsPreview {
    Selected,
    Empty,
}

#[derive(Clone, Copy)]
pub enum Preview {
    ButtonNormal,
    ButtonHovered,
    ButtonPressed,
    ButtonFocused,
    ButtonSelected,
    ButtonDisabled,
    ButtonLongLabel,
    IconButton,
    FieldUnfocused,
    FieldFocused,
    FieldSelection,
    FieldMultiline,
    SearchField,
    FormValidation,
    SettingsForm,
    SettingsRecords(SettingsRecordsPreview),
    SearchCollection(SearchCollectionPreview),
    CompletionDocumentation,
    HoverDocumentation,
    SignatureHelp,
    Checkbox(bool),
    Select {
        open: bool,
    },
    SelectOptions,
    ChoiceGroup,
    Disclosure {
        expanded: bool,
    },
    MenuRows {
        hover: bool,
    },
    ListRow,
    Panel,
    Secondary,
    Recessed,
    Chrome(ChromePreview),
    Editor(EditorPreview),
    OverlayTabs,
    Scrollbar {
        horizontal: bool,
        hovered: bool,
        fits: bool,
        end: bool,
    },
    Splitter {
        horizontal: bool,
    },
}

#[derive(Clone, Copy)]
pub struct Specimen {
    pub id: &'static str,
    pub preview: Preview,
    pub category: usize,
    pub source: &'static str,
    pub tokens: &'static str,
}

pub const SPECIMENS: &[Specimen] = &[
    Specimen {
        id: "settings-form.semantic-controls",
        preview: Preview::SettingsForm,
        category: 4,
        source: "view/overlay_surface.rs + settings typed control presentation",
        tokens: "settings form / checkbox / select / disclosure / validation",
    },
    Specimen {
        id: "settings-records.selected",
        preview: Preview::SettingsRecords(SettingsRecordsPreview::Selected),
        category: 4,
        source: "view/modal.rs · with_settings_spec + settings_page records",
        tokens: "settings collection / selected record / enabled state / narrow layout",
    },
    Specimen {
        id: "settings-records.empty",
        preview: Preview::SettingsRecords(SettingsRecordsPreview::Empty),
        category: 4,
        source: "view/modal.rs · with_settings_spec + settings_page empty state",
        tokens: "settings collection / no saved entries / add record",
    },
    Specimen {
        id: "search-everywhere.grouped-results",
        preview: Preview::SearchCollection(SearchCollectionPreview::Grouped),
        category: 5,
        source: "view/modal.rs · Search Everywhere production modal",
        tokens: "commands + files sections / selection / shared row geometry",
    },
    Specimen {
        id: "search-everywhere.loading",
        preview: Preview::SearchCollection(SearchCollectionPreview::Loading),
        category: 5,
        source: "view/modal.rs · workspace symbol status",
        tokens: "symbols tab / asynchronous loading / footer status",
    },
    Specimen {
        id: "search-everywhere.empty",
        preview: Preview::SearchCollection(SearchCollectionPreview::Empty),
        category: 5,
        source: "view/modal.rs · Search Everywhere empty decoration",
        tokens: "empty grouped collection / non-selectable status",
    },
    Specimen {
        id: "completion.with-documentation",
        preview: Preview::CompletionDocumentation,
        category: 5,
        source: "view/overlay_surface.rs · list + Documentation",
        tokens: "completion selection / code and UI typography / docs scrollbar",
    },
    Specimen {
        id: "hover.documentation",
        preview: Preview::HoverDocumentation,
        category: 5,
        source: "view/overlay_surface.rs · cursor Zones",
        tokens: "hover signature / prose / diagnostic banner",
    },
    Specimen {
        id: "signature-help.active-parameter",
        preview: Preview::SignatureHelp,
        category: 5,
        source: "view/overlay_surface.rs · cursor Zones",
        tokens: "code font / active parameter accent / documentation",
    },
    Specimen {
        id: "terminal-tabs.exited",
        preview: Preview::Chrome(ChromePreview::TerminalExited),
        category: 7,
        source: "panels/terminal.rs · reveal_active_tab + render_tabs",
        tokens: "sidebar / selected exited session / actions",
    },
    Specimen {
        id: "document-tabs.states",
        preview: Preview::Chrome(ChromePreview::DocumentTabs),
        category: 7,
        source: "view/document_tabs.rs + layout/editor.rs",
        tokens: "tab_bar active / inactive / save-error (!) ",
    },
    Specimen {
        id: "document-tabs.overflow",
        preview: Preview::Chrome(ChromePreview::DocumentOverflow),
        category: 7,
        source: "view/document_tabs.rs + layout/editor.rs",
        tokens: "tab_bar / title clipping / horizontal scroll",
    },
    Specimen {
        id: "document-tab.drag-ghost",
        preview: Preview::Chrome(ChromePreview::DocumentDrag),
        category: 7,
        source: "view/mod.rs · render_tab_drag_ghost",
        tokens: "tab_bar active / border / translucent ghost",
    },
    Specimen {
        id: "dock-tabs.active",
        preview: Preview::Chrome(ChromePreview::DockTabs),
        category: 7,
        source: "view/panels.rs · DockPaneScene",
        tokens: "sidebar selection / text / border",
    },
    Specimen {
        id: "terminal-tabs.states",
        preview: Preview::Chrome(ChromePreview::TerminalTabs),
        category: 7,
        source: "panels/terminal.rs · render_tabs",
        tokens: "sidebar / active / hovered / exited / actions",
    },
    Specimen {
        id: "terminal-tabs.overflow",
        preview: Preview::Chrome(ChromePreview::TerminalOverflow),
        category: 7,
        source: "panels/terminal.rs + layout/chrome.rs",
        tokens: "sidebar / tab viewport / horizontal scroll",
    },
    Specimen {
        id: "overlay-tabs.counts",
        preview: Preview::OverlayTabs,
        category: 7,
        source: "view/overlay_surface.rs · TabBar",
        tokens: "overlay / active underline / pending / unavailable",
    },
    Specimen {
        id: "panel.bottom-empty",
        preview: Preview::Chrome(ChromePreview::BottomPanel),
        category: 7,
        source: "view/panels.rs · render_dock",
        tokens: "sidebar / header / border / empty Problems",
    },
    Specimen {
        id: "panel.problems-populated",
        preview: Preview::Chrome(ChromePreview::ProblemsPopulated),
        category: 7,
        source: "view/panels.rs · render_problems_panel",
        tokens: "grouped diagnostics / severity / selection / collapsed file",
    },
    Specimen {
        id: "panel.usages-populated",
        preview: Preview::Chrome(ChromePreview::UsagesPopulated),
        category: 7,
        source: "view/panels.rs · render_usages_panel",
        tokens: "summary / grouped locations / selection / collapsed file",
    },
    Specimen {
        id: "terminal.content",
        preview: Preview::Chrome(ChromePreview::TerminalContent),
        category: 7,
        source: "panels/terminal.rs · render_terminal_panel",
        tokens: "ansi colors / selection / link underline / block cursor",
    },
    Specimen {
        id: "panel.outline-populated",
        preview: Preview::Chrome(ChromePreview::RightPanel),
        category: 7,
        source: "view/panels.rs · render_dock",
        tokens: "sidebar / header / border / expanded tree / selected row",
    },
    Specimen {
        id: "scrollbar.vertical",
        preview: Preview::Scrollbar {
            horizontal: false,
            hovered: false,
            fits: false,
            end: false,
        },
        category: 6,
        source: "view/scrollbar.rs",
        tokens: "scrollbar track / thumb / thumb_hover",
    },
    Specimen {
        id: "scrollbar.vertical-hovered",
        preview: Preview::Scrollbar {
            horizontal: false,
            hovered: true,
            fits: false,
            end: false,
        },
        category: 6,
        source: "view/scrollbar.rs",
        tokens: "scrollbar track / thumb / thumb_hover",
    },
    Specimen {
        id: "scrollbar.vertical-end",
        preview: Preview::Scrollbar {
            horizontal: false,
            hovered: false,
            fits: false,
            end: true,
        },
        category: 6,
        source: "view/scrollbar.rs",
        tokens: "scrollbar track / thumb / thumb_hover",
    },
    Specimen {
        id: "scrollbar.content-fits",
        preview: Preview::Scrollbar {
            horizontal: false,
            hovered: false,
            fits: true,
            end: false,
        },
        category: 6,
        source: "view/scrollbar.rs",
        tokens: "scrollbar track / thumb / thumb_hover",
    },
    Specimen {
        id: "scrollbar.horizontal",
        preview: Preview::Scrollbar {
            horizontal: true,
            hovered: false,
            fits: false,
            end: false,
        },
        category: 6,
        source: "view/scrollbar.rs",
        tokens: "scrollbar track / thumb / thumb_hover",
    },
    Specimen {
        id: "scrollbar.horizontal-hovered",
        preview: Preview::Scrollbar {
            horizontal: true,
            hovered: true,
            fits: false,
            end: false,
        },
        category: 6,
        source: "view/scrollbar.rs",
        tokens: "scrollbar track / thumb / thumb_hover",
    },
    Specimen {
        id: "splitter.horizontal",
        preview: Preview::Splitter { horizontal: true },
        category: 6,
        source: "view/mod.rs · render_splitters",
        tokens: "splitter background / horizontal boundary",
    },
    Specimen {
        id: "splitter.vertical",
        preview: Preview::Splitter { horizontal: false },
        category: 6,
        source: "view/mod.rs · render_splitters",
        tokens: "splitter background / vertical boundary",
    },
    Specimen {
        id: "button.normal",
        preview: Preview::ButtonNormal,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.background / foreground / border",
    },
    Specimen {
        id: "button.hovered",
        preview: Preview::ButtonHovered,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.background_hover",
    },
    Specimen {
        id: "button.pressed",
        preview: Preview::ButtonPressed,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.background_pressed / focus_ring",
    },
    Specimen {
        id: "button.focused",
        preview: Preview::ButtonFocused,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.focus_ring",
    },
    Specimen {
        id: "button.selected",
        preview: Preview::ButtonSelected,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.background_selected (falls back to pressed)",
    },
    Specimen {
        id: "button.disabled",
        preview: Preview::ButtonDisabled,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.foreground_disabled (falls back to dim text)",
    },
    Specimen {
        id: "button.long-label",
        preview: Preview::ButtonLongLabel,
        category: 1,
        source: "view/button.rs · render_button",
        tokens: "button.* · clipping at preview width",
    },
    Specimen {
        id: "field.unfocused",
        preview: Preview::FieldUnfocused,
        category: 2,
        source: "view/controls.rs + view/text_field.rs",
        tokens: "overlay recessed wash / hairline / text",
    },
    Specimen {
        id: "field.focused",
        preview: Preview::FieldFocused,
        category: 2,
        source: "view/controls.rs + view/text_field.rs",
        tokens: "overlay accent / editor cursor",
    },
    Specimen {
        id: "field.selection",
        preview: Preview::FieldSelection,
        category: 2,
        source: "view/controls.rs + view/text_field.rs",
        tokens: "editor selection · code font",
    },
    Specimen {
        id: "checkbox.off",
        preview: Preview::Checkbox(false),
        category: 3,
        source: "view/controls.rs · render_checkbox",
        tokens: "overlay recessed wash / hairline",
    },
    Specimen {
        id: "checkbox.on",
        preview: Preview::Checkbox(true),
        category: 3,
        source: "view/controls.rs · render_checkbox",
        tokens: "overlay accent / bright text",
    },
    Specimen {
        id: "select.closed",
        preview: Preview::Select { open: false },
        category: 3,
        source: "view/controls.rs · render_select",
        tokens: "overlay recessed wash / hairline",
    },
    Specimen {
        id: "select.open-anchor",
        preview: Preview::Select { open: true },
        category: 3,
        source: "view/controls.rs · render_select",
        tokens: "overlay accent · anchor only, not popup",
    },
    Specimen {
        id: "surface.panel",
        preview: Preview::Panel,
        category: 6,
        source: "Theme::overlay · resolved palette",
        tokens: "overlay panel background / hairline",
    },
    Specimen {
        id: "surface.secondary",
        preview: Preview::Secondary,
        category: 6,
        source: "Theme::overlay · resolved palette",
        tokens: "overlay panel secondary / hairline",
    },
    Specimen {
        id: "surface.recessed",
        preview: Preview::Recessed,
        category: 6,
        source: "Theme::overlay · resolved palette",
        tokens: "overlay recessed wash / hairline",
    },
    Specimen {
        id: "icon-button.close",
        preview: Preview::IconButton,
        category: 1,
        source: "view/button.rs · render_button (glyph label)",
        tokens: "button.background / foreground / border",
    },
    Specimen {
        id: "field.multiline",
        preview: Preview::FieldMultiline,
        category: 2,
        source: "view/controls.rs + view/text_field.rs",
        tokens: "overlay recessed wash / hairline / editor selection",
    },
    Specimen {
        id: "search-field.focused",
        preview: Preview::SearchField,
        category: 2,
        source: "view/overlay_surface.rs · render_header",
        tokens: "overlay panel / accent / text",
    },
    Specimen {
        id: "choice-group.selected",
        preview: Preview::ChoiceGroup,
        category: 3,
        source: "view/controls.rs · choice_group_rects + view/button.rs",
        tokens: "button.background_selected / focus_ring",
    },
    Specimen {
        id: "disclosure.collapsed",
        preview: Preview::Disclosure { expanded: false },
        category: 3,
        source: "view/controls.rs · render_disclosure",
        tokens: "overlay text primary",
    },
    Specimen {
        id: "disclosure.expanded",
        preview: Preview::Disclosure { expanded: true },
        category: 3,
        source: "view/controls.rs · render_disclosure",
        tokens: "overlay text primary",
    },
    Specimen {
        id: "select.open-options",
        preview: Preview::SelectOptions,
        category: 3,
        source: "view/controls.rs · select_option_rects + view/button.rs",
        tokens: "overlay recessed wash / accent / button hover",
    },
    Specimen {
        id: "form-field.validation",
        preview: Preview::FormValidation,
        category: 4,
        source: "view/overlay_surface.rs · Field + render_fields",
        tokens: "overlay severity_error_text / recessed wash",
    },
    Specimen {
        id: "menu.rows-hovered",
        preview: Preview::MenuRows { hover: true },
        category: 5,
        source: "view/overlay_surface.rs · render_list",
        tokens: "overlay hover wash / keycaps / hairline",
    },
    Specimen {
        id: "menu.rows-selected",
        preview: Preview::MenuRows { hover: false },
        category: 5,
        source: "view/overlay_surface.rs · render_list",
        tokens: "overlay selection wash / keycaps / hairline",
    },
    Specimen {
        id: "list-row.kind-badge",
        preview: Preview::ListRow,
        category: 5,
        source: "view/overlay_surface.rs · RowIcon::KindBadge",
        tokens: "overlay selection wash / kind badge / text",
    },
    Specimen {
        id: "editor.selection",
        preview: Preview::Editor(EditorPreview::Selection),
        category: 8,
        source: "view/editor_text.rs · render_text_area / render_gutter",
        tokens: "editor selection / current_line / bracket_match_background / cursor",
    },
    Specimen {
        id: "editor.indent-guides",
        preview: Preview::Editor(EditorPreview::IndentGuides),
        category: 8,
        source: "view/editor_text.rs · render_line_decoration_stage",
        tokens: "editor indent_guide / current_line / syntax",
    },
    Specimen {
        id: "editor.folding",
        preview: Preview::Editor(EditorPreview::Folding),
        category: 8,
        source: "view/editor_text.rs · render_gutter_line_number / fold badge",
        tokens: "editor gutter foreground / fold chevron / fold badge",
    },
    Specimen {
        id: "editor.diagnostics",
        preview: Preview::Editor(EditorPreview::Diagnostics),
        category: 8,
        source: "view/mod.rs · diagnostic_decorations + editor_text.rs render_gutter_mark",
        tokens: "overlay severity_error / severity_warning / severity_hint / faded text",
    },
    Specimen {
        id: "editor.inlays",
        preview: Preview::Editor(EditorPreview::Inlays),
        category: 8,
        source: "view/editor_text.rs · render_lsp_annotations / render_ghost_text_stage",
        tokens: "editor ghost_text / syntax",
    },
    Specimen {
        id: "editor.find",
        preview: Preview::Editor(EditorPreview::Find),
        category: 8,
        source: "view/find_bar.rs · render + view/mod.rs find_match_decorations",
        tokens: "editor bracket_match_background / selection / overlay field",
    },
];

pub struct GalleryState {
    pub category: usize,
    pub query: EditableState<StringBuffer>,
    pub scroll: f64,
    pub compact: bool,
    pub theme_names: Vec<String>,
    pub selected_theme: usize,
    pub theme_select: crate::model::select::SelectState,
    pub focus: GalleryFocus,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum GalleryFocus {
    #[default]
    Filter,
    Theme,
    Width,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            category: 0,
            query: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            scroll: 0.0,
            compact: false,
            theme_names: Vec::new(),
            selected_theme: 0,
            theme_select: Default::default(),
            focus: GalleryFocus::Filter,
        }
    }
}

impl GalleryState {
    pub fn specimens(&self) -> Vec<&'static Specimen> {
        let query = self.query.text().to_lowercase();
        SPECIMENS
            .iter()
            .filter(|s| {
                (self.category == 0 || s.category == self.category)
                    && format!("{} {} {}", s.id, s.source, s.tokens)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect()
    }
}
