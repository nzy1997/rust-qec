use std::collections::{HashMap, HashSet, VecDeque};

use rmatching::Matching;
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

fn conditioned_matching_work_from_counts(
    edge_count: usize,
    detector_count: usize,
    edge_terms: usize,
    loss_count: usize,
    loss_terms: usize,
) -> Result<usize, ShotFailure> {
    if edge_count > MAX_CONDITIONED_DECODER_ITEMS
        || detector_count > MAX_CONDITIONED_DECODER_ITEMS
        || loss_count > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_matching_limit_error());
    }
    let work = edge_count
        .checked_add(detector_count)
        .and_then(|value| value.checked_add(edge_terms))
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

    fn canonical_shot(detector_values: Vec<bool>) -> LossAwareDetectorShot {
        LossAwareDetectorShot {
            lost_measurements: Vec::new(),
            detector_valid: vec![true; detector_values.len()],
            checks: Vec::new(),
            canonical_detector_values: detector_values,
        }
    }

    fn canonical_decoder() -> CompiledMatching {
        CompiledMatching {
            edges: vec![
                GraphEdge {
                    node1: 0,
                    node2: Some(1),
                    observables: Vec::new(),
                    weight: 2.0,
                    kind: EdgeKind::TimeLike,
                },
                GraphEdge {
                    node1: 1,
                    node2: Some(2),
                    observables: Vec::new(),
                    weight: 3.0,
                    kind: EdgeKind::SpaceLike,
                },
                GraphEdge {
                    node1: 0,
                    node2: None,
                    observables: vec![0],
                    weight: 4.0,
                    kind: EdgeKind::Boundary,
                },
            ],
            loss_edges: vec![vec![0, 1, 2]],
            mean_weight: 3.0,
            num_observables: 1,
            cache: HashMap::new(),
            cache_order: VecDeque::new(),
            cached_work: 0,
            graph_builds: 0,
            cache_hits: 0,
        }
    }

    #[test]
    fn canonical_matching_builds_reuses_and_evicts_graphs() {
        let shot = canonical_shot(vec![false, false, false]);
        let mut decoder = canonical_decoder();

        assert_eq!(decoder.decode(&shot, &[0]).unwrap(), Vec::<usize>::new());
        assert_eq!(decoder.decode(&shot, &[0]).unwrap(), Vec::<usize>::new());
        assert_eq!(decoder.graph_builds(), 1);
        assert_eq!(decoder.cache_hits(), 1);

        decoder.cache.get_mut(&vec![0]).unwrap().work = MAX_CONDITIONED_DECODER_WORK;
        decoder.cached_work = MAX_CONDITIONED_DECODER_WORK;
        assert_eq!(decoder.decode(&shot, &[]).unwrap(), Vec::<usize>::new());
        assert!(!decoder.cache.contains_key(&vec![0]));
        assert!(decoder.cache.contains_key(&Vec::new()));
        assert_eq!(decoder.graph_builds(), 2);
        assert_eq!(decoder.cached_work, decoder.cache[&Vec::new()].work);

        let inconsistent = canonical_shot(vec![false, false]);
        let error = decoder.decode(&inconsistent, &[]).unwrap_err();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("inconsistent detector basis")
        ));
    }

    #[test]
    fn canonical_matching_preflight_and_build_reject_malformed_inputs() {
        let error = canonical_matching_preflight(&[boundary_edge()], &[], 1, &[0]).unwrap_err();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("unknown envelope")
        ));
        assert!(
            canonical_matching_preflight(
                &[boundary_edge()],
                &[],
                MAX_CONDITIONED_DECODER_ITEMS + 1,
                &[],
            )
            .is_err()
        );
        assert!(conditioned_matching_work_from_counts(1, 1, usize::MAX, 0, 0).is_err());
        assert!(
            conditioned_matching_work_from_counts(1, 1, MAX_CONDITIONED_DECODER_WORK, 0, 0,)
                .is_err()
        );
        assert!(
            conditioned_matching_work_from_counts(MAX_CONDITIONED_DECODER_ITEMS + 1, 0, 0, 0, 0,)
                .is_err()
        );

        let error = build_canonical_matching(&[boundary_edge()], &[], 1.0, 1, &[0], 1)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("unknown envelope")
        ));
        let error = build_canonical_matching(&[boundary_edge()], &[vec![1]], 1.0, 1, &[0], 1)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("unknown matching edge")
        ));
        let mut outside = boundary_edge();
        outside.node1 = 1;
        let error = build_canonical_matching(&[outside], &[], 1.0, 1, &[], 1)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ShotFailure::Other(message) if message.contains("outside the canonical detector basis")
        ));

        let shot = canonical_shot(vec![false]);
        let mut unknown_loss_decoder = canonical_decoder();
        assert!(matches!(
            unknown_loss_decoder.decode(&shot, &[1]),
            Err(ShotFailure::Other(message)) if message.contains("unknown envelope")
        ));
        let mut outside_decoder = canonical_decoder();
        assert!(matches!(
            outside_decoder.decode(&shot, &[]),
            Err(ShotFailure::Other(message)) if message.contains("outside the canonical detector basis")
        ));
    }

    #[test]
    fn canonical_matching_reports_cache_accounting_drift() {
        let mut missing_order = canonical_decoder();
        missing_order.cached_work = MAX_CONDITIONED_DECODER_WORK;
        assert!(matches!(
            missing_order.evict_until_fits(1),
            Err(ShotFailure::Other(message)) if message.contains("accounting drift")
        ));

        let mut missing_entry = canonical_decoder();
        missing_entry.cached_work = MAX_CONDITIONED_DECODER_WORK;
        missing_entry.cache_order.push_back(vec![0]);
        assert!(matches!(
            missing_entry.evict_until_fits(1),
            Err(ShotFailure::Other(message)) if message.contains("accounting drift")
        ));

        let mut underflow = canonical_decoder();
        let model = build_canonical_matching(&[boundary_edge()], &[], 1.0, 1, &[], 2).unwrap();
        underflow.cache.insert(vec![0], model);
        underflow.cache_order.push_back(vec![0]);
        underflow.cached_work = 1;
        assert!(matches!(
            underflow.evict_until_fits(MAX_CONDITIONED_DECODER_WORK),
            Err(ShotFailure::Other(message)) if message.contains("accounting drift")
        ));
    }

    #[test]
    fn canonical_matching_marks_fired_unreachable_detectors_infeasible() {
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
        let shot = canonical_shot(vec![false, true]);
        assert!(matches!(
            decoder.decode(&shot, &[]),
            Err(ShotFailure::Infeasible)
        ));
    }
}
