//! Debug state dump for development diagnostics
//!
//! Serializes a snapshot of the application state to JSON for easier debugging.
//! Triggered by F7 in debug builds.

use serde::Serialize;
use std::collections::HashMap;

use token::model::{AppModel, LayoutNode, SplitDirection};

#[derive(Serialize)]
pub struct StateDump {
    pub timestamp: String,
    pub window_size: (u32, u32),
    pub line_height: usize,
    pub char_width: f32,
    pub editor_area: EditorAreaDump,
    pub ui: UiStateDump,
}

#[derive(Serialize)]
pub struct EditorAreaDump {
    pub focused_group_id: u64,
    pub documents: HashMap<u64, DocumentDump>,
    pub editors: HashMap<u64, EditorDump>,
    pub groups: HashMap<u64, GroupDump>,
    pub layout: LayoutNodeDump,
}

#[derive(Serialize)]
pub struct DocumentDump {
    pub id: u64,
    pub file_path: Option<String>,
    pub is_modified: bool,
    pub line_count: usize,
    pub char_count: usize,
    pub undo_stack_size: usize,
    pub redo_stack_size: usize,
}

#[derive(Serialize)]
pub struct EditorDump {
    pub id: u64,
    pub document_id: Option<u64>,
    pub cursors: Vec<CursorDump>,
    pub selections: Vec<SelectionDump>,
    pub active_cursor_index: usize,
    pub viewport: ViewportDump,
    pub has_multiple_cursors: bool,
}

#[derive(Serialize)]
pub struct CursorDump {
    pub line: usize,
    pub column: usize,
    pub desired_column: Option<usize>,
}

#[derive(Serialize)]
pub struct SelectionDump {
    pub anchor_line: usize,
    pub anchor_column: usize,
    pub head_line: usize,
    pub head_column: usize,
    pub is_empty: bool,
}

#[derive(Serialize)]
pub struct ViewportDump {
    pub top_line: usize,
    pub left_column: usize,
    pub visible_lines: usize,
    pub visible_columns: usize,
}

#[derive(Serialize)]
pub struct GroupDump {
    pub id: u64,
    pub tabs: Vec<TabDump>,
    pub active_tab_index: usize,
    pub rect: RectDump,
}

#[derive(Serialize)]
pub struct TabDump {
    pub id: u64,
    pub editor_id: u64,
    pub is_pinned: bool,
    pub is_preview: bool,
}

#[derive(Serialize)]
pub struct RectDump {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Serialize)]
pub enum LayoutNodeDump {
    Group(u64),
    Split {
        direction: String,
        children: Vec<LayoutNodeDump>,
        ratios: Vec<f32>,
    },
}

#[derive(Serialize)]
pub struct UiStateDump {
    pub cursor_visible: bool,
    pub status_bar_segments: usize,
}

impl StateDump {
    pub fn from_model(model: &AppModel) -> Self {
        let now = chrono_timestamp();

        Self {
            timestamp: now.clone(),
            window_size: model.window_size,
            line_height: model.line_height,
            char_width: model.char_width,
            editor_area: EditorAreaDump::from_model(model),
            ui: UiStateDump {
                cursor_visible: model.ui.cursor_visible,
                status_bar_segments: model.ui.status_bar.all_segments().count(),
            },
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
    }

    pub fn save_to_file(&self) -> std::io::Result<String> {
        let filename = format!("dumps/{}-state-dump.json", self.timestamp);

        // Ensure the dumps directory exists
        if let Some(parent) = std::path::Path::new(&filename).parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&filename, self.to_json())?;
        Ok(filename)
    }
}

impl EditorAreaDump {
    fn from_model(model: &AppModel) -> Self {
        let area = &model.editor_area;

        let documents: HashMap<u64, DocumentDump> = area
            .documents
            .iter()
            .map(|(id, doc)| {
                (
                    id.0,
                    DocumentDump {
                        id: id.0,
                        file_path: doc.file_path.as_ref().map(|p| p.display().to_string()),
                        is_modified: doc.is_modified,
                        line_count: doc.buffer.len_lines(),
                        char_count: doc.buffer.len_chars(),
                        undo_stack_size: doc.undo_stack.len(),
                        redo_stack_size: doc.redo_stack.len(),
                    },
                )
            })
            .collect();

        let editors: HashMap<u64, EditorDump> = area
            .editors
            .iter()
            .map(|(id, editor)| {
                (
                    id.0,
                    EditorDump {
                        id: id.0,
                        document_id: editor.document_id.map(|d| d.0),
                        cursors: editor
                            .cursors
                            .iter()
                            .map(|c| CursorDump {
                                line: c.line,
                                column: c.column,
                                desired_column: c.desired_column,
                            })
                            .collect(),
                        selections: editor
                            .selections
                            .iter()
                            .map(|s| SelectionDump {
                                anchor_line: s.anchor.line,
                                anchor_column: s.anchor.column,
                                head_line: s.head.line,
                                head_column: s.head.column,
                                is_empty: s.is_empty(),
                            })
                            .collect(),
                        active_cursor_index: editor.active_cursor_index,
                        viewport: ViewportDump {
                            top_line: editor.viewport.top_line,
                            left_column: editor.viewport.left_column,
                            visible_lines: editor.viewport.visible_lines,
                            visible_columns: editor.viewport.visible_columns,
                        },
                        has_multiple_cursors: editor.cursors.len() > 1,
                    },
                )
            })
            .collect();

        let groups: HashMap<u64, GroupDump> = area
            .groups
            .iter()
            .map(|(id, group)| {
                (
                    id.0,
                    GroupDump {
                        id: id.0,
                        tabs: group
                            .tabs
                            .iter()
                            .map(|t| TabDump {
                                id: t.id.0,
                                editor_id: t.editor_id.0,
                                is_pinned: t.is_pinned,
                                is_preview: t.is_preview,
                            })
                            .collect(),
                        active_tab_index: group.active_tab_index,
                        rect: RectDump {
                            x: group.rect.x,
                            y: group.rect.y,
                            width: group.rect.width,
                            height: group.rect.height,
                        },
                    },
                )
            })
            .collect();

        Self {
            focused_group_id: area.focused_group_id.0,
            documents,
            editors,
            groups,
            layout: layout_node_dump(&area.layout),
        }
    }
}

fn layout_node_dump(node: &LayoutNode) -> LayoutNodeDump {
    match node {
        LayoutNode::Group(id) => LayoutNodeDump::Group(id.0),
        LayoutNode::Split(container) => LayoutNodeDump::Split {
            direction: match container.direction {
                SplitDirection::Horizontal => "horizontal".to_string(),
                SplitDirection::Vertical => "vertical".to_string(),
            },
            children: container.children.iter().map(layout_node_dump).collect(),
            ratios: container.ratios.clone(),
        },
    }
}

fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    let days_since_epoch = secs / 86400;
    let secs_today = secs % 86400;

    let hours = secs_today / 3600;
    let minutes = (secs_today % 3600) / 60;
    let seconds = secs_today % 60;

    let (year, month, day) = days_to_ymd(days_since_epoch as i64);

    format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m, d)
}
