use std::collections::BTreeMap;

use rand::{rngs::StdRng, Rng, SeedableRng};
use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};

const SX_LABELS: [&str; 7] = ["idle", "1", "4", "3", "5", "0", "2"];
const SZ_LABELS: [&str; 7] = ["3", "5", "0", "1", "2", "4", "idle"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BivariateBicycleParams {
    pub ell: usize,
    pub m: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub b1: usize,
    pub b2: usize,
    pub b3: usize,
}

impl BivariateBicycleParams {
    pub fn upstream_default() -> Self {
        Self {
            ell: 12,
            m: 6,
            a1: 3,
            a2: 1,
            a3: 2,
            b1: 3,
            b2: 1,
            b3: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BbCode {
    params: BivariateBicycleParams,
    n2: usize,
    k: usize,
    x_checks: Vec<usize>,
    z_checks: Vec<usize>,
    data_qubits: Vec<usize>,
    hx_rows: Vec<Vec<usize>>,
    hz_rows: Vec<Vec<usize>>,
    logical_x_rows: Vec<Vec<usize>>,
    logical_z_rows: Vec<Vec<usize>>,
    x_cnot_targets: Vec<[usize; 6]>,
    z_cnot_targets: Vec<[usize; 6]>,
}

impl BbCode {
    pub fn ell(&self) -> usize {
        self.params.ell
    }

    pub fn m(&self) -> usize {
        self.params.m
    }

    pub fn n2(&self) -> usize {
        self.n2
    }

    pub fn n(&self) -> usize {
        2 * self.n2
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn x_checks(&self) -> &[usize] {
        &self.x_checks
    }

    pub fn z_checks(&self) -> &[usize] {
        &self.z_checks
    }

    pub fn data_qubits(&self) -> &[usize] {
        &self.data_qubits
    }

    pub fn num_circuit_qubits(&self) -> usize {
        4 * self.n2
    }

    pub fn hx_rows(&self) -> &[Vec<usize>] {
        &self.hx_rows
    }

    pub fn hz_rows(&self) -> &[Vec<usize>] {
        &self.hz_rows
    }

    pub fn logical_x_rows(&self) -> &[Vec<usize>] {
        &self.logical_x_rows
    }

    pub fn logical_z_rows(&self) -> &[Vec<usize>] {
        &self.logical_z_rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Idle,
    Cnot,
    PrepX,
    PrepZ,
    MeasX,
    MeasZ,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    kind: OperationKind,
    qubits: Vec<usize>,
}

impl Operation {
    fn new(kind: OperationKind, qubits: Vec<usize>) -> Self {
        Self { kind, qubits }
    }

    pub fn kind(&self) -> OperationKind {
        self.kind
    }

    pub fn qubits(&self) -> &[usize] {
        &self.qubits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyndromeCycle {
    operations: Vec<Operation>,
    sx_labels: [&'static str; 7],
    sz_labels: [&'static str; 7],
}

impl SyndromeCycle {
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn count(&self, kind: OperationKind) -> usize {
        self.operations
            .iter()
            .filter(|operation| operation.kind == kind)
            .count()
    }

    pub fn sx_labels(&self) -> [&'static str; 7] {
        self.sx_labels
    }

    pub fn sz_labels(&self) -> [&'static str; 7] {
        self.sz_labels
    }
}

pub fn build_upstream_code() -> Result<BbCode, String> {
    let params = BivariateBicycleParams::upstream_default();
    let n2 = params.ell * params.m;
    let width = 2 * n2;
    let x_checks = (0..n2).collect::<Vec<_>>();
    let z_checks = (3 * n2..4 * n2).collect::<Vec<_>>();
    let data_qubits = (n2..3 * n2).collect::<Vec<_>>();
    let mut hx_rows = Vec::with_capacity(n2);
    let mut hz_rows = Vec::with_capacity(n2);
    let mut x_cnot_targets = Vec::with_capacity(n2);
    let mut z_cnot_targets = Vec::with_capacity(n2);

    for row in 0..n2 {
        let x_slots = x_row_slots_local(&params, row);
        let z_slots = z_row_slots_local(&params, row);

        let mut hx_row = x_slots.to_vec();
        hx_row.sort_unstable();
        hx_rows.push(hx_row);
        x_cnot_targets.push(local_to_circuit_targets(&x_slots, n2));

        let mut hz_row = z_slots.to_vec();
        hz_row.sort_unstable();
        hz_rows.push(hz_row);
        z_cnot_targets.push(local_to_circuit_targets(&z_slots, n2));
    }

    let dense_hx = sparse_to_dense_rows(&hx_rows, width);
    let dense_hz = sparse_to_dense_rows(&hz_rows, width);

    let hx_rank = rank(&dense_hx);
    let hz_rank = rank(&dense_hz);
    let k = width
        .checked_sub(hx_rank + hz_rank)
        .ok_or_else(|| "bb144 rank computation underflowed".to_owned())?;

    let logical_x_rows = select_logical_rows(nullspace(&dense_hz, width), &dense_hx, k)
        .into_iter()
        .map(dense_to_sparse_row)
        .collect::<Vec<_>>();
    let logical_z_rows = select_logical_rows(nullspace(&dense_hx, width), &dense_hz, k)
        .into_iter()
        .map(dense_to_sparse_row)
        .collect::<Vec<_>>();

    if logical_x_rows.len() != k || logical_z_rows.len() != k {
        return Err(format!(
            "failed to extract {k} logical rows (x={}, z={})",
            logical_x_rows.len(),
            logical_z_rows.len()
        ));
    }

    Ok(BbCode {
        params,
        n2,
        k,
        x_checks,
        z_checks,
        data_qubits,
        hx_rows,
        hz_rows,
        logical_x_rows,
        logical_z_rows,
        x_cnot_targets,
        z_cnot_targets,
    })
}

pub fn build_syndrome_cycle(code: &BbCode) -> SyndromeCycle {
    let mut operations = Vec::with_capacity(1440);

    let round0_sz_slot = parse_schedule_slot(SZ_LABELS[0]).expect("round 0 must use a Z slot");
    let round6_sx_slot = parse_schedule_slot(SX_LABELS[6]).expect("round 6 must use an X slot");

    for &check in &code.x_checks {
        operations.push(Operation::new(OperationKind::PrepX, vec![check]));
    }
    for (row, &check) in code.z_checks.iter().enumerate() {
        operations.push(Operation::new(
            OperationKind::Cnot,
            vec![code.z_cnot_targets[row][round0_sz_slot], check],
        ));
    }
    append_idle_untouched_data(
        &mut operations,
        code,
        code.z_cnot_targets
            .iter()
            .map(|targets| targets[round0_sz_slot]),
    );

    for round in 1..6 {
        let sx_slot = parse_schedule_slot(SX_LABELS[round]).expect("middle rounds must use X");
        let sz_slot = parse_schedule_slot(SZ_LABELS[round]).expect("middle rounds must use Z");

        for (row, &check) in code.x_checks.iter().enumerate() {
            operations.push(Operation::new(
                OperationKind::Cnot,
                vec![check, code.x_cnot_targets[row][sx_slot]],
            ));
        }

        for (row, &check) in code.z_checks.iter().enumerate() {
            operations.push(Operation::new(
                OperationKind::Cnot,
                vec![code.z_cnot_targets[row][sz_slot], check],
            ));
        }
    }

    for &check in &code.z_checks {
        operations.push(Operation::new(OperationKind::MeasZ, vec![check]));
    }
    for (row, &check) in code.x_checks.iter().enumerate() {
        operations.push(Operation::new(
            OperationKind::Cnot,
            vec![check, code.x_cnot_targets[row][round6_sx_slot]],
        ));
    }
    append_idle_untouched_data(
        &mut operations,
        code,
        code.x_cnot_targets
            .iter()
            .map(|targets| targets[round6_sx_slot]),
    );

    for &data in &code.data_qubits {
        operations.push(Operation::new(OperationKind::Idle, vec![data]));
    }
    for &check in &code.x_checks {
        operations.push(Operation::new(OperationKind::MeasX, vec![check]));
    }
    for &check in &code.z_checks {
        operations.push(Operation::new(OperationKind::PrepZ, vec![check]));
    }

    SyndromeCycle {
        operations,
        sx_labels: SX_LABELS,
        sz_labels: SZ_LABELS,
    }
}

fn x_row_slots_local(params: &BivariateBicycleParams, row: usize) -> [usize; 6] {
    let n2 = params.ell * params.m;
    [
        shift_x(params, row, params.a1),
        shift_y(params, row, params.a2),
        shift_y(params, row, params.a3),
        n2 + shift_y(params, row, params.b1),
        n2 + shift_x(params, row, params.b2),
        n2 + shift_x(params, row, params.b3),
    ]
}

fn z_row_slots_local(params: &BivariateBicycleParams, row: usize) -> [usize; 6] {
    let n2 = params.ell * params.m;
    [
        shift_y_inverse(params, row, params.b1),
        shift_x_inverse(params, row, params.b2),
        shift_x_inverse(params, row, params.b3),
        n2 + shift_x_inverse(params, row, params.a1),
        n2 + shift_y_inverse(params, row, params.a2),
        n2 + shift_y_inverse(params, row, params.a3),
    ]
}

fn local_to_circuit_targets(local_targets: &[usize; 6], n2: usize) -> [usize; 6] {
    local_targets.map(|target| target + n2)
}

fn shift_x(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    ((x + shift) % params.ell) * params.m + y
}

fn shift_y(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    x * params.m + (y + shift) % params.m
}

fn shift_x_inverse(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    ((x + params.ell - shift % params.ell) % params.ell) * params.m + y
}

fn shift_y_inverse(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    x * params.m + (y + params.m - shift % params.m) % params.m
}

fn parse_schedule_slot(label: &str) -> Option<usize> {
    if label == "idle" {
        None
    } else {
        label.parse::<usize>().ok()
    }
}

fn append_idle_untouched_data(
    operations: &mut Vec<Operation>,
    code: &BbCode,
    touched_data: impl IntoIterator<Item = usize>,
) {
    let mut touched = vec![false; code.num_circuit_qubits()];
    for qubit in touched_data {
        touched[qubit] = true;
    }

    for &data in &code.data_qubits {
        if !touched[data] {
            operations.push(Operation::new(OperationKind::Idle, vec![data]));
        }
    }
}

fn sparse_to_dense_rows(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0u8; width];
            for &col in row {
                dense[col] ^= 1;
            }
            dense
        })
        .collect()
}

fn dense_to_sparse_row(row: Vec<u8>) -> Vec<usize> {
    row.into_iter()
        .enumerate()
        .filter_map(|(index, bit)| (bit == 1).then_some(index))
        .collect()
}

fn rank(rows: &[Vec<u8>]) -> usize {
    rref(rows).1.len()
}

fn rref(rows: &[Vec<u8>]) -> (Vec<Vec<u8>>, Vec<usize>) {
    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let width = rows[0].len();
    let mut reduced = rows.to_vec();
    let mut pivot_cols = Vec::new();
    let mut pivot_row = 0usize;

    for col in 0..width {
        let Some(found) = (pivot_row..reduced.len()).find(|&row| reduced[row][col] == 1) else {
            continue;
        };
        reduced.swap(pivot_row, found);

        for row in 0..reduced.len() {
            if row != pivot_row && reduced[row][col] == 1 {
                for entry in col..width {
                    reduced[row][entry] ^= reduced[pivot_row][entry];
                }
            }
        }

        pivot_cols.push(col);
        pivot_row += 1;
        if pivot_row == reduced.len() {
            break;
        }
    }

    (reduced, pivot_cols)
}

fn nullspace(rows: &[Vec<u8>], width: usize) -> Vec<Vec<u8>> {
    if rows.is_empty() {
        return (0..width)
            .map(|free_col| {
                let mut row = vec![0u8; width];
                row[free_col] = 1;
                row
            })
            .collect();
    }

    let (reduced, pivot_cols) = rref(rows);
    let mut is_pivot = vec![false; width];
    for &pivot in &pivot_cols {
        is_pivot[pivot] = true;
    }

    let mut basis = Vec::new();
    for free_col in 0..width {
        if is_pivot[free_col] {
            continue;
        }
        let mut vector = vec![0u8; width];
        vector[free_col] = 1;
        for (pivot_row, &pivot_col) in pivot_cols.iter().enumerate() {
            vector[pivot_col] = reduced[pivot_row][free_col];
        }
        basis.push(vector);
    }
    basis
}

fn in_row_span(span_rows: &[Vec<u8>], target: &[u8]) -> bool {
    if span_rows.is_empty() {
        return target.iter().all(|bit| *bit == 0);
    }

    rank(&{
        let mut augmented = span_rows.to_vec();
        augmented.push(target.to_vec());
        augmented
    }) == rank(span_rows)
}

fn select_logical_rows(
    candidates: Vec<Vec<u8>>,
    stabilizers: &[Vec<u8>],
    count: usize,
) -> Vec<Vec<u8>> {
    let mut span_rows = stabilizers.to_vec();
    let mut logicals = Vec::with_capacity(count);

    for candidate in candidates {
        if in_row_span(&span_rows, &candidate) {
            continue;
        }

        span_rows.push(candidate.clone());
        logicals.push(candidate);
        if logicals.len() == count {
            break;
        }
    }

    logicals
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationConfig {
    pub physical_error_rate: f64,
    pub num_cycles: usize,
    pub num_trials: usize,
    pub seed: Option<u64>,
    pub max_bp_iterations: usize,
    pub osd_order: usize,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            physical_error_rate: 0.003,
            num_cycles: 12,
            num_trials: 50_000,
            seed: None,
            max_bp_iterations: 10_000,
            osd_order: 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationResult {
    pub physical_error_rate: f64,
    pub num_cycles: usize,
    pub num_trials: usize,
    pub num_failed_trials: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveModels {
    pub z_faults: EffectiveDecoderModel,
    pub x_faults: EffectiveDecoderModel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveDecoderModel {
    pub decoder: ParityCheckMatrix,
    pub augmented_columns: Vec<Vec<usize>>,
    pub channel_probs: Vec<f64>,
    pub first_logical_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultBasis {
    Z,
    X,
}

impl FaultBasis {
    fn measurement_kind(self) -> OperationKind {
        match self {
            Self::Z => OperationKind::MeasX,
            Self::X => OperationKind::MeasZ,
        }
    }

    fn logical_rows<'a>(self, code: &'a BbCode) -> &'a [Vec<usize>] {
        match self {
            Self::Z => code.logical_x_rows(),
            Self::X => code.logical_z_rows(),
        }
    }

    fn check_index(self, code: &BbCode, qubit: usize) -> usize {
        match self {
            Self::Z => qubit,
            Self::X => qubit - (3 * code.n2()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarginalFault {
    Single(usize),
    Pair([usize; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauliAxis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauliFault {
    Single {
        qubit: usize,
        axis: PauliAxis,
    },
    TwoQubit {
        qubits: [usize; 2],
        axes: [PauliAxis; 2],
    },
}

#[derive(Debug, Clone, Copy)]
struct EffectiveFaultCandidate {
    fault: MarginalFault,
    probability: f64,
    before_operation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrialSample {
    z_syndrome: Vec<bool>,
    x_syndrome: Vec<bool>,
    z_logical: Vec<bool>,
    x_logical: Vec<bool>,
}

pub fn build_effective_models(
    code: &BbCode,
    cycle: &SyndromeCycle,
    config: &SimulationConfig,
) -> Result<EffectiveModels, String> {
    validate_model_config(config)?;
    Ok(EffectiveModels {
        z_faults: build_effective_model_for_basis(code, cycle, config, FaultBasis::Z)?,
        x_faults: build_effective_model_for_basis(code, cycle, config, FaultBasis::X)?,
    })
}

pub fn run_simulation(config: SimulationConfig) -> Result<SimulationResult, String> {
    validate_simulation_config(&config)?;

    let code = build_upstream_code()?;
    let cycle = build_syndrome_cycle(&code);
    let models = build_effective_models(&code, &cycle, &config)?;

    if models.z_faults.channel_probs.is_empty() || models.x_faults.channel_probs.is_empty() {
        return Err("effective decoder models must contain at least one probability column".into());
    }

    let decoder_config = DecoderConfig {
        max_bp_iterations: config.max_bp_iterations,
        osd_order: config.osd_order,
        ..DecoderConfig::default()
    };

    let z_decoder = BpOsdDecoder::new(
        models.z_faults.decoder.clone(),
        ChannelModel::BitFlipProbabilities(models.z_faults.channel_probs.clone()),
        decoder_config,
    )
    .map_err(|error| format!("failed to compile Z-fault rbposd decoder: {error}"))?;

    let x_decoder = BpOsdDecoder::new(
        models.x_faults.decoder.clone(),
        ChannelModel::BitFlipProbabilities(models.x_faults.channel_probs.clone()),
        decoder_config,
    )
    .map_err(|error| format!("failed to compile X-fault rbposd decoder: {error}"))?;

    let mut rng = match config.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    };

    let mut num_failed_trials = 0usize;
    for _ in 0..config.num_trials {
        let sample = simulate_trial(
            &code,
            &cycle,
            config.num_cycles,
            config.physical_error_rate,
            &mut rng,
        );
        let predicted_z =
            decode_logicals(&z_decoder, &models.z_faults, &sample.z_syndrome, code.k())
                .map_err(|error| format!("failed to decode Z faults: {error}"))?;
        if predicted_z != sample.z_logical {
            num_failed_trials += 1;
            continue;
        }

        let predicted_x =
            decode_logicals(&x_decoder, &models.x_faults, &sample.x_syndrome, code.k())
                .map_err(|error| format!("failed to decode X faults: {error}"))?;
        if predicted_x != sample.x_logical {
            num_failed_trials += 1;
        }
    }

    Ok(SimulationResult {
        physical_error_rate: config.physical_error_rate,
        num_cycles: config.num_cycles,
        num_trials: config.num_trials,
        num_failed_trials,
    })
}

fn validate_model_config(config: &SimulationConfig) -> Result<(), String> {
    validate_physical_error_rate(config.physical_error_rate)?;
    if config.num_cycles == 0 {
        return Err("num_cycles must be greater than zero".into());
    }
    if config.max_bp_iterations == 0 {
        return Err("max_bp_iterations must be greater than zero".into());
    }
    Ok(())
}

fn validate_simulation_config(config: &SimulationConfig) -> Result<(), String> {
    validate_model_config(config)?;
    if config.num_trials == 0 {
        return Err("num_trials must be greater than zero".into());
    }
    Ok(())
}

fn validate_physical_error_rate(physical_error_rate: f64) -> Result<(), String> {
    if !physical_error_rate.is_finite() || physical_error_rate < 0.0 || physical_error_rate >= 1.0
    {
        return Err("physical_error_rate must be finite and lie in [0, 1)".into());
    }
    Ok(())
}

fn build_effective_model_for_basis(
    code: &BbCode,
    cycle: &SyndromeCycle,
    config: &SimulationConfig,
    basis: FaultBasis,
) -> Result<EffectiveDecoderModel, String> {
    let total_cycles = config.num_cycles + 2;
    let num_checks = code.n2();
    let first_logical_row = num_checks * total_cycles;
    let mut grouped_columns = BTreeMap::<Vec<usize>, f64>::new();

    for noisy_cycle in 0..config.num_cycles {
        for (op_index, operation) in cycle.operations().iter().enumerate() {
            for candidate in
                effective_fault_candidates(operation, basis, config.physical_error_rate)
            {
                let mut state = vec![false; code.num_circuit_qubits()];
                apply_marginal_fault(&mut state, candidate.fault);
                let (start_cycle, start_op) = suffix_start(noisy_cycle, op_index, cycle, candidate);
                let (mut augmented_support, logical_bits) = simulate_basis_suffix(
                    code,
                    cycle,
                    basis,
                    total_cycles,
                    start_cycle,
                    start_op,
                    state,
                );
                append_logical_rows(&mut augmented_support, &logical_bits, first_logical_row);
                *grouped_columns.entry(augmented_support).or_insert(0.0) += candidate.probability;
            }
        }
    }

    if grouped_columns.is_empty() {
        return Err("effective decoder model contained no grouped columns".into());
    }

    let augmented_columns = grouped_columns.keys().cloned().collect::<Vec<_>>();
    let channel_probs = grouped_columns
        .values()
        .copied()
        .map(clamp_decoder_probability)
        .collect::<Result<Vec<_>, _>>()?;
    let decoder_columns = augmented_columns
        .iter()
        .map(|column| {
            column
                .iter()
                .copied()
                .filter(|&row| row < first_logical_row)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let decoder = ParityCheckMatrix::from_sparse_columns(
        first_logical_row,
        decoder_columns.len(),
        decoder_columns,
    )
    .map_err(|error| format!("invalid effective decoder matrix: {error}"))?;

    Ok(EffectiveDecoderModel {
        decoder,
        augmented_columns,
        channel_probs,
        first_logical_row,
    })
}

fn clamp_decoder_probability(probability: f64) -> Result<f64, String> {
    const EPSILON: f64 = 1.0e-15;

    if !probability.is_finite() {
        return Err("effective decoder probability must be finite".into());
    }

    Ok(if probability <= 0.0 {
        EPSILON
    } else if probability >= 1.0 {
        1.0 - EPSILON
    } else {
        probability
    })
}

fn effective_fault_candidates(
    operation: &Operation,
    basis: FaultBasis,
    physical_error_rate: f64,
) -> Vec<EffectiveFaultCandidate> {
    let qubits = operation.qubits();
    match operation.kind() {
        OperationKind::Idle => vec![EffectiveFaultCandidate {
            fault: MarginalFault::Single(qubits[0]),
            probability: (2.0 * physical_error_rate) / 3.0,
            before_operation: false,
        }],
        OperationKind::Cnot => vec![
            EffectiveFaultCandidate {
                fault: MarginalFault::Single(qubits[0]),
                probability: (4.0 * physical_error_rate) / 15.0,
                before_operation: false,
            },
            EffectiveFaultCandidate {
                fault: MarginalFault::Single(qubits[1]),
                probability: (4.0 * physical_error_rate) / 15.0,
                before_operation: false,
            },
            EffectiveFaultCandidate {
                fault: MarginalFault::Pair([qubits[0], qubits[1]]),
                probability: (4.0 * physical_error_rate) / 15.0,
                before_operation: false,
            },
        ],
        OperationKind::PrepX | OperationKind::MeasX if basis == FaultBasis::Z => {
            vec![EffectiveFaultCandidate {
                fault: MarginalFault::Single(qubits[0]),
                probability: physical_error_rate,
                before_operation: operation.kind() == OperationKind::MeasX,
            }]
        }
        OperationKind::PrepZ | OperationKind::MeasZ if basis == FaultBasis::X => {
            vec![EffectiveFaultCandidate {
                fault: MarginalFault::Single(qubits[0]),
                probability: physical_error_rate,
                before_operation: operation.kind() == OperationKind::MeasZ,
            }]
        }
        _ => Vec::new(),
    }
}

fn suffix_start(
    noisy_cycle: usize,
    op_index: usize,
    cycle: &SyndromeCycle,
    candidate: EffectiveFaultCandidate,
) -> (usize, usize) {
    if candidate.before_operation {
        return (noisy_cycle, op_index);
    }

    let mut start_cycle = noisy_cycle;
    let mut start_op = op_index + 1;
    if start_op == cycle.operations().len() {
        start_cycle += 1;
        start_op = 0;
    }
    (start_cycle, start_op)
}

fn simulate_basis_suffix(
    code: &BbCode,
    cycle: &SyndromeCycle,
    basis: FaultBasis,
    total_cycles: usize,
    start_cycle: usize,
    start_op: usize,
    mut state: Vec<bool>,
) -> (Vec<usize>, Vec<bool>) {
    let num_checks = code.n2();
    let mut raw_measurements = vec![vec![false; num_checks]; total_cycles];

    for cycle_index in start_cycle..total_cycles {
        let operations = if cycle_index == start_cycle {
            &cycle.operations()[start_op..]
        } else {
            cycle.operations()
        };

        for operation in operations {
            apply_basis_operation(&mut state, operation, basis);
            if operation.kind() == basis.measurement_kind() {
                let qubit = operation.qubits()[0];
                let check_index = basis.check_index(code, qubit);
                raw_measurements[cycle_index][check_index] = state[qubit];
            }
        }
    }

    (
        flatten_syndrome_differences(&raw_measurements),
        extract_logical_vector(code, basis.logical_rows(code), &state),
    )
}

fn apply_basis_operation(state: &mut [bool], operation: &Operation, basis: FaultBasis) {
    match operation.kind() {
        OperationKind::Cnot => {
            let control = operation.qubits()[0];
            let target = operation.qubits()[1];
            match basis {
                FaultBasis::Z => state[control] ^= state[target],
                FaultBasis::X => state[target] ^= state[control],
            }
        }
        OperationKind::PrepX if basis == FaultBasis::Z => state[operation.qubits()[0]] = false,
        OperationKind::PrepZ if basis == FaultBasis::X => state[operation.qubits()[0]] = false,
        _ => {}
    }
}

fn apply_marginal_fault(state: &mut [bool], fault: MarginalFault) {
    match fault {
        MarginalFault::Single(qubit) => state[qubit] ^= true,
        MarginalFault::Pair(qubits) => {
            state[qubits[0]] ^= true;
            state[qubits[1]] ^= true;
        }
    }
}

fn append_logical_rows(
    augmented_support: &mut Vec<usize>,
    logical_bits: &[bool],
    first_logical_row: usize,
) {
    for (logical_index, &bit) in logical_bits.iter().enumerate() {
        if bit {
            augmented_support.push(first_logical_row + logical_index);
        }
    }
}

fn flatten_syndrome_differences(raw_measurements: &[Vec<bool>]) -> Vec<usize> {
    let mut support = Vec::new();
    let num_checks = raw_measurements.first().map_or(0, Vec::len);

    for round in 0..raw_measurements.len() {
        for check in 0..num_checks {
            let bit = if round == 0 {
                raw_measurements[round][check]
            } else {
                raw_measurements[round][check] ^ raw_measurements[round - 1][check]
            };
            if bit {
                support.push(round * num_checks + check);
            }
        }
    }

    support
}

fn extract_logical_vector(code: &BbCode, logical_rows: &[Vec<usize>], state: &[bool]) -> Vec<bool> {
    logical_rows
        .iter()
        .map(|row| {
            row.iter().fold(false, |parity, &local_column| {
                parity ^ state[code.n2() + local_column]
            })
        })
        .collect()
}

fn decode_logicals(
    decoder: &BpOsdDecoder,
    model: &EffectiveDecoderModel,
    syndrome_bits: &[bool],
    num_logicals: usize,
) -> Result<Vec<bool>, String> {
    let correction = decoder
        .decode(&Syndrome::from(syndrome_bits.to_vec()))
        .map_err(|error| format!("rbposd decode failed: {error}"))?;
    Ok(correction_to_logicals(
        &correction.correction,
        model,
        num_logicals,
    ))
}

fn correction_to_logicals(
    correction: &Correction,
    model: &EffectiveDecoderModel,
    num_logicals: usize,
) -> Vec<bool> {
    let mut logicals = vec![false; num_logicals];
    for (column, &enabled) in correction.as_slice().iter().enumerate() {
        if !enabled {
            continue;
        }
        for &row in &model.augmented_columns[column] {
            if row >= model.first_logical_row {
                logicals[row - model.first_logical_row] ^= true;
            }
        }
    }
    logicals
}

fn simulate_trial<R: Rng + ?Sized>(
    code: &BbCode,
    cycle: &SyndromeCycle,
    num_cycles: usize,
    physical_error_rate: f64,
    rng: &mut R,
) -> TrialSample {
    let total_cycles = num_cycles + 2;
    let mut z_state = vec![false; code.num_circuit_qubits()];
    let mut x_state = vec![false; code.num_circuit_qubits()];
    let mut x_check_measurements = vec![vec![false; code.n2()]; total_cycles];
    let mut z_check_measurements = vec![vec![false; code.n2()]; total_cycles];

    for cycle_index in 0..total_cycles {
        let is_noisy_cycle = cycle_index < num_cycles;
        for operation in cycle.operations() {
            let sampled_fault = if is_noisy_cycle {
                sample_operation_fault(operation, physical_error_rate, rng)
            } else {
                None
            };
            if matches!(
                operation.kind(),
                OperationKind::MeasX | OperationKind::MeasZ
            ) {
                if let Some(fault) = sampled_fault {
                    apply_pauli_fault(&mut x_state, &mut z_state, fault);
                }
            }

            apply_basis_operation(&mut z_state, operation, FaultBasis::Z);
            apply_basis_operation(&mut x_state, operation, FaultBasis::X);

            match operation.kind() {
                OperationKind::MeasX => {
                    let qubit = operation.qubits()[0];
                    x_check_measurements[cycle_index][qubit] = z_state[qubit];
                }
                OperationKind::MeasZ => {
                    let qubit = operation.qubits()[0];
                    z_check_measurements[cycle_index][qubit - (3 * code.n2())] = x_state[qubit];
                }
                _ => {}
            }

            if !matches!(
                operation.kind(),
                OperationKind::MeasX | OperationKind::MeasZ
            ) {
                if let Some(fault) = sampled_fault {
                    apply_pauli_fault(&mut x_state, &mut z_state, fault);
                }
            }
        }
    }

    TrialSample {
        z_syndrome: flatten_syndrome_bits(&x_check_measurements),
        x_syndrome: flatten_syndrome_bits(&z_check_measurements),
        z_logical: extract_logical_vector(code, code.logical_x_rows(), &z_state),
        x_logical: extract_logical_vector(code, code.logical_z_rows(), &x_state),
    }
}

fn flatten_syndrome_bits(raw_measurements: &[Vec<bool>]) -> Vec<bool> {
    let num_checks = raw_measurements.first().map_or(0, Vec::len);
    let mut bits = vec![false; raw_measurements.len() * num_checks];
    for round in 0..raw_measurements.len() {
        for check in 0..num_checks {
            bits[round * num_checks + check] = if round == 0 {
                raw_measurements[round][check]
            } else {
                raw_measurements[round][check] ^ raw_measurements[round - 1][check]
            };
        }
    }
    bits
}

fn sample_operation_fault<R: Rng + ?Sized>(
    operation: &Operation,
    physical_error_rate: f64,
    rng: &mut R,
) -> Option<PauliFault> {
    if rng.r#gen::<f64>() >= physical_error_rate {
        return None;
    }

    let qubits = operation.qubits();
    match operation.kind() {
        OperationKind::Idle => Some(PauliFault::Single {
            qubit: qubits[0],
            axis: sample_single_axis(rng),
        }),
        OperationKind::Cnot => Some(sample_cnot_fault([qubits[0], qubits[1]], rng)),
        OperationKind::PrepX | OperationKind::MeasX => Some(PauliFault::Single {
            qubit: qubits[0],
            axis: PauliAxis::Z,
        }),
        OperationKind::PrepZ | OperationKind::MeasZ => Some(PauliFault::Single {
            qubit: qubits[0],
            axis: PauliAxis::X,
        }),
    }
}

fn sample_single_axis<R: Rng + ?Sized>(rng: &mut R) -> PauliAxis {
    match rng.gen_range(0..3) {
        0 => PauliAxis::X,
        1 => PauliAxis::Y,
        _ => PauliAxis::Z,
    }
}

fn sample_cnot_fault<R: Rng + ?Sized>(qubits: [usize; 2], rng: &mut R) -> PauliFault {
    cnot_fault_for_index(qubits, rng.gen_range(0..15))
}

fn cnot_fault_for_index(qubits: [usize; 2], index: usize) -> PauliFault {
    match index {
        0 => PauliFault::Single {
            qubit: qubits[0],
            axis: PauliAxis::X,
        },
        1 => PauliFault::Single {
            qubit: qubits[0],
            axis: PauliAxis::Y,
        },
        2 => PauliFault::Single {
            qubit: qubits[0],
            axis: PauliAxis::Z,
        },
        3 => PauliFault::Single {
            qubit: qubits[1],
            axis: PauliAxis::X,
        },
        4 => PauliFault::Single {
            qubit: qubits[1],
            axis: PauliAxis::Y,
        },
        5 => PauliFault::Single {
            qubit: qubits[1],
            axis: PauliAxis::Z,
        },
        6 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::X, PauliAxis::X],
        },
        7 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::Y, PauliAxis::Y],
        },
        8 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::Z, PauliAxis::Z],
        },
        9 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::X, PauliAxis::Y],
        },
        10 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::Y, PauliAxis::X],
        },
        11 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::Y, PauliAxis::Z],
        },
        12 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::Z, PauliAxis::Y],
        },
        13 => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::X, PauliAxis::Z],
        },
        _ => PauliFault::TwoQubit {
            qubits,
            axes: [PauliAxis::Z, PauliAxis::X],
        },
    }
}

fn apply_pauli_fault(x_state: &mut [bool], z_state: &mut [bool], fault: PauliFault) {
    match fault {
        PauliFault::Single { qubit, axis } => apply_pauli_axis(x_state, z_state, qubit, axis),
        PauliFault::TwoQubit { qubits, axes } => {
            apply_pauli_axis(x_state, z_state, qubits[0], axes[0]);
            apply_pauli_axis(x_state, z_state, qubits[1], axes[1]);
        }
    }
}

fn apply_pauli_axis(x_state: &mut [bool], z_state: &mut [bool], qubit: usize, axis: PauliAxis) {
    if matches!(axis, PauliAxis::X | PauliAxis::Y) {
        x_state[qubit] ^= true;
    }
    if matches!(axis, PauliAxis::Z | PauliAxis::Y) {
        z_state[qubit] ^= true;
    }
}

#[cfg(test)]
mod tests {
    use super::{PauliAxis, PauliFault, cnot_fault_for_index};

    #[test]
    fn cnot_fault_indices_match_upstream_order() {
        let qubits = [10, 20];

        let expected = [
            PauliFault::Single {
                qubit: 10,
                axis: PauliAxis::X,
            },
            PauliFault::Single {
                qubit: 10,
                axis: PauliAxis::Y,
            },
            PauliFault::Single {
                qubit: 10,
                axis: PauliAxis::Z,
            },
            PauliFault::Single {
                qubit: 20,
                axis: PauliAxis::X,
            },
            PauliFault::Single {
                qubit: 20,
                axis: PauliAxis::Y,
            },
            PauliFault::Single {
                qubit: 20,
                axis: PauliAxis::Z,
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::X, PauliAxis::X],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::Y, PauliAxis::Y],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::Z, PauliAxis::Z],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::X, PauliAxis::Y],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::Y, PauliAxis::X],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::Y, PauliAxis::Z],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::Z, PauliAxis::Y],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::X, PauliAxis::Z],
            },
            PauliFault::TwoQubit {
                qubits,
                axes: [PauliAxis::Z, PauliAxis::X],
            },
        ];

        for (index, expected_fault) in expected.into_iter().enumerate() {
            assert_eq!(cnot_fault_for_index(qubits, index), expected_fault);
        }
    }
}
