use crate::interop::{CompressedEdge, RegionEdge};
use crate::types::*;
use crate::util::arena::Arena;

// ---------------------------------------------------------------------------
// AltTreeEdge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AltTreeEdge {
    pub alt_tree_node: AltTreeIdx,
    pub edge: CompressedEdge,
}

impl AltTreeEdge {
    pub fn empty() -> Self {
        AltTreeEdge {
            alt_tree_node: AltTreeIdx(u32::MAX),
            edge: CompressedEdge::empty(),
        }
    }

    pub fn new(alt_tree_node: AltTreeIdx, edge: CompressedEdge) -> Self {
        AltTreeEdge {
            alt_tree_node,
            edge,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.alt_tree_node.0 == u32::MAX
    }
}

// ---------------------------------------------------------------------------
// AltTreePruneResult
// ---------------------------------------------------------------------------

pub struct AltTreePruneResult {
    pub orphan_edges: Vec<AltTreeEdge>,
    pub pruned_path_region_edges: Vec<RegionEdge>,
    /// Regions whose alt_tree_node should be cleared by the caller.
    pub regions_to_clear: Vec<RegionIdx>,
}

// ---------------------------------------------------------------------------
// AltTreeNode
// ---------------------------------------------------------------------------

/// Each AltTreeNode represents a *pair* of alternating-tree nodes:
/// an inner (shrinking) region and an outer (growing) region.
/// The root node has `inner_region = None`.
pub struct AltTreeNode {
    pub inner_region: Option<RegionIdx>,
    pub outer_region: Option<RegionIdx>,
    pub inner_to_outer_edge: CompressedEdge,
    pub parent: Option<AltTreeEdge>,
    pub children: Vec<AltTreeEdge>,
    pub visited: bool,
}

impl Default for AltTreeNode {
    fn default() -> Self {
        AltTreeNode {
            inner_region: None,
            outer_region: None,
            inner_to_outer_edge: CompressedEdge::empty(),
            parent: None,
            children: Vec::new(),
            visited: false,
        }
    }
}

impl AltTreeNode {
    pub fn reset_for_reuse(&mut self) {
        self.inner_region = None;
        self.outer_region = None;
        self.inner_to_outer_edge = CompressedEdge::empty();
        self.parent = None;
        self.children.clear();
        self.visited = false;
    }

    /// Root-only constructor: outer region only, no inner.
    pub fn new_root(outer_region: RegionIdx) -> Self {
        AltTreeNode {
            inner_region: None,
            outer_region: Some(outer_region),
            inner_to_outer_edge: CompressedEdge::empty(),
            parent: None,
            children: Vec::new(),
            visited: false,
        }
    }

    /// Non-root constructor: inner + outer region pair.
    pub fn new_pair(
        inner_region: RegionIdx,
        outer_region: RegionIdx,
        inner_to_outer_edge: CompressedEdge,
    ) -> Self {
        AltTreeNode {
            inner_region: Some(inner_region),
            outer_region: Some(outer_region),
            inner_to_outer_edge,
            parent: None,
            children: Vec::new(),
            visited: false,
        }
    }

    /// Add a child edge. Sets the child's parent pointer back to `self_idx`.
    pub fn add_child(
        &mut self,
        self_idx: AltTreeIdx,
        child: AltTreeEdge,
        arena: &mut Arena<AltTreeNode>,
    ) {
        let child_idx = child.alt_tree_node;
        let reversed_edge = child.edge.reversed();
        self.children.push(child);
        arena[child_idx.0].parent = Some(AltTreeEdge::new(self_idx, reversed_edge));
    }

    /// Tree rotation: make this node the root.
    /// Recursively rotates parent first, then re-parents.
    pub fn become_root(self_idx: AltTreeIdx, arena: &mut Arena<AltTreeNode>) {
        let parent_edge = arena[self_idx.0].parent.clone();
        if parent_edge.is_none() {
            return; // already root
        }
        let parent_edge = parent_edge.unwrap();
        let old_parent_idx = parent_edge.alt_tree_node;

        // Recurse: make old parent the root first
        AltTreeNode::become_root(old_parent_idx, arena);

        // Now old_parent is root. Rotate.
        // old_parent.inner_region = self.inner_region
        let self_inner = arena[self_idx.0].inner_region;
        let self_inner_to_outer = arena[self_idx.0].inner_to_outer_edge;
        let parent_edge_val = arena[self_idx.0].parent.as_ref().unwrap().edge;

        arena[old_parent_idx.0].inner_region = self_inner;
        arena[old_parent_idx.0].inner_to_outer_edge = parent_edge_val;

        arena[self_idx.0].inner_region = None;

        // Remove self from old_parent's children
        unstable_erase_by_node(&mut arena[old_parent_idx.0].children, self_idx);

        // Clear self's parent
        arena[self_idx.0].parent = None;

        // Add old_parent as child of self
        let edge_to_old_parent = self_inner_to_outer.reversed();
        let child_edge = AltTreeEdge::new(old_parent_idx, edge_to_old_parent);
        let reversed = edge_to_old_parent.reversed();
        arena[self_idx.0].children.push(child_edge);
        arena[old_parent_idx.0].parent = Some(AltTreeEdge::new(self_idx, reversed));

        arena[self_idx.0].inner_to_outer_edge = CompressedEdge::empty();
    }

    /// Find the most recent common ancestor of two nodes in the same tree.
    /// Returns `None` if they are in different trees.
    pub fn most_recent_common_ancestor(
        node_a: AltTreeIdx,
        node_b: AltTreeIdx,
        arena: &mut Arena<AltTreeNode>,
    ) -> Option<AltTreeIdx> {
        arena[node_a.0].visited = true;
        arena[node_b.0].visited = true;

        let mut a_cur = node_a;
        let mut b_cur = node_b;
        let common_ancestor;

        loop {
            let a_parent = arena[a_cur.0]
                .parent
                .as_ref()
                .map(|e| e.alt_tree_node);
            let b_parent = arena[b_cur.0]
                .parent
                .as_ref()
                .map(|e| e.alt_tree_node);

            if a_parent.is_some() || b_parent.is_some() {
                if let Some(ap) = a_parent {
                    a_cur = ap;
                    if arena[a_cur.0].visited {
                        common_ancestor = a_cur;
                        break;
                    }
                    arena[a_cur.0].visited = true;
                }
                if let Some(bp) = b_parent {
                    b_cur = bp;
                    if arena[b_cur.0].visited {
                        common_ancestor = b_cur;
                        break;
                    }
                    arena[b_cur.0].visited = true;
                }
            } else {
                // Both reached root without meeting — different trees
                // Clean up visited flags
                Self::clear_visited_upward(node_a, arena);
                Self::clear_visited_upward(node_b, arena);
                return None;
            }
        }

        // Clean up visited flags for common ancestor and its ancestors
        arena[common_ancestor.0].visited = false;
        let mut cleanup = arena[common_ancestor.0]
            .parent
            .as_ref()
            .map(|e| e.alt_tree_node);
        while let Some(idx) = cleanup {
            if !arena[idx.0].visited {
                break;
            }
            arena[idx.0].visited = false;
            cleanup = arena[idx.0].parent.as_ref().map(|e| e.alt_tree_node);
        }

        Some(common_ancestor)
    }

    fn clear_visited_upward(start: AltTreeIdx, arena: &mut Arena<AltTreeNode>) {
        let mut cur = Some(start);
        while let Some(idx) = cur {
            if !arena[idx.0].visited {
                break;
            }
            arena[idx.0].visited = false;
            cur = arena[idx.0].parent.as_ref().map(|e| e.alt_tree_node);
        }
    }

    /// Prune the upward path from `self_idx` stopping before `prune_parent`.
    /// Returns orphan edges (children of pruned nodes) and the pruned path region edges.
    /// If `back` is true, edges are oriented inner->outer->parent; otherwise outer->inner->parent.
    pub fn prune_upward_path_stopping_before(
        self_idx: AltTreeIdx,
        arena: &mut Arena<AltTreeNode>,
        prune_parent: AltTreeIdx,
        back: bool,
    ) -> AltTreePruneResult {
        let mut orphan_edges: Vec<AltTreeEdge> = Vec::new();
        let mut pruned_path_region_edges: Vec<RegionEdge> = Vec::new();
        let mut regions_to_clear: Vec<RegionIdx> = Vec::new();
        let mut current = self_idx;

        while current != prune_parent {
            // Move children to orphans
            let children = std::mem::take(&mut arena[current.0].children);
            orphan_edges.extend(children);

            let inner = arena[current.0].inner_region.unwrap();
            let outer = arena[current.0].outer_region.unwrap();
            let i2o = arena[current.0].inner_to_outer_edge;
            let parent_edge = arena[current.0].parent.as_ref().unwrap().clone();
            let parent_idx = parent_edge.alt_tree_node;
            let parent_outer = arena[parent_idx.0].outer_region.unwrap();

            if back {
                pruned_path_region_edges.push(RegionEdge {
                    region: inner,
                    edge: i2o,
                });
                pruned_path_region_edges.push(RegionEdge {
                    region: parent_outer,
                    edge: parent_edge.edge.reversed(),
                });
            } else {
                pruned_path_region_edges.push(RegionEdge {
                    region: outer,
                    edge: i2o.reversed(),
                });
                pruned_path_region_edges.push(RegionEdge {
                    region: inner,
                    edge: parent_edge.edge,
                });
            }

            // Remove current from parent's children
            unstable_erase_by_node(&mut arena[parent_idx.0].children, current);

            // Track regions whose alt_tree_node must be cleared
            // (mirrors C++: current->inner_region->alt_tree_node = nullptr)
            regions_to_clear.push(inner);
            regions_to_clear.push(outer);

            let to_free = current;
            current = parent_idx;
            arena.free(to_free.0);
        }

        AltTreePruneResult {
            orphan_edges,
            pruned_path_region_edges,
            regions_to_clear,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Remove the first element matching the given AltTreeIdx (unstable order).
pub fn unstable_erase_by_node(vec: &mut Vec<AltTreeEdge>, target: AltTreeIdx) -> bool {
    if let Some(pos) = vec.iter().position(|e| e.alt_tree_node == target) {
        let last = vec.len() - 1;
        if pos != last {
            vec.swap(pos, last);
        }
        vec.pop();
        true
    } else {
        false
    }
}
