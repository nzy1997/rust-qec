use std::collections::{BTreeSet, HashMap, HashSet};

use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};
use rstim::ir::{StimInstr, StimTarget};
use rstim::measurement_transform::{CheckedMeasurementLayout, MeasurementTransformLimits};
use rstim::sim::bit_table::BitTable;

use super::dataset::Dataset;
use super::matching::{EdgeKind, GraphEdge, candidate_affects_edge};
use super::{
    CompiledCircuit, DecodeFailure, Effect, LossEnvelope, MAX_ENVELOPE_CANDIDATES,
    MAX_PRIMITIVE_PROBES,
};

pub(super) fn compile_circuit(dataset: &Dataset) -> Result<CompiledCircuit, DecodeFailure> {
    let instrs = rstim::validation::parse_and_validate(&dataset.circuit_text)
        .map_err(|error| DecodeFailure::new("unsupported_circuit", error))?;
    if instrs
        .iter()
        .any(|instruction| matches!(instruction, StimInstr::Repeat { .. }))
    {
        return Err(DecodeFailure::new(
            "unsupported_circuit",
            "REPEAT blocks are outside the flat native Mid-SWAP subset",
        ));
    }
    let stats = rstim::stats::summarize(&instrs);
    let declared = &dataset.manifest.circuit;
    if dataset.manifest.row.bits != stats.num_measurements
        || declared.measurements != stats.num_measurements
        || declared.detectors != stats.num_detectors
        || declared.observables != stats.num_observables
        || declared.sweep_bits != stats.num_sweep_bits
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "manifest row width or circuit counts do not match circuit.stim",
        ));
    }
    if stats.num_sweep_bits != 0 || stats.num_observables == 0 || stats.num_observables > 64 {
        return Err(DecodeFailure::new(
            "unsupported_circuit",
            "decode requires 1..=64 observables and no sweep bits",
        ));
    }
    let normalized = normalize_supported_circuit(&instrs)?;
    let layout = CheckedMeasurementLayout::from_circuit_with_limits(
        &instrs,
        MeasurementTransformLimits::default(),
    )
    .map_err(|error| DecodeFailure::new("unsupported_circuit", error.to_string()))?;
    let reference = rstim::data_path::build_reference_sample(
        &instrs,
        rstim::data_path::ReferenceSampleMode::SimulateNoiseless,
    )
    .map_err(|error| DecodeFailure::new("unsupported_circuit", error))?;
    let flag_set: HashSet<usize> = normalized.loss_flags.iter().copied().collect();
    if layout
        .detector_rows()
        .iter()
        .chain(layout.observable_rows())
        .flatten()
        .any(|index| flag_set.contains(index))
    {
        return Err(DecodeFailure::new(
            "unsupported_circuit",
            "detectors and observables must reference value bits, not loss flags",
        ));
    }
    let dem = rstim::error_analyzer::ErrorAnalyzer::circuit_to_dem_decomposed(&normalized.analysis)
        .map_err(|error| DecodeFailure::new("unsupported_circuit", error))?;
    let (independent_effects, graph_edges) = effects_and_edges_from_dem(&dem)?;
    let mut envelopes = Vec::new();
    let mut loss_edges = Vec::new();
    let mut primitive_cache = HashMap::new();
    for probe in normalized.probes {
        let candidates =
            compile_loss_candidates(&normalized.noiseless, &probe, &mut primitive_cache)?;
        let mut mapped = BTreeSet::new();
        for candidate in &candidates {
            for (edge_index, edge) in graph_edges.iter().enumerate() {
                if candidate_affects_edge(candidate, edge) {
                    mapped.insert(edge_index);
                }
            }
        }
        envelopes.push(LossEnvelope {
            id: format!("loss-m{}-q{}", probe.flag_measurement, probe.qubit),
            candidates,
        });
        loss_edges.push(mapped.into_iter().collect());
    }
    Ok(CompiledCircuit {
        layout,
        reference,
        loss_flags: normalized.loss_flags,
        independent_effects,
        envelopes,
        graph_edges,
        loss_edges,
        num_detectors: stats.num_detectors,
        num_observables: stats.num_observables,
    })
}

pub(super) struct LossProbe {
    pub(super) flag_measurement: usize,
    pub(super) qubit: u32,
    pub(super) onset_sites: Vec<usize>,
    pub(super) basis_sites: Vec<usize>,
    pub(super) readout_site: usize,
}

pub(super) struct NormalizedLossCircuit {
    analysis: Vec<StimInstr>,
    noiseless: Vec<StimInstr>,
    pub(super) loss_flags: Vec<usize>,
    pub(super) probes: Vec<LossProbe>,
}

pub(super) fn normalize_supported_circuit(
    flat: &[StimInstr],
) -> Result<NormalizedLossCircuit, DecodeFailure> {
    let mut normalized = Vec::new();
    let mut noiseless = Vec::new();
    let mut loss_flags = Vec::new();
    let mut probes = Vec::new();
    let mut measurement_index = 0usize;
    let mut onset_sites = HashMap::<u32, Vec<usize>>::new();
    let mut basis_sites = HashMap::<u32, Vec<usize>>::new();
    let mut terminal_measurements = HashSet::new();
    for instruction in flat {
        let StimInstr::Op {
            name,
            args,
            targets,
            ..
        } = instruction
        else {
            unreachable!("flattened circuit contains only operations")
        };
        if targets
            .iter()
            .filter_map(StimTarget::qubit_index)
            .any(|qubit| terminal_measurements.contains(&qubit))
        {
            return Err(DecodeFailure::new(
                "unsupported_circuit",
                "ML must be terminal for each measured physical wire",
            ));
        }
        match name.as_str() {
            "LOSS" => {
                for qubit in qubit_targets(targets)? {
                    onset_sites.entry(qubit).or_default().push(noiseless.len());
                }
            }
            "ML" | "MZL" | "MRL" | "MRZL" => {
                if !args.is_empty() {
                    return Err(DecodeFailure::new(
                        "unsupported_circuit",
                        "loss-visible measurements with inline noise are unsupported",
                    ));
                }
                let qubits = qubit_targets(targets)?;
                let ordinary = if matches!(name.as_str(), "MRL" | "MRZL") {
                    "MR"
                } else {
                    "M"
                };
                for qubit in qubits {
                    let onsets = onset_sites.get(&qubit).cloned().unwrap_or_default();
                    if onsets.is_empty() {
                        return Err(DecodeFailure::new(
                            "unsupported_circuit",
                            format!(
                                "loss-visible readout of qubit {qubit} has no LOSS opportunity since reset"
                            ),
                        ));
                    }
                    loss_flags.push(measurement_index);
                    let pad = StimInstr::new("MPAD", Vec::new(), vec![StimTarget::Qubit(0)]);
                    normalized.push(pad.clone());
                    noiseless.push(pad);
                    measurement_index += 1;
                    let readout_site = noiseless.len();
                    let measurement =
                        StimInstr::new(ordinary, Vec::new(), vec![StimTarget::Qubit(qubit)]);
                    normalized.push(measurement.clone());
                    noiseless.push(measurement);
                    probes.push(LossProbe {
                        flag_measurement: measurement_index - 1,
                        qubit,
                        onset_sites: onsets,
                        basis_sites: basis_sites.get(&qubit).cloned().unwrap_or_default(),
                        readout_site,
                    });
                    measurement_index += 1;
                    if ordinary == "MR" {
                        onset_sites.remove(&qubit);
                        basis_sites.remove(&qubit);
                    } else {
                        terminal_measurements.insert(qubit);
                    }
                }
            }
            "MXL" | "MYL" | "MRXL" | "MRYL" => {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    format!("unsupported loss-visible readout instruction {name}"),
                ));
            }
            "H" => {
                normalized.push(instruction.clone());
                noiseless.push(instruction.clone());
                let site = noiseless.len();
                for qubit in qubit_targets(targets)? {
                    basis_sites.entry(qubit).or_default().push(site);
                }
            }
            "CX" | "CNOT" | "ZCX" => {
                let qubits = qubit_targets(targets)?;
                if qubits.len() % 2 != 0 {
                    return Err(DecodeFailure::new(
                        "unsupported_circuit",
                        "CX requires complete qubit pairs",
                    ));
                }
                let mut used = HashSet::new();
                if qubits.iter().any(|&qubit| !used.insert(qubit)) {
                    return Err(DecodeFailure::new(
                        "unsupported_circuit",
                        "parallel CX pairs must use disjoint qubits",
                    ));
                }
                let controls: Vec<_> = qubits.chunks_exact(2).map(|pair| pair[0]).collect();
                let targets: Vec<_> = qubits.chunks_exact(2).map(|pair| pair[1]).collect();
                let h_targets: Vec<_> = targets.iter().copied().map(StimTarget::Qubit).collect();
                let cz_targets: Vec<_> = controls
                    .iter()
                    .zip(&targets)
                    .flat_map(|(&control, &target)| {
                        [StimTarget::Qubit(control), StimTarget::Qubit(target)]
                    })
                    .collect();
                for operation in [
                    StimInstr::new("H", Vec::new(), h_targets.clone()),
                    StimInstr::new("CZ", Vec::new(), cz_targets),
                    StimInstr::new("H", Vec::new(), h_targets),
                ] {
                    normalized.push(operation.clone());
                    noiseless.push(operation);
                    if noiseless.last().and_then(StimInstr::name) == Some("H") {
                        let site = noiseless.len();
                        for &qubit in &targets {
                            basis_sites.entry(qubit).or_default().push(site);
                        }
                    }
                }
            }
            "R" | "RZ" => {
                normalized.push(instruction.clone());
                noiseless.push(instruction.clone());
                for qubit in qubit_targets(targets)? {
                    onset_sites.remove(&qubit);
                    basis_sites.remove(&qubit);
                }
            }
            "QUBIT_COORDS" | "TICK" | "DETECTOR" | "OBSERVABLE_INCLUDE" | "X_ERROR"
            | "DEPOLARIZE1" | "DEPOLARIZE2" => {
                normalized.push(instruction.clone());
                if !is_noise_instruction(name) {
                    noiseless.push(instruction.clone());
                }
            }
            _ => {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    format!("instruction {name} is outside the supported Mid-SWAP subset"),
                ));
            }
        }
    }
    if probes.is_empty() {
        return Err(DecodeFailure::new(
            "unsupported_circuit",
            "circuit has no supported loss-visible measurements",
        ));
    }
    Ok(NormalizedLossCircuit {
        analysis: normalized,
        noiseless,
        loss_flags,
        probes,
    })
}

fn is_noise_instruction(name: &str) -> bool {
    matches!(
        name,
        "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "CORRELATED_ERROR"
            | "E"
            | "ELSE_CORRELATED_ERROR"
            | "I_ERROR"
            | "II_ERROR"
    )
}

fn qubit_targets(targets: &[StimTarget]) -> Result<Vec<u32>, DecodeFailure> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(qubit) => Ok(*qubit),
            _ => Err(DecodeFailure::new(
                "unsupported_circuit",
                "loss-visible targets must be non-inverted qubits",
            )),
        })
        .collect()
}

fn effects_and_edges_from_dem(
    dem: &DetectorErrorModel,
) -> Result<(Vec<Effect>, Vec<GraphEdge>), DecodeFailure> {
    let mut effects = Vec::new();
    let mut edges = Vec::new();
    let mut detector_coords = HashMap::new();
    for instruction in dem.instructions() {
        if let DemInstruction::Detector { index, coords } = instruction {
            if coords.len() < 3 {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    format!("detector {index} needs at least x,y,t coordinates"),
                ));
            }
            detector_coords.insert(*index, coords.clone());
        }
    }
    for instruction in dem.instructions() {
        let DemInstruction::Error {
            probability,
            targets,
        } = instruction
        else {
            continue;
        };
        if *probability <= 0.0 {
            continue;
        }
        if *probability >= 0.5 || !probability.is_finite() {
            return Err(DecodeFailure::new(
                "unsupported_circuit",
                format!("Pauli DEM probability {probability} must be finite and below 0.5"),
            ));
        }
        let weight = ((1.0 - probability) / probability).ln();
        let (detectors, observables) = symptoms(targets);
        effects.push(Effect {
            id: format!("pauli-{}", effects.len()),
            detectors: detectors.clone(),
            observables: observables.clone(),
            weight,
        });
        for component in split_components(targets) {
            let (component_detectors, component_observables) = symptoms(component);
            if component_detectors.is_empty() {
                if !component_observables.is_empty() {
                    return Err(DecodeFailure::new(
                        "unsupported_circuit",
                        "observable-only Pauli effects are unsupported by envelope-matching",
                    ));
                }
                continue;
            }
            if component_detectors.len() > 2 {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    "non-graphlike Pauli effect remains after DEM decomposition",
                ));
            }
            let kind = if component_detectors.len() == 1 {
                EdgeKind::Boundary
            } else {
                let left = detector_coords
                    .get(&component_detectors[0])
                    .ok_or_else(|| {
                        DecodeFailure::new(
                            "unsupported_circuit",
                            format!("detector {} has no coordinates", component_detectors[0]),
                        )
                    })?;
                let right = detector_coords
                    .get(&component_detectors[1])
                    .ok_or_else(|| {
                        DecodeFailure::new(
                            "unsupported_circuit",
                            format!("detector {} has no coordinates", component_detectors[1]),
                        )
                    })?;
                if left[0] == right[0] && left[1] == right[1] {
                    EdgeKind::TimeLike
                } else {
                    EdgeKind::SpaceLike
                }
            };
            edges.push(GraphEdge {
                node1: component_detectors[0],
                node2: component_detectors.get(1).copied(),
                observables: component_observables,
                weight,
                kind,
            });
        }
    }
    if effects.is_empty() || edges.is_empty() {
        return Err(DecodeFailure::new(
            "unsupported_circuit",
            "circuit has no decodable independent Pauli effects",
        ));
    }
    Ok((effects, edges))
}

fn compile_loss_candidates(
    noiseless: &[StimInstr],
    probe: &LossProbe,
    primitive_cache: &mut HashMap<(usize, u32, &'static str), Effect>,
) -> Result<Vec<Effect>, DecodeFailure> {
    let mut union = BTreeSet::<(Vec<usize>, Vec<usize>)>::new();
    for &onset in &probe.onset_sites {
        let mut sites = vec![onset];
        sites.extend(
            probe
                .basis_sites
                .iter()
                .copied()
                .filter(|&site| site >= onset && site <= probe.readout_site),
        );
        sites.push(probe.readout_site);
        sites.sort_unstable();
        sites.dedup();
        let mut states = BTreeSet::from([(Vec::new(), Vec::new())]);
        for site in sites {
            let mut choices = vec![(Vec::new(), Vec::new())];
            for name in ["X_ERROR", "Y_ERROR", "Z_ERROR"] {
                let effect = if let Some(effect) = primitive_cache.get(&(site, probe.qubit, name)) {
                    effect.clone()
                } else {
                    if primitive_cache.len() >= MAX_PRIMITIVE_PROBES {
                        return Err(DecodeFailure::new(
                            "unsupported_circuit",
                            "persistent-loss circuit exceeds primitive probe limit",
                        ));
                    }
                    let effect = probe_effect(noiseless, site, probe.qubit, name)?;
                    primitive_cache.insert((site, probe.qubit, name), effect.clone());
                    effect
                };
                choices.push((effect.detectors, effect.observables));
            }
            let mut next = BTreeSet::new();
            for state in &states {
                for choice in &choices {
                    next.insert((
                        xor_indices(&state.0, &choice.0),
                        xor_indices(&state.1, &choice.1),
                    ));
                }
            }
            if next.len() > MAX_ENVELOPE_CANDIDATES {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    format!(
                        "loss envelope for measurement {} exceeds candidate limit",
                        probe.flag_measurement
                    ),
                ));
            }
            states = next;
        }
        union.extend(states);
        if union.len() > MAX_ENVELOPE_CANDIDATES {
            return Err(DecodeFailure::new(
                "unsupported_circuit",
                format!(
                    "loss envelope for measurement {} exceeds candidate limit",
                    probe.flag_measurement
                ),
            ));
        }
    }
    Ok(union
        .into_iter()
        .enumerate()
        .map(|(index, (detectors, observables))| Effect {
            id: if detectors.is_empty() && observables.is_empty() {
                "identity".to_string()
            } else {
                format!("candidate-{index}")
            },
            detectors,
            observables,
            weight: 0.0,
        })
        .collect())
}

fn xor_indices(left: &[usize], right: &[usize]) -> Vec<usize> {
    let mut values: BTreeSet<usize> = left.iter().copied().collect();
    for &value in right {
        toggle(&mut values, value);
    }
    values.into_iter().collect()
}

fn probe_effect(
    noiseless: &[StimInstr],
    insertion: usize,
    qubit: u32,
    name: &'static str,
) -> Result<Effect, DecodeFailure> {
    let mut circuit = noiseless.to_vec();
    if insertion > circuit.len() {
        return Err(DecodeFailure::new(
            "unsupported_circuit",
            "loss probe position is outside normalized circuit",
        ));
    }
    circuit.insert(
        insertion,
        StimInstr::new(name, vec![0.125], vec![StimTarget::Qubit(qubit)]),
    );
    let dem = rstim::error_analyzer::ErrorAnalyzer::circuit_to_dem(&circuit)
        .map_err(|error| DecodeFailure::new("unsupported_circuit", error))?;
    let mut found = None;
    for instruction in dem.instructions() {
        if let DemInstruction::Error {
            probability,
            targets,
        } = instruction
        {
            if *probability <= 0.0 {
                continue;
            }
            if found.is_some() {
                return Err(DecodeFailure::new(
                    "unsupported_circuit",
                    "a primitive loss probe produced multiple DEM mechanisms",
                ));
            }
            found = Some(symptoms(targets));
        }
    }
    let (detectors, observables) = found.unwrap_or_default();
    Ok(Effect {
        id: name.to_ascii_lowercase(),
        detectors,
        observables,
        weight: 0.0,
    })
}

fn symptoms(targets: &[DemTarget]) -> (Vec<usize>, Vec<usize>) {
    let mut detectors = BTreeSet::new();
    let mut observables = BTreeSet::new();
    for target in targets {
        match target {
            DemTarget::Detector(index) => toggle(&mut detectors, *index),
            DemTarget::Observable(index) => toggle(&mut observables, *index),
            DemTarget::Separator => {}
        }
    }
    (
        detectors.into_iter().collect(),
        observables.into_iter().collect(),
    )
}

fn toggle(set: &mut BTreeSet<usize>, value: usize) {
    if !set.insert(value) {
        set.remove(&value);
    }
}

fn split_components(targets: &[DemTarget]) -> Vec<&[DemTarget]> {
    let mut out = Vec::new();
    let mut start = 0;
    for (index, target) in targets.iter().enumerate() {
        if matches!(target, DemTarget::Separator) {
            out.push(&targets[start..index]);
            start = index + 1;
        }
    }
    out.push(&targets[start..]);
    out
}

impl CompiledCircuit {
    pub(super) fn syndromes(&self, measurements: &BitTable) -> Result<Vec<Vec<u8>>, DecodeFailure> {
        if measurements.num_major() != self.layout.num_measurements() {
            return Err(DecodeFailure::new(
                "invalid_dataset",
                "measurement block width does not match compiled circuit",
            ));
        }
        let mut output = vec![vec![0; self.num_detectors]; measurements.num_minor()];
        for (detector, terms) in self.layout.detector_rows().iter().enumerate() {
            for (shot, row) in output.iter_mut().enumerate() {
                row[detector] = terms.iter().fold(0u8, |parity, &measurement| {
                    parity
                        ^ u8::from(
                            measurements.get(measurement, shot) ^ self.reference[measurement],
                        )
                });
            }
        }
        Ok(output)
    }

    pub(super) fn loss_patterns(&self, measurements: &BitTable) -> Vec<Vec<usize>> {
        (0..measurements.num_minor())
            .map(|shot| {
                self.loss_flags
                    .iter()
                    .enumerate()
                    .filter_map(|(loss, &measurement)| {
                        measurements.get(measurement, shot).then_some(loss)
                    })
                    .collect()
            })
            .collect()
    }
}
