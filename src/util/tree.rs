//! Shared tree traversal utilities used across render, update, and hit testing.

use crate::model::FileNode;
use crate::outline::OutlineNode;

/// Common interface for tree nodes that can be traversed in display order.
pub trait TreeNodeLike {
    /// Child nodes in display order.
    fn children(&self) -> &[Self]
    where
        Self: Sized;
}

impl TreeNodeLike for FileNode {
    fn children(&self) -> &[Self] {
        &self.children
    }
}

impl TreeNodeLike for OutlineNode {
    fn children(&self) -> &[Self] {
        &self.children
    }
}

/// Metadata for a visible tree row.
#[derive(Debug, Clone, Copy)]
pub struct VisibleTreeRow<'a, T> {
    pub node: &'a T,
    pub depth: usize,
    pub index: usize,
}

/// Count visible nodes in a tree using the caller's expansion rule.
pub fn visible_tree_count<T, FExpanded>(roots: &[T], is_expanded: FExpanded) -> usize
where
    T: TreeNodeLike,
    FExpanded: Fn(&T) -> bool,
{
    fn count_node<T, FExpanded>(node: &T, is_expanded: &FExpanded) -> usize
    where
        T: TreeNodeLike,
        FExpanded: Fn(&T) -> bool,
    {
        let mut count = 1;

        if is_expanded(node) {
            for child in node.children() {
                count += count_node(child, is_expanded);
            }
        }

        count
    }

    roots
        .iter()
        .map(|node| count_node(node, &is_expanded))
        .sum()
}

/// Get the visible row at a flattened tree index.
pub fn visible_tree_row_at_index<'a, T, FExpanded>(
    roots: &'a [T],
    target: usize,
    is_expanded: FExpanded,
) -> Option<VisibleTreeRow<'a, T>>
where
    T: TreeNodeLike,
    FExpanded: Fn(&T) -> bool,
{
    fn row_at_index<'a, T, FExpanded>(
        node: &'a T,
        target: usize,
        current: &mut usize,
        depth: usize,
        is_expanded: &FExpanded,
    ) -> Option<VisibleTreeRow<'a, T>>
    where
        T: TreeNodeLike,
        FExpanded: Fn(&T) -> bool,
    {
        if *current == target {
            return Some(VisibleTreeRow {
                node,
                depth,
                index: *current,
            });
        }
        *current += 1;

        if is_expanded(node) {
            for child in node.children() {
                if let Some(found) = row_at_index(child, target, current, depth + 1, is_expanded) {
                    return Some(found);
                }
            }
        }

        None
    }

    let mut current = 0;
    for node in roots {
        if let Some(found) = row_at_index(node, target, &mut current, 0, &is_expanded) {
            return Some(found);
        }
    }

    None
}

/// Find the visible row matching a predicate.
pub fn visible_tree_row_matching<'a, T, FExpanded, FMatch>(
    roots: &'a [T],
    is_expanded: FExpanded,
    matches: FMatch,
) -> Option<VisibleTreeRow<'a, T>>
where
    T: TreeNodeLike,
    FExpanded: Fn(&T) -> bool,
    FMatch: Fn(&T) -> bool,
{
    fn row_matching<'a, T, FExpanded, FMatch>(
        node: &'a T,
        current: &mut usize,
        depth: usize,
        is_expanded: &FExpanded,
        matches: &FMatch,
    ) -> Option<VisibleTreeRow<'a, T>>
    where
        T: TreeNodeLike,
        FExpanded: Fn(&T) -> bool,
        FMatch: Fn(&T) -> bool,
    {
        if matches(node) {
            return Some(VisibleTreeRow {
                node,
                depth,
                index: *current,
            });
        }
        *current += 1;

        if is_expanded(node) {
            for child in node.children() {
                if let Some(found) = row_matching(child, current, depth + 1, is_expanded, matches) {
                    return Some(found);
                }
            }
        }

        None
    }

    let mut current = 0;
    for node in roots {
        if let Some(found) = row_matching(node, &mut current, 0, &is_expanded, &matches) {
            return Some(found);
        }
    }

    None
}

/// Find the flattened visible index of a matching node.
pub fn visible_tree_index_of<T, FExpanded, FMatch>(
    roots: &[T],
    is_expanded: FExpanded,
    matches: FMatch,
) -> Option<usize>
where
    T: TreeNodeLike,
    FExpanded: Fn(&T) -> bool,
    FMatch: Fn(&T) -> bool,
{
    visible_tree_row_matching(roots, is_expanded, matches).map(|row| row.index)
}

#[cfg(test)]
mod tests {
    use super::{
        visible_tree_count, visible_tree_index_of, visible_tree_row_at_index,
        visible_tree_row_matching, TreeNodeLike,
    };

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
    fn counts_only_visible_nodes() {
        let roots = sample_tree();

        assert_eq!(visible_tree_count(&roots, |node| node.expanded), 5);
    }

    #[test]
    fn returns_row_metadata_for_index() {
        let roots = sample_tree();
        let row = visible_tree_row_at_index(&roots, 3, |node| node.expanded).unwrap();

        assert_eq!(row.node.id, "grandchild");
        assert_eq!(row.depth, 2);
        assert_eq!(row.index, 3);
    }

    #[test]
    fn finds_matching_row_and_index() {
        let roots = sample_tree();
        let row =
            visible_tree_row_matching(&roots, |node| node.expanded, |node| node.id == "sibling")
                .unwrap();

        assert_eq!(row.depth, 0);
        assert_eq!(row.index, 4);
        assert_eq!(
            visible_tree_index_of(&roots, |node| node.expanded, |node| node.id == "child-b"),
            Some(2)
        );
    }

    #[test]
    fn skips_hidden_descendants_when_branch_is_collapsed() {
        let roots = vec![TestNode::branch(
            "root",
            false,
            vec![TestNode::leaf("hidden-child")],
        )];

        assert_eq!(visible_tree_count(&roots, |node| node.expanded), 1);
        assert!(visible_tree_row_matching(
            &roots,
            |node| node.expanded,
            |node| node.id == "hidden-child"
        )
        .is_none());
    }
}
