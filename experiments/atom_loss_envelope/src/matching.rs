use std::collections::{BTreeMap, HashMap, HashSet};

use rmatching::Matching;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MATCHING_INPUT_SCHEMA_VERSION: &str = "atom-loss-envelope-matching.v0";
pub const MATCHING_RESULT_SCHEMA_VERSION: &str = "atom-loss-envelope-matching-result.v0";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeMatchingCase {
    pub schema_version: String,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub edges: Vec<EnvelopeMatchingEdge>,
    pub loss_edge_map: Vec<LossEdgeMap>,
    pub shots: Vec<EnvelopeMatchingShot>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeMatchingEdge {
    pub id: String,
    pub node1: usize,
    pub node2: Option<usize>,
    pub observable_indices: Vec<usize>,
    pub weight: f64,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    TimeLike,
    SpaceLike,
    Boundary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LossEdgeMap {
    pub loss_id: String,
    pub edge_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeMatchingShot {
    pub observed_detectors: Vec<usize>,
    pub observed_losses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvelopeMatchingResult {
    pub schema_version: &'static str,
    pub backend: &'static str,
    pub predictions: Vec<u64>,
    pub compiled_loss_configurations: usize,
}

#[derive(Debug, Error, PartialEq)]
pub enum EnvelopeMatchingError {
    #[error("unsupported schema_version {actual:?}; expected {expected:?}")]
    UnsupportedSchema {
        expected: &'static str,
        actual: String,
    },
    #[error("num_observables must be at most 64, got {0}")]
    UnsupportedObservableCount(usize),
    #[error("matching graph must contain at least one edge")]
    EmptyGraph,
    #[error("matching input must contain at least one shot")]
    EmptyShots,
    #[error("{kind} ID must not be empty")]
    EmptyId { kind: &'static str },
    #[error("duplicate {kind} ID {id:?}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("edge {edge_id:?} endpoint {endpoint} is outside num_detectors={num_detectors}")]
    EndpointOutOfRange {
        edge_id: String,
        endpoint: usize,
        num_detectors: usize,
    },
    #[error(
        "edge {edge_id:?} references observable {index}, but num_observables is {num_observables}"
    )]
    ObservableOutOfRange {
        edge_id: String,
        index: usize,
        num_observables: usize,
    },
    #[error("edge {edge_id:?} weight must be finite and non-negative, got {weight}")]
    InvalidWeight { edge_id: String, weight: f64 },
    #[error("edge {edge_id:?} must be kind=boundary exactly when node2 is null")]
    InvalidBoundaryKind { edge_id: String },
    #[error("loss {loss_id:?} references missing edge ID {edge_id:?}")]
    MissingEdge { loss_id: String, edge_id: String },
    #[error("loss {loss_id:?} contains duplicate edge ID {edge_id:?}")]
    DuplicateMappedEdge { loss_id: String, edge_id: String },
    #[error("shot {shot} references detector {index}, but num_detectors is {num_detectors}")]
    DetectorOutOfRange {
        shot: usize,
        index: usize,
        num_detectors: usize,
    },
    #[error("shot {shot} contains duplicate detector {index}")]
    DuplicateDetector { shot: usize, index: usize },
    #[error("shot {shot} references missing loss ID {loss_id:?}")]
    MissingLoss { shot: usize, loss_id: String },
    #[error("shot {shot} contains duplicate loss ID {loss_id:?}")]
    DuplicateLoss { shot: usize, loss_id: String },
    #[error(
        "shot {shot} has odd detection parity in a boundaryless graph component containing detector {detector}"
    )]
    UnmatchableSyndrome { shot: usize, detector: usize },
}

pub fn decode_matching(
    case: &EnvelopeMatchingCase,
) -> Result<EnvelopeMatchingResult, EnvelopeMatchingError> {
    let validated = validate(case)?;
    let mut groups: BTreeMap<Vec<String>, Vec<(usize, Vec<u8>)>> = BTreeMap::new();

    for (shot_index, shot) in case.shots.iter().enumerate() {
        let mut loss_key = shot.observed_losses.clone();
        loss_key.sort();
        let mut syndrome = vec![0; case.num_detectors];
        for &detector in &shot.observed_detectors {
            syndrome[detector] = 1;
        }
        groups
            .entry(loss_key)
            .or_default()
            .push((shot_index, syndrome));
    }

    let compiled_loss_configurations = groups.len();
    let mut predictions = vec![0; case.shots.len()];
    for (loss_ids, shots) in groups {
        let active_edges: HashSet<usize> = loss_ids
            .iter()
            .flat_map(|loss_id| validated.loss_edges[loss_id].iter().copied())
            .collect();
        let mut matching = build_matching(case, validated.mean_weight, &active_edges);
        let syndromes: Vec<Vec<u8>> = shots.iter().map(|(_, syndrome)| syndrome.clone()).collect();
        for ((shot_index, _), bits) in shots.into_iter().zip(matching.decode_batch(&syndromes)) {
            predictions[shot_index] = bits_to_mask(&bits);
        }
    }

    Ok(EnvelopeMatchingResult {
        schema_version: MATCHING_RESULT_SCHEMA_VERSION,
        backend: "rmatching",
        predictions,
        compiled_loss_configurations,
    })
}

struct ValidatedCase {
    mean_weight: f64,
    loss_edges: HashMap<String, Vec<usize>>,
}

fn validate(case: &EnvelopeMatchingCase) -> Result<ValidatedCase, EnvelopeMatchingError> {
    if case.schema_version != MATCHING_INPUT_SCHEMA_VERSION {
        return Err(EnvelopeMatchingError::UnsupportedSchema {
            expected: MATCHING_INPUT_SCHEMA_VERSION,
            actual: case.schema_version.clone(),
        });
    }
    if case.num_observables > 64 {
        return Err(EnvelopeMatchingError::UnsupportedObservableCount(
            case.num_observables,
        ));
    }
    if case.edges.is_empty() {
        return Err(EnvelopeMatchingError::EmptyGraph);
    }
    if case.shots.is_empty() {
        return Err(EnvelopeMatchingError::EmptyShots);
    }

    let mut edge_ids = HashMap::new();
    let mut components = Components::new(case.num_detectors);
    let mut boundary_detectors = Vec::new();
    for (edge_index, edge) in case.edges.iter().enumerate() {
        validate_id("edge", &edge.id)?;
        if edge_ids.insert(edge.id.clone(), edge_index).is_some() {
            return Err(EnvelopeMatchingError::DuplicateId {
                kind: "edge",
                id: edge.id.clone(),
            });
        }
        for endpoint in [Some(edge.node1), edge.node2].into_iter().flatten() {
            if endpoint >= case.num_detectors {
                return Err(EnvelopeMatchingError::EndpointOutOfRange {
                    edge_id: edge.id.clone(),
                    endpoint,
                    num_detectors: case.num_detectors,
                });
            }
        }
        if !edge.weight.is_finite() || edge.weight < 0.0 {
            return Err(EnvelopeMatchingError::InvalidWeight {
                edge_id: edge.id.clone(),
                weight: edge.weight,
            });
        }
        if (edge.node2.is_none()) != (edge.kind == EdgeKind::Boundary) {
            return Err(EnvelopeMatchingError::InvalidBoundaryKind {
                edge_id: edge.id.clone(),
            });
        }
        if let Some(node2) = edge.node2 {
            components.union(edge.node1, node2);
        } else {
            boundary_detectors.push(edge.node1);
        }
        for &observable in &edge.observable_indices {
            if observable >= case.num_observables {
                return Err(EnvelopeMatchingError::ObservableOutOfRange {
                    edge_id: edge.id.clone(),
                    index: observable,
                    num_observables: case.num_observables,
                });
            }
        }
    }

    let component_roots: Vec<usize> = (0..case.num_detectors)
        .map(|detector| components.find(detector))
        .collect();
    let boundary_components: HashSet<usize> = boundary_detectors
        .into_iter()
        .map(|detector| component_roots[detector])
        .collect();

    let mut loss_edges = HashMap::new();
    for mapping in &case.loss_edge_map {
        validate_id("loss", &mapping.loss_id)?;
        let mut mapped_ids = HashSet::new();
        let mut indices = Vec::with_capacity(mapping.edge_ids.len());
        for edge_id in &mapping.edge_ids {
            if !mapped_ids.insert(edge_id) {
                return Err(EnvelopeMatchingError::DuplicateMappedEdge {
                    loss_id: mapping.loss_id.clone(),
                    edge_id: edge_id.clone(),
                });
            }
            let Some(&edge_index) = edge_ids.get(edge_id) else {
                return Err(EnvelopeMatchingError::MissingEdge {
                    loss_id: mapping.loss_id.clone(),
                    edge_id: edge_id.clone(),
                });
            };
            indices.push(edge_index);
        }
        if loss_edges
            .insert(mapping.loss_id.clone(), indices)
            .is_some()
        {
            return Err(EnvelopeMatchingError::DuplicateId {
                kind: "loss",
                id: mapping.loss_id.clone(),
            });
        }
    }

    for (shot_index, shot) in case.shots.iter().enumerate() {
        let mut detectors = HashSet::new();
        for &detector in &shot.observed_detectors {
            if detector >= case.num_detectors {
                return Err(EnvelopeMatchingError::DetectorOutOfRange {
                    shot: shot_index,
                    index: detector,
                    num_detectors: case.num_detectors,
                });
            }
            if !detectors.insert(detector) {
                return Err(EnvelopeMatchingError::DuplicateDetector {
                    shot: shot_index,
                    index: detector,
                });
            }
        }
        let mut losses = HashSet::new();
        for loss_id in &shot.observed_losses {
            if !loss_edges.contains_key(loss_id) {
                return Err(EnvelopeMatchingError::MissingLoss {
                    shot: shot_index,
                    loss_id: loss_id.clone(),
                });
            }
            if !losses.insert(loss_id) {
                return Err(EnvelopeMatchingError::DuplicateLoss {
                    shot: shot_index,
                    loss_id: loss_id.clone(),
                });
            }
        }

        let mut component_parity = HashMap::<usize, (bool, usize)>::new();
        for &detector in &shot.observed_detectors {
            let root = component_roots[detector];
            let entry = component_parity.entry(root).or_insert((false, detector));
            entry.0 ^= true;
        }
        if let Some((_, &(_, detector))) = component_parity
            .iter()
            .find(|(root, (odd, _))| *odd && !boundary_components.contains(root))
        {
            return Err(EnvelopeMatchingError::UnmatchableSyndrome {
                shot: shot_index,
                detector,
            });
        }
    }

    let edge_count = case.edges.len() as f64;
    let mean_weight = case.edges.iter().map(|edge| edge.weight / edge_count).sum();
    Ok(ValidatedCase {
        mean_weight,
        loss_edges,
    })
}

struct Components {
    parent: Vec<usize>,
}

impl Components {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, node: usize) -> usize {
        let mut root = node;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = node;
        while self.parent[current] != current {
            let next = self.parent[current];
            self.parent[current] = root;
            current = next;
        }
        root
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root != right_root {
            self.parent[right_root] = left_root;
        }
    }
}

fn validate_id(kind: &'static str, id: &str) -> Result<(), EnvelopeMatchingError> {
    if id.is_empty() {
        Err(EnvelopeMatchingError::EmptyId { kind })
    } else {
        Ok(())
    }
}

fn build_matching(
    case: &EnvelopeMatchingCase,
    mean_weight: f64,
    active_edges: &HashSet<usize>,
) -> Matching {
    let mut matching = Matching::new();
    let weight_scale = case
        .edges
        .iter()
        .map(|edge| edge.weight)
        .fold(0.0, f64::max)
        .max(1.0);
    for (edge_index, edge) in case.edges.iter().enumerate() {
        let weight =
            effective_weight(edge, mean_weight, active_edges.contains(&edge_index)) / weight_scale;
        let probability = 1.0 / (1.0 + weight.exp());
        if let Some(node2) = edge.node2 {
            matching.add_edge(
                edge.node1,
                node2,
                weight,
                &edge.observable_indices,
                probability,
            );
        } else {
            matching.add_boundary_edge(edge.node1, weight, &edge.observable_indices, probability);
        }
    }
    matching.prepare();
    matching
}

fn effective_weight(edge: &EnvelopeMatchingEdge, mean_weight: f64, active: bool) -> f64 {
    if !active {
        return edge.weight;
    }
    match edge.kind {
        EdgeKind::TimeLike => 0.25 * mean_weight,
        EdgeKind::SpaceLike | EdgeKind::Boundary => 0.5 * mean_weight,
    }
}

fn bits_to_mask(bits: &[u8]) -> u64 {
    bits.iter().enumerate().fold(0, |mask, (index, bit)| {
        mask | (u64::from(*bit != 0) << index)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known_answer() -> EnvelopeMatchingCase {
        EnvelopeMatchingCase {
            schema_version: MATCHING_INPUT_SCHEMA_VERSION.to_string(),
            num_detectors: 1,
            num_observables: 1,
            edges: vec![
                EnvelopeMatchingEdge {
                    id: "base".to_string(),
                    node1: 0,
                    node2: None,
                    observable_indices: vec![],
                    weight: 2.0,
                    kind: EdgeKind::Boundary,
                },
                EnvelopeMatchingEdge {
                    id: "loss-compatible".to_string(),
                    node1: 0,
                    node2: None,
                    observable_indices: vec![0],
                    weight: 4.0,
                    kind: EdgeKind::Boundary,
                },
            ],
            loss_edge_map: vec![LossEdgeMap {
                loss_id: "loss-0".to_string(),
                edge_ids: vec!["loss-compatible".to_string()],
            }],
            shots: vec![
                EnvelopeMatchingShot {
                    observed_detectors: vec![0],
                    observed_losses: vec![],
                },
                EnvelopeMatchingShot {
                    observed_detectors: vec![0],
                    observed_losses: vec!["loss-0".to_string()],
                },
                EnvelopeMatchingShot {
                    observed_detectors: vec![0],
                    observed_losses: vec!["loss-0".to_string()],
                },
            ],
        }
    }

    #[test]
    fn loss_rescaling_changes_the_known_answer_and_groups_shots() {
        let result = decode_matching(&known_answer()).unwrap();
        assert_eq!(result.predictions, vec![0, 1, 1]);
        assert_eq!(result.compiled_loss_configurations, 2);
    }

    #[test]
    fn dangling_edge_map_is_rejected() {
        let mut case = known_answer();
        case.loss_edge_map[0].edge_ids[0] = "missing-edge".to_string();
        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::MissingEdge {
                loss_id: "loss-0".to_string(),
                edge_id: "missing-edge".to_string(),
            })
        );
    }

    #[test]
    fn active_edge_classes_use_the_fixed_envelope_factors() {
        let mut edge = known_answer().edges.remove(0);
        edge.node2 = Some(0);
        edge.kind = EdgeKind::TimeLike;
        assert_eq!(effective_weight(&edge, 8.0, true), 2.0);
        edge.kind = EdgeKind::SpaceLike;
        assert_eq!(effective_weight(&edge, 8.0, true), 4.0);
        edge.node2 = None;
        edge.kind = EdgeKind::Boundary;
        assert_eq!(effective_weight(&edge, 8.0, true), 4.0);
        assert_eq!(effective_weight(&edge, 8.0, false), edge.weight);
    }

    #[test]
    fn empty_shot_list_is_rejected() {
        let mut case = known_answer();
        case.shots.clear();
        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::EmptyShots)
        );
    }

    #[test]
    fn odd_syndrome_in_boundaryless_component_is_rejected() {
        let mut case = known_answer();
        case.num_detectors = 2;
        case.edges = vec![EnvelopeMatchingEdge {
            id: "internal".to_string(),
            node1: 0,
            node2: Some(1),
            observable_indices: vec![],
            weight: 1.0,
            kind: EdgeKind::SpaceLike,
        }];
        case.loss_edge_map.clear();
        case.shots = vec![EnvelopeMatchingShot {
            observed_detectors: vec![0],
            observed_losses: vec![],
        }];

        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::UnmatchableSyndrome {
                shot: 0,
                detector: 0,
            })
        );
    }

    #[test]
    fn reordered_overlapping_losses_share_one_configuration() {
        let mut case = known_answer();
        case.loss_edge_map.push(LossEdgeMap {
            loss_id: "loss-1".to_string(),
            edge_ids: vec!["loss-compatible".to_string()],
        });
        case.shots = vec![
            EnvelopeMatchingShot {
                observed_detectors: vec![0],
                observed_losses: vec!["loss-0".to_string(), "loss-1".to_string()],
            },
            EnvelopeMatchingShot {
                observed_detectors: vec![0],
                observed_losses: vec!["loss-1".to_string(), "loss-0".to_string()],
            },
        ];

        let result = decode_matching(&case).unwrap();
        assert_eq!(result.predictions, vec![1, 1]);
        assert_eq!(result.compiled_loss_configurations, 1);
    }

    #[test]
    fn zero_observables_are_supported() {
        let mut case = known_answer();
        case.num_observables = 0;
        case.edges[1].observable_indices.clear();
        let result = decode_matching(&case).unwrap();
        assert_eq!(result.predictions, vec![0, 0, 0]);
    }

    #[test]
    fn schema_and_dangling_references_are_rejected() {
        let mut case = known_answer();
        case.schema_version = "future".to_string();
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::UnsupportedSchema { .. })
        ));

        case = known_answer();
        case.edges[0].node1 = 1;
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::EndpointOutOfRange { endpoint: 1, .. })
        ));

        case = known_answer();
        case.edges[1].observable_indices = vec![1];
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::ObservableOutOfRange { index: 1, .. })
        ));

        case = known_answer();
        case.shots[0].observed_losses = vec!["missing-loss".to_string()];
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::MissingLoss { .. })
        ));

        case = known_answer();
        case.shots[0].observed_detectors = vec![1];
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::DetectorOutOfRange { index: 1, .. })
        ));
    }

    #[test]
    fn remaining_validation_failures_are_reported() {
        let mut case = known_answer();
        case.num_observables = 65;
        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::UnsupportedObservableCount(65))
        );

        case = known_answer();
        case.edges.clear();
        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::EmptyGraph)
        );

        case = known_answer();
        case.edges[0].id.clear();
        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::EmptyId { kind: "edge" })
        );

        case = known_answer();
        case.edges[1].id = case.edges[0].id.clone();
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::DuplicateId { kind: "edge", .. })
        ));

        case = known_answer();
        case.edges[0].weight = f64::INFINITY;
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::InvalidWeight { .. })
        ));

        case = known_answer();
        case.edges[0].node2 = Some(0);
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::InvalidBoundaryKind { .. })
        ));

        case = known_answer();
        case.loss_edge_map[0].loss_id.clear();
        assert_eq!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::EmptyId { kind: "loss" })
        );

        case = known_answer();
        case.loss_edge_map[0]
            .edge_ids
            .push("loss-compatible".to_string());
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::DuplicateMappedEdge { .. })
        ));

        case = known_answer();
        case.loss_edge_map.push(case.loss_edge_map[0].clone());
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::DuplicateId { kind: "loss", .. })
        ));

        case = known_answer();
        case.shots[0].observed_detectors = vec![0, 0];
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::DuplicateDetector { .. })
        ));

        case = known_answer();
        case.shots[0].observed_losses = vec!["loss-0".to_string(), "loss-0".to_string()];
        assert!(matches!(
            decode_matching(&case),
            Err(EnvelopeMatchingError::DuplicateLoss { .. })
        ));
    }

    #[test]
    fn even_syndrome_in_boundaryless_component_is_matchable() {
        let mut case = known_answer();
        case.num_detectors = 2;
        case.num_observables = 0;
        case.edges = vec![EnvelopeMatchingEdge {
            id: "internal".to_string(),
            node1: 0,
            node2: Some(1),
            observable_indices: vec![],
            weight: 1.0,
            kind: EdgeKind::SpaceLike,
        }];
        case.loss_edge_map.clear();
        case.shots = vec![EnvelopeMatchingShot {
            observed_detectors: vec![0, 1],
            observed_losses: vec![],
        }];

        let result = decode_matching(&case).unwrap();
        assert_eq!(result.predictions, vec![0]);
    }
}
