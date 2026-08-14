//! Shared tree traversal helpers for sidebar-style views.

use crate::layout::{snapshot::snap, RowListView};
use crate::util::tree::TreeNodeLike;

/// Metadata for a visible tree row.
#[derive(Debug, Clone, Copy)]
pub struct TreeRow<'a, T> {
    pub node: &'a T,
    pub depth: usize,
    pub index: usize,
    pub row_y: usize,
}

/// Walk a tree in display order, skipping rows before `scroll_offset` and
/// stopping after the solved Clay row list's drawn range.
pub fn render_tree<T, FExpanded, FRow>(
    roots: &[T],
    rows: RowListView,
    is_expanded: FExpanded,
    mut render_row: FRow,
) where
    T: TreeNodeLike,
    FExpanded: Fn(&T) -> bool,
    FRow: for<'a> FnMut(TreeRow<'a, T>),
{
    fn render_node<'a, T, FExpanded, FRow>(
        node: &'a T,
        rows: RowListView,
        drawn: &std::ops::Range<usize>,
        next_index: &mut usize,
        depth: usize,
        is_expanded: &FExpanded,
        render_row: &mut FRow,
    ) -> bool
    where
        T: TreeNodeLike,
        FExpanded: Fn(&T) -> bool,
        FRow: FnMut(TreeRow<'a, T>),
    {
        if *next_index >= drawn.end {
            return true;
        }

        let index = *next_index;
        *next_index += 1;

        if drawn.contains(&index) {
            let Some(row_rect) = rows.row_rect(index) else {
                return true;
            };
            render_row(TreeRow {
                node,
                depth,
                index,
                row_y: snap(row_rect).1,
            });
        }

        if is_expanded(node) {
            for child in node.children() {
                if render_node(
                    child,
                    rows,
                    drawn,
                    next_index,
                    depth + 1,
                    is_expanded,
                    render_row,
                ) {
                    return true;
                }
            }
        }
        *next_index >= drawn.end
    }

    let drawn = rows.drawn_range();
    let mut next_index = 0;

    for root in roots {
        if render_node(
            root,
            rows,
            &drawn,
            &mut next_index,
            0,
            &is_expanded,
            &mut render_row,
        ) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{render_tree, TreeRow};
    use crate::layout::{Content, ElementDecl, RowListDecl, SizingAxes, UiKey, UiTree};
    use crate::model::editor_area::Rect;
    use crate::util::tree::TreeNodeLike;

    #[derive(Debug)]
    struct TestNode {
        id: &'static str,
        expanded: bool,
        children: Vec<TestNode>,
    }

    impl TestNode {
        fn branch(id: &'static str, expanded: bool, children: Vec<TestNode>) -> Self {
            Self {
                id,
                expanded,
                children,
            }
        }

        fn leaf(id: &'static str) -> Self {
            Self {
                id,
                expanded: false,
                children: Vec::new(),
            }
        }
    }

    impl TreeNodeLike for TestNode {
        fn children(&self) -> &[Self] {
            &self.children
        }
    }

    fn sample_tree() -> Vec<TestNode> {
        vec![
            TestNode::branch(
                "root",
                true,
                vec![
                    TestNode::leaf("child-a"),
                    TestNode::branch("child-b", true, vec![TestNode::leaf("grandchild")]),
                ],
            ),
            TestNode::leaf("sibling"),
        ]
    }

    fn row_view(
        start_y: usize,
        viewport_height: usize,
        row_height: usize,
        count: usize,
        scroll_offset: usize,
    ) -> crate::layout::RowListView {
        let mut tree = UiTree::new();
        tree.leaf(ElementDecl {
            key: Some(UiKey::Sidebar),
            sizing: SizingAxes::grow(),
            content: Content::RowList(RowListDecl {
                row_height: row_height as f32,
                count,
                scroll_offset,
            }),
            ..Default::default()
        });
        let mut measure = crate::layout::CellMeasure {
            char_width: 8.0,
            line_height: 16.0,
        };
        tree.solve(
            Rect::new(0.0, start_y as f32, 100.0, viewport_height as f32),
            1.0,
            &mut measure,
        )
        .row_list(UiKey::Sidebar)
        .expect("test tree declares sidebar rows")
    }

    #[test]
    fn renders_only_visible_window() {
        let roots = sample_tree();
        let mut rows = Vec::new();

        render_tree(
            &roots,
            row_view(10, 20, 10, 5, 1),
            |node| node.expanded,
            |row: TreeRow<'_, TestNode>| rows.push((row.node.id, row.depth, row.index, row.row_y)),
        );

        assert_eq!(rows, vec![("child-a", 1, 1, 10), ("child-b", 1, 2, 20),]);
    }

    #[test]
    fn walks_children_above_viewport_to_reach_visible_rows() {
        let roots = sample_tree();
        let mut rows = Vec::new();

        render_tree(
            &roots,
            row_view(0, 10, 10, 5, 3),
            |node| node.expanded,
            |row: TreeRow<'_, TestNode>| rows.push((row.node.id, row.depth, row.index, row.row_y)),
        );

        assert_eq!(rows, vec![("grandchild", 2, 3, 0)]);
    }
}
