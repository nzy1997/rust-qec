use crate::compiled::SamplingFallbackReason;
use crate::ir::{StimInstr, StimTarget};
use crate::sim::packed_inverse_tableau::PackedInverseTableau;

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
    let mut measurements = Vec::new();
    let mut counters = ReferenceBuildPhaseCounters {
        measurement_bits: crate::stats::num_measurements(instrs),
        ..ReferenceBuildPhaseCounters::default()
    };
    packed_reference_instrs(&mut tableau, &mut measurements, instrs, &mut counters)?;
    Ok((measurements, counters))
}

fn packed_reference_instrs(
    tableau: &mut PackedInverseTableau,
    measurements: &mut Vec<bool>,
    instrs: &[StimInstr],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Result<(), SamplingFallbackReason> {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
                packed_reference_op(tableau, measurements, name, targets, counters)?;
            }
            StimInstr::Repeat { count, body } => {
                counters.expanded_repeat_iterations = counters
                    .expanded_repeat_iterations
                    .saturating_add(usize::try_from(*count).unwrap_or(usize::MAX));
                for _ in 0..*count {
                    packed_reference_instrs(tableau, measurements, body, counters)?;
                }
            }
        }
    }
    Ok(())
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
            for (q, inverted) in qubits_with_inversion(targets)? {
                measurements.push(tableau.measure_x_biased_with_counters(q, inverted, counters));
            }
        }
        "MY" => {
            for (q, inverted) in qubits_with_inversion(targets)? {
                measurements.push(tableau.measure_y_biased_with_counters(q, inverted, counters));
            }
        }
        "MR" | "MRZ" => {
            measurements.extend(tableau.measure_reset_z_many_biased_with_counters(
                &qubits_with_inversion(targets)?,
                counters,
            ));
        }
        "MRX" => {
            for (q, inverted) in qubits_with_inversion(targets)? {
                measurements
                    .push(tableau.measure_reset_x_biased_with_counters(q, inverted, counters));
            }
        }
        "MRY" => {
            for (q, inverted) in qubits_with_inversion(targets)? {
                measurements
                    .push(tableau.measure_reset_y_biased_with_counters(q, inverted, counters));
            }
        }
        "R" | "RZ" => {
            tableau.reset_z_many_biased_with_counters(&qubits(targets)?, counters);
        }
        "RX" => {
            for q in qubits(targets)? {
                tableau.reset_x_biased_with_counters(q, counters);
            }
        }
        "RY" => {
            for q in qubits(targets)? {
                tableau.reset_y_biased_with_counters(q, counters);
            }
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
