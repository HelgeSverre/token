//! Per-property overlay semantics plus pinned upstream core compatibility cases.
use std::path::{Path, PathBuf};
use token::{
    editorconfig::{parse_layer, resolve_layers, ResolvedFilePolicy},
    model::{IndentStyle, LineEnding, TextPreferences},
};

fn resolve(text: &str) -> ResolvedFilePolicy {
    let path = PathBuf::from("/project/source.rs");
    let layer = parse_layer(Path::new("/project/.editorconfig"), text, &path).unwrap();
    resolve_layers(path, vec![layer])
}

#[test]
fn editorconfig_pinned_upstream_core_cases_match_actual_adapter_properties() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editorconfig/core");
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/editorconfig/core/cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let rooted = |value: &str| {
            value
                .strip_prefix("@fixtures/")
                .map_or_else(|| PathBuf::from(value), |suffix| root.join(suffix))
        };
        let target = rooted(case["target"].as_str().unwrap());
        let config = rooted(case["config"].as_str().unwrap());
        let mut layers = Vec::new();
        for parent in target.ancestors().skip(1) {
            let path = parent.join(&config);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let layer = parse_layer(&path, &text, &target).unwrap();
                let root = layer.root;
                layers.push(layer);
                if root {
                    break;
                }
            }
            if config.is_absolute() || parent == root {
                break;
            }
        }
        let policy = resolve_layers(target, layers);
        let mut lines: Vec<_> = policy
            .properties
            .iter()
            .map(|p| format!("{}={}", p.name, p.value))
            .collect();
        if case["sorted"] == true {
            lines.sort();
        }
        let output = format!("{}\n", lines.join("\n"));
        assert!(
            case["expected"].as_array().unwrap().iter().any(|pattern| {
                // CMake accepts a literal backslash as a singleton character class.
                let pattern = pattern.as_str().unwrap().replace(r"[\]", r"\\");
                regex::Regex::new(&pattern).unwrap().is_match(&output)
            }),
            "{}: {output:?}; expected {}",
            case["name"],
            case["expected"]
        );
    }
}

#[test]
fn editorconfig_unset_false_and_tab_fallback_remain_distinct_from_user_defaults() {
    let user = TextPreferences {
        indent_style: Some(IndentStyle::Space),
        indent_size: Some(3),
        tab_width: Some(8),
        trim_trailing_whitespace: Some(true),
        insert_final_newline: Some(true),
        ..Default::default()
    };
    let policy = resolve("root = true\n[*]\nindent_style = TAB\nindent_size = tab\ntrim_trailing_whitespace = false\ninsert_final_newline = unset\nend_of_line = CR\n");
    let settings = policy.settings(user);
    assert_eq!(settings.indent_style, IndentStyle::Tab);
    assert_eq!(settings.indent_size, 8);
    assert_eq!(settings.tabs.width(), 8);
    assert_eq!(settings.trim_trailing_whitespace, Some(false));
    assert_eq!(settings.insert_final_newline, Some(true));
    assert_eq!(settings.end_of_line, Some(LineEnding::Cr));
    assert!(policy.properties.iter().all(|p| p
        .source
        .as_ref()
        .is_some_and(|(path, _)| path == Path::new("/project/.editorconfig"))));
}

#[test]
fn editorconfig_invalid_widths_fall_back_without_hiding_valid_rules() {
    let policy = resolve("[*]\nindent_size = 0\ntab_width = 999999999999999999999999\nend_of_line = crlf\ncharset = latin1\n");
    assert_eq!(policy.diagnostics.len(), 3);
    assert_eq!(policy.settings(Default::default()).tabs.width(), 4);
    assert_eq!(policy.preferences.end_of_line, Some(LineEnding::Crlf));
    assert!(!policy.incomplete);
}
