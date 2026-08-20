use std::collections::HashSet;
#[cfg(test)]
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rstim::measurement_transform::CheckedMeasurementLayout;
use rstim::output::{OutputFormat, write_shots_b8};
use rstim::result_stream::ResultBlockReader;
use rstim::sim::bit_table::BitTable;
use serde::Serialize;

mod compiler;
mod dataset;
mod matching;
mod mle;

use compiler::compile_circuit;
#[cfg(test)]
use compiler::normalize_supported_circuit;
#[cfg(test)]
use dataset::{CircuitManifest, FileManifest, PublicManifest, RowManifest, sha256_hex};
use dataset::{Dataset, read_dataset};
#[cfg(test)]
use matching::EdgeKind;
#[cfg(test)]
use matching::validate_unambiguous_parallel_edges;
use matching::{CompiledMatching, GraphEdge};
use mle::CompiledMle;

pub const COMMAND: &str = "decode";
pub const STATS_SCHEMA_VERSION: &str = "rustqec.decode-stats.v1";
const BATCH_SIZE: usize = 1024;
const MAX_ENVELOPE_CANDIDATES: usize = 100_000;
const MAX_PRIMITIVE_PROBES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderKind {
    EnvelopeMatching,
    EnvelopeMle,
}

impl DecoderKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::EnvelopeMatching => "envelope-matching",
            Self::EnvelopeMle => "envelope-mle",
        }
    }
}

#[derive(Debug)]
pub struct DecodeOptions {
    pub decoder: DecoderKind,
    pub dataset: PathBuf,
    pub predictions_out: PathBuf,
    pub stats_out: PathBuf,
    pub shot_timeout_ms: Option<u64>,
}

#[derive(Debug)]
pub struct DecodeFailure {
    pub code: &'static str,
    pub message: String,
}

impl DecodeFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct DecodeStats {
    pub schema_version: &'static str,
    pub decoder: String,
    pub circuit_sha256: String,
    pub shot_count: usize,
    pub attempted_shot_count: usize,
    pub compile_seconds: f64,
    pub decode_seconds: f64,
    pub distinct_loss_patterns: usize,
    pub cache_hits: usize,
    pub timeout_count: usize,
    pub infeasible_shot_count: usize,
    pub circuit_compilations: usize,
    pub matching_graph_builds: usize,
    pub mle_model_builds: usize,
}

#[derive(Clone, Debug)]
struct Effect {
    id: String,
    detectors: Vec<usize>,
    observables: Vec<usize>,
    weight: f64,
}

#[derive(Clone, Debug)]
struct LossEnvelope {
    id: String,
    candidates: Vec<Effect>,
}

struct CompiledCircuit {
    layout: CheckedMeasurementLayout,
    reference: Vec<bool>,
    loss_flags: Vec<usize>,
    independent_effects: Vec<Effect>,
    envelopes: Vec<LossEnvelope>,
    graph_edges: Vec<GraphEdge>,
    loss_edges: Vec<Vec<usize>>,
    unmapped_loss_primitives: Vec<Vec<String>>,
    num_detectors: usize,
    num_observables: usize,
}

pub fn run(options: &DecodeOptions) -> Result<DecodeStats, DecodeFailure> {
    validate_output_paths(options)?;
    let dataset = read_dataset(&options.dataset)?;
    let compile_started = Instant::now();
    let circuit = compile_circuit(&dataset)?;
    let mut decoder = match options.decoder {
        DecoderKind::EnvelopeMatching => DecoderState::Matching(CompiledMatching::new(&circuit)?),
        DecoderKind::EnvelopeMle => {
            DecoderState::Mle(CompiledMle::new(&circuit, options.shot_timeout_ms)?)
        }
    };
    let compile_seconds = compile_started.elapsed().as_secs_f64();

    let prediction_parent = parent_dir(&options.predictions_out);
    let stats_parent = parent_dir(&options.stats_out);
    fs::create_dir_all(prediction_parent).map_err(|error| {
        DecodeFailure::new(
            "output_error",
            format!("failed to create {}: {error}", prediction_parent.display()),
        )
    })?;
    fs::create_dir_all(stats_parent).map_err(|error| {
        DecodeFailure::new(
            "output_error",
            format!("failed to create {}: {error}", stats_parent.display()),
        )
    })?;
    let mut prediction_temp = tempfile::NamedTempFile::new_in(prediction_parent)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    let mut writer = BufWriter::new(prediction_temp.as_file_mut());
    let shots = File::open(&dataset.shots_path).map_err(|error| {
        DecodeFailure::new(
            "missing_dataset_file",
            format!("failed to open {}: {error}", dataset.shots_path.display()),
        )
    })?;
    let mut reader = ResultBlockReader::new(
        BufReader::new(shots),
        dataset.manifest.row.bits,
        dataset.manifest.shots as u64,
        OutputFormat::B8,
        BATCH_SIZE,
    )
    .map_err(|error| DecodeFailure::new("invalid_dataset", error.to_string()))?;
    let decode_started = Instant::now();
    let mut patterns = HashSet::new();
    let mut shot_offset = 0usize;
    while let Some(measurements) = reader
        .next_block()
        .map_err(|error| DecodeFailure::new("invalid_dataset", error.to_string()))?
    {
        let syndromes = circuit.syndromes(&measurements)?;
        let losses = circuit.loss_patterns(&measurements);
        let mut predictions = BitTable::try_new(circuit.num_observables, measurements.num_minor())
            .map_err(|error| DecodeFailure::new("decode_error", format!("{error:?}")))?;
        for shot in 0..measurements.num_minor() {
            patterns.insert(losses[shot].clone());
            match decoder.decode(&syndromes[shot], &losses[shot]) {
                Ok(bits) => {
                    for observable in bits {
                        predictions.set(observable, shot, true);
                    }
                }
                Err(ShotFailure::Timeout) => {
                    let stats = build_stats(
                        options,
                        &dataset,
                        compile_seconds,
                        decode_started.elapsed().as_secs_f64(),
                        &patterns,
                        &decoder,
                        shot_offset + shot + 1,
                        1,
                        0,
                    );
                    persist_stats_atomic(&options.stats_out, stats_parent, &stats)?;
                    return Err(DecodeFailure::new(
                        "decode_timeout",
                        format!("envelope-mle timed out at shot {}", shot_offset + shot),
                    ));
                }
                Err(ShotFailure::Infeasible) => {
                    let stats = build_stats(
                        options,
                        &dataset,
                        compile_seconds,
                        decode_started.elapsed().as_secs_f64(),
                        &patterns,
                        &decoder,
                        shot_offset + shot + 1,
                        0,
                        1,
                    );
                    persist_stats_atomic(&options.stats_out, stats_parent, &stats)?;
                    return Err(DecodeFailure::new(
                        "decode_infeasible",
                        format!("shot {} has no feasible correction", shot_offset + shot),
                    ));
                }
                Err(ShotFailure::Other(message)) => {
                    return Err(DecodeFailure::new("decode_error", message));
                }
            }
        }
        write_shots_b8(&predictions, &mut writer)
            .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
        shot_offset += measurements.num_minor();
    }
    writer
        .flush()
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    drop(writer);
    let decode_seconds = decode_started.elapsed().as_secs_f64();
    let stats = build_stats(
        options,
        &dataset,
        compile_seconds,
        decode_seconds,
        &patterns,
        &decoder,
        dataset.manifest.shots,
        0,
        0,
    );

    let mut stats_temp = tempfile::NamedTempFile::new_in(stats_parent)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    serde_json::to_writer_pretty(stats_temp.as_file_mut(), &stats)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    stats_temp
        .as_file_mut()
        .write_all(b"\n")
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    stats_temp
        .as_file_mut()
        .flush()
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    prediction_temp
        .persist(&options.predictions_out)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    if let Err(error) = stats_temp.persist(&options.stats_out) {
        let _ = fs::remove_file(&options.predictions_out);
        return Err(DecodeFailure::new("output_error", error.to_string()));
    }
    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
fn build_stats(
    options: &DecodeOptions,
    dataset: &Dataset,
    compile_seconds: f64,
    decode_seconds: f64,
    patterns: &HashSet<Vec<usize>>,
    decoder: &DecoderState,
    attempted_shots: usize,
    timeout_count: usize,
    infeasible_shot_count: usize,
) -> DecodeStats {
    DecodeStats {
        schema_version: STATS_SCHEMA_VERSION,
        decoder: options.decoder.name().to_string(),
        circuit_sha256: dataset.manifest.circuit.sha256.clone(),
        shot_count: dataset.manifest.shots,
        attempted_shot_count: attempted_shots,
        compile_seconds,
        decode_seconds,
        distinct_loss_patterns: patterns.len(),
        cache_hits: if matches!(decoder, DecoderState::Matching(_)) {
            attempted_shots.saturating_sub(patterns.len())
        } else {
            0
        },
        timeout_count,
        infeasible_shot_count,
        circuit_compilations: 1,
        matching_graph_builds: decoder.graph_builds(),
        mle_model_builds: usize::from(matches!(decoder, DecoderState::Mle(_))),
    }
}

fn persist_stats_atomic(
    path: &Path,
    parent: &Path,
    stats: &DecodeStats,
) -> Result<(), DecodeFailure> {
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    serde_json::to_writer_pretty(temp.as_file_mut(), stats)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    temp.as_file_mut()
        .write_all(b"\n")
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    temp.as_file_mut()
        .flush()
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    temp.persist(path)
        .map_err(|error| DecodeFailure::new("output_error", error.to_string()))?;
    Ok(())
}

fn validate_output_paths(options: &DecodeOptions) -> Result<(), DecodeFailure> {
    if options.decoder == DecoderKind::EnvelopeMatching && options.shot_timeout_ms.is_some() {
        return Err(DecodeFailure::new(
            "invalid_arguments",
            "--shot-timeout-ms is supported only by envelope-mle",
        ));
    }
    if options.predictions_out == options.stats_out {
        return Err(DecodeFailure::new(
            "invalid_arguments",
            "--out and --stats-out must be different paths",
        ));
    }
    for path in [&options.predictions_out, &options.stats_out] {
        if path.exists() {
            return Err(DecodeFailure::new(
                "output_error",
                format!("output already exists: {}", path.display()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
enum ShotFailure {
    Timeout,
    Infeasible,
    Other(String),
}

enum DecoderState {
    Matching(CompiledMatching),
    Mle(CompiledMle),
}

impl DecoderState {
    fn decode(&mut self, syndrome: &[u8], losses: &[usize]) -> Result<Vec<usize>, ShotFailure> {
        match self {
            Self::Matching(decoder) => decoder.decode(syndrome, losses),
            Self::Mle(decoder) => decoder.decode(syndrome, losses),
        }
    }

    fn graph_builds(&self) -> usize {
        match self {
            Self::Matching(decoder) => decoder.cache.len(),
            Self::Mle(_) => 0,
        }
    }
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

#[cfg(test)]
mod tests {
    use atom_loss_envelope::{
        AtomLossCase, DecodeOutcome, EdgeKind as ReferenceEdgeKind, Effect as ReferenceEffect,
        EnvelopeMatchingCase, EnvelopeMatchingEdge, EnvelopeMatchingShot, LossEdgeMap,
        LossEnvelope as ReferenceLossEnvelope, decode as decode_reference_mle,
        decode_matching as decode_reference_matching,
    };

    use super::*;

    const PERSISTENT_CIRCUIT: &str = concat!(
        "QUBIT_COORDS(0,0) 0\n",
        "QUBIT_COORDS(1,0) 1\n",
        "R 0 1\n",
        "X_ERROR(0.1) 0\n",
        "X_ERROR(0.01) 1\n",
        "LOSS(0.1) 0\n",
        "H 0\n",
        "H 0\n",
        "CX 0 1\n",
        "X_ERROR(0.02) 0\n",
        "LOSS(0.1) 1\n",
        "ML 0 1\n",
        "DETECTOR(0,0,0) rec[-3]\n",
        "DETECTOR(1,0,0) rec[-1]\n",
        "OBSERVABLE_INCLUDE(0) rec[-3]\n",
    );

    fn dataset_for(circuit_text: &str) -> Dataset {
        let instrs = rstim::validation::parse_and_validate(circuit_text).unwrap();
        let stats = rstim::stats::summarize(&instrs);
        Dataset {
            manifest: PublicManifest {
                schema_version: 1,
                dataset_id: None,
                mode: "measurements_blinded".to_string(),
                shots: 0,
                row: RowManifest {
                    kind: "measurements".to_string(),
                    bits: stats.num_measurements,
                    encoding: "b8".to_string(),
                    bit_order: "lsb_first".to_string(),
                    bytes_per_shot: stats.num_measurements.div_ceil(8),
                },
                circuit: CircuitManifest {
                    file: "circuit.stim".to_string(),
                    sha256: sha256_hex(circuit_text.as_bytes()),
                    measurements: stats.num_measurements,
                    detectors: stats.num_detectors,
                    observables: stats.num_observables,
                    sweep_bits: stats.num_sweep_bits,
                },
                shots_file: FileManifest {
                    file: "shots.b8".to_string(),
                    sha256: String::new(),
                    bits: stats.num_measurements,
                    bytes_per_shot: stats.num_measurements.div_ceil(8),
                },
            },
            circuit_text: circuit_text.to_string(),
            shots_path: PathBuf::new(),
        }
    }

    fn compiled(circuit_text: &str) -> CompiledCircuit {
        compile_circuit(&dataset_for(circuit_text)).unwrap()
    }

    fn observed(bits: &[u8]) -> Vec<usize> {
        bits.iter()
            .enumerate()
            .filter_map(|(index, &bit)| (bit != 0).then_some(index))
            .collect()
    }

    fn mask(indices: &[usize]) -> u64 {
        indices.iter().fold(0, |value, &index| value | (1 << index))
    }

    fn reference_effect(effect: &Effect) -> ReferenceEffect {
        ReferenceEffect {
            id: effect.id.clone(),
            detectors: effect.detectors.clone(),
            observables: effect.observables.clone(),
            weight: effect.weight,
        }
    }

    fn reference_mle(circuit: &CompiledCircuit, syndrome: &[u8], losses: &[usize]) -> u64 {
        let case = AtomLossCase {
            schema_version: "atom-loss-envelope.v0".to_string(),
            num_detectors: circuit.num_detectors,
            num_observables: circuit.num_observables,
            observed_detectors: observed(syndrome),
            independent_effects: circuit
                .independent_effects
                .iter()
                .map(reference_effect)
                .collect(),
            loss_envelopes: losses
                .iter()
                .map(|&loss| ReferenceLossEnvelope {
                    loss_id: circuit.envelopes[loss].id.clone(),
                    candidates: circuit.envelopes[loss]
                        .candidates
                        .iter()
                        .map(reference_effect)
                        .collect(),
                })
                .collect(),
        };
        match decode_reference_mle(&case).unwrap() {
            DecodeOutcome::Optimal(result) => mask(&result.predicted_observables),
            DecodeOutcome::Infeasible(_) => panic!("reference MLE unexpectedly infeasible"),
        }
    }

    fn reference_matching(circuit: &CompiledCircuit, shots: &[(&[u8], &[usize])]) -> Vec<u64> {
        let edge_ids: Vec<_> = (0..circuit.graph_edges.len())
            .map(|index| format!("edge-{index}"))
            .collect();
        let case = EnvelopeMatchingCase {
            schema_version: "atom-loss-envelope-matching.v0".to_string(),
            num_detectors: circuit.num_detectors,
            num_observables: circuit.num_observables,
            edges: circuit
                .graph_edges
                .iter()
                .enumerate()
                .map(|(index, edge)| EnvelopeMatchingEdge {
                    id: edge_ids[index].clone(),
                    node1: edge.node1,
                    node2: edge.node2,
                    observable_indices: edge.observables.clone(),
                    weight: edge.weight,
                    kind: match edge.kind {
                        EdgeKind::TimeLike => ReferenceEdgeKind::TimeLike,
                        EdgeKind::SpaceLike => ReferenceEdgeKind::SpaceLike,
                        EdgeKind::Boundary => ReferenceEdgeKind::Boundary,
                    },
                })
                .collect(),
            loss_edge_map: circuit
                .loss_edges
                .iter()
                .enumerate()
                .map(|(loss, edges)| LossEdgeMap {
                    loss_id: circuit.envelopes[loss].id.clone(),
                    edge_ids: edges.iter().map(|&edge| edge_ids[edge].clone()).collect(),
                })
                .collect(),
            shots: shots
                .iter()
                .map(|(syndrome, losses)| EnvelopeMatchingShot {
                    observed_detectors: observed(syndrome),
                    observed_losses: losses
                        .iter()
                        .map(|&loss| circuit.envelopes[loss].id.clone())
                        .collect(),
                })
                .collect(),
        };
        decode_reference_matching(&case).unwrap().predictions
    }

    #[test]
    fn compiled_batch_matches_explicit_reference_kernels() {
        let circuit = compiled(PERSISTENT_CIRCUIT);
        let shots: Vec<(&[u8], &[usize])> = vec![
            (&[1, 0], &[]),
            (&[0, 0], &[0]),
            (&[0, 0], &[0]),
            (&[0, 0], &[]),
        ];

        let mut mle = CompiledMle::new(&circuit, None).unwrap();
        let mle_predictions: Vec<_> = shots
            .iter()
            .map(|(syndrome, losses)| mask(&mle.decode(syndrome, losses).unwrap()))
            .collect();
        let expected_mle: Vec<_> = shots
            .iter()
            .map(|(syndrome, losses)| reference_mle(&circuit, syndrome, losses))
            .collect();
        assert_eq!(mle_predictions, expected_mle);
        assert_eq!(mle_predictions, [1, 0, 0, 0]);

        let mut matching = CompiledMatching::new(&circuit).unwrap();
        let matching_predictions: Vec<_> = shots
            .iter()
            .map(|(syndrome, losses)| mask(&matching.decode(syndrome, losses).unwrap()))
            .collect();
        assert_eq!(matching_predictions, reference_matching(&circuit, &shots));
        assert_eq!(matching_predictions, [1, 0, 0, 0]);
        assert_eq!(matching.cache.len(), 2);
    }

    #[test]
    fn all_no_loss_reduces_to_ordinary_pauli_decoding() {
        let circuit = compiled(PERSISTENT_CIRCUIT);
        let shots: Vec<(&[u8], &[usize])> = vec![
            (&[0, 0], &[]),
            (&[1, 0], &[]),
            (&[0, 1], &[]),
            (&[1, 1], &[]),
        ];
        let expected = [0, 1, 0, 1];

        let mut mle = CompiledMle::new(&circuit, None).unwrap();
        let actual_mle: Vec<_> = shots
            .iter()
            .map(|(syndrome, losses)| mask(&mle.decode(syndrome, losses).unwrap()))
            .collect();
        assert_eq!(actual_mle, expected);
        assert_eq!(
            actual_mle,
            shots
                .iter()
                .map(|(syndrome, _)| reference_mle(&circuit, syndrome, &[]))
                .collect::<Vec<_>>()
        );

        let mut matching = CompiledMatching::new(&circuit).unwrap();
        let actual_matching: Vec<_> = shots
            .iter()
            .map(|(syndrome, losses)| mask(&matching.decode(syndrome, losses).unwrap()))
            .collect();
        assert_eq!(actual_matching, expected);
        assert_eq!(actual_matching, reference_matching(&circuit, &shots));
        assert_eq!(matching.cache.len(), 1);
    }

    #[test]
    fn early_loss_envelope_propagates_beyond_pre_readout_sensitivity() {
        let early = compiled(PERSISTENT_CIRCUIT);
        assert!(
            early.envelopes[0]
                .candidates
                .iter()
                .any(|candidate| candidate.detectors.contains(&1)),
            "an onset before H/H/CX must be able to affect the target-wire detector"
        );

        let immediate_text = PERSISTENT_CIRCUIT.replacen(
            "LOSS(0.1) 0\nH 0\nH 0\nCX 0 1\n",
            "H 0\nH 0\nCX 0 1\nLOSS(0.1) 0\n",
            1,
        );
        let immediate = compiled(&immediate_text);
        assert!(
            immediate.envelopes[0]
                .candidates
                .iter()
                .all(|candidate| !candidate.detectors.contains(&1)),
            "an onset immediately before q0 readout cannot affect q1"
        );
    }

    #[test]
    fn cx_target_contributes_implicit_basis_sites_and_observables_are_padded() {
        let instrs =
            rstim::validation::parse_and_validate("R 0 1\nLOSS(0.1) 1\nCX 0 1\nML 1\n").unwrap();
        let flat = rstim::transforms::flattened(&instrs);
        let normalized = normalize_supported_circuit(&flat).unwrap();
        assert_eq!(normalized.probes[0].basis_sites.len(), 2);

        let mut matching = CompiledMatching {
            edges: vec![GraphEdge {
                node1: 0,
                node2: None,
                observables: Vec::new(),
                weight: 1.0,
                kind: EdgeKind::Boundary,
            }],
            loss_edges: Vec::new(),
            mean_weight: 1.0,
            num_observables: 2,
            cache: HashMap::new(),
        };
        assert_eq!(matching.decode(&[1], &[]).unwrap(), Vec::<usize>::new());
    }

    #[test]
    fn multi_round_mrl_preserves_flag_value_order_and_reopens_after_reset() {
        let instrs = rstim::validation::parse_and_validate(concat!(
            "R 0 1\n",
            "LOSS(0.1) 0 1\n",
            "MRL 0 1\n",
            "LOSS(0.1) 0\n",
            "MRL 0\n",
            "LOSS(0.1) 0\n",
            "ML 0\n",
        ))
        .unwrap();
        let flat = rstim::transforms::flattened(&instrs);
        let normalized = normalize_supported_circuit(&flat).unwrap();
        assert_eq!(normalized.loss_flags, [0, 2, 4, 6]);
        assert_eq!(
            normalized
                .probes
                .iter()
                .map(|probe| (probe.flag_measurement, probe.qubit))
                .collect::<Vec<_>>(),
            [(0, 0), (2, 1), (4, 0), (6, 0)]
        );
        let q0: Vec<_> = normalized
            .probes
            .iter()
            .filter(|probe| probe.qubit == 0)
            .collect();
        assert!(
            q0.windows(2)
                .all(|pair| pair[1].onset_sites.last() > Some(&pair[0].readout_site))
        );
    }

    #[test]
    fn hand_specified_single_wire_envelope_and_loss_edge_are_exact() {
        let circuit = compiled(concat!(
            "QUBIT_COORDS(0,0) 0\n",
            "R 0\n",
            "X_ERROR(0.1) 0\n",
            "LOSS(0.1) 0\n",
            "ML 0\n",
            "DETECTOR(0,0,0) rec[-1]\n",
            "OBSERVABLE_INCLUDE(0) rec[-1]\n",
        ));
        let actual: BTreeSet<_> = circuit.envelopes[0]
            .candidates
            .iter()
            .map(|candidate| (candidate.detectors.clone(), candidate.observables.clone()))
            .collect();
        assert_eq!(
            actual,
            BTreeSet::from([(Vec::new(), Vec::new()), (vec![0], vec![0])])
        );
        assert_eq!(circuit.graph_edges.len(), 1);
        assert_eq!(circuit.loss_edges, [vec![0]]);
    }

    #[test]
    fn parallel_matching_edges_require_identical_labels() {
        let compatible = vec![
            GraphEdge {
                node1: 0,
                node2: Some(1),
                observables: vec![0],
                weight: 1.0,
                kind: EdgeKind::SpaceLike,
            },
            GraphEdge {
                node1: 1,
                node2: Some(0),
                observables: vec![0],
                weight: 2.0,
                kind: EdgeKind::SpaceLike,
            },
        ];
        assert!(validate_unambiguous_parallel_edges(&compatible).is_ok());

        let mut ambiguous = compatible;
        ambiguous[1].observables.clear();
        let error = validate_unambiguous_parallel_edges(&ambiguous).unwrap_err();
        assert_eq!(error.code, "unsupported_circuit");
        assert!(error.message.contains("ambiguous observable labels"));
    }

    #[test]
    fn matching_accepts_composites_but_rejects_unmapped_primitives_and_empty_edge_sets() {
        let mut unmapped = compiled(PERSISTENT_CIRCUIT);
        unmapped.envelopes[0].candidates = vec![Effect {
            id: "composite".to_string(),
            detectors: vec![0, 1, usize::MAX],
            observables: vec![0],
            weight: 0.0,
        }];
        unmapped.unmapped_loss_primitives[0] = vec!["X_ERROR at site 1".to_string()];
        let error = CompiledMatching::new(&unmapped).err().unwrap();
        assert_eq!(error.code, "unsupported_circuit");
        assert!(error.message.contains("primitive loss effect"));

        unmapped.unmapped_loss_primitives[0].clear();
        assert!(CompiledMatching::new(&unmapped).is_ok());

        let mut empty_map = compiled(PERSISTENT_CIRCUIT);
        empty_map.envelopes[0].candidates = vec![Effect {
            id: "identity".to_string(),
            detectors: Vec::new(),
            observables: Vec::new(),
            weight: 0.0,
        }];
        empty_map.loss_edges[0].clear();
        let error = CompiledMatching::new(&empty_map).err().unwrap();
        assert_eq!(error.code, "unsupported_circuit");
        assert!(error.message.contains("loss envelope"));

        let mut time_like = CompiledMatching {
            edges: vec![GraphEdge {
                node1: 0,
                node2: Some(1),
                observables: vec![0],
                weight: 2.0,
                kind: EdgeKind::TimeLike,
            }],
            loss_edges: vec![vec![0]],
            mean_weight: 2.0,
            num_observables: 1,
            cache: HashMap::new(),
        };
        assert_eq!(time_like.decode(&[1, 1], &[0]).unwrap(), [0]);
    }

    #[test]
    fn compile_and_decode_boundaries_report_structured_errors() {
        let mut mismatched = dataset_for(PERSISTENT_CIRCUIT);
        mismatched.manifest.circuit.detectors += 1;
        let error = compile_circuit(&mismatched).err().unwrap();
        assert_eq!(error.code, "invalid_dataset");
        assert!(error.message.contains("circuit counts"));

        let no_observable = dataset_for(concat!(
            "R 0\n",
            "LOSS(0.1) 0\n",
            "X_ERROR(0.1) 0\n",
            "ML 0\n",
            "DETECTOR(0,0,0) rec[-1]\n",
        ));
        let error = compile_circuit(&no_observable).err().unwrap();
        assert_eq!(error.code, "unsupported_circuit");
        assert!(error.message.contains("1..=64 observables"));

        let circuit = compiled(PERSISTENT_CIRCUIT);
        let wrong_width = BitTable::try_new(1, 1).unwrap();
        let error = circuit.syndromes(&wrong_width).unwrap_err();
        assert_eq!(error.code, "invalid_dataset");
        assert!(error.message.contains("width"));
    }

    #[test]
    fn output_paths_must_be_distinct_and_new() {
        let root = tempfile::tempdir().unwrap();
        let shared = root.path().join("shared");
        let mut options = DecodeOptions {
            decoder: DecoderKind::EnvelopeMle,
            dataset: root.path().join("dataset"),
            predictions_out: shared.clone(),
            stats_out: shared,
            shot_timeout_ms: None,
        };
        let error = validate_output_paths(&options).unwrap_err();
        assert_eq!(error.code, "invalid_arguments");
        assert!(error.message.contains("different paths"));

        fs::write(&options.predictions_out, b"occupied").unwrap();
        options.stats_out = root.path().join("stats.json");
        let error = validate_output_paths(&options).unwrap_err();
        assert_eq!(error.code, "output_error");
        assert!(error.message.contains("already exists"));
    }

    #[test]
    fn native_benchmark_depth_midswap_circuits_compile_persistent_envelopes() {
        for distance in [3, 5] {
            let rounds = distance;
            let text = rstim::codegen::rotated_memory_z_midswap(rstim::codegen::MidSwapConfig {
                distance,
                rounds,
                pauli_probability: 0.001,
                operation_loss_probability: 0.002,
                measurement_loss_probability: 0.003,
            })
            .unwrap();
            let circuit = compiled(&text);
            let loss_flags = rounds * (distance * distance - 1) + distance * distance;
            assert_eq!(circuit.loss_flags.len(), loss_flags);
            assert_eq!(circuit.envelopes.len(), loss_flags);
            assert!(
                circuit
                    .envelopes
                    .iter()
                    .all(|envelope| !envelope.candidates.is_empty())
            );
            assert!(circuit.envelopes.iter().any(|envelope| {
                envelope
                    .candidates
                    .iter()
                    .any(|candidate| candidate.detectors.len() > 1)
            }));
            CompiledMatching::new(&circuit).unwrap();
        }
    }
}
