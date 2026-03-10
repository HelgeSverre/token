//! Shared tree traversal helpers for sidebar-style views.

use crate::util::tree::TreeNodeLike;

/// Viewport configuration for rendering a flattened tree window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRenderLayout {
    pub start_y: usize,
    pub viewport_height: usize,
    pub row_height: usize,
    pub scroll_offset: usize,
}

impl TreeRenderLayout {
    pub fn new(
        start_y: usize,
        viewport_height: usize,
        row_height: usize,
        scroll_offset: usize,
    ) -> Self {
        Self {
            start_y,
            viewport_height,
            row_height,
            scroll_offset,
        }
    }

    #[inline]
    fn end_y(&self) -> usize {
        self.start_y.saturating_add(self.viewport_height)
    }
}

/// Metadata for a visible tree row.
#[derive(Debug, Clone, Copy)]
pub struct TreeRow<'a, T> {
    pub node: &'a T,
    pub depth: usize,
    pub index: usize,
    pub row_y: usize,
}

/// Final traversal state after rendering a tree viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRenderState {
    pub next_row_y: usize,
    pub next_index: usize,
}

/// Walk a tree in display order, skipping rows before `scroll_offset` and
/// stopping once the viewport is full.
pub fn render_tree<T, FExpanded, FRow>(
    roots: &[T],
    layout: TreeRenderLayout,
    is_expanded: FExpanded,
    mut render_row: FRow,
) -> TreeRenderState
where
    T: TreeNodeLike,
    FExpanded: Fn(&T) -> bool,
    FRow: for<'a> FnMut(TreeRow<'a, T>),
{
    fn render_node<'a, T, FExpanded, FRow>(
        node: &'a T,
        layout: &TreeRenderLayout,
        next_row_y: &mut usize,
        next_index: &mut usize,
        depth: usize,
        is_expanded: &FExpanded,
        render_row: &mut FRow,
    ) where
        T: TreeNodeLike,
        FExpanded: Fn(&T) -> bool,
        FRow: FnMut(TreeRow<'a, T>),
    {
        if *next_index >= layout.scroll_offset && *next_row_y >= layout.end_y() {
            return;
        }

        let index = *next_index;
        *next_index += 1;

        if index >= layout.scroll_offset {
            if *next_row_y >= layout.end_y() {
                return;
            }

            render_row(TreeRow {
                node,
                depth,
                index,
                row_y: *next_row_y,
            });
            *next_row_y += layout.row_height;
        }

        if is_expanded(node) {
            for child in node.children() {
                render_node(
                    child,
                    layout,
                    next_row_y,
                    next_index,
                    depth + 1,
                    is_expanded,
                    render_row,
                );
            }
        }
    }

    let mut next_row_y = layout.start_y;
    let mut next_index = 0;

    for root in roots {
        render_node(
            root,
            &layout,
            &mut next_row_y,
            &mut next_index,
            0,
            &is_expanded,
            &mut render_row,
        );

        if next_index >= layout.scroll_offset && next_row_y >= layout.end_y() {
            break;
        }
    }

    TreeRenderState {
        next_row_y,
        next_index,
    }
}

#[cfg(test)]
mod tests {
    use super::{render_tree, TreeRenderLayout, TreeRenderState, TreeRow};
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

    #[test]
    fn renders_only_visible_window() {
        let roots = sample_tree();
        let mut rows = Vec::new();

        let state = render_tree(
            &roots,
            TreeRenderLayout::new(10, 20, 10, 1),
            |node| node.expanded,
            |row: TreeRow<'_, TestNode>| rows.push((row.node.id, row.depth, row.index, row.row_y)),
        );

        assert_eq!(rows, vec![("child-a", 1, 1, 10), ("child-b", 1, 2, 20),]);
        assert_eq!(
            state,
            TreeRenderState {
                next_row_y: 30,
                next_index: 3,
            }
        );
    }

    #[test]
    fn walks_children_above_viewport_to_reach_visible_rows() {
        let roots = sample_tree();
        let mut rows = Vec::new();

        let state = render_tree(
            &roots,
            TreeRenderLayout::new(0, 10, 10, 3),
            |node| node.expanded,
            |row: TreeRow<'_, TestNode>| rows.push((row.node.id, row.depth, row.index, row.row_y)),
        );

        assert_eq!(rows, vec![("grandchild", 2, 3, 0)]);
        assert_eq!(
            state,
            TreeRenderState {
                next_row_y: 10,
                next_index: 4,
            }
        );
    }
}
