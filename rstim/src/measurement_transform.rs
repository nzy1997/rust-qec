use crate::data_path::{build_reference_sample, ReferenceSampleMode};
use crate::ir::{circuit_to_string, StimInstr, StimTarget};
use crate::sample_archive::format::{
    CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1, FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
    REFERENCE_SIMULATE_NOISELESS, TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
};
use crate::sim::bit_table::{checked_bit_table_storage_size, BitTable, BitTableAllocError};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementTransformLimits {
    pub max_measurements: u64,
    pub max_detectors: u64,
    pub max_observables: u64,
    pub max_repeat_depth: u64,
    pub max_expanded_instructions: u64,
    pub max_parity_terms: u64,
    pub max_shots_per_block: u64,
    pub max_transform_working_bytes: u64,
    pub max_block_working_bytes: u64,
}

impl Default for MeasurementTransformLimits {
    fn default() -> Self {
        Self {
            max_measurements: 10_000_000,
            max_detectors: 10_000_000,
            max_observables: 1_000_000,
            max_repeat_depth: 1_000,
            max_expanded_instructions: 100_000_000,
            max_parity_terms: 100_000_000,
            max_shots_per_block: crate::sample_archive::format::DEFAULT_MAX_SHOTS_PER_BLOCK,
            max_transform_working_bytes: 512 * 1024 * 1024,
            max_block_working_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasurementTransformError {
    UnsupportedSweep,
    InvalidRecordTarget { detail: String },
    LimitExceeded { limit: &'static str },
    ShapeMismatch { detail: String },
    Allocation(BitTableAllocError),
    Reference { detail: String },
}

impl fmt::Display for MeasurementTransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSweep => write!(f, "sweep-bit circuits are not supported"),
            Self::InvalidRecordTarget { detail } => write!(f, "invalid record target: {detail}"),
            Self::LimitExceeded { limit } => {
                write!(f, "measurement transform limit exceeded: {limit}")
            }
            Self::ShapeMismatch { detail } => {
                write!(f, "measurement transform shape mismatch: {detail}")
            }
            Self::Allocation(err) => write!(f, "BitTable allocation failed: {err:?}"),
            Self::Reference { detail } => {
                write!(f, "reference sample construction failed: {detail}")
            }
        }
    }
}

impl Error for MeasurementTransformError {}

impl From<BitTableAllocError> for MeasurementTransformError {
    fn from(value: BitTableAllocError) -> Self {
        Self::Allocation(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementTransformIdentity {
    pub circuit_sha256: [u8; 32],
    pub measurement_count: u64,
    pub detector_count: u64,
    pub observable_count: u64,
    pub detector_rank: u64,
    pub canonicalization_id: u16,
    pub fingerprint_id: u16,
    pub transform_algorithm_id: u16,
    pub reference_strategy_id: u16,
}

#[derive(Debug, Clone)]
pub struct EncodedMeasurementBlock {
    pub selected_detectors: BitTable,
    pub free_measurements: BitTable,
}

#[derive(Debug, Clone)]
pub struct DecodedSampleBlock {
    pub measurements: BitTable,
    pub detections: BitTable,
    pub observable_flips: BitTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossVisibleMeasurementPair {
    pub flag: usize,
    pub value: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMeasurementLayout {
    num_measurements: usize,
    detector_rows: Vec<Vec<usize>>,
    observable_rows: Vec<Vec<usize>>,
    loss_visible_measurements: Vec<LossVisibleMeasurementPair>,
    expanded_instructions: u64,
    parity_terms: u64,
    max_repeat_depth: u64,
}

impl CheckedMeasurementLayout {
    pub fn from_circuit_with_limits(
        instrs: &[StimInstr],
        limits: MeasurementTransformLimits,
    ) -> Result<Self, MeasurementTransformError> {
        let mut builder = LayoutBuilder::new(limits);
        builder.visit_instrs(instrs, 0)?;
        let num_measurements = usize_from_u64(builder.measurement_count, "max_measurements")?;
        Ok(Self {
            num_measurements,
            detector_rows: builder.detector_rows,
            observable_rows: builder.observable_rows,
            loss_visible_measurements: builder.loss_visible_measurements,
            expanded_instructions: builder.expanded_instructions,
            parity_terms: builder.parity_terms,
            max_repeat_depth: builder.max_repeat_depth,
        })
    }

    pub fn num_measurements(&self) -> usize {
        self.num_measurements
    }

    pub fn num_detectors(&self) -> usize {
        self.detector_rows.len()
    }

    pub fn num_observables(&self) -> usize {
        self.observable_rows.len()
    }

    pub fn detector_rows(&self) -> &[Vec<usize>] {
        &self.detector_rows
    }

    pub fn observable_rows(&self) -> &[Vec<usize>] {
        &self.observable_rows
    }

    /// Interleaved `flag,value` record pairs produced by loss-visible measurements.
    pub fn loss_visible_measurements(&self) -> &[LossVisibleMeasurementPair] {
        &self.loss_visible_measurements
    }

    pub fn expanded_instructions(&self) -> u64 {
        self.expanded_instructions
    }

    pub fn parity_terms(&self) -> u64 {
        self.parity_terms
    }

    pub fn max_repeat_depth(&self) -> u64 {
        self.max_repeat_depth
    }
}

#[derive(Debug, Clone)]
pub struct MeasurementTransform {
    limits: MeasurementTransformLimits,
    identity: MeasurementTransformIdentity,
    reference: Vec<bool>,
    detector_rows: Vec<Vec<usize>>,
    observable_rows: Vec<Vec<usize>>,
    selected_detector_rows: Vec<usize>,
    pivot_columns: Vec<usize>,
    free_columns: Vec<usize>,
    equations: Vec<EchelonEquation>,
    solve_order: Vec<usize>,
    expanded_instructions: u64,
    parity_terms: u64,
    max_repeat_depth: u64,
    transform_working_bytes: u64,
}

#[derive(Debug, Clone)]
struct EchelonEquation {
    pivot: usize,
    row_words: Vec<u64>,
    rhs_sources: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Elimination {
    selected_detector_rows: Vec<usize>,
    pivot_columns: Vec<usize>,
    free_columns: Vec<usize>,
    equations: Vec<EchelonEquation>,
    solve_order: Vec<usize>,
}

impl MeasurementTransform {
    pub fn from_circuit(instrs: &[StimInstr]) -> Result<Self, MeasurementTransformError> {
        Self::from_circuit_with_limits(instrs, MeasurementTransformLimits::default())
    }

    pub fn from_circuit_with_limits(
        instrs: &[StimInstr],
        limits: MeasurementTransformLimits,
    ) -> Result<Self, MeasurementTransformError> {
        if contains_sweep_target(instrs) {
            return Err(MeasurementTransformError::UnsupportedSweep);
        }

        let layout = CheckedMeasurementLayout::from_circuit_with_limits(instrs, limits)?;
        let m = layout.num_measurements();
        let d = layout.num_detectors();
        let l = layout.num_observables();
        let worst_rank = m.min(d);
        let preflight_bytes =
            estimate_transform_working_bytes(m, d, l, worst_rank, layout.parity_terms())?;
        enforce_bytes(
            "max_transform_working_bytes",
            preflight_bytes,
            limits.max_transform_working_bytes,
        )?;

        let reference = build_reference_sample(instrs, ReferenceSampleMode::SimulateNoiseless)
            .map_err(|err| MeasurementTransformError::Reference { detail: err })?;
        if reference.len() != m {
            return Err(MeasurementTransformError::ShapeMismatch {
                detail: format!("reference has {} bits but layout has {m}", reference.len()),
            });
        }

        let mut h = BitTable::try_new(d, m)?;
        for (row, terms) in layout.detector_rows().iter().enumerate() {
            for &col in terms {
                h.set(row, col, true);
            }
        }
        let mut g = BitTable::try_new(l, m)?;
        for (row, terms) in layout.observable_rows().iter().enumerate() {
            for &col in terms {
                g.set(row, col, true);
            }
        }

        let elimination = eliminate_detector_rows(&h, m);
        drop(g);
        let actual_bytes = estimate_transform_working_bytes(
            m,
            d,
            l,
            elimination.pivot_columns.len(),
            layout.parity_terms(),
        )?;
        enforce_bytes(
            "max_transform_working_bytes",
            actual_bytes,
            limits.max_transform_working_bytes,
        )?;

        let canonical = circuit_to_string(instrs);
        let circuit_sha256 = Sha256::digest(canonical.as_bytes()).into();
        let identity = MeasurementTransformIdentity {
            circuit_sha256,
            measurement_count: m as u64,
            detector_count: d as u64,
            observable_count: l as u64,
            detector_rank: elimination.pivot_columns.len() as u64,
            canonicalization_id: CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
            fingerprint_id: FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
            transform_algorithm_id: TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
            reference_strategy_id: REFERENCE_SIMULATE_NOISELESS,
        };

        Ok(Self {
            limits,
            identity,
            reference,
            detector_rows: layout.detector_rows,
            observable_rows: layout.observable_rows,
            selected_detector_rows: elimination.selected_detector_rows,
            pivot_columns: elimination.pivot_columns,
            free_columns: elimination.free_columns,
            equations: elimination.equations,
            solve_order: elimination.solve_order,
            expanded_instructions: layout.expanded_instructions,
            parity_terms: layout.parity_terms,
            max_repeat_depth: layout.max_repeat_depth,
            transform_working_bytes: actual_bytes,
        })
    }

    pub fn identity(&self) -> &MeasurementTransformIdentity {
        &self.identity
    }

    pub fn limits(&self) -> MeasurementTransformLimits {
        self.limits
    }

    pub fn num_measurements(&self) -> usize {
        self.identity.measurement_count as usize
    }

    pub fn num_detectors(&self) -> usize {
        self.identity.detector_count as usize
    }

    pub fn num_observables(&self) -> usize {
        self.identity.observable_count as usize
    }

    pub fn rank(&self) -> usize {
        self.identity.detector_rank as usize
    }

    pub fn selected_detector_rows(&self) -> &[usize] {
        &self.selected_detector_rows
    }

    pub fn pivot_columns(&self) -> &[usize] {
        &self.pivot_columns
    }

    pub fn free_columns(&self) -> &[usize] {
        &self.free_columns
    }

    pub fn reference_bits(&self) -> &[bool] {
        &self.reference
    }

    pub fn transform_working_bytes(&self) -> u64 {
        self.transform_working_bytes
    }

    pub fn expanded_instructions(&self) -> u64 {
        self.expanded_instructions
    }

    pub fn parity_terms(&self) -> u64 {
        self.parity_terms
    }

    pub fn max_repeat_depth(&self) -> u64 {
        self.max_repeat_depth
    }

    pub fn validate_actual_usage(
        &self,
        limits: MeasurementTransformLimits,
        block_shots: Option<usize>,
    ) -> Result<(), MeasurementTransformError> {
        enforce_u64(
            "max_measurements",
            self.identity.measurement_count,
            limits.max_measurements,
        )?;
        enforce_u64(
            "max_detectors",
            self.identity.detector_count,
            limits.max_detectors,
        )?;
        enforce_u64(
            "max_observables",
            self.identity.observable_count,
            limits.max_observables,
        )?;
        enforce_u64(
            "max_repeat_depth",
            self.max_repeat_depth,
            limits.max_repeat_depth,
        )?;
        enforce_u64(
            "max_expanded_instructions",
            self.expanded_instructions,
            limits.max_expanded_instructions,
        )?;
        enforce_u64(
            "max_parity_terms",
            self.parity_terms,
            limits.max_parity_terms,
        )?;
        enforce_u64(
            "max_transform_working_bytes",
            self.transform_working_bytes,
            limits.max_transform_working_bytes,
        )?;
        if let Some(shots) = block_shots {
            enforce_u64(
                "max_shots_per_block",
                shots as u64,
                limits.max_shots_per_block,
            )?;
            let block_bytes = self.estimate_block_working_bytes(shots)?;
            enforce_u64(
                "max_block_working_bytes",
                block_bytes,
                limits.max_block_working_bytes,
            )?;
        }
        Ok(())
    }

    pub fn estimate_block_working_bytes(
        &self,
        shots: usize,
    ) -> Result<u64, MeasurementTransformError> {
        estimate_block_working_bytes(
            self.num_measurements(),
            self.num_detectors(),
            self.num_observables(),
            self.rank(),
            self.free_columns.len(),
            shots,
        )
    }

    pub fn encode_block(
        &self,
        measurements: &BitTable,
    ) -> Result<EncodedMeasurementBlock, MeasurementTransformError> {
        self.encode_block_prefix(measurements, measurements.num_minor())
    }

    pub(crate) fn encode_block_prefix(
        &self,
        measurements: &BitTable,
        shots: usize,
    ) -> Result<EncodedMeasurementBlock, MeasurementTransformError> {
        if measurements.num_major() != self.num_measurements() {
            return Err(MeasurementTransformError::ShapeMismatch {
                detail: format!(
                    "measurement rows {} do not match transform measurements {}",
                    measurements.num_major(),
                    self.num_measurements()
                ),
            });
        }
        if shots > measurements.num_minor() {
            return Err(MeasurementTransformError::ShapeMismatch {
                detail: format!(
                    "measurement shot prefix {shots} exceeds table shots {}",
                    measurements.num_minor()
                ),
            });
        }
        self.validate_block_shots(shots)?;
        let block_bytes = self.estimate_block_working_bytes(shots)?;
        enforce_bytes(
            "max_block_working_bytes",
            block_bytes,
            self.limits.max_block_working_bytes,
        )?;

        let mut selected = BitTable::try_new(self.rank(), shots)?;
        for (selected_row, &detector_row) in self.selected_detector_rows.iter().enumerate() {
            xor_parity_terms_into_row(
                selected.row_words_mut(selected_row),
                measurements,
                &self.detector_rows[detector_row],
                &self.reference,
                shots,
            );
        }

        let mut free = BitTable::try_new(self.free_columns.len(), shots)?;
        for (free_row, &measurement_col) in self.free_columns.iter().enumerate() {
            copy_measurement_flip_row(
                free.row_words_mut(free_row),
                measurements,
                measurement_col,
                self.reference[measurement_col],
                shots,
            );
        }

        Ok(EncodedMeasurementBlock {
            selected_detectors: selected,
            free_measurements: free,
        })
    }

    pub fn decode_block(
        &self,
        encoded: &EncodedMeasurementBlock,
    ) -> Result<DecodedSampleBlock, MeasurementTransformError> {
        if encoded.selected_detectors.num_major() != self.rank() {
            return Err(MeasurementTransformError::ShapeMismatch {
                detail: format!(
                    "selected detector rows {} do not match transform rank {}",
                    encoded.selected_detectors.num_major(),
                    self.rank()
                ),
            });
        }
        if encoded.free_measurements.num_major() != self.free_columns.len() {
            return Err(MeasurementTransformError::ShapeMismatch {
                detail: format!(
                    "free measurement rows {} do not match transform free columns {}",
                    encoded.free_measurements.num_major(),
                    self.free_columns.len()
                ),
            });
        }
        if encoded.selected_detectors.num_minor() != encoded.free_measurements.num_minor() {
            return Err(MeasurementTransformError::ShapeMismatch {
                detail: format!(
                    "selected detector shots {} do not match free measurement shots {}",
                    encoded.selected_detectors.num_minor(),
                    encoded.free_measurements.num_minor()
                ),
            });
        }
        let shots = encoded.selected_detectors.num_minor();
        self.validate_block_shots(shots)?;
        let block_bytes = self.estimate_block_working_bytes(shots)?;
        enforce_bytes(
            "max_block_working_bytes",
            block_bytes,
            self.limits.max_block_working_bytes,
        )?;

        let mut rhs = BitTable::try_new(self.rank(), shots)?;
        for (eq_index, equation) in self.equations.iter().enumerate() {
            let dst = rhs.row_words_mut(eq_index);
            for &source in &equation.rhs_sources {
                xor_words(dst, encoded.selected_detectors.row_words(source));
            }
        }

        let mut x = BitTable::try_new(self.num_measurements(), shots)?;
        let mut solved = BitTable::try_new(1, shots)?;
        for (free_row, &measurement_col) in self.free_columns.iter().enumerate() {
            x.row_words_mut(measurement_col)
                .copy_from_slice(encoded.free_measurements.row_words(free_row));
        }

        for &eq_index in &self.solve_order {
            let equation = &self.equations[eq_index];
            solved
                .row_words_mut(0)
                .copy_from_slice(rhs.row_words(eq_index));
            for_each_set_bit(&equation.row_words, self.num_measurements(), |col| {
                if col != equation.pivot {
                    xor_words(solved.row_words_mut(0), x.row_words(col));
                }
            });
            x.row_words_mut(equation.pivot)
                .copy_from_slice(solved.row_words(0));
        }

        let mut measurements = BitTable::try_new(self.num_measurements(), shots)?;
        for row in 0..self.num_measurements() {
            measurements
                .row_words_mut(row)
                .copy_from_slice(x.row_words(row));
            if self.reference[row] {
                invert_valid_bits(measurements.row_words_mut(row), shots);
            }
        }

        let mut detections = BitTable::try_new(self.num_detectors(), shots)?;
        apply_parity_rows(&self.detector_rows, &x, &mut detections);
        let mut observable_flips = BitTable::try_new(self.num_observables(), shots)?;
        apply_parity_rows(&self.observable_rows, &x, &mut observable_flips);

        for (selected_row, &detector_row) in self.selected_detector_rows.iter().enumerate() {
            if detections.row_words(detector_row)
                != encoded.selected_detectors.row_words(selected_row)
            {
                return Err(MeasurementTransformError::ShapeMismatch {
                    detail: "selected detector values are inconsistent after reconstruction"
                        .to_string(),
                });
            }
        }

        Ok(DecodedSampleBlock {
            measurements,
            detections,
            observable_flips,
        })
    }

    fn validate_block_shots(&self, shots: usize) -> Result<(), MeasurementTransformError> {
        let shots = shots as u64;
        if shots > self.limits.max_shots_per_block {
            Err(MeasurementTransformError::LimitExceeded {
                limit: "max_shots_per_block",
            })
        } else {
            Ok(())
        }
    }
}

struct LayoutBuilder {
    limits: MeasurementTransformLimits,
    measurement_count: u64,
    detector_rows: Vec<Vec<usize>>,
    observable_rows: Vec<Vec<usize>>,
    loss_visible_measurements: Vec<LossVisibleMeasurementPair>,
    expanded_instructions: u64,
    parity_terms: u64,
    max_repeat_depth: u64,
    layout_working_bytes: u64,
}

impl LayoutBuilder {
    fn new(limits: MeasurementTransformLimits) -> Self {
        Self {
            limits,
            measurement_count: 0,
            detector_rows: Vec::new(),
            observable_rows: Vec::new(),
            loss_visible_measurements: Vec::new(),
            expanded_instructions: 0,
            parity_terms: 0,
            max_repeat_depth: 0,
            layout_working_bytes: 0,
        }
    }

    fn visit_instrs(
        &mut self,
        instrs: &[StimInstr],
        repeat_depth: u64,
    ) -> Result<(), MeasurementTransformError> {
        for instr in instrs {
            match instr {
                StimInstr::Op {
                    name,
                    args,
                    targets,
                    ..
                } => self.visit_op(name, args, targets)?,
                StimInstr::Repeat { count, body } => {
                    let next_depth = repeat_depth.checked_add(1).ok_or(
                        MeasurementTransformError::LimitExceeded {
                            limit: "max_repeat_depth",
                        },
                    )?;
                    if next_depth > self.limits.max_repeat_depth {
                        return Err(MeasurementTransformError::LimitExceeded {
                            limit: "max_repeat_depth",
                        });
                    }
                    self.max_repeat_depth = self.max_repeat_depth.max(next_depth);
                    self.preflight_repeat_body(*count, body, next_depth)?;
                    for _ in 0..*count {
                        self.visit_instrs(body, next_depth)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn visit_op(
        &mut self,
        name: &str,
        args: &[f64],
        targets: &[StimTarget],
    ) -> Result<(), MeasurementTransformError> {
        self.expanded_instructions = self.expanded_instructions.checked_add(1).ok_or(
            MeasurementTransformError::LimitExceeded {
                limit: "max_expanded_instructions",
            },
        )?;
        if self.expanded_instructions > self.limits.max_expanded_instructions {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_expanded_instructions",
            });
        }

        match name {
            "DETECTOR" => {
                let next = (self.detector_rows.len() as u64).checked_add(1).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_detectors",
                    },
                )?;
                if next > self.limits.max_detectors {
                    return Err(MeasurementTransformError::LimitExceeded {
                        limit: "max_detectors",
                    });
                }
                self.reserve_layout_row()?;
                let row = self.resolve_record_terms(targets)?;
                self.detector_rows.push(row);
            }
            "OBSERVABLE_INCLUDE" => {
                let index = observable_index(args)?;
                let needed =
                    index
                        .checked_add(1)
                        .ok_or(MeasurementTransformError::LimitExceeded {
                            limit: "max_observables",
                        })?;
                if needed as u64 > self.limits.max_observables {
                    return Err(MeasurementTransformError::LimitExceeded {
                        limit: "max_observables",
                    });
                }
                while self.observable_rows.len() <= index {
                    self.reserve_layout_row()?;
                    self.observable_rows.push(Vec::new());
                }
                let row = self.resolve_record_terms(targets)?;
                xor_terms_into(&mut self.observable_rows[index], &row);
            }
            _ => {
                if is_loss_visible_measurement(name) {
                    let pair_count = count_qubit_like_targets(targets);
                    let base = usize::try_from(self.measurement_count).map_err(|_| {
                        MeasurementTransformError::LimitExceeded {
                            limit: "max_measurements",
                        }
                    })?;
                    let pair_bytes = pair_count
                        .checked_mul(std::mem::size_of::<LossVisibleMeasurementPair>())
                        .and_then(|value| u64::try_from(value).ok())
                        .ok_or(MeasurementTransformError::LimitExceeded {
                            limit: "max_transform_working_bytes",
                        })?;
                    self.reserve_layout_bytes(pair_bytes)?;
                    for pair in 0..pair_count {
                        let offset = pair.checked_mul(2).ok_or(
                            MeasurementTransformError::LimitExceeded {
                                limit: "max_measurements",
                            },
                        )?;
                        let flag = base.checked_add(offset).ok_or(
                            MeasurementTransformError::LimitExceeded {
                                limit: "max_measurements",
                            },
                        )?;
                        let value = flag.checked_add(1).ok_or(
                            MeasurementTransformError::LimitExceeded {
                                limit: "max_measurements",
                            },
                        )?;
                        self.loss_visible_measurements
                            .push(LossVisibleMeasurementPair { flag, value });
                    }
                }
                let produced = count_measurements_op_u64(name, targets)?;
                self.measurement_count = self.measurement_count.checked_add(produced).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_measurements",
                    },
                )?;
                if self.measurement_count > self.limits.max_measurements {
                    return Err(MeasurementTransformError::LimitExceeded {
                        limit: "max_measurements",
                    });
                }
            }
        }
        Ok(())
    }

    fn resolve_record_terms(
        &mut self,
        targets: &[StimTarget],
    ) -> Result<Vec<usize>, MeasurementTransformError> {
        for target in targets {
            if !matches!(target, StimTarget::Rec(_)) {
                return Err(MeasurementTransformError::InvalidRecordTarget {
                    detail: "DETECTOR and OBSERVABLE_INCLUDE targets must be rec[-k]".to_string(),
                });
            }
        }
        self.reserve_parity_terms(targets.len())?;
        let mut terms = Vec::new();
        for target in targets {
            let StimTarget::Rec(offset) = target else {
                unreachable!("record targets were prevalidated");
            };
            let absolute = self.measurement_count as i128 + *offset as i128;
            if absolute < 0 || absolute >= self.measurement_count as i128 {
                return Err(MeasurementTransformError::InvalidRecordTarget {
                    detail: format!(
                        "rec[{offset}] is out of history at measurement index {}",
                        self.measurement_count
                    ),
                });
            }
            terms.push(usize::try_from(absolute).map_err(|_| {
                MeasurementTransformError::LimitExceeded {
                    limit: "max_measurements",
                }
            })?);
        }
        Ok(normalize_terms(terms))
    }

    fn preflight_repeat_body(
        &self,
        count: u64,
        body: &[StimInstr],
        repeat_depth: u64,
    ) -> Result<(), MeasurementTransformError> {
        let expanded = count_expanded_instructions_limited(body, repeat_depth, self.limits)?
            .checked_mul(count)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_expanded_instructions",
            })?;
        if self.expanded_instructions.checked_add(expanded).ok_or(
            MeasurementTransformError::LimitExceeded {
                limit: "max_expanded_instructions",
            },
        )? > self.limits.max_expanded_instructions
        {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_expanded_instructions",
            });
        }

        let parity_terms = count_parity_terms_limited(body, repeat_depth, self.limits)?
            .checked_mul(count)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_parity_terms",
            })?;
        if self.parity_terms.checked_add(parity_terms).ok_or(
            MeasurementTransformError::LimitExceeded {
                limit: "max_parity_terms",
            },
        )? > self.limits.max_parity_terms
        {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_parity_terms",
            });
        }

        let added_measurements = count_measurements_instrs_u128(body)
            .checked_mul(count as u128)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_measurements",
            })?;
        if (self.measurement_count as u128)
            .checked_add(added_measurements)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_measurements",
            })?
            > self.limits.max_measurements as u128
        {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_measurements",
            });
        }

        let added_detectors = count_detectors_instrs_u128(body)
            .checked_mul(count as u128)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_detectors",
            })?;
        if (self.detector_rows.len() as u128)
            .checked_add(added_detectors)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_detectors",
            })?
            > self.limits.max_detectors as u128
        {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_detectors",
            });
        }

        if let Some(observable) = max_observable_index(body) {
            if observable as u64 >= self.limits.max_observables {
                return Err(MeasurementTransformError::LimitExceeded {
                    limit: "max_observables",
                });
            }
        }

        Ok(())
    }

    fn reserve_layout_row(&mut self) -> Result<(), MeasurementTransformError> {
        self.reserve_layout_bytes(std::mem::size_of::<Vec<usize>>() as u64)
    }

    fn reserve_parity_terms(&mut self, count: usize) -> Result<(), MeasurementTransformError> {
        let count = u64::try_from(count).map_err(|_| MeasurementTransformError::LimitExceeded {
            limit: "max_parity_terms",
        })?;
        self.parity_terms = self.parity_terms.checked_add(count).ok_or(
            MeasurementTransformError::LimitExceeded {
                limit: "max_parity_terms",
            },
        )?;
        if self.parity_terms > self.limits.max_parity_terms {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_parity_terms",
            });
        }
        self.reserve_layout_bytes(
            count
                .checked_mul(std::mem::size_of::<usize>() as u64)
                .ok_or(MeasurementTransformError::LimitExceeded {
                    limit: "max_transform_working_bytes",
                })?,
        )
    }

    fn reserve_layout_bytes(&mut self, bytes: u64) -> Result<(), MeasurementTransformError> {
        self.layout_working_bytes = self.layout_working_bytes.checked_add(bytes).ok_or(
            MeasurementTransformError::LimitExceeded {
                limit: "max_transform_working_bytes",
            },
        )?;
        if self.layout_working_bytes > self.limits.max_transform_working_bytes {
            Err(MeasurementTransformError::LimitExceeded {
                limit: "max_transform_working_bytes",
            })
        } else {
            Ok(())
        }
    }
}

fn is_loss_visible_measurement(name: &str) -> bool {
    matches!(
        name,
        "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL"
    )
}

pub(crate) fn num_measurements_unchecked(instrs: &[StimInstr]) -> usize {
    usize_from_u128(count_measurements_instrs_u128(instrs))
}

pub(crate) fn num_detectors_unchecked(instrs: &[StimInstr]) -> usize {
    usize_from_u128(count_detectors_instrs_u128(instrs))
}

pub(crate) fn num_observables_unchecked(instrs: &[StimInstr]) -> usize {
    max_observable_index(instrs).map_or(0, |idx| idx.saturating_add(1))
}

fn count_measurements_op_u64(
    name: &str,
    targets: &[StimTarget],
) -> Result<u64, MeasurementTransformError> {
    u64::try_from(count_measurements_op_u128(name, targets)).map_err(|_| {
        MeasurementTransformError::LimitExceeded {
            limit: "max_measurements",
        }
    })
}

fn count_measurements_instrs_u128(instrs: &[StimInstr]) -> u128 {
    let mut count = 0u128;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
                count = count.saturating_add(count_measurements_op_u128(name, targets));
            }
            StimInstr::Repeat {
                count: repeat,
                body,
            } => {
                count = count.saturating_add(
                    (*repeat as u128).saturating_mul(count_measurements_instrs_u128(body)),
                );
            }
        }
    }
    count
}

fn count_expanded_instructions_limited(
    instrs: &[StimInstr],
    repeat_depth: u64,
    limits: MeasurementTransformLimits,
) -> Result<u64, MeasurementTransformError> {
    let mut count = 0u64;
    for instr in instrs {
        match instr {
            StimInstr::Op { .. } => {
                count = count
                    .checked_add(1)
                    .ok_or(MeasurementTransformError::LimitExceeded {
                        limit: "max_expanded_instructions",
                    })?;
            }
            StimInstr::Repeat {
                count: repeat,
                body,
            } => {
                let next_depth = repeat_depth.checked_add(1).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_repeat_depth",
                    },
                )?;
                if next_depth > limits.max_repeat_depth {
                    return Err(MeasurementTransformError::LimitExceeded {
                        limit: "max_repeat_depth",
                    });
                }
                let body_count = count_expanded_instructions_limited(body, next_depth, limits)?;
                let expanded = body_count.checked_mul(*repeat).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_expanded_instructions",
                    },
                )?;
                count = count.checked_add(expanded).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_expanded_instructions",
                    },
                )?;
            }
        }
        if count > limits.max_expanded_instructions {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_expanded_instructions",
            });
        }
    }
    Ok(count)
}

fn count_parity_terms_limited(
    instrs: &[StimInstr],
    repeat_depth: u64,
    limits: MeasurementTransformLimits,
) -> Result<u64, MeasurementTransformError> {
    let mut count = 0u64;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. }
                if name == "DETECTOR" || name == "OBSERVABLE_INCLUDE" =>
            {
                let terms = u64::try_from(targets.len()).map_err(|_| {
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_parity_terms",
                    }
                })?;
                count =
                    count
                        .checked_add(terms)
                        .ok_or(MeasurementTransformError::LimitExceeded {
                            limit: "max_parity_terms",
                        })?;
            }
            StimInstr::Repeat {
                count: repeat,
                body,
            } => {
                let next_depth = repeat_depth.checked_add(1).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_repeat_depth",
                    },
                )?;
                if next_depth > limits.max_repeat_depth {
                    return Err(MeasurementTransformError::LimitExceeded {
                        limit: "max_repeat_depth",
                    });
                }
                let body_count = count_parity_terms_limited(body, next_depth, limits)?;
                let expanded = body_count.checked_mul(*repeat).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_parity_terms",
                    },
                )?;
                count = count.checked_add(expanded).ok_or(
                    MeasurementTransformError::LimitExceeded {
                        limit: "max_parity_terms",
                    },
                )?;
            }
            _ => {}
        }
        if count > limits.max_parity_terms {
            return Err(MeasurementTransformError::LimitExceeded {
                limit: "max_parity_terms",
            });
        }
    }
    Ok(count)
}

fn count_measurements_op_u128(name: &str, targets: &[StimTarget]) -> u128 {
    match name {
        "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ" => {
            count_qubit_like_targets(targets) as u128
        }
        "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL" => {
            2 * count_qubit_like_targets(targets) as u128
        }
        "MPP" => count_mpp_products(targets) as u128,
        "MXX" | "MYY" | "MZZ" => (targets.len() / 2) as u128,
        "MPAD" => targets.len() as u128,
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => targets.len() as u128,
        _ => 0,
    }
}

fn count_qubit_like_targets(targets: &[StimTarget]) -> usize {
    targets
        .iter()
        .filter(|target| matches!(target, StimTarget::Qubit(_) | StimTarget::QubitInv(_)))
        .count()
}

fn count_mpp_products(targets: &[StimTarget]) -> usize {
    let mut count = 0usize;
    let mut after_combiner = false;
    let mut have_current_product = false;
    for target in targets {
        match target {
            StimTarget::Pauli { .. } => {
                if !after_combiner || !have_current_product {
                    count += 1;
                }
                have_current_product = true;
                after_combiner = false;
            }
            StimTarget::Combiner => {
                after_combiner = true;
            }
            _ => {
                after_combiner = false;
            }
        }
    }
    count
}

fn count_detectors_instrs_u128(instrs: &[StimInstr]) -> u128 {
    let mut count = 0u128;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } if name == "DETECTOR" => count = count.saturating_add(1),
            StimInstr::Repeat {
                count: repeat,
                body,
            } => {
                count = count.saturating_add(
                    (*repeat as u128).saturating_mul(count_detectors_instrs_u128(body)),
                );
            }
            _ => {}
        }
    }
    count
}

fn max_observable_index(instrs: &[StimInstr]) -> Option<usize> {
    let mut max_idx = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, args, .. } if name == "OBSERVABLE_INCLUDE" => {
                if let Ok(index) = observable_index(args) {
                    max_idx = Some(max_idx.map_or(index, |current: usize| current.max(index)));
                }
            }
            StimInstr::Repeat { body, .. } => {
                if let Some(index) = max_observable_index(body) {
                    max_idx = Some(max_idx.map_or(index, |current| current.max(index)));
                }
            }
            _ => {}
        }
    }
    max_idx
}

fn eliminate_detector_rows(h: &BitTable, measurement_count: usize) -> Elimination {
    let mut selected_detector_rows = Vec::new();
    let mut pivot_columns = Vec::new();
    let mut equations = Vec::new();
    let mut pivot_to_equation = vec![None; measurement_count];

    for detector_index in 0..h.num_major() {
        let mut row_words = h.row_words(detector_index).to_vec();
        let mut rhs_sources = Vec::new();
        loop {
            let Some(highest) = highest_set_bit(&row_words, measurement_count) else {
                break;
            };
            let Some(eq_index) = pivot_to_equation[highest] else {
                let selected_row = selected_detector_rows.len();
                rhs_sources.push(selected_row);
                normalize_sources(&mut rhs_sources);
                pivot_to_equation[highest] = Some(equations.len());
                selected_detector_rows.push(detector_index);
                pivot_columns.push(highest);
                equations.push(EchelonEquation {
                    pivot: highest,
                    row_words,
                    rhs_sources,
                });
                break;
            };
            xor_words(&mut row_words, &equations[eq_index].row_words);
            xor_terms_into(&mut rhs_sources, &equations[eq_index].rhs_sources);
        }
    }

    let pivot_set: BTreeSet<usize> = pivot_columns.iter().copied().collect();
    let free_columns = (0..measurement_count)
        .filter(|col| !pivot_set.contains(col))
        .collect::<Vec<_>>();
    let mut solve_order = (0..equations.len()).collect::<Vec<_>>();
    solve_order.sort_by_key(|&eq_index| equations[eq_index].pivot);

    Elimination {
        selected_detector_rows,
        pivot_columns,
        free_columns,
        equations,
        solve_order,
    }
}

fn apply_parity_rows(rows: &[Vec<usize>], x: &BitTable, out: &mut BitTable) {
    for (row_index, terms) in rows.iter().enumerate() {
        let dst = out.row_words_mut(row_index);
        for &term in terms {
            xor_words(dst, x.row_words(term));
        }
    }
}

fn xor_parity_terms_into_row(
    dst: &mut [u64],
    measurements: &BitTable,
    terms: &[usize],
    reference: &[bool],
    shots: usize,
) {
    for &term in terms {
        xor_words(dst, measurements.row_words(term));
        if reference[term] {
            xor_all_valid_bits(dst, shots);
        }
    }
    clear_invalid_bits(dst, shots);
}

fn copy_measurement_flip_row(
    dst: &mut [u64],
    measurements: &BitTable,
    measurement_col: usize,
    reference: bool,
    shots: usize,
) {
    let src = measurements.row_words(measurement_col);
    dst.copy_from_slice(&src[..dst.len()]);
    if reference {
        invert_valid_bits(dst, shots);
    }
    clear_invalid_bits(dst, shots);
}

fn xor_all_valid_bits(words: &mut [u64], shots: usize) {
    if shots == 0 {
        return;
    }
    let full_words = shots / 64;
    for word in words.iter_mut().take(full_words) {
        *word ^= !0u64;
    }
    let rem = shots % 64;
    if rem != 0 {
        words[full_words] ^= (1u64 << rem) - 1;
    }
}

fn invert_valid_bits(words: &mut [u64], shots: usize) {
    xor_all_valid_bits(words, shots);
}

fn clear_invalid_bits(words: &mut [u64], shots: usize) {
    if shots == 0 || words.is_empty() {
        return;
    }
    let rem = shots % 64;
    if rem != 0 {
        let last = shots / 64;
        words[last] &= (1u64 << rem) - 1;
    }
}

fn for_each_set_bit(row_words: &[u64], num_bits: usize, mut f: impl FnMut(usize)) {
    for (word_index, &word) in row_words.iter().enumerate() {
        let mut word = word;
        while word != 0 {
            let bit = word.trailing_zeros() as usize;
            let col = word_index * 64 + bit;
            if col < num_bits {
                f(col);
            }
            word &= word - 1;
        }
    }
}

fn highest_set_bit(row_words: &[u64], num_bits: usize) -> Option<usize> {
    if num_bits == 0 {
        return None;
    }
    let last_word = (num_bits - 1) / 64;
    for word_index in (0..=last_word).rev() {
        let mut word = row_words[word_index];
        if word_index == last_word {
            let used_bits = num_bits % 64;
            if used_bits != 0 {
                word &= (1u64 << used_bits) - 1;
            }
        }
        if word != 0 {
            let bit = 63 - word.leading_zeros() as usize;
            return Some(word_index * 64 + bit);
        }
    }
    None
}

pub(crate) fn xor_words(dst: &mut [u64], src: &[u64]) {
    for (dst, src) in dst.iter_mut().zip(src.iter()) {
        *dst ^= *src;
    }
}

fn xor_terms_into(dst: &mut Vec<usize>, src: &[usize]) {
    dst.extend_from_slice(src);
    normalize_sources(dst);
}

fn normalize_sources(values: &mut Vec<usize>) {
    *values = normalize_terms(std::mem::take(values));
}

fn normalize_terms(mut terms: Vec<usize>) -> Vec<usize> {
    terms.sort_unstable();
    let mut out = Vec::new();
    let mut i = 0;
    while i < terms.len() {
        let value = terms[i];
        let mut count = 0usize;
        while i < terms.len() && terms[i] == value {
            count += 1;
            i += 1;
        }
        if count % 2 == 1 {
            out.push(value);
        }
    }
    out
}

fn estimate_transform_working_bytes(
    measurements: usize,
    detectors: usize,
    observables: usize,
    rank: usize,
    parity_terms: u64,
) -> Result<u64, MeasurementTransformError> {
    let h = bit_table_bytes(detectors, measurements)?;
    let g = bit_table_bytes(observables, measurements)?;
    let echelon = bit_table_bytes(rank, measurements)?;
    let scratch = bit_table_bytes(1, measurements)?;
    let reference = checked_usize_bytes(measurements, 1, "max_transform_working_bytes")?;
    let sparse_rows = checked_usize_bytes(
        detectors
            .checked_add(observables)
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_transform_working_bytes",
            })?,
        std::mem::size_of::<Vec<usize>>(),
        "max_transform_working_bytes",
    )?;
    let pivots = checked_usize_bytes(
        measurements
            .checked_add(detectors)
            .and_then(|v| v.checked_add(observables))
            .and_then(|v| v.checked_add(rank))
            .ok_or(MeasurementTransformError::LimitExceeded {
                limit: "max_transform_working_bytes",
            })?,
        std::mem::size_of::<usize>(),
        "max_transform_working_bytes",
    )?;
    let sparse_terms = parity_terms
        .checked_mul(std::mem::size_of::<usize>() as u64)
        .ok_or(MeasurementTransformError::LimitExceeded {
            limit: "max_transform_working_bytes",
        })?;
    checked_sum(
        "max_transform_working_bytes",
        &[
            h,
            g,
            echelon,
            scratch,
            reference,
            sparse_rows,
            pivots,
            sparse_terms,
        ],
    )
}

fn estimate_block_working_bytes(
    measurements: usize,
    detectors: usize,
    observables: usize,
    rank: usize,
    free: usize,
    shots: usize,
) -> Result<u64, MeasurementTransformError> {
    let encoded_selected = bit_table_bytes(rank, shots)?;
    let encoded_free = bit_table_bytes(free, shots)?;
    let encode_peak = checked_sum("max_block_working_bytes", &[encoded_selected, encoded_free])?;
    let decoded_measurements = bit_table_bytes(measurements, shots)?;
    let decoded_detections = bit_table_bytes(detectors, shots)?;
    let decoded_observables = bit_table_bytes(observables, shots)?;
    let scratch_x = bit_table_bytes(measurements, shots)?;
    let scratch_rhs = bit_table_bytes(rank, shots)?;
    let scratch_row = bit_table_bytes(1, shots)?;
    let decode_peak = checked_sum(
        "max_block_working_bytes",
        &[
            decoded_measurements,
            decoded_detections,
            decoded_observables,
            scratch_x,
            scratch_rhs,
            scratch_row,
        ],
    )?;
    Ok(encode_peak.max(decode_peak))
}

fn bit_table_bytes(major: usize, minor: usize) -> Result<u64, MeasurementTransformError> {
    let size = checked_bit_table_storage_size(major, minor)?;
    u64::try_from(size.total_bytes).map_err(|_| MeasurementTransformError::LimitExceeded {
        limit: "max_transform_working_bytes",
    })
}

fn checked_usize_bytes(
    count: usize,
    bytes_per_item: usize,
    limit: &'static str,
) -> Result<u64, MeasurementTransformError> {
    let bytes = count
        .checked_mul(bytes_per_item)
        .ok_or(MeasurementTransformError::LimitExceeded { limit })?;
    u64::try_from(bytes).map_err(|_| MeasurementTransformError::LimitExceeded { limit })
}

fn checked_sum(limit: &'static str, values: &[u64]) -> Result<u64, MeasurementTransformError> {
    values.iter().try_fold(0u64, |acc, value| {
        acc.checked_add(*value)
            .ok_or(MeasurementTransformError::LimitExceeded { limit })
    })
}

fn enforce_bytes(
    limit: &'static str,
    bytes: u64,
    max_bytes: u64,
) -> Result<(), MeasurementTransformError> {
    if bytes > max_bytes {
        Err(MeasurementTransformError::LimitExceeded { limit })
    } else {
        Ok(())
    }
}

fn enforce_u64(
    limit: &'static str,
    actual: u64,
    allowed: u64,
) -> Result<(), MeasurementTransformError> {
    if actual > allowed {
        Err(MeasurementTransformError::LimitExceeded { limit })
    } else {
        Ok(())
    }
}

fn observable_index(args: &[f64]) -> Result<usize, MeasurementTransformError> {
    let raw = args.first().copied().unwrap_or(0.0);
    if !raw.is_finite() || raw < 0.0 || raw.fract() != 0.0 {
        return Err(MeasurementTransformError::InvalidRecordTarget {
            detail: format!("invalid observable index {raw}"),
        });
    }
    usize::try_from(raw as u128).map_err(|_| MeasurementTransformError::LimitExceeded {
        limit: "max_observables",
    })
}

fn contains_sweep_target(instrs: &[StimInstr]) -> bool {
    instrs.iter().any(|instr| match instr {
        StimInstr::Op { targets, .. } => targets
            .iter()
            .any(|target| matches!(target, StimTarget::Sweep(_))),
        StimInstr::Repeat { body, .. } => contains_sweep_target(body),
    })
}

fn usize_from_u64(value: u64, limit: &'static str) -> Result<usize, MeasurementTransformError> {
    usize::try_from(value).map_err(|_| MeasurementTransformError::LimitExceeded { limit })
}

fn usize_from_u128(value: u128) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::PauliBasis;
    use crate::parser::parse_lines;

    #[test]
    fn private_helpers_cover_edge_cases() {
        let mut empty = Vec::new();
        xor_all_valid_bits(&mut empty, 0);
        assert!(empty.is_empty());
        assert_eq!(highest_set_bit(&[], 0), None);

        let mut words = vec![0u64; 2];
        xor_all_valid_bits(&mut words, 65);
        assert_eq!(words, vec![!0u64, 1]);

        assert_eq!(
            count_mpp_products(&[
                StimTarget::pauli(0, PauliBasis::X, false),
                StimTarget::Combiner,
                StimTarget::pauli(1, PauliBasis::Z, false),
                StimTarget::Qubit(2),
                StimTarget::pauli(3, PauliBasis::Y, false),
            ]),
            2
        );

        assert!(matches!(
            observable_index(&[f64::NAN]),
            Err(MeasurementTransformError::InvalidRecordTarget { .. })
        ));
        assert!(matches!(
            checked_usize_bytes(usize::MAX, 2, "test_limit"),
            Err(MeasurementTransformError::LimitExceeded {
                limit: "test_limit"
            })
        ));
        assert!(matches!(
            checked_sum("test_limit", &[u64::MAX, 1]),
            Err(MeasurementTransformError::LimitExceeded {
                limit: "test_limit"
            })
        ));
        assert!(matches!(
            enforce_bytes("test_limit", 2, 1),
            Err(MeasurementTransformError::LimitExceeded {
                limit: "test_limit"
            })
        ));

        let circuit = parse_lines("REPEAT 2 {\n    M 0\n    DETECTOR rec[-1]\n}\n")
            .expect("parse repeat circuit");
        let layout = CheckedMeasurementLayout::from_circuit_with_limits(
            &circuit,
            MeasurementTransformLimits::default(),
        )
        .expect("layout builds");
        assert_eq!(layout.max_repeat_depth(), 1);

        let transform = MeasurementTransform::from_circuit(&circuit).expect("transform builds");
        assert!(transform
            .validate_actual_usage(MeasurementTransformLimits::default(), Some(1))
            .is_ok());
    }

    #[test]
    fn encode_block_prefix_uses_only_requested_shots() {
        let circuit = parse_lines("M 0 1 2\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\n")
            .expect("parse transform circuit");
        let transform = MeasurementTransform::from_circuit(&circuit).expect("transform builds");
        let mut buffered =
            BitTable::try_new(transform.num_measurements(), 4096).expect("buffer allocates");
        let mut prefix =
            BitTable::try_new(transform.num_measurements(), 65).expect("prefix allocates");
        for row in 0..buffered.num_major() {
            for shot in 0..buffered.num_minor() {
                let value = if shot < prefix.num_minor() {
                    (row * 19 + shot * 23) % 3 == 1
                } else {
                    true
                };
                buffered.set(row, shot, value);
                if shot < prefix.num_minor() {
                    prefix.set(row, shot, value);
                }
            }
        }

        let encoded_prefix = transform
            .encode_block_prefix(&buffered, prefix.num_minor())
            .expect("encode buffered prefix");
        let encoded_exact = transform
            .encode_block(&prefix)
            .expect("encode exact prefix");
        let encoded_exact_prefix = transform
            .encode_block_prefix(&prefix, prefix.num_minor())
            .expect("encode exact full prefix");
        assert!(matches!(
            transform
                .encode_block_prefix(&prefix, prefix.num_minor() + 1)
                .unwrap_err(),
            MeasurementTransformError::ShapeMismatch { detail }
                if detail.contains("measurement shot prefix")
        ));
        assert_tables_eq(
            &encoded_exact.selected_detectors,
            &encoded_prefix.selected_detectors,
        );
        assert_tables_eq(
            &encoded_exact.free_measurements,
            &encoded_prefix.free_measurements,
        );
        assert_tables_eq(
            &encoded_exact.selected_detectors,
            &encoded_exact_prefix.selected_detectors,
        );
        assert_tables_eq(
            &encoded_exact.free_measurements,
            &encoded_exact_prefix.free_measurements,
        );
        assert_row_words_eq(
            &encoded_exact.selected_detectors,
            &encoded_prefix.selected_detectors,
        );
        assert_row_words_eq(
            &encoded_exact.free_measurements,
            &encoded_prefix.free_measurements,
        );
    }

    fn assert_tables_eq(left: &BitTable, right: &BitTable) {
        assert_eq!(left.num_major(), right.num_major());
        assert_eq!(left.num_minor(), right.num_minor());
        for row in 0..left.num_major() {
            for shot in 0..left.num_minor() {
                assert_eq!(left.get(row, shot), right.get(row, shot));
            }
        }
    }

    fn assert_row_words_eq(left: &BitTable, right: &BitTable) {
        assert_eq!(left.num_major(), right.num_major());
        assert_eq!(left.num_minor(), right.num_minor());
        for row in 0..left.num_major() {
            assert_eq!(left.row_words(row), right.row_words(row));
        }
    }
}
