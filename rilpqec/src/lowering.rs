use std::collections::BTreeSet;

use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};

use crate::error::IlpDecodeError;
use crate::problem::{ColumnTerm, LoweredDemProblem};

pub fn lower_dem_to_problem(dem: &DetectorErrorModel) -> Result<LoweredDemProblem, IlpDecodeError> {
    let num_detectors = dem.effective_num_detectors();
    let num_observables = dem.num_observables();
    let mut columns = Vec::new();
    let mut forced_syndrome = vec![false; num_detectors];
    let mut baseline_observables = vec![false; num_observables];

    visit_dem(
        dem.instructions(),
        0,
        &mut columns,
        &mut forced_syndrome,
        &mut baseline_observables,
    )?;

    Ok(LoweredDemProblem {
        num_detectors,
        num_observables,
        columns,
        forced_syndrome,
        baseline_observables,
    })
}

fn visit_dem(
    instrs: &[DemInstruction],
    detector_offset: usize,
    columns: &mut Vec<ColumnTerm>,
    forced_syndrome: &mut [bool],
    baseline_observables: &mut [bool],
) -> Result<usize, IlpDecodeError> {
    let mut offset = detector_offset;
    for instr in instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                push_error_term(
                    *probability,
                    targets,
                    offset,
                    columns,
                    forced_syndrome,
                    baseline_observables,
                )?;
            }
            DemInstruction::ShiftDetectors {
                detector_offset, ..
            } => {
                offset += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    offset = visit_dem(
                        body.instructions(),
                        offset,
                        columns,
                        forced_syndrome,
                        baseline_observables,
                    )?;
                }
            }
            DemInstruction::Detector { .. } | DemInstruction::LogicalObservable { .. } => {}
        }
    }
    Ok(offset)
}

fn push_error_term(
    probability: f64,
    targets: &[DemTarget],
    detector_offset: usize,
    columns: &mut Vec<ColumnTerm>,
    forced_syndrome: &mut [bool],
    baseline_observables: &mut [bool],
) -> Result<(), IlpDecodeError> {
    if !(0.0..=1.0).contains(&probability) {
        return Err(IlpDecodeError::InvalidProbability(probability));
    }

    let mut detectors = BTreeSet::new();
    let mut observables = BTreeSet::new();
    for target in targets {
        match target {
            DemTarget::Detector(det) => toggle(&mut detectors, detector_offset + det),
            DemTarget::Observable(obs) => toggle(&mut observables, *obs),
            DemTarget::Separator => {}
        }
    }

    let detectors: Vec<usize> = detectors.into_iter().collect();
    let observables: Vec<usize> = observables.into_iter().collect();

    if probability == 0.0 {
        return Ok(());
    }

    if probability == 1.0 {
        xor_indices(forced_syndrome, &detectors);
        xor_indices(baseline_observables, &observables);
        return Ok(());
    }

    let mut effective_probability = probability;
    if probability > 0.5 {
        xor_indices(forced_syndrome, &detectors);
        xor_indices(baseline_observables, &observables);
        effective_probability = 1.0 - probability;
    }

    if detectors.is_empty() {
        return Ok(());
    }

    columns.push(ColumnTerm {
        detectors,
        observables,
        weight: log_likelihood_weight(effective_probability),
    });
    Ok(())
}

fn log_likelihood_weight(probability: f64) -> f64 {
    let p = probability.clamp(1e-12, 1.0 - 1e-12);
    ((1.0 - p) / p).ln()
}

fn toggle(set: &mut BTreeSet<usize>, value: usize) {
    if !set.insert(value) {
        set.remove(&value);
    }
}

fn xor_indices(bits: &mut [bool], indices: &[usize]) {
    for &index in indices {
        bits[index] ^= true;
    }
}
