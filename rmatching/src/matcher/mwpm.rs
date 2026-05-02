use crate::flooder::graph_flooder::GraphFlooder;
use crate::interop::*;
use crate::types::*;

use super::alt_tree::{unstable_erase_by_node, AltTreeEdge, AltTreeNode};

// ---------------------------------------------------------------------------
// MatchingResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchingResult {
    pub obs_mask: ObsMask,
    pub weight: TotalWeight,
}

impl MatchingResult {
    pub fn new() -> Self {
        MatchingResult {
            obs_mask: 0,
            weight: 0,
        }
    }
}

impl std::ops::AddAssign for MatchingResult {
    fn add_assign(&mut self, rhs: Self) {
        self.obs_mask ^= rhs.obs_mask;
        self.weight += rhs.weight;
    }
}

// ---------------------------------------------------------------------------
// Mwpm
// ---------------------------------------------------------------------------

pub struct Mwpm {
    pub flooder: GraphFlooder,
    // SearchFlooder will be added in Task 7.
}

impl Mwpm {
    pub fn new(flooder: GraphFlooder) -> Self {
        Mwpm {
            flooder,
        }
    }

    // -------------------------------------------------------------------
    // Detection event creation
    // -------------------------------------------------------------------

    pub fn create_detection_event(&mut self, node_idx: NodeIdx) {
        let region_idx = self.flooder.create_detection_event(node_idx);
        let alt_idx = AltTreeIdx(
            self.flooder
                .node_arena
                .alloc_with_reset(AltTreeNode::reset_for_reuse),
        );
        self.flooder.node_arena[alt_idx.0] = AltTreeNode::new_root(region_idx);
        self.flooder.region_arena[region_idx.0].alt_tree_node = Some(alt_idx);
        self.flooder.set_region_growing(region_idx);
    }

    // -------------------------------------------------------------------
    // Event processing
    // -------------------------------------------------------------------

    pub fn process_event(&mut self, event: MwpmEvent) {
        match event {
            MwpmEvent::RegionHitRegion {
                region1,
                region2,
                edge,
            } => self.handle_region_hit_region(region1, region2, edge),
            MwpmEvent::RegionHitBoundary { region, edge } => {
                self.handle_tree_hitting_boundary(region, edge);
            }
            MwpmEvent::BlossomShatter {
                blossom,
                in_parent,
                in_child,
            } => {
                self.handle_blossom_shattering(blossom, in_parent, in_child);
            }
            MwpmEvent::NoEvent => {}
        }
    }

    // -------------------------------------------------------------------
    // Region hit region dispatch
    // -------------------------------------------------------------------

    fn handle_region_hit_region(
        &mut self,
        region1: RegionIdx,
        region2: RegionIdx,
        edge: CompressedEdge,
    ) {
        let alt_node_1 = self.flooder.region_arena[region1.0].alt_tree_node;
        let alt_node_2 = self.flooder.region_arena[region2.0].alt_tree_node;

        match (alt_node_1, alt_node_2) {
            (Some(an1), Some(an2)) => {
                let common = AltTreeNode::most_recent_common_ancestor(an1, an2, &mut self.flooder.node_arena);
                if let Some(ancestor) = common {
                    self.handle_tree_hitting_same_tree(region1, region2, edge, ancestor);
                } else {
                    self.handle_tree_hitting_other_tree(region1, region2, edge);
                }
            }
            (Some(_), None) => {
                // Region 2 is not in a tree — it's matched
                let r2_match = self.flooder.region_arena[region2.0].match_.clone();
                if let Some(m) = r2_match {
                    if m.region.is_some() {
                        self.handle_tree_hitting_match(region1, region2, edge);
                    } else {
                        self.handle_tree_hitting_boundary_match(region1, region2, edge);
                    }
                } else {
                    self.handle_tree_hitting_boundary_match(region1, region2, edge);
                }
            }
            (None, Some(_)) => {
                let r1_match = self.flooder.region_arena[region1.0].match_.clone();
                let rev_edge = edge.reversed();
                if let Some(m) = r1_match {
                    if m.region.is_some() {
                        self.handle_tree_hitting_match(region2, region1, rev_edge);
                    } else {
                        self.handle_tree_hitting_boundary_match(region2, region1, rev_edge);
                    }
                } else {
                    self.handle_tree_hitting_boundary_match(region2, region1, rev_edge);
                }
            }
            (None, None) => {
                // Neither in a tree — shouldn't happen in normal operation
            }
        }
    }

    // -------------------------------------------------------------------
    // Tree hitting boundary
    // -------------------------------------------------------------------

    fn handle_tree_hitting_boundary(&mut self, region: RegionIdx, edge: CompressedEdge) {
        let alt_node = self.flooder.region_arena[region.0]
            .alt_tree_node
            .unwrap();
        AltTreeNode::become_root(alt_node, &mut self.flooder.node_arena);
        self.shatter_descendants_into_matches_and_freeze(alt_node);

        // Match region to boundary and freeze
        self.flooder.region_arena[region.0].match_ = Some(Match {
            region: None,
            edge,
        });
        self.flooder.set_region_frozen(region);
    }

    // -------------------------------------------------------------------
    // Tree hitting boundary match
    // -------------------------------------------------------------------

    fn handle_tree_hitting_boundary_match(
        &mut self,
        unmatched_region: RegionIdx,
        matched_region: RegionIdx,
        edge: CompressedEdge,
    ) {
        let alt_node = self.flooder.region_arena[unmatched_region.0]
            .alt_tree_node
            .unwrap();

        // Match unmatched to matched
        self.flooder.region_arena[unmatched_region.0].match_ = Some(Match {
            region: Some(matched_region),
            edge,
        });
        self.flooder.region_arena[matched_region.0].match_ = Some(Match {
            region: Some(unmatched_region),
            edge: edge.reversed(),
        });
        self.flooder.set_region_frozen(unmatched_region);

        AltTreeNode::become_root(alt_node, &mut self.flooder.node_arena);
        self.shatter_descendants_into_matches_and_freeze(alt_node);
    }

    // -------------------------------------------------------------------
    // Tree hitting other tree
    // -------------------------------------------------------------------

    fn handle_tree_hitting_other_tree(
        &mut self,
        region1: RegionIdx,
        region2: RegionIdx,
        edge: CompressedEdge,
    ) {
        let alt_node_1 = self.flooder.region_arena[region1.0]
            .alt_tree_node
            .unwrap();
        let alt_node_2 = self.flooder.region_arena[region2.0]
            .alt_tree_node
            .unwrap();

        AltTreeNode::become_root(alt_node_1, &mut self.flooder.node_arena);
        AltTreeNode::become_root(alt_node_2, &mut self.flooder.node_arena);

        self.shatter_descendants_into_matches_and_freeze(alt_node_1);
        self.shatter_descendants_into_matches_and_freeze(alt_node_2);

        // Match the two colliding regions
        self.flooder.region_arena[region1.0].match_ = Some(Match {
            region: Some(region2),
            edge,
        });
        self.flooder.region_arena[region2.0].match_ = Some(Match {
            region: Some(region1),
            edge: edge.reversed(),
        });
        self.flooder.set_region_frozen(region1);
        self.flooder.set_region_frozen(region2);
    }

    // -------------------------------------------------------------------
    // Tree hitting a matched pair (absorb into tree)
    // -------------------------------------------------------------------

    fn handle_tree_hitting_match(
        &mut self,
        unmatched_region: RegionIdx,
        matched_region: RegionIdx,
        edge: CompressedEdge,
    ) {
        let alt_node = self.flooder.region_arena[unmatched_region.0]
            .alt_tree_node
            .unwrap();

        let m = self.flooder.region_arena[matched_region.0]
            .match_
            .clone()
            .unwrap();
        let other_match = m.region.unwrap();
        let match_edge = m.edge;

        let child = self.make_child(
            alt_node,
            matched_region,
            other_match,
            match_edge,
            edge,
        );

        // Clear the match on both sides
        self.flooder.region_arena[other_match.0].match_ = None;
        self.flooder.region_arena[matched_region.0].match_ = None;

        // Set inner shrinking, outer growing
        self.flooder.set_region_shrinking(matched_region);
        self.flooder.set_region_growing(other_match);

        let _ = child;
    }

    // -------------------------------------------------------------------
    // Tree hitting same tree (blossom formation)
    // -------------------------------------------------------------------

    fn handle_tree_hitting_same_tree(
        &mut self,
        region1: RegionIdx,
        region2: RegionIdx,
        edge: CompressedEdge,
        common_ancestor: AltTreeIdx,
    ) {
        let alt_node_1 = self.flooder.region_arena[region1.0]
            .alt_tree_node
            .unwrap();
        let alt_node_2 = self.flooder.region_arena[region2.0]
            .alt_tree_node
            .unwrap();

        let prune_result_1 = AltTreeNode::prune_upward_path_stopping_before(
            alt_node_1,
            &mut self.flooder.node_arena,
            common_ancestor,
            true,
        );
        let prune_result_2 = AltTreeNode::prune_upward_path_stopping_before(
            alt_node_2,
            &mut self.flooder.node_arena,
            common_ancestor,
            false,
        );

        // Clear alt_tree_node on all pruned regions
        // (mirrors C++: current->inner_region->alt_tree_node = nullptr)
        for r in &prune_result_1.regions_to_clear {
            self.flooder.region_arena[r.0].alt_tree_node = None;
        }
        for r in &prune_result_2.regions_to_clear {
            self.flooder.region_arena[r.0].alt_tree_node = None;
        }

        // Build blossom cycle: path2 + reversed(path1) + closing edge
        let mut blossom_cycle = prune_result_2.pruned_path_region_edges;
        let p1s = prune_result_1.pruned_path_region_edges.len();
        blossom_cycle.reserve(p1s + 1);
        for i in 0..p1s {
            blossom_cycle.push(prune_result_1.pruned_path_region_edges[p1s - i - 1].clone());
        }
        blossom_cycle.push(RegionEdge {
            region: region1,
            edge,
        });

        // Detach old outer_region from tree
        let old_outer = self.flooder.node_arena[common_ancestor.0].outer_region.unwrap();
        self.flooder.region_arena[old_outer.0].alt_tree_node = None;

        // Create blossom region in flooder
        let blossom_region = self.create_blossom(&blossom_cycle);

        // Update common ancestor
        self.flooder.node_arena[common_ancestor.0].outer_region = Some(blossom_region);
        self.flooder.region_arena[blossom_region.0].alt_tree_node = Some(common_ancestor);

        // Store anchor nodes for blossom shattering
        let inner_to_outer_loc = self.flooder.node_arena[common_ancestor.0].inner_to_outer_edge.loc_from;
        let parent_loc = self.flooder.node_arena[common_ancestor.0].parent
            .as_ref()
            .and_then(|p| p.edge.loc_from);
        self.flooder.region_arena[blossom_region.0].blossom_in_parent_loc = parent_loc;
        self.flooder.region_arena[blossom_region.0].blossom_in_child_loc = inner_to_outer_loc;

        // Re-parent orphans
        for c in prune_result_1.orphan_edges {
            let child_idx = c.alt_tree_node;
            let edge = c.edge;
            self.flooder.node_arena[common_ancestor.0]
                .children
                .push(AltTreeEdge::new(child_idx, edge));
            self.flooder.node_arena[child_idx.0].parent =
                Some(AltTreeEdge::new(common_ancestor, edge.reversed()));
        }
        for c in prune_result_2.orphan_edges {
            let child_idx = c.alt_tree_node;
            let edge = c.edge;
            self.flooder.node_arena[common_ancestor.0]
                .children
                .push(AltTreeEdge::new(child_idx, edge));
            self.flooder.node_arena[child_idx.0].parent =
                Some(AltTreeEdge::new(common_ancestor, edge.reversed()));
        }
    }

    // -------------------------------------------------------------------
    // Blossom shattering
    // -------------------------------------------------------------------

    fn handle_blossom_shattering(
        &mut self,
        blossom_region: RegionIdx,
        in_parent_region: RegionIdx,
        in_child_region: RegionIdx,
    ) {
        // Clear blossom parent on all children
        let blossom_children: Vec<RegionEdge> =
            std::mem::take(&mut self.flooder.region_arena[blossom_region.0].blossom_children);
        for child in &blossom_children {
            self.clear_region_blossom_parent(child.region, true);
        }

        let blossom_alt_node = self.flooder.region_arena[blossom_region.0]
            .alt_tree_node
            .unwrap();
        let bsize = blossom_children.len();

        // Find indices of in_parent and in_child
        let mut parent_idx = 0;
        let mut child_idx = 0;
        for i in 0..bsize {
            if blossom_children[i].region == in_parent_region {
                parent_idx = i;
            }
            if blossom_children[i].region == in_child_region {
                child_idx = i;
            }
        }

        let gap = (child_idx + bsize - parent_idx) % bsize;

        // Get parent of blossom alt node and remove blossom from parent's children
        let blossom_parent_alt = self.flooder.node_arena[blossom_alt_node.0]
            .parent
            .as_ref()
            .unwrap()
            .alt_tree_node;
        unstable_erase_by_node(
            &mut self.flooder.node_arena[blossom_parent_alt.0].children,
            blossom_alt_node,
        );
        let child_edge_val = self.flooder.node_arena[blossom_alt_node.0]
            .parent
            .as_ref()
            .unwrap()
            .edge
            .reversed();

        let mut current_alt_node = blossom_parent_alt;
        let mut child_edge = child_edge_val;

        let evens_start;
        let evens_end;

        if gap % 2 == 0 {
            evens_start = child_idx + 1;
            evens_end = child_idx + bsize - gap;

            // Insert odd-length path from in_parent to in_child
            let mut i = parent_idx;
            while i < parent_idx + gap {
                let k1 = i % bsize;
                let k2 = (i + 1) % bsize;
                current_alt_node = self.make_child(
                    current_alt_node,
                    blossom_children[k1].region,
                    blossom_children[k2].region,
                    blossom_children[k1].edge,
                    child_edge,
                );
                child_edge = blossom_children[k2].edge;
                let inner = self.flooder.node_arena[current_alt_node.0].inner_region.unwrap();
                let outer = self.flooder.node_arena[current_alt_node.0].outer_region.unwrap();
                self.flooder.set_region_shrinking(inner);
                self.flooder.set_region_growing(outer);
                i += 2;
            }
        } else {
            evens_start = parent_idx + 1;
            evens_end = parent_idx + gap;

            let mut i = 0;
            while i < bsize - gap {
                let k1 = (parent_idx + bsize - i) % bsize;
                let k2 = (parent_idx + bsize - i - 1) % bsize;
                let k3 = (parent_idx + bsize - i - 2) % bsize;
                current_alt_node = self.make_child(
                    current_alt_node,
                    blossom_children[k1].region,
                    blossom_children[k2].region,
                    blossom_children[k2].edge.reversed(),
                    child_edge,
                );
                child_edge = blossom_children[k3].edge.reversed();
                let inner = self.flooder.node_arena[current_alt_node.0].inner_region.unwrap();
                let outer = self.flooder.node_arena[current_alt_node.0].outer_region.unwrap();
                self.flooder.set_region_shrinking(inner);
                self.flooder.set_region_growing(outer);
                i += 2;
            }
        }

        // Match even-length path regions
        let mut j = evens_start;
        while j < evens_end {
            let k1 = j % bsize;
            let k2 = (j + 1) % bsize;
            let r1 = blossom_children[k1].region;
            let r2 = blossom_children[k2].region;
            let e = blossom_children[k1].edge;
            self.flooder.region_arena[r1.0].match_ = Some(Match {
                region: Some(r2),
                edge: e,
            });
            self.flooder.region_arena[r2.0].match_ = Some(Match {
                region: Some(r1),
                edge: e.reversed(),
            });
            // Reschedule events for frozen regions
            self.reschedule_region_nodes(r1);
            self.reschedule_region_nodes(r2);
            j += 2;
        }

        // Update blossom alt node: inner = in_child_region
        self.flooder.node_arena[blossom_alt_node.0].inner_region = Some(blossom_children[child_idx].region);
        let inner_region = blossom_children[child_idx].region;
        self.flooder.set_region_shrinking(inner_region);
        self.flooder.region_arena[inner_region.0].alt_tree_node = Some(blossom_alt_node);

        // Add blossom_alt_node as child of current_alt_node
        let blossom_child_edge = AltTreeEdge::new(blossom_alt_node, child_edge);
        let rev = child_edge.reversed();
        self.flooder.node_arena[current_alt_node.0]
            .children
            .push(blossom_child_edge);
        self.flooder.node_arena[blossom_alt_node.0].parent =
            Some(AltTreeEdge::new(current_alt_node, rev));

        // Free the blossom region
        self.flooder.region_arena.free(blossom_region.0);
    }

    // -------------------------------------------------------------------
    // Shatter descendants into matches and freeze
    // -------------------------------------------------------------------

    fn shatter_descendants_into_matches_and_freeze(&mut self, alt_node: AltTreeIdx) {
        // Recursively process children first
        let children: Vec<AltTreeEdge> =
            std::mem::take(&mut self.flooder.node_arena[alt_node.0].children);
        for child_edge in &children {
            self.shatter_descendants_into_matches_and_freeze(child_edge.alt_tree_node);
        }

        if let Some(inner) = self.flooder.node_arena[alt_node.0].inner_region {
            let outer = self.flooder.node_arena[alt_node.0].outer_region.unwrap();
            let i2o = self.flooder.node_arena[alt_node.0].inner_to_outer_edge;

            // Match inner to outer
            self.flooder.region_arena[inner.0].match_ = Some(Match {
                region: Some(outer),
                edge: i2o,
            });
            self.flooder.region_arena[outer.0].match_ = Some(Match {
                region: Some(inner),
                edge: i2o.reversed(),
            });
            self.flooder.set_region_frozen(inner);
            self.flooder.set_region_frozen(outer);
            self.flooder.region_arena[inner.0].alt_tree_node = None;
            self.flooder.region_arena[outer.0].alt_tree_node = None;
        }

        if let Some(outer) = self.flooder.node_arena[alt_node.0].outer_region {
            self.flooder.region_arena[outer.0].alt_tree_node = None;
        }

        self.flooder.node_arena.free(alt_node.0);
    }

    // -------------------------------------------------------------------
    // Make child helper
    // -------------------------------------------------------------------

    fn make_child(
        &mut self,
        parent: AltTreeIdx,
        child_inner: RegionIdx,
        child_outer: RegionIdx,
        child_inner_to_outer_edge: CompressedEdge,
        child_compressed_edge: CompressedEdge,
    ) -> AltTreeIdx {
        let child_idx = AltTreeIdx(
            self.flooder
                .node_arena
                .alloc_with_reset(AltTreeNode::reset_for_reuse),
        );
        self.flooder.node_arena[child_idx.0] =
            AltTreeNode::new_pair(child_inner, child_outer, child_inner_to_outer_edge);
        self.flooder.region_arena[child_inner.0].alt_tree_node = Some(child_idx);
        self.flooder.region_arena[child_outer.0].alt_tree_node = Some(child_idx);

        let edge = AltTreeEdge::new(child_idx, child_compressed_edge);
        let rev = child_compressed_edge.reversed();
        self.flooder.node_arena[parent.0].children.push(edge);
        self.flooder.node_arena[child_idx.0].parent = Some(AltTreeEdge::new(parent, rev));

        child_idx
    }

    fn wrap_region_into_blossom(&mut self, region: RegionIdx, new_blossom_parent_and_top: RegionIdx) {
        self.flooder.region_arena[region.0].blossom_parent = Some(new_blossom_parent_and_top);
        self.wrap_region_descendants_into_blossom(region, new_blossom_parent_and_top);
    }

    fn wrap_region_descendants_into_blossom(
        &mut self,
        region: RegionIdx,
        new_blossom_parent_and_top: RegionIdx,
    ) {
        self.flooder.region_arena[region.0].blossom_parent_top = Some(new_blossom_parent_and_top);

        let shell_len = self.flooder.region_arena[region.0].shell_area.len();
        for i in 0..shell_len {
            let node_idx = self.flooder.region_arena[region.0].shell_area[i];
            self.flooder.graph.nodes[node_idx.0 as usize].region_that_arrived_top =
                Some(new_blossom_parent_and_top);
            let wrapped_radius = self.flooder.graph.nodes[node_idx.0 as usize]
                .compute_wrapped_radius(self.flooder.region_arena.items());
            self.flooder.graph.nodes[node_idx.0 as usize].wrapped_radius_cached = wrapped_radius;
        }

        let child_len = self.flooder.region_arena[region.0].blossom_children.len();
        for i in 0..child_len {
            let child_region = self.flooder.region_arena[region.0].blossom_children[i].region;
            self.wrap_region_descendants_into_blossom(child_region, new_blossom_parent_and_top);
        }
    }

    fn clear_region_blossom_parent(
        &mut self,
        region: RegionIdx,
        recompute_wrapped_radius: bool,
    ) {
        self.flooder.region_arena[region.0].blossom_parent = None;
        self.clear_region_descendant_blossom_parent(region, region, recompute_wrapped_radius);
    }

    fn clear_region_descendant_blossom_parent(
        &mut self,
        region: RegionIdx,
        new_top: RegionIdx,
        recompute_wrapped_radius: bool,
    ) {
        self.flooder.region_arena[region.0].blossom_parent_top = Some(new_top);

        let shell_len = self.flooder.region_arena[region.0].shell_area.len();
        for i in 0..shell_len {
            let node_idx = self.flooder.region_arena[region.0].shell_area[i];
            self.flooder.graph.nodes[node_idx.0 as usize].region_that_arrived_top = Some(new_top);
            if recompute_wrapped_radius {
                let wrapped_radius = self.flooder.graph.nodes[node_idx.0 as usize]
                    .compute_wrapped_radius(self.flooder.region_arena.items());
                self.flooder.graph.nodes[node_idx.0 as usize].wrapped_radius_cached =
                    wrapped_radius;
            }
        }

        let child_len = self.flooder.region_arena[region.0].blossom_children.len();
        for i in 0..child_len {
            let child_region = self.flooder.region_arena[region.0].blossom_children[i].region;
            self.clear_region_descendant_blossom_parent(
                child_region,
                new_top,
                recompute_wrapped_radius,
            );
        }
    }

    // -------------------------------------------------------------------
    // Blossom creation (simplified — delegates to flooder)
    // -------------------------------------------------------------------

    fn create_blossom(&mut self, cycle: &[RegionEdge]) -> RegionIdx {
        let blossom_idx = RegionIdx(
            self.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        self.flooder.region_arena[blossom_idx.0].blossom_parent_top = Some(blossom_idx);

        // Set blossom children
        self.flooder.region_arena[blossom_idx.0].blossom_children = cycle.to_vec();

        // Freeze each child region, set blossom parent, clear shrink events
        // (mirrors C++ create_blossom: freeze + wrap_into_blossom + clear shrink_event_tracker)
        let cur_time = self.flooder.queue.cur_time;
        for child in cycle {
            {
                let r = &mut self.flooder.region_arena[child.region.0];
                r.radius = r.radius.then_frozen_at_time(cur_time);
                r.shrink_event_tracker.set_no_desired_event();
            }
            self.wrap_region_into_blossom(child.region, blossom_idx);
        }

        // The blossom region starts growing
        self.flooder.region_arena[blossom_idx.0].radius =
            crate::util::varying::VaryingCT::growing_varying_with_zero_distance_at_time(cur_time);

        self.update_blossom_area_and_reschedule(blossom_idx, blossom_idx);

        blossom_idx
    }

    fn update_blossom_area_and_reschedule(
        &mut self,
        region: RegionIdx,
        blossom_top: RegionIdx,
    ) {
        let shell_len = self.flooder.region_arena[region.0].shell_area.len();
        for i in 0..shell_len {
            let node_idx = self.flooder.region_arena[region.0].shell_area[i];
            self.flooder.graph.nodes[node_idx.0 as usize].region_that_arrived_top =
                Some(blossom_top);
            self.flooder.graph.nodes[node_idx.0 as usize].wrapped_radius_cached =
                self.flooder.graph.nodes[node_idx.0 as usize]
                    .compute_wrapped_radius(self.flooder.region_arena.items());
            self.flooder.reschedule_events_at_detector_node(node_idx);
        }

        let child_len = self.flooder.region_arena[region.0].blossom_children.len();
        for i in 0..child_len {
            let child_region = self.flooder.region_arena[region.0].blossom_children[i].region;
            self.update_blossom_area_and_reschedule(child_region, blossom_top);
        }
    }

    // -------------------------------------------------------------------
    // Shatter blossom and extract matches
    // -------------------------------------------------------------------

    pub fn shatter_blossom_and_extract_matches(
        &mut self,
        region: RegionIdx,
    ) -> MatchingResult {
        let boundary_edge = self.flooder.region_arena[region.0]
            .match_
            .as_ref()
            .map(|m| m.edge)
            .unwrap_or_else(CompressedEdge::empty);
        let has_match_region = self.flooder.region_arena[region.0]
            .match_
            .as_ref()
            .and_then(|m| m.region)
            .is_some();
        let has_blossom_children =
            !self.flooder.region_arena[region.0].blossom_children.is_empty();

        if has_match_region {
            let match_region = self.flooder.region_arena[region.0]
                .match_
                .as_ref()
                .unwrap()
                .region
                .unwrap();
            let match_region_has_blossom =
                !self.flooder.region_arena[match_region.0].blossom_children.is_empty();

            if !has_blossom_children && !match_region_has_blossom {
                // Base case: neither has blossom children
                let edge = self.flooder.region_arena[region.0]
                    .match_
                    .as_ref()
                    .unwrap()
                    .edge;
                let w1 = self.flooder.region_arena[region.0].radius.y_intercept();
                let w2 = self.flooder.region_arena[match_region.0]
                    .radius
                    .y_intercept();
                self.flooder.region_arena.free(match_region.0);
                self.flooder.region_arena.free(region.0);
                return MatchingResult {
                    obs_mask: edge.obs_mask,
                    weight: w1 + w2,
                };
            }
        } else if !has_blossom_children {
            // PyMatching keeps a default-initialized match object even when the
            // region is only carrying an implicit boundary/empty match state.
            let w = self.flooder.region_arena[region.0].radius.y_intercept();
            self.flooder.region_arena.free(region.0);
            return MatchingResult {
                obs_mask: boundary_edge.obs_mask,
                weight: w,
            };
        }

        // Complex case: shatter sub-blossoms
        let mut res = MatchingResult::new();
        let mut region = region;

        if !self.flooder.region_arena[region.0].blossom_children.is_empty() {
            region = self.pair_and_shatter_subblossoms(region, &mut res);
        }

        let match_region = self.flooder.region_arena[region.0]
            .match_
            .as_ref()
            .and_then(|m| m.region);
        if let Some(mr) = match_region {
            if !self.flooder.region_arena[mr.0].blossom_children.is_empty() {
                self.pair_and_shatter_subblossoms(mr, &mut res);
            }
        }

        res += self.shatter_blossom_and_extract_matches(region);
        res
    }

    fn resolve_subblossom_for_match_edge(
        &self,
        region: RegionIdx,
        match_edge: CompressedEdge,
    ) -> (RegionIdx, CompressedEdge) {
        let endpoint_subblossom = |loc: Option<NodeIdx>| {
            loc.and_then(|node_idx| {
                self.flooder.graph.nodes[node_idx.0 as usize]
                    .heir_region_for_blossom(self.flooder.region_arena.items(), region)
            })
        };
        let describe = |loc: Option<NodeIdx>| {
            loc.map(|node_idx| {
                let node = &self.flooder.graph.nodes[node_idx.0 as usize];
                let mut chain = Vec::new();
                let mut cursor = node.region_that_arrived;
                while let Some(region_idx) = cursor {
                    chain.push(region_idx.0);
                    cursor = self.flooder.region_arena[region_idx.0].blossom_parent;
                }
                (
                    node_idx.0,
                    node.region_that_arrived.map(|r| r.0),
                    node.region_that_arrived_top.map(|r| r.0),
                    chain,
                )
            })
        };

        if let Some(region_idx) = endpoint_subblossom(match_edge.loc_from) {
            return (region_idx, match_edge);
        }
        if let Some(region_idx) = endpoint_subblossom(match_edge.loc_to) {
            return (region_idx, match_edge.reversed());
        }
        if let Some(region_idx) =
            endpoint_subblossom(self.flooder.region_arena[region.0].blossom_in_child_loc)
        {
            let mut edge = match_edge;
            edge.loc_from = self.flooder.region_arena[region.0].blossom_in_child_loc;
            return (region_idx, edge);
        }
        if let Some(region_idx) =
            endpoint_subblossom(self.flooder.region_arena[region.0].blossom_in_parent_loc)
        {
            let mut edge = match_edge;
            edge.loc_from = self.flooder.region_arena[region.0].blossom_in_parent_loc;
            return (region_idx, edge);
        }

        panic!(
            "match edge touches no descendant of blossom {}: children={:?} loc_from={:?} loc_to={:?} from={:?} to={:?} in_parent_loc={:?} in_parent={:?} in_child_loc={:?} in_child={:?}",
            region.0,
            self.flooder.region_arena[region.0]
                .blossom_children
                .iter()
                .map(|c| c.region.0)
                .collect::<Vec<_>>(),
            match_edge.loc_from.map(|n| n.0),
            match_edge.loc_to.map(|n| n.0),
            describe(match_edge.loc_from),
            describe(match_edge.loc_to),
            self.flooder.region_arena[region.0].blossom_in_parent_loc.map(|n| n.0),
            describe(self.flooder.region_arena[region.0].blossom_in_parent_loc),
            self.flooder.region_arena[region.0].blossom_in_child_loc.map(|n| n.0),
            describe(self.flooder.region_arena[region.0].blossom_in_child_loc),
        )
    }

    fn pair_and_shatter_subblossoms(
        &mut self,
        region: RegionIdx,
        res: &mut MatchingResult,
    ) -> RegionIdx {
        let match_edge = self.flooder.region_arena[region.0].match_.as_ref().unwrap().edge;
        let (subblossom, subblossom_edge) =
            self.resolve_subblossom_for_match_edge(region, match_edge);
        let children: Vec<RegionEdge> =
            std::mem::take(&mut self.flooder.region_arena[region.0].blossom_children);

        // 1. Clear blossom parent on all children so detector nodes point back
        //    to the direct child blossom that survives the shatter.
        for child in &children {
            self.clear_region_blossom_parent(child.region, false);
        }

        // 3. Transfer the blossom's match to subblossom
        let blossom_match = self.flooder.region_arena[region.0].match_.clone().unwrap();
        let mut subblossom_edge = subblossom_edge;
        let mut other_edge = subblossom_edge.reversed();
        if let Some(other) = blossom_match.region {
            if !self.flooder.region_arena[other.0].blossom_children.is_empty() {
                let (_, normalized_other_edge) =
                    self.resolve_subblossom_for_match_edge(other, other_edge);
                other_edge = normalized_other_edge;
                subblossom_edge = other_edge.reversed();
            }
        }
        self.flooder.region_arena[subblossom.0].match_ = Some(Match {
            region: blossom_match.region,
            edge: subblossom_edge,
        });
        if let Some(other) = blossom_match.region {
            self.flooder.region_arena[other.0].match_ = Some(Match {
                region: Some(subblossom),
                edge: other_edge,
            });
        }

        // 4. Accumulate blossom radius weight
        res.weight += self.flooder.region_arena[region.0].radius.y_intercept();

        // 5. Find subblossom index in children
        let index = children
            .iter()
            .position(|c| c.region == subblossom)
            .expect("subblossom must be in blossom_children");
        let num_children = children.len();

        // 6. Pair up remaining children starting after subblossom
        let mut i = 0;
        while i < num_children - 1 {
            let re1 = &children[(index + i + 1) % num_children];
            let re2 = &children[(index + i + 2) % num_children];
            let r1 = re1.region;
            let r2 = re2.region;
            let e = re1.edge;
            self.flooder.region_arena[r1.0].match_ = Some(Match { region: Some(r2), edge: e });
            self.flooder.region_arena[r2.0].match_ = Some(Match { region: Some(r1), edge: e.reversed() });
            let sub_res = self.shatter_blossom_and_extract_matches(r1);
            *res += sub_res;
            i += 2;
        }

        // 7. Free blossom region and return subblossom
        self.flooder.region_arena.free(region.0);
        subblossom
    }

    // -------------------------------------------------------------------
    // Shatter blossom and extract match edges (for decode_to_edges)
    // -------------------------------------------------------------------

    pub fn shatter_blossom_and_extract_match_edges(
        &mut self,
        region: RegionIdx,
        match_edges: &mut Vec<CompressedEdge>,
    ) {
        let boundary_edge = self.flooder.region_arena[region.0]
            .match_
            .as_ref()
            .map(|m| m.edge)
            .unwrap_or_else(CompressedEdge::empty);
        let has_match_region = self.flooder.region_arena[region.0]
            .match_
            .as_ref()
            .and_then(|m| m.region)
            .is_some();
        let has_blossom_children =
            !self.flooder.region_arena[region.0].blossom_children.is_empty();

        if has_match_region {
            let match_region = self.flooder.region_arena[region.0]
                .match_
                .as_ref()
                .unwrap()
                .region
                .unwrap();
            let match_region_has_blossom =
                !self.flooder.region_arena[match_region.0].blossom_children.is_empty();

            if !has_blossom_children && !match_region_has_blossom {
                let edge = self.flooder.region_arena[region.0]
                    .match_
                    .as_ref()
                    .unwrap()
                    .edge;
                match_edges.push(edge);
                self.flooder.region_arena.free(match_region.0);
                self.flooder.region_arena.free(region.0);
                return;
            }
        } else if !has_blossom_children {
            if boundary_edge.loc_from.is_some() || boundary_edge.loc_to.is_some() || boundary_edge.obs_mask != 0 {
                match_edges.push(boundary_edge);
            }
            self.flooder.region_arena.free(region.0);
            return;
        }

        // Complex case: shatter sub-blossoms
        let mut region = region;

        if !self.flooder.region_arena[region.0].blossom_children.is_empty() {
            region = self.pair_and_shatter_subblossoms_edges(region, match_edges);
        }

        let match_region = self.flooder.region_arena[region.0]
            .match_
            .as_ref()
            .and_then(|m| m.region);
        if let Some(mr) = match_region {
            if !self.flooder.region_arena[mr.0].blossom_children.is_empty() {
                self.pair_and_shatter_subblossoms_edges(mr, match_edges);
            }
        }

        self.shatter_blossom_and_extract_match_edges(region, match_edges);
    }

    fn pair_and_shatter_subblossoms_edges(
        &mut self,
        region: RegionIdx,
        match_edges: &mut Vec<CompressedEdge>,
    ) -> RegionIdx {
        let match_edge = self.flooder.region_arena[region.0].match_.as_ref().unwrap().edge;
        let (subblossom, subblossom_edge) =
            self.resolve_subblossom_for_match_edge(region, match_edge);
        let children: Vec<RegionEdge> =
            std::mem::take(&mut self.flooder.region_arena[region.0].blossom_children);

        for child in &children {
            self.clear_region_blossom_parent(child.region, false);
        }

        let blossom_match = self.flooder.region_arena[region.0].match_.clone().unwrap();
        let mut subblossom_edge = subblossom_edge;
        let mut other_edge = subblossom_edge.reversed();
        if let Some(other) = blossom_match.region {
            if !self.flooder.region_arena[other.0].blossom_children.is_empty() {
                let (_, normalized_other_edge) =
                    self.resolve_subblossom_for_match_edge(other, other_edge);
                other_edge = normalized_other_edge;
                subblossom_edge = other_edge.reversed();
            }
        }
        self.flooder.region_arena[subblossom.0].match_ = Some(Match {
            region: blossom_match.region,
            edge: subblossom_edge,
        });
        if let Some(other) = blossom_match.region {
            self.flooder.region_arena[other.0].match_ = Some(Match {
                region: Some(subblossom),
                edge: other_edge,
            });
        }

        let index = children.iter().position(|c| c.region == subblossom)
            .expect("subblossom must be in blossom_children");
        let num_children = children.len();

        let mut i = 0;
        while i < num_children - 1 {
            let re1 = &children[(index + i + 1) % num_children];
            let re2 = &children[(index + i + 2) % num_children];
            let r1 = re1.region;
            let r2 = re2.region;
            let e = re1.edge;
            self.flooder.region_arena[r1.0].match_ = Some(Match { region: Some(r2), edge: e });
            self.flooder.region_arena[r2.0].match_ = Some(Match { region: Some(r1), edge: e.reversed() });
            self.shatter_blossom_and_extract_match_edges(r1, match_edges);
            i += 2;
        }

        self.flooder.region_arena.free(region.0);
        subblossom
    }

    // -------------------------------------------------------------------
    // Reschedule helper
    // -------------------------------------------------------------------

    pub fn reschedule_events_at_detector_node(&mut self, node_idx: NodeIdx) {
        self.flooder.reschedule_events_at_detector_node(node_idx);
    }

    fn reschedule_region_nodes(&mut self, region: RegionIdx) {
        let shell_len = self.flooder.region_arena[region.0].shell_area.len();
        for i in 0..shell_len {
            let node_idx = self.flooder.region_arena[region.0].shell_area[i];
            self.flooder.reschedule_events_at_detector_node(node_idx);
        }
    }

    // -------------------------------------------------------------------
    // Reset
    // -------------------------------------------------------------------

    pub fn reset(&mut self) {
        self.flooder.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flooder::graph::MatchingGraph;
    use crate::test_alloc::{allocation_count, reset_allocation_count};

    #[test]
    fn reschedule_region_nodes_does_not_allocate() {
        let mut graph = MatchingGraph::new(1, 0);
        graph.add_boundary_edge(0, 5, &[]);

        let mut mwpm = Mwpm::new(GraphFlooder::new(graph));
        let region = mwpm.flooder.create_detection_event(NodeIdx(0));

        reset_allocation_count();
        mwpm.reschedule_region_nodes(region);

        assert_eq!(allocation_count(), 0);
    }

    #[test]
    fn pair_and_shatter_subblossoms_can_resolve_subblossom_from_loc_to() {
        let graph = MatchingGraph::new(2, 0);
        let mut mwpm = Mwpm::new(GraphFlooder::new(graph));

        let leaf = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let child_a = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let child_b = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let child_c = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let blossom = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let outside = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );

        mwpm.flooder.region_arena[leaf.0].shell_area.push(NodeIdx(0));
        mwpm.flooder.region_arena[leaf.0].blossom_parent = Some(child_a);
        mwpm.flooder.region_arena[leaf.0].blossom_parent_top = Some(blossom);

        mwpm.flooder.region_arena[child_a.0].blossom_parent = Some(blossom);
        mwpm.flooder.region_arena[child_a.0].blossom_parent_top = Some(blossom);
        mwpm.flooder.region_arena[child_a.0].blossom_children = vec![RegionEdge {
            region: leaf,
            edge: CompressedEdge::empty(),
        }];

        mwpm.flooder.region_arena[child_b.0].blossom_parent = Some(blossom);
        mwpm.flooder.region_arena[child_b.0].blossom_parent_top = Some(blossom);
        mwpm.flooder.region_arena[child_c.0].blossom_parent = Some(blossom);
        mwpm.flooder.region_arena[child_c.0].blossom_parent_top = Some(blossom);

        mwpm.flooder.region_arena[outside.0].shell_area.push(NodeIdx(1));
        mwpm.flooder.region_arena[outside.0].match_ = Some(Match {
            region: Some(blossom),
            edge: CompressedEdge {
                loc_from: Some(NodeIdx(0)),
                loc_to: Some(NodeIdx(1)),
                obs_mask: 0,
            },
        });

        mwpm.flooder.region_arena[blossom.0].blossom_children = vec![
            RegionEdge {
                region: child_a,
                edge: CompressedEdge::empty(),
            },
            RegionEdge {
                region: child_b,
                edge: CompressedEdge::empty(),
            },
            RegionEdge {
                region: child_c,
                edge: CompressedEdge::empty(),
            },
        ];
        mwpm.flooder.region_arena[blossom.0].match_ = Some(Match {
            region: Some(outside),
            edge: CompressedEdge {
                loc_from: Some(NodeIdx(1)),
                loc_to: Some(NodeIdx(0)),
                obs_mask: 0,
            },
        });

        mwpm.flooder.graph.nodes[0].region_that_arrived = Some(leaf);
        mwpm.flooder.graph.nodes[0].region_that_arrived_top = Some(blossom);
        mwpm.flooder.graph.nodes[1].region_that_arrived = Some(outside);
        mwpm.flooder.graph.nodes[1].region_that_arrived_top = Some(outside);

        let mut res = MatchingResult::new();
        let surviving = mwpm.pair_and_shatter_subblossoms(blossom, &mut res);

        assert_eq!(surviving, child_a);
        assert_eq!(
            mwpm.flooder.region_arena[child_a.0]
                .match_
                .as_ref()
                .and_then(|m| m.region),
            Some(outside)
        );
        assert_eq!(
            mwpm.flooder.region_arena[child_a.0]
                .match_
                .as_ref()
                .map(|m| m.edge.loc_from),
            Some(Some(NodeIdx(0)))
        );
        assert_eq!(
            mwpm.flooder.region_arena[outside.0]
                .match_
                .as_ref()
                .and_then(|m| m.region),
            Some(child_a)
        );
        assert_eq!(
            mwpm.flooder.region_arena[outside.0]
                .match_
                .as_ref()
                .map(|m| m.edge.loc_from),
            Some(Some(NodeIdx(1)))
        );
    }

    #[test]
    fn resolve_subblossom_for_match_edge_prefers_loc_from_when_both_endpoints_match() {
        let graph = MatchingGraph::new(2, 0);
        let mut mwpm = Mwpm::new(GraphFlooder::new(graph));

        let leaf_a = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let leaf_b = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let child_a = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let child_b = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let child_c = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );
        let blossom = RegionIdx(
            mwpm.flooder
                .region_arena
                .alloc_with_reset(crate::flooder::fill_region::GraphFillRegion::reset_for_reuse),
        );

        mwpm.flooder.region_arena[leaf_a.0].shell_area.push(NodeIdx(0));
        mwpm.flooder.region_arena[leaf_a.0].blossom_parent = Some(child_a);
        mwpm.flooder.region_arena[leaf_b.0].shell_area.push(NodeIdx(1));
        mwpm.flooder.region_arena[leaf_b.0].blossom_parent = Some(child_b);

        mwpm.flooder.region_arena[child_a.0].blossom_parent = Some(blossom);
        mwpm.flooder.region_arena[child_a.0].blossom_children = vec![RegionEdge {
            region: leaf_a,
            edge: CompressedEdge::empty(),
        }];
        mwpm.flooder.region_arena[child_b.0].blossom_parent = Some(blossom);
        mwpm.flooder.region_arena[child_b.0].blossom_children = vec![RegionEdge {
            region: leaf_b,
            edge: CompressedEdge::empty(),
        }];
        mwpm.flooder.region_arena[child_c.0].blossom_parent = Some(blossom);
        mwpm.flooder.region_arena[blossom.0].blossom_children = vec![
            RegionEdge {
                region: child_a,
                edge: CompressedEdge::empty(),
            },
            RegionEdge {
                region: child_b,
                edge: CompressedEdge::empty(),
            },
            RegionEdge {
                region: child_c,
                edge: CompressedEdge::empty(),
            },
        ];

        mwpm.flooder.graph.nodes[0].region_that_arrived = Some(leaf_a);
        mwpm.flooder.graph.nodes[1].region_that_arrived = Some(leaf_b);

        let (resolved, resolved_edge) = mwpm.resolve_subblossom_for_match_edge(
            blossom,
            CompressedEdge {
                loc_from: Some(NodeIdx(0)),
                loc_to: Some(NodeIdx(1)),
                obs_mask: 0,
            },
        );

        assert_eq!(resolved, child_a);
        assert_eq!(resolved_edge.loc_from, Some(NodeIdx(0)));
    }
}
