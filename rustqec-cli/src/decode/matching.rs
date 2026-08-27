use std::collections::{HashMap, HashSet, VecDeque};

use rmatching::Matching;
#[cfg(test)]
use rstim::m2d::LossAwareDetectorCheck;
use rstim::m2d::LossAwareDetectorShot;

use super::{
    CompiledCircuit, DecodeFailure, MAX_CONDITIONED_DECODER_ITEMS, MAX_CONDITIONED_DECODER_WORK,
    ShotFailure, conditioned_cache_needs_eviction,
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
    pub(super) cache_order: VecDeque<Vec<usize>>,
    pub(super) cached_work: usize,
    pub(super) graph_builds: usize,
    pub(super) cache_hits: usize,
}

pub(super) struct ConditionedMatching {
    matching: Matching,
    detector_count: usize,
    reachable_detectors: Vec<bool>,
    work: usize,
}

#[cfg(test)]
struct ConditionedMatchingPreflight {
    work: usize,
    detector_checks: HashMap<usize, Vec<usize>>,
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
            cache_order: VecDeque::new(),
            cached_work: 0,
            graph_builds: 0,
            cache_hits: 0,
        })
    }

    pub(super) fn decode(
        &mut self,
        syndrome: &LossAwareDetectorShot,
        losses: &[usize],
    ) -> Result<Vec<usize>, ShotFailure> {
        let key = losses.to_vec();
        if self.cache.contains_key(&key) {
            self.cache_hits += 1;
        } else {
            let work = canonical_matching_preflight(
                &self.edges,
                &self.loss_edges,
                syndrome.canonical_detector_values.len(),
                losses,
            )?;
            self.evict_until_fits(work)?;
            let matching = build_canonical_matching(
                &self.edges,
                &self.loss_edges,
                self.mean_weight,
                syndrome.canonical_detector_values.len(),
                losses,
                work,
            )?;
            self.cache.insert(key.clone(), matching);
            self.cache_order.push_back(key.clone());
            self.cached_work += work;
            self.graph_builds += 1;
        }
        let conditioned = self.cache.get_mut(&key).unwrap();
        if conditioned.detector_count != syndrome.canonical_detector_values.len() {
            return Err(ShotFailure::Other(
                "loss pattern produced an inconsistent detector basis".to_string(),
            ));
        }
        validate_reachable_detectors(conditioned, &syndrome.canonical_detector_values)?;
        let detector_values: Vec<u8> = syndrome
            .canonical_detector_values
            .iter()
            .map(|&value| u8::from(value))
            .collect();
        let bits = conditioned.matching.decode(&detector_values);
        Ok(bits
            .iter()
            .chain(std::iter::repeat(&0))
            .take(self.num_observables)
            .enumerate()
            .filter_map(|(index, &bit)| (bit != 0).then_some(index))
            .collect())
    }

    fn evict_until_fits(&mut self, artifact_work: usize) -> Result<(), ShotFailure> {
        while conditioned_cache_needs_eviction(self.cache.len(), self.cached_work, artifact_work)
            .map_err(ShotFailure::Other)?
        {
            let oldest = self.cache_order.pop_front().ok_or_else(|| {
                ShotFailure::Other("conditioned matching cache accounting drift".to_string())
            })?;
            let removed = self.cache.remove(&oldest).ok_or_else(|| {
                ShotFailure::Other("conditioned matching cache accounting drift".to_string())
            })?;
            self.cached_work = self.cached_work.checked_sub(removed.work).ok_or_else(|| {
                ShotFailure::Other("conditioned matching cache accounting drift".to_string())
            })?;
        }
        Ok(())
    }

    pub(super) fn graph_builds(&self) -> usize {
        self.graph_builds
    }

    pub(super) fn cache_hits(&self) -> usize {
        self.cache_hits
    }
}

fn canonical_matching_preflight(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    detector_count: usize,
    losses: &[usize],
) -> Result<usize, ShotFailure> {
    if edges.len() > MAX_CONDITIONED_DECODER_ITEMS
        || detector_count > MAX_CONDITIONED_DECODER_ITEMS
        || losses.len() > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_matching_limit_error());
    }
    let edge_terms = edges
        .iter()
        .try_fold(0usize, |total, edge| {
            total
                .checked_add(1 + usize::from(edge.node2.is_some()))?
                .checked_add(edge.observables.len())
        })
        .ok_or_else(conditioned_matching_limit_error)?;
    let loss_terms = losses
        .iter()
        .try_fold(0usize, |total, &loss| {
            let mapped = loss_edges.get(loss)?;
            total.checked_add(mapped.len())
        })
        .ok_or_else(|| {
            ShotFailure::Other(
                "loss pattern references an unknown envelope or exceeds the matching work limit"
                    .to_string(),
            )
        })?;
    conditioned_matching_work_from_counts(
        edges.len(),
        detector_count,
        edge_terms,
        0,
        0,
        losses.len(),
        loss_terms,
    )
}

fn build_canonical_matching(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    mean_weight: f64,
    detector_count: usize,
    losses: &[usize],
    artifact_work: usize,
) -> Result<ConditionedMatching, ShotFailure> {
    let mut active = HashSet::new();
    for &loss in losses {
        let mapped_edges = loss_edges.get(loss).ok_or_else(|| {
            ShotFailure::Other(format!("loss pattern references unknown envelope {loss}"))
        })?;
        for &edge in mapped_edges {
            if edge >= edges.len() {
                return Err(ShotFailure::Other(format!(
                    "loss envelope {loss} references unknown matching edge {edge}"
                )));
            }
            active.insert(edge);
        }
    }

    let scale = edges.iter().map(|edge| edge.weight).fold(1.0f64, f64::max);
    let mut matching = Matching::new();
    let mut reachable_detectors = vec![false; detector_count];
    for (index, edge) in edges.iter().enumerate() {
        if edge.node1 >= detector_count || edge.node2.is_some_and(|node| node >= detector_count) {
            return Err(ShotFailure::Other(format!(
                "matching edge {index} references a detector outside the canonical detector basis"
            )));
        }
        let weight = if active.contains(&index) {
            match edge.kind {
                EdgeKind::TimeLike => 0.25 * mean_weight,
                EdgeKind::SpaceLike | EdgeKind::Boundary => 0.5 * mean_weight,
            }
        } else {
            edge.weight
        } / scale;
        let probability = 1.0 / (1.0 + weight.exp());
        reachable_detectors[edge.node1] = true;
        if let Some(node2) = edge.node2 {
            reachable_detectors[node2] = true;
            matching.add_edge(edge.node1, node2, weight, &edge.observables, probability);
        } else {
            matching.add_boundary_edge(edge.node1, weight, &edge.observables, probability);
        }
    }
    matching.prepare();
    Ok(ConditionedMatching {
        matching,
        detector_count,
        reachable_detectors,
        work: artifact_work,
    })
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

#[cfg(test)]
fn build_matching(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    mean_weight: f64,
    checks: &[LossAwareDetectorCheck],
    losses: &[usize],
) -> Result<ConditionedMatching, ShotFailure> {
    let preflight = conditioned_matching_preflight(edges, loss_edges, checks, losses)?;
    build_matching_preflighted(
        edges,
        loss_edges,
        mean_weight,
        checks,
        losses,
        &preflight.detector_checks,
        preflight.work,
    )
}

#[cfg(test)]
fn build_matching_preflighted(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    mean_weight: f64,
    checks: &[LossAwareDetectorCheck],
    losses: &[usize],
    detector_checks: &HashMap<usize, Vec<usize>>,
    artifact_work: usize,
) -> Result<ConditionedMatching, ShotFailure> {
    let mut active = HashSet::new();
    for &loss in losses {
        let mapped_edges = loss_edges.get(loss).ok_or_else(|| {
            ShotFailure::Other(format!("loss pattern references unknown envelope {loss}"))
        })?;
        for &edge in mapped_edges {
            if edge >= edges.len() {
                return Err(ShotFailure::Other(format!(
                    "loss envelope {loss} references unknown matching edge {edge}"
                )));
            }
            active.insert(edge);
        }
    }
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
        let transformed = transformed_check_nodes(edge, detector_checks);
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
        detector_count: checks.len(),
        reachable_detectors: reachable_checks,
        work: artifact_work,
    })
}

#[cfg(test)]
fn conditioned_matching_work(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    checks: &[LossAwareDetectorCheck],
    losses: &[usize],
) -> Result<usize, ShotFailure> {
    conditioned_matching_preflight(edges, loss_edges, checks, losses)
        .map(|preflight| preflight.work)
}

#[cfg(test)]
fn conditioned_matching_preflight(
    edges: &[GraphEdge],
    loss_edges: &[Vec<usize>],
    checks: &[LossAwareDetectorCheck],
    losses: &[usize],
) -> Result<ConditionedMatchingPreflight, ShotFailure> {
    if edges.len() > MAX_CONDITIONED_DECODER_ITEMS
        || checks.len() > MAX_CONDITIONED_DECODER_ITEMS
        || losses.len() > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_matching_limit_error());
    }
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
    let loss_terms = losses
        .iter()
        .try_fold(0usize, |total, &loss| {
            let mapped = loss_edges.get(loss)?;
            total.checked_add(mapped.len())
        })
        .ok_or_else(|| {
            ShotFailure::Other("loss pattern references an unknown envelope or exceeds the conditioned matching work limit".to_string())
        })?;
    conditioned_matching_work_from_counts(
        edges.len(),
        checks.len(),
        edge_terms,
        check_terms,
        0,
        losses.len(),
        loss_terms,
    )?;
    let detector_checks = detector_check_incidence(checks);
    // `transformed_check_nodes` merges both endpoint incidence vectors for
    // every edge, so their lengths are the tight checked upper bound on that
    // phase's iterations and temporary output capacity.
    let edge_incidence_terms = edges
        .iter()
        .try_fold(0usize, |total, edge| {
            let left = detector_checks.get(&edge.node1).map_or(0, Vec::len);
            let right = edge
                .node2
                .and_then(|node| detector_checks.get(&node))
                .map_or(0, Vec::len);
            total.checked_add(left)?.checked_add(right)
        })
        .ok_or_else(conditioned_matching_limit_error)?;
    let work = conditioned_matching_work_from_counts(
        edges.len(),
        checks.len(),
        edge_terms,
        check_terms,
        edge_incidence_terms,
        losses.len(),
        loss_terms,
    )?;
    Ok(ConditionedMatchingPreflight {
        work,
        detector_checks,
    })
}

fn conditioned_matching_work_from_counts(
    edge_count: usize,
    check_count: usize,
    edge_terms: usize,
    check_terms: usize,
    edge_incidence_terms: usize,
    loss_count: usize,
    loss_terms: usize,
) -> Result<usize, ShotFailure> {
    if edge_count > MAX_CONDITIONED_DECODER_ITEMS
        || check_count > MAX_CONDITIONED_DECODER_ITEMS
        || loss_count > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_matching_limit_error());
    }
    let work = edge_count
        .checked_add(check_count)
        .and_then(|value| value.checked_add(edge_terms))
        .and_then(|value| value.checked_add(check_terms))
        .and_then(|value| value.checked_add(edge_incidence_terms))
        .and_then(|value| value.checked_add(loss_count))
        .and_then(|value| value.checked_add(loss_terms))
        .ok_or_else(conditioned_matching_limit_error)?;
    if work > MAX_CONDITIONED_DECODER_WORK {
        return Err(conditioned_matching_limit_error());
    }
    Ok(work)
}

fn conditioned_matching_limit_error() -> ShotFailure {
    ShotFailure::Other("conditioned matching work limit exceeded".to_string())
}

fn validate_reachable_detectors(
    conditioned: &ConditionedMatching,
    detector_values: &[bool],
) -> Result<(), ShotFailure> {
    if conditioned
        .reachable_detectors
        .iter()
        .zip(detector_values)
        .any(|(&reachable, &value)| value && !reachable)
    {
        return Err(ShotFailure::Infeasible);
    }
    Ok(())
}

#[cfg(test)]
fn detector_check_incidence(checks: &[LossAwareDetectorCheck]) -> HashMap<usize, Vec<usize>> {
    let mut incidence = HashMap::<usize, Vec<usize>>::new();
    for (check_index, check) in checks.iter().enumerate() {
        for &detector in &check.source_detectors {
            incidence.entry(detector).or_default().push(check_index);
        }
    }
    incidence
}

#[cfg(test)]
fn transformed_check_nodes(
    edge: &GraphEdge,
    detector_checks: &HashMap<usize, Vec<usize>>,
) -> Vec<usize> {
    let left = detector_checks
        .get(&edge.node1)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let Some(node2) = edge.node2 else {
        return left.to_vec();
    };
    let right = detector_checks
        .get(&node2)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut transformed = Vec::with_capacity(left.len() + right.len());
    let (mut a, mut b) = (0, 0);
    while a < left.len() || b < right.len() {
        match (left.get(a), right.get(b)) {
            (Some(&left_value), Some(&right_value)) if left_value == right_value => {
                a += 1;
                b += 1;
            }
            (Some(&left_value), Some(&right_value)) if left_value < right_value => {
                transformed.push(left_value);
                a += 1;
            }
            (Some(_), Some(&right_value)) => {
                transformed.push(right_value);
                b += 1;
            }
            (Some(&left_value), None) => {
                transformed.push(left_value);
                a += 1;
            }
            (None, Some(&right_value)) => {
                transformed.push(right_value);
                b += 1;
            }
            (None, None) => break,
        }
    }
    transformed
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
        let error =
            conditioned_matching_work_from_counts(1, 1, MAX_CONDITIONED_DECODER_WORK, 0, 0, 0, 0)
                .unwrap_err();
        assert!(matches!(error, ShotFailure::Other(message) if message.contains("work limit")));
        assert!(conditioned_matching_work_from_counts(usize::MAX, 0, 0, 0, 0, 0, 0).is_err());
        assert!(conditioned_matching_work_from_counts(1, 1, 0, usize::MAX, 0, 0, 0).is_err());
        assert!(conditioned_matching_work_from_counts(1, 1, 0, 0, usize::MAX, 0, 0).is_err());
    }

    #[test]
    fn conditioned_matching_preflights_each_edge_endpoint_incidence_scan() {
        let small_edges = [GraphEdge {
            node1: 0,
            node2: Some(1),
            observables: Vec::new(),
            weight: 1.0,
            kind: EdgeKind::SpaceLike,
        }];
        let small_checks = [
            LossAwareDetectorCheck {
                source_detectors: vec![0, 1],
                value: false,
            },
            LossAwareDetectorCheck {
                source_detectors: vec![0, 1],
                value: false,
            },
        ];
        assert_eq!(
            conditioned_matching_work(&small_edges, &[], &small_checks, &[]).unwrap(),
            13
        );

        let edges = (0..501)
            .map(|_| GraphEdge {
                node1: 0,
                node2: Some(1),
                observables: Vec::new(),
                weight: 1.0,
                kind: EdgeKind::SpaceLike,
            })
            .collect::<Vec<_>>();
        let checks = (0..10_000)
            .map(|_| LossAwareDetectorCheck {
                source_detectors: vec![0, 1],
                value: false,
            })
            .collect::<Vec<_>>();

        let error = conditioned_matching_work(&edges, &[], &checks, &[]).unwrap_err();
        assert!(matches!(error, ShotFailure::Other(message) if message.contains("work limit")));
    }

    #[test]
    fn conditioned_matching_preflights_active_loss_edge_work_and_indices() {
        assert!(
            conditioned_matching_work_from_counts(
                1,
                0,
                1,
                0,
                0,
                10_000,
                MAX_CONDITIONED_DECODER_WORK,
            )
            .is_err()
        );
        let error = conditioned_matching_work(&[boundary_edge()], &[], &[], &[0]).unwrap_err();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("unknown envelope")
        ));
        let error = build_matching(&[boundary_edge()], &[vec![1]], 1.0, &[], &[0])
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("unknown matching edge")
        ));
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
            cache_order: VecDeque::new(),
            cached_work: 0,
            graph_builds: 0,
            cache_hits: 0,
        };
        let shot = LossAwareDetectorShot {
            lost_measurements: Vec::new(),
            detector_valid: vec![true, true],
            checks,
            canonical_detector_values: vec![false, true],
        };
        assert!(matches!(
            decoder.decode(&shot, &[]),
            Err(ShotFailure::Infeasible)
        ));
    }
}
