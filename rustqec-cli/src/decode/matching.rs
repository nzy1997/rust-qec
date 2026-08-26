use std::collections::{HashMap, HashSet};

use rmatching::Matching;
use rstim::m2d::{LossAwareDetectorCheck, LossAwareDetectorShot};

use super::{
    CompiledCircuit, DecodeFailure, Effect, MAX_CONDITIONED_DECODER_ITEMS,
    MAX_CONDITIONED_DECODER_WORK, ShotFailure, conditioned_cache_total,
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
    pub(super) cached_work: usize,
}

pub(super) struct ConditionedMatching {
    matching: Matching,
    check_sources: Vec<Vec<usize>>,
    reachable_checks: Vec<bool>,
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
            cached_work: 0,
        })
    }

    pub(super) fn decode(
        &mut self,
        syndrome: &LossAwareDetectorShot,
        losses: &[usize],
    ) -> Result<Vec<usize>, ShotFailure> {
        let key = losses.to_vec();
        if !self.cache.contains_key(&key) {
            let artifact_work = conditioned_matching_work(&self.edges, &syndrome.checks)?;
            let cached_work = conditioned_cache_total(
                "matching",
                self.cache.len(),
                self.cached_work,
                artifact_work,
            )
            .map_err(ShotFailure::Other)?;
            let matching = build_matching(
                &self.edges,
                &self.loss_edges,
                self.mean_weight,
                &syndrome.checks,
                losses,
            )?;
            self.cache.insert(key.clone(), matching);
            self.cached_work = cached_work;
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
        validate_reachable_checks(conditioned, &syndrome.checks)?;
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
    conditioned_matching_work(edges, checks)?;
    let active: HashSet<usize> = losses
        .iter()
        .flat_map(|&loss| loss_edges[loss].iter().copied())
        .collect();
    let scale = edges.iter().map(|edge| edge.weight).fold(1.0f64, f64::max);
    let mut matching = Matching::new();
    let mut reachable_checks = vec![false; checks.len()];
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
        for &check in &transformed {
            reachable_checks[check] = true;
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
        reachable_checks,
    })
}

fn conditioned_matching_work(
    edges: &[GraphEdge],
    checks: &[LossAwareDetectorCheck],
) -> Result<usize, ShotFailure> {
    let edge_terms = edges
        .iter()
        .try_fold(0usize, |total, edge| {
            total
                .checked_add(1 + usize::from(edge.node2.is_some()))?
                .checked_add(edge.observables.len())
        })
        .ok_or_else(conditioned_matching_limit_error)?;
    let check_terms = checks
        .iter()
        .try_fold(0usize, |total, check| {
            total.checked_add(check.source_detectors.len())
        })
        .ok_or_else(conditioned_matching_limit_error)?;
    conditioned_matching_work_from_counts(edges.len(), checks.len(), edge_terms, check_terms)
}

fn conditioned_matching_work_from_counts(
    edge_count: usize,
    check_count: usize,
    edge_terms: usize,
    check_terms: usize,
) -> Result<usize, ShotFailure> {
    if edge_count > MAX_CONDITIONED_DECODER_ITEMS
        || check_count > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_matching_limit_error());
    }
    let work = edge_count
        .checked_add(check_count)
        .and_then(|value| value.checked_add(edge_terms))
        .and_then(|value| value.checked_add(check_terms))
        .and_then(|value| value.checked_add(edge_count.checked_mul(check_count)?))
        .and_then(|value| value.checked_add(edge_terms.checked_mul(check_terms)?))
        .ok_or_else(conditioned_matching_limit_error)?;
    if work > MAX_CONDITIONED_DECODER_WORK {
        return Err(conditioned_matching_limit_error());
    }
    Ok(work)
}

fn conditioned_matching_limit_error() -> ShotFailure {
    ShotFailure::Other("conditioned matching work limit exceeded".to_string())
}

fn validate_reachable_checks(
    conditioned: &ConditionedMatching,
    checks: &[LossAwareDetectorCheck],
) -> Result<(), ShotFailure> {
    if conditioned
        .reachable_checks
        .iter()
        .zip(checks)
        .any(|(&reachable, check)| check.value && !reachable)
    {
        return Err(ShotFailure::Infeasible);
    }
    Ok(())
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
        let checks = vec![
            LossAwareDetectorCheck {
                source_detectors: vec![0],
                value: false,
            },
            LossAwareDetectorCheck {
                source_detectors: vec![0, 1],
                value: false,
            },
            LossAwareDetectorCheck {
                source_detectors: vec![0, 2],
                value: false,
            },
        ];
        let error = build_matching(&[boundary_edge()], &[], 1.0, &checks, &[])
            .err()
            .unwrap();
        assert!(
            matches!(error, ShotFailure::Other(message) if message.contains("3-detector hyperedge"))
        );
    }

    #[test]
    fn conditioned_matching_preflights_incidence_work() {
        let error = conditioned_matching_work_from_counts(10_001, 1_000, 0, 0).unwrap_err();
        assert!(
            matches!(error, ShotFailure::Other(message) if message.contains("work limit"))
        );
        assert!(conditioned_matching_work_from_counts(usize::MAX, 0, 0, 0).is_err());
        assert!(conditioned_matching_work_from_counts(1, 1, 0, usize::MAX).is_err());
    }

    #[test]
    fn conditioned_matching_marks_fired_unreachable_checks_infeasible() {
        let checks = vec![LossAwareDetectorCheck {
            source_detectors: vec![1],
            value: true,
        }];
        let mut decoder = CompiledMatching {
            edges: vec![boundary_edge()],
            loss_edges: Vec::new(),
            mean_weight: 1.0,
            num_observables: 0,
            cache: HashMap::new(),
            cached_work: 0,
        };
        let shot = LossAwareDetectorShot {
            lost_measurements: Vec::new(),
            detector_valid: vec![true, true],
            checks,
        };
        assert!(matches!(
            decoder.decode(&shot, &[]),
            Err(ShotFailure::Infeasible)
        ));
    }
}
