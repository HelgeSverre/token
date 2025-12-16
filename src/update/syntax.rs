//! Syntax highlighting update handlers
//!
//! Handles syntax-related messages for the Elm architecture.

use crate::commands::Cmd;
#[cfg(debug_assertions)]
use crate::debug_overlay::SyntaxEventType;
use crate::messages::SyntaxMsg;
use crate::model::AppModel;

/// Debounce delay in milliseconds
/// Kept short since we preserve old highlights during the wait (no FOUC)
pub const SYNTAX_DEBOUNCE_MS: u64 = 30;

/// Handle syntax-related messages
pub fn update_syntax(model: &mut AppModel, msg: SyntaxMsg) -> Option<Cmd> {
    match msg {
        SyntaxMsg::ParseReady {
            document_id,
            revision,
        } => {
            tracing::debug!(
                "update_syntax: ParseReady received for doc={} rev={}",
                document_id.0,
                revision
            );

            // Check if document still exists and revision matches
            let doc = match model.editor_area.documents.get(&document_id) {
                Some(d) => d,
                None => {
                    tracing::warn!(
                        "update_syntax: Document {} not found for ParseReady",
                        document_id.0
                    );
                    return None;
                }
            };

            // Skip if document has been edited since debounce started
            if doc.revision != revision {
                tracing::debug!(
                    "Skipping stale parse request: doc revision {} != request revision {}",
                    doc.revision,
                    revision
                );

                #[cfg(debug_assertions)]
                if let Some(ref mut overlay) = model.debug_overlay {
                    overlay.record_syntax_event(
                        SyntaxEventType::ParseStale,
                        document_id.0,
                        revision,
                        format!("ParseReady stale (doc rev {})", doc.revision),
                    );
                }

                return None;
            }

            // Snapshot the document content for parsing
            let source = doc.buffer.to_string();
            let language = doc.language;

            #[cfg(debug_assertions)]
            if let Some(ref mut overlay) = model.debug_overlay {
                overlay.record_syntax_event(
                    SyntaxEventType::ParseStarted,
                    document_id.0,
                    revision,
                    format!("ParseReady → RunParse ({} chars)", source.len()),
                );
            }

            Some(Cmd::RunSyntaxParse {
                document_id,
                revision,
                source,
                language,
            })
        }

        SyntaxMsg::ParseCompleted {
            document_id,
            revision,
            highlights,
        } => {
            tracing::debug!(
                "update_syntax: ParseCompleted received for doc={} rev={}",
                document_id.0,
                revision
            );

            // Check if document still exists and revision matches
            let doc = match model.editor_area.documents.get_mut(&document_id) {
                Some(d) => d,
                None => {
                    tracing::warn!(
                        "update_syntax: Document {} not found, discarding ParseCompleted",
                        document_id.0
                    );
                    return None;
                }
            };

            // Skip if document has been edited since parse started
            if doc.revision != revision {
                tracing::debug!(
                    "Discarding stale parse results: doc revision {} != result revision {}",
                    doc.revision,
                    revision
                );

                #[cfg(debug_assertions)]
                if let Some(ref mut overlay) = model.debug_overlay {
                    overlay.record_syntax_event(
                        SyntaxEventType::ParseStale,
                        document_id.0,
                        revision,
                        format!("ParseCompleted stale (doc rev {})", doc.revision),
                    );
                }

                return None;
            }

            // Store the highlights and record debug info
            #[cfg(debug_assertions)]
            let (line_count, token_count) = {
                let lc = highlights.lines.len();
                let tc: usize = highlights.lines.values().map(|lh| lh.tokens.len()).sum();
                (lc, tc)
            };

            doc.syntax_highlights = Some(highlights);
            tracing::debug!(
                "Applied syntax highlights for document {:?}, revision {}",
                document_id,
                revision
            );

            #[cfg(debug_assertions)]
            if let Some(ref mut overlay) = model.debug_overlay {
                overlay.record_syntax_event(
                    SyntaxEventType::HighlightsApplied,
                    document_id.0,
                    revision,
                    format!("{} lines, {} tokens", line_count, token_count),
                );
            }

            Some(Cmd::Redraw)
        }

        SyntaxMsg::LanguageChanged {
            document_id,
            language,
        } => {
            let doc = model.editor_area.documents.get_mut(&document_id)?;

            // Update language and clear old highlights
            doc.language = language;
            doc.syntax_highlights = None;

            // Trigger a new parse
            let revision = doc.revision;

            #[cfg(debug_assertions)]
            if let Some(ref mut overlay) = model.debug_overlay {
                overlay.record_syntax_event(
                    SyntaxEventType::HighlightsCleared,
                    document_id.0,
                    revision,
                    format!("Language changed to {:?}", language),
                );
            }

            Some(Cmd::DebouncedSyntaxParse {
                document_id,
                revision,
                delay_ms: 0, // Immediate parse on language change
            })
        }
    }
}

/// Schedule a syntax parse for a document (call after document edits)
///
/// This returns a `Cmd::DebouncedSyntaxParse` that should be included
/// in the command returned from the document edit handler.
pub fn schedule_syntax_parse(
    model: &mut AppModel,
    document_id: crate::model::editor_area::DocumentId,
) -> Option<Cmd> {
    let doc = model.editor_area.documents.get(&document_id)?;

    // Skip plain text documents
    if !doc.language.has_highlighting() {
        return None;
    }

    let revision = doc.revision;

    #[cfg(debug_assertions)]
    if let Some(ref mut overlay) = model.debug_overlay {
        overlay.record_syntax_event(
            SyntaxEventType::ParseScheduled,
            document_id.0,
            revision,
            format!("Debounce {}ms", SYNTAX_DEBOUNCE_MS),
        );
    }

    Some(Cmd::DebouncedSyntaxParse {
        document_id,
        revision,
        delay_ms: SYNTAX_DEBOUNCE_MS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{LanguageId, ParserState, SyntaxHighlights};

    #[test]
    fn test_schedule_syntax_parse_for_supported_language() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        // Get the document ID from the model
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set language to something with highlighting
        model
            .editor_area
            .documents
            .get_mut(&doc_id)
            .unwrap()
            .language = LanguageId::Rust;

        let cmd = schedule_syntax_parse(&mut model, doc_id);
        assert!(cmd.is_some(), "Should return a command for Rust files");

        if let Some(Cmd::DebouncedSyntaxParse {
            document_id,
            revision: _,
            delay_ms,
        }) = cmd
        {
            assert_eq!(document_id, doc_id);
            assert_eq!(delay_ms, SYNTAX_DEBOUNCE_MS);
        } else {
            panic!("Expected DebouncedSyntaxParse command");
        }
    }

    #[test]
    fn test_schedule_syntax_parse_skips_plain_text() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");
        // Default is PlainText, so no parse should be scheduled
        let cmd = schedule_syntax_parse(&mut model, doc_id);
        assert!(cmd.is_none(), "Should not schedule parse for plain text");
    }

    #[test]
    fn test_parse_ready_triggers_run_syntax_parse() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set up a Rust document
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.language = LanguageId::Rust;
            doc.buffer = ropey::Rope::from("fn main() {}");
            doc.revision = 5;
        }

        let cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseReady {
                document_id: doc_id,
                revision: 5,
            },
        );

        assert!(cmd.is_some(), "ParseReady should produce a command");
        if let Some(Cmd::RunSyntaxParse {
            document_id,
            revision,
            source,
            language,
        }) = cmd
        {
            assert_eq!(document_id, doc_id);
            assert_eq!(revision, 5);
            assert_eq!(source, "fn main() {}");
            assert_eq!(language, LanguageId::Rust);
        } else {
            panic!("Expected RunSyntaxParse command");
        }
    }

    #[test]
    fn test_parse_ready_skips_stale_revision() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set up document with revision 10
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.language = LanguageId::Rust;
            doc.revision = 10;
        }

        // Send ParseReady with old revision 5
        let cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseReady {
                document_id: doc_id,
                revision: 5,
            },
        );

        assert!(cmd.is_none(), "Stale ParseReady should produce no command");
    }

    #[test]
    fn test_parse_completed_stores_highlights() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set up document with specific revision
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.language = LanguageId::Rust;
            doc.revision = 7;
        }

        // Create mock highlights
        let highlights = SyntaxHighlights::new(LanguageId::Rust, 7);

        let cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseCompleted {
                document_id: doc_id,
                revision: 7,
                highlights: highlights.clone(),
            },
        );

        // Should trigger redraw
        assert!(matches!(cmd, Some(Cmd::Redraw)));

        // Highlights should be stored
        let doc = model.editor_area.documents.get(&doc_id).unwrap();
        assert!(doc.syntax_highlights.is_some());
    }

    #[test]
    fn test_parse_completed_discards_stale_results() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set up document with revision 10
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.language = LanguageId::Rust;
            doc.revision = 10;
        }

        // Send completed results for old revision 5
        let highlights = SyntaxHighlights::new(LanguageId::Rust, 5);
        let cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseCompleted {
                document_id: doc_id,
                revision: 5,
                highlights,
            },
        );

        // Should produce no command (stale)
        assert!(cmd.is_none(), "Stale results should be discarded");

        // Highlights should NOT be stored
        let doc = model.editor_area.documents.get(&doc_id).unwrap();
        assert!(doc.syntax_highlights.is_none());
    }

    #[test]
    fn test_full_syntax_update_flow() {
        // Simulate the complete flow: edit -> schedule -> ready -> parse -> completed
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set up a JavaScript document
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.language = LanguageId::JavaScript;
            doc.buffer = ropey::Rope::from("let x = 1;");
            doc.revision = 1;
        }

        // Step 1: Schedule a parse (this would be called after an edit)
        let schedule_cmd = schedule_syntax_parse(&mut model, doc_id);
        assert!(schedule_cmd.is_some());

        // Step 2: ParseReady comes in (after debounce)
        let run_cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseReady {
                document_id: doc_id,
                revision: 1,
            },
        );
        assert!(matches!(run_cmd, Some(Cmd::RunSyntaxParse { .. })));

        // Step 3: Simulate actual parsing
        let mut parser_state = ParserState::new();
        let source = model.document().buffer.to_string();
        let highlights =
            parser_state.parse_and_highlight(&source, LanguageId::JavaScript, doc_id, 1);

        // Step 4: ParseCompleted comes back
        let redraw_cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseCompleted {
                document_id: doc_id,
                revision: 1,
                highlights,
            },
        );
        assert!(matches!(redraw_cmd, Some(Cmd::Redraw)));

        // Verify highlights are stored
        let doc = model.editor_area.documents.get(&doc_id).unwrap();
        assert!(doc.syntax_highlights.is_some());
        assert!(!doc.syntax_highlights.as_ref().unwrap().lines.is_empty());
    }

    #[test]
    fn test_insert_newline_at_start_clears_and_reparses() {
        use crate::messages::{DocumentMsg, Msg};
        use crate::update::update;

        // Create model with Rust content
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        let doc_id = model.document().id.expect("Document should have an ID");

        // Set up a Rust document with some code
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.language = LanguageId::Rust;
            doc.buffer = ropey::Rope::from("fn main() {}");
            doc.revision = 0;
        }

        // Parse initially
        let mut parser_state = ParserState::new();
        let source = model.document().buffer.to_string();
        let highlights = parser_state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 0);

        // Store initial highlights
        {
            let doc = model.editor_area.documents.get_mut(&doc_id).unwrap();
            doc.syntax_highlights = Some(highlights);
        }

        // Verify we have highlights on line 0
        assert!(
            model
                .document()
                .syntax_highlights
                .as_ref()
                .unwrap()
                .lines
                .contains_key(&0),
            "Should have highlights on line 0 before edit"
        );

        // Cursor is at start (0, 0)
        assert_eq!(model.editor().primary_cursor().line, 0);
        assert_eq!(model.editor().primary_cursor().column, 0);

        // Insert newline at start of file
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

        // After the edit:
        // 1. Document should have a new revision
        assert_eq!(
            model.document().revision,
            1,
            "Revision should be incremented"
        );

        // 2. Old highlights should be preserved until new ones arrive (prevents FOUC)
        assert!(
            model.document().syntax_highlights.is_some(),
            "Old highlights should be preserved after edit until new ones arrive"
        );

        // 3. The command should include a syntax parse request
        let cmd = cmd.expect("Should return a command");
        let has_syntax_parse = match &cmd {
            Cmd::Batch(cmds) => cmds
                .iter()
                .any(|c| matches!(c, Cmd::DebouncedSyntaxParse { revision: 1, .. })),
            Cmd::DebouncedSyntaxParse { revision: 1, .. } => true,
            _ => false,
        };
        assert!(
            has_syntax_parse,
            "Should schedule syntax parse with new revision"
        );

        // 4. Buffer should have newline at start
        assert_eq!(
            model.document().buffer.to_string(),
            "\nfn main() {}",
            "Buffer should have newline at start"
        );

        // 5. Cursor should be on line 1, column 0
        assert_eq!(model.editor().primary_cursor().line, 1);
        assert_eq!(model.editor().primary_cursor().column, 0);

        // Now simulate the parse completing
        // Use a fresh parser state (like the actual worker thread does)
        let mut fresh_parser_state = ParserState::new();
        let new_source = model.document().buffer.to_string();
        let new_highlights =
            fresh_parser_state.parse_and_highlight(&new_source, LanguageId::Rust, doc_id, 1);

        // Verify the new highlights have tokens on line 1 (where fn main is now)
        assert!(
            new_highlights.lines.contains_key(&1),
            "New highlights should have tokens on line 1"
        );
        assert!(
            !new_highlights.lines.contains_key(&0)
                || new_highlights.lines.get(&0).unwrap().tokens.is_empty(),
            "Line 0 should be empty or have no tokens (it's just a newline)"
        );

        // Apply the new highlights
        let redraw_cmd = update_syntax(
            &mut model,
            SyntaxMsg::ParseCompleted {
                document_id: doc_id,
                revision: 1,
                highlights: new_highlights,
            },
        );
        assert!(matches!(redraw_cmd, Some(Cmd::Redraw)));

        // Final verification: highlights are stored and aligned correctly
        let doc = model.editor_area.documents.get(&doc_id).unwrap();
        assert!(doc.syntax_highlights.is_some());

        let final_highlights = doc.syntax_highlights.as_ref().unwrap();
        assert!(
            final_highlights.lines.contains_key(&1),
            "Final highlights should have tokens on line 1"
        );
    }
}
