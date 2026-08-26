use std::collections::{HashMap, HashSet};

use rmatching::Matching;
use rstim::m2d::{LossAwareDetectorCheck, LossAwareDetectorShot};

use super::{
    CompiledCircuit, DecodeFailure, Effect, MAX_CONDITIONED_DECODER_INCIDENCES, ShotFailure,
};

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
    pub(super) cache: HashMap<Vec<usize>, ConditionedMatching>,
}

pub(super) struct ConditionedMatching {
    matching: Matching,
    check_sources: Vec<Vec<usize>>,
}

impl CompiledMatching {
    pub(super) fn new(circuit: &CompiledCircuit) -> Result<Self, DecodeFailure> {
        validate_unambiguous_parallel_edges(&circuit.graph_edges)?;
        // `loss_edges` is the union of graphlike primitive effects present in an
        // envelope. The compiler validates each primitive while building it; a
        // composite candidate can span a multi-edge path and does not itself
        // need to contain both endpoints of one base edge.
        if let Some((envelope, primitive)) = circuit
            .envelopes
            .iter()
            .zip(&circuit.unmapped_loss_primitives)
            .find_map(|(envelope, primitives)| {
                primitives.first().map(|primitive| (envelope, primitive))
            })
        {
            return Err(DecodeFailure::new(
                "unsupported_circuit",
                format!(
                    "primitive loss effect {primitive} of {:?} has no compatible matching edge",
                    envelope.id
                ),
            ));
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
        syndrome: &LossAwareDetectorShot,
        losses: &[usize],
    ) -> Result<Vec<usize>, ShotFailure> {
        let key = losses.to_vec();
        if !self.cache.contains_key(&key) {
            let matching = build_matching(
                &self.edges,
                &self.loss_edges,
                self.mean_weight,
                &syndrome.checks,
                losses,
            )?;
            self.cache.insert(key.clone(), matching);
        }
        let conditioned = self.cache.get_mut(&key).unwrap();
        if conditioned.check_sources.len() != syndrome.checks.len()
            || conditioned
                .check_sources
                .iter()
                .zip(&syndrome.checks)
                .any(|(expected, actual)| expected != &actual.source_detectors)
        {
            return Err(ShotFailure::Other(
                "loss pattern produced inconsistent detector-check basis".to_string(),
            ));
        }
        let check_values: Vec<u8> = syndrome
            .checks
            .iter()
            .map(|check| u8::from(check.value))
            .collect();
        let bits = conditioned.matching.decode(&check_values);
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
    checks: &[LossAwareDetectorCheck],
    losses: &[usize],
) -> Result<ConditionedMatching, ShotFailure> {
    let incidences = edges.len().checked_mul(checks.len()).ok_or_else(|| {
        ShotFailure::Other("conditioned matching incidence limit exceeded".to_string())
    })?;
    if incidences > MAX_CONDITIONED_DECODER_INCIDENCES {
        return Err(ShotFailure::Other(
            "conditioned matching incidence limit exceeded".to_string(),
        ));
    }
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
        let transformed = transformed_check_nodes(edge, checks);
        if transformed.len() > 2 {
            return Err(ShotFailure::Other(format!(
                "loss pattern turns a graphlike mechanism into a {}-detector hyperedge",
                transformed.len()
            )));
        }
        if transformed.is_empty() {
            // A positive-weight mechanism with no surviving detector symptom
            // has maximum-likelihood representative "not selected". Matching
            // cannot express an observable-only edge, so omit it instead of
            // inventing a detector or splitting its logical label.
            continue;
        }
        let probability = 1.0 / (1.0 + weight.exp());
        if transformed.len() == 2 {
            matching.add_edge(
                transformed[0],
                transformed[1],
                weight,
                &edge.observables,
                probability,
            );
        } else {
            matching.add_boundary_edge(transformed[0], weight, &edge.observables, probability);
        }
    }
    matching.prepare();
    Ok(ConditionedMatching {
        matching,
        check_sources: checks
            .iter()
            .map(|check| check.source_detectors.clone())
            .collect(),
    })
}

fn transformed_check_nodes(edge: &GraphEdge, checks: &[LossAwareDetectorCheck]) -> Vec<usize> {
    checks
        .iter()
        .enumerate()
        .filter_map(|(check_index, check)| {
            let mut parity = check.source_detectors.contains(&edge.node1);
            if let Some(node2) = edge.node2 {
                parity ^= check.source_detectors.contains(&node2);
            }
            parity.then_some(check_index)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary_edge() -> GraphEdge {
        GraphEdge {
            node1: 0,
            node2: None,
            observables: Vec::new(),
            weight: 1.0,
            kind: EdgeKind::Boundary,
        }
    }

    #[test]
    fn conditioned_matching_rejects_hyperedges_without_splitting_them() {
        let checks: Vec<_> = (0..3)
            .map(|_| LossAwareDetectorCheck {
                source_detectors: vec![0],
                value: false,
            })
            .collect();
        let error = build_matching(&[boundary_edge()], &[], 1.0, &checks, &[])
            .err()
            .unwrap();
        assert!(
            matches!(error, ShotFailure::Other(message) if message.contains("3-detector hyperedge"))
        );
    }

    #[test]
    fn conditioned_matching_preflights_incidence_work() {
        let edges = vec![boundary_edge(); 10_001];
        let checks = vec![
            LossAwareDetectorCheck {
                source_detectors: Vec::new(),
                value: false,
            };
            1_000
        ];
        let error = build_matching(&edges, &[], 1.0, &checks, &[])
            .err()
            .unwrap();
        assert!(
            matches!(error, ShotFailure::Other(message) if message.contains("incidence limit"))
        );
    }
}
