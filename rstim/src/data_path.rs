use crate::compiled::SamplingFallbackReason;
use crate::ir::{StimInstr, StimTarget};
use crate::reference_sample_tree::ReferenceSampleTree;
use crate::sim::packed_inverse_tableau::PackedInverseTableau;

const REPEAT_FOLD_THRESHOLD: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSampleMode {
    SimulateNoiseless,
    AssumeAllZero,
}

impl Default for ReferenceSampleMode {
    fn default() -> Self {
        Self::SimulateNoiseless
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSampleDecision {
    PackedInverse,
    LegacyFallback(SamplingFallbackReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReferenceBuildPhaseCounters {
    pub measurement_reset_batches: usize,
    pub canonical_materializations: usize,
    pub canonical_writebacks: usize,
    pub direct_inverse_batches: usize,
    pub transposed_collapse_batches: usize,
    pub collapse_pivots: usize,
    pub expanded_repeat_iterations: usize,
    pub executed_repeat_iterations: usize,
    pub skipped_repeat_iterations: usize,
    pub measurement_bits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSampleResult {
    pub bits: Vec<bool>,
    pub decision: ReferenceSampleDecision,
    pub phase_counters: ReferenceBuildPhaseCounters,
}

pub fn build_reference_sample(
    instrs: &[StimInstr],
    mode: ReferenceSampleMode,
) -> Result<Vec<bool>, String> {
    match mode {
        ReferenceSampleMode::SimulateNoiseless => {
            Ok(build_reference_sample_with_decision(instrs)?.bits)
        }
        ReferenceSampleMode::AssumeAllZero => {
            Ok(vec![false; crate::stats::num_measurements(instrs)])
        }
    }
}

pub fn build_reference_sample_with_decision(
    instrs: &[StimInstr],
) -> Result<ReferenceSampleResult, String> {
    build_reference_sample_with_sweep_bits_and_decision(instrs, None)
}

pub fn build_reference_sample_with_sweep_bits_and_decision(
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<ReferenceSampleResult, String> {
    match build_packed_reference_sample(instrs) {
        Ok((bits, counters)) => Ok(ReferenceSampleResult {
            bits,
            decision: ReferenceSampleDecision::PackedInverse,
            phase_counters: counters,
        }),
        Err(reason) => {
            let bits = crate::executor::reference_sample_with_sweep_bits(instrs, sweep_bits)?;
            Ok(ReferenceSampleResult {
                bits,
                decision: ReferenceSampleDecision::LegacyFallback(reason),
                phase_counters: ReferenceBuildPhaseCounters {
                    measurement_bits: crate::stats::num_measurements(instrs),
                    ..ReferenceBuildPhaseCounters::default()
                },
            })
        }
    }
}

fn build_packed_reference_sample(
    instrs: &[StimInstr],
) -> Result<(Vec<bool>, ReferenceBuildPhaseCounters), SamplingFallbackReason> {
    let num_qubits =
        crate::executor::max_qubit(instrs).map_err(SamplingFallbackReason::UnsupportedOperation)?;
    let mut tableau = PackedInverseTableau::identity(num_qubits);
    let mut counters = ReferenceBuildPhaseCounters {
        measurement_bits: crate::stats::num_measurements(instrs),
        ..ReferenceBuildPhaseCounters::default()
    };
    let tree = packed_reference_instrs(&mut tableau, instrs, &mut counters)?;
    let mut measurements = Vec::with_capacity(tree.size());
    tree.decompress_into(&mut measurements);
    Ok((measurements, counters))
}

fn saturating_usize_from_u64(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn add_repeat_counter(slot: &mut usize, value: u64) {
    *slot = slot.saturating_add(saturating_usize_from_u64(value));
}

fn logical_repeat_iterations(instrs: &[StimInstr]) -> u64 {
    instrs.iter().fold(0_u64, |total, instr| match instr {
        StimInstr::Op { .. } => total,
        StimInstr::Repeat { count, body } => total.saturating_add(
            count.saturating_add(count.saturating_mul(logical_repeat_iterations(body))),
        ),
    })
}

fn append_tree_bits(tree: &mut ReferenceSampleTree, bits: Vec<bool>) {
    if bits.is_empty() {
        return;
    }
    if tree.suffix_children.is_empty() {
        tree.prefix_bits.extend(bits);
    } else {
        tree.suffix_children.push(ReferenceSampleTree {
            prefix_bits: bits,
            suffix_children: Vec::new(),
            repetitions: 1,
        });
    }
}

fn append_tree_child(tree: &mut ReferenceSampleTree, child: ReferenceSampleTree) {
    if !child.empty() {
        tree.suffix_children.push(child);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepeatBoundaryState {
    num_qubits: usize,
    stabilizer_basis: Vec<Option<RepeatPauliRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepeatPauliRow {
    x: Vec<bool>,
    z: Vec<bool>,
    phase: u8,
}

impl RepeatPauliRow {
    fn bit(&self, bit: usize) -> bool {
        let n = self.x.len();
        if bit < n {
            self.x[bit]
        } else {
            self.z[bit - n]
        }
    }

    fn leading_bit(&self) -> Option<usize> {
        (0..2 * self.x.len()).find(|&bit| self.bit(bit))
    }

    fn is_identity(&self) -> bool {
        !self.x.iter().any(|&bit| bit) && !self.z.iter().any(|&bit| bit)
    }

    fn multiply_assign(&mut self, rhs: &RepeatPauliRow) {
        let mut phase_delta = 0;
        for q in 0..self.x.len() {
            let (x, z, phase) = multiply_pauli(self.x[q], self.z[q], rhs.x[q], rhs.z[q]);
            self.x[q] = x;
            self.z[q] = z;
            phase_delta = (phase_delta + phase) % 4;
        }
        self.phase = (self.phase + rhs.phase + phase_delta) % 4;
    }
}

fn multiply_pauli(x1: bool, z1: bool, x2: bool, z2: bool) -> (bool, bool, u8) {
    match ((x1, z1), (x2, z2)) {
        ((false, false), _) => (x2, z2, 0),
        (_, (false, false)) => (x1, z1, 0),
        ((true, false), (true, false)) => (false, false, 0),
        ((false, true), (false, true)) => (false, false, 0),
        ((true, true), (true, true)) => (false, false, 0),
        ((true, false), (false, true)) => (true, true, 1),
        ((false, true), (true, false)) => (true, true, 3),
        ((true, false), (true, true)) => (false, true, 1),
        ((true, true), (true, false)) => (false, true, 3),
        ((false, true), (true, true)) => (true, false, 3),
        ((true, true), (false, true)) => (true, false, 1),
    }
}

fn reduce_pauli_row(row: &mut RepeatPauliRow, basis: &[Option<RepeatPauliRow>]) {
    for pivot in 0..basis.len() {
        if row.bit(pivot) {
            if let Some(basis_row) = &basis[pivot] {
                row.multiply_assign(basis_row);
            }
        }
    }
}

fn repeat_boundary_state(tableau: &PackedInverseTableau) -> RepeatBoundaryState {
    let snapshot = tableau.canonical_snapshot();
    let n = snapshot.num_qubits;
    let mut stabilizer_basis: Vec<Option<RepeatPauliRow>> = vec![None; 2 * n];
    // Measurement/reset rounds can change destabilizer bookkeeping while leaving
    // the quantum stabilizer state unchanged. Fold only on exact stabilizer
    // state equality, never on produced measurement bits alone.
    for row in n..2 * n {
        let mut candidate = RepeatPauliRow {
            x: snapshot.x[row].clone(),
            z: snapshot.z[row].clone(),
            phase: snapshot.phase[row],
        };
        reduce_pauli_row(&mut candidate, &stabilizer_basis);
        if let Some(pivot) = candidate.leading_bit() {
            for basis_row in stabilizer_basis.iter_mut().flatten() {
                if basis_row.bit(pivot) {
                    basis_row.multiply_assign(&candidate);
                }
            }
            stabilizer_basis[pivot] = Some(candidate);
        } else {
            debug_assert!(candidate.is_identity());
            debug_assert_eq!(
                candidate.phase % 4,
                0,
                "dependent stabilizer row reduced to non-identity phase",
            );
        }
    }
    RepeatBoundaryState {
        num_qubits: n,
        stabilizer_basis,
    }
}

fn packed_reference_instrs(
    tableau: &mut PackedInverseTableau,
    instrs: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<ReferenceSampleTree, SamplingFallbackReason> {
    let mut tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 1,
    };
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
                let mut bits = Vec::new();
                packed_reference_op(tableau, &mut bits, name, targets, counters)?;
                append_tree_bits(&mut tree, bits);
            }
            StimInstr::Repeat { count, body } => {
                add_repeat_counter(&mut counters.expanded_repeat_iterations, *count);
                let child = packed_reference_repeat(tableau, *count, body, counters)?;
                append_tree_child(&mut tree, child);
            }
        }
    }
    Ok(tree.simplified())
}

fn packed_reference_repeat(
    tableau: &mut PackedInverseTableau,
    count: u64,
    body: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<ReferenceSampleTree, SamplingFallbackReason> {
    if count < REPEAT_FOLD_THRESHOLD {
        return packed_reference_repeat_without_skip(tableau, count, body, counters);
    }

    let mut tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 1,
    };
    let mut seen = vec![(repeat_boundary_state(tableau), 0_u64, 0_usize)];
    let mut iteration = 0_u64;

    while iteration < count {
        add_repeat_counter(&mut counters.executed_repeat_iterations, 1);
        let child = packed_reference_instrs(tableau, body, counters)?;
        append_tree_child(&mut tree, child);
        iteration += 1;

        let current_state = repeat_boundary_state(tableau);
        if let Some((previous_iteration, child_start)) = seen
            .iter()
            .find(|(state, _, _)| state == &current_state)
            .map(|(_, previous_iteration, child_start)| (*previous_iteration, *child_start))
        {
            let period = iteration - previous_iteration;
            let remaining = count - iteration;
            let whole_cycles = remaining / period;
            if whole_cycles > 0 {
                let total_period_repetitions = whole_cycles + 1;
                let period_children = tree.suffix_children[child_start..].to_vec();
                tree.suffix_children.truncate(child_start);
                append_tree_child(
                    &mut tree,
                    ReferenceSampleTree {
                        prefix_bits: Vec::new(),
                        suffix_children: period_children,
                        repetitions: total_period_repetitions,
                    }
                    .simplified(),
                );
                let skipped = whole_cycles * period;
                let nested_skipped = skipped.saturating_mul(logical_repeat_iterations(body));
                add_repeat_counter(&mut counters.expanded_repeat_iterations, nested_skipped);
                add_repeat_counter(
                    &mut counters.skipped_repeat_iterations,
                    skipped.saturating_add(nested_skipped),
                );
                iteration += skipped;
            }
            break;
        }

        seen.push((current_state, iteration, tree.suffix_children.len()));
    }

    while iteration < count {
        add_repeat_counter(&mut counters.executed_repeat_iterations, 1);
        let child = packed_reference_instrs(tableau, body, counters)?;
        append_tree_child(&mut tree, child);
        iteration += 1;
    }

    Ok(tree.simplified())
}

fn packed_reference_repeat_without_skip(
    tableau: &mut PackedInverseTableau,
    count: u64,
    body: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<ReferenceSampleTree, SamplingFallbackReason> {
    let mut tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 1,
    };
    for _ in 0..count {
        add_repeat_counter(&mut counters.executed_repeat_iterations, 1);
        let child = packed_reference_instrs(tableau, body, counters)?;
        append_tree_child(&mut tree, child);
    }
    Ok(tree.simplified())
}

fn packed_reference_op(
    tableau: &mut PackedInverseTableau,
    measurements: &mut Vec<bool>,
    name: &str,
    targets: &[StimTarget],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<(), SamplingFallbackReason> {
    if is_loss_operation(name) {
        return Err(SamplingFallbackReason::Loss);
    }
    if is_feedback_operation(name, targets) {
        return Err(SamplingFallbackReason::MeasurementRecordFeedback);
    }
    if is_sweep_dependent_operation(name, targets) {
        return Err(SamplingFallbackReason::SweepDependent);
    }
    if is_noiselessly_skipped_or_metadata_operation(name) {
        return Ok(());
    }
    if is_measurement_reset_operation(name) {
        counters.measurement_reset_batches += 1;
    }

    match name {
        "H" => {
            for q in qubits(targets)? {
                tableau.h(q);
            }
        }
        "S" | "SQRT_Z" => {
            for q in qubits(targets)? {
                tableau.s(q);
            }
        }
        "S_DAG" | "SQRT_Z_DAG" => {
            for q in qubits(targets)? {
                tableau.s_dag(q);
            }
        }
        "X" => {
            for q in qubits(targets)? {
                tableau.x_gate(q);
            }
        }
        "Y" => {
            for q in qubits(targets)? {
                tableau.y_gate(q);
            }
        }
        "Z" => {
            for q in qubits(targets)? {
                tableau.z_gate(q);
            }
        }
        "CX" | "CNOT" | "ZCX" => {
            for (control, target) in qubit_pairs(targets)? {
                tableau.cx(control, target);
            }
        }
        "M" | "MZ" => {
            measurements.extend(
                tableau.measure_z_many_biased_with_counters(
                    &qubits_with_inversion(targets)?,
                    counters,
                ),
            );
        }
        "MX" => {
            measurements.extend(
                tableau.measure_x_many_biased_with_counters(
                    &qubits_with_inversion(targets)?,
                    counters,
                ),
            );
        }
        "MY" => {
            measurements.extend(
                tableau.measure_y_many_biased_with_counters(
                    &qubits_with_inversion(targets)?,
                    counters,
                ),
            );
        }
        "MR" | "MRZ" => {
            measurements.extend(tableau.measure_reset_z_many_biased_with_counters(
                &qubits_with_inversion(targets)?,
                counters,
            ));
        }
        "MRX" => {
            measurements.extend(tableau.measure_reset_x_many_biased_with_counters(
                &qubits_with_inversion(targets)?,
                counters,
            ));
        }
        "MRY" => {
            measurements.extend(tableau.measure_reset_y_many_biased_with_counters(
                &qubits_with_inversion(targets)?,
                counters,
            ));
        }
        "R" | "RZ" => {
            tableau.reset_z_many_biased_with_counters(&qubits(targets)?, counters);
        }
        "RX" => {
            tableau.reset_x_many_biased_with_counters(&qubits(targets)?, counters);
        }
        "RY" => {
            tableau.reset_y_many_biased_with_counters(&qubits(targets)?, counters);
        }
        _ => {
            return Err(SamplingFallbackReason::UnsupportedOperation(
                name.to_string(),
            ));
        }
    }

    Ok(())
}

fn qubits(targets: &[StimTarget]) -> Result<Vec<usize>, SamplingFallbackReason> {
    targets.iter().map(expect_qubit).collect()
}

fn qubits_with_inversion(
    targets: &[StimTarget],
) -> Result<Vec<(usize, bool)>, SamplingFallbackReason> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => Ok((*q as usize, false)),
            StimTarget::QubitInv(q) => Ok((*q as usize, true)),
            _ => Err(unsupported_target()),
        })
        .collect()
}

fn qubit_pairs(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, SamplingFallbackReason> {
    if targets.len() % 2 != 0 {
        return Err(SamplingFallbackReason::UnsupportedOperation(
            "odd target count".to_string(),
        ));
    }
    let mut pairs = Vec::new();
    let mut iter = targets.iter();
    while let (Some(control), Some(target)) = (iter.next(), iter.next()) {
        pairs.push((expect_qubit(control)?, expect_qubit(target)?));
    }
    Ok(pairs)
}

fn expect_qubit(target: &StimTarget) -> Result<usize, SamplingFallbackReason> {
    match target {
        StimTarget::Qubit(q) => Ok(*q as usize),
        _ => Err(unsupported_target()),
    }
}

fn unsupported_target() -> SamplingFallbackReason {
    SamplingFallbackReason::UnsupportedOperation("target".to_string())
}

fn is_measurement_reset_operation(name: &str) -> bool {
    matches!(
        name,
        "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" | "R" | "RZ" | "RX" | "RY"
    )
}

fn is_loss_operation(name: &str) -> bool {
    matches!(
        name,
        "LOSS"
            | "ML"
            | "MXL"
            | "MYL"
            | "MZL"
            | "MRL"
            | "MRXL"
            | "MRYL"
            | "MRZL"
            | "HERALDED_ERASE"
            | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn is_feedback_operation(name: &str, targets: &[StimTarget]) -> bool {
    matches!(name, "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ")
        && targets
            .chunks_exact(2)
            .any(|pair| matches!(pair, [StimTarget::Rec(_), StimTarget::Qubit(_)]))
}

fn is_sweep_dependent_operation(name: &str, targets: &[StimTarget]) -> bool {
    targets
        .iter()
        .any(|target| matches!(target, StimTarget::Sweep(_)))
        && !is_noiselessly_skipped_or_metadata_operation(name)
}

fn is_noiselessly_skipped_or_metadata_operation(name: &str) -> bool {
    matches!(
        name,
        "I" | "I_ERROR"
            | "II_ERROR"
            | "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "CORRELATED_ERROR"
            | "E"
            | "ELSE_CORRELATED_ERROR"
            | "TICK"
            | "QUBIT_COORDS"
            | "SHIFT_COORDS"
            | "DETECTOR"
            | "OBSERVABLE_INCLUDE"
    )
}
