use std::collections::{HashMap, HashSet};

use rmatching::Matching;

use super::{CompiledCircuit, DecodeFailure, Effect, ShotFailure};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum EdgeKind {
    TimeLike,
    SpaceLike,
    Boundary,
}

#[derive(Clone, Debug)]
pub(super) struct GraphEdge {
    pub(super) node1: usize,
    pub(super) node2: Option<usize>,
    pub(super) observables: Vec<usize>,
    pub(super) weight: f64,
    pub(super) kind: EdgeKind,
}

pub(super) struct CompiledMatching {
    pub(super) edges: Vec<GraphEdge>,
    pub(super) loss_edges: Vec<Vec<usize>>,
    pub(super) mean_weight: f64,
    pub(super) num_observables: usize,
    pub(super) cache: HashMap<Vec<usize>, Matching>,
}

impl CompiledMatching {
    pub(super) fn new(circuit: &CompiledCircuit) -> Result<Self, DecodeFailure> {
        validate_unambiguous_parallel_edges(&circuit.graph_edges)?;
        for envelope in &circuit.envelopes {
            for candidate in &envelope.candidates {
                if candidate.detectors.is_empty() && candidate.observables.is_empty() {
                    continue;
                }
                if !circuit
                    .graph_edges
                    .iter()
                    .any(|edge| candidate_affects_edge(candidate, edge))
                {
                    return Err(DecodeFailure::new(
                        "unsupported_circuit",
                        format!(
                            "loss candidate {:?} of {:?} has no compatible matching edge",
                            candidate.id, envelope.id
                        ),
                    ));
                }
            }
        }
        if let Some(envelope) = circuit
            .loss_edges
            .iter()
            .zip(&circuit.envelopes)
            .find(|(edges, _)| edges.is_empty())
            .map(|(_, envelope)| envelope)
        {
            return Err(DecodeFailure::new(
                "unsupported_circuit",
                format!(
                    "loss envelope {:?} has no compatible matching edge",
                    envelope.id
                ),
            ));
        }
        let mean_weight = circuit
            .graph_edges
            .iter()
            .map(|edge| edge.weight)
            .sum::<f64>()
            / circuit.graph_edges.len() as f64;
        Ok(Self {
            edges: circuit.graph_edges.clone(),
            loss_edges: circuit.loss_edges.clone(),
            mean_weight,
            num_observables: circuit.num_observables,
            cache: HashMap::new(),
        })
    }

    pub(super) fn decode(
        &mut self,
        syndrome: &[u8],
        losses: &[usize],
    ) -> Result<Vec<usize>, ShotFailure> {
        let key = losses.to_vec();
        if !self.cache.contains_key(&key) {
            let matching = build_matching(&self.edges, &self.loss_edges, self.mean_weight, losses);
            self.cache.insert(key.clone(), matching);
        }
        let bits = self.cache.get_mut(&key).unwrap().decode(syndrome);
        Ok(bits
            .iter()
            .chain(std::iter::repeat(&0))
            .take(self.num_observables)
            .enumerate()
            .filter_map(|(index, &bit)| (bit != 0).then_some(index))
            .collect())
    }
}

pub(super) fn candidate_affects_edge(effect: &Effect, edge: &GraphEdge) -> bool {
    effect.detectors.contains(&edge.node1)
        && edge
            .node2
            .is_none_or(|node2| effect.detectors.contains(&node2))
}

pub(super) fn validate_unambiguous_parallel_edges(
    edges: &[GraphEdge],
) -> Result<(), DecodeFailure> {
    let mut endpoint_labels = HashMap::<(usize, Option<usize>), (Vec<usize>, EdgeKind)>::new();
    for edge in edges {
        let key = match edge.node2 {
            Some(node2) if node2 < edge.node1 => (node2, Some(edge.node1)),
            _ => (edge.node1, edge.node2),
        };
        let label = (edge.observables.clone(), edge.kind);
        if let Some(existing) = endpoint_labels.get(&key) {
            if existing != &label {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    format!(
                        "parallel matching edges at endpoints {key:?} have ambiguous observable labels or edge kinds"
                    ),
                ));
            }
        } else {
            endpoint_labels.insert(key, label);
        }
    }
    Ok(())
}

fn build_matching(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    mean_weight: f64,
    losses: &[usize],
) -> Matching {
    let active: HashSet<usize> = losses
        .iter()
        .flat_map(|&loss| loss_edges[loss].iter().copied())
        .collect();
    let scale = edges.iter().map(|edge| edge.weight).fold(1.0f64, f64::max);
    let mut matching = Matching::new();
    for (index, edge) in edges.iter().enumerate() {
        let weight = if active.contains(&index) {
            match edge.kind {
                EdgeKind::TimeLike => 0.25 * mean_weight,
                EdgeKind::SpaceLike | EdgeKind::Boundary => 0.5 * mean_weight,
            }
        } else {
            edge.weight
        } / scale;
        let probability = 1.0 / (1.0 + weight.exp());
        if let Some(node2) = edge.node2 {
            matching.add_edge(edge.node1, node2, weight, &edge.observables, probability);
        } else {
            matching.add_boundary_edge(edge.node1, weight, &edge.observables, probability);
        }
    }
    matching.prepare();
    matching
}
