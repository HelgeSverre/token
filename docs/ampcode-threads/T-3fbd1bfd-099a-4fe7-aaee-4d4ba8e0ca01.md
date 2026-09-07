---
title: "Fix cursor calls in update/document.rs"
threadId: T-3fbd1bfd-099a-4fe7-aaee-4d4ba8e0ca01
created: 2025-12-07T06:24:25.379Z
agentMode: smart
archiveSource: local Amp session cache
---

# Fix cursor calls in update/document.rs

## User

Fix all cursor()/cursor_mut()/selection()/selection_mut() calls in src/update/document.rs after they were renamed to primary_cursor()/primary_cursor_mut()/primary_selection()/primary_selection_mut().

Context: This is part of a multi-cursor refactor. Most usages in document.rs are for undo/redo metadata (cursor_before/cursor_after) which should use primary_cursor() since undo metadata stores primary cursor state.

For each call site, determine:
1. If it's undo metadata (cursor_before, cursor_after) → use primary_cursor()
2. If it's for current editing position → use primary_cursor() (single-cursor code path)
3. Multi-cursor paths already use cursors[] directly, leave those alone

Run `cargo build 2>&1 | grep "src/update/document.rs"` after fixing to verify no errors remain in this file.

## Assistant

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs"
}
```

## User

**Tool Result:** `toolu_vrtx_01FH37seY8Xqn8HRTGvdtWyr`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "1: //! Document update functions for text editing and undo/redo\n2: \n3: use crate::commands::Cmd;\n4: use crate::messages::DocumentMsg;\n5: use crate::model::{AppModel, Cursor, EditOperation, Position, Selection};\n6: \n7: use super::editor::{cursors_in_reverse_order, delete_selection, sync_other_editor_cursors};\n8: \n9: /// Handle document messages (text editing, undo/redo)\n10: pub fn update_document(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {\n11:     // Clear occurrence selection state on any editing operation\n12:     // (except Copy which doesn't modify the document)\n13:     if !matches!(msg, DocumentMsg::Copy) {\n14:         model.editor_mut().occurrence_state = None;\n15:     }\n16: \n17:     match msg {\n18:         DocumentMsg::InsertChar(ch) => {\n19:             let cursor_before = *model.editor().cursor();\n20: \n21:             // Multi-cursor: process all cursors in reverse document order\n22:             if model.editor().has_multiple_cursors() {\n23:                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n24:                 let indices = cursors_in_reverse_order(model);\n25:                 let mut operations = Vec::new();\n26: \n27:                 for idx in indices {\n28:                     // Get cursor position and convert to buffer offset\n29:                     let cursor = model.editor().cursors[idx].clone();\n30:                     let pos = model\n31:                         .document()\n32:                         .cursor_to_offset(cursor.line, cursor.column);\n33: \n34:                     // Insert character\n35:                     model.document_mut().buffer.insert_char(pos, ch);\n36: \n37:                     // Record individual operation (positions are at time of insert)\n38:                     operations.push(EditOperation::Insert {\n39:                         position: pos,\n40:                         text: ch.to_string(),\n41:                         cursor_before: cursor.clone(),\n42:                         cursor_after: Cursor::at(cursor.line, cursor.column + 1),\n43:                     });\n44: \n45:                     // Update this cursor's position (move right by 1)\n46:                     model.editor_mut().cursors[idx].column += 1;\n47:                     model.editor_mut().cursors[idx].desired_column = None;\n48: \n49:                     // Clear this cursor's selection\n50:                     let new_pos = model.editor().cursors[idx].to_position();\n51:                     model.editor_mut().selections[idx] = Selection::new(new_pos);\n52:                 }\n53: \n54:                 // Record batch for proper multi-cursor undo\n55:                 // Operations are stored in application order (reverse document order)\n56:                 // Undo will iterate .rev() to process in forward document order\n57:                 let cursors_after: Vec<Cursor> = model.editor().cursors.clone();\n58:                 model.document_mut().push_edit(EditOperation::Batch {\n59:                     operations,\n60:                     cursors_before,\n61:                     cursors_after,\n62:                 });\n63: \n64:                 model.document_mut().is_modified = true;\n65:                 model.ensure_cursor_visible();\n66:                 model.reset_cursor_blink();\n67:                 return Some(Cmd::Redraw);\n68:             }\n69: \n70:             // Single cursor: existing behavior\n71:             // If there's a selection, delete it first and use Replace for atomic undo\n72:             if let Some((pos, deleted_text)) = delete_selection(model) {\n73:                 // Insert at selection start\n74:                 model.document_mut().buffer.insert_char(pos, ch);\n75:                 model.set_cursor_from_position(pos + 1);\n76:                 model.ensure_cursor_visible();\n77: \n78:                 // Record as a single Replace operation for atomic undo\n79:                 let cursor_after = *model.editor().cursor();\n80:                 model.document_mut().push_edit(EditOperation::Replace {\n81:                     position: pos,\n82:                     deleted_text,\n83:                     inserted_text: ch.to_string(),\n84:                     cursor_before,\n85:                     cursor_after,\n86:                 });\n87:             } else {\n88:                 // No selection - normal insert\n89:                 let edit_line = cursor_before.line;\n90:                 let edit_column = cursor_before.column;\n91:                 let pos = model.cursor_buffer_position();\n92:                 model.document_mut().buffer.insert_char(pos, ch);\n93:                 model.set_cursor_from_position(pos + 1);\n94:                 model.ensure_cursor_visible();\n95: \n96:                 let cursor_after = *model.editor().cursor();\n97:                 model.document_mut().push_edit(EditOperation::Insert {\n98:                     position: pos,\n99:                     text: ch.to_string(),\n100:                     cursor_before,\n101:                     cursor_after,\n102:                 });\n103: \n104:                 // Sync cursors in other views\n105:                 sync_other_editor_cursors(model, edit_line, edit_column, 0, 1);\n106:             }\n107: \n108:             model.reset_cursor_blink();\n109:             Some(Cmd::Redraw)\n110:         }\n111: \n112:         DocumentMsg::InsertNewline => {\n113:             let cursor_before = *model.editor().cursor();\n114: \n115:             // Multi-cursor: process all cursors in reverse document order\n116:             if model.editor().has_multiple_cursors() {\n117:                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n118:                 let indices = cursors_in_reverse_order(model);\n119:                 let mut operations = Vec::new();\n120: \n121:                 for idx in indices {\n122:                     let cursor = model.editor().cursors[idx].clone();\n123:                     let pos = model\n124:                         .document()\n125:                         .cursor_to_offset(cursor.line, cursor.column);\n126:                     model.document_mut().buffer.insert_char(pos, '\\n');\n127: \n128:                     // Record individual operation\n129:                     operations.push(EditOperation::Insert {\n130:                         position: pos,\n131:                         text: \"\\n\".to_string(),\n132:                         cursor_before: cursor.clone(),\n133:                         cursor_after: Cursor::at(cursor.line + 1, 0),\n134:                     });\n135: \n136:                     // Move cursor to beginning of next line\n137:                     model.editor_mut().cursors[idx].line += 1;\n138:                     model.editor_mut().cursors[idx].column = 0;\n139:                     model.editor_mut().cursors[idx].desired_column = None;\n140: \n141:                     let new_pos = model.editor().cursors[idx].to_position();\n142:                     model.editor_mut().selections[idx] = Selection::new(new_pos);\n143:                 }\n144: \n145:                 // Record batch for proper multi-cursor undo\n146:                 // Operations stored in application order; undo iterates .rev()\n147:                 let cursors_after: Vec<Cursor> = model.editor().cursors.clone();\n148:                 model.document_mut().push_edit(EditOperation::Batch {\n149:                     operations,\n150:                     cursors_before,\n151:                     cursors_after,\n152:                 });\n153: \n154:                 model.document_mut().is_modified = true;\n155:                 model.ensure_cursor_visible();\n156:                 model.reset_cursor_blink();\n157:                 return Some(Cmd::Redraw);\n158:             }\n159: \n160:             // Single cursor: check for selection first\n161:             if let Some((pos, deleted_text)) = delete_selection(model) {\n162:                 model.document_mut().buffer.insert_char(pos, '\\n');\n163:                 model.set_cursor_from_position(pos + 1);\n164:                 model.ensure_cursor_visible();\n165: \n166:                 let cursor_after = *model.editor().cursor();\n167:                 model.document_mut().push_edit(EditOperation::Replace {\n168:                     position: pos,\n169:                     deleted_text,\n170:                     inserted_text: \"\\n\".to_string(),\n171:                     cursor_before,\n172:                     cursor_after,\n173:                 });\n174:             } else {\n175:                 let edit_line = cursor_before.line;\n176:                 let edit_column = cursor_before.column;\n177:                 let pos = model.cursor_buffer_position();\n178:                 model.document_mut().buffer.insert_char(pos, '\\n');\n179:                 model.set_cursor_from_position(pos + 1);\n180:                 model.ensure_cursor_visible();\n181: \n182:                 let cursor_after = *model.editor().cursor();\n183:                 model.document_mut().push_edit(EditOperation::Insert {\n184:                     position: pos,\n185:                     text: \"\\n\".to_string(),\n186:                     cursor_before,\n187:                     cursor_after,\n188:                 });\n189: \n190:                 // Sync cursors in other views: newline adds 1 line\n191:                 sync_other_editor_cursors(model, edit_line, edit_column, 1, 0);\n192:             }\n193: \n194:             model.reset_cursor_blink();\n195:             Some(Cmd::Redraw)\n196:         }\n197: \n198:         DocumentMsg::DeleteBackward => {\n199:             let cursor_before = *model.editor().cursor();\n200: \n201:             // Multi-cursor: process all cursors in reverse document order\n202:             if model.editor().has_multiple_cursors() {\n203:                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n204:                 let indices = cursors_in_reverse_order(model);\n205:                 let mut operations = Vec::new();\n206: \n207:                 for idx in indices {\n208:                     let selection = model.editor().selections[idx].clone();\n209:                     if !selection.is_empty() {\n210:                         // Delete selection\n211:                         let start = selection.start();\n212:                         let end = selection.end();\n213:                         let start_offset =\n214:                             model.document().cursor_to_offset(start.line, start.column);\n215:                         let end_offset = model.document().cursor_to_offset(end.line, end.column);\n216:                         let deleted_text: String = model\n217:                             .document()\n218:                             .buffer\n219:                             .slice(start_offset..end_offset)\n220:                             .to_string();\n221:                         model.document_mut().buffer.remove(start_offset..end_offset);\n222: \n223:                         // Record operation\n224:                         operations.push(EditOperation::Delete {\n225:                             position: start_offset,\n226:                             text: deleted_text,\n227:                             cursor_before: model.editor().cursors[idx].clone(),\n228:                             cursor_after: Cursor::at(start.line, start.column),\n229:                         });\n230: \n231:                         model.editor_mut().cursors[idx].line = start.line;\n232:                         model.editor_mut().cursors[idx].column = start.column;\n233:                         model.editor_mut().selections[idx] = Selection::new(start);\n234:                     } else {\n235:                         let cursor = model.editor().cursors[idx].clone();\n236:                         let pos = model\n237:                             .document()\n238:                             .cursor_to_offset(cursor.line, cursor.column);\n239:                         if pos > 0 {\n240:                             let deleted_char: String = model\n241:                                 .document()\n242:                                 .buffer\n243:                                 .slice(pos - 1..pos)\n244:                                 .chars()\n245:                                 .collect();\n246:                             model.document_mut().buffer.remove(pos - 1..pos);\n247:                             let (new_line, new_col) = model.document().offset_to_cursor(pos - 1);\n248: \n249:                             // Record operation\n250:                             operations.push(EditOperation::Delete {\n251:                                 position: pos - 1,\n252:                                 text: deleted_char,\n253:                                 cursor_before: cursor.clone(),\n254:                                 cursor_after: Cursor::at(new_line, new_col),\n255:                             });\n256: \n257:                             model.editor_mut().cursors[idx].line = new_line;\n258:                             model.editor_mut().cursors[idx].column = new_col;\n259:                             let new_pos = model.editor().cursors[idx].to_position();\n260:                             model.editor_mut().selections[idx] = Selection::new(new_pos);\n261:                         }\n262:                     }\n263:                 }\n264: \n265:                 // Record batch for proper multi-cursor undo\n266:                 // Operations stored in application order; undo iterates .rev()\n267:                 let cursors_after: Vec<Cursor> = model.editor().cursors.clone();\n268:                 model.document_mut().push_edit(EditOperation::Batch {\n269:                     operations,\n270:                     cursors_before,\n271:                     cursors_after,\n272:                 });\n273: \n274:                 model.document_mut().is_modified = true;\n275:                 model.ensure_cursor_visible();\n276:                 model.reset_cursor_blink();\n277:                 return Some(Cmd::Redraw);\n278:             }\n279: \n280:             // Single cursor: check for selection\n281:             if let Some((pos, deleted_text)) = delete_selection(model) {\n282:                 let cursor_after = *model.editor().cursor();\n283:                 model.document_mut().push_edit(EditOperation::Delete {\n284:                     position: pos,\n285:                     text: deleted_text,\n286:                     cursor_before,\n287:                     cursor_after,\n288:                 });\n289:                 model.document_mut().is_modified = true;\n290:                 model.ensure_cursor_visible();\n291:                 model.reset_cursor_blink();\n292:                 return Some(Cmd::Redraw);\n293:             }\n294: \n295:             let pos = model.cursor_buffer_position();\n296:             if pos > 0 {\n297:                 let deleted_char: String = model\n298:                     .document()\n299:                     .buffer\n300:                     .slice(pos - 1..pos)\n301:                     .chars()\n302:                     .collect();\n303: \n304:                 // Calculate edit info for cursor sync\n305:                 let is_newline = deleted_char == \"\\n\";\n306:                 let edit_line = cursor_before.line;\n307:                 let edit_column = cursor_before.column;\n308: \n309:                 model.document_mut().buffer.remove(pos - 1..pos);\n310:                 model.set_cursor_from_position(pos - 1);\n311:                 model.ensure_cursor_visible();\n312: \n313:                 let cursor_after = *model.editor().cursor();\n314:                 model.document_mut().push_edit(EditOperation::Delete {\n315:                     position: pos - 1,\n316:                     text: deleted_char,\n317:                     cursor_before,\n318:                     cursor_after,\n319:                 });\n320: \n321:                 // Sync cursors in other views\n322:                 if is_newline {\n323:                     // Deleted newline: removes a line, cursors on later lines shift up\n324:                     sync_other_editor_cursors(model, edit_line.saturating_sub(1), 0, -1, 0);\n325:                 } else {\n326:                     // Deleted character: cursors after shift left\n327:                     sync_other_editor_cursors(\n328:                         model,\n329:                         edit_line,\n330:                         edit_column.saturating_sub(1),\n331:                         0,\n332:                         -1,\n333:                     );\n334:                 }\n335:             }\n336: \n337:             model.reset_cursor_blink();\n338:             Some(Cmd::Redraw)\n339:         }\n340: \n341:         DocumentMsg::DeleteForward => {\n342:             let cursor_before = *model.editor().cursor();\n343: \n344:             // Multi-cursor: process all cursors in reverse document order\n345:             if model.editor().has_multiple_cursors() {\n346:                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n347:                 let indices = cursors_in_reverse_order(model);\n348:                 let mut operations = Vec::new();\n349: \n350:                 for idx in indices {\n351:                     let selection = model.editor().selections[idx].clone();\n352:                     if !selection.is_empty() {\n353:                         let start = selection.start();\n354:                         let end = selection.end();\n355:                         let start_offset =\n356:                             model.document().cursor_to_offset(start.line, start.column);\n357:                         let end_offset = model.document().cursor_to_offset(end.line, end.column);\n358:                         let deleted_text: String = model\n359:                             .document()\n360:                             .buffer\n361:                             .slice(start_offset..end_offset)\n362:                             .to_string();\n363:                         model.document_mut().buffer.remove(start_offset..end_offset);\n364: \n365:                         // Record operation\n366:                         operations.push(EditOperation::Delete {\n367:                             position: start_offset,\n368:                             text: deleted_text,\n369:                             cursor_before: model.editor().cursors[idx].clone(),\n370:                             cursor_after: Cursor::at(start.line, start.column),\n371:                         });\n372: \n373:                         model.editor_mut().cursors[idx].line = start.line;\n374:                         model.editor_mut().cursors[idx].column = start.column;\n375:                         model.editor_mut().selections[idx] = Selection::new(start);\n376:                     } else {\n377:                         let cursor = model.editor().cursors[idx].clone();\n378:                         let pos = model\n379:                             .document()\n380:                             .cursor_to_offset(cursor.line, cursor.column);\n381:                         if pos < model.document().buffer.len_chars() {\n382:                             let deleted_char: String = model\n383:                                 .document()\n384:                                 .buffer\n385:                                 .slice(pos..pos + 1)\n386:                                 .chars()\n387:                                 .collect();\n388:                             model.document_mut().buffer.remove(pos..pos + 1);\n389: \n390:                             // Record operation (cursor doesn't move for delete forward)\n391:                             operations.push(EditOperation::Delete {\n392:                                 position: pos,\n393:                                 text: deleted_char,\n394:                                 cursor_before: cursor.clone(),\n395:                                 cursor_after: cursor.clone(),\n396:                             });\n397:                         }\n398:                     }\n399:                 }\n400: \n401:                 // Record batch for proper multi-cursor undo\n402:                 // Operations stored in application order; undo iterates .rev()\n403:                 let cursors_after: Vec<Cursor> = model.editor().cursors.clone();\n404:                 model.document_mut().push_edit(EditOperation::Batch {\n405:                     operations,\n406:                     cursors_before,\n407:                     cursors_after,\n408:                 });\n409: \n410:                 model.document_mut().is_modified = true;\n411:                 model.reset_cursor_blink();\n412:                 return Some(Cmd::Redraw);\n413:             }\n414: \n415:             // Single cursor: check for selection\n416:             if let Some((pos, deleted_text)) = delete_selection(model) {\n417:                 let cursor_after = *model.editor().cursor();\n418:                 model.document_mut().push_edit(EditOperation::Delete {\n419:                     position: pos,\n420:                     text: deleted_text,\n421:                     cursor_before,\n422:                     cursor_after,\n423:                 });\n424:                 model.document_mut().is_modified = true;\n425:                 model.reset_cursor_blink();\n426:                 return Some(Cmd::Redraw);\n427:             }\n428: \n429:             let pos = model.cursor_buffer_position();\n430:             if pos < model.document().buffer.len_chars() {\n431:                 let deleted_char: String = model\n432:                     .document()\n433:                     .buffer\n434:                     .slice(pos..pos + 1)\n435:                     .chars()\n436:                     .collect();\n437: \n438:                 // Calculate edit info for cursor sync\n439:                 let is_newline = deleted_char == \"\\n\";\n440:                 let edit_line = cursor_before.line;\n441:                 let edit_column = cursor_before.column;\n442: \n443:                 model.document_mut().buffer.remove(pos..pos + 1);\n444: \n445:                 let cursor_after = *model.editor().cursor();\n446:                 model.document_mut().push_edit(EditOperation::Delete {\n447:                     position: pos,\n448:                     text: deleted_char,\n449:                     cursor_before,\n450:                     cursor_after,\n451:                 });\n452: \n453:                 // Sync cursors in other views\n454:                 if is_newline {\n455:                     // Deleted newline: removes a line\n456:                     sync_other_editor_cursors(model, edit_line, edit_column, -1, 0);\n457:                 } else {\n458:                     // Deleted character: cursors after shift left\n459:                     sync_other_editor_cursors(model, edit_line, edit_column, 0, -1);\n460:                 }\n461:             }\n462: \n463:             model.reset_cursor_blink();\n464:             Some(Cmd::Redraw)\n465:         }\n466: \n467:         DocumentMsg::DeleteLine => {\n468:             let cursor_before = *model.editor().cursor();\n469:             let line_idx = model.editor().cursor().line;\n470:             let total_lines = model.document().line_count();\n471: \n472:             if total_lines == 0 {\n473:                 return Some(Cmd::Redraw);\n474:             }\n475: \n476:             // Calculate the range to delete\n477:             let (start_offset, end_offset) = if line_idx + 1 < total_lines {\n478:                 // Not the last line: delete from start of line to start of next line\n479:                 let start = model.document().cursor_to_offset(line_idx, 0);\n480:                 let end = model.document().cursor_to_offset(line_idx + 1, 0);\n481:                 (start, end)\n482:             } else if line_idx > 0 {\n483:                 // Last line but not the only line: delete preceding newline + content\n484:                 // The newline is at the end of the previous line\n485:                 let prev_line_len = model.document().line_length(line_idx - 1);\n486:                 let start = model\n487:                     .document()\n488:                     .cursor_to_offset(line_idx - 1, prev_line_len);\n489:                 let end = model.document().buffer.len_chars();\n490:                 (start, end)\n491:             } else {\n492:                 // Only line: delete everything\n493:                 let start = 0;\n494:                 let end = model.document().buffer.len_chars();\n495:                 (start, end)\n496:             };\n497: \n498:             if start_offset < end_offset {\n499:                 // Determine if we're deleting the last line (which means cursor goes to prev line end)\n500:                 let was_last_line = line_idx + 1 >= total_lines && line_idx > 0;\n[... 231 lines omitted ...]\n501: \n502:                     // If clipboard has same number of lines as cursors, distribute one per cursor\n503:                     if lines.len() == cursor_count {\n504:                         let indices = cursors_in_reverse_order(model);\n505:                         for (i, idx) in indices.iter().enumerate() {\n506:                             let line_to_paste = lines[cursor_count - 1 - i]; // Reverse order\n507:                             let cursor = model.editor().cursors[*idx].clone();\n508:                             let pos = model\n509:                                 .document()\n510:                                 .cursor_to_offset(cursor.line, cursor.column);\n511:                             model.document_mut().buffer.insert(pos, line_to_paste);\n512:                             model.editor_mut().cursors[*idx].column +=\n513:                                 line_to_paste.chars().count();\n514:                             let new_pos = model.editor().cursors[*idx].to_position();\n515:                             model.editor_mut().selections[*idx] = Selection::new(new_pos);\n516:                         }\n517:                     } else {\n518:                         // Paste full text at each cursor\n519:                         let indices = cursors_in_reverse_order(model);\n520:                         for idx in indices {\n521:                             let cursor = model.editor().cursors[idx].clone();\n522:                             let pos = model\n523:                                 .document()\n524:                                 .cursor_to_offset(cursor.line, cursor.column);\n525:                             model.document_mut().buffer.insert(pos, &text);\n526: \n527:                             // Update cursor position (move to end of pasted text)\n528:                             let new_offset = pos + text.chars().count();\n529:                             let (new_line, new_col) = model.document().offset_to_cursor(new_offset);\n530:                             model.editor_mut().cursors[idx].line = new_line;\n531:                             model.editor_mut().cursors[idx].column = new_col;\n532:                             let new_pos = model.editor().cursors[idx].to_position();\n533:                             model.editor_mut().selections[idx] = Selection::new(new_pos);\n534:                         }\n535:                     }\n536:                 } else {\n537:                     // Single cursor: use Replace if selection exists for atomic undo\n538:                     if !model.editor().selection().is_empty() {\n539:                         let (pos, deleted_text) = delete_selection(model).unwrap();\n540: \n541:                         model.document_mut().buffer.insert(pos, &text);\n542: \n543:                         // Move cursor to end of pasted text\n544:                         let new_offset = pos + text.chars().count();\n545:                         model.set_cursor_from_position(new_offset);\n546: \n547:                         let cursor_after = *model.editor().cursor();\n548:                         model.document_mut().push_edit(EditOperation::Replace {\n549:                             position: pos,\n550:                             deleted_text,\n551:                             inserted_text: text.clone(),\n552:                             cursor_before,\n553:                             cursor_after,\n554:                         });\n555:                     } else {\n556:                         let pos = model.cursor_buffer_position();\n557: \n558:                         model.document_mut().buffer.insert(pos, &text);\n559: \n560:                         // Move cursor to end of pasted text\n561:                         let new_offset = pos + text.chars().count();\n562:                         model.set_cursor_from_position(new_offset);\n563: \n564:                         let cursor_after = *model.editor().cursor();\n565:                         model.document_mut().push_edit(EditOperation::Insert {\n566:                             position: pos,\n567:                             text: text.clone(),\n568:                             cursor_before,\n569:                             cursor_after,\n570:                         });\n571:                     }\n572:                 }\n573: \n574:                 model.document_mut().is_modified = true;\n575:                 model.ui.set_status(format!(\"Pasted {} chars\", text.len()));\n576:                 model.ensure_cursor_visible();\n577:                 model.reset_cursor_blink();\n578:             }\n579: \n580:             Some(Cmd::Redraw)\n581:         }\n582: \n583:         DocumentMsg::Duplicate => {\n584:             let cursor_before = *model.editor().cursor();\n585:             let selection = model.editor().selection().clone();\n586: \n587:             if selection.is_empty() {\n588:                 // No selection: duplicate the current line\n589:                 let line_idx = model.editor().cursor().line;\n590:                 let column = model.editor().cursor().column;\n591: \n592:                 // Get the current line content\n593:                 let line_text = model.document().get_line(line_idx).unwrap_or_default();\n594:                 let has_newline = line_text.ends_with('\\n');\n595: \n596:                 // Calculate insert position (end of current line)\n597:                 let line_end_offset = if has_newline {\n598:                     // Insert after the newline\n599:                     model.document().cursor_to_offset(line_idx + 1, 0)\n600:                 } else {\n601:                     // No newline - insert at end with a newline prefix\n602:                     model\n603:                         .document()\n604:                         .cursor_to_offset(line_idx, model.document().line_length(line_idx))\n605:                 };\n606: \n607:                 // Text to insert: for lines with newline, just the line content\n608:                 // For lines without, prefix with newline\n609:                 let text_to_insert = if has_newline {\n610:                     line_text.clone()\n611:                 } else {\n612:                     format!(\"\\n{}\", line_text)\n613:                 };\n614: \n615:                 model\n616:                     .document_mut()\n617:                     .buffer\n618:                     .insert(line_end_offset, &text_to_insert);\n619: \n620:                 // Move cursor to duplicated line at same column\n621:                 model.editor_mut().cursor_mut().line += 1;\n622:                 let new_line = model.editor().cursor().line;\n623:                 model.editor_mut().cursor_mut().column =\n624:                     column.min(model.document().line_length(new_line));\n625: \n626:                 let cursor_after = *model.editor().cursor();\n627:                 model.document_mut().push_edit(EditOperation::Insert {\n628:                     position: line_end_offset,\n629:                     text: text_to_insert,\n630:                     cursor_before,\n631:                     cursor_after,\n632:                 });\n633:             } else {\n634:                 // With selection: duplicate the selected text after selection end\n635:                 let sel_start = selection.start();\n636:                 let sel_end = selection.end();\n637: \n638:                 let start_offset = model\n639:                     .document()\n640:                     .cursor_to_offset(sel_start.line, sel_start.column);\n641:                 let end_offset = model\n642:                     .document()\n643:                     .cursor_to_offset(sel_end.line, sel_end.column);\n644: \n645:                 // Get selected text\n646:                 let selected_text: String = model\n647:                     .document()\n648:                     .buffer\n649:                     .slice(start_offset..end_offset)\n650:                     .chars()\n651:                     .collect();\n652: \n653:                 // Insert at end of selection\n654:                 model\n655:                     .document_mut()\n656:                     .buffer\n657:                     .insert(end_offset, &selected_text);\n658: \n659:                 // Move cursor to end of duplicated text\n660:                 let new_offset = end_offset + selected_text.chars().count();\n661:                 model.set_cursor_from_position(new_offset);\n662: \n663:                 // Clear selection\n664:                 model.editor_mut().clear_selection();\n665: \n666:                 let cursor_after = *model.editor().cursor();\n667:                 model.document_mut().push_edit(EditOperation::Insert {\n668:                     position: end_offset,\n669:                     text: selected_text,\n670:                     cursor_before,\n671:                     cursor_after,\n672:                 });\n673:             }\n674: \n675:             model.document_mut().is_modified = true;\n676:             model.ensure_cursor_visible();\n677:             model.reset_cursor_blink();\n678:             Some(Cmd::Redraw)\n679:         }\n680: \n681:         DocumentMsg::IndentLines => {\n682:             let cursor_before = *model.editor().cursor();\n683:             let selection = model.editor().selection().clone();\n684: \n685:             let start_line = selection.start().line;\n686:             let end_line = selection.end().line;\n687: \n688:             // Skip last line if selection ends at column 0\n689:             let effective_end = if selection.end().column == 0 && end_line > start_line {\n690:                 end_line - 1\n691:             } else {\n692:                 end_line\n693:             };\n694: \n695:             let mut inserted_text = String::new();\n696: \n697:             // Insert tabs in reverse order (to preserve offsets)\n698:             for line in (start_line..=effective_end).rev() {\n699:                 let offset = model.document().cursor_to_offset(line, 0);\n700:                 model.document_mut().buffer.insert_char(offset, '\\t');\n701:                 inserted_text.push('\\t');\n702:             }\n703: \n704:             // Adjust cursor column +1\n705:             model.editor_mut().cursor_mut().column += 1;\n706: \n707:             // Adjust selection: bump both anchor and head columns +1\n708:             model.editor_mut().selection_mut().anchor.column += 1;\n709:             model.editor_mut().selection_mut().head.column += 1;\n710: \n711:             let cursor_after = *model.editor().cursor();\n712:             // Record single Insert for undo (position = first line start)\n713:             let first_offset = model.document().cursor_to_offset(start_line, 0);\n714:             model.document_mut().push_edit(EditOperation::Insert {\n715:                 position: first_offset,\n716:                 text: inserted_text,\n717:                 cursor_before,\n718:                 cursor_after,\n719:             });\n720: \n721:             model.document_mut().is_modified = true;\n722:             model.ensure_cursor_visible();\n723:             model.reset_cursor_blink();\n724:             Some(Cmd::Redraw)\n725:         }\n726: \n727:         DocumentMsg::UnindentLines => {\n728:             let cursor_before = *model.editor().cursor();\n729:             let selection = model.editor().selection().clone();\n730: \n731:             let (start_line, end_line) = if selection.is_empty() {\n732:                 let line = model.editor().cursor().line;\n733:                 (line, line)\n734:             } else {\n735:                 let end = if selection.end().column == 0\n736:                     && selection.end().line > selection.start().line\n737:                 {\n738:                     selection.end().line - 1\n739:                 } else {\n740:                     selection.end().line\n741:                 };\n742:                 (selection.start().line, end)\n743:             };\n744: \n745:             let mut total_removed = 0;\n746:             let mut removed_from_cursor_line = 0;\n747:             let mut removed_from_anchor_line = 0;\n748:             let mut removed_from_head_line = 0;\n749: \n750:             // Process in reverse order\n751:             for line in (start_line..=end_line).rev() {\n752:                 let line_start = model.document().cursor_to_offset(line, 0);\n753:                 let line_text: String = model.document().buffer.line(line).chars().collect();\n754: \n755:                 let chars_to_remove = if line_text.starts_with('\\t') {\n756:                     1\n757:                 } else {\n758:                     // Count leading spaces (up to 4)\n759:                     line_text.chars().take_while(|c| *c == ' ').count().min(4)\n760:                 };\n761: \n762:                 if chars_to_remove > 0 {\n763:                     model\n764:                         .document_mut()\n765:                         .buffer\n766:                         .remove(line_start..line_start + chars_to_remove);\n767:                     total_removed += chars_to_remove;\n768: \n769:                     if line == model.editor().cursor().line {\n770:                         removed_from_cursor_line = chars_to_remove;\n771:                     }\n772:                     if line == selection.anchor.line {\n773:                         removed_from_anchor_line = chars_to_remove;\n774:                     }\n775:                     if line == selection.head.line {\n776:                         removed_from_head_line = chars_to_remove;\n777:                     }\n778:                 }\n779:             }\n780: \n781:             if total_removed > 0 {\n782:                 // Calculate position before mutable borrows\n783:                 let edit_position = model.document().cursor_to_offset(start_line, 0);\n784: \n785:                 // Adjust cursor\n786:                 model.editor_mut().cursor_mut().column = model\n787:                     .editor()\n788:                     .cursor()\n789:                     .column\n790:                     .saturating_sub(removed_from_cursor_line);\n791: \n792:                 // Adjust selection if not empty\n793:                 if !selection.is_empty() {\n794:                     model.editor_mut().selection_mut().anchor.column = selection\n795:                         .anchor\n796:                         .column\n797:                         .saturating_sub(removed_from_anchor_line);\n798:                     model.editor_mut().selection_mut().head.column =\n799:                         selection.head.column.saturating_sub(removed_from_head_line);\n800:                 }\n801: \n802:                 let cursor_after = *model.editor().cursor();\n803:                 model.document_mut().push_edit(EditOperation::Delete {\n804:                     position: edit_position,\n805:                     text: \"\\t\".repeat(total_removed),\n806:                     cursor_before,\n807:                     cursor_after,\n808:                 });\n809: \n810:                 model.document_mut().is_modified = true;\n811:             }\n812: \n813:             model.ensure_cursor_visible();\n814:             model.reset_cursor_blink();\n815:             Some(Cmd::Redraw)\n816:         }\n817:     }\n818: }\n819: \n820: /// Apply an undo operation to the model (reverses the edit)\n821: fn apply_undo_operation(model: &mut AppModel, edit: &EditOperation) {\n822:     match edit {\n823:         EditOperation::Insert {\n824:             position,\n825:             text,\n826:             cursor_before,\n827:             ..\n828:         } => {\n829:             model\n830:                 .document_mut()\n831:                 .buffer\n832:                 .remove(*position..*position + text.chars().count());\n833:             *model.editor_mut().cursor_mut() = *cursor_before;\n834:         }\n835:         EditOperation::Delete {\n836:             position,\n837:             text,\n838:             cursor_before,\n839:             ..\n840:         } => {\n841:             model.document_mut().buffer.insert(*position, text);\n842:             *model.editor_mut().cursor_mut() = *cursor_before;\n843:         }\n844:         EditOperation::Replace {\n845:             position,\n846:             deleted_text,\n847:             inserted_text,\n848:             cursor_before,\n849:             ..\n850:         } => {\n851:             model\n852:                 .document_mut()\n853:                 .buffer\n854:                 .remove(*position..*position + inserted_text.chars().count());\n855:             model.document_mut().buffer.insert(*position, deleted_text);\n856:             *model.editor_mut().cursor_mut() = *cursor_before;\n857:         }\n858:         EditOperation::Batch {\n859:             operations,\n860:             cursors_before,\n861:             ..\n862:         } => {\n863:             // Undo in reverse order\n864:             for op in operations.iter().rev() {\n865:                 apply_undo_operation_buffer_only(model, op);\n866:             }\n867:             // Restore all cursors\n868:             let editor = model.editor_mut();\n869:             editor.cursors = cursors_before.clone();\n870:             // Ensure selections array matches\n871:             while editor.selections.len() < editor.cursors.len() {\n872:                 editor.selections.push(Selection::new(Position::new(0, 0)));\n873:             }\n874:             editor.selections.truncate(editor.cursors.len());\n875:         }\n876:     }\n877: }\n878: \n879: /// Apply undo to buffer only (for batch operations - cursor handled separately)\n880: fn apply_undo_operation_buffer_only(model: &mut AppModel, edit: &EditOperation) {\n881:     match edit {\n882:         EditOperation::Insert { position, text, .. } => {\n883:             model\n884:                 .document_mut()\n885:                 .buffer\n886:                 .remove(*position..*position + text.chars().count());\n887:         }\n888:         EditOperation::Delete { position, text, .. } => {\n889:             model.document_mut().buffer.insert(*position, text);\n890:         }\n891:         EditOperation::Replace {\n892:             position,\n893:             deleted_text,\n894:             inserted_text,\n895:             ..\n896:         } => {\n897:             model\n898:                 .document_mut()\n899:                 .buffer\n900:                 .remove(*position..*position + inserted_text.chars().count());\n901:             model.document_mut().buffer.insert(*position, deleted_text);\n902:         }\n903:         EditOperation::Batch { operations, .. } => {\n904:             for op in operations.iter().rev() {\n905:                 apply_undo_operation_buffer_only(model, op);\n906:             }\n907:         }\n908:     }\n909: }\n910: \n911: /// Apply a redo operation to the model (re-applies the edit)\n912: fn apply_redo_operation(model: &mut AppModel, edit: &EditOperation) {\n913:     match edit {\n914:         EditOperation::Insert {\n915:             position,\n916:             text,\n917:             cursor_after,\n918:             ..\n919:         } => {\n920:             model.document_mut().buffer.insert(*position, text);\n921:             *model.editor_mut().cursor_mut() = *cursor_after;\n922:         }\n923:         EditOperation::Delete {\n924:             position,\n925:             text,\n926:             cursor_after,\n927:             ..\n928:         } => {\n929:             model\n930:                 .document_mut()\n931:                 .buffer\n932:                 .remove(*position..*position + text.chars().count());\n933:             *model.editor_mut().cursor_mut() = *cursor_after;\n934:         }\n935:         EditOperation::Replace {\n936:             position,\n937:             deleted_text,\n938:             inserted_text,\n939:             cursor_after,\n940:             ..\n941:         } => {\n942:             model\n943:                 .document_mut()\n944:                 .buffer\n945:                 .remove(*position..*position + deleted_text.chars().count());\n946:             model.document_mut().buffer.insert(*position, inserted_text);\n947:             *model.editor_mut().cursor_mut() = *cursor_after;\n948:         }\n949:         EditOperation::Batch {\n950:             operations,\n951:             cursors_after,\n952:             ..\n953:         } => {\n954:             // Redo in forward order\n955:             for op in operations.iter() {\n956:                 apply_redo_operation_buffer_only(model, op);\n957:             }\n958:             // Restore all cursors\n959:             let editor = model.editor_mut();\n960:             editor.cursors = cursors_after.clone();\n961:             // Ensure selections array matches\n962:             while editor.selections.len() < editor.cursors.len() {\n963:                 editor.selections.push(Selection::new(Position::new(0, 0)));\n964:             }\n965:             editor.selections.truncate(editor.cursors.len());\n966:         }\n967:     }\n968: }\n969: \n970: /// Apply redo to buffer only (for batch operations - cursor handled separately)\n971: fn apply_redo_operation_buffer_only(model: &mut AppModel, edit: &EditOperation) {\n972:     match edit {\n973:         EditOperation::Insert { position, text, .. } => {\n974:             model.document_mut().buffer.insert(*position, text);\n975:         }\n976:         EditOperation::Delete { position, text, .. } => {\n977:             model\n978:                 .document_mut()\n979:                 .buffer\n980:                 .remove(*position..*position + text.chars().count());\n981:         }\n982:         EditOperation::Replace {\n983:             position,\n984:             deleted_text,\n985:             inserted_text,\n986:             ..\n987:         } => {\n988:             model\n989:                 .document_mut()\n990:                 .buffer\n991:                 .remove(*position..*position + deleted_text.chars().count());\n992:             model.document_mut().buffer.insert(*position, inserted_text);\n993:         }\n994:         EditOperation::Batch { operations, .. } => {\n995:             for op in operations.iter() {\n996:                 apply_redo_operation_buffer_only(model, op);\n997:             }\n998:         }\n999:     }\n1000: }"
}
```

## Assistant

Now I'll update all the `cursor()`, `cursor_mut()`, `selection()`, and `selection_mut()` calls to their `primary_` prefixed versions. Let me make these edits:

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            let cursor_before = *model.editor().cursor();\n\n            // Multi-cursor: process all cursors in reverse document order\n            if model.editor().has_multiple_cursors() {",
  "new_str": "            let cursor_before = *model.editor().primary_cursor();\n\n            // Multi-cursor: process all cursors in reverse document order\n            if model.editor().has_multiple_cursors() {",
  "replace_all": true
}
```

## User

**Tool Result:** `toolu_vrtx_01NEnt4E1biLnnHPmrsvypq8`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -15,9 +15,9 @@\n     }\n \n     match msg {\n         DocumentMsg::InsertChar(ch) => {\n-            let cursor_before = *model.editor().cursor();\n+            let cursor_before = *model.editor().primary_cursor();\n \n             // Multi-cursor: process all cursors in reverse document order\n             if model.editor().has_multiple_cursors() {\n                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n@@ -109,9 +109,9 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::InsertNewline => {\n-            let cursor_before = *model.editor().cursor();\n+            let cursor_before = *model.editor().primary_cursor();\n \n             // Multi-cursor: process all cursors in reverse document order\n             if model.editor().has_multiple_cursors() {\n                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n@@ -195,9 +195,9 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::DeleteBackward => {\n-            let cursor_before = *model.editor().cursor();\n+            let cursor_before = *model.editor().primary_cursor();\n \n             // Multi-cursor: process all cursors in reverse document order\n             if model.editor().has_multiple_cursors() {\n                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n@@ -338,9 +338,9 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::DeleteForward => {\n-            let cursor_before = *model.editor().cursor();\n+            let cursor_before = *model.editor().primary_cursor();\n \n             // Multi-cursor: process all cursors in reverse document order\n             if model.editor().has_multiple_cursors() {\n                 let cursors_before: Vec<Cursor> = model.editor().cursors.clone();\n```",
  "lineRange": [
    1,
    1232
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Replace {",
  "new_str": "                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Replace {",
  "replace_all": true
}
```

## User

**Tool Result:** `toolu_vrtx_01QbKJuuReSMDEsaKHBaH1UL`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -75,9 +75,9 @@\n                 model.set_cursor_from_position(pos + 1);\n                 model.ensure_cursor_visible();\n \n                 // Record as a single Replace operation for atomic undo\n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Replace {\n                     position: pos,\n                     deleted_text,\n                     inserted_text: ch.to_string(),\n@@ -162,9 +162,9 @@\n                 model.document_mut().buffer.insert_char(pos, '\\n');\n                 model.set_cursor_from_position(pos + 1);\n                 model.ensure_cursor_visible();\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Replace {\n                     position: pos,\n                     deleted_text,\n                     inserted_text: \"\\n\".to_string(),\n```",
  "lineRange": [
    1,
    1232
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Insert {\n                    position: pos,\n                    text: ch.to_string(),",
  "new_str": "                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Insert {\n                    position: pos,\n                    text: ch.to_string(),"
}
```

## User

**Tool Result:** `toolu_vrtx_01AigAocehXtiPtaLDGjbKnu`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -92,9 +92,9 @@\n                 model.document_mut().buffer.insert_char(pos, ch);\n                 model.set_cursor_from_position(pos + 1);\n                 model.ensure_cursor_visible();\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Insert {\n                     position: pos,\n                     text: ch.to_string(),\n                     cursor_before,\n```",
  "lineRange": [
    96,
    99
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Insert {\n                    position: pos,\n                    text: \"\\n\".to_string(),",
  "new_str": "                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Insert {\n                    position: pos,\n                    text: \"\\n\".to_string(),"
}
```

## User

**Tool Result:** `toolu_vrtx_01Pqc8uSKKAnkTfFA2HuRyu2`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -178,9 +178,9 @@\n                 model.document_mut().buffer.insert_char(pos, '\\n');\n                 model.set_cursor_from_position(pos + 1);\n                 model.ensure_cursor_visible();\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Insert {\n                     position: pos,\n                     text: \"\\n\".to_string(),\n                     cursor_before,\n```",
  "lineRange": [
    182,
    185
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            // Single cursor: check for selection\n            if let Some((pos, deleted_text)) = delete_selection(model) {\n                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos,\n                    text: deleted_text,\n                    cursor_before,\n                    cursor_after,\n                });\n                model.document_mut().is_modified = true;\n                model.ensure_cursor_visible();\n                model.reset_cursor_blink();\n                return Some(Cmd::Redraw);\n            }\n\n            let pos = model.cursor_buffer_position();\n            if pos > 0 {\n                let deleted_char: String = model\n                    .document()\n                    .buffer\n                    .slice(pos - 1..pos)\n                    .chars()\n                    .collect();\n\n                // Calculate edit info for cursor sync\n                let is_newline = deleted_char == \"\\n\";\n                let edit_line = cursor_before.line;\n                let edit_column = cursor_before.column;\n\n                model.document_mut().buffer.remove(pos - 1..pos);\n                model.set_cursor_from_position(pos - 1);\n                model.ensure_cursor_visible();\n\n                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos - 1,\n                    text: deleted_char,\n                    cursor_before,\n                    cursor_after,\n                });",
  "new_str": "            // Single cursor: check for selection\n            if let Some((pos, deleted_text)) = delete_selection(model) {\n                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos,\n                    text: deleted_text,\n                    cursor_before,\n                    cursor_after,\n                });\n                model.document_mut().is_modified = true;\n                model.ensure_cursor_visible();\n                model.reset_cursor_blink();\n                return Some(Cmd::Redraw);\n            }\n\n            let pos = model.cursor_buffer_position();\n            if pos > 0 {\n                let deleted_char: String = model\n                    .document()\n                    .buffer\n                    .slice(pos - 1..pos)\n                    .chars()\n                    .collect();\n\n                // Calculate edit info for cursor sync\n                let is_newline = deleted_char == \"\\n\";\n                let edit_line = cursor_before.line;\n                let edit_column = cursor_before.column;\n\n                model.document_mut().buffer.remove(pos - 1..pos);\n                model.set_cursor_from_position(pos - 1);\n                model.ensure_cursor_visible();\n\n                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos - 1,\n                    text: deleted_char,\n                    cursor_before,\n                    cursor_after,\n                });"
}
```

## User

**Tool Result:** `toolu_vrtx_017M9Z7ZdbJidoanevndnwjD`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -278,9 +278,9 @@\n             }\n \n             // Single cursor: check for selection\n             if let Some((pos, deleted_text)) = delete_selection(model) {\n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Delete {\n                     position: pos,\n                     text: deleted_text,\n                     cursor_before,\n@@ -309,9 +309,9 @@\n                 model.document_mut().buffer.remove(pos - 1..pos);\n                 model.set_cursor_from_position(pos - 1);\n                 model.ensure_cursor_visible();\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Delete {\n                     position: pos - 1,\n                     text: deleted_char,\n                     cursor_before,\n```",
  "lineRange": [
    280,
    319
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            // Single cursor: check for selection\n            if let Some((pos, deleted_text)) = delete_selection(model) {\n                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos,\n                    text: deleted_text,\n                    cursor_before,\n                    cursor_after,\n                });\n                model.document_mut().is_modified = true;\n                model.reset_cursor_blink();\n                return Some(Cmd::Redraw);\n            }\n\n            let pos = model.cursor_buffer_position();\n            if pos < model.document().buffer.len_chars() {\n                let deleted_char: String = model\n                    .document()\n                    .buffer\n                    .slice(pos..pos + 1)\n                    .chars()\n                    .collect();\n\n                // Calculate edit info for cursor sync\n                let is_newline = deleted_char == \"\\n\";\n                let edit_line = cursor_before.line;\n                let edit_column = cursor_before.column;\n\n                model.document_mut().buffer.remove(pos..pos + 1);\n\n                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos,\n                    text: deleted_char,\n                    cursor_before,\n                    cursor_after,\n                });",
  "new_str": "            // Single cursor: check for selection\n            if let Some((pos, deleted_text)) = delete_selection(model) {\n                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos,\n                    text: deleted_text,\n                    cursor_before,\n                    cursor_after,\n                });\n                model.document_mut().is_modified = true;\n                model.reset_cursor_blink();\n                return Some(Cmd::Redraw);\n            }\n\n            let pos = model.cursor_buffer_position();\n            if pos < model.document().buffer.len_chars() {\n                let deleted_char: String = model\n                    .document()\n                    .buffer\n                    .slice(pos..pos + 1)\n                    .chars()\n                    .collect();\n\n                // Calculate edit info for cursor sync\n                let is_newline = deleted_char == \"\\n\";\n                let edit_line = cursor_before.line;\n                let edit_column = cursor_before.column;\n\n                model.document_mut().buffer.remove(pos..pos + 1);\n\n                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Delete {\n                    position: pos,\n                    text: deleted_char,\n                    cursor_before,\n                    cursor_after,\n                });"
}
```

## User

**Tool Result:** `toolu_vrtx_01MQtSxXyPbAg5VFgrGJD1di`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -413,9 +413,9 @@\n             }\n \n             // Single cursor: check for selection\n             if let Some((pos, deleted_text)) = delete_selection(model) {\n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Delete {\n                     position: pos,\n                     text: deleted_text,\n                     cursor_before,\n@@ -441,9 +441,9 @@\n                 let edit_column = cursor_before.column;\n \n                 model.document_mut().buffer.remove(pos..pos + 1);\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Delete {\n                     position: pos,\n                     text: deleted_char,\n                     cursor_before,\n```",
  "lineRange": [
    415,
    451
  ]
}
```

## Assistant

Now let me fix the DeleteLine and remaining occurrences:

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "        DocumentMsg::DeleteLine => {\n            let cursor_before = *model.editor().cursor();\n            let line_idx = model.editor().cursor().line;\n            let total_lines = model.document().line_count();",
  "new_str": "        DocumentMsg::DeleteLine => {\n            let cursor_before = *model.editor().primary_cursor();\n            let line_idx = model.editor().primary_cursor().line;\n            let total_lines = model.document().line_count();"
}
```

## User

**Tool Result:** `toolu_vrtx_01H9oG1USWeGcMhRJDEocjNg`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -464,10 +464,10 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::DeleteLine => {\n-            let cursor_before = *model.editor().cursor();\n-            let line_idx = model.editor().cursor().line;\n+            let cursor_before = *model.editor().primary_cursor();\n+            let line_idx = model.editor().primary_cursor().line;\n             let total_lines = model.document().line_count();\n \n             if total_lines == 0 {\n                 return Some(Cmd::Redraw);\n```",
  "lineRange": [
    467,
    470
  ]
}
```

## Assistant

Now let me check for remaining occurrences in the rest of the file:

**Tool Use:** `Grep`

```json
{
  "pattern": "\\.cursor\\(\\)",
  "path": "~/code/rust-editor/src/update/document.rs"
}
```

## User

**Tool Result:** `toolu_vrtx_019jx3oVAZ5rryG6MdY9o3MD`

```
[
  "~/code/rust-editor/src/update/document.rs:518:let line_len = model.document().line_length(model.editor().cursor().line);",
  "~/code/rust-editor/src/update/document.rs:520:model.editor().cursor().column.min(line_len);",
  "~/code/rust-editor/src/update/document.rs:527:model.editor().cursor().column.min(new_line_len);",
  "~/code/rust-editor/src/update/document.rs:530:let cursor_after = *model.editor().cursor();",
  "~/code/rust-editor/src/update/document.rs:727:let cursor_before = *model.editor().cursor();",
  "~/code/rust-editor/src/update/document.rs:778:let cursor_after = *model.editor().cursor();",
  "~/code/rust-editor/src/update/document.rs:795:let cursor_after = *model.editor().cursor();",
  "~/code/rust-editor/src/update/document.rs:815:let cursor_before = *model.editor().cursor();",
  "~/code/rust-editor/src/update/document.rs:820:let line_idx = model.editor().cursor().line;",
  "~/code/rust-editor/src/update/document.rs:821:let column = model.editor().cursor().column;"
]
```

## Assistant

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    515,
    545
  ]
}
```

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    720,
    800
  ]
}
```

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    810,
    860
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_016fd49yDyc1rMft7zmP9J9z`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 514 ...]\n515:                 } else if was_last_line {\n516:                     // Deleted last line: cursor goes to previous line, retain column (clamped)\n517:                     model.editor_mut().cursor_mut().line = line_idx.saturating_sub(1);\n518:                     let line_len = model.document().line_length(model.editor().cursor().line);\n519:                     model.editor_mut().cursor_mut().column =\n520:                         model.editor().cursor().column.min(line_len);\n521:                 } else {\n522:                     // Deleted non-last line: stay at same line index, clamp column\n523:                     let new_line = line_idx.min(new_line_count.saturating_sub(1));\n524:                     let new_line_len = model.document().line_length(new_line);\n525:                     model.editor_mut().cursor_mut().line = new_line;\n526:                     model.editor_mut().cursor_mut().column =\n527:                         model.editor().cursor().column.min(new_line_len);\n528:                 }\n529: \n530:                 let cursor_after = *model.editor().cursor();\n531:                 model.document_mut().push_edit(EditOperation::Delete {\n532:                     position: start_offset,\n533:                     text: deleted,\n534:                     cursor_before,\n535:                     cursor_after,\n536:                 });\n537: \n538:                 model.document_mut().is_modified = true;\n539: \n540:                 // Scroll viewport up by 1 to keep cursor at same screen position\n541:                 if model.editor().viewport.top_line > 0 {\n542:                     model.editor_mut().viewport.top_line -= 1;\n543:                 }\n544:             }\n545: \n[... omitted lines 546 to ∞ ...]"
}
```

**Tool Result:** `toolu_vrtx_0118M5NupcSwLtzaSSc2sKtW`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 719 ...]\n720:             };\n721: \n722:             if let Some(text) = clipboard_text {\n723:                 if text.is_empty() {\n724:                     return Some(Cmd::Redraw);\n725:                 }\n726: \n727:                 let cursor_before = *model.editor().cursor();\n728: \n729:                 if model.editor().has_multiple_cursors() {\n730:                     let lines: Vec<&str> = text.lines().collect();\n731:                     let cursor_count = model.editor().cursors.len();\n732: \n733:                     // If clipboard has same number of lines as cursors, distribute one per cursor\n734:                     if lines.len() == cursor_count {\n735:                         let indices = cursors_in_reverse_order(model);\n736:                         for (i, idx) in indices.iter().enumerate() {\n737:                             let line_to_paste = lines[cursor_count - 1 - i]; // Reverse order\n738:                             let cursor = model.editor().cursors[*idx].clone();\n739:                             let pos = model\n740:                                 .document()\n741:                                 .cursor_to_offset(cursor.line, cursor.column);\n742:                             model.document_mut().buffer.insert(pos, line_to_paste);\n743:                             model.editor_mut().cursors[*idx].column +=\n744:                                 line_to_paste.chars().count();\n745:                             let new_pos = model.editor().cursors[*idx].to_position();\n746:                             model.editor_mut().selections[*idx] = Selection::new(new_pos);\n747:                         }\n748:                     } else {\n749:                         // Paste full text at each cursor\n750:                         let indices = cursors_in_reverse_order(model);\n751:                         for idx in indices {\n752:                             let cursor = model.editor().cursors[idx].clone();\n753:                             let pos = model\n754:                                 .document()\n755:                                 .cursor_to_offset(cursor.line, cursor.column);\n756:                             model.document_mut().buffer.insert(pos, &text);\n757: \n758:                             // Update cursor position (move to end of pasted text)\n759:                             let new_offset = pos + text.chars().count();\n760:                             let (new_line, new_col) = model.document().offset_to_cursor(new_offset);\n761:                             model.editor_mut().cursors[idx].line = new_line;\n762:                             model.editor_mut().cursors[idx].column = new_col;\n763:                             let new_pos = model.editor().cursors[idx].to_position();\n764:                             model.editor_mut().selections[idx] = Selection::new(new_pos);\n765:                         }\n766:                     }\n767:                 } else {\n768:                     // Single cursor: use Replace if selection exists for atomic undo\n769:                     if !model.editor().selection().is_empty() {\n770:                         let (pos, deleted_text) = delete_selection(model).unwrap();\n771: \n772:                         model.document_mut().buffer.insert(pos, &text);\n773: \n774:                         // Move cursor to end of pasted text\n775:                         let new_offset = pos + text.chars().count();\n776:                         model.set_cursor_from_position(new_offset);\n777: \n778:                         let cursor_after = *model.editor().cursor();\n779:                         model.document_mut().push_edit(EditOperation::Replace {\n780:                             position: pos,\n781:                             deleted_text,\n782:                             inserted_text: text.clone(),\n783:                             cursor_before,\n784:                             cursor_after,\n785:                         });\n786:                     } else {\n787:                         let pos = model.cursor_buffer_position();\n788: \n789:                         model.document_mut().buffer.insert(pos, &text);\n790: \n791:                         // Move cursor to end of pasted text\n792:                         let new_offset = pos + text.chars().count();\n793:                         model.set_cursor_from_position(new_offset);\n794: \n795:                         let cursor_after = *model.editor().cursor();\n796:                         model.document_mut().push_edit(EditOperation::Insert {\n797:                             position: pos,\n798:                             text: text.clone(),\n799:                             cursor_before,\n800:                             cursor_after,\n[... omitted lines 801 to ∞ ...]"
}
```

**Tool Result:** `toolu_vrtx_01LjbGE72Vm9V6qGg4wxTXqA`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 809 ...]\n810: \n811:             Some(Cmd::Redraw)\n812:         }\n813: \n814:         DocumentMsg::Duplicate => {\n815:             let cursor_before = *model.editor().cursor();\n816:             let selection = model.editor().selection().clone();\n817: \n818:             if selection.is_empty() {\n819:                 // No selection: duplicate the current line\n820:                 let line_idx = model.editor().cursor().line;\n821:                 let column = model.editor().cursor().column;\n822: \n823:                 // Get the current line content\n824:                 let line_text = model.document().get_line(line_idx).unwrap_or_default();\n825:                 let has_newline = line_text.ends_with('\\n');\n826: \n827:                 // Calculate insert position (end of current line)\n828:                 let line_end_offset = if has_newline {\n829:                     // Insert after the newline\n830:                     model.document().cursor_to_offset(line_idx + 1, 0)\n831:                 } else {\n832:                     // No newline - insert at end with a newline prefix\n833:                     model\n834:                         .document()\n835:                         .cursor_to_offset(line_idx, model.document().line_length(line_idx))\n836:                 };\n837: \n838:                 // Text to insert: for lines with newline, just the line content\n839:                 // For lines without, prefix with newline\n840:                 let text_to_insert = if has_newline {\n841:                     line_text.clone()\n842:                 } else {\n843:                     format!(\"\\n{}\", line_text)\n844:                 };\n845: \n846:                 model\n847:                     .document_mut()\n848:                     .buffer\n849:                     .insert(line_end_offset, &text_to_insert);\n850: \n851:                 // Move cursor to duplicated line at same column\n852:                 model.editor_mut().cursor_mut().line += 1;\n853:                 let new_line = model.editor().cursor().line;\n854:                 model.editor_mut().cursor_mut().column =\n855:                     column.min(model.document().line_length(new_line));\n856: \n857:                 let cursor_after = *model.editor().cursor();\n858:                 model.document_mut().push_edit(EditOperation::Insert {\n859:                     position: line_end_offset,\n860:                     text: text_to_insert,\n[... omitted lines 861 to ∞ ...]"
}
```

## Assistant

Now let me fix these remaining occurrences:

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                } else if was_last_line {\n                    // Deleted last line: cursor goes to previous line, retain column (clamped)\n                    model.editor_mut().cursor_mut().line = line_idx.saturating_sub(1);\n                    let line_len = model.document().line_length(model.editor().cursor().line);\n                    model.editor_mut().cursor_mut().column =\n                        model.editor().cursor().column.min(line_len);\n                } else {\n                    // Deleted non-last line: stay at same line index, clamp column\n                    let new_line = line_idx.min(new_line_count.saturating_sub(1));\n                    let new_line_len = model.document().line_length(new_line);\n                    model.editor_mut().cursor_mut().line = new_line;\n                    model.editor_mut().cursor_mut().column =\n                        model.editor().cursor().column.min(new_line_len);\n                }\n\n                let cursor_after = *model.editor().cursor();",
  "new_str": "                } else if was_last_line {\n                    // Deleted last line: cursor goes to previous line, retain column (clamped)\n                    model.editor_mut().primary_cursor_mut().line = line_idx.saturating_sub(1);\n                    let line_len = model.document().line_length(model.editor().primary_cursor().line);\n                    model.editor_mut().primary_cursor_mut().column =\n                        model.editor().primary_cursor().column.min(line_len);\n                } else {\n                    // Deleted non-last line: stay at same line index, clamp column\n                    let new_line = line_idx.min(new_line_count.saturating_sub(1));\n                    let new_line_len = model.document().line_length(new_line);\n                    model.editor_mut().primary_cursor_mut().line = new_line;\n                    model.editor_mut().primary_cursor_mut().column =\n                        model.editor().primary_cursor().column.min(new_line_len);\n                }\n\n                let cursor_after = *model.editor().primary_cursor();"
}
```

## User

**Tool Result:** `toolu_vrtx_01LebnYBoLiG4sTnMK1H6tHS`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -513,22 +513,22 @@\n                     model.editor_mut().cursor_mut().line = 0;\n                     model.editor_mut().cursor_mut().column = 0;\n                 } else if was_last_line {\n                     // Deleted last line: cursor goes to previous line, retain column (clamped)\n-                    model.editor_mut().cursor_mut().line = line_idx.saturating_sub(1);\n-                    let line_len = model.document().line_length(model.editor().cursor().line);\n-                    model.editor_mut().cursor_mut().column =\n-                        model.editor().cursor().column.min(line_len);\n+                    model.editor_mut().primary_cursor_mut().line = line_idx.saturating_sub(1);\n+                    let line_len = model.document().line_length(model.editor().primary_cursor().line);\n+                    model.editor_mut().primary_cursor_mut().column =\n+                        model.editor().primary_cursor().column.min(line_len);\n                 } else {\n                     // Deleted non-last line: stay at same line index, clamp column\n                     let new_line = line_idx.min(new_line_count.saturating_sub(1));\n                     let new_line_len = model.document().line_length(new_line);\n-                    model.editor_mut().cursor_mut().line = new_line;\n-                    model.editor_mut().cursor_mut().column =\n-                        model.editor().cursor().column.min(new_line_len);\n+                    model.editor_mut().primary_cursor_mut().line = new_line;\n+                    model.editor_mut().primary_cursor_mut().column =\n+                        model.editor().primary_cursor().column.min(new_line_len);\n                 }\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Delete {\n                     position: start_offset,\n                     text: deleted,\n                     cursor_before,\n```",
  "lineRange": [
    515,
    530
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                let cursor_before = *model.editor().cursor();\n\n                if model.editor().has_multiple_cursors() {",
  "new_str": "                let cursor_before = *model.editor().primary_cursor();\n\n                if model.editor().has_multiple_cursors() {"
}
```

## User

**Tool Result:** `toolu_vrtx_01VFkbbMSg9zeVQPwXMasdLz`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -723,9 +723,9 @@\n                 if text.is_empty() {\n                     return Some(Cmd::Redraw);\n                 }\n \n-                let cursor_before = *model.editor().cursor();\n+                let cursor_before = *model.editor().primary_cursor();\n \n                 if model.editor().has_multiple_cursors() {\n                     let lines: Vec<&str> = text.lines().collect();\n                     let cursor_count = model.editor().cursors.len();\n```",
  "lineRange": [
    727,
    729
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                } else {\n                    // Single cursor: use Replace if selection exists for atomic undo\n                    if !model.editor().selection().is_empty() {\n                        let (pos, deleted_text) = delete_selection(model).unwrap();\n\n                        model.document_mut().buffer.insert(pos, &text);\n\n                        // Move cursor to end of pasted text\n                        let new_offset = pos + text.chars().count();\n                        model.set_cursor_from_position(new_offset);\n\n                        let cursor_after = *model.editor().cursor();\n                        model.document_mut().push_edit(EditOperation::Replace {\n                            position: pos,\n                            deleted_text,\n                            inserted_text: text.clone(),\n                            cursor_before,\n                            cursor_after,\n                        });\n                    } else {\n                        let pos = model.cursor_buffer_position();\n\n                        model.document_mut().buffer.insert(pos, &text);\n\n                        // Move cursor to end of pasted text\n                        let new_offset = pos + text.chars().count();\n                        model.set_cursor_from_position(new_offset);\n\n                        let cursor_after = *model.editor().cursor();\n                        model.document_mut().push_edit(EditOperation::Insert {\n                            position: pos,\n                            text: text.clone(),\n                            cursor_before,\n                            cursor_after,",
  "new_str": "                } else {\n                    // Single cursor: use Replace if selection exists for atomic undo\n                    if !model.editor().primary_selection().is_empty() {\n                        let (pos, deleted_text) = delete_selection(model).unwrap();\n\n                        model.document_mut().buffer.insert(pos, &text);\n\n                        // Move cursor to end of pasted text\n                        let new_offset = pos + text.chars().count();\n                        model.set_cursor_from_position(new_offset);\n\n                        let cursor_after = *model.editor().primary_cursor();\n                        model.document_mut().push_edit(EditOperation::Replace {\n                            position: pos,\n                            deleted_text,\n                            inserted_text: text.clone(),\n                            cursor_before,\n                            cursor_after,\n                        });\n                    } else {\n                        let pos = model.cursor_buffer_position();\n\n                        model.document_mut().buffer.insert(pos, &text);\n\n                        // Move cursor to end of pasted text\n                        let new_offset = pos + text.chars().count();\n                        model.set_cursor_from_position(new_offset);\n\n                        let cursor_after = *model.editor().primary_cursor();\n                        model.document_mut().push_edit(EditOperation::Insert {\n                            position: pos,\n                            text: text.clone(),\n                            cursor_before,\n                            cursor_after,"
}
```

## User

**Tool Result:** `toolu_vrtx_017jnq2o7sJrFRg7vqx3smjA`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -765,18 +765,18 @@\n                         }\n                     }\n                 } else {\n                     // Single cursor: use Replace if selection exists for atomic undo\n-                    if !model.editor().selection().is_empty() {\n+                    if !model.editor().primary_selection().is_empty() {\n                         let (pos, deleted_text) = delete_selection(model).unwrap();\n \n                         model.document_mut().buffer.insert(pos, &text);\n \n                         // Move cursor to end of pasted text\n                         let new_offset = pos + text.chars().count();\n                         model.set_cursor_from_position(new_offset);\n \n-                        let cursor_after = *model.editor().cursor();\n+                        let cursor_after = *model.editor().primary_cursor();\n                         model.document_mut().push_edit(EditOperation::Replace {\n                             position: pos,\n                             deleted_text,\n                             inserted_text: text.clone(),\n@@ -791,9 +791,9 @@\n                         // Move cursor to end of pasted text\n                         let new_offset = pos + text.chars().count();\n                         model.set_cursor_from_position(new_offset);\n \n-                        let cursor_after = *model.editor().cursor();\n+                        let cursor_after = *model.editor().primary_cursor();\n                         model.document_mut().push_edit(EditOperation::Insert {\n                             position: pos,\n                             text: text.clone(),\n                             cursor_before,\n```",
  "lineRange": [
    767,
    800
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "        DocumentMsg::Duplicate => {\n            let cursor_before = *model.editor().cursor();\n            let selection = model.editor().selection().clone();\n\n            if selection.is_empty() {\n                // No selection: duplicate the current line\n                let line_idx = model.editor().cursor().line;\n                let column = model.editor().cursor().column;",
  "new_str": "        DocumentMsg::Duplicate => {\n            let cursor_before = *model.editor().primary_cursor();\n            let selection = model.editor().primary_selection().clone();\n\n            if selection.is_empty() {\n                // No selection: duplicate the current line\n                let line_idx = model.editor().primary_cursor().line;\n                let column = model.editor().primary_cursor().column;"
}
```

## User

**Tool Result:** `toolu_vrtx_01MeL8FPQi4ocaraW5UjQCej`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -811,15 +811,15 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::Duplicate => {\n-            let cursor_before = *model.editor().cursor();\n-            let selection = model.editor().selection().clone();\n+            let cursor_before = *model.editor().primary_cursor();\n+            let selection = model.editor().primary_selection().clone();\n \n             if selection.is_empty() {\n                 // No selection: duplicate the current line\n-                let line_idx = model.editor().cursor().line;\n-                let column = model.editor().cursor().column;\n+                let line_idx = model.editor().primary_cursor().line;\n+                let column = model.editor().primary_cursor().column;\n \n                 // Get the current line content\n                 let line_text = model.document().get_line(line_idx).unwrap_or_default();\n                 let has_newline = line_text.ends_with('\\n');\n```",
  "lineRange": [
    814,
    821
  ]
}
```

## Assistant

Now I need to check and fix the remaining occurrences in the Duplicate case:

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    845,
    920
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_015buZpCKd2BzmgeEny1u8x2`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 844 ...]\n845: \n846:                 model\n847:                     .document_mut()\n848:                     .buffer\n849:                     .insert(line_end_offset, &text_to_insert);\n850: \n851:                 // Move cursor to duplicated line at same column\n852:                 model.editor_mut().cursor_mut().line += 1;\n853:                 let new_line = model.editor().cursor().line;\n854:                 model.editor_mut().cursor_mut().column =\n855:                     column.min(model.document().line_length(new_line));\n856: \n857:                 let cursor_after = *model.editor().cursor();\n858:                 model.document_mut().push_edit(EditOperation::Insert {\n859:                     position: line_end_offset,\n860:                     text: text_to_insert,\n861:                     cursor_before,\n862:                     cursor_after,\n863:                 });\n864:             } else {\n865:                 // With selection: duplicate the selected text after selection end\n866:                 let sel_start = selection.start();\n867:                 let sel_end = selection.end();\n868: \n869:                 let start_offset = model\n870:                     .document()\n871:                     .cursor_to_offset(sel_start.line, sel_start.column);\n872:                 let end_offset = model\n873:                     .document()\n874:                     .cursor_to_offset(sel_end.line, sel_end.column);\n875: \n876:                 // Get selected text\n877:                 let selected_text: String = model\n878:                     .document()\n879:                     .buffer\n880:                     .slice(start_offset..end_offset)\n881:                     .chars()\n882:                     .collect();\n883: \n884:                 // Insert at end of selection\n885:                 model\n886:                     .document_mut()\n887:                     .buffer\n888:                     .insert(end_offset, &selected_text);\n889: \n890:                 // Move cursor to end of duplicated text\n891:                 let new_offset = end_offset + selected_text.chars().count();\n892:                 model.set_cursor_from_position(new_offset);\n893: \n894:                 // Clear selection\n895:                 model.editor_mut().clear_selection();\n896: \n897:                 let cursor_after = *model.editor().cursor();\n898:                 model.document_mut().push_edit(EditOperation::Insert {\n899:                     position: end_offset,\n900:                     text: selected_text,\n901:                     cursor_before,\n902:                     cursor_after,\n903:                 });\n904:             }\n905: \n906:             model.document_mut().is_modified = true;\n907:             model.ensure_cursor_visible();\n908:             model.reset_cursor_blink();\n909:             Some(Cmd::Redraw)\n910:         }\n911: \n912:         DocumentMsg::IndentLines => {\n913:             let cursor_before = *model.editor().cursor();\n914:             let selection = model.editor().selection().clone();\n915: \n916:             let start_line = selection.start().line;\n917:             let end_line = selection.end().line;\n918: \n919:             // Skip last line if selection ends at column 0\n920:             let effective_end = if selection.end().column == 0 && end_line > start_line {\n[... omitted lines 921 to ∞ ...]"
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                // Move cursor to duplicated line at same column\n                model.editor_mut().cursor_mut().line += 1;\n                let new_line = model.editor().cursor().line;\n                model.editor_mut().cursor_mut().column =\n                    column.min(model.document().line_length(new_line));\n\n                let cursor_after = *model.editor().cursor();",
  "new_str": "                // Move cursor to duplicated line at same column\n                model.editor_mut().primary_cursor_mut().line += 1;\n                let new_line = model.editor().primary_cursor().line;\n                model.editor_mut().primary_cursor_mut().column =\n                    column.min(model.document().line_length(new_line));\n\n                let cursor_after = *model.editor().primary_cursor();"
}
```

## User

**Tool Result:** `toolu_vrtx_01Xw1yRmBCGLMAwtC3iCpViA`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -848,14 +848,14 @@\n                     .buffer\n                     .insert(line_end_offset, &text_to_insert);\n \n                 // Move cursor to duplicated line at same column\n-                model.editor_mut().cursor_mut().line += 1;\n-                let new_line = model.editor().cursor().line;\n-                model.editor_mut().cursor_mut().column =\n+                model.editor_mut().primary_cursor_mut().line += 1;\n+                let new_line = model.editor().primary_cursor().line;\n+                model.editor_mut().primary_cursor_mut().column =\n                     column.min(model.document().line_length(new_line));\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Insert {\n                     position: line_end_offset,\n                     text: text_to_insert,\n                     cursor_before,\n```",
  "lineRange": [
    851,
    857
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                // Clear selection\n                model.editor_mut().clear_selection();\n\n                let cursor_after = *model.editor().cursor();\n                model.document_mut().push_edit(EditOperation::Insert {\n                    position: end_offset,\n                    text: selected_text,\n                    cursor_before,\n                    cursor_after,\n                });\n            }\n\n            model.document_mut().is_modified = true;\n            model.ensure_cursor_visible();\n            model.reset_cursor_blink();\n            Some(Cmd::Redraw)\n        }\n\n        DocumentMsg::IndentLines => {\n            let cursor_before = *model.editor().cursor();\n            let selection = model.editor().selection().clone();",
  "new_str": "                // Clear selection\n                model.editor_mut().clear_selection();\n\n                let cursor_after = *model.editor().primary_cursor();\n                model.document_mut().push_edit(EditOperation::Insert {\n                    position: end_offset,\n                    text: selected_text,\n                    cursor_before,\n                    cursor_after,\n                });\n            }\n\n            model.document_mut().is_modified = true;\n            model.ensure_cursor_visible();\n            model.reset_cursor_blink();\n            Some(Cmd::Redraw)\n        }\n\n        DocumentMsg::IndentLines => {\n            let cursor_before = *model.editor().primary_cursor();\n            let selection = model.editor().primary_selection().clone();"
}
```

## User

**Tool Result:** `toolu_vrtx_013MpfEnreAXEH4QQv7MRzzf`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -893,9 +893,9 @@\n \n                 // Clear selection\n                 model.editor_mut().clear_selection();\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Insert {\n                     position: end_offset,\n                     text: selected_text,\n                     cursor_before,\n@@ -909,10 +909,10 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::IndentLines => {\n-            let cursor_before = *model.editor().cursor();\n-            let selection = model.editor().selection().clone();\n+            let cursor_before = *model.editor().primary_cursor();\n+            let selection = model.editor().primary_selection().clone();\n \n             let start_line = selection.start().line;\n             let end_line = selection.end().line;\n \n```",
  "lineRange": [
    894,
    914
  ]
}
```

## Assistant

Now let me check and fix the remaining parts in IndentLines and UnindentLines:

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    920,
    1000
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_01X65mfDv7gyTqWNdZGJp9A5`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 919 ...]\n920:             let effective_end = if selection.end().column == 0 && end_line > start_line {\n921:                 end_line - 1\n922:             } else {\n923:                 end_line\n924:             };\n925: \n926:             let mut inserted_text = String::new();\n927: \n928:             // Insert tabs in reverse order (to preserve offsets)\n929:             for line in (start_line..=effective_end).rev() {\n930:                 let offset = model.document().cursor_to_offset(line, 0);\n931:                 model.document_mut().buffer.insert_char(offset, '\\t');\n932:                 inserted_text.push('\\t');\n933:             }\n934: \n935:             // Adjust cursor column +1\n936:             model.editor_mut().cursor_mut().column += 1;\n937: \n938:             // Adjust selection: bump both anchor and head columns +1\n939:             model.editor_mut().selection_mut().anchor.column += 1;\n940:             model.editor_mut().selection_mut().head.column += 1;\n941: \n942:             let cursor_after = *model.editor().cursor();\n943:             // Record single Insert for undo (position = first line start)\n944:             let first_offset = model.document().cursor_to_offset(start_line, 0);\n945:             model.document_mut().push_edit(EditOperation::Insert {\n946:                 position: first_offset,\n947:                 text: inserted_text,\n948:                 cursor_before,\n949:                 cursor_after,\n950:             });\n951: \n952:             model.document_mut().is_modified = true;\n953:             model.ensure_cursor_visible();\n954:             model.reset_cursor_blink();\n955:             Some(Cmd::Redraw)\n956:         }\n957: \n958:         DocumentMsg::UnindentLines => {\n959:             let cursor_before = *model.editor().cursor();\n960:             let selection = model.editor().selection().clone();\n961: \n962:             let (start_line, end_line) = if selection.is_empty() {\n963:                 let line = model.editor().cursor().line;\n964:                 (line, line)\n965:             } else {\n966:                 let end = if selection.end().column == 0\n967:                     && selection.end().line > selection.start().line\n968:                 {\n969:                     selection.end().line - 1\n970:                 } else {\n971:                     selection.end().line\n972:                 };\n973:                 (selection.start().line, end)\n974:             };\n975: \n976:             let mut total_removed = 0;\n977:             let mut removed_from_cursor_line = 0;\n978:             let mut removed_from_anchor_line = 0;\n979:             let mut removed_from_head_line = 0;\n980: \n981:             // Process in reverse order\n982:             for line in (start_line..=end_line).rev() {\n983:                 let line_start = model.document().cursor_to_offset(line, 0);\n984:                 let line_text: String = model.document().buffer.line(line).chars().collect();\n985: \n986:                 let chars_to_remove = if line_text.starts_with('\\t') {\n987:                     1\n988:                 } else {\n989:                     // Count leading spaces (up to 4)\n990:                     line_text.chars().take_while(|c| *c == ' ').count().min(4)\n991:                 };\n992: \n993:                 if chars_to_remove > 0 {\n994:                     model\n995:                         .document_mut()\n996:                         .buffer\n997:                         .remove(line_start..line_start + chars_to_remove);\n998:                     total_removed += chars_to_remove;\n999: \n1000:                     if line == model.editor().cursor().line {\n[... omitted lines 1001 to ∞ ...]"
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            // Adjust cursor column +1\n            model.editor_mut().cursor_mut().column += 1;\n\n            // Adjust selection: bump both anchor and head columns +1\n            model.editor_mut().selection_mut().anchor.column += 1;\n            model.editor_mut().selection_mut().head.column += 1;\n\n            let cursor_after = *model.editor().cursor();",
  "new_str": "            // Adjust cursor column +1\n            model.editor_mut().primary_cursor_mut().column += 1;\n\n            // Adjust selection: bump both anchor and head columns +1\n            model.editor_mut().primary_selection_mut().anchor.column += 1;\n            model.editor_mut().primary_selection_mut().head.column += 1;\n\n            let cursor_after = *model.editor().primary_cursor();"
}
```

## User

**Tool Result:** `toolu_vrtx_01BuNgKoY88DanERHRSdpC2j`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -932,15 +932,15 @@\n                 inserted_text.push('\\t');\n             }\n \n             // Adjust cursor column +1\n-            model.editor_mut().cursor_mut().column += 1;\n+            model.editor_mut().primary_cursor_mut().column += 1;\n \n             // Adjust selection: bump both anchor and head columns +1\n-            model.editor_mut().selection_mut().anchor.column += 1;\n-            model.editor_mut().selection_mut().head.column += 1;\n+            model.editor_mut().primary_selection_mut().anchor.column += 1;\n+            model.editor_mut().primary_selection_mut().head.column += 1;\n \n-            let cursor_after = *model.editor().cursor();\n+            let cursor_after = *model.editor().primary_cursor();\n             // Record single Insert for undo (position = first line start)\n             let first_offset = model.document().cursor_to_offset(start_line, 0);\n             model.document_mut().push_edit(EditOperation::Insert {\n                 position: first_offset,\n```",
  "lineRange": [
    935,
    942
  ]
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "        DocumentMsg::UnindentLines => {\n            let cursor_before = *model.editor().cursor();\n            let selection = model.editor().selection().clone();\n\n            let (start_line, end_line) = if selection.is_empty() {\n                let line = model.editor().cursor().line;\n                (line, line)",
  "new_str": "        DocumentMsg::UnindentLines => {\n            let cursor_before = *model.editor().primary_cursor();\n            let selection = model.editor().primary_selection().clone();\n\n            let (start_line, end_line) = if selection.is_empty() {\n                let line = model.editor().primary_cursor().line;\n                (line, line)"
}
```

## User

**Tool Result:** `toolu_vrtx_013vhYEsH6J7DNTXds6NHsa5`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -955,13 +955,13 @@\n             Some(Cmd::Redraw)\n         }\n \n         DocumentMsg::UnindentLines => {\n-            let cursor_before = *model.editor().cursor();\n-            let selection = model.editor().selection().clone();\n+            let cursor_before = *model.editor().primary_cursor();\n+            let selection = model.editor().primary_selection().clone();\n \n             let (start_line, end_line) = if selection.is_empty() {\n-                let line = model.editor().cursor().line;\n+                let line = model.editor().primary_cursor().line;\n                 (line, line)\n             } else {\n                 let end = if selection.end().column == 0\n                     && selection.end().line > selection.start().line\n```",
  "lineRange": [
    958,
    964
  ]
}
```

## Assistant

Now let me check the remaining parts of UnindentLines:

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    995,
    1050
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_01WuKMVkvSYLtbnhmPzTnjkg`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 994 ...]\n995:                         .document_mut()\n996:                         .buffer\n997:                         .remove(line_start..line_start + chars_to_remove);\n998:                     total_removed += chars_to_remove;\n999: \n1000:                     if line == model.editor().cursor().line {\n1001:                         removed_from_cursor_line = chars_to_remove;\n1002:                     }\n1003:                     if line == selection.anchor.line {\n1004:                         removed_from_anchor_line = chars_to_remove;\n1005:                     }\n1006:                     if line == selection.head.line {\n1007:                         removed_from_head_line = chars_to_remove;\n1008:                     }\n1009:                 }\n1010:             }\n1011: \n1012:             if total_removed > 0 {\n1013:                 // Calculate position before mutable borrows\n1014:                 let edit_position = model.document().cursor_to_offset(start_line, 0);\n1015: \n1016:                 // Adjust cursor\n1017:                 model.editor_mut().cursor_mut().column = model\n1018:                     .editor()\n1019:                     .cursor()\n1020:                     .column\n1021:                     .saturating_sub(removed_from_cursor_line);\n1022: \n1023:                 // Adjust selection if not empty\n1024:                 if !selection.is_empty() {\n1025:                     model.editor_mut().selection_mut().anchor.column = selection\n1026:                         .anchor\n1027:                         .column\n1028:                         .saturating_sub(removed_from_anchor_line);\n1029:                     model.editor_mut().selection_mut().head.column =\n1030:                         selection.head.column.saturating_sub(removed_from_head_line);\n1031:                 }\n1032: \n1033:                 let cursor_after = *model.editor().cursor();\n1034:                 model.document_mut().push_edit(EditOperation::Delete {\n1035:                     position: edit_position,\n1036:                     text: \"\\t\".repeat(total_removed),\n1037:                     cursor_before,\n1038:                     cursor_after,\n1039:                 });\n1040: \n1041:                 model.document_mut().is_modified = true;\n1042:             }\n1043: \n1044:             model.ensure_cursor_visible();\n1045:             model.reset_cursor_blink();\n1046:             Some(Cmd::Redraw)\n1047:         }\n1048:     }\n1049: }\n1050: \n[... omitted lines 1051 to ∞ ...]"
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                    if line == model.editor().cursor().line {\n                        removed_from_cursor_line = chars_to_remove;\n                    }\n                    if line == selection.anchor.line {\n                        removed_from_anchor_line = chars_to_remove;\n                    }\n                    if line == selection.head.line {\n                        removed_from_head_line = chars_to_remove;\n                    }\n                }\n            }\n\n            if total_removed > 0 {\n                // Calculate position before mutable borrows\n                let edit_position = model.document().cursor_to_offset(start_line, 0);\n\n                // Adjust cursor\n                model.editor_mut().cursor_mut().column = model\n                    .editor()\n                    .cursor()\n                    .column\n                    .saturating_sub(removed_from_cursor_line);\n\n                // Adjust selection if not empty\n                if !selection.is_empty() {\n                    model.editor_mut().selection_mut().anchor.column = selection\n                        .anchor\n                        .column\n                        .saturating_sub(removed_from_anchor_line);\n                    model.editor_mut().selection_mut().head.column =\n                        selection.head.column.saturating_sub(removed_from_head_line);\n                }\n\n                let cursor_after = *model.editor().cursor();",
  "new_str": "                    if line == model.editor().primary_cursor().line {\n                        removed_from_cursor_line = chars_to_remove;\n                    }\n                    if line == selection.anchor.line {\n                        removed_from_anchor_line = chars_to_remove;\n                    }\n                    if line == selection.head.line {\n                        removed_from_head_line = chars_to_remove;\n                    }\n                }\n            }\n\n            if total_removed > 0 {\n                // Calculate position before mutable borrows\n                let edit_position = model.document().cursor_to_offset(start_line, 0);\n\n                // Adjust cursor\n                model.editor_mut().primary_cursor_mut().column = model\n                    .editor()\n                    .primary_cursor()\n                    .column\n                    .saturating_sub(removed_from_cursor_line);\n\n                // Adjust selection if not empty\n                if !selection.is_empty() {\n                    model.editor_mut().primary_selection_mut().anchor.column = selection\n                        .anchor\n                        .column\n                        .saturating_sub(removed_from_anchor_line);\n                    model.editor_mut().primary_selection_mut().head.column =\n                        selection.head.column.saturating_sub(removed_from_head_line);\n                }\n\n                let cursor_after = *model.editor().primary_cursor();"
}
```

## User

**Tool Result:** `toolu_vrtx_019gUyN6MiAewtib2jMFmtDj`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -996,9 +996,9 @@\n                         .buffer\n                         .remove(line_start..line_start + chars_to_remove);\n                     total_removed += chars_to_remove;\n \n-                    if line == model.editor().cursor().line {\n+                    if line == model.editor().primary_cursor().line {\n                         removed_from_cursor_line = chars_to_remove;\n                     }\n                     if line == selection.anchor.line {\n                         removed_from_anchor_line = chars_to_remove;\n@@ -1013,25 +1013,25 @@\n                 // Calculate position before mutable borrows\n                 let edit_position = model.document().cursor_to_offset(start_line, 0);\n \n                 // Adjust cursor\n-                model.editor_mut().cursor_mut().column = model\n+                model.editor_mut().primary_cursor_mut().column = model\n                     .editor()\n-                    .cursor()\n+                    .primary_cursor()\n                     .column\n                     .saturating_sub(removed_from_cursor_line);\n \n                 // Adjust selection if not empty\n                 if !selection.is_empty() {\n-                    model.editor_mut().selection_mut().anchor.column = selection\n+                    model.editor_mut().primary_selection_mut().anchor.column = selection\n                         .anchor\n                         .column\n                         .saturating_sub(removed_from_anchor_line);\n-                    model.editor_mut().selection_mut().head.column =\n+                    model.editor_mut().primary_selection_mut().head.column =\n                         selection.head.column.saturating_sub(removed_from_head_line);\n                 }\n \n-                let cursor_after = *model.editor().cursor();\n+                let cursor_after = *model.editor().primary_cursor();\n                 model.document_mut().push_edit(EditOperation::Delete {\n                     position: edit_position,\n                     text: \"\\t\".repeat(total_removed),\n                     cursor_before,\n```",
  "lineRange": [
    1000,
    1033
  ]
}
```

## Assistant

Now let me check if there are still remaining cursor_mut() calls for DeleteLine (around line 510-515):

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    505,
    520
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_01Ty972H2i44Ps5KRuvtMMNw`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 504 ...]\n505:                     .slice(start_offset..end_offset)\n506:                     .chars()\n507:                     .collect();\n508:                 model.document_mut().buffer.remove(start_offset..end_offset);\n509: \n510:                 // Adjust cursor position\n511:                 let new_line_count = model.document().line_count();\n512:                 if new_line_count == 0 {\n513:                     model.editor_mut().cursor_mut().line = 0;\n514:                     model.editor_mut().cursor_mut().column = 0;\n515:                 } else if was_last_line {\n516:                     // Deleted last line: cursor goes to previous line, retain column (clamped)\n517:                     model.editor_mut().primary_cursor_mut().line = line_idx.saturating_sub(1);\n518:                     let line_len = model.document().line_length(model.editor().primary_cursor().line);\n519:                     model.editor_mut().primary_cursor_mut().column =\n520:                         model.editor().primary_cursor().column.min(line_len);\n[... omitted lines 521 to ∞ ...]"
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "                // Adjust cursor position\n                let new_line_count = model.document().line_count();\n                if new_line_count == 0 {\n                    model.editor_mut().cursor_mut().line = 0;\n                    model.editor_mut().cursor_mut().column = 0;\n                } else if was_last_line {",
  "new_str": "                // Adjust cursor position\n                let new_line_count = model.document().line_count();\n                if new_line_count == 0 {\n                    model.editor_mut().primary_cursor_mut().line = 0;\n                    model.editor_mut().primary_cursor_mut().column = 0;\n                } else if was_last_line {"
}
```

## User

**Tool Result:** `toolu_vrtx_01J7oG9YnYe4x46E8zqnZoMx`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -509,10 +509,10 @@\n \n                 // Adjust cursor position\n                 let new_line_count = model.document().line_count();\n                 if new_line_count == 0 {\n-                    model.editor_mut().cursor_mut().line = 0;\n-                    model.editor_mut().cursor_mut().column = 0;\n+                    model.editor_mut().primary_cursor_mut().line = 0;\n+                    model.editor_mut().primary_cursor_mut().column = 0;\n                 } else if was_last_line {\n                     // Deleted last line: cursor goes to previous line, retain column (clamped)\n                     model.editor_mut().primary_cursor_mut().line = line_idx.saturating_sub(1);\n                     let line_len = model.document().line_length(model.editor().primary_cursor().line);\n```",
  "lineRange": [
    510,
    515
  ]
}
```

## Assistant

Now let me verify the build:

**Tool Use:** `Bash`

```json
{
  "cmd": "cargo build 2>&1 | grep \"src/update/document.rs\"",
  "cwd": "~/code/rust-editor"
}
```

## User

**Tool Result:** `toolu_vrtx_01QV92VtHQQyojkhwWr9gimw`

```
{
  "output": "   --> src/update/document.rs:604:48\n   --> src/update/document.rs:657:48\n    --> src/update/document.rs:1064:33\n    --> src/update/document.rs:1073:33\n    --> src/update/document.rs:1087:33\n    --> src/update/document.rs:1152:33\n    --> src/update/document.rs:1164:33\n    --> src/update/document.rs:1178:33\n",
  "exitCode": 0
}
```

## Assistant

Let me check those remaining lines:

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    600,
    665
  ]
}
```

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    1060,
    1085
  ]
}
```

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    1145,
    1185
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_01VUXC6ebKWQsKZiB1uEAjnB`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 599 ...]\n600:                     .map(|(_, t)| t)\n601:                     .collect::<Vec<_>>()\n602:                     .join(\"\\n\");\n603:             } else {\n604:                 let selection = model.editor().selection().clone();\n605:                 if !selection.is_empty() {\n606:                     let start = selection.start();\n607:                     let end = selection.end();\n608:                     let start_offset = model.document().cursor_to_offset(start.line, start.column);\n609:                     let end_offset = model.document().cursor_to_offset(end.line, end.column);\n610:                     text_to_copy = model\n611:                         .document()\n612:                         .buffer\n613:                         .slice(start_offset..end_offset)\n614:                         .chars()\n615:                         .collect();\n616:                 }\n617:             }\n618: \n619:             if !text_to_copy.is_empty() {\n620:                 if let Ok(mut clipboard) = arboard::Clipboard::new() {\n621:                     let _ = clipboard.set_text(&text_to_copy);\n622:                 }\n623:                 model\n624:                     .ui\n625:                     .set_status(format!(\"Copied {} chars\", text_to_copy.len()));\n626:             }\n627: \n628:             Some(Cmd::Redraw)\n629:         }\n630: \n631:         DocumentMsg::Cut => {\n632:             let mut text_to_copy = String::new();\n633:             let has_selection;\n634: \n635:             if model.editor().has_multiple_cursors() {\n636:                 // Collect text from selections\n637:                 let mut parts = Vec::new();\n638:                 for selection in model.editor().selections.iter() {\n639:                     if !selection.is_empty() {\n640:                         let start = selection.start();\n641:                         let end = selection.end();\n642:                         let start_offset =\n643:                             model.document().cursor_to_offset(start.line, start.column);\n644:                         let end_offset = model.document().cursor_to_offset(end.line, end.column);\n645:                         let text: String = model\n646:                             .document()\n647:                             .buffer\n648:                             .slice(start_offset..end_offset)\n649:                             .chars()\n650:                             .collect();\n651:                         parts.push(text);\n652:                     }\n653:                 }\n654:                 text_to_copy = parts.join(\"\\n\");\n655:                 has_selection = !text_to_copy.is_empty();\n656:             } else {\n657:                 let selection = model.editor().selection().clone();\n658:                 has_selection = !selection.is_empty();\n659:                 if has_selection {\n660:                     let start = selection.start();\n661:                     let end = selection.end();\n662:                     let start_offset = model.document().cursor_to_offset(start.line, start.column);\n663:                     let end_offset = model.document().cursor_to_offset(end.line, end.column);\n664:                     text_to_copy = model\n665:                         .document()\n[... omitted lines 666 to ∞ ...]"
}
```

**Tool Result:** `toolu_vrtx_013pogx8DV2upjSgHBQBfbNk`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 1059 ...]\n1060:             model\n1061:                 .document_mut()\n1062:                 .buffer\n1063:                 .remove(*position..*position + text.chars().count());\n1064:             *model.editor_mut().cursor_mut() = *cursor_before;\n1065:         }\n1066:         EditOperation::Delete {\n1067:             position,\n1068:             text,\n1069:             cursor_before,\n1070:             ..\n1071:         } => {\n1072:             model.document_mut().buffer.insert(*position, text);\n1073:             *model.editor_mut().cursor_mut() = *cursor_before;\n1074:         }\n1075:         EditOperation::Replace {\n1076:             position,\n1077:             deleted_text,\n1078:             inserted_text,\n1079:             cursor_before,\n1080:             ..\n1081:         } => {\n1082:             model\n1083:                 .document_mut()\n1084:                 .buffer\n1085:                 .remove(*position..*position + inserted_text.chars().count());\n[... omitted lines 1086 to ∞ ...]"
}
```

**Tool Result:** `toolu_vrtx_01G4eYaw988WrLKSvyFaYNfz`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 1144 ...]\n1145:         EditOperation::Insert {\n1146:             position,\n1147:             text,\n1148:             cursor_after,\n1149:             ..\n1150:         } => {\n1151:             model.document_mut().buffer.insert(*position, text);\n1152:             *model.editor_mut().cursor_mut() = *cursor_after;\n1153:         }\n1154:         EditOperation::Delete {\n1155:             position,\n1156:             text,\n1157:             cursor_after,\n1158:             ..\n1159:         } => {\n1160:             model\n1161:                 .document_mut()\n1162:                 .buffer\n1163:                 .remove(*position..*position + text.chars().count());\n1164:             *model.editor_mut().cursor_mut() = *cursor_after;\n1165:         }\n1166:         EditOperation::Replace {\n1167:             position,\n1168:             deleted_text,\n1169:             inserted_text,\n1170:             cursor_after,\n1171:             ..\n1172:         } => {\n1173:             model\n1174:                 .document_mut()\n1175:                 .buffer\n1176:                 .remove(*position..*position + deleted_text.chars().count());\n1177:             model.document_mut().buffer.insert(*position, inserted_text);\n1178:             *model.editor_mut().cursor_mut() = *cursor_after;\n1179:         }\n1180:         EditOperation::Batch {\n1181:             operations,\n1182:             cursors_after,\n1183:             ..\n1184:         } => {\n1185:             // Redo in forward order\n[... omitted lines 1186 to ∞ ...]"
}
```

## Assistant

Now I need to fix the remaining instances - Copy/Cut selection checks and undo/redo functions:

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            } else {\n                let selection = model.editor().selection().clone();\n                if !selection.is_empty() {\n                    let start = selection.start();\n                    let end = selection.end();\n                    let start_offset = model.document().cursor_to_offset(start.line, start.column);\n                    let end_offset = model.document().cursor_to_offset(end.line, end.column);\n                    text_to_copy = model\n                        .document()\n                        .buffer\n                        .slice(start_offset..end_offset)\n                        .chars()\n                        .collect();\n                }\n            }\n\n            if !text_to_copy.is_empty() {\n                if let Ok(mut clipboard) = arboard::Clipboard::new() {\n                    let _ = clipboard.set_text(&text_to_copy);\n                }\n                model\n                    .ui\n                    .set_status(format!(\"Copied {} chars\", text_to_copy.len()));\n            }\n\n            Some(Cmd::Redraw)\n        }\n\n        DocumentMsg::Cut => {\n            let mut text_to_copy = String::new();\n            let has_selection;\n\n            if model.editor().has_multiple_cursors() {\n                // Collect text from selections\n                let mut parts = Vec::new();\n                for selection in model.editor().selections.iter() {\n                    if !selection.is_empty() {\n                        let start = selection.start();\n                        let end = selection.end();\n                        let start_offset =\n                            model.document().cursor_to_offset(start.line, start.column);\n                        let end_offset = model.document().cursor_to_offset(end.line, end.column);\n                        let text: String = model\n                            .document()\n                            .buffer\n                            .slice(start_offset..end_offset)\n                            .chars()\n                            .collect();\n                        parts.push(text);\n                    }\n                }\n                text_to_copy = parts.join(\"\\n\");\n                has_selection = !text_to_copy.is_empty();\n            } else {\n                let selection = model.editor().selection().clone();",
  "new_str": "            } else {\n                let selection = model.editor().primary_selection().clone();\n                if !selection.is_empty() {\n                    let start = selection.start();\n                    let end = selection.end();\n                    let start_offset = model.document().cursor_to_offset(start.line, start.column);\n                    let end_offset = model.document().cursor_to_offset(end.line, end.column);\n                    text_to_copy = model\n                        .document()\n                        .buffer\n                        .slice(start_offset..end_offset)\n                        .chars()\n                        .collect();\n                }\n            }\n\n            if !text_to_copy.is_empty() {\n                if let Ok(mut clipboard) = arboard::Clipboard::new() {\n                    let _ = clipboard.set_text(&text_to_copy);\n                }\n                model\n                    .ui\n                    .set_status(format!(\"Copied {} chars\", text_to_copy.len()));\n            }\n\n            Some(Cmd::Redraw)\n        }\n\n        DocumentMsg::Cut => {\n            let mut text_to_copy = String::new();\n            let has_selection;\n\n            if model.editor().has_multiple_cursors() {\n                // Collect text from selections\n                let mut parts = Vec::new();\n                for selection in model.editor().selections.iter() {\n                    if !selection.is_empty() {\n                        let start = selection.start();\n                        let end = selection.end();\n                        let start_offset =\n                            model.document().cursor_to_offset(start.line, start.column);\n                        let end_offset = model.document().cursor_to_offset(end.line, end.column);\n                        let text: String = model\n                            .document()\n                            .buffer\n                            .slice(start_offset..end_offset)\n                            .chars()\n                            .collect();\n                        parts.push(text);\n                    }\n                }\n                text_to_copy = parts.join(\"\\n\");\n                has_selection = !text_to_copy.is_empty();\n            } else {\n                let selection = model.editor().primary_selection().clone();"
}
```

## User

**Tool Result:** `toolu_vrtx_01LScqHxZZE9GjSxQZ3AYiJ2`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -600,9 +600,9 @@\n                     .map(|(_, t)| t)\n                     .collect::<Vec<_>>()\n                     .join(\"\\n\");\n             } else {\n-                let selection = model.editor().selection().clone();\n+                let selection = model.editor().primary_selection().clone();\n                 if !selection.is_empty() {\n                     let start = selection.start();\n                     let end = selection.end();\n                     let start_offset = model.document().cursor_to_offset(start.line, start.column);\n@@ -653,9 +653,9 @@\n                 }\n                 text_to_copy = parts.join(\"\\n\");\n                 has_selection = !text_to_copy.is_empty();\n             } else {\n-                let selection = model.editor().selection().clone();\n+                let selection = model.editor().primary_selection().clone();\n                 has_selection = !selection.is_empty();\n                 if has_selection {\n                     let start = selection.start();\n                     let end = selection.end();\n```",
  "lineRange": [
    603,
    657
  ]
}
```

## Assistant

Now fix the undo/redo functions:

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + text.chars().count());\n            *model.editor_mut().cursor_mut() = *cursor_before;\n        }\n        EditOperation::Delete {\n            position,\n            text,\n            cursor_before,\n            ..\n        } => {\n            model.document_mut().buffer.insert(*position, text);\n            *model.editor_mut().cursor_mut() = *cursor_before;\n        }\n        EditOperation::Replace {\n            position,\n            deleted_text,\n            inserted_text,\n            cursor_before,\n            ..\n        } => {\n            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + inserted_text.chars().count());",
  "new_str": "            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + text.chars().count());\n            *model.editor_mut().primary_cursor_mut() = *cursor_before;\n        }\n        EditOperation::Delete {\n            position,\n            text,\n            cursor_before,\n            ..\n        } => {\n            model.document_mut().buffer.insert(*position, text);\n            *model.editor_mut().primary_cursor_mut() = *cursor_before;\n        }\n        EditOperation::Replace {\n            position,\n            deleted_text,\n            inserted_text,\n            cursor_before,\n            ..\n        } => {\n            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + inserted_text.chars().count());"
}
```

## User

**Tool Result:** `toolu_vrtx_01AKmQ9axzkULJ7Z92BLsvMS`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -1060,18 +1060,18 @@\n             model\n                 .document_mut()\n                 .buffer\n                 .remove(*position..*position + text.chars().count());\n-            *model.editor_mut().cursor_mut() = *cursor_before;\n+            *model.editor_mut().primary_cursor_mut() = *cursor_before;\n         }\n         EditOperation::Delete {\n             position,\n             text,\n             cursor_before,\n             ..\n         } => {\n             model.document_mut().buffer.insert(*position, text);\n-            *model.editor_mut().cursor_mut() = *cursor_before;\n+            *model.editor_mut().primary_cursor_mut() = *cursor_before;\n         }\n         EditOperation::Replace {\n             position,\n             deleted_text,\n```",
  "lineRange": [
    1060,
    1085
  ]
}
```

## Assistant

**Tool Use:** `Read`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "read_range": [
    1080,
    1100
  ]
}
```

## User

**Tool Result:** `toolu_vrtx_01GtWu32HASw8isYWV26krmb`

```
{
  "absolutePath": "~/code/rust-editor/src/update/document.rs",
  "content": "[... omitted lines 1 to 1079 ...]\n1080:             ..\n1081:         } => {\n1082:             model\n1083:                 .document_mut()\n1084:                 .buffer\n1085:                 .remove(*position..*position + inserted_text.chars().count());\n1086:             model.document_mut().buffer.insert(*position, deleted_text);\n1087:             *model.editor_mut().cursor_mut() = *cursor_before;\n1088:         }\n1089:         EditOperation::Batch {\n1090:             operations,\n1091:             cursors_before,\n1092:             ..\n1093:         } => {\n1094:             // Undo in reverse order\n1095:             for op in operations.iter().rev() {\n1096:                 apply_undo_operation_buffer_only(model, op);\n1097:             }\n1098:             // Restore all cursors\n1099:             let editor = model.editor_mut();\n1100:             editor.cursors = cursors_before.clone();\n[... omitted lines 1101 to ∞ ...]"
}
```

## Assistant

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "            model.document_mut().buffer.insert(*position, deleted_text);\n            *model.editor_mut().cursor_mut() = *cursor_before;\n        }\n        EditOperation::Batch {\n            operations,\n            cursors_before,\n            ..\n        } => {\n            // Undo in reverse order\n            for op in operations.iter().rev() {\n                apply_undo_operation_buffer_only(model, op);\n            }\n            // Restore all cursors\n            let editor = model.editor_mut();\n            editor.cursors = cursors_before.clone();",
  "new_str": "            model.document_mut().buffer.insert(*position, deleted_text);\n            *model.editor_mut().primary_cursor_mut() = *cursor_before;\n        }\n        EditOperation::Batch {\n            operations,\n            cursors_before,\n            ..\n        } => {\n            // Undo in reverse order\n            for op in operations.iter().rev() {\n                apply_undo_operation_buffer_only(model, op);\n            }\n            // Restore all cursors\n            let editor = model.editor_mut();\n            editor.cursors = cursors_before.clone();"
}
```

## User

**Tool Result:** `toolu_vrtx_01C5LybCtatQWrH9ipb1Rowz`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -1083,9 +1083,9 @@\n                 .document_mut()\n                 .buffer\n                 .remove(*position..*position + inserted_text.chars().count());\n             model.document_mut().buffer.insert(*position, deleted_text);\n-            *model.editor_mut().cursor_mut() = *cursor_before;\n+            *model.editor_mut().primary_cursor_mut() = *cursor_before;\n         }\n         EditOperation::Batch {\n             operations,\n             cursors_before,\n```",
  "lineRange": [
    1086,
    1100
  ]
}
```

## Assistant

Now fix the redo operations:

**Tool Use:** `edit_file`

```json
{
  "path": "~/code/rust-editor/src/update/document.rs",
  "old_str": "        EditOperation::Insert {\n            position,\n            text,\n            cursor_after,\n            ..\n        } => {\n            model.document_mut().buffer.insert(*position, text);\n            *model.editor_mut().cursor_mut() = *cursor_after;\n        }\n        EditOperation::Delete {\n            position,\n            text,\n            cursor_after,\n            ..\n        } => {\n            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + text.chars().count());\n            *model.editor_mut().cursor_mut() = *cursor_after;\n        }\n        EditOperation::Replace {\n            position,\n            deleted_text,\n            inserted_text,\n            cursor_after,\n            ..\n        } => {\n            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + deleted_text.chars().count());\n            model.document_mut().buffer.insert(*position, inserted_text);\n            *model.editor_mut().cursor_mut() = *cursor_after;\n        }\n        EditOperation::Batch {\n            operations,\n            cursors_after,\n            ..\n        } => {\n            // Redo in forward order",
  "new_str": "        EditOperation::Insert {\n            position,\n            text,\n            cursor_after,\n            ..\n        } => {\n            model.document_mut().buffer.insert(*position, text);\n            *model.editor_mut().primary_cursor_mut() = *cursor_after;\n        }\n        EditOperation::Delete {\n            position,\n            text,\n            cursor_after,\n            ..\n        } => {\n            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + text.chars().count());\n            *model.editor_mut().primary_cursor_mut() = *cursor_after;\n        }\n        EditOperation::Replace {\n            position,\n            deleted_text,\n            inserted_text,\n            cursor_after,\n            ..\n        } => {\n            model\n                .document_mut()\n                .buffer\n                .remove(*position..*position + deleted_text.chars().count());\n            model.document_mut().buffer.insert(*position, inserted_text);\n            *model.editor_mut().primary_cursor_mut() = *cursor_after;\n        }\n        EditOperation::Batch {\n            operations,\n            cursors_after,\n            ..\n        } => {\n            // Redo in forward order"
}
```

## User

**Tool Result:** `toolu_vrtx_018AH6dM7TEqYMcVED51HJiE`

```
{
  "diff": "```diff\nIndex: ~/code/rust-editor/src/update/document.rs\n===================================================================\n--- ~/code/rust-editor/src/update/document.rs\toriginal\n+++ ~/code/rust-editor/src/update/document.rs\tmodified\n@@ -1148,9 +1148,9 @@\n             cursor_after,\n             ..\n         } => {\n             model.document_mut().buffer.insert(*position, text);\n-            *model.editor_mut().cursor_mut() = *cursor_after;\n+            *model.editor_mut().primary_cursor_mut() = *cursor_after;\n         }\n         EditOperation::Delete {\n             position,\n             text,\n@@ -1160,9 +1160,9 @@\n             model\n                 .document_mut()\n                 .buffer\n                 .remove(*position..*position + text.chars().count());\n-            *model.editor_mut().cursor_mut() = *cursor_after;\n+            *model.editor_mut().primary_cursor_mut() = *cursor_after;\n         }\n         EditOperation::Replace {\n             position,\n             deleted_text,\n@@ -1174,9 +1174,9 @@\n                 .document_mut()\n                 .buffer\n                 .remove(*position..*position + deleted_text.chars().count());\n             model.document_mut().buffer.insert(*position, inserted_text);\n-            *model.editor_mut().cursor_mut() = *cursor_after;\n+            *model.editor_mut().primary_cursor_mut() = *cursor_after;\n         }\n         EditOperation::Batch {\n             operations,\n             cursors_after,\n```",
  "lineRange": [
    1145,
    1185
  ]
}
```

## Assistant

Now let's verify no errors remain:

**Tool Use:** `Bash`

```json
{
  "cmd": "cargo build 2>&1 | grep \"src/update/document.rs\"",
  "cwd": "~/code/rust-editor"
}
```

## User

**Tool Result:** `toolu_vrtx_01VshVmVgQES4PfeUgUMKrp4`

```
{
  "output": "",
  "exitCode": 1
}
```

## Assistant

No errors in `src/update/document.rs`. The exit code 1 is from grep finding no matches, which means there are no build errors in this file. Let me verify the build completes successfully:

**Tool Use:** `Bash`

```json
{
  "cmd": "cargo build 2>&1 | tail -5",
  "cwd": "~/code/rust-editor"
}
```

## User

**Tool Result:** `toolu_vrtx_015uzcf4k9Ze8stS7C7R7n3b`

```
{
  "output": "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s\n",
  "exitCode": 0
}
```

## Assistant

All `cursor()/cursor_mut()/selection()/selection_mut()` calls in `src/update/document.rs` have been updated to their `primary_` prefixed versions. The build completes successfully with no errors.
