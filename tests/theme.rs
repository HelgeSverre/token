use token::theme::{
    list_available_themes, load_theme, Color, Theme, ThemeSource, BUILTIN_THEMES,
    DEFAULT_DARK_YAML, FLEET_DARK_YAML, GITHUB_DARK_YAML, GITHUB_LIGHT_YAML,
};

/// WCAG contrast ratio, computed independently of `theme.rs`'s internal
/// helper so this check verifies the actual resolved colors, not the
/// derivation math that produced them.
fn contrast_ratio(a: Color, b: Color) -> f64 {
    fn luminance(c: Color) -> f64 {
        fn chan(v: u8) -> f64 {
            let v = v as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * chan(c.r) + 0.7152 * chan(c.g) + 0.0722 * chan(c.b)
    }
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn test_color_from_hex_6() {
    let color = Color::from_hex("#1E1E1E").unwrap();
    assert_eq!(color.r, 0x1E);
    assert_eq!(color.g, 0x1E);
    assert_eq!(color.b, 0x1E);
    assert_eq!(color.a, 255);
}

#[test]
fn test_color_from_hex_8() {
    let color = Color::from_hex("#1E1E1E80").unwrap();
    assert_eq!(color.r, 0x1E);
    assert_eq!(color.g, 0x1E);
    assert_eq!(color.b, 0x1E);
    assert_eq!(color.a, 0x80);
}

#[test]
fn test_color_from_hex_3() {
    let color = Color::from_hex("#F50").unwrap();
    assert_eq!(color.r, 0xFF);
    assert_eq!(color.g, 0x55);
    assert_eq!(color.b, 0x00);
    assert_eq!(color.a, 255);
}

#[test]
fn test_color_from_hex_4() {
    let color = Color::from_hex("#F508").unwrap();
    assert_eq!(color.r, 0xFF);
    assert_eq!(color.g, 0x55);
    assert_eq!(color.b, 0x00);
    assert_eq!(color.a, 0x88);
}

#[test]
fn test_color_to_argb_u32() {
    let color = Color::rgb(0x1E, 0x1E, 0x1E);
    assert_eq!(color.to_argb_u32(), 0xFF1E1E1E);
}

#[test]
fn test_default_theme() {
    let theme = Theme::default_dark();
    assert_eq!(theme.name, "Default Dark");
    assert_eq!(theme.editor.background.to_argb_u32(), 0xFF1E1E1E);
}

#[test]
fn test_default_dark_yaml_parses() {
    let theme = Theme::from_yaml(DEFAULT_DARK_YAML).unwrap();
    assert_eq!(theme.name, "Default Dark");
}

#[test]
fn test_parse_fleet_dark() {
    let theme = Theme::from_yaml(FLEET_DARK_YAML).unwrap();
    assert_eq!(theme.name, "Fleet Dark");
    assert_eq!(theme.editor.background.r, 0x18);
}

#[test]
fn test_parse_github_dark() {
    let theme = Theme::from_yaml(GITHUB_DARK_YAML).unwrap();
    assert_eq!(theme.name, "GitHub Dark");
    assert_eq!(theme.editor.background.r, 0x0D);
}

#[test]
fn test_parse_github_light() {
    let theme = Theme::from_yaml(GITHUB_LIGHT_YAML).unwrap();
    assert_eq!(theme.name, "GitHub Light");
    assert_eq!(theme.editor.background.r, 0xFF);
}

#[test]
fn test_from_builtin() {
    let theme = Theme::from_builtin("fleet-dark").unwrap();
    assert_eq!(theme.name, "Fleet Dark");

    let result = Theme::from_builtin("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_all_builtin_themes_parse() {
    for builtin in BUILTIN_THEMES {
        let theme = Theme::from_yaml(builtin.yaml)
            .unwrap_or_else(|e| panic!("Failed to parse theme '{}': {}", builtin.id, e));
        assert!(
            !theme.name.is_empty(),
            "Theme '{}' has empty name",
            builtin.id
        );
    }
}

// ============================================================================
// Theme loading tests
// ============================================================================

#[test]
fn test_load_theme_builtin() {
    let theme = load_theme("default-dark").unwrap();
    assert_eq!(theme.name, "Default Dark");

    let theme = load_theme("fleet-dark").unwrap();
    assert_eq!(theme.name, "Fleet Dark");
}

#[test]
fn test_load_theme_nonexistent() {
    let result = load_theme("nonexistent-theme-that-does-not-exist");
    assert!(result.is_err());
}

#[test]
fn test_list_available_themes_includes_builtins() {
    let themes = list_available_themes();

    // Should have at least the 4 builtin themes
    assert!(themes.len() >= 4);

    // Check that all builtins are present
    let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"default-dark"));
    assert!(ids.contains(&"fleet-dark"));
    assert!(ids.contains(&"github-dark"));
    assert!(ids.contains(&"github-light"));
}

#[test]
fn test_list_available_themes_has_builtin_source() {
    let themes = list_available_themes();

    // Find a builtin theme and check its source
    let default_dark = themes.iter().find(|t| t.id == "default-dark").unwrap();
    assert_eq!(default_dark.source, ThemeSource::Builtin);
    assert_eq!(default_dark.name, "Default Dark");
}

#[test]
fn test_list_available_themes_names_populated() {
    let themes = list_available_themes();

    for theme in &themes {
        assert!(
            !theme.name.is_empty(),
            "Theme '{}' has empty name",
            theme.id
        );
    }
}

// ============================================================================
// overlay-surface.md Phase 1: overlay palette contrast + fallback derivation
// ============================================================================

#[test]
fn all_bundled_themes_clear_overlay_text_contrast() {
    // A light-theme (or any theme) regression in the overlay palette must be
    // caught mechanically, not by eye — every text/keycap/severity-text
    // color must clear 4.5:1 against its actual paint ground for all 9
    // bundled themes.
    for builtin in BUILTIN_THEMES {
        let theme = Theme::from_yaml(builtin.yaml)
            .unwrap_or_else(|e| panic!("theme '{}' failed to parse: {}", builtin.id, e));
        let overlay = &theme.overlay;
        let panel = overlay.panel_background;

        let checks: &[(&str, Color, Color)] = &[
            ("text_primary", overlay.text_primary, panel),
            ("text_secondary", overlay.text_secondary, panel),
            ("text_dim", overlay.text_dim, panel),
            ("keycap_fg", overlay.keycap_fg, overlay.keycap_bg),
            ("severity_error_text", overlay.severity_error_text, panel),
            (
                "severity_warning_text",
                overlay.severity_warning_text,
                panel,
            ),
            ("severity_info_text", overlay.severity_info_text, panel),
            ("severity_hint_text", overlay.severity_hint_text, panel),
        ];

        for (label, fg, ground) in checks {
            let ratio = contrast_ratio(*fg, *ground);
            assert!(
                ratio >= 4.5,
                "theme '{}': overlay.{} contrast {:.2}:1 is below the 4.5:1 minimum",
                builtin.id,
                label,
                ratio
            );
        }
    }
}

#[test]
fn overlay_theme_with_no_new_keys_still_derives_legible_colors() {
    // A user theme predating overlay-surface.md provides none of the new
    // `overlay.*` keys. The derivation fallbacks must still produce a
    // usable, contrast-clearing palette.
    let yaml = r##"
version: 1
name: Legacy User Theme
ui:
  editor:
    background: "#1E1E1E"
    foreground: "#D4D4D4"
    current_line_background: "#2A2A2A"
    cursor_color: "#FFFFFF"
  gutter:
    background: "#1E1E1E"
    foreground: "#666666"
    foreground_active: "#D4D4D4"
  status_bar:
    background: "#007ACC"
    foreground: "#FFFFFF"
"##;
    let theme = Theme::from_yaml(yaml).expect("legacy theme without overlay.* keys must parse");
    let overlay = &theme.overlay;

    // Accent falls back to the status bar background.
    assert_eq!(
        overlay.accent.to_argb_u32(),
        theme.status_bar.background.to_argb_u32()
    );
    assert!(contrast_ratio(overlay.text_primary, overlay.panel_background) >= 7.0);
    assert!(contrast_ratio(overlay.text_secondary, overlay.panel_background) >= 4.5);
    assert!(contrast_ratio(overlay.keycap_fg, overlay.keycap_bg) >= 4.5);
}

#[test]
fn bundled_themes_have_a_real_derived_accent_and_wash() {
    // Every bundled theme except default-dark (which sets `overlay.accent`
    // explicitly) relies on the `status_bar.background` fallback. That
    // fallback must not collapse onto `panel_background` — a selection row
    // needs a visible fill, and `accent_bright` needs to be distinguishable
    // from body text.
    for builtin in BUILTIN_THEMES {
        let theme = Theme::from_yaml(builtin.yaml)
            .unwrap_or_else(|e| panic!("theme '{}' failed to parse: {}", builtin.id, e));
        let overlay = &theme.overlay;
        assert!(
            contrast_ratio(overlay.accent, overlay.panel_background) >= 1.5,
            "theme '{}': accent is indistinguishable from panel_background",
            builtin.id
        );
        assert_ne!(
            overlay.selection_wash.to_argb_u32() & 0x00FF_FFFF,
            overlay.panel_background.to_argb_u32() & 0x00FF_FFFF,
            "theme '{}': selection_wash has the same RGB as panel_background \
             (a selected row would have no visible fill)",
            builtin.id
        );
        assert_ne!(
            overlay.accent_bright.to_argb_u32(),
            overlay.text_primary.to_argb_u32(),
            "theme '{}': accent_bright matches text_primary (match highlighting \
             would be indistinguishable from body text)",
            builtin.id
        );
    }
}

#[test]
fn bundled_themes_derive_a_non_degenerate_text_ramp() {
    // text_primary/secondary/dim must not all resolve to the same color —
    // the whole point of the ramp is a visible hierarchy between section
    // headers (dim), body text (secondary), and emphasized text (primary).
    for builtin in BUILTIN_THEMES {
        let theme = Theme::from_yaml(builtin.yaml)
            .unwrap_or_else(|e| panic!("theme '{}' failed to parse: {}", builtin.id, e));
        let overlay = &theme.overlay;
        assert_ne!(
            overlay.text_primary.to_argb_u32(),
            overlay.text_dim.to_argb_u32(),
            "theme '{}': text_primary and text_dim resolved identically",
            builtin.id
        );
    }
}

#[test]
fn status_bar_border_falls_back_to_gutter_border() {
    // A user theme predating the `status_bar.border` key must resolve the
    // border to the gutter border color so the bar still separates from the
    // editor area above it.
    let yaml = r##"
version: 1
name: Legacy User Theme
ui:
  editor:
    background: "#1E1E1E"
    foreground: "#D4D4D4"
    current_line_background: "#2A2A2A"
    cursor_color: "#FFFFFF"
  gutter:
    background: "#1E1E1E"
    foreground: "#666666"
    foreground_active: "#D4D4D4"
    border_color: "#313438"
  status_bar:
    background: "#007ACC"
    foreground: "#FFFFFF"
"##;
    let theme = Theme::from_yaml(yaml).expect("legacy theme without status_bar.border must parse");
    assert_eq!(
        theme.status_bar.border.to_argb_u32(),
        theme.gutter.border_color.to_argb_u32()
    );
}

#[test]
fn bundled_themes_have_a_visible_status_bar_border() {
    // The border exists to separate the status bar from the editor area; a
    // border equal to the bar background is invisible (nord's gutter border
    // matches its status bar background, which is why it sets an explicit
    // `status_bar.border`).
    for builtin in BUILTIN_THEMES {
        let theme = Theme::from_yaml(builtin.yaml)
            .unwrap_or_else(|e| panic!("theme '{}' failed to parse: {}", builtin.id, e));
        assert_ne!(
            theme.status_bar.border.to_argb_u32(),
            theme.status_bar.background.to_argb_u32(),
            "theme '{}': status_bar.border equals status_bar.background (invisible border)",
            builtin.id
        );
    }
}
