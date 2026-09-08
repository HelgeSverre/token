//! Session storage and startup coordination, off the update/render path.
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use token::session::Session;
use token::util::ByteSize;

const MAX_SESSION_SIZE: ByteSize = ByteSize::mebibytes(4);

pub(super) struct SessionStore {
    path: PathBuf,
    workspace: Option<PathBuf>,
    cwd: PathBuf,
}

impl SessionStore {
    pub fn for_model(model: &token::AppModel) -> Option<Self> {
        let directory = token::config_paths::config_dir()?.join("sessions");
        let cwd = std::env::current_dir().ok()?;
        Some(Self::new(
            directory,
            model.workspace.as_ref().map(|ws| ws.root.clone()),
            cwd,
        ))
    }

    fn new(directory: PathBuf, workspace: Option<PathBuf>, cwd: PathBuf) -> Self {
        let name = workspace.as_ref().map_or_else(
            || "default.json".to_owned(),
            |path| {
                // Stable filename across processes/toolchain versions; root equality
                // is checked on both read and write, so collisions cannot mix sessions.
                let hash = path
                    .as_os_str()
                    .as_encoded_bytes()
                    .iter()
                    .fold(0xcbf29ce484222325u64, |hash, byte| {
                        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                    });
                format!("{hash:016x}.json")
            },
        );
        Self {
            path: directory.join(name),
            workspace,
            cwd,
        }
    }

    fn load(&self) -> Result<Option<Session>> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_SESSION_SIZE.as_u64() + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= MAX_SESSION_SIZE.as_usize(),
            "Session exceeds {MAX_SESSION_SIZE}"
        );
        let session: Session = serde_json::from_slice(&bytes).context("Invalid session JSON")?;
        session.validate().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            session.workspace() == self.workspace.as_deref(),
            "Session belongs to a different workspace"
        );
        Ok(Some(session))
    }

    pub fn restore(&self, model: &mut token::AppModel) {
        let result = (|| -> Result<()> {
            let Some(session) = self.load()? else {
                return Ok(());
            };
            super::file_io::prepare_session_files(model, session.paths());
            let count = session.install(model).map_err(anyhow::Error::msg)?;
            if count == 0 {
                model
                    .ui
                    .set_status("No saved session files could be restored");
            }
            Ok(())
        })();
        if let Err(error) = result {
            tracing::warn!("Session restore: {error:#}");
            model
                .ui
                .set_status(format!("Session not restored: {error}"));
        }
    }

    pub fn save(&self, model: &token::AppModel) -> Result<()> {
        // Validate existing ownership. Corrupt files are left intact so a failed
        // startup cannot immediately overwrite the user's recovery evidence.
        self.load()?;
        let session = Session::capture(model, &self.cwd);
        session.validate().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            session.workspace() == self.workspace.as_deref(),
            "Workspace changed before session save"
        );
        let bytes = serde_json::to_vec_pretty(&session)?;
        anyhow::ensure!(
            bytes.len() <= MAX_SESSION_SIZE.as_usize(),
            "Session exceeds {MAX_SESSION_SIZE}"
        );
        let parent = self
            .path
            .parent()
            .context("Session path has no directory")?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(&self.path)
            .context("Could not replace session file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::messages::{LayoutMsg, Msg};
    use token::model::editor_area::LayoutNode;
    use token::model::{Cursor, Position, Selection, SplitDirection, ViewMode};
    use token::update::update;

    fn model() -> token::AppModel {
        let mut model = token::AppModel::new(1000, 700, 1.0);
        model.config.lsp.enabled = false;
        model.resize(1000, 700);
        model
    }

    #[test]
    fn session_round_trip_preserves_independent_panes_and_reads_current_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.txt");
        let text = format!("{}\n", "a".repeat(300)).repeat(200);
        std::fs::write(&path, &text).unwrap();
        let store = SessionStore::new(dir.path().join("sessions"), None, dir.path().into());
        let mut original = model();
        super::super::file_io::prepare_startup_files(&mut original, vec![path.clone()]);
        let first = original.editor_area.focused_group_id;
        update(
            &mut original,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        );
        let second = original.editor_area.focused_group_id;
        update(
            &mut original,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        if let LayoutNode::Split(split) = &mut original.editor_area.layout {
            split.ratios = vec![0.35, 0.65];
        }
        original.resize(1000, 700);
        for (i, group) in original
            .editor_area
            .layout
            .group_ids()
            .into_iter()
            .enumerate()
        {
            update(&mut original, Msg::Layout(LayoutMsg::FocusGroup(group)));
            let editor = original.editor_mut();
            editor.cursors = vec![Cursor::at(40 + i * 20, 13), Cursor::at(45 + i * 20, 7)];
            editor.selections = vec![
                Selection::from_anchor_head(
                    Position::new(41 + i * 20, 3),
                    Position::new(40 + i * 20, 13),
                ),
                Selection::from_anchor_head(
                    Position::new(45 + i * 20, 7),
                    Position::new(45 + i * 20, 7),
                ),
            ];
            editor.active_cursor_index = 1;
            editor.viewport.top_line = 30 + i * 20;
            editor.viewport.left_column = 5 + i;
        }
        update(&mut original, Msg::Layout(LayoutMsg::FocusGroup(second)));
        original.document_mut().buffer.insert(0, "UNSAVED SECRET");
        original.document_mut().is_modified = true;
        store.save(&original).unwrap();
        let saved = std::fs::read_to_string(&store.path).unwrap();
        assert!(!saved.contains("UNSAVED SECRET"));
        assert_eq!(store.load().unwrap().unwrap().paths(), vec![path.clone()]);
        std::fs::write(&path, text.replace('a', "b")).unwrap();
        let mut restored = model();
        store.restore(&mut restored);
        assert_eq!(restored.editor_area.documents.len(), 1);
        assert_eq!(restored.editor_area.editors.len(), 3);
        assert!(restored.document().buffer.to_string().starts_with('b'));
        assert!(!restored.document().is_modified);
        let groups = restored.editor_area.layout.group_ids();
        assert_eq!(restored.editor_area.focused_group_id, groups[1]);
        for (i, group) in groups.into_iter().enumerate() {
            let editor = &restored.editor_area.editors[&restored.editor_area.groups[&group]
                .active_editor_id()
                .unwrap()];
            assert_eq!(
                editor.cursors,
                vec![Cursor::at(40 + i * 20, 13), Cursor::at(45 + i * 20, 7)]
            );
            assert_eq!(editor.selections[0].anchor, Position::new(41 + i * 20, 3));
            assert_eq!(editor.active_cursor_index, 1);
            assert_eq!(editor.viewport.top_line, 30 + i * 20);
            assert_eq!(editor.viewport.left_column, 5 + i);
        }
        let LayoutNode::Split(split) = &restored.editor_area.layout else {
            panic!("restored split")
        };
        assert_eq!(split.ratios, vec![0.35, 0.65]);
        assert_eq!(split.direction, SplitDirection::Horizontal);
        assert!(
            matches!(&split.children[1], LayoutNode::Split(child) if child.direction == SplitDirection::Vertical)
        );
        assert_ne!(first, second);
    }

    #[test]
    fn session_missing_files_collapse_and_cli_focuses_first_successful_file() {
        let dir = tempfile::tempdir().unwrap();
        let kept = dir.path().join("kept.txt");
        let missing = dir.path().join("missing.txt");
        let cli = dir.path().join("cli.txt");
        for path in [&kept, &missing, &cli] {
            std::fs::write(path, "on disk").unwrap();
        }
        let store = SessionStore::new(dir.path().join("sessions"), None, dir.path().into());
        let mut original = model();
        super::super::file_io::prepare_startup_files(&mut original, vec![kept.clone()]);
        original.editor_mut().cursors[0] = Cursor::at(999, 999);
        original.editor_mut().clear_selection();
        update(
            &mut original,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        let duplicate = original
            .editor_area
            .focused_group()
            .unwrap()
            .active_tab()
            .unwrap()
            .id;
        super::super::file_io::prepare_startup_files(&mut original, vec![missing.clone()]);
        update(&mut original, Msg::Layout(LayoutMsg::CloseTab(duplicate)));
        store.save(&original).unwrap();
        std::fs::remove_file(&missing).unwrap();
        let mut restored = model();
        store.restore(&mut restored);
        assert!(matches!(restored.editor_area.layout, LayoutNode::Group(_)));
        assert_eq!(restored.document().file_path.as_ref(), Some(&kept));
        assert_eq!(restored.editor().cursors[0], Cursor::at(0, 7));
        assert!(!missing.exists());
        super::super::file_io::prepare_startup_files(&mut restored, vec![]);
        assert_eq!(restored.editor_area.documents.len(), 1);
        super::super::file_io::prepare_startup_files(
            &mut restored,
            vec![dir.path().into(), cli.clone(), kept.clone()],
        );
        assert_eq!(restored.document().file_path.as_ref(), Some(&cli));
        assert_eq!(restored.editor_area.documents.len(), 2);
        super::super::file_io::prepare_startup_files(&mut restored, vec![kept.clone(), cli]);
        assert_eq!(restored.document().file_path.as_ref(), Some(&kept));
    }

    #[test]
    fn session_workspace_isolation_and_invalid_files_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("file.txt");
        std::fs::write(&path, "file").unwrap();
        let mut original = model();
        original.open_workspace(root.clone());
        super::super::file_io::prepare_startup_files(&mut original, vec![path]);
        let store = SessionStore::new(root.join("sessions"), Some(root.clone()), root.clone());
        let default = SessionStore::new(root.join("sessions"), None, root.clone());
        store.save(&original).unwrap();
        assert!(default.load().unwrap().is_none());
        let bytes = std::fs::read(&store.path).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["layout"]["tabs"][0]["path"], "file.txt");
        std::fs::copy(&store.path, &default.path).unwrap();
        assert!(default.load().is_err());
        assert!(default.save(&model()).is_err());
        assert_eq!(std::fs::read(&default.path).unwrap(), bytes);
        for malformed in [
            b"not JSON".to_vec(),
            b"{\"version\":999,\"workspace\":null,\"layout\":null}".to_vec(),
            vec![b' '; MAX_SESSION_SIZE.as_usize() + 1],
        ] {
            std::fs::write(&store.path, &malformed).unwrap();
            assert!(store.load().is_err());
            assert!(store.save(&original).is_err());
            assert_eq!(std::fs::read(&store.path).unwrap(), malformed);
            let mut restored = model();
            store.restore(&mut restored);
            assert!(restored.document().file_path.is_none());
        }
    }

    #[test]
    fn session_wrapped_scroll_keeps_its_logical_anchor_across_window_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wrapped.txt");
        std::fs::write(&path, format!("{}\n", "word ".repeat(90)).repeat(100)).unwrap();
        let store = SessionStore::new(dir.path().join("sessions"), None, dir.path().into());
        let mut original = model();
        super::super::file_io::prepare_startup_files(&mut original, vec![path]);
        original.editor_mut().soft_wrap = true;
        original.resize(650, 500);
        original.editor_mut().viewport.top_line = 53;
        original.editor_mut().viewport.pixels.y.offset = original.line_height as f64 * 0.375;
        let top = original
            .editor()
            .viewport_map(original.document())
            .position_at_display_column(original.document(), 53, 0);
        store.save(&original).unwrap();
        let mut restored = model();
        store.restore(&mut restored);
        assert!(restored.editor().soft_wrap);
        let expected = restored
            .editor()
            .viewport_map(restored.document())
            .visual_line_for_position(top.line, top.column);
        assert_eq!(restored.editor().viewport.top_line, expected);
        assert_eq!(restored.editor().viewport.left_column, 0);
        assert_eq!(
            restored.editor().viewport.pixels.y.offset / restored.line_height as f64,
            0.375
        );
        assert_ne!(expected, 53);
    }

    #[test]
    fn session_csv_restores_view_but_not_cell_editing_or_untitled_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("table.csv");
        std::fs::write(&path, "name,value\na,1\nb,2").unwrap();
        let store = SessionStore::new(dir.path().join("sessions"), None, dir.path().into());
        let mut original = model();
        super::super::file_io::prepare_startup_files(&mut original, vec![path]);
        let data = token::csv::parse_csv(
            &original.document().buffer.to_string(),
            token::csv::Delimiter::Comma,
        )
        .unwrap();
        let mut csv = token::csv::CsvState::new(data, token::csv::Delimiter::Comma);
        csv.selected_cell = token::csv::CellPosition::new(1, 1);
        csv.has_header_row = true;
        csv.start_editing();
        original.editor_mut().view_mode = ViewMode::Csv(Box::new(csv));
        update(&mut original, Msg::Layout(LayoutMsg::NewTab));
        original.document_mut().buffer.insert(0, "UNTITLED SECRET");
        original.document_mut().is_modified = true;
        store.save(&original).unwrap();
        assert!(!std::fs::read_to_string(&store.path)
            .unwrap()
            .contains("UNTITLED SECRET"));
        let mut restored = model();
        store.restore(&mut restored);
        assert_eq!(restored.editor_area.editors.len(), 1);
        let csv = restored.editor().view_mode.as_csv().unwrap();
        assert_eq!(csv.selected_cell, token::csv::CellPosition::new(1, 1));
        assert!(csv.has_header_row);
        assert!(!csv.is_editing());
        assert!(!restored.document().is_modified);
    }
}
