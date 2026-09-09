//! Undoable cleanup before snapshot capture. Serialization never edits bytes.
use super::text_edits::{apply_planned_edits, EditCarets, PlannedEdit};
use crate::{
    commands::Cmd,
    model::{AppModel, Document, DocumentId, DocumentTextSettings},
};

pub(super) fn apply(
    model: &mut AppModel,
    id: DocumentId,
    settings: DocumentTextSettings,
) -> Option<Cmd> {
    if model
        .editor_area
        .editors
        .values()
        .any(|editor| editor.document_id == Some(id) && !editor.is_plain_text_mode())
    {
        return None;
    }
    let document = model.editor_area.documents.get(&id)?;
    let edits = plan(document, settings);
    if edits.is_empty() {
        return None;
    }
    apply_planned_edits(model, id, &edits, EditCarets::Preserve)
}

fn plan(document: &Document, settings: DocumentTextSettings) -> Vec<PlannedEdit> {
    let trim = settings.trim_trailing_whitespace == Some(true);
    if !trim && settings.end_of_line.is_none() && settings.insert_final_newline.is_none() {
        return Vec::new();
    }
    let mut eof = document.buffer.len_chars();
    if settings.insert_final_newline == Some(false) {
        let mut chars = document.buffer.chars_at(eof);
        while let Some(ch) = chars.prev() {
            if !matches!(ch, '\r' | '\n') && !(trim && matches!(ch, ' ' | '\t')) {
                break;
            }
            eof -= 1;
        }
    }
    let preferred = settings
        .end_of_line
        .unwrap_or(document.detected_line_ending)
        .as_str();
    let mut edits = Vec::new();
    let mut offset = 0;
    for (index, line) in document.buffer.lines().enumerate() {
        let raw: std::borrow::Cow<'_, str> = line.into();
        let body = crate::util::text::trim_line_ending(&raw);
        let ending = &raw[body.len()..];
        let body = if trim {
            body.trim_end_matches([' ', '\t'])
        } else {
            body
        };
        let mut replacement = if !ending.is_empty() {
            if settings.insert_final_newline == Some(false) && offset + line.len_chars() > eof {
                ""
            } else {
                settings.end_of_line.map_or(ending, |e| e.as_str())
            }
        } else {
            ""
        };
        if index + 1 == document.line_count()
            && settings.insert_final_newline == Some(true)
            && !body.is_empty()
            && !(offset == 0 && body == "\u{feff}")
        {
            replacement = preferred;
        }
        let removed = &raw[body.len()..];
        if removed != replacement {
            edits.push(PlannedEdit {
                start: offset + body.chars().count(),
                deleted: removed.to_owned(),
                inserted: replacement.to_owned(),
            });
        }
        offset += line.len_chars();
    }
    edits.reverse();
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LineEnding, TextPreferences};
    fn cleaned(text: &str, preferences: TextPreferences) -> (String, usize) {
        let mut document = Document::with_text(text);
        let edits = plan(
            &document,
            DocumentTextSettings::resolve(preferences, Default::default()),
        );
        for edit in &edits {
            document.buffer.remove(edit.start..edit.end());
            document.buffer.insert(edit.start, &edit.inserted);
        }
        (document.buffer.to_string(), edits.len())
    }

    #[test]
    fn save_cleanup_combines_trimming_endings_and_eof_without_overlapping_edits() {
        for (text, final_newline, expected) in [
            ("a \r\nb\t\rc  \n", Some(true), "a\r\nb\r\nc\r\n"),
            ("a \r\n \t\r\n\t", Some(false), "a"),
            ("a\n\n", Some(true), "a\r\n\r\n"),
            ("a\n\n", Some(false), "a"),
            ("", Some(true), ""),
            (" \t", Some(true), ""),
            ("\u{feff}", Some(true), "\u{feff}"),
            ("\u{feff}a\t", Some(true), "\u{feff}a\r\n"),
        ] {
            let preferences = TextPreferences {
                end_of_line: Some(LineEnding::Crlf),
                trim_trailing_whitespace: Some(true),
                insert_final_newline: final_newline,
                ..Default::default()
            };
            assert_eq!(cleaned(text, preferences).0, expected);
            assert_eq!(
                cleaned(expected, preferences).1,
                0,
                "cleanup must be idempotent"
            );
        }
    }

    #[test]
    fn save_cleanup_false_and_absent_preserve_whitespace_and_blank_lines() {
        assert_eq!(
            cleaned("a \t\r\n\r", Default::default()),
            ("a \t\r\n\r".into(), 0)
        );
        let preferences = TextPreferences {
            trim_trailing_whitespace: Some(false),
            insert_final_newline: Some(true),
            ..Default::default()
        };
        assert_eq!(cleaned("a \t", preferences).0, "a \t\n");
        assert_eq!(cleaned("a\r\n\r\n", preferences).1, 0);
        assert_eq!(
            cleaned(
                "a\u{a0}",
                TextPreferences {
                    trim_trailing_whitespace: Some(true),
                    ..Default::default()
                }
            )
            .1,
            0
        );
    }
}
