use std::collections::HashSet;

use crate::flooder::graph::MatchingGraph;
use crate::flooder::graph_flooder::GraphFlooder;
use crate::matcher::mwpm::Mwpm;
use crate::search::search_graph::SearchGraph;
use crate::types::*;

/// Number of distinct weight levels for discretization.
/// Matches PyMatching's `NUM_DISTINCT_WEIGHTS = 1 << (sizeof(weight_int)*8 - 8)`.
pub const NUM_DISTINCT_WEIGHTS: Weight = 1 << (std::mem::size_of::<Weight>() * 8 - 8);

/// A user-facing edge between two detector nodes (or one node and boundary).
#[derive(Debug, Clone)]
pub struct UserEdge {
    pub node1: usize,
    pub node2: usize,
    pub observable_indices: Vec<usize>,
    pub weight: f64,
    pub error_probability: f64,
}

/// Placeholder for per-node metadata.
#[derive(Debug, Clone, Default)]
pub struct UserNode {
    pub is_boundary: bool,
}

/// High-level graph that accumulates edges from user / DEM input and
/// converts them to the internal `MatchingGraph` / `SearchGraph` / `Mwpm`.
pub struct UserGraph {
    pub nodes: Vec<UserNode>,
    pub edges: Vec<UserEdge>,
    pub boundary_nodes: HashSet<usize>,
    pub num_observables: usize,
    mwpm: Option<Mwpm>,
    all_edges_have_error_probabilities: bool,
}

impl UserGraph {
    pub fn new() -> Self {
        UserGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            boundary_nodes: HashSet::new(),
            num_observables: 0,
            mwpm: None,
            all_edges_have_error_probabilities: true,
        }
    }

    /// Ensure `nodes` is large enough to hold index `id`.
    fn ensure_node(&mut self, id: usize) {
        if id >= self.nodes.len() {
            self.nodes.resize_with(id + 1, UserNode::default);
        }
    }

    /// Track observable count from a set of observable indices.
    fn update_num_observables(&mut self, observables: &[usize]) {
        for &obs in observables {
            if obs + 1 > self.num_observables {
                self.num_observables = obs + 1;
            }
        }
    }

    /// Add an edge between two detector nodes.
    pub fn add_edge(
        &mut self,
        node1: usize,
        node2: usize,
        observables: Vec<usize>,
        weight: f64,
        error_probability: f64,
    ) {
        self.ensure_node(node1.max(node2));
        self.update_num_observables(&observables);
        if !(0.0..=1.0).contains(&error_probability) {
            self.all_edges_have_error_probabilities = false;
        }
        self.edges.push(UserEdge {
            node1,
            node2,
            observable_indices: observables,
            weight,
            error_probability,
        });
        self.mwpm = None;
    }

    /// Add an edge from a detector node to the boundary.
    /// Internally stored with `node2 = usize::MAX`.
    pub fn add_boundary_edge(
        &mut self,
        node: usize,
        observables: Vec<usize>,
        weight: f64,
        error_probability: f64,
    ) {
        self.ensure_node(node);
        self.update_num_observables(&observables);
        if !(0.0..=1.0).contains(&error_probability) {
            self.all_edges_have_error_probabilities = false;
        }
        self.edges.push(UserEdge {
            node1: node,
            node2: usize::MAX,
            observable_indices: observables,
            weight,
            error_probability,
        });
        self.mwpm = None;
    }

    /// Mark a set of nodes as boundary nodes.
    pub fn set_boundary(&mut self, nodes: HashSet<usize>) {
        // Clear old boundary flags
        for &n in &self.boundary_nodes {
            if n < self.nodes.len() {
                self.nodes[n].is_boundary = false;
            }
        }
        self.boundary_nodes = nodes;
        let max_boundary = self.boundary_nodes.iter().copied().max();
        if let Some(m) = max_boundary {
            self.ensure_node(m);
        }
        for &n in &self.boundary_nodes {
            self.nodes[n].is_boundary = true;
        }
        self.mwpm = None;
    }

    /// Whether a node index represents a boundary node.
    pub fn is_boundary_node(&self, node_id: usize) -> bool {
        node_id == usize::MAX
            || (node_id < self.nodes.len() && self.nodes[node_id].is_boundary)
    }

    /// Maximum absolute weight across all edges.
    fn max_abs_weight(&self) -> f64 {
        self.edges
            .iter()
            .map(|e| e.weight.abs())
            .fold(0.0f64, f64::max)
    }

    /// Compute the normalising constant for weight discretization.
    ///
    /// If all weights are integral, returns 1.0.
    /// Otherwise: `(num_distinct_weights - 1) / max_abs_weight`.
    fn get_edge_weight_normalising_constant(
        &self,
        num_distinct_weights: Weight,
    ) -> f64 {
        let max_abs = self.max_abs_weight();
        let all_integral = self
            .edges
            .iter()
            .all(|e| e.weight.round() == e.weight);
        if all_integral {
            1.0
        } else {
            let max_half: f64 = (num_distinct_weights - 1) as f64;
            max_half / max_abs
        }
    }

    /// Convert observable indices to a bitmask.
    fn obs_mask(observables: &[usize]) -> ObsMask {
        let mut mask: ObsMask = 0;
        for &obs in observables {
            mask ^= 1u64 << obs;
        }
        mask
    }

    /// Convert to a `MatchingGraph` with discretized weights.
    pub fn to_matching_graph(
        &self,
        num_distinct_weights: Weight,
    ) -> MatchingGraph {
        let mut mg =
            MatchingGraph::new(self.nodes.len(), self.num_observables);
        let norm = self.get_edge_weight_normalising_constant(num_distinct_weights);

        // Collect boundary edges per node, keeping only the smallest signed weight
        // (matches PyMatching's parallel boundary edge deduplication).
        let num_nodes = self.nodes.len();
        let mut has_boundary_edge = vec![false; num_nodes];
        let mut boundary_edge_weights: Vec<SignedWeight> = vec![0; num_nodes];
        let mut boundary_edge_observables: Vec<Vec<usize>> = vec![Vec::new(); num_nodes];

        for e in &self.edges {
            let w = (e.weight * norm).round() as SignedWeight * 2;
            let n1_boundary = self.is_boundary_node(e.node1);
            let n2_boundary = self.is_boundary_node(e.node2);

            if n2_boundary && !n1_boundary {
                if !has_boundary_edge[e.node1] || boundary_edge_weights[e.node1] > w {
                    boundary_edge_weights[e.node1] = w;
                    boundary_edge_observables[e.node1] = e.observable_indices.clone();
                    has_boundary_edge[e.node1] = true;
                }
            } else if n1_boundary && !n2_boundary {
                if !has_boundary_edge[e.node2] || boundary_edge_weights[e.node2] > w {
                    boundary_edge_weights[e.node2] = w;
                    boundary_edge_observables[e.node2] = e.observable_indices.clone();
                    has_boundary_edge[e.node2] = true;
                }
            } else if !n1_boundary {
                mg.add_edge(e.node1, e.node2, w, &e.observable_indices);
            }
        }

        // Now add the deduplicated boundary edges
        for i in 0..num_nodes {
            if has_boundary_edge[i] {
                mg.add_boundary_edge(i, boundary_edge_weights[i], &boundary_edge_observables[i]);
            }
        }

        mg.normalising_constant = norm * 2.0;

        if !self.boundary_nodes.is_empty() {
            mg.is_user_graph_boundary_node = vec![false; self.nodes.len()];
            for &i in &self.boundary_nodes {
                mg.is_user_graph_boundary_node[i] = true;
            }
        }

        mg
    }

    /// Convert to a `SearchGraph` with discretized weights.
    pub fn to_search_graph(
        &self,
        num_distinct_weights: Weight,
    ) -> SearchGraph {
        let mut sg =
            SearchGraph::new(self.nodes.len(), self.num_observables);
        let norm = self.get_edge_weight_normalising_constant(num_distinct_weights);

        // Collect boundary edges per node, keeping only the smallest signed weight
        let num_nodes = self.nodes.len();
        let mut has_boundary_edge = vec![false; num_nodes];
        let mut boundary_edge_weights: Vec<SignedWeight> = vec![0; num_nodes];
        let mut boundary_edge_obs: Vec<ObsMask> = vec![0; num_nodes];

        for e in &self.edges {
            let w_signed = (e.weight * norm).round() as SignedWeight * 2;
            let obs = Self::obs_mask(&e.observable_indices);
            let n1_boundary = self.is_boundary_node(e.node1);
            let n2_boundary = self.is_boundary_node(e.node2);

            if n2_boundary && !n1_boundary {
                if !has_boundary_edge[e.node1] || boundary_edge_weights[e.node1] > w_signed {
                    boundary_edge_weights[e.node1] = w_signed;
                    boundary_edge_obs[e.node1] = obs;
                    has_boundary_edge[e.node1] = true;
                }
            } else if n1_boundary && !n2_boundary {
                if !has_boundary_edge[e.node2] || boundary_edge_weights[e.node2] > w_signed {
                    boundary_edge_weights[e.node2] = w_signed;
                    boundary_edge_obs[e.node2] = obs;
                    has_boundary_edge[e.node2] = true;
                }
            } else if !n1_boundary {
                let w = w_signed.unsigned_abs();
                sg.add_edge(e.node1, e.node2, w, obs);
            }
        }

        // Now add the deduplicated boundary edges
        for i in 0..num_nodes {
            if has_boundary_edge[i] {
                let w = boundary_edge_weights[i].unsigned_abs();
                sg.add_boundary_edge(i, w, boundary_edge_obs[i]);
            }
        }

        sg
    }

    /// Build a full `Mwpm` solver from the current graph.
    pub fn to_mwpm(&self) -> Mwpm {
        let mg = self.to_matching_graph(NUM_DISTINCT_WEIGHTS);
        let flooder = GraphFlooder::new(mg);
        Mwpm::new(flooder)
    }

    /// Lazy-initialise and return a mutable reference to the cached `Mwpm`.
    pub fn get_mwpm(&mut self) -> &mut Mwpm {
        if self.mwpm.is_none() {
            self.mwpm = Some(self.to_mwpm());
        }
        self.mwpm.as_mut().unwrap()
    }

    /// Handle a detector-error-model instruction.
    ///
    /// Converts probability `p` to weight `ln((1-p)/p)` and adds the
    /// appropriate edge.
    pub fn handle_dem_instruction(
        &mut self,
        p: f64,
        detectors: &[usize],
        observables: Vec<usize>,
    ) {
        let weight = ((1.0 - p) / p).ln();
        match detectors.len() {
            2 => self.add_edge(
                detectors[0],
                detectors[1],
                observables,
                weight,
                p,
            ),
            1 => self.add_boundary_edge(detectors[0], observables, weight, p),
            _ => {}
        }
    }

    pub fn get_num_edges(&self) -> usize {
        self.edges.len()
    }

    pub fn get_num_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_num_detectors(&self) -> usize {
        self.nodes.len() - self.boundary_nodes.len()
    }
}
