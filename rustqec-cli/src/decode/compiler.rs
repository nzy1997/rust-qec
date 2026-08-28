use std::collections::{BTreeSet, HashMap, HashSet};

use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};
use rstim::error_analyzer::{ErrorAnalyzer, PauliEffectAnalysisError, PauliEffectProbe};
use rstim::ir::{PauliBasis, StimInstr, StimTarget};
use rstim::m2d::CompiledLossAwareM2d;
use rstim::sim::bit_table::BitTable;

use super::dataset::Dataset;
use super::matching::{EdgeKind, GraphEdge};
use super::{
    CompiledCircuit, DecodeFailure, DecoderKind, Effect, LossEnvelope, MAX_ENVELOPE_CANDIDATES,
    MAX_PRIMITIVE_PROBES, MAX_PRIMITIVE_SYMPTOM_TERMS,
};

type PrimitiveKey = (usize, u32, &'static str);

pub(super) fn compile_circuit(
    dataset: &Dataset,
    decoder: DecoderKind,
) -> Result<CompiledCircuit, DecodeFailure> {
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
    let loss_aware_m2d = CompiledLossAwareM2d::new(&instrs)
        .map_err(|error| DecodeFailure::new("unsupported_circuit", error))?;
    let layout = loss_aware_m2d.layout();
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
    let graph_edge_index = GraphEdgeIndex::new(&graph_edges);
    let mut envelopes = Vec::new();
    let mut loss_edges = Vec::new();
    let mut unmapped_loss_primitives = Vec::new();
    let primitive_analysis = compile_primitive_effects(&normalized.noiseless, &normalized.probes)?;
    for probe in &normalized.probes {
        let compiled = match decoder {
            DecoderKind::EnvelopeMatching => {
                compile_loss_primitives(probe, &graph_edge_index, &primitive_analysis.effects)?
            }
            DecoderKind::EnvelopeMle => {
                compile_loss_candidates(probe, &graph_edge_index, &primitive_analysis.effects)?
            }
        };
        envelopes.push(LossEnvelope {
            id: format!("loss-m{}-q{}", probe.flag_measurement, probe.qubit),
            candidates: compiled.candidates,
        });
        loss_edges.push(compiled.mapped_edges);
        unmapped_loss_primitives.push(compiled.unmapped_primitives);
    }
    Ok(CompiledCircuit {
        loss_aware_m2d,
        loss_flags: normalized.loss_flags,
        independent_effects,
        envelopes,
        graph_edges,
        loss_edges,
        unmapped_loss_primitives,
        num_observables: stats.num_observables,
        primitive_probe_count: primitive_analysis.effects.len(),
        primitive_symptom_terms: primitive_analysis.symptom_terms,
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
            "QUBIT_COORDS" | "SHIFT_COORDS" | "TICK" | "DETECTOR" | "OBSERVABLE_INCLUDE"
            | "X_ERROR" | "DEPOLARIZE1" | "DEPOLARIZE2" => {
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

struct CompiledLossEnvelope {
    candidates: Vec<Effect>,
    mapped_edges: Vec<usize>,
    unmapped_primitives: Vec<String>,
}

struct GraphEdgeIndex {
    boundary: HashMap<usize, Vec<usize>>,
    internal: HashMap<(usize, usize), Vec<usize>>,
    detector_components: HashMap<usize, usize>,
    observables: Vec<Vec<usize>>,
}

impl GraphEdgeIndex {
    fn new(edges: &[GraphEdge]) -> Self {
        let mut boundary = HashMap::<usize, Vec<usize>>::new();
        let mut internal = HashMap::<(usize, usize), Vec<usize>>::new();
        let mut adjacency = HashMap::<usize, Vec<usize>>::new();
        for (index, edge) in edges.iter().enumerate() {
            adjacency.entry(edge.node1).or_default();
            if let Some(node2) = edge.node2 {
                adjacency.entry(node2).or_default();
                adjacency.entry(edge.node1).or_default().push(node2);
                adjacency.entry(node2).or_default().push(edge.node1);
                let endpoints = if edge.node1 <= node2 {
                    (edge.node1, node2)
                } else {
                    (node2, edge.node1)
                };
                internal.entry(endpoints).or_default().push(index);
            } else {
                boundary.entry(edge.node1).or_default().push(index);
            }
        }
        let mut detector_components = HashMap::new();
        let mut component = 0usize;
        for &detector in adjacency.keys() {
            if detector_components.contains_key(&detector) {
                continue;
            }
            let mut pending = vec![detector];
            detector_components.insert(detector, component);
            while let Some(current) = pending.pop() {
                for &neighbor in &adjacency[&current] {
                    if detector_components.insert(neighbor, component).is_none() {
                        pending.push(neighbor);
                    }
                }
            }
            component += 1;
        }
        Self {
            boundary,
            internal,
            detector_components,
            observables: edges.iter().map(|edge| edge.observables.clone()).collect(),
        }
    }

    fn compatible_edges(&self, effect: &Effect) -> Vec<usize> {
        // A primitive Pauli effect must be represented by one exact graph edge
        // in each disconnected detector subgraph. Merely mentioning a detector
        // does not make every boundary edge incident on it part of the
        // envelope, and a multi-detector symptom must not be expanded into a
        // clique of cheap matching edges.
        let mut by_component = HashMap::<usize, Vec<usize>>::new();
        for &detector in &effect.detectors {
            let Some(&component) = self.detector_components.get(&detector) else {
                return Vec::new();
            };
            by_component.entry(component).or_default().push(detector);
        }
        let mut compatible = Vec::new();
        let mut represented_observables = Vec::new();
        for detectors in by_component.values_mut() {
            detectors.sort_unstable();
            let edges = match detectors.as_slice() {
                [detector] => self.boundary.get(detector),
                [left, right] => self.internal.get(&(*left, *right)),
                _ => return Vec::new(),
            };
            let Some(edges) = edges else {
                return Vec::new();
            };
            let (first, rest) = (edges[0], &edges[1..]);
            let label = &self.observables[first];
            if rest.iter().any(|&edge| self.observables[edge] != *label) {
                return Vec::new();
            }
            represented_observables = xor_indices(&represented_observables, label);
            compatible.extend(edges);
        }
        if represented_observables != effect.observables {
            return Vec::new();
        }
        compatible
    }
}

struct PrimitiveKeyCollector {
    keys: BTreeSet<PrimitiveKey>,
    limit: usize,
    #[cfg(test)]
    inspected_candidates: usize,
}

impl PrimitiveKeyCollector {
    fn new(limit: usize) -> Self {
        Self {
            keys: BTreeSet::new(),
            limit,
            #[cfg(test)]
            inspected_candidates: 0,
        }
    }

    fn add_probe(&mut self, probe: &LossProbe) -> Result<(), DecodeFailure> {
        let Some(earliest_onset) = probe.onset_sites.iter().copied().min() else {
            return Ok(());
        };
        for site in probe
            .onset_sites
            .iter()
            .copied()
            .chain(
                probe
                    .basis_sites
                    .iter()
                    .copied()
                    .filter(|&site| site >= earliest_onset && site <= probe.readout_site),
            )
            .chain(std::iter::once(probe.readout_site))
        {
            for name in ["X_ERROR", "Y_ERROR", "Z_ERROR"] {
                #[cfg(test)]
                {
                    self.inspected_candidates += 1;
                }
                let key = (site, probe.qubit, name);
                if !self.keys.contains(&key) {
                    if self.keys.len() >= self.limit {
                        return Err(primitive_probe_limit_error());
                    }
                    self.keys.insert(key);
                }
            }
        }
        Ok(())
    }

    fn into_keys(self) -> BTreeSet<PrimitiveKey> {
        self.keys
    }
}

struct PrimitiveEffectAnalysis {
    effects: HashMap<PrimitiveKey, Effect>,
    symptom_terms: usize,
}

fn compile_primitive_effects(
    noiseless: &[StimInstr],
    probes: &[LossProbe],
) -> Result<PrimitiveEffectAnalysis, DecodeFailure> {
    let mut collector = PrimitiveKeyCollector::new(MAX_PRIMITIVE_PROBES);
    for probe in probes {
        collector.add_probe(probe)?;
    }
    let keys = collector.into_keys();
    let queries: Vec<_> = keys
        .iter()
        .map(|&(insertion, qubit, name)| PauliEffectProbe {
            insertion,
            qubit,
            basis: match name {
                "X_ERROR" => PauliBasis::X,
                "Y_ERROR" => PauliBasis::Y,
                "Z_ERROR" => PauliBasis::Z,
                _ => unreachable!("primitive keys use Pauli error instructions"),
            },
        })
        .collect();
    let analyzed = ErrorAnalyzer::circuit_pauli_effects_with_target_limit(
        noiseless,
        &queries,
        MAX_PRIMITIVE_SYMPTOM_TERMS,
    )
    .map_err(|error| match error {
        PauliEffectAnalysisError::Circuit(message) => {
            DecodeFailure::new("unsupported_circuit", message)
        }
        PauliEffectAnalysisError::TargetTermLimitExceeded { .. } => primitive_symptom_limit_error(),
    })?;
    // The analyzer capped target-vector allocation incrementally. Compute the
    // exact detector/observable count here for stats and as a defensive check.
    let mut symptom_terms = 0usize;
    let mut effects = HashMap::with_capacity(keys.len());
    for (key, analyzed) in keys.into_iter().zip(analyzed) {
        let (detectors, observables) = symptoms(&analyzed.targets);
        symptom_terms = symptom_terms
            .checked_add(detectors.len())
            .and_then(|total| total.checked_add(observables.len()))
            .ok_or_else(primitive_symptom_limit_error)?;
        if symptom_terms > MAX_PRIMITIVE_SYMPTOM_TERMS {
            return Err(primitive_symptom_limit_error());
        }
        effects.insert(
            key,
            Effect {
                id: key.2.to_ascii_lowercase(),
                detectors,
                observables,
                weight: 0.0,
            },
        );
    }
    Ok(PrimitiveEffectAnalysis {
        effects,
        symptom_terms,
    })
}

fn primitive_probe_limit_error() -> DecodeFailure {
    DecodeFailure::new(
        "unsupported_circuit",
        format!("persistent-loss circuit exceeds primitive probe limit of {MAX_PRIMITIVE_PROBES}"),
    )
}

fn primitive_symptom_limit_error() -> DecodeFailure {
    DecodeFailure::new(
        "unsupported_circuit",
        format!(
            "persistent-loss circuit exceeds primitive symptom-term limit of {MAX_PRIMITIVE_SYMPTOM_TERMS}"
        ),
    )
}

fn map_primitive_effect(
    key: PrimitiveKey,
    effect: &Effect,
    graph_edges: &GraphEdgeIndex,
    mapped: &mut BTreeSet<usize>,
    unmapped: &mut BTreeSet<String>,
) {
    if effect.detectors.is_empty() && effect.observables.is_empty() {
        return;
    }
    let compatible = graph_edges.compatible_edges(effect);
    for edge_index in &compatible {
        mapped.insert(*edge_index);
    }
    if compatible.is_empty() {
        unmapped.insert(format!("{} at site {} for qubit {}", key.2, key.0, key.1));
    }
}

fn compile_loss_primitives(
    probe: &LossProbe,
    graph_edges: &GraphEdgeIndex,
    primitive_effects: &HashMap<PrimitiveKey, Effect>,
) -> Result<CompiledLossEnvelope, DecodeFailure> {
    let mut mapped = BTreeSet::new();
    let mut unmapped = BTreeSet::new();
    let mut collector = PrimitiveKeyCollector::new(MAX_PRIMITIVE_PROBES);
    collector.add_probe(probe)?;
    for key in collector.into_keys() {
        let effect = primitive_effects.get(&key).ok_or_else(|| {
            DecodeFailure::new(
                "unsupported_circuit",
                "primitive loss effect is missing from batched analysis",
            )
        })?;
        map_primitive_effect(key, effect, graph_edges, &mut mapped, &mut unmapped);
    }
    Ok(CompiledLossEnvelope {
        candidates: Vec::new(),
        mapped_edges: mapped.into_iter().collect(),
        unmapped_primitives: unmapped.into_iter().collect(),
    })
}

fn compile_loss_candidates(
    probe: &LossProbe,
    graph_edges: &GraphEdgeIndex,
    primitive_effects: &HashMap<PrimitiveKey, Effect>,
) -> Result<CompiledLossEnvelope, DecodeFailure> {
    let mut union = BTreeSet::<(Vec<usize>, Vec<usize>)>::new();
    let mut mapped = BTreeSet::new();
    let mut unmapped = BTreeSet::new();
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
                let key = (site, probe.qubit, name);
                let effect = primitive_effects.get(&key).ok_or_else(|| {
                    DecodeFailure::new(
                        "unsupported_circuit",
                        "primitive loss effect is missing from batched analysis",
                    )
                })?;
                map_primitive_effect(key, effect, graph_edges, &mut mapped, &mut unmapped);
                choices.push((effect.detectors.clone(), effect.observables.clone()));
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
    Ok(CompiledLossEnvelope {
        candidates: union
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
            .collect(),
        mapped_edges: mapped.into_iter().collect(),
        unmapped_primitives: unmapped.into_iter().collect(),
    })
}

fn xor_indices(left: &[usize], right: &[usize]) -> Vec<usize> {
    let mut values: BTreeSet<usize> = left.iter().copied().collect();
    for &value in right {
        toggle(&mut values, value);
    }
    values.into_iter().collect()
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
    pub(super) fn loss_aware_syndromes(
        &self,
        measurements: &BitTable,
    ) -> Result<Vec<rstim::m2d::LossAwareDetectorShot>, DecodeFailure> {
        if measurements.num_major() != self.loss_aware_m2d.layout().num_measurements() {
            return Err(DecodeFailure::new(
                "invalid_dataset",
                "measurement block width does not match compiled circuit",
            ));
        }
        self.loss_aware_m2d
            .convert(measurements)
            .map(|output| output.shots)
            .map_err(|error| DecodeFailure::new("invalid_dataset", error))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn error_message<T>(result: Result<T, DecodeFailure>) -> String {
        let error = result.err().expect("operation must be rejected");
        assert_eq!(error.code, "unsupported_circuit");
        error.message
    }

    #[test]
    fn normalization_rejects_invalid_loss_windows_and_targets() {
        let inline_noise = [StimInstr::new("ML", vec![0.1], vec![StimTarget::Qubit(0)])];
        assert!(error_message(normalize_supported_circuit(&inline_noise)).contains("inline noise"));

        let missing_loss = [StimInstr::new("ML", Vec::new(), vec![StimTarget::Qubit(0)])];
        assert!(
            error_message(normalize_supported_circuit(&missing_loss))
                .contains("no LOSS opportunity")
        );

        let incomplete_cx = [StimInstr::new("CX", Vec::new(), vec![StimTarget::Qubit(0)])];
        assert!(
            error_message(normalize_supported_circuit(&incomplete_cx))
                .contains("complete qubit pairs")
        );

        let invalid_target = [StimInstr::new(
            "LOSS",
            Vec::new(),
            vec![StimTarget::Sweep(0)],
        )];
        assert!(
            error_message(normalize_supported_circuit(&invalid_target))
                .contains("non-inverted qubits")
        );

        let no_readout = [StimInstr::new("R", Vec::new(), vec![StimTarget::Qubit(0)])];
        assert!(
            error_message(normalize_supported_circuit(&no_readout))
                .contains("no supported loss-visible measurements")
        );
    }

    #[test]
    fn dem_validation_reports_each_unsupported_graph_shape() {
        for (text, expected) in [
            (
                "detector(0,0) D0\nerror(0.1) D0\n",
                "at least x,y,t coordinates",
            ),
            (
                "detector(0,0,0) D0\nerror(0.5) D0\n",
                "finite and below 0.5",
            ),
            ("error(0.1) L0\n", "observable-only"),
            (
                concat!(
                    "detector(0,0,0) D0\n",
                    "detector(1,0,0) D1\n",
                    "detector(2,0,0) D2\n",
                    "error(0.1) D0 D1 D2\n",
                ),
                "non-graphlike",
            ),
            ("error(0.1) D0 D1\n", "detector 0 has no coordinates"),
            (
                "detector(0,0,0) D0\nerror(0.1) D0 D1\n",
                "detector 1 has no coordinates",
            ),
            ("", "no decodable independent Pauli effects"),
        ] {
            let dem = DetectorErrorModel::parse(text).unwrap();
            assert!(
                error_message(effects_and_edges_from_dem(&dem)).contains(expected),
                "expected {expected:?} for {text:?}"
            );
        }
    }

    #[test]
    fn dem_graph_classification_handles_time_space_boundary_and_separators() {
        let dem = DetectorErrorModel::parse(concat!(
            "detector(0,0,0) D0\n",
            "detector(0,0,1) D1\n",
            "detector(1,0,0) D2\n",
            "error(0) D0\n",
            "error(0.1) D0 D1 ^ D0 D2 ^ D2 L0\n",
        ))
        .unwrap();
        let (effects, edges) = effects_and_edges_from_dem(&dem).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0].kind, EdgeKind::TimeLike);
        assert_eq!(edges[1].kind, EdgeKind::SpaceLike);
        assert_eq!(edges[2].kind, EdgeKind::Boundary);
        assert_eq!(edges[2].observables, [0]);
    }

    #[test]
    fn primitive_edge_mapping_requires_exact_components_and_observables() {
        let edges = vec![
            GraphEdge {
                node1: 0,
                node2: None,
                observables: vec![0],
                weight: 1.0,
                kind: EdgeKind::Boundary,
            },
            GraphEdge {
                node1: 1,
                node2: None,
                observables: Vec::new(),
                weight: 1.0,
                kind: EdgeKind::Boundary,
            },
            GraphEdge {
                node1: 1,
                node2: Some(0),
                observables: Vec::new(),
                weight: 1.0,
                kind: EdgeKind::SpaceLike,
            },
            GraphEdge {
                node1: 1,
                node2: Some(2),
                observables: Vec::new(),
                weight: 1.0,
                kind: EdgeKind::SpaceLike,
            },
            GraphEdge {
                node1: 1,
                node2: None,
                observables: vec![1],
                weight: 1.0,
                kind: EdgeKind::Boundary,
            },
            GraphEdge {
                node1: 3,
                node2: None,
                observables: vec![1],
                weight: 1.0,
                kind: EdgeKind::Boundary,
            },
        ];
        let index = GraphEdgeIndex::new(&edges);

        assert_eq!(
            index.compatible_edges(&Effect {
                id: "pair".to_string(),
                detectors: vec![0, 1],
                observables: Vec::new(),
                weight: 0.0,
            }),
            [2]
        );
        assert!(
            index
                .compatible_edges(&Effect {
                    id: "wrong-boundary-label".to_string(),
                    detectors: vec![0],
                    observables: Vec::new(),
                    weight: 0.0,
                })
                .is_empty()
        );
        assert_eq!(
            index.compatible_edges(&Effect {
                id: "logical-boundary".to_string(),
                detectors: vec![0],
                observables: vec![0],
                weight: 0.0,
            }),
            [0]
        );
        for (id, detectors, observables) in [
            ("unknown-detector", vec![99], Vec::new()),
            ("three-detector-component", vec![0, 1, 2], Vec::new()),
            ("missing-boundary", vec![2], Vec::new()),
            ("ambiguous-parallel-label", vec![1], Vec::new()),
        ] {
            assert!(
                index
                    .compatible_edges(&Effect {
                        id: id.to_string(),
                        detectors,
                        observables,
                        weight: 0.0,
                    })
                    .is_empty(),
                "{id} must not map approximately"
            );
        }
        let mut disconnected = index.compatible_edges(&Effect {
            id: "disconnected-xor".to_string(),
            detectors: vec![0, 3],
            observables: vec![0, 1],
            weight: 0.0,
        });
        disconnected.sort_unstable();
        assert_eq!(disconnected, [0, 5]);
    }

    #[test]
    fn current_rstim_atom_loss_resource_bound_and_unmapped_effects_fail_explicitly() {
        let oversized_probe = LossProbe {
            flag_measurement: 7,
            qubit: 0,
            onset_sites: (0..=(MAX_PRIMITIVE_PROBES / 3 + 1)).collect(),
            basis_sites: Vec::new(),
            readout_site: 0,
        };
        let message = error_message(compile_primitive_effects(&[], &[oversized_probe]));
        assert_eq!(
            message,
            format!(
                "persistent-loss circuit exceeds primitive probe limit of {MAX_PRIMITIVE_PROBES}"
            )
        );

        let probe = LossProbe {
            flag_measurement: 7,
            qubit: 0,
            onset_sites: vec![0],
            basis_sites: Vec::new(),
            readout_site: 0,
        };
        let unmapped = Effect {
            id: "unmapped".to_string(),
            detectors: vec![0],
            observables: Vec::new(),
            weight: 0.0,
        };
        let cache = ["X_ERROR", "Y_ERROR", "Z_ERROR"]
            .into_iter()
            .map(|name| ((0, 0, name), unmapped.clone()))
            .collect();
        let compiled = compile_loss_candidates(&probe, &GraphEdgeIndex::new(&[]), &cache).unwrap();
        assert_eq!(compiled.unmapped_primitives.len(), 3);
        assert!(
            primitive_symptom_limit_error()
                .message
                .contains("symptom-term limit")
        );
    }

    #[test]
    fn primitive_key_collection_is_linear_and_stops_at_its_budget() {
        const SIDE: usize = 50_000;
        let probe = LossProbe {
            flag_measurement: 7,
            qubit: 0,
            onset_sites: (0..SIDE).collect(),
            basis_sites: (SIDE..2 * SIDE).collect(),
            readout_site: 2 * SIDE,
        };
        let mut collector = PrimitiveKeyCollector::new(9);

        let message = error_message(collector.add_probe(&probe));

        assert_eq!(message, primitive_probe_limit_error().message);
        assert_eq!(collector.keys.len(), 9);
        assert_eq!(collector.inspected_candidates, 10);
    }

    #[test]
    fn primitive_key_collection_keeps_the_union_of_all_loss_windows() {
        let probe = LossProbe {
            flag_measurement: 7,
            qubit: 4,
            onset_sites: vec![2, 5],
            basis_sites: vec![1, 3, 5, 7, 11],
            readout_site: 10,
        };
        let mut collector = PrimitiveKeyCollector::new(MAX_PRIMITIVE_PROBES);

        collector.add_probe(&probe).unwrap();

        let sites: BTreeSet<_> = collector
            .into_keys()
            .into_iter()
            .map(|(site, _, _)| site)
            .collect();
        assert_eq!(sites, BTreeSet::from([2, 3, 5, 7, 10]));
    }
}
