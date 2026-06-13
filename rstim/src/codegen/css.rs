use std::collections::BTreeSet;
use std::fmt;

use qec_code::css::CssCode;
use serde::Deserialize;

use crate::codegen::NoiseParams;
use crate::ir::{StimInstr, StimTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBasis {
    X,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSchedule {
    Sequential,
    Greedy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssCheckMatrices {
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub num_data_qubits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssObservableSource {
    Explicit(Vec<Vec<usize>>),
    CanonicalFallback,
    ExplicitOrCanonical(Vec<Vec<usize>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssMemoryConfig {
    pub checks: CssCheckMatrices,
    pub rounds: usize,
    pub noise: NoiseParams,
    pub basis: MemoryBasis,
    pub schedule: CssSchedule,
    pub observables: CssObservableSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCssMatrix {
    pub num_cols: usize,
    pub rows: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssJsonError {
    Json(String),
    UnknownFormat(String),
    EmptyWidth,
    RaggedDenseRow {
        row: usize,
        expected: usize,
        actual: usize,
    },
    NonBinaryEntry {
        row: usize,
        col: usize,
        value: u8,
    },
    DuplicateIndex {
        row: usize,
        col: usize,
    },
    OutOfRangeIndex {
        row: usize,
        col: usize,
        width: usize,
    },
}

impl fmt::Display for CssJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(f, "{message}"),
            Self::UnknownFormat(format) => write!(f, "unknown CSS matrix format: {format}"),
            Self::EmptyWidth => write!(f, "CSS matrix width must be positive"),
            Self::RaggedDenseRow {
                row,
                expected,
                actual,
            } => write!(f, "dense row {row} has width {actual}, expected {expected}"),
            Self::NonBinaryEntry { row, col, value } => {
                write!(
                    f,
                    "dense row {row} has non-binary entry {value} at column {col}"
                )
            }
            Self::DuplicateIndex { row, col } => write!(f, "sparse row {row} repeats column {col}"),
            Self::OutOfRangeIndex { row, col, width } => {
                write!(
                    f,
                    "sparse row {row} contains out-of-range column {col} for width {width}"
                )
            }
        }
    }
}

impl std::error::Error for CssJsonError {}

#[derive(Debug, Deserialize)]
struct MatrixWrapper {
    format: String,
    num_cols: Option<usize>,
    rows: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssCodegenError {
    InvalidRounds,
    InvalidWidth,
    DuplicateIndex {
        matrix: &'static str,
        row: usize,
        col: usize,
    },
    OutOfRangeIndex {
        matrix: &'static str,
        row: usize,
        col: usize,
        width: usize,
    },
    InvalidCss(String),
    MissingObservables,
    InvalidObservable {
        row: usize,
        col: usize,
        width: usize,
    },
    MixedCanonicalLogical {
        index: usize,
        basis: MemoryBasis,
    },
}

impl fmt::Display for CssCodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRounds => write!(f, "rounds must be >= 1"),
            Self::InvalidWidth => write!(f, "CSS matrices must have at least one data qubit"),
            Self::DuplicateIndex { matrix, row, col } => {
                write!(f, "{matrix} row {row} repeats column {col}")
            }
            Self::OutOfRangeIndex {
                matrix,
                row,
                col,
                width,
            } => write!(
                f,
                "{matrix} row {row} contains out-of-range column {col} for width {width}"
            ),
            Self::InvalidCss(message) => write!(f, "{message}"),
            Self::MissingObservables => {
                write!(f, "canonical logical fallback produced no observables")
            }
            Self::InvalidObservable { row, col, width } => write!(
                f,
                "observable {row} references data qubit {col}, but width is {width}"
            ),
            Self::MixedCanonicalLogical { index, basis } => {
                write!(
                    f,
                    "canonical logical {index} is not pure in memory-{basis:?} basis"
                )
            }
        }
    }
}

impl std::error::Error for CssCodegenError {}

pub fn parse_css_matrix_json(text: &str) -> Result<NormalizedCssMatrix, CssJsonError> {
    parse_matrix_wrapper(text)
}

pub fn parse_css_observable_json(text: &str) -> Result<NormalizedCssMatrix, CssJsonError> {
    let parsed = parse_matrix_wrapper(text)?;
    if parsed.rows.is_empty() {
        return Err(CssJsonError::EmptyWidth);
    }
    Ok(parsed)
}

fn parse_matrix_wrapper(text: &str) -> Result<NormalizedCssMatrix, CssJsonError> {
    let wrapper: MatrixWrapper =
        serde_json::from_str(text).map_err(|error| CssJsonError::Json(error.to_string()))?;
    match wrapper.format.as_str() {
        "dense" => parse_dense_rows(wrapper.rows),
        "sparse_rows" => parse_sparse_rows(wrapper.num_cols, wrapper.rows),
        other => Err(CssJsonError::UnknownFormat(other.to_string())),
    }
}

fn parse_dense_rows(rows: serde_json::Value) -> Result<NormalizedCssMatrix, CssJsonError> {
    let rows: Vec<Vec<u8>> =
        serde_json::from_value(rows).map_err(|error| CssJsonError::Json(error.to_string()))?;
    let width = rows.first().map(Vec::len).ok_or(CssJsonError::EmptyWidth)?;
    if width == 0 {
        return Err(CssJsonError::EmptyWidth);
    }
    let mut supports = Vec::with_capacity(rows.len());
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != width {
            return Err(CssJsonError::RaggedDenseRow {
                row: row_index,
                expected: width,
                actual: row.len(),
            });
        }
        let mut support = Vec::new();
        for (col, &value) in row.iter().enumerate() {
            match value {
                0 => {}
                1 => support.push(col),
                _ => {
                    return Err(CssJsonError::NonBinaryEntry {
                        row: row_index,
                        col,
                        value,
                    });
                }
            }
        }
        supports.push(support);
    }
    Ok(NormalizedCssMatrix {
        num_cols: width,
        rows: supports,
    })
}

fn parse_sparse_rows(
    num_cols: Option<usize>,
    rows: serde_json::Value,
) -> Result<NormalizedCssMatrix, CssJsonError> {
    let width = num_cols.ok_or(CssJsonError::EmptyWidth)?;
    if width == 0 {
        return Err(CssJsonError::EmptyWidth);
    }
    let mut rows: Vec<Vec<usize>> =
        serde_json::from_value(rows).map_err(|error| CssJsonError::Json(error.to_string()))?;
    for (row_index, row) in rows.iter_mut().enumerate() {
        row.sort_unstable();
        let mut previous = None;
        for &col in row.iter() {
            if col >= width {
                return Err(CssJsonError::OutOfRangeIndex {
                    row: row_index,
                    col,
                    width,
                });
            }
            if previous == Some(col) {
                return Err(CssJsonError::DuplicateIndex {
                    row: row_index,
                    col,
                });
            }
            previous = Some(col);
        }
    }
    Ok(NormalizedCssMatrix {
        num_cols: width,
        rows,
    })
}

pub fn css_memory(config: CssMemoryConfig) -> Result<Vec<StimInstr>, CssCodegenError> {
    if config.rounds == 0 {
        return Err(CssCodegenError::InvalidRounds);
    }
    validate_supports("hx", &config.checks.hx, config.checks.num_data_qubits)?;
    validate_supports("hz", &config.checks.hz, config.checks.num_data_qubits)?;
    validate_css_orthogonality(&config.checks.hx, &config.checks.hz)?;
    let observables = resolve_observables(&config)?;
    emit_css_memory_circuit(&config, &observables)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckKind {
    X,
    Z,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Check {
    kind: CheckKind,
    row: usize,
    ancilla: u32,
    support: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CnotInteraction {
    control: u32,
    target: u32,
}

fn resolve_observables(config: &CssMemoryConfig) -> Result<Vec<Vec<usize>>, CssCodegenError> {
    match &config.observables {
        CssObservableSource::Explicit(rows) | CssObservableSource::ExplicitOrCanonical(rows)
            if !rows.is_empty() =>
        {
            validate_observables(rows, config.checks.num_data_qubits)?;
            Ok(rows.clone())
        }
        CssObservableSource::ExplicitOrCanonical(_) | CssObservableSource::CanonicalFallback => {
            let hx_dense = supports_to_dense(&config.checks.hx, config.checks.num_data_qubits);
            let hz_dense = supports_to_dense(&config.checks.hz, config.checks.num_data_qubits);
            let css_code = CssCode::from_hx_hz(hx_dense, hz_dense)
                .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
            canonical_observables(config, &css_code)
        }
        CssObservableSource::Explicit(rows) => {
            validate_observables(rows, config.checks.num_data_qubits)?;
            Ok(rows.clone())
        }
    }
}

fn canonical_observables(
    config: &CssMemoryConfig,
    css_code: &CssCode,
) -> Result<Vec<Vec<usize>>, CssCodegenError> {
    let basis = css_code
        .code()
        .canonical_logical_basis()
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    let logicals = match config.basis {
        MemoryBasis::X => basis.logical_x,
        MemoryBasis::Z => basis.logical_z,
    };
    let mut observables = Vec::with_capacity(logicals.len());
    for (index, logical) in logicals.iter().enumerate() {
        let support = match config.basis {
            MemoryBasis::X => {
                if logical.z_bits().iter().any(|&bit| bit != 0) {
                    return Err(CssCodegenError::MixedCanonicalLogical {
                        index,
                        basis: config.basis,
                    });
                }
                logical
                    .x_bits()
                    .iter()
                    .enumerate()
                    .filter_map(|(qubit, &bit)| (bit == 1).then_some(qubit))
                    .collect()
            }
            MemoryBasis::Z => {
                if logical.x_bits().iter().any(|&bit| bit != 0) {
                    return Err(CssCodegenError::MixedCanonicalLogical {
                        index,
                        basis: config.basis,
                    });
                }
                logical
                    .z_bits()
                    .iter()
                    .enumerate()
                    .filter_map(|(qubit, &bit)| (bit == 1).then_some(qubit))
                    .collect()
            }
        };
        observables.push(support);
    }
    validate_observables(&observables, config.checks.num_data_qubits)?;
    Ok(observables)
}

fn validate_observables(rows: &[Vec<usize>], width: usize) -> Result<(), CssCodegenError> {
    if rows.is_empty() {
        return Err(CssCodegenError::MissingObservables);
    }
    for (row_index, row) in rows.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for &col in row {
            if col >= width {
                return Err(CssCodegenError::InvalidObservable {
                    row: row_index,
                    col,
                    width,
                });
            }
            if !seen.insert(col) {
                return Err(CssCodegenError::InvalidObservable {
                    row: row_index,
                    col,
                    width,
                });
            }
        }
    }
    Ok(())
}

fn emit_css_memory_circuit(
    config: &CssMemoryConfig,
    observables: &[Vec<usize>],
) -> Result<Vec<StimInstr>, CssCodegenError> {
    let width = config.checks.num_data_qubits;
    let checks = build_checks(&config.checks);
    let num_checks = checks.len();
    let mut instrs = Vec::new();

    for q in 0..width {
        instrs.push(op(
            "QUBIT_COORDS",
            &[q as f64],
            &[StimTarget::Qubit(q as u32)],
        ));
    }
    for (index, check) in checks.iter().enumerate() {
        instrs.push(op(
            "QUBIT_COORDS",
            &[width as f64, index as f64],
            &[StimTarget::Qubit(check.ancilla)],
        ));
    }

    let reset_data = match config.basis {
        MemoryBasis::X => "RX",
        MemoryBasis::Z => "R",
    };
    for q in 0..width {
        instrs.push(op(reset_data, &[], &[StimTarget::Qubit(q as u32)]));
    }
    if config.noise.after_reset_flip_probability > 0.0 {
        for q in 0..width {
            instrs.push(op(
                "X_ERROR",
                &[config.noise.after_reset_flip_probability],
                &[StimTarget::Qubit(q as u32)],
            ));
        }
    }
    for check in &checks {
        instrs.push(op("R", &[], &[StimTarget::Qubit(check.ancilla)]));
    }
    if config.noise.after_reset_flip_probability > 0.0 {
        for check in &checks {
            instrs.push(op(
                "X_ERROR",
                &[config.noise.after_reset_flip_probability],
                &[StimTarget::Qubit(check.ancilla)],
            ));
        }
    }

    for round in 0..config.rounds {
        if round > 0 {
            instrs.push(op("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]));
        }
        emit_round(&mut instrs, config, &checks);
        emit_round_detectors(&mut instrs, config, &checks, round, num_checks);
    }

    instrs.push(op("TICK", &[], &[]));
    if config.noise.before_measure_flip_probability > 0.0 {
        for q in 0..width {
            instrs.push(op(
                "X_ERROR",
                &[config.noise.before_measure_flip_probability],
                &[StimTarget::Qubit(q as u32)],
            ));
        }
    }
    let measure_data = match config.basis {
        MemoryBasis::X => "MX",
        MemoryBasis::Z => "M",
    };
    for q in 0..width {
        instrs.push(op(measure_data, &[], &[StimTarget::Qubit(q as u32)]));
    }
    emit_tail_detectors(&mut instrs, config, &checks, width, num_checks);
    emit_observables(&mut instrs, observables, width);

    Ok(instrs)
}

fn build_checks(matrices: &CssCheckMatrices) -> Vec<Check> {
    let x_base = matrices.num_data_qubits as u32;
    let z_base = x_base + matrices.hx.len() as u32;
    let mut checks = Vec::with_capacity(matrices.hx.len() + matrices.hz.len());
    for (row, support) in matrices.hx.iter().enumerate() {
        checks.push(Check {
            kind: CheckKind::X,
            row,
            ancilla: x_base + row as u32,
            support: support.clone(),
        });
    }
    for (row, support) in matrices.hz.iter().enumerate() {
        checks.push(Check {
            kind: CheckKind::Z,
            row,
            ancilla: z_base + row as u32,
            support: support.clone(),
        });
    }
    checks
}

fn emit_round(instrs: &mut Vec<StimInstr>, config: &CssMemoryConfig, checks: &[Check]) {
    instrs.push(op("TICK", &[], &[]));
    if config.noise.before_round_data_depolarization > 0.0 {
        for q in 0..config.checks.num_data_qubits {
            instrs.push(op(
                "DEPOLARIZE1",
                &[config.noise.before_round_data_depolarization],
                &[StimTarget::Qubit(q as u32)],
            ));
        }
    }
    let x_checks: Vec<_> = checks
        .iter()
        .filter(|check| check.kind == CheckKind::X)
        .cloned()
        .collect();
    let z_checks: Vec<_> = checks
        .iter()
        .filter(|check| check.kind == CheckKind::Z)
        .cloned()
        .collect();
    if !x_checks.is_empty() {
        emit_x_check_measurements(instrs, config, &x_checks);
    }
    if !z_checks.is_empty() {
        emit_z_check_measurements(instrs, config, &z_checks);
    }
}

fn emit_x_check_measurements(
    instrs: &mut Vec<StimInstr>,
    config: &CssMemoryConfig,
    checks: &[Check],
) {
    for check in checks {
        instrs.push(op("H", &[], &[StimTarget::Qubit(check.ancilla)]));
    }
    if config.noise.after_clifford_depolarization > 0.0 {
        for check in checks {
            instrs.push(op(
                "DEPOLARIZE1",
                &[config.noise.after_clifford_depolarization],
                &[StimTarget::Qubit(check.ancilla)],
            ));
        }
    }
    emit_cnot_layers(instrs, config, checks);
    instrs.push(op("TICK", &[], &[]));
    for check in checks {
        instrs.push(op("H", &[], &[StimTarget::Qubit(check.ancilla)]));
    }
    if config.noise.after_clifford_depolarization > 0.0 {
        for check in checks {
            instrs.push(op(
                "DEPOLARIZE1",
                &[config.noise.after_clifford_depolarization],
                &[StimTarget::Qubit(check.ancilla)],
            ));
        }
    }
    emit_check_measurements(instrs, config, checks);
}

fn emit_z_check_measurements(
    instrs: &mut Vec<StimInstr>,
    config: &CssMemoryConfig,
    checks: &[Check],
) {
    emit_cnot_layers(instrs, config, checks);
    emit_check_measurements(instrs, config, checks);
}

fn emit_cnot_layers(instrs: &mut Vec<StimInstr>, config: &CssMemoryConfig, checks: &[Check]) {
    for layer in schedule_layers(config.schedule, checks) {
        instrs.push(op("TICK", &[], &[]));
        let targets: Vec<_> = layer
            .iter()
            .flat_map(|cnot| {
                [
                    StimTarget::Qubit(cnot.control),
                    StimTarget::Qubit(cnot.target),
                ]
            })
            .collect();
        if !targets.is_empty() {
            instrs.push(op("CX", &[], &targets));
        }
        if config.noise.after_clifford_depolarization > 0.0 && !targets.is_empty() {
            instrs.push(op(
                "DEPOLARIZE2",
                &[config.noise.after_clifford_depolarization],
                &targets,
            ));
        }
    }
}

fn emit_check_measurements(
    instrs: &mut Vec<StimInstr>,
    config: &CssMemoryConfig,
    checks: &[Check],
) {
    instrs.push(op("TICK", &[], &[]));
    if config.noise.before_measure_flip_probability > 0.0 {
        for check in checks {
            instrs.push(op(
                "X_ERROR",
                &[config.noise.before_measure_flip_probability],
                &[StimTarget::Qubit(check.ancilla)],
            ));
        }
    }
    for check in checks {
        instrs.push(op("MR", &[], &[StimTarget::Qubit(check.ancilla)]));
    }
    if config.noise.after_reset_flip_probability > 0.0 {
        for check in checks {
            instrs.push(op(
                "X_ERROR",
                &[config.noise.after_reset_flip_probability],
                &[StimTarget::Qubit(check.ancilla)],
            ));
        }
    }
}

fn schedule_layers(schedule: CssSchedule, checks: &[Check]) -> Vec<Vec<CnotInteraction>> {
    let interactions = cnot_interactions(checks);
    match schedule {
        CssSchedule::Sequential => interactions.into_iter().map(|cnot| vec![cnot]).collect(),
        CssSchedule::Greedy => {
            let mut layers: Vec<Vec<CnotInteraction>> = Vec::new();
            for cnot in interactions {
                if let Some(layer) = layers
                    .iter_mut()
                    .find(|layer| cnot_fits_layer(&cnot, layer))
                {
                    layer.push(cnot);
                } else {
                    layers.push(vec![cnot]);
                }
            }
            layers
        }
    }
}

fn cnot_fits_layer(cnot: &CnotInteraction, layer: &[CnotInteraction]) -> bool {
    layer.iter().all(|existing| {
        existing.control != cnot.control
            && existing.target != cnot.control
            && existing.control != cnot.target
            && existing.target != cnot.target
    })
}

fn cnot_interactions(checks: &[Check]) -> Vec<CnotInteraction> {
    let mut interactions = Vec::new();
    for check in checks {
        for &data in &check.support {
            match check.kind {
                CheckKind::X => interactions.push(CnotInteraction {
                    control: check.ancilla,
                    target: data as u32,
                }),
                CheckKind::Z => interactions.push(CnotInteraction {
                    control: data as u32,
                    target: check.ancilla,
                }),
            }
        }
    }
    interactions
}

fn emit_round_detectors(
    instrs: &mut Vec<StimInstr>,
    config: &CssMemoryConfig,
    checks: &[Check],
    round: usize,
    num_checks: usize,
) {
    for (order, check) in checks.iter().enumerate() {
        if round == 0 && !check_is_deterministic(config.basis, check.kind) {
            continue;
        }
        let current = -((num_checks - order) as i32);
        let targets = if round == 0 {
            vec![StimTarget::Rec(current)]
        } else {
            vec![
                StimTarget::Rec(current),
                StimTarget::Rec(current - num_checks as i32),
            ]
        };
        instrs.push(op("DETECTOR", &[order as f64, 0.0], &targets));
    }
}

fn emit_tail_detectors(
    instrs: &mut Vec<StimInstr>,
    config: &CssMemoryConfig,
    checks: &[Check],
    width: usize,
    num_checks: usize,
) {
    for (order, check) in checks.iter().enumerate() {
        if !check_is_deterministic(config.basis, check.kind) {
            continue;
        }
        let mut targets: Vec<StimTarget> = check
            .support
            .iter()
            .map(|&data| StimTarget::Rec(-((width - data) as i32)))
            .collect();
        targets.push(StimTarget::Rec(-((width + num_checks - order) as i32)));
        targets.sort_by_key(|target| match target {
            StimTarget::Rec(offset) => *offset,
            _ => 0,
        });
        instrs.push(op("DETECTOR", &[order as f64, 1.0], &targets));
    }
}

fn emit_observables(instrs: &mut Vec<StimInstr>, observables: &[Vec<usize>], width: usize) {
    for (index, support) in observables.iter().enumerate() {
        let mut targets: Vec<StimTarget> = support
            .iter()
            .map(|&data| StimTarget::Rec(-((width - data) as i32)))
            .collect();
        targets.sort_by_key(|target| match target {
            StimTarget::Rec(offset) => *offset,
            _ => 0,
        });
        instrs.push(op("OBSERVABLE_INCLUDE", &[index as f64], &targets));
    }
}

fn check_is_deterministic(basis: MemoryBasis, kind: CheckKind) -> bool {
    matches!(
        (basis, kind),
        (MemoryBasis::X, CheckKind::X) | (MemoryBasis::Z, CheckKind::Z)
    )
}

fn validate_supports(
    matrix: &'static str,
    rows: &[Vec<usize>],
    width: usize,
) -> Result<(), CssCodegenError> {
    if width == 0 {
        return Err(CssCodegenError::InvalidWidth);
    }
    for (row_index, row) in rows.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for &col in row {
            if col >= width {
                return Err(CssCodegenError::OutOfRangeIndex {
                    matrix,
                    row: row_index,
                    col,
                    width,
                });
            }
            if !seen.insert(col) {
                return Err(CssCodegenError::DuplicateIndex {
                    matrix,
                    row: row_index,
                    col,
                });
            }
        }
    }
    Ok(())
}

fn validate_css_orthogonality(hx: &[Vec<usize>], hz: &[Vec<usize>]) -> Result<(), CssCodegenError> {
    let hz_supports: Vec<_> = hz
        .iter()
        .map(|row| row.iter().copied().collect::<BTreeSet<_>>())
        .collect();
    for x_row in hx {
        for z_support in &hz_supports {
            let parity = x_row
                .iter()
                .filter(|&&qubit| z_support.contains(&qubit))
                .count()
                % 2;
            if parity != 0 {
                return Err(CssCodegenError::InvalidCss(
                    "CSS X/Z checks are not orthogonal".into(),
                ));
            }
        }
    }
    Ok(())
}

fn supports_to_dense(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0; width];
            for &col in row {
                dense[col] = 1;
            }
            dense
        })
        .collect()
}

#[allow(dead_code)]
fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}
