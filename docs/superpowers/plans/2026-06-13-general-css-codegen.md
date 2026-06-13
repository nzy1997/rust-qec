# General CSS Codegen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build issue #46 end to end: general CSS memory circuit codegen from `hx/hz`, CSS CLI generation, and CSS-backed `rsinter` benchmark inputs.

**Architecture:** `qec-code` validates CSS matrices and derives canonical logicals. `rstim::codegen::css` normalizes inputs, schedules CNOTs, emits `StimInstr`, detectors, and observables. `rsinter` expands benchmark points into circuit sources so existing decoder runners can benchmark either legacy rotated-surface circuits or CSS matrix files.

**Tech Stack:** Rust 2024 workspace, `serde`/`serde_json` for JSON wrappers, `clap` for CLI, `toml` for `rsinter` specs, existing `rstim` IR/sampler/DEM analyzer, existing `qec-code` CSS/logical APIs.

---

## Source Documents

- Design spec: `docs/superpowers/specs/2026-06-13-general-css-codegen-design.md`
- GitHub issue: `https://github.com/nzy1997/rstim/issues/46`

## File Structure

Create or modify these files:

- Modify `rstim/Cargo.toml`: add workspace path dependency on `qec-code`.
- Modify `rstim/src/codegen/mod.rs`: expose `css`.
- Create `rstim/src/codegen/css.rs`: CSS API types, JSON parsing helpers, validation, schedulers, circuit emission, and BB/surface test helpers kept behind `#[cfg(test)]` only when local to module tests.
- Create `rstim/tests/css_codegen.rs`: library behavior tests for parsing, validation, scheduling, explicit observables, canonical fallback, and DEM compilation.
- Modify `rstim/src/cli.rs`: add CSS-specific `gen` flags and route `--code css --task memory` to `rstim::codegen::css`.
- Modify `rstim/tests/cli_gen.rs`: CLI tests for dense/sparse CSS generation and invalid matrix errors.
- Create `rsinter/src/bench/circuit_source.rs`: benchmark circuit-source parsing and point expansion for legacy surface and CSS inputs.
- Modify `rsinter/src/bench/mod.rs`: expose `circuit_source`.
- Modify `rsinter/src/bench/registry.rs`: replace surface-only point expansion with circuit-source point expansion while preserving legacy behavior.
- Modify `rsinter/src/bench/runners/mod.rs`: consume prebuilt circuit metadata from `BenchCasePoint`.
- Modify `rsinter/tests/bench_registry.rs`: point expansion tests for CSS and legacy surface inputs.
- Modify `rsinter/tests/bench_run.rs`: CSS benchmark run smoke and legacy unchanged behavior.
- Create `rsinter/tests/fixtures/bench/minimal_css_decoder.toml`: CSS benchmark fixture.
- Create `rsinter/tests/fixtures/css/steane_hx.json`, `rsinter/tests/fixtures/css/steane_hz.json`, `rsinter/tests/fixtures/css/steane_logicals_x.json`: small CSS fixture files.
- Create `rsinter/tests/css_surface_special.rs`: rotated-surface behavioral smoke and BB `[[72,12,6]]` smoke at the benchmark/decoder layer.

## Task 1: Add CSS Codegen API Shell

**Files:**
- Modify: `rstim/Cargo.toml`
- Modify: `rstim/src/codegen/mod.rs`
- Create: `rstim/src/codegen/css.rs`
- Test: `rstim/tests/css_codegen.rs`

- [ ] **Step 1: Write the failing API import test**

Add `rstim/tests/css_codegen.rs`:

```rust
use rstim::codegen::css::{
    css_memory, CssCheckMatrices, CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::NoiseParams;

#[test]
fn css_memory_rejects_zero_rounds() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0]],
            hz: vec![],
            num_data_qubits: 1,
        },
        rounds: 0,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();
    assert!(err.contains("rounds must be >= 1"), "error was: {err}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p rstim --test css_codegen css_memory_rejects_zero_rounds
```

Expected: FAIL because `rstim::codegen::css` does not exist.

- [ ] **Step 3: Add dependency and API shell**

In `rstim/Cargo.toml`, add `qec-code` to `[dependencies]`:

```toml
qec-code = { path = "../qec-code" }
```

In `rstim/src/codegen/mod.rs`, add:

```rust
pub mod css;
```

Create `rstim/src/codegen/css.rs`:

```rust
use std::collections::BTreeSet;
use std::fmt;

use qec_code::css::CssCode;

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
            Self::MissingObservables => write!(f, "canonical logical fallback produced no observables"),
            Self::InvalidObservable { row, col, width } => write!(
                f,
                "observable {row} references data qubit {col}, but width is {width}"
            ),
            Self::MixedCanonicalLogical { index, basis } => {
                write!(f, "canonical logical {index} is not pure in memory-{basis:?} basis")
            }
        }
    }
}

impl std::error::Error for CssCodegenError {}

pub fn css_memory(config: CssMemoryConfig) -> Result<Vec<StimInstr>, CssCodegenError> {
    if config.rounds == 0 {
        return Err(CssCodegenError::InvalidRounds);
    }
    validate_supports("hx", &config.checks.hx, config.checks.num_data_qubits)?;
    validate_supports("hz", &config.checks.hz, config.checks.num_data_qubits)?;
    let hx_dense = supports_to_dense(&config.checks.hx, config.checks.num_data_qubits);
    let hz_dense = supports_to_dense(&config.checks.hz, config.checks.num_data_qubits);
    CssCode::from_hx_hz(hx_dense, hz_dense)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    Ok(Vec::new())
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

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cargo test -p rstim --test css_codegen css_memory_rejects_zero_rounds
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rstim/Cargo.toml rstim/src/codegen/mod.rs rstim/src/codegen/css.rs rstim/tests/css_codegen.rs
git commit -m "feat: add css codegen api shell"
```

## Task 2: Matrix JSON Normalization

**Files:**
- Modify: `rstim/src/codegen/css.rs`
- Modify: `rstim/tests/css_codegen.rs`

- [ ] **Step 1: Write failing dense/sparse parser tests**

Append to `rstim/tests/css_codegen.rs`:

```rust
use rstim::codegen::css::{parse_css_matrix_json, parse_css_observable_json};

#[test]
fn dense_and_sparse_json_normalize_to_same_supports() {
    let dense = r#"{"format":"dense","rows":[[1,0,1],[0,1,1]]}"#;
    let sparse = r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,2],[1,2]]}"#;

    let dense_matrix = parse_css_matrix_json(dense).unwrap();
    let sparse_matrix = parse_css_matrix_json(sparse).unwrap();

    assert_eq!(dense_matrix.num_cols, 3);
    assert_eq!(dense_matrix.rows, vec![vec![0, 2], vec![1, 2]]);
    assert_eq!(dense_matrix, sparse_matrix);
}

#[test]
fn parser_rejects_bad_dense_and_sparse_inputs() {
    let bad_dense = r#"{"format":"dense","rows":[[1,2]]}"#;
    let err = parse_css_matrix_json(bad_dense).unwrap_err().to_string();
    assert!(err.contains("non-binary entry 2"), "error was: {err}");

    let repeated_sparse = r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,0]]}"#;
    let err = parse_css_matrix_json(repeated_sparse)
        .unwrap_err()
        .to_string();
    assert!(err.contains("repeats column 0"), "error was: {err}");

    let out_of_range = r#"{"format":"sparse_rows","num_cols":3,"rows":[[3]]}"#;
    let err = parse_css_matrix_json(out_of_range)
        .unwrap_err()
        .to_string();
    assert!(err.contains("out-of-range column 3"), "error was: {err}");
}

#[test]
fn observable_json_uses_sparse_support_rows() {
    let logicals = r#"{"format":"sparse_rows","num_cols":4,"rows":[[0,2],[1,3]]}"#;
    let parsed = parse_css_observable_json(logicals).unwrap();

    assert_eq!(parsed.num_cols, 4);
    assert_eq!(parsed.rows, vec![vec![0, 2], vec![1, 3]]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rstim --test css_codegen dense_and_sparse_json_normalize_to_same_supports parser_rejects_bad_dense_and_sparse_inputs observable_json_uses_sparse_support_rows
```

Expected: FAIL because parser functions do not exist.

- [ ] **Step 3: Implement JSON parser and normalized matrix type**

Add to `rstim/src/codegen/css.rs`:

```rust
use serde::Deserialize;

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
                write!(f, "dense row {row} has non-binary entry {value} at column {col}")
            }
            Self::DuplicateIndex { row, col } => write!(f, "sparse row {row} repeats column {col}"),
            Self::OutOfRangeIndex { row, col, width } => {
                write!(f, "sparse row {row} contains out-of-range column {col} for width {width}")
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
```

- [ ] **Step 4: Run parser tests**

Run:

```bash
cargo test -p rstim --test css_codegen dense_and_sparse_json_normalize_to_same_supports parser_rejects_bad_dense_and_sparse_inputs observable_json_uses_sparse_support_rows
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rstim/src/codegen/css.rs rstim/tests/css_codegen.rs
git commit -m "feat: parse css matrix json wrappers"
```

## Task 3: Sequential CSS Memory Circuit With Explicit Observables

**Files:**
- Modify: `rstim/src/codegen/css.rs`
- Modify: `rstim/tests/css_codegen.rs`

- [ ] **Step 1: Write failing sequential circuit tests**

Append to `rstim/tests/css_codegen.rs`:

```rust
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::circuit_to_string;
use rstim::stats;

fn repetition_like_css_config(rounds: usize, basis: MemoryBasis) -> CssMemoryConfig {
    CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0, 1]],
            hz: vec![],
            num_data_qubits: 2,
        },
        rounds,
        noise: NoiseParams::none(),
        basis,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0, 1]]),
    }
}

#[test]
fn sequential_css_memory_x_emits_detectors_observable_and_dem() {
    let circuit = css_memory(repetition_like_css_config(2, MemoryBasis::X)).unwrap();
    let text = circuit_to_string(&circuit);

    assert!(text.contains("QUBIT_COORDS(0) 0"));
    assert!(text.contains("RX 0"));
    assert!(text.contains("H 2"));
    assert!(text.contains("CX 2 0"));
    assert!(text.contains("MX 0"));
    assert_eq!(stats::num_detectors(&circuit), 3);
    assert_eq!(stats::num_observables(&circuit), 1);

    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
}

#[test]
fn css_memory_rejects_non_orthogonal_checks() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0]],
            hz: vec![vec![0]],
            num_data_qubits: 1,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();
    assert!(
        err.contains("CSS X/Z checks are not orthogonal"),
        "error was: {err}"
    );
}

#[test]
fn css_memory_rejects_out_of_range_observable_support() {
    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.observables = CssObservableSource::Explicit(vec![vec![2]]);

    let err = css_memory(config).unwrap_err().to_string();
    assert!(
        err.contains("observable 0 references data qubit 2"),
        "error was: {err}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rstim --test css_codegen sequential_css_memory_x_emits_detectors_observable_and_dem css_memory_rejects_non_orthogonal_checks css_memory_rejects_out_of_range_observable_support
```

Expected: FAIL because `css_memory` returns an empty circuit and does not validate observables.

- [ ] **Step 3: Implement sequential circuit emission**

In `rstim/src/codegen/css.rs`, replace `css_memory` with:

```rust
pub fn css_memory(config: CssMemoryConfig) -> Result<Vec<StimInstr>, CssCodegenError> {
    if config.rounds == 0 {
        return Err(CssCodegenError::InvalidRounds);
    }
    validate_supports("hx", &config.checks.hx, config.checks.num_data_qubits)?;
    validate_supports("hz", &config.checks.hz, config.checks.num_data_qubits)?;
    let hx_dense = supports_to_dense(&config.checks.hx, config.checks.num_data_qubits);
    let hz_dense = supports_to_dense(&config.checks.hz, config.checks.num_data_qubits);
    CssCode::from_hx_hz(hx_dense, hz_dense)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    let observables = explicit_observables(&config)?;
    emit_css_memory_circuit(&config, &observables)
}
```

Add these helpers to the same file:

```rust
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

fn explicit_observables(config: &CssMemoryConfig) -> Result<Vec<Vec<usize>>, CssCodegenError> {
    match &config.observables {
        CssObservableSource::Explicit(rows) | CssObservableSource::ExplicitOrCanonical(rows) => {
            validate_observables(rows, config.checks.num_data_qubits)?;
            Ok(rows.clone())
        }
        CssObservableSource::CanonicalFallback => Err(CssCodegenError::MissingObservables),
    }
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
        instrs.push(op("QUBIT_COORDS", &[q as f64], &[StimTarget::Qubit(q as u32)]));
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
    for check in checks.iter().filter(|check| check.kind == CheckKind::X) {
        instrs.push(op("H", &[], &[StimTarget::Qubit(check.ancilla)]));
    }
    for layer in schedule_layers(config.schedule, checks) {
        instrs.push(op("TICK", &[], &[]));
        let targets: Vec<_> = layer
            .iter()
            .flat_map(|cnot| [StimTarget::Qubit(cnot.control), StimTarget::Qubit(cnot.target)])
            .collect();
        if !targets.is_empty() {
            instrs.push(op("CX", &[], &targets));
        }
    }
    instrs.push(op("TICK", &[], &[]));
    for check in checks.iter().filter(|check| check.kind == CheckKind::X) {
        instrs.push(op("H", &[], &[StimTarget::Qubit(check.ancilla)]));
    }
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
        CssSchedule::Greedy => interactions.into_iter().map(|cnot| vec![cnot]).collect(),
    }
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
        instrs.push(op(
            "DETECTOR",
            &[order as f64, 0.0],
            &targets,
        ));
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
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p rstim --test css_codegen sequential_css_memory_x_emits_detectors_observable_and_dem css_memory_rejects_non_orthogonal_checks css_memory_rejects_out_of_range_observable_support
```

Expected: PASS.

- [ ] **Step 5: Run existing generator tests for regression coverage**

Run:

```bash
cargo test -p rstim --test gen_surface_code --test gen_rep_code --test gen_color_code
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rstim/src/codegen/css.rs rstim/tests/css_codegen.rs
git commit -m "feat: generate sequential css memory circuits"
```

## Task 4: Greedy CNOT Scheduler And Noise Placement

**Files:**
- Modify: `rstim/src/codegen/css.rs`
- Modify: `rstim/tests/css_codegen.rs`

- [ ] **Step 1: Write failing greedy scheduler tests**

Append to `rstim/tests/css_codegen.rs`:

```rust
#[test]
fn greedy_schedule_packs_disjoint_cnots() {
    let sequential = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0], vec![1]],
            hz: vec![],
            num_data_qubits: 2,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0, 1]]),
    };
    let mut greedy = sequential.clone();
    greedy.schedule = CssSchedule::Greedy;

    let sequential_text = circuit_to_string(&css_memory(sequential).unwrap());
    let greedy_text = circuit_to_string(&css_memory(greedy).unwrap());

    assert!(sequential_text.contains("CX 2 0\nTICK\nCX 3 1"));
    assert!(greedy_text.contains("CX 2 0 3 1"));
}

#[test]
fn css_memory_places_requested_noise_channels() {
    let mut config = repetition_like_css_config(1, MemoryBasis::X);
    config.noise = NoiseParams::uniform(0.125);

    let text = circuit_to_string(&css_memory(config).unwrap());

    assert!(text.contains("DEPOLARIZE1(0.125) 0"));
    assert!(text.contains("DEPOLARIZE2(0.125) 2 0"));
    assert!(text.contains("X_ERROR(0.125) 2"));
}
```

- [ ] **Step 2: Run tests to verify greedy packing and noise fail**

Run:

```bash
cargo test -p rstim --test css_codegen greedy_schedule_packs_disjoint_cnots css_memory_places_requested_noise_channels
```

Expected: FAIL because greedy still behaves sequentially and CNOT noise is not emitted.

- [ ] **Step 3: Implement greedy layer packing and CNOT depolarization**

Replace `schedule_layers` in `rstim/src/codegen/css.rs` with:

```rust
fn schedule_layers(schedule: CssSchedule, checks: &[Check]) -> Vec<Vec<CnotInteraction>> {
    let interactions = cnot_interactions(checks);
    match schedule {
        CssSchedule::Sequential => interactions.into_iter().map(|cnot| vec![cnot]).collect(),
        CssSchedule::Greedy => {
            let mut layers: Vec<Vec<CnotInteraction>> = Vec::new();
            for cnot in interactions {
                if let Some(layer) = layers.iter_mut().find(|layer| cnot_fits_layer(&cnot, layer)) {
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
```

In `emit_round`, after the `CX` op for each layer, add:

```rust
        if config.noise.after_clifford_depolarization > 0.0 && !targets.is_empty() {
            instrs.push(op(
                "DEPOLARIZE2",
                &[config.noise.after_clifford_depolarization],
                &targets,
            ));
        }
```

After each X-ancilla `H` layer, add `DEPOLARIZE1` for those ancillas:

```rust
    if config.noise.after_clifford_depolarization > 0.0 {
        for check in checks.iter().filter(|check| check.kind == CheckKind::X) {
            instrs.push(op(
                "DEPOLARIZE1",
                &[config.noise.after_clifford_depolarization],
                &[StimTarget::Qubit(check.ancilla)],
            ));
        }
    }
```

Use that block after the first H loop and again after the second H loop.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p rstim --test css_codegen greedy_schedule_packs_disjoint_cnots css_memory_places_requested_noise_channels
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rstim/src/codegen/css.rs rstim/tests/css_codegen.rs
git commit -m "feat: add greedy css cnot scheduling"
```

## Task 5: Canonical Logical Fallback

**Files:**
- Modify: `rstim/src/codegen/css.rs`
- Modify: `rstim/tests/css_codegen.rs`

- [ ] **Step 1: Write failing canonical fallback tests**

Append to `rstim/tests/css_codegen.rs`:

```rust
fn steane_h() -> Vec<Vec<usize>> {
    vec![vec![0, 3, 5, 6], vec![1, 3, 4, 6], vec![2, 4, 5, 6]]
}

#[test]
fn canonical_fallback_adds_steane_observable() {
    let h = steane_h();
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: h.clone(),
            hz: h,
            num_data_qubits: 7,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::CanonicalFallback,
    };

    let circuit = css_memory(config).unwrap();

    assert_eq!(stats::num_observables(&circuit), 1);
    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
}

#[test]
fn explicit_or_canonical_prefers_explicit_observables() {
    let h = steane_h();
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: h.clone(),
            hz: h,
            num_data_qubits: 7,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::ExplicitOrCanonical(vec![vec![0, 1, 2]]),
    };

    let text = circuit_to_string(&css_memory(config).unwrap());

    assert!(text.contains("OBSERVABLE_INCLUDE(0) rec[-7] rec[-6] rec[-5]"));
}
```

- [ ] **Step 2: Run tests to verify canonical fallback fails**

Run:

```bash
cargo test -p rstim --test css_codegen canonical_fallback_adds_steane_observable explicit_or_canonical_prefers_explicit_observables
```

Expected: FAIL because `CanonicalFallback` currently returns `MissingObservables`.

- [ ] **Step 3: Implement canonical fallback selection**

In `css_memory`, keep the `CssCode` value:

```rust
    let css_code = CssCode::from_hx_hz(hx_dense, hz_dense)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    let observables = resolve_observables(&config, &css_code)?;
```

Replace `explicit_observables` with:

```rust
fn resolve_observables(
    config: &CssMemoryConfig,
    css_code: &CssCode,
) -> Result<Vec<Vec<usize>>, CssCodegenError> {
    match &config.observables {
        CssObservableSource::Explicit(rows) | CssObservableSource::ExplicitOrCanonical(rows)
            if !rows.is_empty() =>
        {
            validate_observables(rows, config.checks.num_data_qubits)?;
            Ok(rows.clone())
        }
        CssObservableSource::ExplicitOrCanonical(_) | CssObservableSource::CanonicalFallback => {
            canonical_observables(config, css_code)
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
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p rstim --test css_codegen canonical_fallback_adds_steane_observable explicit_or_canonical_prefers_explicit_observables
```

Expected: PASS.

- [ ] **Step 5: Run all CSS library tests**

Run:

```bash
cargo test -p rstim --test css_codegen
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rstim/src/codegen/css.rs rstim/tests/css_codegen.rs
git commit -m "feat: derive css logical observables"
```

## Task 6: CSS CLI Generation

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_gen.rs`

- [ ] **Step 1: Write failing CLI tests**

Append to `rstim/tests/cli_gen.rs`:

```rust
#[test]
fn gen_css_memory_from_sparse_json_files() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    std::fs::write(&hx, r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#).unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#).unwrap();
    std::fs::write(&obs, r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#).unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "2",
            "--schedule",
            "greedy",
            "--observables",
            obs.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DETECTOR"));
    assert!(stdout.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_css_memory_reports_non_orthogonal_checks() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    std::fs::write(&hx, r#"{"format":"dense","rows":[[1]]}"#).unwrap();
    std::fs::write(&hz, r#"{"format":"dense","rows":[[1]]}"#).unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CSS X/Z checks are not orthogonal"),
        "stderr: {stderr}"
    );
}
```

- [ ] **Step 2: Run CLI tests to verify they fail**

Run:

```bash
cargo test -p rstim --test cli_gen gen_css_memory_from_sparse_json_files gen_css_memory_reports_non_orthogonal_checks
```

Expected: FAIL because `Gen` has no `--hx`, `--hz`, `--basis`, `--schedule`, or `--observables` flags.

- [ ] **Step 3: Extend `Gen` arguments and dispatch**

In `rstim/src/cli.rs`, import CSS types near the existing imports:

```rust
use crate::codegen::css::{
    css_memory, parse_css_matrix_json, parse_css_observable_json, CssCheckMatrices,
    CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
```

Change the `Commands::Gen` fields:

```rust
    Gen {
        #[arg(long)]
        code: String,
        #[arg(long)]
        task: String,
        #[arg(long)]
        distance: Option<usize>,
        #[arg(long)]
        rounds: usize,
        #[arg(long = "after_clifford_depolarization", default_value = "0")]
        noise: f64,
        #[arg(long)]
        hx: Option<String>,
        #[arg(long)]
        hz: Option<String>,
        #[arg(long)]
        basis: Option<String>,
        #[arg(long, default_value = "greedy")]
        schedule: String,
        #[arg(long)]
        observables: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
```

Update the `Commands::Gen` match arm:

```rust
        Some(Commands::Gen {
            code,
            task,
            distance,
            rounds,
            noise,
            hx,
            hz,
            basis,
            schedule,
            observables,
            out,
        }) => {
            let mut w = open_output(out.as_deref())?;
            if code == "css" {
                run_css_gen(
                    &task,
                    hx.as_deref(),
                    hz.as_deref(),
                    basis.as_deref(),
                    rounds,
                    noise,
                    &schedule,
                    observables.as_deref(),
                    &mut w,
                )
            } else {
                let distance = distance.ok_or_else(|| "distance is required for common generators".to_string())?;
                run_gen(&code, &task, distance, rounds, noise, &mut w)
            }
        }
```

Add helper functions near `run_gen`:

```rust
pub fn run_css_gen(
    task: &str,
    hx_path: Option<&str>,
    hz_path: Option<&str>,
    basis: Option<&str>,
    rounds: usize,
    noise: f64,
    schedule: &str,
    observables_path: Option<&str>,
    out: &mut dyn Write,
) -> Result<(), String> {
    if task != "memory" {
        return Err(format!("unknown css task: {task}"));
    }
    let hx_text = read_input(hx_path)?;
    let hz_text = read_input(hz_path)?;
    let hx = parse_css_matrix_json(&hx_text).map_err(|error| error.to_string())?;
    let hz = parse_css_matrix_json(&hz_text).map_err(|error| error.to_string())?;
    if hx.num_cols != hz.num_cols {
        return Err(format!(
            "hx and hz widths differ: {} != {}",
            hx.num_cols, hz.num_cols
        ));
    }
    let observables = if let Some(path) = observables_path {
        let text = read_input(Some(path))?;
        let parsed = parse_css_observable_json(&text).map_err(|error| error.to_string())?;
        if parsed.num_cols != hx.num_cols {
            return Err(format!(
                "observable width differs from CSS width: {} != {}",
                parsed.num_cols, hx.num_cols
            ));
        }
        CssObservableSource::Explicit(parsed.rows)
    } else {
        CssObservableSource::CanonicalFallback
    };
    let basis = parse_memory_basis(basis.unwrap_or("x"))?;
    let schedule = parse_css_schedule(schedule)?;
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: hx.rows,
            hz: hz.rows,
            num_data_qubits: hx.num_cols,
        },
        rounds,
        noise: NoiseParams::uniform(noise),
        basis,
        schedule,
        observables,
    })
    .map_err(|error| error.to_string())?;
    out.write_all(crate::ir::circuit_to_string(&circuit).as_bytes())
        .map_err(|error| format!("write error: {error}"))
}

fn parse_memory_basis(value: &str) -> Result<MemoryBasis, String> {
    match value {
        "x" | "X" => Ok(MemoryBasis::X),
        "z" | "Z" => Ok(MemoryBasis::Z),
        other => Err(format!("unknown CSS memory basis: {other}")),
    }
}

fn parse_css_schedule(value: &str) -> Result<CssSchedule, String> {
    match value {
        "sequential" => Ok(CssSchedule::Sequential),
        "greedy" => Ok(CssSchedule::Greedy),
        other => Err(format!("unknown CSS schedule: {other}")),
    }
}
```

Update every existing in-process `Commands::Gen` construction in `rstim/src/cli.rs` so legacy generator calls set `distance: Some(value)` and CSS generator calls set `distance: None`. Keep direct `run_gen` calls unchanged because `run_gen` still receives a concrete `usize` distance.

- [ ] **Step 4: Run CLI tests**

Run:

```bash
cargo test -p rstim --test cli_gen gen_css_memory_from_sparse_json_files gen_css_memory_reports_non_orthogonal_checks
```

Expected: PASS.

- [ ] **Step 5: Run all CLI generator tests**

Run:

```bash
cargo test -p rstim --test cli_gen
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_gen.rs
git commit -m "feat: add css memory generator cli"
```

## Task 7: `rsinter` Circuit Source Abstraction

**Files:**
- Create: `rsinter/src/bench/circuit_source.rs`
- Modify: `rsinter/src/bench/mod.rs`
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/tests/bench_registry.rs`

- [ ] **Step 1: Write failing circuit-source expansion tests**

Append to `rsinter/tests/bench_registry.rs`:

```rust
#[test]
fn expand_runner_points_accepts_css_input_type() {
    let params = BTreeMap::from([
        ("input_type".into(), toml::Value::String("css".into())),
        ("code_id".into(), toml::Value::String("steane".into())),
        ("hx".into(), toml::Value::String("tests/fixtures/css/steane_hx.json".into())),
        ("hz".into(), toml::Value::String("tests/fixtures/css/steane_hz.json".into())),
        ("basis".into(), toml::Value::String("x".into())),
        ("schedule".into(), toml::Value::String("greedy".into())),
        ("observables".into(), toml::Value::String("tests/fixtures/css/steane_logicals_x.json".into())),
        (
            "rounds".into(),
            toml::Value::Array(vec![toml::Value::Integer(1)]),
        ),
        (
            "p".into(),
            toml::Value::Array(vec![toml::Value::Float(0.0)]),
        ),
        ("max_shots".into(), toml::Value::Integer(8)),
        ("max_errors".into(), toml::Value::Integer(4)),
        ("batch_size".into(), toml::Value::Integer(4)),
    ]);

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].rounds, 1);
    assert_eq!(points[0].p, 0.0);
    assert_eq!(points[0].input_type, "css");
    assert_eq!(points[0].basis.as_deref(), Some("x"));
    assert_eq!(points[0].code_id.as_deref(), Some("steane"));
}

#[test]
fn expand_runner_points_defaults_to_legacy_surface_input() {
    let points = expand_runner_points(&valid_runner_params()).unwrap();

    assert_eq!(points[0].input_type, "surface_rotated_memory_x");
    assert_eq!(points[0].distance, Some(3));
    assert_eq!(points[0].basis, None);
}
```

- [ ] **Step 2: Create CSS fixture files used by the tests**

Create `rsinter/tests/fixtures/css/steane_hx.json`:

```json
{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}
```

Create `rsinter/tests/fixtures/css/steane_hz.json`:

```json
{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}
```

Create `rsinter/tests/fixtures/css/steane_logicals_x.json`:

```json
{"format":"sparse_rows","num_cols":7,"rows":[[0,1,2]]}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
cargo test -p rsinter --test bench_registry expand_runner_points_accepts_css_input_type expand_runner_points_defaults_to_legacy_surface_input
```

Expected: FAIL because `BenchCasePoint` does not have `input_type`, `basis`, or `code_id`.

- [ ] **Step 4: Add circuit-source fields to `BenchCasePoint`**

In `rsinter/src/bench/registry.rs`, change `BenchCasePoint`:

```rust
pub struct BenchCasePoint {
    pub input_type: String,
    pub code_id: Option<String>,
    pub distance: Option<usize>,
    pub rounds: usize,
    pub p: f64,
    pub basis: Option<String>,
    pub schedule: Option<String>,
    pub hx_path: Option<String>,
    pub hz_path: Option<String>,
    pub observables_path: Option<String>,
    pub max_shots: u64,
    pub max_errors: u64,
    pub batch_size: usize,
}
```

Update existing tests in `rsinter/src/bench/runners/mod.rs` that construct `BenchCasePoint` manually to include:

```rust
input_type: "surface_rotated_memory_x".into(),
code_id: None,
distance: Some(3),
basis: None,
schedule: None,
hx_path: None,
hz_path: None,
observables_path: None,
```

- [ ] **Step 5: Update point expansion**

In `rsinter/src/bench/registry.rs`, add helpers:

```rust
fn optional_string(params: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_string)
}

fn require_string(params: &BTreeMap<String, Value>, key: &str) -> Result<String, String> {
    require_param(params, key)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{key} must be a string"))
}
```

Replace `expand_runner_points` with:

```rust
pub fn expand_runner_points(
    params: &BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String> {
    let input_type = optional_string(params, "input_type")
        .unwrap_or_else(|| "surface_rotated_memory_x".to_string());
    let rounds = require_array(params, "rounds")?;
    let ps = require_array(params, "p")?;
    let max_shots = require_u64(params, "max_shots")?;
    let max_errors = require_u64(params, "max_errors")?;
    let batch_size = require_usize(params, "batch_size")?;
    if rounds.is_empty() {
        return Err("rounds must not be empty".into());
    }
    if ps.is_empty() {
        return Err("p must not be empty".into());
    }
    if batch_size == 0 {
        return Err("batch_size must be positive".into());
    }

    match input_type.as_str() {
        "surface_rotated_memory_x" => expand_surface_points(
            params, rounds, ps, max_shots, max_errors, batch_size,
        ),
        "css" => expand_css_points(params, rounds, ps, max_shots, max_errors, batch_size),
        other => Err(format!("unknown input_type: {other}")),
    }
}
```

Add:

```rust
fn expand_surface_points(
    params: &BTreeMap<String, Value>,
    rounds: &[Value],
    ps: &[Value],
    max_shots: u64,
    max_errors: u64,
    batch_size: usize,
) -> Result<Vec<BenchCasePoint>, String> {
    let distances = require_array(params, "distance")?;
    if distances.is_empty() {
        return Err("distance must not be empty".into());
    }
    let mut points = Vec::new();
    for distance in distances {
        for round in rounds {
            for p in ps {
                let distance = value_as_usize(distance, "distance entry")?;
                let rounds = value_as_usize(round, "round entry")?;
                if distance < 2 {
                    return Err("distance entry must be >= 2".into());
                }
                if rounds < 1 {
                    return Err("round entry must be >= 1".into());
                }
                points.push(BenchCasePoint {
                    input_type: "surface_rotated_memory_x".into(),
                    code_id: None,
                    distance: Some(distance),
                    rounds,
                    p: value_as_f64(p, "p entry")?,
                    basis: None,
                    schedule: None,
                    hx_path: None,
                    hz_path: None,
                    observables_path: None,
                    max_shots,
                    max_errors,
                    batch_size,
                });
            }
        }
    }
    Ok(points)
}

fn expand_css_points(
    params: &BTreeMap<String, Value>,
    rounds: &[Value],
    ps: &[Value],
    max_shots: u64,
    max_errors: u64,
    batch_size: usize,
) -> Result<Vec<BenchCasePoint>, String> {
    let hx_path = require_string(params, "hx")?;
    let hz_path = require_string(params, "hz")?;
    let basis = require_string(params, "basis")?;
    let schedule = optional_string(params, "schedule").unwrap_or_else(|| "greedy".to_string());
    let observables_path = optional_string(params, "observables");
    let code_id = optional_string(params, "code_id");
    let mut points = Vec::new();
    for round in rounds {
        for p in ps {
            let rounds = value_as_usize(round, "round entry")?;
            if rounds < 1 {
                return Err("round entry must be >= 1".into());
            }
            points.push(BenchCasePoint {
                input_type: "css".into(),
                code_id: code_id.clone(),
                distance: None,
                rounds,
                p: value_as_f64(p, "p entry")?,
                basis: Some(basis.clone()),
                schedule: Some(schedule.clone()),
                hx_path: Some(hx_path.clone()),
                hz_path: Some(hz_path.clone()),
                observables_path: observables_path.clone(),
                max_shots,
                max_errors,
                batch_size,
            });
        }
    }
    Ok(points)
}
```

- [ ] **Step 6: Run registry tests**

Run:

```bash
cargo test -p rsinter --test bench_registry
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add rsinter/src/bench/registry.rs rsinter/src/bench/runners/mod.rs rsinter/tests/bench_registry.rs rsinter/tests/fixtures/css/steane_hx.json rsinter/tests/fixtures/css/steane_hz.json rsinter/tests/fixtures/css/steane_logicals_x.json
git commit -m "feat: expand css benchmark points"
```

## Task 8: Build Circuits From `rsinter` Points

**Files:**
- Create: `rsinter/src/bench/circuit_source.rs`
- Modify: `rsinter/src/bench/mod.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/tests/bench_run.rs`
- Create: `rsinter/tests/fixtures/bench/minimal_css_decoder.toml`

- [ ] **Step 1: Write failing CSS benchmark run test**

Append to `rsinter/tests/bench_run.rs`:

```rust
#[test]
fn rust_benchmark_run_supports_css_input_type() {
    let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/minimal_css_decoder.toml");
    let text = fs::read_to_string(spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(&spec, "rust", dir.path(), &registry).unwrap();
    let data = fs::read(
        artifact_root
            .join("rmatching")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["input_type"], serde_json::json!("css"));
    assert_eq!(rows[0].params["code_id"], serde_json::json!("steane"));
    assert_eq!(rows[0].params["basis"], serde_json::json!("x"));
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(1));
}
```

Create `rsinter/tests/fixtures/bench/minimal_css_decoder.toml`:

```toml
name = "css_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "css"
code_id = "steane"
hx = "tests/fixtures/css/steane_hx.json"
hz = "tests/fixtures/css/steane_hz.json"
basis = "x"
rounds = [1]
p = [0.0]
schedule = "greedy"
observables = "tests/fixtures/css/steane_logicals_x.json"
max_shots = 8
max_errors = 4
batch_size = 4

[plot]
title = "CSS Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
```

- [ ] **Step 2: Run the benchmark test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_css_input_type
```

Expected: FAIL because runners still call `rotated_memory_x` unconditionally.

- [ ] **Step 3: Add circuit source module**

Create `rsinter/src/bench/circuit_source.rs`:

```rust
use std::path::{Path, PathBuf};

use rstim::codegen::css::{
    css_memory, parse_css_matrix_json, parse_css_observable_json, CssCheckMatrices,
    CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::surface_code::rotated_memory_x;
use rstim::codegen::NoiseParams;
use rstim::ir::StimInstr;

use crate::bench::registry::BenchCasePoint;
use crate::bench::result::{CaseSummary, PairMapExt, ParamMap};

pub struct BuiltCircuit {
    pub circuit: Vec<StimInstr>,
    pub params: ParamMap,
    pub case_summary: CaseSummary,
}

pub fn build_circuit_for_point(
    point: &BenchCasePoint,
    spec_dir: &Path,
) -> Result<BuiltCircuit, String> {
    match point.input_type.as_str() {
        "surface_rotated_memory_x" => build_surface(point),
        "css" => build_css(point, spec_dir),
        other => Err(format!("unknown input_type: {other}")),
    }
}

fn build_surface(point: &BenchCasePoint) -> Result<BuiltCircuit, String> {
    let distance = point
        .distance
        .ok_or_else(|| "surface point is missing distance".to_string())?;
    let circuit = rotated_memory_x(distance, point.rounds, point.p);
    Ok(BuiltCircuit {
        circuit,
        params: ParamMap::from_pairs([
            ("input_type", serde_json::json!("surface_rotated_memory_x")),
            ("distance", serde_json::json!(distance)),
            ("rounds", serde_json::json!(point.rounds)),
            ("p", serde_json::json!(point.p)),
            ("max_shots", serde_json::json!(point.max_shots)),
            ("max_errors", serde_json::json!(point.max_errors)),
            ("batch_size", serde_json::json!(point.batch_size)),
        ]),
        case_summary: CaseSummary::new(),
    })
}

fn build_css(point: &BenchCasePoint, spec_dir: &Path) -> Result<BuiltCircuit, String> {
    let hx_path = point
        .hx_path
        .as_deref()
        .ok_or_else(|| "css point is missing hx".to_string())?;
    let hz_path = point
        .hz_path
        .as_deref()
        .ok_or_else(|| "css point is missing hz".to_string())?;
    let hx_text = std::fs::read_to_string(resolve_spec_path(spec_dir, hx_path))
        .map_err(|error| error.to_string())?;
    let hz_text = std::fs::read_to_string(resolve_spec_path(spec_dir, hz_path))
        .map_err(|error| error.to_string())?;
    let hx = parse_css_matrix_json(&hx_text).map_err(|error| error.to_string())?;
    let hz = parse_css_matrix_json(&hz_text).map_err(|error| error.to_string())?;
    if hx.num_cols != hz.num_cols {
        return Err(format!("hx and hz widths differ: {} != {}", hx.num_cols, hz.num_cols));
    }
    let observables = if let Some(path) = point.observables_path.as_deref() {
        let text = std::fs::read_to_string(resolve_spec_path(spec_dir, path))
            .map_err(|error| error.to_string())?;
        let parsed = parse_css_observable_json(&text).map_err(|error| error.to_string())?;
        if parsed.num_cols != hx.num_cols {
            return Err(format!(
                "observable width differs from CSS width: {} != {}",
                parsed.num_cols, hx.num_cols
            ));
        }
        CssObservableSource::Explicit(parsed.rows)
    } else {
        CssObservableSource::CanonicalFallback
    };
    let basis = parse_memory_basis(point.basis.as_deref().unwrap_or("x"))?;
    let schedule = parse_css_schedule(point.schedule.as_deref().unwrap_or("greedy"))?;
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: hx.rows,
            hz: hz.rows,
            num_data_qubits: hx.num_cols,
        },
        rounds: point.rounds,
        noise: NoiseParams::uniform(point.p),
        basis,
        schedule,
        observables,
    })
    .map_err(|error| error.to_string())?;
    Ok(BuiltCircuit {
        circuit,
        params: ParamMap::from_pairs([
            ("input_type", serde_json::json!("css")),
            ("code_id", serde_json::json!(point.code_id.as_deref().unwrap_or("css"))),
            ("basis", serde_json::json!(point.basis.as_deref().unwrap_or("x"))),
            ("schedule", serde_json::json!(point.schedule.as_deref().unwrap_or("greedy"))),
            ("rounds", serde_json::json!(point.rounds)),
            ("p", serde_json::json!(point.p)),
            ("hx", serde_json::json!(hx_path)),
            ("hz", serde_json::json!(hz_path)),
            ("max_shots", serde_json::json!(point.max_shots)),
            ("max_errors", serde_json::json!(point.max_errors)),
            ("batch_size", serde_json::json!(point.batch_size)),
        ]),
        case_summary: CaseSummary::new(),
    })
}

fn resolve_spec_path(spec_dir: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        spec_dir.join(path)
    }
}

fn parse_memory_basis(value: &str) -> Result<MemoryBasis, String> {
    match value {
        "x" | "X" => Ok(MemoryBasis::X),
        "z" | "Z" => Ok(MemoryBasis::Z),
        other => Err(format!("unknown CSS memory basis: {other}")),
    }
}

fn parse_css_schedule(value: &str) -> Result<CssSchedule, String> {
    match value {
        "sequential" => Ok(CssSchedule::Sequential),
        "greedy" => Ok(CssSchedule::Greedy),
        other => Err(format!("unknown CSS schedule: {other}")),
    }
}
```

In `rsinter/src/bench/mod.rs`, add:

```rust
pub mod circuit_source;
```

- [ ] **Step 4: Add spec directory to benchmark context**

In `rsinter/src/bench/registry.rs`, add `spec_dir`:

```rust
pub struct BenchRunContext {
    pub benchmark_name: String,
    pub runner_name: String,
    pub language: String,
    pub seed: u64,
    pub spec_dir: std::path::PathBuf,
}
```

When `BenchRunContext` is constructed in `rsinter/src/bench/run.rs` after Step 6 changes the function signature, include:

```rust
spec_dir: spec_dir.clone(),
```

- [ ] **Step 5: Use circuit source in `run_decoder_point`**

In `rsinter/src/bench/runners/mod.rs`, remove the `rotated_memory_x` import and add:

```rust
use crate::bench::circuit_source::build_circuit_for_point;
```

Inside `run_decoder_point`, replace the circuit creation line:

```rust
let built = build_circuit_for_point(point, &ctx.spec_dir)?;
let circuit = built.circuit;
let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)?;
```

Replace the `params` map in the result with:

```rust
params: built.params,
```

Replace the `case_summary` creation with:

```rust
case_summary: {
    let mut summary = built.case_summary;
    summary.insert("num_dets".into(), serde_json::json!(num_dets));
    summary.insert("num_obs".into(), serde_json::json!(num_obs));
    summary.insert("num_shots_generated".into(), serde_json::json!(generated_shots));
    summary
},
```

- [ ] **Step 6: Fix CLI spec-dir handling**

In `rsinter/src/bench/run.rs`, change function signature:

```rust
pub fn run_rust_benchmark(
    spec: &BenchmarkSpec,
    language: &str,
    out_root: &Path,
    registry: &RustRunnerRegistry,
    spec_dir: &Path,
) -> Result<PathBuf, String>
```

Use `spec_dir.to_path_buf()` in `BenchRunContext`.

Update direct test call sites to pass `Path::new(env!("CARGO_MANIFEST_DIR"))`.

In `rsinter/src/bin/rsinter.rs`, compute:

```rust
let spec_path = PathBuf::from(&spec);
let spec_dir = spec_path
    .parent()
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("."));
```

Pass `&spec_dir` to `run_rust_benchmark`.

- [ ] **Step 7: Run CSS benchmark test**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_css_input_type
```

Expected: PASS.

- [ ] **Step 8: Run all benchmark runner tests**

Run:

```bash
cargo test -p rsinter --test bench_run --test bench_registry --test bench_cli
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add rsinter/src/bench/circuit_source.rs rsinter/src/bench/mod.rs rsinter/src/bench/registry.rs rsinter/src/bench/run.rs rsinter/src/bench/runners/mod.rs rsinter/src/bin/rsinter.rs rsinter/tests/bench_run.rs rsinter/tests/bench_registry.rs rsinter/tests/bench_cli.rs rsinter/tests/fixtures/bench/minimal_css_decoder.toml
git commit -m "feat: run css inputs in rsinter benchmarks"
```

## Task 9: Surface Special Case And BB Smoke Verification

**Files:**
- Create: `rsinter/tests/css_surface_special.rs`

- [ ] **Step 1: Write surface and BB smoke tests**

Create `rsinter/tests/css_surface_special.rs`:

```rust
use rsinter::decode::{Decoder, RmatchingDemDecoder};
use rstim::codegen::css::{
    css_memory, CssCheckMatrices, CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::surface_code::rotated_memory_x;
use rstim::codegen::NoiseParams;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::output::write_shots_b8;
use rstim::sampler::sample_batch;
use rstim::stats;
use rand::rngs::StdRng;
use rand::SeedableRng;

#[test]
fn css_surface_style_counts_match_rotated_surface_memory_x() {
    for distance in [3, 5] {
        let css = rotated_surface_css_memory_x(distance, distance, 0.001);
        let rotated = rotated_memory_x(distance, distance, 0.001);

        assert_eq!(stats::num_observables(&css), stats::num_observables(&rotated));
        assert_eq!(stats::num_detectors(&css), stats::num_detectors(&rotated));
        ErrorAnalyzer::circuit_to_dem_decomposed(&css).unwrap();
    }
}

#[test]
fn css_surface_style_rmatching_smoke_tracks_rotated_baseline() {
    let css = rotated_surface_css_memory_x(3, 3, 0.002);
    let rotated = rotated_memory_x(3, 3, 0.002);

    let css_rate = logical_error_rate(&css, 256, 12_345);
    let rotated_rate = logical_error_rate(&rotated, 256, 12_345);

    assert!(
        (css_rate - rotated_rate).abs() <= 0.15,
        "css_rate={css_rate}, rotated_rate={rotated_rate}"
    );
}

#[test]
fn bb72_css_smoke_builds_dem_with_twelve_observables() {
    let (hx, hz) = bb72_checks();
    let observables = (0..12).map(|index| vec![index]).collect();
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx,
            hz,
            num_data_qubits: 72,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(observables),
    })
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();

    assert_eq!(stats::num_observables(&circuit), 12);
    assert_eq!(dem.num_observables(), 12);
    assert_eq!(stats::num_detectors(&circuit), 72);
}

fn logical_error_rate(circuit: &[rstim::ir::StimInstr], shots: usize, seed: u64) -> f64 {
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(circuit).unwrap();
    let decoder = RmatchingDemDecoder;
    let compiled = decoder.compile_for_dem(&dem);
    let num_dets = dem.effective_num_detectors();
    let num_obs = dem.num_observables();
    let obs_bytes = num_obs.div_ceil(8);
    let mut rng = StdRng::seed_from_u64(seed);
    let batch = sample_batch(circuit, shots, &mut rng).unwrap();
    let mut dets = Vec::new();
    write_shots_b8(&batch.detections, &mut dets).unwrap();
    let mut obs = Vec::new();
    write_shots_b8(&batch.observable_flips, &mut obs).unwrap();
    let predictions = compiled.decode_shots_bit_packed(&dets, shots, num_dets, num_obs);
    let mut errors = 0usize;
    for shot in 0..shots {
        let start = shot * obs_bytes;
        let end = start + obs_bytes;
        if predictions[start..end] != obs[start..end] {
            errors += 1;
        }
    }
    errors as f64 / shots as f64
}

fn rotated_surface_css_memory_x(distance: usize, rounds: usize, noise: f64) -> Vec<rstim::ir::StimInstr> {
    let (hx, hz, logical_x) = rotated_surface_css_checks(distance);
    css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx,
            hz,
            num_data_qubits: distance * distance,
        },
        rounds,
        noise: NoiseParams::uniform(noise),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(vec![logical_x]),
    })
    .unwrap()
}

fn rotated_surface_css_checks(distance: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>, Vec<usize>) {
    let data_index = |x: usize, y: usize| -> usize { y * distance + x };
    let mut hx = Vec::new();
    let mut hz = Vec::new();
    for ax in 0..=distance {
        for ay in 0..=distance {
            let on_boundary_1 = ax == 0 || ax == distance;
            let on_boundary_2 = ay == 0 || ay == distance;
            let parity = (ax % 2) != (ay % 2);
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }
            let mut support = Vec::new();
            for (dx, dy) in [(1isize, 1isize), (1, -1), (-1, 1), (-1, -1)] {
                let x = ax as isize + dx;
                let y = ay as isize + dy;
                if x >= 1
                    && x <= (2 * distance - 1) as isize
                    && y >= 1
                    && y <= (2 * distance - 1) as isize
                    && x % 2 == 1
                    && y % 2 == 1
                {
                    let qx = ((x - 1) / 2) as usize;
                    let qy = ((y - 1) / 2) as usize;
                    if qx < distance && qy < distance {
                        support.push(data_index(qx, qy));
                    }
                }
            }
            support.sort_unstable();
            if parity {
                hx.push(support);
            } else {
                hz.push(support);
            }
        }
    }
    let logical_x = (0..distance).map(|y| data_index(0, y)).collect();
    (hx, hz, logical_x)
}

fn bb72_checks() -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    bivariate_bicycle_checks(
        6,
        6,
        &[(3, 0), (0, 1), (0, 2)],
        &[(0, 3), (1, 0), (2, 0)],
    )
}

fn bivariate_bicycle_checks(
    lx: usize,
    ly: usize,
    a_terms: &[(usize, usize)],
    b_terms: &[(usize, usize)],
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block = lx * ly;
    let index = |x: usize, y: usize| -> usize { (x % lx) * ly + (y % ly) };
    let mut hx = Vec::with_capacity(block);
    let mut hz = Vec::with_capacity(block);
    for x in 0..lx {
        for y in 0..ly {
            let mut x_row = Vec::new();
            for &(dx, dy) in a_terms {
                x_row.push(index(x + dx, y + dy));
            }
            for &(dx, dy) in b_terms {
                x_row.push(block + index(x + dx, y + dy));
            }
            x_row.sort_unstable();
            hx.push(x_row);

            let mut z_row = Vec::new();
            for &(dx, dy) in b_terms {
                z_row.push(index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            for &(dx, dy) in a_terms {
                z_row.push(block + index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            z_row.sort_unstable();
            hz.push(z_row);
        }
    }
    (hx, hz)
}
```

- [ ] **Step 2: Run verification tests to observe failures**

Run:

```bash
cargo test -p rsinter --test css_surface_special
```

Expected: one or more failures while the surface helper is aligned with the generic CSS detector semantics.

- [ ] **Step 3: Adjust only the CSS generator or test helper when the failure identifies a real mismatch**

Use the failure details:

- If `ErrorAnalyzer` fails on the CSS circuit, inspect the printed circuit with `rstim::ir::circuit_to_string` and fix detector rec offsets in `rstim/src/codegen/css.rs`.
- If `num_detectors` differs from `rotated_memory_x`, compare first-round and tail detector counts. For memory-X, the intended count is:

```text
first round X checks + (rounds - 1) * all checks + tail X checks
```

- If the BB smoke fails orthogonality, fix the `bivariate_bicycle_checks` transpose shifts in the test helper. Keep `H_X = [A, B]` and `H_Z = [B^T, A^T]`.

- [ ] **Step 4: Run verification tests**

Run:

```bash
cargo test -p rsinter --test css_surface_special
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rsinter/tests/css_surface_special.rs rstim/src/codegen/css.rs
git commit -m "test: verify css surface and bb smoke cases"
```

## Task 10: Workspace Verification And Documentation Touch-Up

**Files:**
- Modify: `README.md` only if the CLI section needs a CSS example after implementation
- Modify: `rstim/doc/cli.md` if this file already documents `rstim gen`

- [ ] **Step 1: Run all affected crate tests**

Run:

```bash
cargo test -p qec-code
cargo test -p rstim --test css_codegen --test cli_gen
cargo test -p rsinter --test bench_registry --test bench_run --test bench_cli --test css_surface_special
```

Expected: PASS for all commands.

- [ ] **Step 2: Run full workspace check**

Run:

```bash
cargo test --workspace
```

Expected: PASS. If unrelated long-running benchmark tests are already known to be excluded locally, record the exact failure and run the affected package tests above again before finishing.

- [ ] **Step 3: Add CLI docs if `rstim/doc/cli.md` documents `gen`**

If `rstim/doc/cli.md` has a `gen` section, add this exact example under it:

````markdown
CSS memory circuits can be generated from explicit matrix wrappers:

```sh
rstim gen \
  --code css \
  --task memory \
  --hx hx.json \
  --hz hz.json \
  --basis x \
  --rounds 3 \
  --schedule greedy \
  --observables logicals_x.json
```

`hx.json`, `hz.json`, and observable files use the explicit JSON wrappers accepted by `rstim::codegen::css`.
````

- [ ] **Step 4: Run doc-adjacent tests**

Run:

```bash
cargo test -p rstim --test cli_gen --test perf_docs
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add README.md rstim/doc/cli.md
git diff --cached --quiet || git commit -m "docs: document css memory generation"
```

If no docs changed, run:

```bash
git status --short
```

Expected: no unstaged documentation changes.

## Self-Review

Spec coverage:

- `rstim::codegen::css::css_memory`: Tasks 1, 3, 4, 5.
- Dense and sparse JSON wrappers: Task 2.
- Explicit and canonical observables: Tasks 3 and 5.
- Sequential and greedy schedules: Tasks 3 and 4.
- CLI path: Task 6.
- `rsinter` CSS input path: Tasks 7 and 8.
- Surface special case and BB smoke: Task 9.
- Legacy surface behavior: Tasks 7, 8, and 10.

Completeness scan:

- The plan avoids open-ended implementation text and undefined commit checkpoints.
- The only conditional work is documentation touch-up in Task 10; it has exact commands and expected outcomes for both changed and unchanged docs.

Type consistency:

- `CssMemoryConfig`, `CssCheckMatrices`, `CssObservableSource`, `CssSchedule`, and `MemoryBasis` are introduced in Task 1 and used consistently afterward.
- `BenchCasePoint` fields introduced in Task 7 are consumed in Task 8.
- CLI parser helper names in Task 6 are local to `rstim/src/cli.rs` and separate from the `rsinter` parser helpers in Task 8.
