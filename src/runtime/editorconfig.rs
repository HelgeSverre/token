//! Bounded EditorConfig discovery on the ordered file worker.
use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};
use token::{
    editorconfig::{parse_layer, resolve_layers, ResolvedFilePolicy},
    util::ByteSize,
};

const FILE_LIMIT: ByteSize = ByteSize::mebibytes(1);
const CHAIN_LIMIT: ByteSize = ByteSize::mebibytes(4);

fn absolute_normalized(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut result = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            part => result.push(part.as_os_str()),
        }
    }
    result
}

pub(super) fn resolve(path: &Path) -> ResolvedFilePolicy {
    let identity = token::util::FileIdentity::resolve(path.to_path_buf());
    let path = absolute_normalized(identity.path());
    let mut dependencies = Vec::new();
    let mut diagnostics = Vec::new();
    let mut layers = Vec::new();
    let mut total = 0;
    for parent in path.ancestors().skip(1) {
        let config = parent.join(".editorconfig");
        dependencies.push(config.clone());
        let text = (|| -> std::io::Result<Option<String>> {
            let meta = match std::fs::metadata(&config) {
                Ok(meta) => meta,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            };
            if !meta.is_file()
                || meta.len() > FILE_LIMIT.as_u64()
                || total >= CHAIN_LIMIT.as_usize()
            {
                return Err(std::io::Error::other(format!(
                    "Config is not a file within the {FILE_LIMIT} limit"
                )));
            }
            let file = File::open(&config)?;
            let mut text = String::new();
            file.take(FILE_LIMIT.as_u64() + 1)
                .read_to_string(&mut text)?;
            total += text.len();
            if text.len() > FILE_LIMIT.as_usize() || total > CHAIN_LIMIT.as_usize() {
                return Err(std::io::Error::other(format!(
                    "EditorConfig chain exceeds the {CHAIN_LIMIT} limit"
                )));
            }
            Ok(Some(text))
        })();
        match text {
            Ok(Some(text)) => match parse_layer(&config, &text, &path) {
                Ok(layer) => {
                    let root = layer.root;
                    layers.push(layer);
                    if root {
                        break;
                    }
                }
                Err(error) => diagnostics.push(error),
            },
            Ok(None) => {}
            Err(error) => diagnostics.push(format!("{}: {error}", config.display())),
        }
    }
    let mut resolved = resolve_layers(path, layers);
    resolved.dependencies = dependencies;
    resolved.incomplete = !diagnostics.is_empty();
    resolved.diagnostics.extend(diagnostics);
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editorconfig_discovery_tracks_absent_ancestors_and_stops_at_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join(".editorconfig"),
            "root = true\n[*]\nindent_size = 3\n",
        )
        .unwrap();
        let nested = root.join("workspace/missing/file.rs");
        let policy = resolve(&nested);
        assert_eq!(policy.preferences.indent_size, Some(3));
        assert_eq!(policy.dependencies.len(), 3);
        assert!(policy.dependencies.iter().all(|p| p.starts_with(
            policy
                .path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
        )));
        assert!(!policy.incomplete);
        std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
        std::fs::write(
            root.join("workspace/.editorconfig"),
            "root = true\n[*]\nindent_style = space\nindent_size = 2\n",
        )
        .unwrap();
        let policy = resolve(&nested);
        assert_eq!(policy.preferences.indent_size, Some(2));
        assert_eq!(policy.dependencies.len(), 2);
    }

    #[test]
    fn editorconfig_discovery_distinguishes_unreadable_configs_and_bounded_reads() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join(".editorconfig");
        std::fs::write(&config, [0xff]).unwrap();
        let policy = resolve(&dir.path().join("file.rs"));
        assert!(policy.incomplete);
        assert!(policy.diagnostics[0].contains(".editorconfig"));
        std::fs::write(&config, "x".repeat(FILE_LIMIT.as_usize() + 1)).unwrap();
        let policy = resolve(&dir.path().join("file.rs"));
        assert!(policy.incomplete);
        assert!(policy.diagnostics[0].contains("limit"));
    }

    #[cfg(unix)]
    #[test]
    fn editorconfig_symlink_uses_physical_file_policy() {
        let dir = tempfile::tempdir().unwrap();
        let physical = dir.path().join("physical");
        let aliases = dir.path().join("aliases");
        std::fs::create_dir(&physical).unwrap();
        std::fs::create_dir(&aliases).unwrap();
        for (parent, width) in [(&physical, 2), (&aliases, 8)] {
            std::fs::write(
                parent.join(".editorconfig"),
                format!("root = true\n[*]\nindent_size = {width}\n"),
            )
            .unwrap();
        }
        let file = physical.join("file.rs");
        std::fs::write(&file, "source").unwrap();
        let alias = aliases.join("alias.rs");
        std::os::unix::fs::symlink(&file, &alias).unwrap();
        assert_eq!(resolve(&alias).preferences.indent_size, Some(2));
    }
}
