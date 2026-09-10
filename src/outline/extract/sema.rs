use tree_sitter::Node;

use super::{flat_sym, FlatSymbol, OutlineKind};

pub(super) fn extract(root: Node<'_>, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        let mut children = node.walk();
        let mut forms = node
            .named_children(&mut children)
            .filter(|node| !node.is_extra());
        let head = forms
            .next()
            .and_then(|head| head.utf8_text(source.as_bytes()).ok());
        let quoted = matches!(node.kind(), "quote" | "quasiquote")
            || (node.kind() == "list" && matches!(head, Some("quote" | "quasiquote")));
        if node.kind() == "list" && !quoted {
            if let (Some(form), Some(target)) = (head, forms.next()) {
                let (kind, name) = match form {
                    "define" | "def" if target.kind() == "list" => {
                        let mut walk = target.walk();
                        let name = target
                            .named_children(&mut walk)
                            .find(|node| !node.is_extra());
                        (Some(OutlineKind::Function), name)
                    }
                    "define" | "def" | "defpolicy" => (Some(OutlineKind::Constant), Some(target)),
                    "defun" | "defn" | "defmacro" | "define-syntax" | "defagent" | "deftool"
                    | "defworkflow" => (Some(OutlineKind::Function), Some(target)),
                    "define-record-type" => (Some(OutlineKind::Struct), Some(target)),
                    "module" => (Some(OutlineKind::Module), Some(target)),
                    _ => (None, None),
                };
                if let (Some(kind), Some(name)) =
                    (kind, name.filter(|node| node.kind() == "symbol"))
                {
                    if let Ok(name) = name.utf8_text(source.as_bytes()) {
                        let mut symbol = flat_sym(kind, name, &node);
                        // Tree-sitter columns are bytes; editor navigation uses characters.
                        symbol.range.start_col = source
                            [node.start_byte() - node.start_position().column..node.start_byte()]
                            .chars()
                            .count();
                        symbol.range.end_col = source
                            [node.end_byte() - node.end_position().column..node.end_byte()]
                            .chars()
                            .count();
                        symbols.push(symbol);
                    }
                }
            }
        }
        if symbols.len() >= 10_000 {
            break;
        }
        if !quoted && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return symbols;
            }
        }
    }
    symbols
}

#[cfg(test)]
mod tests {
    use crate::{outline::extract_outline, syntax::LanguageId};

    #[test]
    fn sema_outline_columns_count_unicode_characters() {
        let source = "\"🙂\" (define π 3)";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_sema::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let outline = extract_outline(&tree, source, LanguageId::Sema, 1);
        assert_eq!(outline.roots[0].range.start_col, 4);
        assert_eq!(outline.roots[0].range.end_col, source.chars().count());
    }

    #[test]
    fn sema_outline_tracks_definitions_and_skips_quoted_data() {
        let source = "(def π 3.14)\n(define (square x) (* x x))\n(defn run (x) x)\n(defmacro m (x) x)\n(defworkflow deploy \"ship\" {} (step \"build\"))\n(defpolicy safe {})\n(define-record-type point (make-point x) point? (x point-x))\n'(define fake 1)\n(quote (def fake2 2))\n(module tools (define helper 1))\n";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_sema::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(!tree.root_node().has_error());
        let outline = extract_outline(&tree, source, LanguageId::Sema, 1);
        assert_eq!(
            outline
                .roots
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            ["π", "square", "run", "m", "deploy", "safe", "point", "tools"]
        );
        assert_eq!(outline.roots[1].kind, super::OutlineKind::Function);
        assert_eq!(outline.roots[6].kind, super::OutlineKind::Struct);
        assert_eq!(outline.roots[7].children[0].name, "helper");
    }
}
