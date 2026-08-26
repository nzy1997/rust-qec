use crate::data_path::{build_reference_sample, ReferenceSampleMode};
use crate::ir::StimInstr;
use crate::measurement_transform::{CheckedMeasurementLayout, MeasurementTransformLimits};
use crate::sim::bit_table::BitTable;
use std::collections::HashMap;

pub struct M2dOutput {
    pub detections: BitTable,
    pub observable_flips: BitTable,
}

/// One loss-independent parity check for a single shot.
///
/// `source_detectors` identifies the original circuit detectors whose XOR
/// defines this check. A singleton is an unaffected original detector; two or
/// more sources form a supercheck.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossAwareDetectorCheck {
    pub source_detectors: Vec<usize>,
    pub value: bool,
}

impl LossAwareDetectorCheck {
    pub fn is_supercheck(&self) -> bool {
        self.source_detectors.len() > 1
    }
}

/// Shot-conditioned detector information after removing lost measurement
/// degrees of freedom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossAwareDetectorShot {
    /// Measurement-record indices known to be lost for this shot.
    pub lost_measurements: Vec<usize>,
    /// Fixed-width validity mask for the original circuit detectors.
    pub detector_valid: Vec<bool>,
    /// A maximal independent basis of loss-independent detector checks.
    pub checks: Vec<LossAwareDetectorCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossAwareM2dOutput {
    pub shots: Vec<LossAwareDetectorShot>,
}

/// Resource limits for shot-conditioned loss elimination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossAwareM2dLimits {
    pub max_detectors_per_shot: usize,
    pub max_shots_per_batch: usize,
    pub max_batch_table_bits: u64,
    pub max_pivots_per_shot: usize,
    pub max_elimination_steps: u64,
    pub max_materialized_terms: u64,
}

impl Default for LossAwareM2dLimits {
    fn default() -> Self {
        Self {
            max_detectors_per_shot: 10_000_000,
            max_shots_per_batch: 1_000_000,
            max_batch_table_bits: 1_000_000_000,
            max_pivots_per_shot: 1_000_000,
            max_elimination_steps: 100_000_000,
            max_materialized_terms: 100_000_000,
        }
    }
}

/// Reusable loss-aware measurement-to-detector transform.
///
/// Construction validates the circuit and computes its noiseless reference
/// sample once. [`Self::convert`] can then process many measurement batches
/// without recompiling either artifact.
pub struct CompiledLossAwareM2d {
    layout: CheckedMeasurementLayout,
    reference_sample: Vec<bool>,
    limits: LossAwareM2dLimits,
}

impl CompiledLossAwareM2d {
    pub fn new(instrs: &[StimInstr]) -> Result<Self, String> {
        Self::with_limits(instrs, LossAwareM2dLimits::default())
    }

    pub fn with_limits(instrs: &[StimInstr], limits: LossAwareM2dLimits) -> Result<Self, String> {
        let layout = CheckedMeasurementLayout::from_circuit_with_limits(
            instrs,
            MeasurementTransformLimits::default(),
        )
        .map_err(|err| err.to_string())?;
        validate_loss_flag_references(&layout)?;
        let reference_sample =
            build_reference_sample(instrs, ReferenceSampleMode::SimulateNoiseless)?;
        Ok(Self {
            layout,
            reference_sample,
            limits,
        })
    }

    pub fn layout(&self) -> &CheckedMeasurementLayout {
        &self.layout
    }

    pub fn convert(&self, meas_table: &BitTable) -> Result<LossAwareM2dOutput, String> {
        preflight_loss_aware_tables(&self.layout, meas_table, self.limits)?;
        let loss_mask = loss_mask_from_loss_visible_measurements(&self.layout, meas_table)?;
        let raw = measurements_to_detections_with_layout_and_reference(
            &self.layout,
            meas_table,
            &self.reference_sample,
        )?;
        build_loss_aware_output(
            &self.layout,
            &raw,
            &loss_mask,
            meas_table.num_minor(),
            self.limits,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct M2dOptions {
    pub reference_sample_mode: ReferenceSampleMode,
    pub ran_without_feedback: bool,
}

impl Default for M2dOptions {
    fn default() -> Self {
        Self {
            reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
            ran_without_feedback: false,
        }
    }
}

/// Convert raw measurement bits to detection events.
/// meas_table: BitTable(num_measurements, num_shots)
/// Returns M2dOutput with detections and observable_flips BitTables.
pub fn measurements_to_detections(
    instrs: &[StimInstr],
    meas_table: &BitTable,
) -> Result<M2dOutput, String> {
    measurements_to_detections_with_options(instrs, meas_table, None, M2dOptions::default())
}

pub fn measurements_to_detections_with_options(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    sweep_table: Option<&BitTable>,
    options: M2dOptions,
) -> Result<M2dOutput, String> {
    let normalization = if options.ran_without_feedback {
        Some(crate::transforms::normalize_feedbackless_m2d(instrs)?)
    } else {
        None
    };
    let work_instrs = normalization
        .as_ref()
        .map(|normalized| normalized.circuit.as_slice())
        .unwrap_or(instrs);
    let empty_corrections: &[Vec<usize>] = &[];
    let measurement_corrections = normalization
        .as_ref()
        .map(|normalized| normalized.measurement_corrections.as_slice())
        .unwrap_or(empty_corrections);

    let shared_reference = match options.reference_sample_mode {
        ReferenceSampleMode::SimulateNoiseless if sweep_table.is_none() => Some(
            build_reference_sample(work_instrs, ReferenceSampleMode::SimulateNoiseless)?,
        ),
        ReferenceSampleMode::AssumeAllZero => {
            Some(vec![false; crate::stats::num_measurements(work_instrs)])
        }
        ReferenceSampleMode::SimulateNoiseless => None,
    };
    measurements_to_detections_impl(
        work_instrs,
        meas_table,
        sweep_table,
        shared_reference.as_deref(),
        measurement_corrections,
    )
}

/// Convert measurements containing interleaved loss-visible `flag,value`
/// records into shot-conditioned detectors and superchecks.
///
/// A set flag marks its paired value record as lost. Detectors and observables
/// may not use flag records as parity terms because flags are metadata, not
/// stabilizer measurements.
pub fn measurements_to_loss_aware_detections(
    instrs: &[StimInstr],
    meas_table: &BitTable,
) -> Result<LossAwareM2dOutput, String> {
    let layout = CheckedMeasurementLayout::from_circuit_with_limits(
        instrs,
        MeasurementTransformLimits::default(),
    )
    .map_err(|err| err.to_string())?;
    validate_loss_flag_references(&layout)?;
    preflight_loss_aware_tables(&layout, meas_table, LossAwareM2dLimits::default())?;
    let loss_mask = loss_mask_from_loss_visible_measurements(&layout, meas_table)?;
    measurements_to_loss_aware_detections_with_layout(
        instrs,
        meas_table,
        &loss_mask,
        &layout,
        LossAwareM2dLimits::default(),
    )
}

/// Convert measurements with an explicit, same-shape measurement-loss mask.
///
/// This entry point supports circuits whose loss metadata arrives out of band.
/// A `true` mask entry means the corresponding measurement value is unknown;
/// its stored 0/1 bit is only a placeholder.
pub fn measurements_to_loss_aware_detections_with_loss_mask(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    measurement_loss_mask: &BitTable,
) -> Result<LossAwareM2dOutput, String> {
    measurements_to_loss_aware_detections_with_loss_mask_and_limits(
        instrs,
        meas_table,
        measurement_loss_mask,
        LossAwareM2dLimits::default(),
    )
}

/// Explicit-mask conversion with caller-selected sparse-elimination limits.
pub fn measurements_to_loss_aware_detections_with_loss_mask_and_limits(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    measurement_loss_mask: &BitTable,
    limits: LossAwareM2dLimits,
) -> Result<LossAwareM2dOutput, String> {
    let layout = CheckedMeasurementLayout::from_circuit_with_limits(
        instrs,
        MeasurementTransformLimits::default(),
    )
    .map_err(|err| err.to_string())?;
    validate_loss_flag_references(&layout)?;
    measurements_to_loss_aware_detections_with_layout(
        instrs,
        meas_table,
        measurement_loss_mask,
        &layout,
        limits,
    )
}

fn measurements_to_loss_aware_detections_with_layout(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    measurement_loss_mask: &BitTable,
    layout: &CheckedMeasurementLayout,
    limits: LossAwareM2dLimits,
) -> Result<LossAwareM2dOutput, String> {
    validate_loss_mask_shape(meas_table, measurement_loss_mask, layout.num_measurements())?;
    preflight_loss_aware_tables(layout, meas_table, limits)?;
    let merged_loss_mask = merge_embedded_loss_flags(layout, meas_table, measurement_loss_mask)?;

    let raw = measurements_to_detections(instrs, meas_table)?;
    build_loss_aware_output(
        layout,
        &raw,
        &merged_loss_mask,
        meas_table.num_minor(),
        limits,
    )
}

fn build_loss_aware_output(
    layout: &CheckedMeasurementLayout,
    raw: &M2dOutput,
    loss_mask: &BitTable,
    num_shots: usize,
    limits: LossAwareM2dLimits,
) -> Result<LossAwareM2dOutput, String> {
    let mut shots = Vec::new();
    shots
        .try_reserve_exact(num_shots)
        .map_err(|_| "loss-aware output allocation failed".to_string())?;
    let mut budget = LossAwareWorkBudget::new(limits);
    for shot in 0..num_shots {
        shots.push(build_loss_aware_shot(
            layout.detector_rows(),
            &raw.detections,
            loss_mask,
            shot,
            &mut budget,
        )?);
    }
    Ok(LossAwareM2dOutput { shots })
}

fn preflight_loss_aware_tables(
    layout: &CheckedMeasurementLayout,
    meas_table: &BitTable,
    limits: LossAwareM2dLimits,
) -> Result<(), String> {
    let shots = meas_table.num_minor();
    if shots > limits.max_shots_per_batch {
        return Err("loss-aware input exceeded max_shots_per_batch".to_string());
    }
    if layout.num_detectors() > limits.max_detectors_per_shot {
        return Err("loss-aware output exceeded max_detectors_per_shot".to_string());
    }
    for (name, width) in [
        ("measurement", layout.num_measurements()),
        ("detector", layout.num_detectors()),
        ("observable", layout.num_observables()),
    ] {
        let bits = u64::try_from(width)
            .ok()
            .and_then(|width| {
                u64::try_from(shots)
                    .ok()
                    .and_then(|shots| width.checked_mul(shots))
            })
            .ok_or_else(|| format!("loss-aware {name} table exceeded max_batch_table_bits"))?;
        if bits > limits.max_batch_table_bits {
            return Err(format!(
                "loss-aware {name} table exceeded max_batch_table_bits"
            ));
        }
    }
    Ok(())
}

fn merge_embedded_loss_flags(
    layout: &CheckedMeasurementLayout,
    meas_table: &BitTable,
    explicit_mask: &BitTable,
) -> Result<BitTable, String> {
    let mut merged = BitTable::try_new(explicit_mask.num_major(), explicit_mask.num_minor())
        .map_err(|err| format!("failed to allocate merged measurement loss mask: {err:?}"))?;
    for measurement in 0..explicit_mask.num_major() {
        merged
            .row_words_mut(measurement)
            .copy_from_slice(explicit_mask.row_words(measurement));
    }
    for pair in layout.loss_visible_measurements() {
        for shot in 0..explicit_mask.num_minor() {
            if explicit_mask.get(pair.flag, shot) {
                return Err(format!(
                    "measurement_loss_mask marks loss-flag record {} as lost; mark its value record {} instead",
                    pair.flag, pair.value
                ));
            }
            if meas_table.get(pair.flag, shot) {
                merged.set(pair.value, shot, true);
            }
        }
    }
    Ok(merged)
}

fn loss_mask_from_loss_visible_measurements(
    layout: &CheckedMeasurementLayout,
    meas_table: &BitTable,
) -> Result<BitTable, String> {
    if meas_table.num_major() != layout.num_measurements() {
        return Err(format!(
            "meas_table has {} bits but circuit has {} measurements",
            meas_table.num_major(),
            layout.num_measurements()
        ));
    }
    let mut mask = BitTable::try_new(meas_table.num_major(), meas_table.num_minor())
        .map_err(|err| format!("failed to allocate measurement loss mask: {err:?}"))?;
    for pair in layout.loss_visible_measurements() {
        for shot in 0..meas_table.num_minor() {
            if meas_table.get(pair.flag, shot) {
                mask.set(pair.value, shot, true);
            }
        }
    }
    Ok(mask)
}

fn validate_loss_flag_references(layout: &CheckedMeasurementLayout) -> Result<(), String> {
    let mut flag_to_value = HashMap::<usize, usize>::new();
    flag_to_value
        .try_reserve(layout.loss_visible_measurements().len())
        .map_err(|_| "loss-flag index allocation failed".to_string())?;
    for pair in layout.loss_visible_measurements() {
        flag_to_value.insert(pair.flag, pair.value);
    }
    for (detector, row) in layout.detector_rows().iter().enumerate() {
        if let Some(&flag) = row.iter().find(|term| flag_to_value.contains_key(term)) {
            return Err(format!(
                "detector {detector} references loss-flag record {flag}; detectors must reference measurement values"
            ));
        }
    }
    for (observable, row) in layout.observable_rows().iter().enumerate() {
        if let Some(&flag) = row.iter().find(|term| flag_to_value.contains_key(term)) {
            return Err(format!(
                "observable {observable} references loss-flag record {flag}; observables must reference measurement values"
            ));
        }
    }
    Ok(())
}

fn validate_loss_mask_shape(
    meas_table: &BitTable,
    loss_mask: &BitTable,
    expected_measurements: usize,
) -> Result<(), String> {
    if meas_table.num_major() != expected_measurements {
        return Err(format!(
            "meas_table has {} bits but circuit has {expected_measurements} measurements",
            meas_table.num_major()
        ));
    }
    if loss_mask.num_major() != expected_measurements {
        return Err(format!(
            "measurement_loss_mask has {} bits but circuit has {expected_measurements} measurements",
            loss_mask.num_major()
        ));
    }
    if loss_mask.num_minor() != meas_table.num_minor() {
        return Err(format!(
            "measurement_loss_mask has {} shots but meas_table has {} shots",
            loss_mask.num_minor(),
            meas_table.num_minor()
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct LossPivot {
    measurement_terms: Vec<usize>,
    detector_sources: Vec<usize>,
}

struct LossAwareWorkBudget {
    limits: LossAwareM2dLimits,
    elimination_steps: u64,
    materialized_terms: u64,
}

impl LossAwareWorkBudget {
    fn new(limits: LossAwareM2dLimits) -> Self {
        Self {
            limits,
            elimination_steps: 0,
            materialized_terms: 0,
        }
    }

    fn charge_elimination_step(&mut self) -> Result<(), String> {
        self.elimination_steps = self
            .elimination_steps
            .checked_add(1)
            .ok_or_else(|| "loss-aware elimination exceeded max_elimination_steps".to_string())?;
        if self.elimination_steps > self.limits.max_elimination_steps {
            return Err("loss-aware elimination exceeded max_elimination_steps".to_string());
        }
        Ok(())
    }

    fn charge_terms(&mut self, terms: usize) -> Result<(), String> {
        let terms = u64::try_from(terms)
            .map_err(|_| "loss-aware elimination exceeded max_materialized_terms".to_string())?;
        self.materialized_terms = self
            .materialized_terms
            .checked_add(terms)
            .ok_or_else(|| "loss-aware elimination exceeded max_materialized_terms".to_string())?;
        if self.materialized_terms > self.limits.max_materialized_terms {
            return Err("loss-aware elimination exceeded max_materialized_terms".to_string());
        }
        Ok(())
    }
}

fn build_loss_aware_shot(
    detector_rows: &[Vec<usize>],
    raw_detections: &BitTable,
    loss_mask: &BitTable,
    shot: usize,
    budget: &mut LossAwareWorkBudget,
) -> Result<LossAwareDetectorShot, String> {
    let mut lost_measurements = Vec::new();
    for measurement in 0..loss_mask.num_major() {
        if loss_mask.get(measurement, shot) {
            budget.charge_terms(1)?;
            lost_measurements
                .try_reserve(1)
                .map_err(|_| "loss-aware lost-measurement allocation failed".to_string())?;
            lost_measurements.push(measurement);
        }
    }
    if detector_rows.len() > budget.limits.max_detectors_per_shot {
        return Err("loss-aware output exceeded max_detectors_per_shot".to_string());
    }
    let mut detector_valid = Vec::new();
    detector_valid
        .try_reserve_exact(detector_rows.len())
        .map_err(|_| "loss-aware detector-valid allocation failed".to_string())?;
    let mut pivots = HashMap::<usize, LossPivot>::new();
    let mut checks = Vec::new();

    for (detector, row) in detector_rows.iter().enumerate() {
        let mut measurement_terms = Vec::new();
        for &measurement in row {
            if loss_mask.get(measurement, shot) {
                budget.charge_terms(1)?;
                measurement_terms
                    .try_reserve(1)
                    .map_err(|_| "loss-aware pivot allocation failed".to_string())?;
                measurement_terms.push(measurement);
            }
        }
        detector_valid.push(measurement_terms.is_empty());
        let mut detector_sources = Vec::new();
        detector_sources
            .try_reserve_exact(1)
            .map_err(|_| "loss-aware detector-source allocation failed".to_string())?;
        detector_sources.push(detector);
        budget.charge_terms(1)?;
        let mut became_pivot = false;

        while let Some(&pivot_measurement) = measurement_terms.first() {
            let Some(pivot) = pivots.get(&pivot_measurement) else {
                if pivots.len() >= budget.limits.max_pivots_per_shot {
                    return Err("loss-aware elimination exceeded max_pivots_per_shot".to_string());
                }
                pivots
                    .try_reserve(1)
                    .map_err(|_| "loss-aware pivot-map allocation failed".to_string())?;
                pivots.insert(
                    pivot_measurement,
                    LossPivot {
                        measurement_terms: std::mem::take(&mut measurement_terms),
                        detector_sources: std::mem::take(&mut detector_sources),
                    },
                );
                became_pivot = true;
                break;
            };
            budget.charge_elimination_step()?;
            measurement_terms =
                checked_symmetric_difference(&measurement_terms, &pivot.measurement_terms, budget)?;
            detector_sources =
                checked_symmetric_difference(&detector_sources, &pivot.detector_sources, budget)?;
        }

        if !became_pivot {
            let value = detector_sources
                .iter()
                .fold(false, |acc, &source| acc ^ raw_detections.get(source, shot));
            checks
                .try_reserve(1)
                .map_err(|_| "loss-aware check allocation failed".to_string())?;
            checks.push(LossAwareDetectorCheck {
                source_detectors: detector_sources,
                value,
            });
        }
    }

    Ok(LossAwareDetectorShot {
        lost_measurements,
        detector_valid,
        checks,
    })
}

fn checked_symmetric_difference(
    left: &[usize],
    right: &[usize],
    budget: &mut LossAwareWorkBudget,
) -> Result<Vec<usize>, String> {
    let capacity = left
        .len()
        .checked_add(right.len())
        .ok_or_else(|| "loss-aware elimination exceeded max_materialized_terms".to_string())?;
    budget.charge_terms(capacity)?;
    let mut result = Vec::new();
    result
        .try_reserve(capacity)
        .map_err(|_| "loss-aware elimination allocation failed".to_string())?;
    let (mut a, mut b) = (0, 0);
    while a < left.len() || b < right.len() {
        match (left.get(a), right.get(b)) {
            (Some(&x), Some(&y)) if x == y => {
                a += 1;
                b += 1;
            }
            (Some(&x), Some(&y)) if x < y => {
                result.push(x);
                a += 1;
            }
            (Some(_), Some(&y)) => {
                result.push(y);
                b += 1;
            }
            (Some(&x), None) => {
                result.push(x);
                a += 1;
            }
            (None, Some(&y)) => {
                result.push(y);
                b += 1;
            }
            (None, None) => break,
        }
    }
    Ok(result)
}

pub(crate) fn measurements_to_detections_with_reference(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    reference_sample: &[bool],
) -> Result<M2dOutput, String> {
    measurements_to_detections_impl(instrs, meas_table, None, Some(reference_sample), &[])
}

fn measurements_to_detections_impl(
    work_instrs: &[StimInstr],
    meas_table: &BitTable,
    sweep_table: Option<&BitTable>,
    shared_reference: Option<&[bool]>,
    measurement_corrections: &[Vec<usize>],
) -> Result<M2dOutput, String> {
    let layout = CheckedMeasurementLayout::from_circuit_with_limits(
        work_instrs,
        MeasurementTransformLimits::default(),
    )
    .map_err(|err| err.to_string())?;
    measurements_to_detections_with_layout(
        work_instrs,
        &layout,
        meas_table,
        sweep_table,
        shared_reference,
        measurement_corrections,
    )
}

fn measurements_to_detections_with_layout(
    work_instrs: &[StimInstr],
    layout: &CheckedMeasurementLayout,
    meas_table: &BitTable,
    sweep_table: Option<&BitTable>,
    shared_reference: Option<&[bool]>,
    measurement_corrections: &[Vec<usize>],
) -> Result<M2dOutput, String> {
    let n_meas = layout.num_measurements();
    if let Some(reference) = shared_reference {
        if reference.len() != n_meas {
            return Err(reference_measurement_count_mismatch(
                reference.len(),
                n_meas,
            ));
        }
    }
    let n_shots = meas_table.num_minor();

    if meas_table.num_major() != n_meas {
        return Err(format!(
            "meas_table has {} bits but circuit has {} measurements",
            meas_table.num_major(),
            n_meas
        ));
    }
    if let Some(sweep_table) = sweep_table {
        if sweep_table.num_minor() != n_shots {
            return Err(format!(
                "sweep shots {} do not match measurement shots {}",
                sweep_table.num_minor(),
                n_shots
            ));
        }
    }

    let n_dets = layout.num_detectors();
    let n_obs = layout.num_observables();

    let mut dets = BitTable::try_new(n_dets, n_shots)
        .map_err(|err| format!("failed to allocate detection table: {err:?}"))?;
    let mut obs = BitTable::try_new(n_obs, n_shots)
        .map_err(|err| format!("failed to allocate observable table: {err:?}"))?;

    for shot in 0..n_shots {
        let per_shot_reference;
        let reference = if let Some(reference) = shared_reference {
            reference
        } else {
            let sweep_table = sweep_table.expect("validated above");
            let sweep_row: Vec<bool> = (0..sweep_table.num_major())
                .map(|i| sweep_table.get(i, shot))
                .collect();
            per_shot_reference =
                crate::executor::reference_sample_with_sweep_bits(work_instrs, Some(&sweep_row))?;
            per_shot_reference.as_slice()
        };
        let mut flips = Vec::with_capacity(n_meas);
        for i in 0..n_meas {
            let mut flip = meas_table.get(i, shot) ^ reference[i];
            if let Some(extra_terms) = measurement_corrections.get(i) {
                for &j in extra_terms {
                    flip ^= flips[j];
                }
            }
            flips.push(flip);
        }

        for (d, rec_offsets) in layout.detector_rows().iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val {
                dets.set(d, shot, true);
            }
        }
        for (o, rec_offsets) in layout.observable_rows().iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val {
                obs.set(o, shot, true);
            }
        }
    }

    Ok(M2dOutput {
        detections: dets,
        observable_flips: obs,
    })
}

fn measurements_to_detections_with_layout_and_reference(
    layout: &CheckedMeasurementLayout,
    meas_table: &BitTable,
    reference_sample: &[bool],
) -> Result<M2dOutput, String> {
    measurements_to_detections_with_layout(
        &[],
        layout,
        meas_table,
        None,
        Some(reference_sample),
        &[],
    )
}

fn reference_measurement_count_mismatch(reference_len: usize, n_meas: usize) -> String {
    format!("reference has {reference_len} bits but circuit has {n_meas} measurements")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bit_rank(mut rows: Vec<usize>, columns: usize) -> usize {
        let mut rank = 0;
        for column in 0..columns {
            let Some(pivot) = (rank..rows.len()).find(|&row| (rows[row] >> column) & 1 == 1) else {
                continue;
            };
            rows.swap(rank, pivot);
            for row in 0..rows.len() {
                if row != rank && (rows[row] >> column) & 1 == 1 {
                    rows[row] ^= rows[rank];
                }
            }
            rank += 1;
        }
        rank
    }

    #[test]
    fn precomputed_reference_is_used_instead_of_rebuilding_from_the_circuit() {
        let instrs = crate::parser::parse_lines("X 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
        let mut measurements = BitTable::try_new(1, 1).unwrap();
        measurements.set(0, 0, true);

        let output =
            measurements_to_detections_with_reference(&instrs, &measurements, &[false]).unwrap();

        assert!(output.detections.get(0, 0));
    }

    #[test]
    fn reference_measurement_count_mismatch_is_actionable() {
        assert_eq!(
            reference_measurement_count_mismatch(2, 3),
            "reference has 2 bits but circuit has 3 measurements"
        );
    }

    #[test]
    fn exhaustive_small_matrices_return_a_complete_independent_left_kernel() {
        const DETECTORS: usize = 3;
        const MEASUREMENTS: usize = 3;
        let raw = BitTable::new(DETECTORS, 1);

        for matrix_bits in 0usize..(1 << (DETECTORS * MEASUREMENTS)) {
            let detector_rows: Vec<Vec<usize>> = (0..DETECTORS)
                .map(|detector| {
                    (0..MEASUREMENTS)
                        .filter(|&measurement| {
                            let bit = detector * MEASUREMENTS + measurement;
                            (matrix_bits >> bit) & 1 == 1
                        })
                        .collect()
                })
                .collect();
            for lost_bits in 0usize..(1 << MEASUREMENTS) {
                let mut loss_mask = BitTable::new(MEASUREMENTS, 1);
                for measurement in 0..MEASUREMENTS {
                    loss_mask.set(measurement, 0, (lost_bits >> measurement) & 1 == 1);
                }
                let restricted_rows: Vec<usize> = detector_rows
                    .iter()
                    .map(|row| {
                        row.iter().fold(0, |bits, &measurement| {
                            if (lost_bits >> measurement) & 1 == 1 {
                                bits | (1 << measurement)
                            } else {
                                bits
                            }
                        })
                    })
                    .collect();
                let expected_dimension =
                    DETECTORS - bit_rank(restricted_rows.clone(), MEASUREMENTS);
                let mut budget = LossAwareWorkBudget::new(LossAwareM2dLimits::default());
                let output =
                    build_loss_aware_shot(&detector_rows, &raw, &loss_mask, 0, &mut budget)
                        .unwrap();

                assert_eq!(output.checks.len(), expected_dimension);
                let source_rows: Vec<usize> = output
                    .checks
                    .iter()
                    .map(|check| {
                        let combined_restricted = check
                            .source_detectors
                            .iter()
                            .fold(0, |bits, &detector| bits ^ restricted_rows[detector]);
                        assert_eq!(combined_restricted, 0);
                        check
                            .source_detectors
                            .iter()
                            .fold(0, |bits, &detector| bits | (1 << detector))
                    })
                    .collect();
                assert_eq!(bit_rank(source_rows, DETECTORS), expected_dimension);
            }
        }
    }
}
