use std::num::Wrapping;

use crate::interop::*;
use crate::matcher::alt_tree::AltTreeNode;
use crate::types::*;
use crate::util::arena::Arena;
use crate::util::radix_heap::{HasTime, RadixHeapQueue};
use crate::util::varying::VaryingCT;

use super::fill_region::GraphFillRegion;
use super::graph::{MatchingGraph, BOUNDARY_NODE};

pub struct GraphFlooder {
    pub graph: MatchingGraph,
    pub region_arena: Arena<GraphFillRegion>,
    pub node_arena: Arena<AltTreeNode>,
    pub queue: RadixHeapQueue<FloodCheckEvent>,
    pub match_edges: Vec<CompressedEdge>,
    pub node_cleanup_buffer: Vec<NodeIdx>,
    touched_nodes: Vec<NodeIdx>,
    node_was_touched: Vec<bool>,
}

impl GraphFlooder {
    pub fn new(graph: MatchingGraph) -> Self {
        GraphFlooder {
            node_was_touched: vec![false; graph.nodes.len()],
            graph,
            region_arena: Arena::new(),
            node_arena: Arena::new(),
            queue: RadixHeapQueue::new(),
            match_edges: Vec::new(),
            node_cleanup_buffer: Vec::new(),
            touched_nodes: Vec::new(),
        }
    }

    // ---------------------------------------------------------------
    // Detection event creation
    // ---------------------------------------------------------------

    pub fn create_detection_event(&mut self, node_idx: NodeIdx) -> RegionIdx {
        self.mark_node_touched(node_idx);
        let region_idx =
            RegionIdx(self.region_arena.alloc_with_reset(GraphFillRegion::reset_for_reuse));
        {
            let region = self.region_arena.get_mut(region_idx.0);
            region.radius =
                VaryingCT::growing_varying_with_zero_distance_at_time(self.queue.cur_time);
            region.blossom_parent_top = Some(region_idx);
            region.shell_area.push(node_idx);
        }

        let node = &mut self.graph.nodes[node_idx.0 as usize];
        node.region_that_arrived = Some(region_idx);
        node.region_that_arrived_top = Some(region_idx);
        node.reached_from_source = Some(node_idx);
        node.observables_crossed_from_source = 0;
        node.radius_of_arrival = 0;
        node.wrapped_radius_cached = 0;

        self.reschedule_events_at_detector_node(node_idx);
        region_idx
    }

    // ---------------------------------------------------------------
    // Main loop
    // ---------------------------------------------------------------

    pub fn run_until_next_mwpm_notification(&mut self) -> MwpmEvent {
        loop {
            let event = self.dequeue_valid();
            if event.is_no_event() {
                return MwpmEvent::NoEvent;
            }
            let notification = self.process_tentative_event(event);
            if !notification.is_no_event() {
                return notification;
            }
        }
    }

    /// Dequeue events, skipping stale ones, until we get a valid one or the queue is empty.
    fn dequeue_valid(&mut self) -> FloodCheckEvent {
        loop {
            let ev = self.queue.dequeue();
            if ev.is_no_event() {
                return ev;
            }
            if self.dequeue_decision(&ev) {
                return ev;
            }
        }
    }

    /// Check whether a dequeued event is still valid (not stale).
    fn dequeue_decision(&mut self, ev: &FloodCheckEvent) -> bool {
        match ev {
            FloodCheckEvent::LookAtNode { node, .. } => {
                let node_idx = *node;
                let tracker = &mut self.graph.nodes[node_idx.0 as usize].node_event_tracker;
                tracker.dequeue_decision(ev, &mut self.queue, |time| {
                    FloodCheckEvent::LookAtNode { node: node_idx, time }
                })
            }
            FloodCheckEvent::LookAtShrinkingRegion { region, .. } => {
                let region_idx = *region;
                let tracker = &mut self.region_arena[region_idx.0].shrink_event_tracker;
                tracker.dequeue_decision(ev, &mut self.queue, |time| {
                    FloodCheckEvent::LookAtShrinkingRegion { region: region_idx, time }
                })
            }
            FloodCheckEvent::NoEvent => true,
            _ => false,
        }
    }

    fn process_tentative_event(&mut self, event: FloodCheckEvent) -> MwpmEvent {
        match event {
            FloodCheckEvent::LookAtNode { node, .. } => self.do_look_at_node_event(node),
            FloodCheckEvent::LookAtShrinkingRegion { region, .. } => {
                self.do_region_shrinking(region)
            }
            _ => MwpmEvent::NoEvent,
        }
    }

    // ---------------------------------------------------------------
    // Core node event processing (mirrors PyMatching do_look_at_node_event)
    // ---------------------------------------------------------------

    fn do_look_at_node_event(&mut self, node_idx: NodeIdx) -> MwpmEvent {
        let (best_neighbor, best_time) = self.find_next_event_at_node(node_idx);

        if best_time <= self.queue.cur_time {
            // Event is happening NOW. Reschedule immediately so we revisit for other edges.
            let event = FloodCheckEvent::LookAtNode {
                node: node_idx,
                time: Wrapping(self.queue.cur_time as u32),
            };
            self.graph.nodes[node_idx.0 as usize]
                .node_event_tracker
                .set_desired_event(event, &mut self.queue);

            let neighbor_node_idx = self.graph.nodes[node_idx.0 as usize].neighbors[best_neighbor];

            if neighbor_node_idx == BOUNDARY_NODE {
                return self.do_region_hit_boundary(node_idx, best_neighbor);
            }
            return self.do_neighbor_interaction(node_idx, best_neighbor, neighbor_node_idx);
        } else if best_neighbor != NO_NEIGHBOR {
            // Future event — schedule it.
            let event = FloodCheckEvent::LookAtNode {
                node: node_idx,
                time: Wrapping(best_time as u32),
            };
            self.graph.nodes[node_idx.0 as usize]
                .node_event_tracker
                .set_desired_event(event, &mut self.queue);
        }

        MwpmEvent::NoEvent
    }

    // ---------------------------------------------------------------
    // Neighbor interaction (grow or collide)
    // ---------------------------------------------------------------

    fn do_neighbor_interaction(
        &mut self,
        src_idx: NodeIdx,
        src_to_dst_index: usize,
        dst_idx: NodeIdx,
    ) -> MwpmEvent {
        let src_has_region = self.graph.nodes[src_idx.0 as usize].region_that_arrived.is_some();
        let dst_has_region = self.graph.nodes[dst_idx.0 as usize].region_that_arrived.is_some();

        if src_has_region && !dst_has_region {
            // Grow into empty neighbor
            self.do_region_arriving_at_empty_node(dst_idx, src_idx, src_to_dst_index);
            return MwpmEvent::NoEvent;
        } else if dst_has_region && !src_has_region {
            // Reverse: dst grows into empty src
            let dst_to_src_index = self.index_of_neighbor(dst_idx, src_idx);
            self.do_region_arriving_at_empty_node(src_idx, dst_idx, dst_to_src_index);
            return MwpmEvent::NoEvent;
        }

        // Two regions colliding
        let src = &self.graph.nodes[src_idx.0 as usize];
        let dst = &self.graph.nodes[dst_idx.0 as usize];
        let obs = src.neighbor_observables[src_to_dst_index];
        let edge = CompressedEdge {
            loc_from: src.reached_from_source,
            loc_to: dst.reached_from_source,
            obs_mask: src.observables_crossed_from_source
                ^ dst.observables_crossed_from_source
                ^ obs,
        };
        MwpmEvent::RegionHitRegion {
            region1: src.region_that_arrived_top.unwrap(),
            region2: dst.region_that_arrived_top.unwrap(),
            edge,
        }
    }

    fn do_region_hit_boundary(&self, node_idx: NodeIdx, boundary_neighbor_idx: usize) -> MwpmEvent {
        let node = &self.graph.nodes[node_idx.0 as usize];
        let edge = CompressedEdge {
            loc_from: node.reached_from_source,
            loc_to: None,
            obs_mask: node.observables_crossed_from_source
                ^ node.neighbor_observables[boundary_neighbor_idx],
        };
        MwpmEvent::RegionHitBoundary {
            region: node.region_that_arrived_top.unwrap(),
            edge,
        }
    }

    // ---------------------------------------------------------------
    // Region growth into an empty node
    // ---------------------------------------------------------------

    fn do_region_arriving_at_empty_node(
        &mut self,
        empty_node_idx: NodeIdx,
        from_node_idx: NodeIdx,
        from_to_empty_index: usize,
    ) {
        self.mark_node_touched(empty_node_idx);
        // Read from the source node
        let from_node = &self.graph.nodes[from_node_idx.0 as usize];
        let obs = from_node.neighbor_observables[from_to_empty_index];
        let obs_crossed = from_node.observables_crossed_from_source ^ obs;
        let source = from_node.reached_from_source;
        let region_top = from_node
            .region_that_arrived_top
            .expect("growing into an empty node requires a top region");
        let arriving_top = self.region_arena[region_top.0]
            .blossom_parent_top
            .unwrap_or(region_top);

        // Compute radius_of_arrival from the top region's current radius
        let radius_of_arrival = self.region_arena[region_top.0]
            .radius
            .get_distance_at_time(self.queue.cur_time);

        // Write to the empty node
        let empty_node = &mut self.graph.nodes[empty_node_idx.0 as usize];
        empty_node.observables_crossed_from_source = obs_crossed;
        empty_node.reached_from_source = source;
        empty_node.radius_of_arrival = radius_of_arrival;
        empty_node.region_that_arrived = Some(region_top);
        empty_node.region_that_arrived_top = Some(arriving_top);
        empty_node.wrapped_radius_cached =
            empty_node.compute_wrapped_radius(self.region_arena.items());

        // Add to region's shell area
        self.region_arena
            .get_mut(region_top.0)
            .shell_area
            .push(empty_node_idx);

        self.reschedule_events_at_detector_node(empty_node_idx);
    }

    // ---------------------------------------------------------------
    // Find next event at a node
    // ---------------------------------------------------------------

    #[inline]
    fn node_local_radius_parts(
        node: &super::detector_node::DetectorNode,
        regions: &[GraphFillRegion],
    ) -> (CumulativeTime, bool, bool) {
        match node.region_that_arrived_top {
            None => (0, false, false),
            Some(top_idx) => {
                let top_radius = regions[top_idx.0 as usize].radius;
                (
                    top_radius.y_intercept() + node.wrapped_radius_cached as i64,
                    top_radius.is_growing(),
                    top_radius.is_shrinking(),
                )
            }
        }
    }

    fn find_next_event_at_node(&self, node_idx: NodeIdx) -> (usize, CumulativeTime) {
        let regions = self.region_arena.items();
        let node = &self.graph.nodes[node_idx.0 as usize];
        let (rad1_y, rad1_growing, _rad1_shrinking) = Self::node_local_radius_parts(node, regions);

        if rad1_growing {
            self.find_next_event_growing(node, regions, rad1_y)
        } else {
            self.find_next_event_not_growing(node, regions, rad1_y)
        }
    }

    /// When the node's top region is growing: check boundary, unoccupied, and other-region neighbors.
    fn find_next_event_growing(
        &self,
        node: &super::detector_node::DetectorNode,
        regions: &[GraphFillRegion],
        rad1_y: CumulativeTime,
    ) -> (usize, CumulativeTime) {
        let mut best_time = i64::MAX;
        let mut best_neighbor = NO_NEIGHBOR;

        for i in 0..node.neighbors.len() {
            let neighbor_idx = node.neighbors[i];
            let weight = node.neighbor_weights[i] as CumulativeTime;

            if neighbor_idx == BOUNDARY_NODE {
                let collision_time = weight - rad1_y;
                if collision_time < best_time {
                    best_time = collision_time;
                    best_neighbor = i;
                }
                continue;
            }

            let neighbor = &self.graph.nodes[neighbor_idx.0 as usize];
            if node.has_same_owner_as(neighbor) {
                continue;
            }

            if neighbor.region_that_arrived_top.is_none() {
                let collision_time = weight - rad1_y;
                if collision_time < best_time {
                    best_time = collision_time;
                    best_neighbor = i;
                }
                continue;
            }

            let (rad2_y, rad2_growing, rad2_shrinking) =
                Self::node_local_radius_parts(neighbor, regions);
            if rad2_shrinking {
                continue;
            }

            let mut collision_time = weight - rad1_y - rad2_y;
            if rad2_growing {
                collision_time >>= 1; // Both growing: combined slope = 2
            }
            if collision_time < best_time {
                best_time = collision_time;
                best_neighbor = i;
            }
        }

        (best_neighbor, best_time)
    }

    /// When the node's top region is NOT growing (frozen/shrinking):
    /// only look for growing neighbors colliding into this node.
    fn find_next_event_not_growing(
        &self,
        node: &super::detector_node::DetectorNode,
        regions: &[GraphFillRegion],
        rad1_y: CumulativeTime,
    ) -> (usize, CumulativeTime) {
        let mut best_time = i64::MAX;
        let mut best_neighbor = NO_NEIGHBOR;

        // Skip boundary neighbors (index 0 if it's boundary) since we're not growing
        let start = if !node.neighbors.is_empty() && node.neighbors[0] == BOUNDARY_NODE {
            1
        } else {
            0
        };

        for i in start..node.neighbors.len() {
            let neighbor_idx = node.neighbors[i];
            if neighbor_idx == BOUNDARY_NODE {
                continue;
            }
            let weight = node.neighbor_weights[i] as CumulativeTime;
            let neighbor = &self.graph.nodes[neighbor_idx.0 as usize];
            if neighbor.region_that_arrived_top.is_none() {
                continue;
            }
            let (rad2_y, rad2_growing, _rad2_shrinking) =
                Self::node_local_radius_parts(neighbor, regions);

            if rad2_growing {
                let collision_time = weight - rad1_y - rad2_y;
                if collision_time < best_time {
                    best_time = collision_time;
                    best_neighbor = i;
                }
            }
        }

        (best_neighbor, best_time)
    }

    // ---------------------------------------------------------------
    // Reschedule events at a detector node
    // ---------------------------------------------------------------

    pub fn reschedule_events_at_detector_node(&mut self, node_idx: NodeIdx) {
        let (best_neighbor, best_time) = self.find_next_event_at_node(node_idx);
        let node = &mut self.graph.nodes[node_idx.0 as usize];
        if best_neighbor == NO_NEIGHBOR {
            node.node_event_tracker.set_no_desired_event();
        } else {
            let event = FloodCheckEvent::LookAtNode {
                node: node_idx,
                time: Wrapping(best_time.max(self.queue.cur_time) as u32),
            };
            node.node_event_tracker
                .set_desired_event(event, &mut self.queue);
        }
    }

    // ---------------------------------------------------------------
    // Region state transitions
    // ---------------------------------------------------------------

    fn reschedule_total_area_nodes(&mut self, region_idx: RegionIdx) {
        let shell_len = self.region_arena[region_idx.0].shell_area.len();
        for i in 0..shell_len {
            let node_idx = self.region_arena[region_idx.0].shell_area[i];
            self.reschedule_events_at_detector_node(node_idx);
        }

        let child_len = self.region_arena[region_idx.0].blossom_children.len();
        for i in 0..child_len {
            let child_region = self.region_arena[region_idx.0].blossom_children[i].region;
            self.reschedule_total_area_nodes(child_region);
        }
    }

    fn clear_total_area_node_events(&mut self, region_idx: RegionIdx) {
        let shell_len = self.region_arena[region_idx.0].shell_area.len();
        for i in 0..shell_len {
            let node_idx = self.region_arena[region_idx.0].shell_area[i];
            self.graph.nodes[node_idx.0 as usize]
                .node_event_tracker
                .set_no_desired_event();
        }

        let child_len = self.region_arena[region_idx.0].blossom_children.len();
        for i in 0..child_len {
            let child_region = self.region_arena[region_idx.0].blossom_children[i].region;
            self.clear_total_area_node_events(child_region);
        }
    }

    pub fn set_region_growing(&mut self, region_idx: RegionIdx) {
        {
            let region = self.region_arena.get_mut(region_idx.0);
            region.radius = region.radius.then_growing_at_time(self.queue.cur_time);
            region.shrink_event_tracker.set_no_desired_event();
        }
        self.reschedule_total_area_nodes(region_idx);
    }

    pub fn set_region_frozen(&mut self, region_idx: RegionIdx) {
        let was_shrinking = {
            let region = self.region_arena.get_mut(region_idx.0);
            let was_shrinking = region.radius.is_shrinking();
            region.radius = region.radius.then_frozen_at_time(self.queue.cur_time);
            region.shrink_event_tracker.set_no_desired_event();
            was_shrinking
        };
        if was_shrinking {
            self.reschedule_total_area_nodes(region_idx);
        }
    }

    pub fn set_region_shrinking(&mut self, region_idx: RegionIdx) {
        {
            let region = self.region_arena.get_mut(region_idx.0);
            region.radius = region.radius.then_shrinking_at_time(self.queue.cur_time);
        }
        // Schedule tentative shrink event
        self.schedule_tentative_shrink_event(region_idx);
        // No node events while shrinking
        self.clear_total_area_node_events(region_idx);
    }

    fn schedule_tentative_shrink_event(&mut self, region_idx: RegionIdx) {
        let region = &self.region_arena[region_idx.0];
        let t = if region.shell_area.is_empty() {
            region.radius.time_of_x_intercept()
        } else {
            let last_node_idx = *region.shell_area.last().unwrap();
            let last_node = &self.graph.nodes[last_node_idx.0 as usize];
            last_node
                .local_radius(self.region_arena.items())
                .time_of_x_intercept()
        }
        .max(self.queue.cur_time);
        let event = FloodCheckEvent::LookAtShrinkingRegion {
            region: region_idx,
            time: Wrapping(t as u32),
        };
        self.region_arena
            .get_mut(region_idx.0)
            .shrink_event_tracker
            .set_desired_event(event, &mut self.queue);
    }

    // ---------------------------------------------------------------
    // Region shrinking
    // ---------------------------------------------------------------

    fn do_region_shrinking(&mut self, region_idx: RegionIdx) -> MwpmEvent {
        let region = &self.region_arena[region_idx.0];
        if region.shell_area.is_empty() {
            // Blossom shattering — return event for matcher
            return self.do_blossom_shattering(region_idx);
        }

        if region.shell_area.len() == 1 && region.blossom_children.is_empty() {
            // Degenerate implosion: inner region with single node and no blossom
            // children implodes, generating a RegionHitRegion between the parent's
            // outer region and this node's outer region.
            return self.do_degenerate_implosion(region_idx);
        }

        // Remove the last node from the shell
        let leaving_node_idx = {
            let region = self.region_arena.get_mut(region_idx.0);
            region.shell_area.pop().unwrap()
        };

        let leaving = &mut self.graph.nodes[leaving_node_idx.0 as usize];
        leaving.region_that_arrived = None;
        leaving.region_that_arrived_top = None;
        leaving.wrapped_radius_cached = 0;
        leaving.reached_from_source = None;
        leaving.radius_of_arrival = 0;
        leaving.observables_crossed_from_source = 0;

        self.reschedule_events_at_detector_node(leaving_node_idx);
        self.schedule_tentative_shrink_event(region_idx);

        MwpmEvent::NoEvent
    }

    fn do_blossom_shattering(&self, region_idx: RegionIdx) -> MwpmEvent {
        let region = &self.region_arena[region_idx.0];
        let Some(alt_tree_idx) = region.alt_tree_node else {
            return MwpmEvent::NoEvent;
        };
        let alt_tree_node = &self.node_arena[alt_tree_idx.0];

        let in_parent = alt_tree_node.parent.as_ref().and_then(|parent| {
            parent.edge.loc_from.and_then(|node_idx| {
                self.graph.nodes[node_idx.0 as usize]
                    .heir_region_on_shatter(self.region_arena.items())
            })
        });

        let in_child = alt_tree_node.inner_to_outer_edge.loc_from.and_then(|node_idx| {
            self.graph.nodes[node_idx.0 as usize]
                .heir_region_on_shatter(self.region_arena.items())
        });

        match (in_parent, in_child) {
            (Some(ip), Some(ic)) => MwpmEvent::BlossomShatter {
                blossom: region_idx,
                in_parent: ip,
                in_child: ic,
            },
            _ => MwpmEvent::NoEvent,
        }
    }

    /// Handle the case where an inner region in an alternating tree shrinks
    /// down to a single node with no blossom children. This generates a
    /// RegionHitRegion event between the parent's outer region and this
    /// node's outer region, effectively collapsing the tree path.
    fn do_degenerate_implosion(&self, region_idx: RegionIdx) -> MwpmEvent {
        let region = &self.region_arena[region_idx.0];
        let alt_node = region.alt_tree_node.unwrap();
        let parent_alt = self.node_arena[alt_node.0]
            .parent
            .as_ref()
            .unwrap()
            .alt_tree_node;
        let parent_outer = self.node_arena[parent_alt.0].outer_region.unwrap();
        let this_outer = self.node_arena[alt_node.0].outer_region.unwrap();
        let parent_edge = &self.node_arena[alt_node.0].parent.as_ref().unwrap().edge;
        let i2o_edge = self.node_arena[alt_node.0].inner_to_outer_edge;
        MwpmEvent::RegionHitRegion {
            region1: parent_outer,
            region2: this_outer,
            edge: CompressedEdge {
                loc_from: parent_edge.loc_to,
                loc_to: i2o_edge.loc_to,
                obs_mask: i2o_edge.obs_mask ^ parent_edge.obs_mask,
            },
        }
    }

    // ---------------------------------------------------------------
    // Reset
    // ---------------------------------------------------------------

    pub fn reset(&mut self) {
        for node_idx in self.touched_nodes.drain(..) {
            self.graph.nodes[node_idx.0 as usize].reset();
            self.node_was_touched[node_idx.0 as usize] = false;
        }
        self.region_arena
            .recycle_touched(GraphFillRegion::reset_for_reuse);
        self.node_arena
            .recycle_touched(AltTreeNode::reset_for_reuse);
        self.queue.reset();
        self.match_edges.clear();
        self.node_cleanup_buffer.clear();
    }

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    fn index_of_neighbor(&self, node_idx: NodeIdx, target: NodeIdx) -> usize {
        self.graph.nodes[node_idx.0 as usize]
            .neighbors
            .iter()
            .position(|n| *n == target)
            .expect("neighbor not found")
    }

    fn mark_node_touched(&mut self, node_idx: NodeIdx) {
        let touched = &mut self.node_was_touched[node_idx.0 as usize];
        if !*touched {
            *touched = true;
            self.touched_nodes.push(node_idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flooder::detector_node::DetectorNode;
    use crate::interop::{CompressedEdge, RegionEdge};
    use crate::test_alloc::{allocation_count, reset_allocation_count};

    #[test]
    fn reset_only_visits_touched_nodes() {
        let mut graph = MatchingGraph::new(10, 0);
        graph.add_edge(0, 1, 5, &[]);
        graph.add_edge(1, 2, 5, &[]);
        graph.add_boundary_edge(2, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        flooder.create_detection_event(NodeIdx(0));
        let event = flooder.run_until_next_mwpm_notification();
        assert!(matches!(event, MwpmEvent::RegionHitBoundary { .. }));

        DetectorNode::reset_reset_call_count();
        flooder.reset();

        assert_eq!(DetectorNode::reset_call_count(), 3);
    }

    #[test]
    fn create_detection_event_after_reset_reuses_region_storage() {
        let mut graph = MatchingGraph::new(1, 0);
        graph.add_boundary_edge(0, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        flooder.create_detection_event(NodeIdx(0));
        flooder.reset();

        reset_allocation_count();
        flooder.create_detection_event(NodeIdx(0));

        assert_eq!(allocation_count(), 0);
    }

    #[test]
    fn set_region_shrinking_clears_descendant_node_events_for_blossom() {
        let mut graph = MatchingGraph::new(2, 0);
        graph.add_edge(0, 1, 5, &[]);
        graph.add_boundary_edge(0, 9, &[]);
        graph.add_boundary_edge(1, 9, &[]);

        let mut flooder = GraphFlooder::new(graph);
        let left = flooder.create_detection_event(NodeIdx(0));
        let right = flooder.create_detection_event(NodeIdx(1));

        assert!(flooder.graph.nodes[0].node_event_tracker.has_desired_time);
        assert!(flooder.graph.nodes[1].node_event_tracker.has_desired_time);

        let blossom = RegionIdx(flooder.region_arena.alloc());
        flooder.region_arena[blossom.0].radius =
            VaryingCT::growing_varying_with_zero_distance_at_time(flooder.queue.cur_time);
        flooder.region_arena[blossom.0].blossom_parent_top = Some(blossom);
        flooder.region_arena[blossom.0].blossom_children = vec![
            RegionEdge {
                region: left,
                edge: CompressedEdge::empty(),
            },
            RegionEdge {
                region: right,
                edge: CompressedEdge::empty(),
            },
        ];

        flooder.region_arena[left.0].blossom_parent = Some(blossom);
        flooder.region_arena[left.0].blossom_parent_top = Some(blossom);
        flooder.region_arena[right.0].blossom_parent = Some(blossom);
        flooder.region_arena[right.0].blossom_parent_top = Some(blossom);
        flooder.graph.nodes[0].region_that_arrived_top = Some(blossom);
        flooder.graph.nodes[1].region_that_arrived_top = Some(blossom);

        flooder.set_region_shrinking(blossom);

        assert!(!flooder.graph.nodes[0].node_event_tracker.has_desired_time);
        assert!(!flooder.graph.nodes[1].node_event_tracker.has_desired_time);
    }

    #[test]
    fn schedule_tentative_shrink_event_clamps_past_time_to_cur_time() {
        let mut graph = MatchingGraph::new(1, 0);
        graph.add_boundary_edge(0, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        let region = flooder.create_detection_event(NodeIdx(0));

        flooder.queue.cur_time = 100;
        flooder.graph.nodes[0].wrapped_radius_cached = -150;

        flooder.set_region_shrinking(region);

        let tracker = &flooder.region_arena[region.0].shrink_event_tracker;
        assert!(tracker.has_desired_time);
        assert_eq!(tracker.desired_time, Wrapping(100));
    }

    #[test]
    fn reschedule_events_at_detector_node_clamps_past_time_to_cur_time() {
        let mut graph = MatchingGraph::new(2, 0);
        graph.add_edge(0, 1, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        let region = flooder.create_detection_event(NodeIdx(0));

        flooder.queue.cur_time = 100;
        flooder.region_arena[region.0].radius =
            VaryingCT::growing_varying_with_zero_distance_at_time(0);
        flooder.graph.nodes[0].wrapped_radius_cached = 150;

        flooder.reschedule_events_at_detector_node(NodeIdx(0));

        let tracker = &flooder.graph.nodes[0].node_event_tracker;
        assert!(tracker.has_desired_time);
        assert_eq!(tracker.desired_time, Wrapping(100));
    }

    #[test]
    fn find_next_event_growing_skips_local_radius_for_unoccupied_neighbor() {
        let mut graph = MatchingGraph::new(2, 0);
        graph.add_edge(0, 1, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        flooder.create_detection_event(NodeIdx(0));

        DetectorNode::reset_local_radius_call_count();
        let (_best_neighbor, _best_time) = flooder.find_next_event_at_node(NodeIdx(0));

        assert_eq!(DetectorNode::local_radius_call_count(), 0);
    }

    #[test]
    fn find_next_event_growing_skips_local_radius_for_occupied_neighbor() {
        let mut graph = MatchingGraph::new(2, 0);
        graph.add_edge(0, 1, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        flooder.create_detection_event(NodeIdx(0));
        flooder.create_detection_event(NodeIdx(1));

        DetectorNode::reset_local_radius_call_count();
        let (_best_neighbor, _best_time) = flooder.find_next_event_at_node(NodeIdx(0));

        assert_eq!(DetectorNode::local_radius_call_count(), 0);
    }

    #[test]
    fn find_next_event_not_growing_skips_local_radius_for_occupied_neighbor() {
        let mut graph = MatchingGraph::new(2, 0);
        graph.add_edge(0, 1, 5, &[]);

        let mut flooder = GraphFlooder::new(graph);
        let left = flooder.create_detection_event(NodeIdx(0));
        flooder.create_detection_event(NodeIdx(1));
        flooder.set_region_frozen(left);

        DetectorNode::reset_local_radius_call_count();
        let (_best_neighbor, _best_time) = flooder.find_next_event_at_node(NodeIdx(0));

        assert_eq!(DetectorNode::local_radius_call_count(), 0);
    }

}
