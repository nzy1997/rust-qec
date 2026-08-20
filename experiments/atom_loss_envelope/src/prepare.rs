use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::{StimInstr, StimTarget};
use rstim::m2d::measurements_to_detections;
use rstim::output::read_shots_b8;
use rstim::sim::bit_table::BitTable;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::matching::MATCHING_INPUT_SCHEMA_VERSION;
use crate::schema::INPUT_SCHEMA_VERSION;
use crate::{
    AtomLossCase, EdgeKind, Effect, EnvelopeMatchingCase, EnvelopeMatchingEdge,
    EnvelopeMatchingShot, LossEdgeMap, LossEnvelope,
};

pub const PREPARATION_SCHEMA_VERSION: &str = "atom-loss-envelope-preparation.v0";

#[derive(Debug, Clone)]
pub struct PrepareConfig {
    pub circuit: PathBuf,
    pub calibration_in: PathBuf,
    pub calibration_shots: usize,
    pub input: PathBuf,
    pub shots: usize,
    pub out: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparationManifest {
    pub schema_version: &'static str,
    pub circuit_sha256: String,
    pub calibration_sha256: String,
    pub shots_sha256: String,
    pub raw_measurement_row_bits: usize,
    pub raw_measurement_row_bytes: usize,
    pub compact_value_row_bits: usize,
    pub compact_value_row_bytes: usize,
    pub observable_row_bits: usize,
    pub observable_row_bytes: usize,
    pub observables_sha256: String,
    pub calibration_shots: usize,
    pub target_shots: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub loss_readout_count: usize,
    pub retained_single_loss_calibration_rows: usize,
    pub calibrated_pattern_count: usize,
    pub independent_effect_count: usize,
    pub matching_edge_count: usize,
    pub loss_edge_membership_count: usize,
    pub losses: Vec<LossPreparationSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LossPreparationSummary {
    pub loss_id: String,
    pub raw_flag_index: usize,
    pub retained_single_loss_rows: usize,
    pub calibrated_patterns: usize,
    pub edge_memberships: usize,
}

#[derive(Debug, Clone)]
struct LossReadout {
    id: String,
    raw_flag_index: usize,
}

#[derive(Debug)]
struct NormalizedCircuit {
    instructions: Vec<StimInstr>,
    raw_to_compact: Vec<Option<usize>>,
    loss_readouts: Vec<LossReadout>,
    raw_measurements: usize,
    compact_measurements: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Pattern {
    detectors: Vec<usize>,
    observables: Vec<usize>,
}

#[derive(Debug)]
struct DemArtifacts {
    independent_effects: Vec<Effect>,
    edges: Vec<EnvelopeMatchingEdge>,
}

struct CalibrationResult {
    patterns: Vec<BTreeSet<Pattern>>,
    retained_rows: usize,
    retained_per_loss: Vec<usize>,
}

#[derive(Default)]
struct ExpandedDem {
    errors: Vec<(f64, Vec<DemTarget>)>,
    detector_coords: HashMap<usize, Vec<f64>>,
    detector_offset: usize,
    coord_offsets: Vec<f64>,
}

pub fn prepare(config: &PrepareConfig) -> Result<PreparationManifest, String> {
    validate_output_target(&config.out)?;
    if config.calibration_shots == 0 {
        return Err("--calibration_shots must be greater than zero".to_string());
    }
    if config.shots == 0 {
        return Err("--shots must be greater than zero".to_string());
    }

    let circuit_bytes = read_input(&config.circuit, "circuit")?;
    let circuit_text = std::str::from_utf8(&circuit_bytes)
        .map_err(|error| format!("circuit {} is not UTF-8: {error}", config.circuit.display()))?;
    let circuit = rstim::validation::parse_and_validate(circuit_text)
        .map_err(|error| format!("invalid circuit {}: {error}", config.circuit.display()))?;
    if rstim::stats::num_observables(&circuit) != 1 {
        return Err(format!(
            "prepare requires exactly one logical observable, found {}",
            rstim::stats::num_observables(&circuit)
        ));
    }
    let normalized = normalize_circuit(&circuit)?;
    if normalized.loss_readouts.is_empty() {
        return Err("prepare requires at least one loss-visible measurement".to_string());
    }

    let calibration_bytes = read_input(&config.calibration_in, "calibration input")?;
    let shot_bytes = read_input(&config.input, "target input")?;
    let calibration_raw = read_exact_b8(
        &calibration_bytes,
        normalized.raw_measurements,
        config.calibration_shots,
        "calibration",
    )?;
    let target_raw = read_exact_b8(
        &shot_bytes,
        normalized.raw_measurements,
        config.shots,
        "target",
    )?;

    let calibration_values = compact_values(&calibration_raw, &normalized)?;
    let target_values = compact_values(&target_raw, &normalized)?;
    let calibration_m2d = measurements_to_detections(&normalized.instructions, &calibration_values)
        .map_err(|error| format!("failed to convert calibration measurements: {error}"))?;
    let target_m2d = measurements_to_detections(&normalized.instructions, &target_values)
        .map_err(|error| format!("failed to convert target measurements: {error}"))?;

    let calibration = calibrate_patterns(
        &calibration_raw,
        &calibration_m2d.detections,
        &calibration_m2d.observable_flips,
        &normalized.loss_readouts,
    )?;

    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&normalized.instructions)
        .map_err(|error| format!("failed to derive Pauli-only detector error model: {error}"))?;
    if dem.num_observables() != 1 {
        return Err(format!(
            "Pauli-only detector error model must contain one observable, found {}",
            dem.num_observables()
        ));
    }
    let dem_artifacts = dem_artifacts(&dem)?;
    if dem_artifacts.edges.is_empty() {
        return Err("Pauli-only detector error model produced no matching edges".to_string());
    }

    let loss_edge_map = build_loss_edge_map(
        &normalized.loss_readouts,
        &calibration.patterns,
        &dem_artifacts.edges,
    );
    let prepared_shots = build_shots(
        &target_raw,
        &target_m2d.detections,
        &normalized.loss_readouts,
    );
    let mle_cases = build_mle_cases(
        dem.num_detectors(),
        dem.num_observables(),
        &dem_artifacts.independent_effects,
        &normalized.loss_readouts,
        &calibration.patterns,
        &prepared_shots,
    );
    let matching_case = EnvelopeMatchingCase {
        schema_version: MATCHING_INPUT_SCHEMA_VERSION.to_string(),
        num_detectors: dem.num_detectors(),
        num_observables: dem.num_observables(),
        edges: dem_artifacts.edges.clone(),
        loss_edge_map: loss_edge_map.clone(),
        shots: prepared_shots,
    };
    let mut observable_bytes = Vec::new();
    rstim::output::append_shots_b8(&target_m2d.observable_flips, &mut observable_bytes)
        .map_err(|error| format!("failed to encode target observable values: {error}"))?;

    let losses = normalized
        .loss_readouts
        .iter()
        .enumerate()
        .map(|(index, loss)| LossPreparationSummary {
            loss_id: loss.id.clone(),
            raw_flag_index: loss.raw_flag_index,
            retained_single_loss_rows: calibration.retained_per_loss[index],
            calibrated_patterns: calibration.patterns[index].len(),
            edge_memberships: loss_edge_map[index].edge_ids.len(),
        })
        .collect();
    let manifest = PreparationManifest {
        schema_version: PREPARATION_SCHEMA_VERSION,
        circuit_sha256: sha256_hex(&circuit_bytes),
        calibration_sha256: sha256_hex(&calibration_bytes),
        shots_sha256: sha256_hex(&shot_bytes),
        raw_measurement_row_bits: normalized.raw_measurements,
        raw_measurement_row_bytes: normalized.raw_measurements.div_ceil(8),
        compact_value_row_bits: normalized.compact_measurements,
        compact_value_row_bytes: normalized.compact_measurements.div_ceil(8),
        observable_row_bits: target_m2d.observable_flips.num_major(),
        observable_row_bytes: target_m2d.observable_flips.num_major().div_ceil(8),
        observables_sha256: sha256_hex(&observable_bytes),
        calibration_shots: config.calibration_shots,
        target_shots: config.shots,
        num_detectors: dem.num_detectors(),
        num_observables: dem.num_observables(),
        loss_readout_count: normalized.loss_readouts.len(),
        retained_single_loss_calibration_rows: calibration.retained_rows,
        calibrated_pattern_count: calibration.patterns.iter().map(BTreeSet::len).sum(),
        independent_effect_count: dem_artifacts.independent_effects.len(),
        matching_edge_count: dem_artifacts.edges.len(),
        loss_edge_membership_count: loss_edge_map
            .iter()
            .map(|mapping| mapping.edge_ids.len())
            .sum(),
        losses,
    };

    let mut files = Vec::with_capacity(mle_cases.len() + 3);
    files.push((PathBuf::from("manifest.json"), json_bytes(&manifest)?));
    files.push((PathBuf::from("observables.b8"), observable_bytes));
    for (shot, case) in mle_cases.iter().enumerate() {
        files.push((
            PathBuf::from(format!("mle/shot-{shot:06}.json")),
            json_bytes(case)?,
        ));
    }
    files.push((PathBuf::from("matching.json"), json_bytes(&matching_case)?));
    install_bundle(&config.out, &files)?;
    Ok(manifest)
}

fn read_input(path: &Path, kind: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("failed to read {kind} {}: {error}", path.display()))
}

fn normalize_circuit(circuit: &[StimInstr]) -> Result<NormalizedCircuit, String> {
    let flattened = rstim::transforms::flattened(circuit);
    let mut instructions = Vec::with_capacity(flattened.len());
    let mut raw_to_compact = Vec::new();
    let mut loss_readouts = Vec::new();
    let mut raw_measurements = 0usize;
    let mut compact_measurements = 0usize;

    for instruction in flattened {
        let StimInstr::Op {
            name,
            tag,
            args,
            mut targets,
        } = instruction
        else {
            return Err("internal error: flattened circuit retained a REPEAT block".to_string());
        };

        rewrite_record_targets(
            &name,
            &mut targets,
            raw_measurements,
            compact_measurements,
            &raw_to_compact,
        )?;
        if name == "LOSS" {
            continue;
        }

        if let Some(ordinary_name) = ordinary_measurement_name(&name) {
            let target_count = targets
                .iter()
                .filter(|target| matches!(target, StimTarget::Qubit(_) | StimTarget::QubitInv(_)))
                .count();
            if target_count == 0 {
                return Err(format!("{name} must contain at least one qubit target"));
            }
            for _ in 0..target_count {
                let flag_index = raw_measurements;
                raw_to_compact.push(None);
                raw_to_compact.push(Some(compact_measurements));
                loss_readouts.push(LossReadout {
                    id: format!("loss-m{flag_index}"),
                    raw_flag_index: flag_index,
                });
                raw_measurements += 2;
                compact_measurements += 1;
            }
            instructions.push(StimInstr::Op {
                name: ordinary_name.to_string(),
                tag,
                args,
                targets,
            });
            continue;
        }

        let rewritten = StimInstr::Op {
            name,
            tag,
            args,
            targets,
        };
        let produced = rstim::stats::num_measurements(std::slice::from_ref(&rewritten));
        for _ in 0..produced {
            raw_to_compact.push(Some(compact_measurements));
            raw_measurements += 1;
            compact_measurements += 1;
        }
        instructions.push(rewritten);
    }

    if rstim::stats::num_measurements(&instructions) != compact_measurements {
        return Err("internal error: compact measurement count is inconsistent".to_string());
    }
    Ok(NormalizedCircuit {
        instructions,
        raw_to_compact,
        loss_readouts,
        raw_measurements,
        compact_measurements,
    })
}

fn ordinary_measurement_name(name: &str) -> Option<&'static str> {
    match name {
        "ML" => Some("M"),
        "MZL" => Some("MZ"),
        "MXL" => Some("MX"),
        "MYL" => Some("MY"),
        "MRL" => Some("MR"),
        "MRZL" => Some("MRZ"),
        "MRXL" => Some("MRX"),
        "MRYL" => Some("MRY"),
        _ => None,
    }
}

fn rewrite_record_targets(
    operation: &str,
    targets: &mut [StimTarget],
    raw_measurements: usize,
    compact_measurements: usize,
    raw_to_compact: &[Option<usize>],
) -> Result<(), String> {
    for target in targets {
        let StimTarget::Rec(offset) = target else {
            continue;
        };
        let absolute = i128::try_from(raw_measurements)
            .map_err(|_| "raw measurement count is too large".to_string())?
            + i128::from(*offset);
        if absolute < 0 || absolute >= raw_measurements as i128 {
            return Err(format!(
                "{operation} has invalid record target rec[{offset}] after {raw_measurements} raw measurements"
            ));
        }
        let raw_index = usize::try_from(absolute)
            .map_err(|_| "record target index is too large".to_string())?;
        let compact_index = raw_to_compact[raw_index].ok_or_else(|| {
            format!(
                "invalid loss-flag reference: {operation} rec[{offset}] refers to raw measurement {raw_index}"
            )
        })?;
        let rewritten = i128::try_from(compact_index)
            .map_err(|_| "compact measurement index is too large".to_string())?
            - i128::try_from(compact_measurements)
                .map_err(|_| "compact measurement count is too large".to_string())?;
        if rewritten >= 0 {
            return Err(format!(
                "{operation} record target rec[{offset}] does not refer to an earlier compact value"
            ));
        }
        *offset = i32::try_from(rewritten)
            .map_err(|_| format!("rewritten record target for {operation} exceeds i32 range"))?;
    }
    Ok(())
}

fn read_exact_b8(data: &[u8], bits: usize, shots: usize, kind: &str) -> Result<BitTable, String> {
    let row_bytes = bits
        .checked_add(7)
        .ok_or_else(|| format!("{kind} b8 row width overflows"))?
        / 8;
    let expected = row_bytes
        .checked_mul(shots)
        .ok_or_else(|| format!("{kind} b8 input length overflows"))?;
    if data.len() != expected {
        return Err(format!(
            "{kind} b8 input has {} bytes; expected {expected} for {shots} shots of {bits} bits",
            data.len()
        ));
    }
    if !bits.is_multiple_of(8) {
        let used_mask = (1u8 << (bits % 8)) - 1;
        for (shot, row) in data.chunks_exact(row_bytes).enumerate() {
            if row[row_bytes - 1] & !used_mask != 0 {
                return Err(format!(
                    "{kind} b8 input shot {shot} has nonzero unused high bits"
                ));
            }
        }
    }
    let table = read_shots_b8(data, bits)?;
    if table.num_minor() != shots {
        return Err(format!(
            "{kind} b8 input decoded {} shots; expected {shots}",
            table.num_minor()
        ));
    }
    Ok(table)
}

fn compact_values(raw: &BitTable, normalized: &NormalizedCircuit) -> Result<BitTable, String> {
    let mut compact = BitTable::try_new(normalized.compact_measurements, raw.num_minor())
        .map_err(|error| format!("failed to allocate compact measurement table: {error:?}"))?;
    for (raw_index, compact_index) in normalized.raw_to_compact.iter().enumerate() {
        let Some(compact_index) = compact_index else {
            continue;
        };
        for shot in 0..raw.num_minor() {
            if raw.get(raw_index, shot) {
                compact.set(*compact_index, shot, true);
            }
        }
    }
    Ok(compact)
}

fn calibrate_patterns(
    raw: &BitTable,
    detections: &BitTable,
    observables: &BitTable,
    losses: &[LossReadout],
) -> Result<CalibrationResult, String> {
    let mut patterns = vec![BTreeSet::new(); losses.len()];
    let mut retained_rows = 0usize;
    let mut retained_per_loss = vec![0usize; losses.len()];
    for shot in 0..raw.num_minor() {
        let asserted: Vec<usize> = losses
            .iter()
            .enumerate()
            .filter_map(|(index, loss)| raw.get(loss.raw_flag_index, shot).then_some(index))
            .collect();
        if let [loss_index] = asserted.as_slice() {
            retained_rows += 1;
            retained_per_loss[*loss_index] += 1;
            patterns[*loss_index].insert(Pattern {
                detectors: asserted_indices(detections, shot),
                observables: asserted_indices(observables, shot),
            });
        }
    }
    for (loss, calibrated) in losses.iter().zip(&patterns) {
        if calibrated.is_empty() {
            return Err(format!(
                "loss readout {:?} has no pure single-loss calibration pattern",
                loss.id
            ));
        }
    }
    Ok(CalibrationResult {
        patterns,
        retained_rows,
        retained_per_loss,
    })
}

fn asserted_indices(table: &BitTable, shot: usize) -> Vec<usize> {
    (0..table.num_major())
        .filter(|&index| table.get(index, shot))
        .collect()
}

fn dem_artifacts(dem: &DetectorErrorModel) -> Result<DemArtifacts, String> {
    let mut expanded = ExpandedDem::default();
    expand_dem(dem.instructions(), &mut expanded)?;
    let mut independent_effects = Vec::with_capacity(expanded.errors.len());
    let mut edges = Vec::new();

    for (error_index, (probability, targets)) in expanded.errors.iter().enumerate() {
        let id = format!("dem-e{error_index}");
        let weight = llr_weight(*probability, &id)?;
        let components = split_components(targets);
        let mut all_detectors = BTreeSet::new();
        let mut all_observables = BTreeSet::new();
        for component in &components {
            let (detectors, observables) = component_pattern(component);
            toggle_all(&mut all_detectors, &detectors);
            toggle_all(&mut all_observables, &observables);
        }
        independent_effects.push(Effect {
            id: id.clone(),
            detectors: all_detectors.into_iter().collect(),
            observables: all_observables.into_iter().collect(),
            weight,
        });

        let component_count = components.len();
        for (component_index, component) in components.iter().enumerate() {
            let (detectors, observables) = component_pattern(component);
            let edge_id = if component_count == 1 {
                id.clone()
            } else {
                format!("{id}-c{component_index}")
            };
            let (node1, node2, kind) = match detectors.as_slice() {
                [] if observables.is_empty() => continue,
                [] => {
                    return Err(format!(
                        "DEM component {edge_id} changes an observable without a detector"
                    ));
                }
                [node] => (*node, None, EdgeKind::Boundary),
                [node1, node2] => {
                    let time_like = same_spatial_coordinates(
                        expanded.detector_coords.get(node1).map(Vec::as_slice),
                        expanded.detector_coords.get(node2).map(Vec::as_slice),
                    );
                    (
                        *node1,
                        Some(*node2),
                        if time_like {
                            EdgeKind::TimeLike
                        } else {
                            EdgeKind::SpaceLike
                        },
                    )
                }
                _ => {
                    return Err(format!(
                        "DEM component {edge_id} is not graphlike after decomposition"
                    ));
                }
            };
            edges.push(EnvelopeMatchingEdge {
                id: edge_id,
                node1,
                node2,
                observable_indices: observables,
                weight,
                kind,
            });
        }
    }
    Ok(DemArtifacts {
        independent_effects,
        edges,
    })
}

fn expand_dem(instructions: &[DemInstruction], state: &mut ExpandedDem) -> Result<(), String> {
    for instruction in instructions {
        match instruction {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                let shifted = targets
                    .iter()
                    .map(|target| match target {
                        DemTarget::Detector(index) => index
                            .checked_add(state.detector_offset)
                            .map(DemTarget::Detector)
                            .ok_or_else(|| "DEM detector index overflows".to_string()),
                        other => Ok(other.clone()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                state.errors.push((*probability, shifted));
            }
            DemInstruction::Detector { index, coords } => {
                let index = index
                    .checked_add(state.detector_offset)
                    .ok_or_else(|| "DEM detector coordinate index overflows".to_string())?;
                state
                    .detector_coords
                    .insert(index, shifted_coords(coords, &state.coord_offsets));
            }
            DemInstruction::LogicalObservable { .. } => {}
            DemInstruction::ShiftDetectors {
                detector_offset,
                coord_offsets,
            } => {
                state.detector_offset = state
                    .detector_offset
                    .checked_add(*detector_offset)
                    .ok_or_else(|| "DEM detector shift overflows".to_string())?;
                if state.coord_offsets.len() < coord_offsets.len() {
                    state.coord_offsets.resize(coord_offsets.len(), 0.0);
                }
                for (index, offset) in coord_offsets.iter().enumerate() {
                    state.coord_offsets[index] += offset;
                }
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    expand_dem(body.instructions(), state)?;
                }
            }
        }
    }
    Ok(())
}

fn shifted_coords(coords: &[f64], offsets: &[f64]) -> Vec<f64> {
    let width = coords.len().max(offsets.len());
    (0..width)
        .map(|index| {
            coords.get(index).copied().unwrap_or(0.0) + offsets.get(index).copied().unwrap_or(0.0)
        })
        .collect()
}

fn split_components(targets: &[DemTarget]) -> Vec<Vec<DemTarget>> {
    let mut components = vec![Vec::new()];
    for target in targets {
        if matches!(target, DemTarget::Separator) {
            if !components.last().is_some_and(Vec::is_empty) {
                components.push(Vec::new());
            }
        } else {
            components.last_mut().unwrap().push(target.clone());
        }
    }
    components.retain(|component| !component.is_empty());
    components
}

fn component_pattern(component: &[DemTarget]) -> (Vec<usize>, Vec<usize>) {
    let mut detectors = BTreeSet::new();
    let mut observables = BTreeSet::new();
    for target in component {
        match target {
            DemTarget::Detector(index) => toggle(&mut detectors, *index),
            DemTarget::Observable(index) => toggle(&mut observables, *index),
            DemTarget::Separator => unreachable!("components do not contain separators"),
        }
    }
    (
        detectors.into_iter().collect(),
        observables.into_iter().collect(),
    )
}

fn toggle(values: &mut BTreeSet<usize>, value: usize) {
    if !values.insert(value) {
        values.remove(&value);
    }
}

fn toggle_all(values: &mut BTreeSet<usize>, others: &[usize]) {
    for &value in others {
        toggle(values, value);
    }
}

fn llr_weight(probability: f64, id: &str) -> Result<f64, String> {
    if !probability.is_finite() || probability <= 0.0 || probability > 0.5 {
        return Err(format!(
            "DEM effect {id} probability must be finite and in (0, 0.5], got {probability}"
        ));
    }
    Ok(((1.0 - probability) / probability).ln())
}

fn same_spatial_coordinates(left: Option<&[f64]>, right: Option<&[f64]>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let left = spatial_coordinates(left);
    let right = spatial_coordinates(right);
    !left.is_empty() && left == right
}

fn spatial_coordinates(coords: &[f64]) -> &[f64] {
    if coords.len() >= 3 {
        &coords[..coords.len() - 1]
    } else {
        coords
    }
}

fn build_loss_edge_map(
    losses: &[LossReadout],
    patterns: &[BTreeSet<Pattern>],
    edges: &[EnvelopeMatchingEdge],
) -> Vec<LossEdgeMap> {
    losses
        .iter()
        .zip(patterns)
        .map(|(loss, patterns)| {
            let edge_ids = edges
                .iter()
                .filter(|edge| {
                    let mut incidence = vec![edge.node1];
                    if let Some(node2) = edge.node2 {
                        incidence.push(node2);
                        incidence.sort_unstable();
                    }
                    patterns.iter().any(|pattern| {
                        matches!(pattern.detectors.len(), 1 | 2) && pattern.detectors == incidence
                    })
                })
                .map(|edge| edge.id.clone())
                .collect();
            LossEdgeMap {
                loss_id: loss.id.clone(),
                edge_ids,
            }
        })
        .collect()
}

fn build_shots(
    raw: &BitTable,
    detections: &BitTable,
    losses: &[LossReadout],
) -> Vec<EnvelopeMatchingShot> {
    (0..raw.num_minor())
        .map(|shot| EnvelopeMatchingShot {
            observed_detectors: asserted_indices(detections, shot),
            observed_losses: losses
                .iter()
                .filter(|loss| raw.get(loss.raw_flag_index, shot))
                .map(|loss| loss.id.clone())
                .collect(),
        })
        .collect()
}

fn build_mle_cases(
    num_detectors: usize,
    num_observables: usize,
    independent_effects: &[Effect],
    losses: &[LossReadout],
    patterns: &[BTreeSet<Pattern>],
    shots: &[EnvelopeMatchingShot],
) -> Vec<AtomLossCase> {
    let candidates: HashMap<&str, Vec<Effect>> = losses
        .iter()
        .zip(patterns)
        .map(|(loss, patterns)| {
            let candidates = patterns
                .iter()
                .enumerate()
                .map(|(index, pattern)| Effect {
                    id: format!("{}-c{index}", loss.id),
                    detectors: pattern.detectors.clone(),
                    observables: pattern.observables.clone(),
                    weight: 0.0,
                })
                .collect();
            (loss.id.as_str(), candidates)
        })
        .collect();
    shots
        .iter()
        .map(|shot| AtomLossCase {
            schema_version: INPUT_SCHEMA_VERSION.to_string(),
            num_detectors,
            num_observables,
            observed_detectors: shot.observed_detectors.clone(),
            independent_effects: independent_effects.to_vec(),
            loss_envelopes: shot
                .observed_losses
                .iter()
                .map(|loss_id| LossEnvelope {
                    loss_id: loss_id.clone(),
                    candidates: candidates[loss_id.as_str()].clone(),
                })
                .collect(),
        })
        .collect()
}

fn validate_output_target(out: &Path) -> Result<(), String> {
    if !out.exists() {
        let parent = output_parent(out);
        if !parent.is_dir() {
            return Err(format!(
                "output parent directory {} does not exist",
                parent.display()
            ));
        }
        return Ok(());
    }
    if !out.is_dir() {
        return Err(format!(
            "output {} exists and is not a directory",
            out.display()
        ));
    }
    let mut entries = fs::read_dir(out)
        .map_err(|error| format!("failed to inspect output {}: {error}", out.display()))?;
    if entries
        .next()
        .transpose()
        .map_err(|error| format!("failed to inspect output {}: {error}", out.display()))?
        .is_some()
    {
        return Err(format!(
            "output directory {} is not empty; overwrite is not supported",
            out.display()
        ));
    }
    Ok(())
}

fn install_bundle(out: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<(), String> {
    validate_output_target(out)?;
    let parent = output_parent(out);
    let staging = tempfile::Builder::new()
        .prefix(".atom-loss-envelope-")
        .tempdir_in(parent)
        .map_err(|error| format!("failed to create staging directory: {error}"))?;
    fs::create_dir(staging.path().join("mle"))
        .map_err(|error| format!("failed to create staged mle directory: {error}"))?;
    for (relative, bytes) in files {
        let path = staging.path().join(relative);
        fs::write(&path, bytes).map_err(|error| {
            format!("failed to write staged output {}: {error}", path.display())
        })?;
    }

    if out.exists() {
        validate_output_target(out)?;
        fs::remove_dir(out).map_err(|error| {
            format!(
                "failed to replace empty output directory {}: {error}",
                out.display()
            )
        })?;
    }
    fs::rename(staging.path(), out).map_err(|error| {
        format!(
            "failed to install output directory {}: {error}",
            out.display()
        )
    })?;
    Ok(())
}

fn output_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to serialize prepared output: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_compacts_loss_pairs_and_rewrites_records() {
        let circuit = rstim::validation::parse_and_validate(
            "R 0\nMRL 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        )
        .unwrap();
        let normalized = normalize_circuit(&circuit).unwrap();

        assert_eq!(normalized.raw_measurements, 2);
        assert_eq!(normalized.compact_measurements, 1);
        assert_eq!(normalized.raw_to_compact, vec![None, Some(0)]);
        assert_eq!(
            rstim::ir::circuit_to_string(&normalized.instructions),
            "R 0\nMR 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
        );
    }

    #[test]
    fn normalization_rejects_loss_flag_references() {
        let circuit = rstim::validation::parse_and_validate(
            "R 0\nMRL 0\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        )
        .unwrap();
        let error = normalize_circuit(&circuit).unwrap_err();
        assert!(error.contains("invalid loss-flag reference"));
        assert!(error.contains("DETECTOR"));
    }

    #[test]
    fn normalization_rewrites_multi_target_value_records() {
        let circuit = rstim::validation::parse_and_validate(
            "R 0 1\nMRL 0 1\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        )
        .unwrap();
        let normalized = normalize_circuit(&circuit).unwrap();

        assert_eq!(normalized.raw_measurements, 4);
        assert_eq!(normalized.compact_measurements, 2);
        assert_eq!(
            normalized.raw_to_compact,
            vec![None, Some(0), None, Some(1)]
        );
        assert_eq!(
            rstim::ir::circuit_to_string(&normalized.instructions),
            "R 0 1\nMR 0 1\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
        );
    }

    #[test]
    fn loss_edge_mapping_does_not_gf2_close_patterns() {
        let losses = vec![LossReadout {
            id: "loss-m0".to_string(),
            raw_flag_index: 0,
        }];
        let patterns = vec![BTreeSet::from([Pattern {
            detectors: vec![0, 1, 2],
            observables: vec![],
        }])];
        let edges = vec![EnvelopeMatchingEdge {
            id: "dem-e0".to_string(),
            node1: 0,
            node2: Some(1),
            observable_indices: vec![],
            weight: 1.0,
            kind: EdgeKind::SpaceLike,
        }];

        assert!(
            build_loss_edge_map(&losses, &patterns, &edges)[0]
                .edge_ids
                .is_empty()
        );
    }

    #[test]
    fn loss_edge_mapping_keeps_every_parallel_primitive_edge() {
        let losses = vec![LossReadout {
            id: "loss-m0".to_string(),
            raw_flag_index: 0,
        }];
        let patterns = vec![BTreeSet::from([Pattern {
            detectors: vec![0, 1],
            observables: vec![0],
        }])];
        let edges = ["dem-e0", "dem-e1"]
            .into_iter()
            .map(|id| EnvelopeMatchingEdge {
                id: id.to_string(),
                node1: 0,
                node2: Some(1),
                observable_indices: vec![],
                weight: 1.0,
                kind: EdgeKind::SpaceLike,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            build_loss_edge_map(&losses, &patterns, &edges)[0].edge_ids,
            vec!["dem-e0", "dem-e1"]
        );
    }

    #[test]
    fn edge_kind_compares_spatial_but_not_time_coordinates() {
        assert!(same_spatial_coordinates(
            Some(&[1.0, 2.0, 3.0]),
            Some(&[1.0, 2.0, 4.0])
        ));
        assert!(!same_spatial_coordinates(
            Some(&[1.0, 2.0, 3.0]),
            Some(&[1.0, 4.0, 3.0])
        ));
        assert!(!same_spatial_coordinates(None, Some(&[1.0, 2.0])));
    }
}
