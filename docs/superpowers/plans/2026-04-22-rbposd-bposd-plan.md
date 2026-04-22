# rbposd BPOSD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new `rbposd` crate to this workspace that provides a Rust MVP of BPOSD (`minimum-sum + parallel + OSD_0`), then layer on CSS helpers and a `rsinter` adapter.

**Architecture:** Keep the algorithm core isolated in a new `rbposd` crate centered on `ParityCheckMatrix + Syndrome -> Correction`. Build the work in order: crate scaffold, matrix/vector/algebra foundations, BP, OSD fallback, examples and regression fixtures, CSS helpers, then an adapter that compiles `rstim` DEMs into `rbposd` matrix problems for `rsinter`.

**Tech Stack:** Rust workspace crates (`rbposd`, `rstim`, `rsinter`), standard library only for the new core crate, existing `rstim` DEM types and `rsinter` decoder traits, `cargo test`, `cargo run`

---

## File Structure

- Modify: `Cargo.toml`
  - Add `rbposd` as a workspace member.
- Create: `rbposd/Cargo.toml`
  - Define the new crate and keep dependencies minimal.
- Create: `rbposd/doc/ldpc_mvp_reference.md`
  - Lock the exact MVP behavior and fixture set that the Rust implementation is expected to match.
- Create: `rbposd/src/lib.rs`
  - Re-export the public API in the order the crate grows.
- Create: `rbposd/src/config.rs`
  - Define `BpVariant`, `Schedule`, `OsdVariant`, `ChannelModel`, and `DecoderConfig`.
- Create: `rbposd/src/error.rs`
  - Define matrix construction and decode errors.
- Create: `rbposd/src/vector.rs`
  - Define `Syndrome` and `Correction` wrappers plus small GF(2) helpers.
- Create: `rbposd/src/matrix.rs`
  - Define `ParityCheckMatrix`, row/column adjacency, sparse row and sparse column constructors, and parity multiplication.
- Create: `rbposd/src/gf2.rs`
  - Hold internal dense-elimination and permutation helpers used by OSD.
- Create: `rbposd/src/bp.rs`
  - Hold the minimum-sum BP inner loop and reliability extraction.
- Create: `rbposd/src/osd.rs`
  - Hold column ordering and `OSD_0` solve logic.
- Create: `rbposd/src/decoder.rs`
  - Own `BpOsdDecoder`, compiled graph state, and `DecodeResult`.
- Create: `rbposd/src/css.rs`
  - Thin convenience layer that owns paired `BpOsdDecoder` values for `Hx`/`Hz`.
- Create: `rbposd/examples/basic_decode.rs`
  - Small copy-pastable example for `new + decode`.
- Create: `rbposd/examples/profile_repetition.rs`
  - Repeatable decode loop that prints timing and iteration metrics for regression checks.
- Create: `rbposd/tests/smoke.rs`
  - Lock the public config defaults and crate scaffold.
- Create: `rbposd/tests/matrix.rs`
  - Cover sparse constructors and GF(2) parity multiplication.
- Create: `rbposd/tests/bp.rs`
  - Cover BP-only convergence on small repetition-style matrices.
- Create: `rbposd/tests/osd.rs`
  - Cover `OSD_0` fallback and validity of the returned correction.
- Create: `rbposd/tests/reference.rs`
  - Encode the fixture set described in `rbposd/doc/ldpc_mvp_reference.md`.
- Create: `rbposd/tests/css.rs`
  - Cover the CSS convenience layer.
- Modify: `rsinter/Cargo.toml`
  - Add a path dependency on `rbposd`.
- Modify: `rsinter/src/lib.rs`
  - Register the `rbposd` adapter module.
- Modify: `rsinter/src/decode.rs`
  - Re-export the new adapter alongside the existing traits and `VacuousDecoder`.
- Modify: `rsinter/src/collect.rs`
  - Use `DetectorErrorModel::effective_num_detectors()` so the adapter sees the right packed width for shifted/repeated DEMs.
- Create: `rsinter/src/rbposd_adapter.rs`
  - Translate `DetectorErrorModel` error terms into `rbposd` columns and map decoded corrections back to observables.
- Create: `rsinter/tests/decode_rbposd.rs`
  - Cover direct DEM compilation and a tiny `collect(...)` smoke test using the new adapter.
- Modify: `rstim/doc/getting_started.md`
  - Add a short example that uses the `rsinter` adapter backed by `rbposd`.

## Phase 0

### Task 1: Scaffold The `rbposd` Crate And Lock The MVP Contract

**Files:**
- Modify: `Cargo.toml`
- Create: `rbposd/Cargo.toml`
- Create: `rbposd/doc/ldpc_mvp_reference.md`
- Create: `rbposd/src/lib.rs`
- Create: `rbposd/src/config.rs`
- Create: `rbposd/src/error.rs`
- Test: `rbposd/tests/smoke.rs`

- [ ] **Step 1: Write the failing test**

Create `rbposd/tests/smoke.rs`:

```rust
use rbposd::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};

#[test]
fn decoder_defaults_pin_the_mvp_algorithm_surface() {
    let cfg = DecoderConfig::default();

    assert_eq!(cfg.max_bp_iterations, 30);
    assert!(cfg.early_stop);
    assert_eq!(cfg.bp_variant, BpVariant::MinimumSum);
    assert_eq!(cfg.schedule, Schedule::Parallel);
    assert_eq!(cfg.osd_variant, OsdVariant::Osd0);

    match ChannelModel::Bsc { error_rate: 0.05 } {
        ChannelModel::Bsc { error_rate } => assert_eq!(error_rate, 0.05),
        ChannelModel::BitFlipProbabilities(_) => unreachable!(),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd --test smoke -v`

Expected: FAIL with `package ID specification 'rbposd' did not match any packages`.

- [ ] **Step 3: Write minimal implementation**

Update the workspace root `Cargo.toml`:

```toml
[workspace]
members = ["rstim", "rsinter", "rbposd"]
resolver = "3"
```

Create `rbposd/Cargo.toml`:

```toml
[package]
name = "rbposd"
version = "0.1.0"
edition = "2024"

[dependencies]
```

Create `rbposd/doc/ldpc_mvp_reference.md`:

```markdown
# rbposd MVP Reference Contract

Date: 2026-04-22

This file locks the reference surface for the first Rust BPOSD version.

Included:

- binary parity-check matrix input
- syndrome decoding
- `minimum_sum`
- `parallel` schedule
- `OSD_0`
- uniform and per-bit priors

Excluded:

- `product_sum`
- serial scheduling
- `OSD_CS`
- DEM-native input in the core crate
- batch decode APIs

Reference fixtures:

1. Repetition-style 4-check / 5-bit code with a single-flip syndrome that BP should solve without OSD.
2. Small 2-check / 3-bit code that is solved by `OSD_0` when BP is disabled.
3. Small sparse non-identity matrix built from sparse columns to verify constructor symmetry.
```

Create `rbposd/src/config.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpVariant {
    MinimumSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Parallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdVariant {
    Osd0,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelModel {
    Bsc { error_rate: f64 },
    BitFlipProbabilities(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecoderConfig {
    pub max_bp_iterations: usize,
    pub early_stop: bool,
    pub bp_variant: BpVariant,
    pub schedule: Schedule,
    pub osd_variant: OsdVariant,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            max_bp_iterations: 30,
            early_stop: true,
            bp_variant: BpVariant::MinimumSum,
            schedule: Schedule::Parallel,
            osd_variant: OsdVariant::Osd0,
        }
    }
}
```

Create `rbposd/src/error.rs`:

```rust
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    EmptyMatrix,
    InvalidProbability,
    InvalidColumnIndex { column: usize, num_bits: usize },
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
    BpDidNotConverge,
    NoOsdSolution,
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::EmptyMatrix => write!(f, "parity-check matrix must not be empty"),
            DecodeError::InvalidProbability => write!(f, "probability must lie in [0, 1]"),
            DecodeError::InvalidColumnIndex { column, num_bits } => {
                write!(f, "column index {column} is out of bounds for {num_bits} bits")
            }
            DecodeError::DimensionMismatch {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what} length mismatch: expected {expected}, got {actual}")
            }
            DecodeError::BpDidNotConverge => write!(f, "belief propagation did not converge"),
            DecodeError::NoOsdSolution => write!(f, "OSD_0 could not construct a valid solution"),
        }
    }
}

impl Error for DecodeError {}
```

Create `rbposd/src/lib.rs`:

```rust
pub mod config;
pub mod error;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use error::DecodeError;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rbposd --test smoke -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rbposd/Cargo.toml rbposd/doc/ldpc_mvp_reference.md rbposd/src/lib.rs rbposd/src/config.rs rbposd/src/error.rs rbposd/tests/smoke.rs
git commit -m "feat: add rbposd crate scaffold"
```

## Phase 1

### Task 2: Implement Matrix And Vector Foundations

**Files:**
- Modify: `rbposd/src/lib.rs`
- Modify: `rbposd/src/error.rs`
- Create: `rbposd/src/vector.rs`
- Create: `rbposd/src/matrix.rs`
- Test: `rbposd/tests/matrix.rs`

- [ ] **Step 1: Write the failing test**

Create `rbposd/tests/matrix.rs`:

```rust
use rbposd::{Correction, ParityCheckMatrix, Syndrome};

#[test]
fn sparse_rows_reject_an_out_of_bounds_column() {
    let err = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![3]])
        .unwrap_err();

    assert!(err.to_string().contains("out of bounds"));
}

#[test]
fn sparse_columns_and_sparse_rows_encode_the_same_code() {
    let from_rows =
        ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let from_cols =
        ParityCheckMatrix::from_sparse_columns(2, 3, vec![vec![0], vec![0, 1], vec![1]])
            .unwrap();

    let correction = Correction::from(vec![true, false, true]);
    let expected = Syndrome::from(vec![true, true]);

    assert_eq!(from_rows.multiply(&correction), expected);
    assert_eq!(from_cols.multiply(&correction), expected);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd --test matrix -v`

Expected: FAIL with unresolved imports for `ParityCheckMatrix`, `Correction`, or `Syndrome`.

- [ ] **Step 3: Write minimal implementation**

Update `rbposd/src/error.rs` so matrix constructors can report a missing row or column domain:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    EmptyMatrix,
    InvalidProbability,
    InvalidColumnIndex { column: usize, num_bits: usize },
    InvalidRowIndex { row: usize, num_checks: usize },
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
    BpDidNotConverge,
    NoOsdSolution,
}
```

Create `rbposd/src/vector.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syndrome(Vec<bool>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction(Vec<bool>);

impl Syndrome {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[bool] {
        &self.0
    }

    pub fn weight(&self) -> usize {
        self.0.iter().filter(|&&bit| bit).count()
    }
}

impl Correction {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[bool] {
        &self.0
    }

    pub fn zero(len: usize) -> Self {
        Self(vec![false; len])
    }
}

impl From<Vec<bool>> for Syndrome {
    fn from(bits: Vec<bool>) -> Self {
        Self(bits)
    }
}

impl From<Vec<bool>> for Correction {
    fn from(bits: Vec<bool>) -> Self {
        Self(bits)
    }
}
```

Create `rbposd/src/matrix.rs`:

```rust
use crate::error::DecodeError;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityCheckMatrix {
    num_checks: usize,
    num_bits: usize,
    rows: Vec<Vec<usize>>,
    columns: Vec<Vec<usize>>,
}

impl ParityCheckMatrix {
    pub fn from_sparse_rows(
        num_checks: usize,
        num_bits: usize,
        rows: Vec<Vec<usize>>,
    ) -> Result<Self, DecodeError> {
        if num_checks == 0 || num_bits == 0 {
            return Err(DecodeError::EmptyMatrix);
        }
        if rows.len() != num_checks {
            return Err(DecodeError::DimensionMismatch {
                what: "row count",
                expected: num_checks,
                actual: rows.len(),
            });
        }
        let mut columns = vec![Vec::new(); num_bits];
        for (row_index, cols) in rows.iter().enumerate() {
            for &column in cols {
                if column >= num_bits {
                    return Err(DecodeError::InvalidColumnIndex { column, num_bits });
                }
                columns[column].push(row_index);
            }
        }
        Ok(Self {
            num_checks,
            num_bits,
            rows,
            columns,
        })
    }

    pub fn from_sparse_columns(
        num_checks: usize,
        num_bits: usize,
        columns: Vec<Vec<usize>>,
    ) -> Result<Self, DecodeError> {
        if num_checks == 0 || num_bits == 0 {
            return Err(DecodeError::EmptyMatrix);
        }
        if columns.len() != num_bits {
            return Err(DecodeError::DimensionMismatch {
                what: "column count",
                expected: num_bits,
                actual: columns.len(),
            });
        }
        let mut rows = vec![Vec::new(); num_checks];
        for (column_index, checks) in columns.iter().enumerate() {
            for &row in checks {
                if row >= num_checks {
                    return Err(DecodeError::InvalidRowIndex { row, num_checks });
                }
                rows[row].push(column_index);
            }
        }
        Ok(Self {
            num_checks,
            num_bits,
            rows,
            columns,
        })
    }

    pub fn num_checks(&self) -> usize {
        self.num_checks
    }

    pub fn num_bits(&self) -> usize {
        self.num_bits
    }

    pub fn row_neighbors(&self, check: usize) -> &[usize] {
        &self.rows[check]
    }

    pub fn column_neighbors(&self, bit: usize) -> &[usize] {
        &self.columns[bit]
    }

    pub fn multiply(&self, correction: &Correction) -> Syndrome {
        let mut syndrome = vec![false; self.num_checks];
        for (row_index, cols) in self.rows.iter().enumerate() {
            let mut parity = false;
            for &column in cols {
                parity ^= correction.as_slice()[column];
            }
            syndrome[row_index] = parity;
        }
        Syndrome::from(syndrome)
    }

    pub(crate) fn dense_rows(&self) -> Vec<Vec<bool>> {
        let mut dense = vec![vec![false; self.num_bits]; self.num_checks];
        for (row_index, cols) in self.rows.iter().enumerate() {
            for &column in cols {
                dense[row_index][column] = true;
            }
        }
        dense
    }
}
```

Update `rbposd/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rbposd --test matrix -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rbposd/src/lib.rs rbposd/src/error.rs rbposd/src/vector.rs rbposd/src/matrix.rs rbposd/tests/matrix.rs
git commit -m "feat: add rbposd matrix and vector core"
```

### Task 3: Add GF(2) Elimination Helpers For OSD

**Files:**
- Modify: `rbposd/src/lib.rs`
- Create: `rbposd/src/gf2.rs`
- Modify: `rbposd/src/error.rs`
- Test: `rbposd/src/gf2.rs`

- [ ] **Step 1: Write the failing test**

Add unit tests to `rbposd/src/gf2.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::Syndrome;

    use super::{solve_with_column_order, sort_columns_by_reliability};

    #[test]
    fn reliability_sort_is_stable_for_equal_scores() {
        let order = sort_columns_by_reliability(&[0.9, 0.9, 0.4, 0.9]);
        assert_eq!(order, vec![0, 1, 3, 2]);
    }

    #[test]
    fn solve_with_column_order_returns_a_valid_solution() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, false]);
        let order = vec![0, 1, 2];

        let correction = solve_with_column_order(&pcm, &syndrome, &order).unwrap();

        assert_eq!(pcm.multiply(&correction), syndrome);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd solve_with_column_order_returns_a_valid_solution -v`

Expected: FAIL with `could not find gf2 in the crate root` or missing functions.

- [ ] **Step 3: Write minimal implementation**

Extend `rbposd/src/error.rs` with one extra variant for singular elimination that cannot satisfy the target syndrome:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    EmptyMatrix,
    InvalidProbability,
    InvalidColumnIndex { column: usize, num_bits: usize },
    InvalidRowIndex { row: usize, num_checks: usize },
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
    SingularSystem,
    BpDidNotConverge,
    NoOsdSolution,
}
```

Create `rbposd/src/gf2.rs`:

```rust
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

pub(crate) fn sort_columns_by_reliability(scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    order
}

pub(crate) fn solve_with_column_order(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    column_order: &[usize],
) -> Result<Correction, DecodeError> {
    let mut matrix = pcm.dense_rows();
    let mut rhs = syndrome.as_slice().to_vec();
    let mut pivot_columns = Vec::new();
    let mut row = 0usize;

    for &column in column_order {
        if row == matrix.len() {
            break;
        }
        let pivot = (row..matrix.len()).find(|&candidate| matrix[candidate][column]);
        if let Some(pivot_row) = pivot {
            matrix.swap(row, pivot_row);
            rhs.swap(row, pivot_row);
            for other in 0..matrix.len() {
                if other != row && matrix[other][column] {
                    for c in column..column_order.len() {
                        let physical = column_order[c];
                        matrix[other][physical] ^= matrix[row][physical];
                    }
                    rhs[other] ^= rhs[row];
                }
            }
            pivot_columns.push(column);
            row += 1;
        }
    }

    if rhs.iter().skip(row).any(|&bit| bit) {
        return Err(DecodeError::SingularSystem);
    }

    let mut solution = vec![false; pcm.num_bits()];
    for (pivot_row, &column) in pivot_columns.iter().enumerate().rev() {
        let mut value = rhs[pivot_row];
        for later_column in pivot_columns.iter().skip(pivot_row + 1) {
            value ^= matrix[pivot_row][*later_column] && solution[*later_column];
        }
        solution[column] = value;
    }

    Ok(Correction::from(solution))
}
```

Update `rbposd/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod gf2;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rbposd solve_with_column_order_returns_a_valid_solution -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rbposd/src/lib.rs rbposd/src/error.rs rbposd/src/gf2.rs
git commit -m "feat: add rbposd gf2 elimination helpers"
```

## Phase 2

### Task 4: Implement Minimum-Sum BP And The Public Decoder Shell

**Files:**
- Modify: `rbposd/src/lib.rs`
- Create: `rbposd/src/bp.rs`
- Create: `rbposd/src/decoder.rs`
- Modify: `rbposd/src/error.rs`
- Test: `rbposd/tests/bp.rs`

- [ ] **Step 1: Write the failing test**

Create `rbposd/tests/bp.rs`:

```rust
use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};

fn repetition_pcm() -> ParityCheckMatrix {
    ParityCheckMatrix::from_sparse_rows(
        4,
        5,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
    )
    .unwrap()
}

#[test]
fn minimum_sum_decodes_a_single_flip_without_osd() {
    let pcm = repetition_pcm();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false, false, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations > 0, true);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
    assert_eq!(result.correction, Correction::from(vec![true, false, false, false, false]));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd --test bp -v`

Expected: FAIL with unresolved imports for `BpOsdDecoder` or missing `decode`.

- [ ] **Step 3: Write minimal implementation**

Extend `rbposd/src/error.rs` so decoder construction can reject bad prior vectors:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    EmptyMatrix,
    InvalidProbability,
    InvalidColumnIndex { column: usize, num_bits: usize },
    InvalidRowIndex { row: usize, num_checks: usize },
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
    SingularSystem,
    BpDidNotConverge,
    NoOsdSolution,
}
```

Create `rbposd/src/bp.rs`:

```rust
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BpSnapshot {
    pub hard_decision: Correction,
    pub reliability: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
    pub residual_weight: usize,
}

pub(crate) fn run_minimum_sum(
    pcm: &ParityCheckMatrix,
    prior_llrs: &[f64],
    syndrome: &Syndrome,
    max_iterations: usize,
    early_stop: bool,
) -> BpSnapshot {
    let num_bits = pcm.num_bits();
    let num_checks = pcm.num_checks();

    let mut check_to_var: Vec<Vec<f64>> = pcm
        .row_neighbors(0)
        .iter()
        .map(|_| vec![0.0; 0])
        .collect();
    check_to_var = (0..num_checks)
        .map(|check| vec![0.0; pcm.row_neighbors(check).len()])
        .collect();
    let mut var_to_check = (0..num_checks)
        .map(|check| vec![0.0; pcm.row_neighbors(check).len()])
        .collect::<Vec<_>>();

    for check in 0..num_checks {
        for (edge_index, &bit) in pcm.row_neighbors(check).iter().enumerate() {
            var_to_check[check][edge_index] = prior_llrs[bit];
        }
    }

    let mut final_llrs = prior_llrs.to_vec();
    let mut hard_decision = Correction::zero(num_bits);
    let mut residual_weight = syndrome.len();

    for iteration in 1..=max_iterations.max(1) {
        for check in 0..num_checks {
            let neighbors = pcm.row_neighbors(check);
            for target_edge in 0..neighbors.len() {
                let mut sign = if syndrome.as_slice()[check] { -1.0 } else { 1.0 };
                let mut min_abs = f64::INFINITY;
                for source_edge in 0..neighbors.len() {
                    if source_edge == target_edge {
                        continue;
                    }
                    let value = var_to_check[check][source_edge];
                    sign *= value.signum();
                    min_abs = min_abs.min(value.abs());
                }
                if min_abs.is_infinite() {
                    min_abs = 0.0;
                }
                check_to_var[check][target_edge] = sign * min_abs;
            }
        }

        for bit in 0..num_bits {
            let checks = pcm.column_neighbors(bit);
            let mut llr = prior_llrs[bit];
            for &check in checks {
                let edge_index = pcm
                    .row_neighbors(check)
                    .iter()
                    .position(|&candidate| candidate == bit)
                    .unwrap();
                llr += check_to_var[check][edge_index];
            }
            final_llrs[bit] = llr;
        }

        for check in 0..num_checks {
            for (edge_index, &bit) in pcm.row_neighbors(check).iter().enumerate() {
                let mut llr = prior_llrs[bit];
                for &neighbor_check in pcm.column_neighbors(bit) {
                    if neighbor_check == check {
                        continue;
                    }
                    let neighbor_edge = pcm
                        .row_neighbors(neighbor_check)
                        .iter()
                        .position(|&candidate| candidate == bit)
                        .unwrap();
                    llr += check_to_var[neighbor_check][neighbor_edge];
                }
                var_to_check[check][edge_index] = llr;
            }
        }

        hard_decision = Correction::from(final_llrs.iter().map(|&llr| llr < 0.0).collect());
        residual_weight = pcm
            .multiply(&hard_decision)
            .as_slice()
            .iter()
            .zip(syndrome.as_slice().iter())
            .filter(|(lhs, rhs)| lhs != rhs)
            .count();

        if early_stop && residual_weight == 0 {
            return BpSnapshot {
                hard_decision,
                reliability: final_llrs.iter().map(|value| value.abs()).collect(),
                iterations: iteration,
                converged: true,
                residual_weight,
            };
        }
    }

    BpSnapshot {
        hard_decision,
        reliability: final_llrs.iter().map(|value| value.abs()).collect(),
        iterations: max_iterations.max(1),
        converged: residual_weight == 0,
        residual_weight,
    }
}
```

Create `rbposd/src/decoder.rs`:

```rust
use crate::bp::run_minimum_sum;
use crate::config::ChannelModel;
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};
use crate::DecoderConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    pub correction: Correction,
    pub converged: bool,
    pub bp_iterations: usize,
    pub used_osd: bool,
    pub residual_syndrome_weight: usize,
}

#[derive(Debug, Clone)]
pub struct BpOsdDecoder {
    pcm: ParityCheckMatrix,
    prior_llrs: Vec<f64>,
    config: DecoderConfig,
}

impl BpOsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        let prior_llrs = match channel {
            ChannelModel::Bsc { error_rate } => {
                if !(0.0..=1.0).contains(&error_rate) || error_rate == 0.0 || error_rate == 1.0 {
                    return Err(DecodeError::InvalidProbability);
                }
                let llr = ((1.0 - error_rate) / error_rate).ln();
                vec![llr; pcm.num_bits()]
            }
            ChannelModel::BitFlipProbabilities(probabilities) => {
                if probabilities.len() != pcm.num_bits() {
                    return Err(DecodeError::DimensionMismatch {
                        what: "channel probabilities",
                        expected: pcm.num_bits(),
                        actual: probabilities.len(),
                    });
                }
                let mut llrs = Vec::with_capacity(probabilities.len());
                for probability in probabilities {
                    if !(0.0..=1.0).contains(&probability) || probability == 0.0 || probability == 1.0 {
                        return Err(DecodeError::InvalidProbability);
                    }
                    llrs.push(((1.0 - probability) / probability).ln());
                }
                llrs
            }
        };

        Ok(Self {
            pcm,
            prior_llrs,
            config,
        })
    }

    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            return Ok(DecodeResult {
                correction: Correction::zero(self.pcm.num_bits()),
                converged: true,
                bp_iterations: 0,
                used_osd: false,
                residual_syndrome_weight: 0,
            });
        }

        let snapshot = run_minimum_sum(
            &self.pcm,
            &self.prior_llrs,
            syndrome,
            self.config.max_bp_iterations,
            self.config.early_stop,
        );

        if snapshot.residual_weight != 0 {
            return Err(DecodeError::BpDidNotConverge);
        }

        Ok(DecodeResult {
            correction: snapshot.hard_decision,
            converged: snapshot.converged,
            bp_iterations: snapshot.iterations,
            used_osd: false,
            residual_syndrome_weight: snapshot.residual_weight,
        })
    }
}
```

Update `rbposd/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod bp;
mod decoder;
mod gf2;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use decoder::{BpOsdDecoder, DecodeResult};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rbposd --test bp -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rbposd/src/lib.rs rbposd/src/bp.rs rbposd/src/decoder.rs rbposd/src/error.rs rbposd/tests/bp.rs
git commit -m "feat: add rbposd minimum-sum decoder path"
```

### Task 5: Add `OSD_0` Fallback And Return Valid Corrections On BP Failure

**Files:**
- Modify: `rbposd/src/lib.rs`
- Create: `rbposd/src/osd.rs`
- Modify: `rbposd/src/decoder.rs`
- Test: `rbposd/tests/osd.rs`

- [ ] **Step 1: Write the failing test**

Create `rbposd/tests/osd.rs`:

```rust
use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

#[test]
fn osd0_recovers_a_valid_solution_when_bp_is_disabled() {
    let pcm =
        ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;

    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
        config,
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd --test osd -v`

Expected: FAIL with `belief propagation did not converge`.

- [ ] **Step 3: Write minimal implementation**

Create `rbposd/src/osd.rs`:

```rust
use crate::error::DecodeError;
use crate::gf2::{solve_with_column_order, sort_columns_by_reliability};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

pub(crate) fn decode_osd0(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    reliability: &[f64],
) -> Result<Correction, DecodeError> {
    let order = sort_columns_by_reliability(reliability);
    solve_with_column_order(pcm, syndrome, &order).map_err(|_| DecodeError::NoOsdSolution)
}
```

Update `rbposd/src/decoder.rs` so `decode(...)` falls back to `OSD_0`:

```rust
use crate::bp::run_minimum_sum;
use crate::config::ChannelModel;
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::osd::decode_osd0;
use crate::vector::{Correction, Syndrome};
use crate::DecoderConfig;

impl BpOsdDecoder {
    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            return Ok(DecodeResult {
                correction: Correction::zero(self.pcm.num_bits()),
                converged: true,
                bp_iterations: 0,
                used_osd: false,
                residual_syndrome_weight: 0,
            });
        }

        let snapshot = run_minimum_sum(
            &self.pcm,
            &self.prior_llrs,
            syndrome,
            self.config.max_bp_iterations,
            self.config.early_stop,
        );

        if snapshot.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: snapshot.hard_decision,
                converged: snapshot.converged,
                bp_iterations: snapshot.iterations,
                used_osd: false,
                residual_syndrome_weight: 0,
            });
        }

        let correction = decode_osd0(&self.pcm, syndrome, &snapshot.reliability)?;

        Ok(DecodeResult {
            correction,
            converged: snapshot.converged,
            bp_iterations: snapshot.iterations,
            used_osd: true,
            residual_syndrome_weight: 0,
        })
    }
}
```

Update `rbposd/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod bp;
mod decoder;
mod gf2;
mod osd;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use decoder::{BpOsdDecoder, DecodeResult};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rbposd --test osd -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rbposd/src/lib.rs rbposd/src/osd.rs rbposd/src/decoder.rs rbposd/tests/osd.rs
git commit -m "feat: add rbposd OSD0 fallback"
```

## Phase 3

### Task 6: Add Reference Fixtures, Example Usage, And A Repeatable Profile Loop

**Files:**
- Create: `rbposd/tests/reference.rs`
- Create: `rbposd/examples/basic_decode.rs`
- Create: `rbposd/examples/profile_repetition.rs`
- Modify: `rbposd/src/lib.rs`
- Test: `rbposd/tests/reference.rs`

- [ ] **Step 1: Write the failing test**

Create `rbposd/tests/reference.rs`:

```rust
use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

struct Case {
    name: &'static str,
    pcm: ParityCheckMatrix,
    channel: ChannelModel,
    syndrome: Syndrome,
    expect_osd: bool,
}

#[test]
fn reference_contract_cases_stay_valid() {
    let mut osd_only = DecoderConfig::default();
    osd_only.max_bp_iterations = 0;

    let cases = vec![
        Case {
            name: "bp repetition single flip",
            pcm: ParityCheckMatrix::from_sparse_rows(
                4,
                5,
                vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
            )
            .unwrap(),
            channel: ChannelModel::Bsc { error_rate: 0.05 },
            syndrome: Syndrome::from(vec![true, false, false, false]),
            expect_osd: false,
        },
        Case {
            name: "osd fallback small sparse code",
            pcm: ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap(),
            channel: ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
            syndrome: Syndrome::from(vec![true, false]),
            expect_osd: true,
        },
    ];

    for case in cases {
        let config = if case.expect_osd {
            osd_only.clone()
        } else {
            DecoderConfig::default()
        };
        let decoder = BpOsdDecoder::new(case.pcm.clone(), case.channel, config).unwrap();
        let result = decoder.decode(&case.syndrome).unwrap();

        assert_eq!(result.used_osd, case.expect_osd, "{}", case.name);
        assert_eq!(case.pcm.multiply(&result.correction), case.syndrome, "{}", case.name);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd --test reference -v`

Expected: FAIL because the reference fixture path has not been stabilized yet.

- [ ] **Step 3: Write minimal implementation**

Create `rbposd/examples/basic_decode.rs`:

```rust
use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        4,
        5,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
    )?;
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )?;
    let syndrome = Syndrome::from(vec![true, false, false, false]);
    let result = decoder.decode(&syndrome)?;

    println!("used_osd={}", result.used_osd);
    println!("bp_iterations={}", result.bp_iterations);
    println!("correction={:?}", result.correction.as_slice());
    println!("valid={}", pcm.multiply(&result.correction) == syndrome);
    Ok(())
}
```

Create `rbposd/examples/profile_repetition.rs`:

```rust
use std::time::Instant;

use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        4,
        5,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
    )?;
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )?;

    let syndromes = [
        Syndrome::from(vec![true, false, false, false]),
        Syndrome::from(vec![false, true, false, false]),
        Syndrome::from(vec![false, false, true, false]),
        Syndrome::from(vec![false, false, false, true]),
    ];

    let mut total_ns = 0u128;
    let mut total_iterations = 0usize;
    let mut osd_uses = 0usize;

    for syndrome in syndromes.iter().cycle().take(200) {
        let start = Instant::now();
        let result = decoder.decode(syndrome)?;
        total_ns += start.elapsed().as_nanos();
        total_iterations += result.bp_iterations;
        osd_uses += usize::from(result.used_osd);
    }

    println!("shots=200");
    println!("avg_ns={}", total_ns / 200);
    println!("avg_iterations={:.2}", total_iterations as f64 / 200.0);
    println!("osd_uses={}", osd_uses);
    Ok(())
}
```

Append a short crate-level usage example to `rbposd/src/lib.rs`:

```rust
//! ```rust
//! use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};
//!
//! let pcm = ParityCheckMatrix::from_sparse_rows(
//!     2,
//!     3,
//!     vec![vec![0, 1], vec![1, 2]],
//! )
//! .unwrap();
//! let decoder = BpOsdDecoder::new(
//!     pcm.clone(),
//!     ChannelModel::Bsc { error_rate: 0.05 },
//!     DecoderConfig::default(),
//! )
//! .unwrap();
//! let syndrome = Syndrome::from(vec![true, false]);
//! let result = decoder.decode(&syndrome).unwrap();
//! assert_eq!(pcm.multiply(&result.correction), syndrome);
//! ```
```

- [ ] **Step 4: Run test and examples**

Run: `cargo test -p rbposd --test reference -v`

Expected: PASS

Run: `cargo run -p rbposd --example basic_decode`

Expected:

```text
used_osd=false
bp_iterations=1
correction=[true, false, false, false, false]
valid=true
```

Run: `cargo run -p rbposd --example profile_repetition`

Expected: PASS with four metric lines including `shots=200`.

- [ ] **Step 5: Commit**

```bash
git add rbposd/tests/reference.rs rbposd/examples/basic_decode.rs rbposd/examples/profile_repetition.rs rbposd/src/lib.rs
git commit -m "test: add rbposd reference fixtures and examples"
```

## Phase 4

### Task 7: Add The Thin CSS Helper Layer

**Files:**
- Modify: `rbposd/src/lib.rs`
- Create: `rbposd/src/css.rs`
- Create: `rbposd/tests/css.rs`
- Test: `rbposd/tests/css.rs`

- [ ] **Step 1: Write the failing test**

Create `rbposd/tests/css.rs`:

```rust
use rbposd::{ChannelModel, CssDecoders, DecoderConfig, ParityCheckMatrix, Syndrome};

#[test]
fn css_decoders_route_x_and_z_syndromes_to_different_matrices() {
    let hx = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let hz = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![1]]).unwrap();

    let css = CssDecoders::new(
        hx.clone(),
        hz.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )
    .unwrap();

    let x = css.decode_x(&Syndrome::from(vec![true])).unwrap();
    let z = css.decode_z(&Syndrome::from(vec![true])).unwrap();

    assert_eq!(hx.multiply(&x.correction), Syndrome::from(vec![true]));
    assert_eq!(hz.multiply(&z.correction), Syndrome::from(vec![true]));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rbposd --test css -v`

Expected: FAIL with unresolved import for `CssDecoders`.

- [ ] **Step 3: Write minimal implementation**

Create `rbposd/src/css.rs`:

```rust
use crate::config::ChannelModel;
use crate::decoder::{BpOsdDecoder, DecodeResult};
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::Syndrome;
use crate::DecoderConfig;

#[derive(Debug, Clone)]
pub struct CssDecoders {
    x: BpOsdDecoder,
    z: BpOsdDecoder,
}

impl CssDecoders {
    pub fn new(
        hx: ParityCheckMatrix,
        hz: ParityCheckMatrix,
        x_channel: ChannelModel,
        z_channel: ChannelModel,
        config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        Ok(Self {
            x: BpOsdDecoder::new(hx, x_channel, config.clone())?,
            z: BpOsdDecoder::new(hz, z_channel, config)?,
        })
    }

    pub fn decode_x(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        self.x.decode(syndrome)
    }

    pub fn decode_z(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        self.z.decode(syndrome)
    }
}
```

Update `rbposd/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod matrix;
pub mod vector;

mod bp;
mod css;
mod decoder;
mod gf2;
mod osd;

pub use config::{BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule};
pub use css::CssDecoders;
pub use decoder::{BpOsdDecoder, DecodeResult};
pub use error::DecodeError;
pub use matrix::ParityCheckMatrix;
pub use vector::{Correction, Syndrome};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rbposd --test css -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rbposd/src/lib.rs rbposd/src/css.rs rbposd/tests/css.rs
git commit -m "feat: add rbposd css convenience layer"
```

## Phase 5

### Task 8: Add The `rsinter` DEM Adapter Backed By `rbposd`

**Files:**
- Modify: `rsinter/Cargo.toml`
- Modify: `rsinter/src/lib.rs`
- Modify: `rsinter/src/decode.rs`
- Modify: `rsinter/src/collect.rs`
- Create: `rsinter/src/rbposd_adapter.rs`
- Create: `rsinter/tests/decode_rbposd.rs`
- Modify: `rstim/doc/getting_started.md`
- Test: `rsinter/tests/decode_rbposd.rs`

- [ ] **Step 1: Write the failing test**

Create `rsinter/tests/decode_rbposd.rs`:

```rust
use std::collections::HashMap;

use rbposd::DecoderConfig;
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::{Decoder, RbposdDemDecoder};
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

#[test]
fn rbposd_dem_decoder_predicts_a_single_observable_flip() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RbposdDemDecoder::new(DecoderConfig::default());
    let compiled = decoder.compile_for_dem(&dem);

    let predictions = compiled.decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1);

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn collect_runs_with_the_rbposd_adapter() {
    let circuit = parse_lines(
        "R 0\nX_ERROR(0.05) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();

    let task = Task {
        circuit,
        decoder: "rbposd".into(),
        dem,
        metadata: serde_json::json!({"case": "single-qubit"}),
        collection_options: CollectionOptions {
            max_shots: Some(32),
            max_errors: Some(32),
        },
    };

    let mut decoders: HashMap<String, Box<dyn Decoder>> = HashMap::new();
    decoders.insert(
        "rbposd".into(),
        Box::new(RbposdDemDecoder::new(DecoderConfig::default())),
    );

    let results = collect(
        vec![task],
        decoders,
        &CollectOptions {
            num_workers: 1,
            max_shots: None,
            max_errors: None,
            max_batch_size: Some(32),
            start_batch_size: 8,
            save_resume_filepath: None,
            print_progress: false,
        },
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].shots > 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rsinter --test decode_rbposd -v`

Expected: FAIL with unresolved import for `RbposdDemDecoder` and missing `rbposd` dependency.

- [ ] **Step 3: Write minimal implementation**

Update `rsinter/Cargo.toml`:

```toml
[dependencies]
rstim = { path = "../rstim" }
rbposd = { path = "../rbposd" }
rand = "0.8"
rayon = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
csv = "1"
sha2 = "0.10"
plotters = { version = "0.3", default-features = false, features = ["svg_backend", "bitmap_backend", "bitmap_encoder", "line_series", "full_palette", "ttf"] }
```

Create `rsinter/src/rbposd_adapter.rs`:

```rust
use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};
use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};

use crate::decode::{CompiledDecoder, Decoder};

pub struct RbposdDemDecoder {
    config: DecoderConfig,
}

impl RbposdDemDecoder {
    pub fn new(config: DecoderConfig) -> Self {
        Self { config }
    }
}

struct CompiledRbposdDemDecoder {
    decoder: BpOsdDecoder,
    observable_columns: Vec<Vec<usize>>,
    num_obs: usize,
}

impl Decoder for RbposdDemDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let (pcm, probabilities, observable_columns, num_obs) = dem_to_matrix_problem(dem);
        let decoder = BpOsdDecoder::new(
            pcm,
            ChannelModel::BitFlipProbabilities(probabilities),
            self.config.clone(),
        )
        .expect("DEM lowering produced an invalid rbposd problem");

        Box::new(CompiledRbposdDemDecoder {
            decoder,
            observable_columns,
            num_obs,
        })
    }
}

impl CompiledDecoder for CompiledRbposdDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        let obs_bytes = (num_obs + 7) / 8;
        let det_bytes = num_dets.div_ceil(8);
        let mut out = vec![0u8; num_shots * obs_bytes];

        for shot in 0..num_shots {
            let offset = shot * det_bytes;
            let syndrome_bits: Vec<bool> = (0..num_dets)
                .map(|det| {
                    let byte = dets[offset + (det / 8)];
                    ((byte >> (det % 8)) & 1) == 1
                })
                .collect();
            let result = self
                .decoder
                .decode(&Syndrome::from(syndrome_bits))
                .expect("rbposd decode failed");
            let observable_bits = correction_to_observables(
                &result.correction,
                &self.observable_columns,
                self.num_obs,
            );
            for obs in 0..self.num_obs {
                if observable_bits[obs] {
                    out[shot * obs_bytes + (obs / 8)] |= 1 << (obs % 8);
                }
            }
        }

        out
    }
}

fn correction_to_observables(
    correction: &Correction,
    observable_columns: &[Vec<usize>],
    num_obs: usize,
) -> Vec<bool> {
    let mut out = vec![false; num_obs];
    for (column, &enabled) in correction.as_slice().iter().enumerate() {
        if !enabled {
            continue;
        }
        for &obs in &observable_columns[column] {
            out[obs] ^= true;
        }
    }
    out
}

fn dem_to_matrix_problem(
    dem: &DetectorErrorModel,
) -> (ParityCheckMatrix, Vec<f64>, Vec<Vec<usize>>, usize) {
    let num_dets = dem.effective_num_detectors();
    let num_obs = dem.num_observables();
    let mut detector_columns: Vec<Vec<usize>> = Vec::new();
    let mut observable_columns: Vec<Vec<usize>> = Vec::new();
    let mut probabilities: Vec<f64> = Vec::new();

    fn visit(
        instrs: &[DemInstruction],
        detector_offset: usize,
        detector_columns: &mut Vec<Vec<usize>>,
        observable_columns: &mut Vec<Vec<usize>>,
        probabilities: &mut Vec<f64>,
    ) {
        let mut offset = detector_offset;
        for instr in instrs {
            match instr {
                DemInstruction::Error { probability, targets } => {
                    let mut current_dets = Vec::new();
                    let mut current_obs = Vec::new();
                    for target in targets {
                        match target {
                            DemTarget::Detector(det) => current_dets.push(offset + det),
                            DemTarget::Observable(obs) => current_obs.push(*obs),
                            DemTarget::Separator => {
                                detector_columns.push(current_dets.clone());
                                observable_columns.push(current_obs.clone());
                                probabilities.push(*probability);
                                current_dets.clear();
                                current_obs.clear();
                            }
                        }
                    }
                    detector_columns.push(current_dets);
                    observable_columns.push(current_obs);
                    probabilities.push(*probability);
                }
                DemInstruction::ShiftDetectors { detector_offset, .. } => {
                    offset += detector_offset;
                }
                DemInstruction::Repeat { count, body } => {
                    for _ in 0..*count {
                        visit(
                            body.instructions(),
                            offset,
                            detector_columns,
                            observable_columns,
                            probabilities,
                        );
                        offset += body.instructions().iter().fold(0usize, |acc, instruction| {
                            match instruction {
                                DemInstruction::ShiftDetectors { detector_offset, .. } => {
                                    acc + detector_offset
                                }
                                _ => acc,
                            }
                        });
                    }
                }
                DemInstruction::Detector { .. } | DemInstruction::LogicalObservable { .. } => {}
            }
        }
    }

    visit(
        dem.instructions(),
        0,
        &mut detector_columns,
        &mut observable_columns,
        &mut probabilities,
    );

    let pcm =
        ParityCheckMatrix::from_sparse_columns(num_dets, detector_columns.len(), detector_columns)
            .expect("generated DEM matrix should be valid");

    (pcm, probabilities, observable_columns, num_obs)
}
```

Update `rsinter/src/decode.rs`:

```rust
use rstim::dem::DetectorErrorModel;

pub use crate::rbposd_adapter::RbposdDemDecoder;

pub trait CompiledDecoder: Send {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8>;
}

pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder>;
}

pub struct VacuousDecoder;

struct VacuousCompiled;

impl CompiledDecoder for VacuousCompiled {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        let obs_bytes = (num_obs + 7) / 8;
        vec![0u8; num_shots * obs_bytes]
    }
}

impl Decoder for VacuousDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        Box::new(VacuousCompiled)
    }
}
```

Update `rsinter/src/lib.rs`:

```rust
mod rbposd_adapter;

pub mod stats;
pub mod decode;
pub mod task;
pub mod task_stats;
pub mod csv_io;
pub mod collect;
pub mod plot;
```

Update `rsinter/src/collect.rs` so detector width matches shifted/repeated DEMs:

```rust
let num_dets = task.dem.effective_num_detectors();
let num_obs = task.dem.num_observables();
```

Add a short adapter example to `rstim/doc/getting_started.md` after the existing `rsinter` section:

```markdown
## Decode with rbposd through rsinter

Once `rbposd` is available in the workspace, `rsinter` can compile a DEM into
an in-tree BPOSD decoder:

    use rsinter::decode::RbposdDemDecoder;
    use rbposd::DecoderConfig;

    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("rbposd".into(), Box::new(RbposdDemDecoder::new(DecoderConfig::default())));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rsinter --test decode_rbposd -v`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rsinter/Cargo.toml rsinter/src/lib.rs rsinter/src/decode.rs rsinter/src/collect.rs rsinter/src/rbposd_adapter.rs rsinter/tests/decode_rbposd.rs rstim/doc/getting_started.md
git commit -m "feat: add rbposd adapter for rsinter"
```

## Self-Review Checklist

- [ ] **Spec coverage:** Confirm the plan covers the spec sections for independent crate setup, matrix-first API, `minimum-sum + parallel + OSD_0`, CSS helper, and `rsinter` integration.
- [ ] **Placeholder scan:** Search this file for `TBD`, `TODO`, `implement later`, `fill in details`, and `similar to`.
- [ ] **Type consistency:** Confirm these names stay consistent across tasks:
  - `ParityCheckMatrix`
  - `Syndrome`
  - `Correction`
  - `ChannelModel`
  - `DecoderConfig`
  - `BpOsdDecoder`
  - `DecodeResult`
  - `CssDecoders`
  - `RbposdDemDecoder`
