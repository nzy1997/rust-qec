use crate::types::*;
use std::collections::HashSet;

use super::detector_node::DetectorNode;

/// Sentinel NodeIdx for boundary neighbors.
pub const BOUNDARY_NODE: NodeIdx = NodeIdx(u32::MAX);

pub struct MatchingGraph {
    pub nodes: Vec<DetectorNode>,
    pub num_observables: usize,
    pub negative_weight_detection_events_set: HashSet<usize>,
    pub negative_weight_observables_set: HashSet<usize>,
    pub negative_weight_obs_mask: ObsMask,
    pub negative_weight_sum: TotalWeight,
    pub is_user_graph_boundary_node: Vec<bool>,
    pub normalising_constant: f64,
}

impl MatchingGraph {
    pub fn new(num_nodes: usize, num_observables: usize) -> Self {
        MatchingGraph {
            nodes: (0..num_nodes).map(|_| DetectorNode::new()).collect(),
            num_observables,
            negative_weight_detection_events_set: HashSet::new(),
            negative_weight_observables_set: HashSet::new(),
            negative_weight_obs_mask: 0,
            negative_weight_sum: 0,
            is_user_graph_boundary_node: Vec::new(),
            normalising_constant: 1.0,
        }
    }

    pub fn add_edge(
        &mut self,
        u: usize,
        v: usize,
        weight: SignedWeight,
        observables: &[usize],
    ) {
        if weight < 0 {
            for &obs in observables {
                if !self.negative_weight_observables_set.remove(&obs) {
                    self.negative_weight_observables_set.insert(obs);
                }
            }
            if !self.negative_weight_detection_events_set.remove(&u) {
                self.negative_weight_detection_events_set.insert(u);
            }
            if !self.negative_weight_detection_events_set.remove(&v) {
                self.negative_weight_detection_events_set.insert(v);
            }
            self.negative_weight_sum += weight as TotalWeight;
        }

        let mut obs_mask: ObsMask = 0;
        for &obs in observables {
            if obs < 64 {
                obs_mask ^= 1u64 << obs;
            }
        }

        if u == v {
            return; // skip self-loops
        }

        let abs_weight = weight.unsigned_abs();

        // Add u -> v
        self.nodes[u].neighbors.push(NodeIdx(v as u32));
        self.nodes[u].neighbor_weights.push(abs_weight);
        self.nodes[u].neighbor_observables.push(obs_mask);

        // Add v -> u
        self.nodes[v].neighbors.push(NodeIdx(u as u32));
        self.nodes[v].neighbor_weights.push(abs_weight);
        self.nodes[v].neighbor_observables.push(obs_mask);
    }

    pub fn add_boundary_edge(
        &mut self,
        u: usize,
        weight: SignedWeight,
        observables: &[usize],
    ) {
        if weight < 0 {
            for &obs in observables {
                if !self.negative_weight_observables_set.remove(&obs) {
                    self.negative_weight_observables_set.insert(obs);
                }
            }
            if !self.negative_weight_detection_events_set.remove(&u) {
                self.negative_weight_detection_events_set.insert(u);
            }
            self.negative_weight_sum += weight as TotalWeight;
        }

        let abs_weight = weight.unsigned_abs();
        let mut obs_mask: ObsMask = 0;
        for &obs in observables {
            if obs < 64 {
                obs_mask ^= 1u64 << obs;
            }
        }

        // Boundary edge: neighbor is BOUNDARY_NODE sentinel
        self.nodes[u].neighbors.push(BOUNDARY_NODE);
        self.nodes[u].neighbor_weights.push(abs_weight);
        self.nodes[u].neighbor_observables.push(obs_mask);
    }
}
