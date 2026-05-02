use std::num::Wrapping;

use crate::interop::CompressedEdge;
use crate::search::search_graph::SearchGraph;
use crate::types::*;
use crate::util::radix_heap::{HasTime, RadixHeapQueue};

// ---------------------------------------------------------------------------
// Search-specific event type for the radix heap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum SearchEvent {
    NoEvent,
    LookAtNode { node: SearchNodeIdx, time: Wrapping<u32> },
}

impl HasTime for SearchEvent {
    fn time(&self) -> Wrapping<u32> {
        match self {
            SearchEvent::NoEvent => Wrapping(0),
            SearchEvent::LookAtNode { time, .. } => *time,
        }
    }
    fn no_event() -> Self {
        SearchEvent::NoEvent
    }
    fn is_no_event(&self) -> bool {
        matches!(self, SearchEvent::NoEvent)
    }
}

// ---------------------------------------------------------------------------
// Collision edge returned by the search
// ---------------------------------------------------------------------------

/// The edge on which two search regions collided.
#[derive(Debug, Clone, Copy)]
pub struct SearchGraphEdge {
    pub node: Option<SearchNodeIdx>,
    pub neighbor_index: usize,
}

// ---------------------------------------------------------------------------
// Target type for the search
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetType {
    DetectorNode,
    Boundary,
    NoTarget,
}

// ---------------------------------------------------------------------------
// SearchFlooder
// ---------------------------------------------------------------------------

/// Bidirectional Dijkstra on the search graph.
///
/// Used by Mwpm to extract the actual shortest path between matched nodes
/// after the flooding phase finds the matching.
pub struct SearchFlooder {
    pub graph: SearchGraph,
    pub queue: RadixHeapQueue<SearchEvent>,
    reached_nodes: Vec<SearchNodeIdx>,
    target_type: TargetType,
}

impl SearchFlooder {
    pub fn new(graph: SearchGraph) -> Self {
        SearchFlooder {
            graph,
            queue: RadixHeapQueue::new(),
            reached_nodes: Vec::new(),
            target_type: TargetType::NoTarget,
        }
    }

    // -- internal helpers ---------------------------------------------------

    /// Find the best next event (neighbor index, collision time) for a node.
    fn find_next_event(
        &self,
        node_idx: SearchNodeIdx,
    ) -> (Option<usize>, CumulativeTime) {
        let node = &self.graph.nodes[node_idx.0 as usize];
        let mut best_time: CumulativeTime = CumulativeTime::MAX;
        let mut best_neighbor: Option<usize> = None;

        let mut start = 0usize;

        // Check boundary neighbor (always at index 0 if present).
        if !node.neighbors.is_empty() && node.neighbors[0].is_none() {
            if self.target_type == TargetType::Boundary {
                let weight = node.neighbor_weights[0] as CumulativeTime;
                let covered = self.queue.cur_time - node.distance_from_source;
                let collision_time = self.queue.cur_time + weight - covered;
                if collision_time < best_time {
                    best_time = collision_time;
                    best_neighbor = Some(0);
                }
            }
            start = 1;
        }

        // Non-boundary neighbors.
        for i in start..node.neighbors.len() {
            let weight = node.neighbor_weights[i] as CumulativeTime;
            let nb_idx = node.neighbors[i].unwrap();
            let nb = &self.graph.nodes[nb_idx.0 as usize];

            let collision_time;
            if nb.reached_from_source == node.reached_from_source {
                // Same source -- skip.
                continue;
            } else if nb.reached_from_source.is_none() {
                // Unreached neighbor.
                let covered = self.queue.cur_time - node.distance_from_source;
                collision_time = self.queue.cur_time + weight - covered;
            } else {
                // Reached from different source -- two fronts meeting.
                let covered_this = self.queue.cur_time - node.distance_from_source;
                let covered_nb = self.queue.cur_time - nb.distance_from_source;
                collision_time =
                    self.queue.cur_time + (weight - covered_this - covered_nb) / 2;
            }

            if collision_time < best_time {
                best_time = collision_time;
                best_neighbor = Some(i);
            }
        }

        (best_neighbor, best_time)
    }

    /// Schedule the next event for a node.
    fn reschedule_events(&mut self, node_idx: SearchNodeIdx) {
        let (best_nb, best_time) = self.find_next_event(node_idx);
        let tracker =
            &mut self.graph.nodes[node_idx.0 as usize].node_event_tracker;
        match best_nb {
            None => tracker.set_no_desired_event(),
            Some(_) => {
                let event = SearchEvent::LookAtNode {
                    node: node_idx,
                    time: Wrapping(best_time as u32),
                };
                tracker.set_desired_event(event, &mut self.queue);
            }
        }
    }

    /// Start a search from an empty (unreached) node.
    fn start_at_empty_node(&mut self, src: SearchNodeIdx) {
        {
            let node = &mut self.graph.nodes[src.0 as usize];
            node.reached_from_source = Some(src);
            node.index_of_predecessor = None;
            node.distance_from_source = 0;
        }
        self.reached_nodes.push(src);
        self.reschedule_events(src);
    }

    /// Explore an unreached neighbor.
    fn explore_empty_node(
        &mut self,
        empty_idx: SearchNodeIdx,
        empty_to_from_index: usize,
    ) {
        let from_idx =
            self.graph.nodes[empty_idx.0 as usize].neighbors[empty_to_from_index]
                .unwrap();
        let from_source =
            self.graph.nodes[from_idx.0 as usize].reached_from_source;
        let from_dist =
            self.graph.nodes[from_idx.0 as usize].distance_from_source;
        let weight = self.graph.nodes[empty_idx.0 as usize].neighbor_weights
            [empty_to_from_index] as CumulativeTime;

        {
            let empty = &mut self.graph.nodes[empty_idx.0 as usize];
            empty.reached_from_source = from_source;
            empty.index_of_predecessor = Some(empty_to_from_index);
            empty.distance_from_source = weight + from_dist;
        }
        self.reached_nodes.push(empty_idx);
        self.reschedule_events(empty_idx);
    }

    /// Process a "look at node" event. Returns a collision edge if found.
    fn do_look_at_node_event(
        &mut self,
        node_idx: SearchNodeIdx,
    ) -> SearchGraphEdge {
        let (next_nb, next_time) = self.find_next_event(node_idx);

        if let Some(nb_i) = next_nb {
            if next_time == self.queue.cur_time {
                let dst_opt =
                    self.graph.nodes[node_idx.0 as usize].neighbors[nb_i];
                match dst_opt {
                    None => {
                        // Boundary collision.
                        return SearchGraphEdge {
                            node: Some(node_idx),
                            neighbor_index: nb_i,
                        };
                    }
                    Some(dst_idx) => {
                        let dst_reached = self.graph.nodes[dst_idx.0 as usize]
                            .reached_from_source;
                        if dst_reached.is_none() {
                            // Explore the empty neighbor.
                            let reverse_idx = self.graph.nodes
                                [dst_idx.0 as usize]
                                .index_of_neighbor(Some(node_idx));
                            self.explore_empty_node(dst_idx, reverse_idx);
                            // Revisit this node immediately.
                            let tracker = &mut self.graph.nodes
                                [node_idx.0 as usize]
                                .node_event_tracker;
                            let event = SearchEvent::LookAtNode {
                                node: node_idx,
                                time: Wrapping(self.queue.cur_time as u32),
                            };
                            tracker.set_desired_event(
                                event,
                                &mut self.queue,
                            );
                            return SearchGraphEdge {
                                node: None,
                                neighbor_index: NO_NEIGHBOR,
                            };
                        } else {
                            // Two-front collision.
                            return SearchGraphEdge {
                                node: Some(node_idx),
                                neighbor_index: nb_i,
                            };
                        }
                    }
                }
            } else {
                // Revisit later.
                let tracker =
                    &mut self.graph.nodes[node_idx.0 as usize]
                        .node_event_tracker;
                let event = SearchEvent::LookAtNode {
                    node: node_idx,
                    time: Wrapping(next_time as u32),
                };
                tracker.set_desired_event(event, &mut self.queue);
            }
        }

        SearchGraphEdge {
            node: None,
            neighbor_index: NO_NEIGHBOR,
        }
    }

    // -- public API ---------------------------------------------------------

    /// Run bidirectional Dijkstra from `src` to `dst`.
    /// `dst` is `None` for boundary search.
    /// Returns the collision edge.
    pub fn run_until_collision(
        &mut self,
        src: SearchNodeIdx,
        dst: Option<SearchNodeIdx>,
    ) -> SearchGraphEdge {
        match dst {
            None => {
                self.target_type = TargetType::Boundary;
            }
            Some(d) => {
                self.target_type = TargetType::DetectorNode;
                self.start_at_empty_node(d);
            }
        }
        self.start_at_empty_node(src);

        while !self.queue.is_empty() {
            let ev = self.queue.dequeue();
            if let SearchEvent::LookAtNode { node, .. } = ev {
                let n = &mut self.graph.nodes[node.0 as usize];
                let should_process = n.node_event_tracker.dequeue_decision(
                    &ev,
                    &mut self.queue,
                    |t| SearchEvent::LookAtNode { node, time: t },
                );
                if should_process {
                    let edge = self.do_look_at_node_event(node);
                    if edge.node.is_some() {
                        return edge;
                    }
                }
            }
        }

        SearchGraphEdge {
            node: None,
            neighbor_index: NO_NEIGHBOR,
        }
    }

    /// Trace back from a node to its source, collecting edges.
    fn trace_back_from_node(
        &self,
        start: SearchNodeIdx,
    ) -> Vec<SearchGraphEdge> {
        let mut edges = Vec::new();
        let mut cur = start;
        loop {
            let pred = self.graph.nodes[cur.0 as usize].index_of_predecessor;
            match pred {
                None => break,
                Some(pred_idx) => {
                    edges.push(SearchGraphEdge {
                        node: Some(cur),
                        neighbor_index: pred_idx,
                    });
                    cur = self.graph.nodes[cur.0 as usize].neighbors[pred_idx]
                        .unwrap();
                }
            }
        }
        edges
    }

    /// Iterate edges on the shortest path from `src` to `dst` (in order),
    /// calling `callback` with `(from: Option<SearchNodeIdx>, to: Option<SearchNodeIdx>, obs_mask)`.
    pub fn iter_edges_on_shortest_path(
        &mut self,
        src: usize,
        dst: Option<usize>,
        mut callback: impl FnMut(Option<SearchNodeIdx>, Option<SearchNodeIdx>, ObsMask),
    ) {
        let src_idx = SearchNodeIdx(src as u32);
        let dst_idx = dst.map(|d| SearchNodeIdx(d as u32));

        let collision_edge = self.run_until_collision(src_idx, dst_idx);

        if collision_edge.node.is_none() {
            self.reset();
            return;
        }

        let collision_node = collision_edge.node.unwrap();

        // Path 1: trace back from collision node.
        let path1 = self.trace_back_from_node(collision_node);

        // The collision edge itself.
        let other_opt = self.graph.nodes[collision_node.0 as usize].neighbors
            [collision_edge.neighbor_index];

        // Path 2: trace back from the other side of the collision edge.
        let mut path2 = vec![collision_edge];
        if let Some(other) = other_opt {
            let mut more = self.trace_back_from_node(other);
            path2.append(&mut more);
        }

        // Determine which path leads back to src.
        let last_of_path2 = {
            let last_edge = path2.last().unwrap();
            self.graph.nodes[last_edge.node.unwrap().0 as usize].neighbors
                [last_edge.neighbor_index]
        };

        let leads_to_src = last_of_path2 == Some(src_idx);

        if leads_to_src {
            // Reverse path2 (it goes collision->src, we want src->collision).
            self.emit_reversed(&path2, &mut callback);
            // Path1 goes collision->dst, emit in order.
            self.emit_forward(&path1, &mut callback);
        } else {
            // Reverse path1 (it goes collision->src, we want src->collision).
            self.emit_reversed(&path1, &mut callback);
            // Path2 goes collision->dst, emit in order.
            self.emit_forward(&path2, &mut callback);
        }

        self.reset();
    }

    /// Emit edges in forward order (node -> neighbor).
    fn emit_forward(
        &self,
        edges: &[SearchGraphEdge],
        callback: &mut impl FnMut(Option<SearchNodeIdx>, Option<SearchNodeIdx>, ObsMask),
    ) {
        for e in edges {
            let from = e.node;
            let node_i = e.node.unwrap().0 as usize;
            let to = self.graph.nodes[node_i].neighbors[e.neighbor_index];
            let obs = self.graph.nodes[node_i].neighbor_observables
                [e.neighbor_index];
            callback(from, to, obs);
        }
    }

    /// Emit edges in reversed order (neighbor -> node, traversed backwards).
    fn emit_reversed(
        &self,
        edges: &[SearchGraphEdge],
        callback: &mut impl FnMut(Option<SearchNodeIdx>, Option<SearchNodeIdx>, ObsMask),
    ) {
        for e in edges.iter().rev() {
            let node_i = e.node.unwrap().0 as usize;
            let nb_opt = self.graph.nodes[node_i].neighbors[e.neighbor_index];
            // Reversed: from = neighbor, to = node.
            let from = nb_opt;
            let to = e.node;
            // Find the reverse edge's observable.
            let obs = if let Some(nb_idx) = nb_opt {
                let reverse_i = self.graph.nodes[nb_idx.0 as usize]
                    .index_of_neighbor(e.node);
                self.graph.nodes[nb_idx.0 as usize].neighbor_observables
                    [reverse_i]
            } else {
                // Boundary edge -- use the same observable.
                self.graph.nodes[node_i].neighbor_observables
                    [e.neighbor_index]
            };
            callback(from, to, obs);
        }
    }

    /// Build a `CompressedEdge` for the shortest path between two nodes.
    pub fn find_shortest_path(
        &mut self,
        src: usize,
        dst: Option<usize>,
    ) -> CompressedEdge {
        let mut obs_mask: ObsMask = 0;
        self.iter_edges_on_shortest_path(src, dst, |_, _, obs| {
            obs_mask ^= obs;
        });
        CompressedEdge {
            loc_from: Some(NodeIdx(src as u32)),
            loc_to: dst.map(|d| NodeIdx(d as u32)),
            obs_mask,
        }
    }

    /// Reset the graph and queue for the next search.
    pub fn reset(&mut self) {
        for &idx in &self.reached_nodes {
            self.graph.nodes[idx.0 as usize].reset();
        }
        self.reached_nodes.clear();
        self.queue.reset();
        self.target_type = TargetType::NoTarget;
    }
}
