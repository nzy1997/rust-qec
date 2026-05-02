use crate::interop::QueuedEventTracker;
use crate::types::*;

/// A node in the search graph used for shortest-path extraction.
///
/// Mirrors PyMatching's `SearchDetectorNode`. Neighbors are stored as
/// `Option<SearchNodeIdx>` where `None` represents the boundary.
#[derive(Debug, Clone)]
pub struct SearchDetectorNode {
    // -- Permanent graph structure --
    pub neighbors: Vec<Option<SearchNodeIdx>>,
    pub neighbor_weights: Vec<Weight>,
    pub neighbor_observables: Vec<ObsMask>,

    // -- Ephemeral Dijkstra state --
    pub reached_from_source: Option<SearchNodeIdx>,
    pub distance_from_source: CumulativeTime,
    pub index_of_predecessor: Option<usize>,
    pub node_event_tracker: QueuedEventTracker,
}

impl SearchDetectorNode {
    pub fn new() -> Self {
        SearchDetectorNode {
            neighbors: Vec::new(),
            neighbor_weights: Vec::new(),
            neighbor_observables: Vec::new(),
            reached_from_source: None,
            distance_from_source: 0,
            index_of_predecessor: None,
            node_event_tracker: QueuedEventTracker::default(),
        }
    }

    /// Find the index of a neighbor by its `Option<SearchNodeIdx>`.
    pub fn index_of_neighbor(&self, target: Option<SearchNodeIdx>) -> usize {
        for (k, n) in self.neighbors.iter().enumerate() {
            if *n == target {
                return k;
            }
        }
        panic!("Failed to find neighbor");
    }

    /// Reset ephemeral Dijkstra state.
    pub fn reset(&mut self) {
        self.reached_from_source = None;
        self.distance_from_source = 0;
        self.index_of_predecessor = None;
        self.node_event_tracker.clear();
    }
}

impl Default for SearchDetectorNode {
    fn default() -> Self {
        Self::new()
    }
}

/// The search graph used for shortest-path extraction between matched nodes.
pub struct SearchGraph {
    pub nodes: Vec<SearchDetectorNode>,
    pub num_observables: usize,
}

impl SearchGraph {
    pub fn new(num_nodes: usize, num_observables: usize) -> Self {
        SearchGraph {
            nodes: (0..num_nodes).map(|_| SearchDetectorNode::new()).collect(),
            num_observables,
        }
    }

    /// Add an edge between two detector nodes.
    pub fn add_edge(
        &mut self,
        u: usize,
        v: usize,
        weight: Weight,
        obs_mask: ObsMask,
    ) {
        if u == v {
            return; // self-loops ignored
        }
        let u_idx = SearchNodeIdx(u as u32);
        let v_idx = SearchNodeIdx(v as u32);

        self.nodes[u].neighbors.push(Some(v_idx));
        self.nodes[u].neighbor_weights.push(weight);
        self.nodes[u].neighbor_observables.push(obs_mask);

        self.nodes[v].neighbors.push(Some(u_idx));
        self.nodes[v].neighbor_weights.push(weight);
        self.nodes[v].neighbor_observables.push(obs_mask);
    }

    /// Add a boundary edge (inserted at the front, matching C++ behavior).
    pub fn add_boundary_edge(
        &mut self,
        u: usize,
        weight: Weight,
        obs_mask: ObsMask,
    ) {
        self.nodes[u].neighbors.insert(0, None);
        self.nodes[u].neighbor_weights.insert(0, weight);
        self.nodes[u].neighbor_observables.insert(0, obs_mask);
    }
}
