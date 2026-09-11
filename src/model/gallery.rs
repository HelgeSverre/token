//! Isolated development fixtures. Nothing here reads or writes editor settings.
use crate::editable::{EditConstraints, EditableState, StringBuffer};

pub const CATEGORIES: &[&str] = &["All", "Buttons", "Inputs", "Selection", "Surfaces"];

#[derive(Clone, Copy)]
pub enum Preview {
    ButtonNormal,
    ButtonHovered,
    ButtonPressed,
    ButtonFocused,
    ButtonSelected,
    ButtonDisabled,
    ButtonLongLabel,
    FieldUnfocused,
    FieldFocused,
    FieldSelection,
    Checkbox(bool),
    Select { open: bool },
    Panel,
    Secondary,
    Recessed,
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
        category: 4,
        source: "Theme::overlay · resolved palette",
        tokens: "overlay panel background / hairline",
    },
    Specimen {
        id: "surface.secondary",
        preview: Preview::Secondary,
        category: 4,
        source: "Theme::overlay · resolved palette",
        tokens: "overlay panel secondary / hairline",
    },
    Specimen {
        id: "surface.recessed",
        preview: Preview::Recessed,
        category: 4,
        source: "Theme::overlay · resolved palette",
        tokens: "overlay recessed wash / hairline",
    },
];

pub struct GalleryState {
    pub category: usize,
    pub query: EditableState<StringBuffer>,
    pub scroll: f64,
    pub compact: bool,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            category: 0,
            query: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            scroll: 0.0,
            compact: false,
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
