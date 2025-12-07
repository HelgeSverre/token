use token::theme::{
    Color, Theme, BUILTIN_THEMES, DEFAULT_DARK_YAML, FLEET_DARK_YAML, GITHUB_DARK_YAML,
    GITHUB_LIGHT_YAML,
};

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
