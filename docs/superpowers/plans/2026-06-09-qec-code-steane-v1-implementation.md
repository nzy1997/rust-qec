# QEC Code Steane V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new in-workspace `qec-code` crate that models qubit stabilizer codes in binary symplectic form, ships Steane as the first built-in code, computes validated logical operators and exact small-code distance, and exposes a minimal CLI for inspecting Steane.

**Architecture:** Build the crate in four layers. First scaffold the crate and the GF(2)/Pauli foundations. Then add the validated `StabilizerCode` core plus CSS and Steane constructors. Next implement logical-basis extraction and exact distance on top of the same core representation. Finish with a small `clap` CLI and smoke tests that exercise the Steane workflows end to end.

**Tech Stack:** Rust 2024 workspace crate, `clap`, `thiserror`, `serde`, `serde_json`, `cargo test`, existing workspace conventions for `src/lib.rs`, `src/main.rs`, and `tests/*.rs`

---

## File Structure

- Modify: `Cargo.toml`
  - Add `qec-code` as a workspace member.
- Create: `qec-code/Cargo.toml`
  - Define the new package, library, and binary dependencies.
- Create: `qec-code/src/lib.rs`
  - Re-export the public API in dependency order.
- Create: `qec-code/src/error.rs`
  - Define domain-specific construction and analysis errors.
- Create: `qec-code/src/binary.rs`
  - Hold GF(2) matrix helpers used by code validation, logical extraction, and distance.
- Create: `qec-code/src/pauli.rs`
  - Hold binary symplectic `Pauli` representation, weight, and commutation helpers.
- Create: `qec-code/src/code.rs`
  - Define `StabilizerCode`, constructor validation, and basic invariants such as `n`, rank, and `k`.
- Create: `qec-code/src/css.rs`
  - Define CSS convenience construction from `Hx`/`Hz`.
- Create: `qec-code/src/codes/mod.rs`
  - Register built-in code modules.
- Create: `qec-code/src/codes/steane.rs`
  - Define `Steane::new()` and expose its stabilizer metadata.
- Create: `qec-code/src/logical.rs`
  - Define `LogicalBasis` and exact logical-operator extraction over the symplectic core.
- Create: `qec-code/src/distance.rs`
  - Define `DistanceResult` and exact small-code distance search.
- Create: `qec-code/src/cli.rs`
  - Define `clap` types and text/JSON output helpers for Steane inspection commands.
- Create: `qec-code/src/main.rs`
  - Parse CLI args and route to `qec_code::cli::run(...)`.
- Create: `qec-code/tests/binary.rs`
  - Cover GF(2) rank and span membership.
- Create: `qec-code/tests/code.rs`
  - Cover stabilizer-code validation, CSS construction, and Steane invariants.
- Create: `qec-code/tests/logical_distance.rs`
  - Cover logical-basis properties and exact Steane distance.
- Create: `qec-code/tests/cli.rs`
  - Cover Steane CLI subcommands and stable output surface.
- Create: `docs/superpowers/plans/2026-06-09-qec-code-steane-v1-implementation.md`
  - Implementation handoff for the approved design.

## Task 1: Scaffold The `qec-code` Crate And Lock The Algebra Foundations

**Files:**
- Modify: `Cargo.toml`
- Create: `qec-code/Cargo.toml`
- Create: `qec-code/src/lib.rs`
- Create: `qec-code/src/error.rs`
- Create: `qec-code/src/binary.rs`
- Create: `qec-code/src/pauli.rs`
- Create: `qec-code/tests/binary.rs`

- [ ] **Step 1: Write the failing algebra tests**

Create `qec-code/tests/binary.rs`:

```rust
use qec_code::binary::{binary_rank, in_row_span};
use qec_code::pauli::Pauli;

#[test]
fn binary_rank_counts_independent_rows_over_gf2() {
    let rows = vec![
        vec![1, 0, 1, 0],
        vec![0, 1, 1, 0],
        vec![1, 1, 0, 0],
    ];

    assert_eq!(binary_rank(&rows), 2);
}

#[test]
fn in_row_span_detects_membership_after_reduction() {
    let basis = vec![
        vec![1, 0, 1, 0],
        vec![0, 1, 1, 0],
    ];

    assert!(in_row_span(&basis, &[1, 1, 0, 0]));
    assert!(!in_row_span(&basis, &[1, 0, 0, 1]));
}

#[test]
fn pauli_commutation_and_weight_follow_symplectic_rules() {
    let x0 = Pauli::from_xz_bits(2, vec![1, 0], vec![0, 0]).unwrap();
    let z0 = Pauli::from_xz_bits(2, vec![0, 0], vec![1, 0]).unwrap();
    let x1 = Pauli::from_xz_bits(2, vec![0, 1], vec![0, 0]).unwrap();

    assert_eq!(x0.weight(), 1);
    assert!(x0.anticommutes_with(&z0));
    assert!(x0.commutes_with(&x1));
}
```

- [ ] **Step 2: Run the focused test target and verify it fails**

Run:

```bash
cargo test -p qec-code --test binary -v
```

Expected: FAIL with `package ID specification 'qec-code' did not match any packages`.

- [ ] **Step 3: Create the crate scaffold and minimal algebra implementation**

Update the workspace root `Cargo.toml`:

```toml
[workspace]
members = [
    "rstim",
    "rsinter",
    "rbposd",
    "rmatching",
    "rilpqec",
    "qec-code",
    "benchmarks/surface_decoder_compare/rust_bridge",
]
resolver = "3"
```

Create `qec-code/Cargo.toml`:

```toml
[package]
name = "qec-code"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

Create `qec-code/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
    #[error("invalid pauli width: expected {expected}, got {actual}")]
    InvalidPauliWidth { expected: usize, actual: usize },
    #[error("stabilizers do not commute")]
    NonCommutingStabilizers,
    #[error("stabilizer generators are linearly dependent")]
    DependentStabilizers,
    #[error("css parity checks do not commute")]
    InvalidCssOrthogonality,
    #[error("logical basis could not be extracted")]
    LogicalBasisNotFound,
    #[error("distance witness could not be found")]
    DistanceWitnessNotFound,
}
```

Create `qec-code/src/binary.rs`:

```rust
pub fn binary_rank(rows: &[Vec<u8>]) -> usize {
    let mut matrix = rows.to_vec();
    eliminate_to_row_echelon(&mut matrix)
}

pub fn in_row_span(rows: &[Vec<u8>], target: &[u8]) -> bool {
    let mut matrix = rows.to_vec();
    matrix.push(target.to_vec());
    binary_rank(&matrix) == binary_rank(rows)
}

pub fn eliminate_to_row_echelon(matrix: &mut [Vec<u8>]) -> usize {
    if matrix.is_empty() {
        return 0;
    }
    let width = matrix[0].len();
    let mut rank = 0;

    for col in 0..width {
        let pivot = (rank..matrix.len()).find(|&row| matrix[row][col] == 1);
        if let Some(pivot_row) = pivot {
            matrix.swap(rank, pivot_row);
            for row in 0..matrix.len() {
                if row != rank && matrix[row][col] == 1 {
                    for c in col..width {
                        matrix[row][c] ^= matrix[rank][c];
                    }
                }
            }
            rank += 1;
        }
    }

    rank
}
```

Create `qec-code/src/pauli.rs`:

```rust
use crate::error::QecError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pauli {
    n: usize,
    x: Vec<u8>,
    z: Vec<u8>,
}

impl Pauli {
    pub fn from_xz_bits(n: usize, x: Vec<u8>, z: Vec<u8>) -> Result<Self, QecError> {
        if x.len() != n {
            return Err(QecError::InvalidPauliWidth {
                expected: n,
                actual: x.len(),
            });
        }
        if z.len() != n {
            return Err(QecError::InvalidPauliWidth {
                expected: n,
                actual: z.len(),
            });
        }
        Ok(Self { n, x, z })
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn x_bits(&self) -> &[u8] {
        &self.x
    }

    pub fn z_bits(&self) -> &[u8] {
        &self.z
    }

    pub fn symplectic_product(&self, other: &Self) -> u8 {
        let xz: u8 = self
            .x
            .iter()
            .zip(other.z.iter())
            .map(|(a, b)| a & b)
            .fold(0, |acc, bit| acc ^ bit);
        let zx: u8 = self
            .z
            .iter()
            .zip(other.x.iter())
            .map(|(a, b)| a & b)
            .fold(0, |acc, bit| acc ^ bit);
        xz ^ zx
    }

    pub fn commutes_with(&self, other: &Self) -> bool {
        self.symplectic_product(other) == 0
    }

    pub fn anticommutes_with(&self, other: &Self) -> bool {
        self.symplectic_product(other) == 1
    }

    pub fn weight(&self) -> usize {
        self.x
            .iter()
            .zip(self.z.iter())
            .filter(|(x, z)| **x == 1 || **z == 1)
            .count()
    }

    pub fn to_symplectic_row(&self) -> Vec<u8> {
        let mut row = self.x.clone();
        row.extend_from_slice(&self.z);
        row
    }
}
```

Create `qec-code/src/lib.rs`:

```rust
pub mod binary;
pub mod error;
pub mod pauli;

pub use error::QecError;
pub use pauli::Pauli;
```

- [ ] **Step 4: Run the focused algebra tests and verify they pass**

Run:

```bash
cargo test -p qec-code --test binary -v
```

Expected: PASS with 3 passing tests in `tests/binary.rs`.

- [ ] **Step 5: Commit the scaffold checkpoint**

```bash
git add Cargo.toml qec-code/Cargo.toml qec-code/src/lib.rs qec-code/src/error.rs qec-code/src/binary.rs qec-code/src/pauli.rs qec-code/tests/binary.rs
git commit -m "feat: scaffold qec-code algebra foundations"
```

## Task 2: Add `StabilizerCode`, CSS Construction, And Built-In Steane

**Files:**
- Create: `qec-code/src/code.rs`
- Create: `qec-code/src/css.rs`
- Create: `qec-code/src/codes/mod.rs`
- Create: `qec-code/src/codes/steane.rs`
- Modify: `qec-code/src/lib.rs`
- Create: `qec-code/tests/code.rs`

- [ ] **Step 1: Write the failing code-construction tests**

Create `qec-code/tests/code.rs`:

```rust
use qec_code::codes::steane::Steane;
use qec_code::css::CssCode;
use qec_code::{Pauli, QecError, StabilizerCode};

#[test]
fn stabilizer_code_rejects_non_commuting_generators() {
    let x0 = Pauli::from_xz_bits(1, vec![1], vec![0]).unwrap();
    let z0 = Pauli::from_xz_bits(1, vec![0], vec![1]).unwrap();

    let err = StabilizerCode::from_stabilizers(1, vec![x0, z0]).unwrap_err();
    assert_eq!(err, QecError::NonCommutingStabilizers);
}

#[test]
fn css_code_requires_hx_hz_orthogonality() {
    let err = CssCode::from_hx_hz(vec![vec![1]], vec![vec![1]]).unwrap_err();
    assert_eq!(err, QecError::InvalidCssOrthogonality);
}

#[test]
fn steane_constructor_exposes_expected_basic_invariants() {
    let steane = Steane::new().unwrap();

    assert_eq!(steane.code().n(), 7);
    assert_eq!(steane.code().stabilizer_rank(), 6);
    assert_eq!(steane.code().num_logical_qubits(), 1);
    assert_eq!(steane.code().stabilizers().len(), 6);
}
```

- [ ] **Step 2: Run the focused construction tests and verify they fail**

Run:

```bash
cargo test -p qec-code --test code -v
```

Expected: FAIL with unresolved imports for `StabilizerCode`, `CssCode`, and `codes::steane::Steane`.

- [ ] **Step 3: Implement validated code construction and Steane**

Create `qec-code/src/code.rs`:

```rust
use crate::binary::binary_rank;
use crate::{Pauli, QecError};

#[derive(Debug, Clone)]
pub struct StabilizerCode {
    n: usize,
    stabilizers: Vec<Pauli>,
    stabilizer_rank: usize,
}

impl StabilizerCode {
    pub fn from_stabilizers(n: usize, stabilizers: Vec<Pauli>) -> Result<Self, QecError> {
        for stabilizer in &stabilizers {
            if stabilizer.n() != n {
                return Err(QecError::InvalidPauliWidth {
                    expected: n,
                    actual: stabilizer.n(),
                });
            }
        }

        for i in 0..stabilizers.len() {
            for j in (i + 1)..stabilizers.len() {
                if !stabilizers[i].commutes_with(&stabilizers[j]) {
                    return Err(QecError::NonCommutingStabilizers);
                }
            }
        }

        let rows: Vec<Vec<u8>> = stabilizers.iter().map(Pauli::to_symplectic_row).collect();
        let rank = binary_rank(&rows);
        if rank != stabilizers.len() {
            return Err(QecError::DependentStabilizers);
        }

        Ok(Self {
            n,
            stabilizers,
            stabilizer_rank: rank,
        })
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn stabilizers(&self) -> &[Pauli] {
        &self.stabilizers
    }

    pub fn stabilizer_rank(&self) -> usize {
        self.stabilizer_rank
    }

    pub fn num_logical_qubits(&self) -> usize {
        self.n - self.stabilizer_rank
    }
}
```

Create `qec-code/src/css.rs`:

```rust
use crate::{Pauli, QecError, StabilizerCode};

#[derive(Debug, Clone)]
pub struct CssCode {
    code: StabilizerCode,
}

impl CssCode {
    pub fn from_hx_hz(hx: Vec<Vec<u8>>, hz: Vec<Vec<u8>>) -> Result<Self, QecError> {
        let n = hx
            .first()
            .map(|row| row.len())
            .or_else(|| hz.first().map(|row| row.len()))
            .unwrap_or(0);

        for row in &hx {
            if row.len() != n {
                return Err(QecError::RowWidthMismatch {
                    expected: n,
                    actual: row.len(),
                });
            }
        }
        for row in &hz {
            if row.len() != n {
                return Err(QecError::RowWidthMismatch {
                    expected: n,
                    actual: row.len(),
                });
            }
        }

        for x_row in &hx {
            for z_row in &hz {
                let overlap = x_row
                    .iter()
                    .zip(z_row.iter())
                    .map(|(x, z)| x & z)
                    .fold(0, |acc, bit| acc ^ bit);
                if overlap == 1 {
                    return Err(QecError::InvalidCssOrthogonality);
                }
            }
        }

        let mut stabilizers = Vec::new();
        for row in hx {
            stabilizers.push(Pauli::from_xz_bits(n, row, vec![0; n])?);
        }
        for row in hz {
            stabilizers.push(Pauli::from_xz_bits(n, vec![0; n], row)?);
        }

        Ok(Self {
            code: StabilizerCode::from_stabilizers(n, stabilizers)?,
        })
    }

    pub fn code(&self) -> &StabilizerCode {
        &self.code
    }
}
```

Create `qec-code/src/codes/mod.rs`:

```rust
pub mod steane;
```

Create `qec-code/src/codes/steane.rs`:

```rust
use crate::css::CssCode;
use crate::{QecError, StabilizerCode};

#[derive(Debug, Clone)]
pub struct Steane {
    code: StabilizerCode,
}

impl Steane {
    pub fn new() -> Result<Self, QecError> {
        let h = vec![
            vec![1, 1, 1, 1, 0, 0, 0],
            vec![1, 1, 0, 0, 1, 1, 0],
            vec![1, 0, 1, 0, 1, 0, 1],
        ];
        let css = CssCode::from_hx_hz(h.clone(), h)?;
        Ok(Self {
            code: css.code().clone(),
        })
    }

    pub fn code(&self) -> &StabilizerCode {
        &self.code
    }
}
```

Update `qec-code/src/lib.rs`:

```rust
pub mod binary;
pub mod code;
pub mod codes;
pub mod css;
pub mod error;
pub mod pauli;

pub use code::StabilizerCode;
pub use error::QecError;
pub use pauli::Pauli;
```

- [ ] **Step 4: Run the construction tests and verify they pass**

Run:

```bash
cargo test -p qec-code --test code -v
```

Expected: PASS with 3 passing tests in `tests/code.rs`.

- [ ] **Step 5: Commit the code-construction checkpoint**

```bash
git add qec-code/src/lib.rs qec-code/src/code.rs qec-code/src/css.rs qec-code/src/codes/mod.rs qec-code/src/codes/steane.rs qec-code/tests/code.rs
git commit -m "feat: add stabilizer code and steane constructors"
```

## Task 3: Implement Logical-Basis Extraction And Exact Steane Distance

**Files:**
- Create: `qec-code/src/logical.rs`
- Create: `qec-code/src/distance.rs`
- Modify: `qec-code/src/code.rs`
- Modify: `qec-code/src/lib.rs`
- Create: `qec-code/tests/logical_distance.rs`

- [ ] **Step 1: Write the failing logical/distance tests**

Create `qec-code/tests/logical_distance.rs`:

```rust
use qec_code::codes::steane::Steane;
use qec_code::distance::compute_distance;
use qec_code::logical::extract_logical_basis;

#[test]
fn steane_logical_basis_has_one_xz_pair_with_correct_commutation() {
    let steane = Steane::new().unwrap();
    let logicals = extract_logical_basis(steane.code()).unwrap();

    assert_eq!(logicals.k, 1);
    assert_eq!(logicals.logical_x.len(), 1);
    assert_eq!(logicals.logical_z.len(), 1);
    assert!(logicals.logical_x[0].anticommutes_with(&logicals.logical_z[0]));

    for stabilizer in steane.code().stabilizers() {
        assert!(logicals.logical_x[0].commutes_with(stabilizer));
        assert!(logicals.logical_z[0].commutes_with(stabilizer));
    }
}

#[test]
fn steane_distance_is_exactly_three_with_nontrivial_witness() {
    let steane = Steane::new().unwrap();
    let result = compute_distance(steane.code()).unwrap();

    assert_eq!(result.distance, 3);
    assert_eq!(result.witness.weight(), 3);

    for stabilizer in steane.code().stabilizers() {
        assert!(result.witness.commutes_with(stabilizer));
    }
}
```

- [ ] **Step 2: Run the focused logical/distance tests and verify they fail**

Run:

```bash
cargo test -p qec-code --test logical_distance -v
```

Expected: FAIL with unresolved imports for `logical` and `distance`.

- [ ] **Step 3: Implement exact logical-basis extraction and distance search**

Append the following helper to `qec-code/src/code.rs`:

```rust
    pub fn stabilizer_rows(&self) -> Vec<Vec<u8>> {
        self.stabilizers.iter().map(Pauli::to_symplectic_row).collect()
    }
```

Create `qec-code/src/logical.rs`:

```rust
use crate::binary::in_row_span;
use crate::{Pauli, QecError, StabilizerCode};

#[derive(Debug, Clone)]
pub struct LogicalBasis {
    pub k: usize,
    pub logical_x: Vec<Pauli>,
    pub logical_z: Vec<Pauli>,
}

pub fn extract_logical_basis(code: &StabilizerCode) -> Result<LogicalBasis, QecError> {
    let candidates = all_paulis(code.n())?;
    let stabilizer_rows = code.stabilizer_rows();
    let mut normalizer = Vec::new();

    for pauli in candidates {
        if code.stabilizers().iter().all(|s| pauli.commutes_with(s))
            && !in_row_span(&stabilizer_rows, &pauli.to_symplectic_row())
        {
            normalizer.push(pauli);
        }
    }

    for x in &normalizer {
        for z in &normalizer {
            if x.anticommutes_with(z) {
                return Ok(LogicalBasis {
                    k: code.num_logical_qubits(),
                    logical_x: vec![x.clone()],
                    logical_z: vec![z.clone()],
                });
            }
        }
    }

    Err(QecError::LogicalBasisNotFound)
}

fn all_paulis(n: usize) -> Result<Vec<Pauli>, QecError> {
    let mut out = Vec::new();
    let limit = 1usize << (2 * n);
    for mask in 0..limit {
        let mut x = vec![0; n];
        let mut z = vec![0; n];
        for i in 0..n {
            x[i] = ((mask >> i) & 1) as u8;
            z[i] = ((mask >> (n + i)) & 1) as u8;
        }
        if x.iter().all(|bit| *bit == 0) && z.iter().all(|bit| *bit == 0) {
            continue;
        }
        out.push(Pauli::from_xz_bits(n, x, z)?);
    }
    Ok(out)
}
```

Create `qec-code/src/distance.rs`:

```rust
use crate::binary::in_row_span;
use crate::{Pauli, QecError, StabilizerCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalClass {
    XLike,
    ZLike,
    Mixed,
}

#[derive(Debug, Clone)]
pub struct DistanceResult {
    pub distance: usize,
    pub witness: Pauli,
    pub logical_class: LogicalClass,
}

pub fn compute_distance(code: &StabilizerCode) -> Result<DistanceResult, QecError> {
    let stabilizer_rows = code.stabilizer_rows();
    let mut best: Option<Pauli> = None;

    for pauli in super::logical::extract_logical_basis(code)?
        .logical_x
        .into_iter()
        .chain(super::logical::extract_logical_basis(code)?.logical_z.into_iter())
        .chain(all_normalizer_candidates(code)?.into_iter())
    {
        if in_row_span(&stabilizer_rows, &pauli.to_symplectic_row()) {
            continue;
        }
        if best
            .as_ref()
            .map(|current| pauli.weight() < current.weight())
            .unwrap_or(true)
        {
            best = Some(pauli);
        }
    }

    let witness = best.ok_or(QecError::DistanceWitnessNotFound)?;
    let logical_class = match (
        witness.x_bits().iter().any(|bit| *bit == 1),
        witness.z_bits().iter().any(|bit| *bit == 1),
    ) {
        (true, false) => LogicalClass::XLike,
        (false, true) => LogicalClass::ZLike,
        _ => LogicalClass::Mixed,
    };

    Ok(DistanceResult {
        distance: witness.weight(),
        witness,
        logical_class,
    })
}

fn all_normalizer_candidates(code: &StabilizerCode) -> Result<Vec<Pauli>, QecError> {
    let mut out = Vec::new();
    let limit = 1usize << (2 * code.n());
    for mask in 0..limit {
        let mut x = vec![0; code.n()];
        let mut z = vec![0; code.n()];
        for i in 0..code.n() {
            x[i] = ((mask >> i) & 1) as u8;
            z[i] = ((mask >> (code.n() + i)) & 1) as u8;
        }
        if x.iter().all(|bit| *bit == 0) && z.iter().all(|bit| *bit == 0) {
            continue;
        }
        let pauli = Pauli::from_xz_bits(code.n(), x, z)?;
        if code.stabilizers().iter().all(|s| pauli.commutes_with(s)) {
            out.push(pauli);
        }
    }
    out.sort_by_key(Pauli::weight);
    Ok(out)
}
```

Update `qec-code/src/lib.rs`:

```rust
pub mod binary;
pub mod code;
pub mod codes;
pub mod css;
pub mod distance;
pub mod error;
pub mod logical;
pub mod pauli;

pub use code::StabilizerCode;
pub use error::QecError;
pub use pauli::Pauli;
```

- [ ] **Step 4: Run the logical/distance tests and then the full crate test suite**

Run:

```bash
cargo test -p qec-code --test logical_distance -v
cargo test -p qec-code -v
```

Expected:

- first command: PASS with 2 passing tests
- second command: PASS for `binary`, `code`, and `logical_distance`

- [ ] **Step 5: Commit the logical/distance checkpoint**

```bash
git add qec-code/src/lib.rs qec-code/src/code.rs qec-code/src/logical.rs qec-code/src/distance.rs qec-code/tests/logical_distance.rs
git commit -m "feat: add steane logicals and exact distance"
```

## Task 4: Add The Steane Inspection CLI And Smoke Tests

**Files:**
- Create: `qec-code/src/cli.rs`
- Create: `qec-code/src/main.rs`
- Modify: `qec-code/src/lib.rs`
- Create: `qec-code/tests/cli.rs`

- [ ] **Step 1: Write the failing CLI smoke tests**

Create `qec-code/tests/cli.rs`:

```rust
use std::process::Command;

fn qec_code_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qec-code"))
}

#[test]
fn steane_summary_reports_basic_invariants() {
    let output = qec_code_cmd()
        .args(["code", "steane", "summary"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("n: 7"));
    assert!(text.contains("stabilizer_rank: 6"));
    assert!(text.contains("k: 1"));
}

#[test]
fn steane_distance_reports_exact_distance_and_class() {
    let output = qec_code_cmd()
        .args(["code", "steane", "distance"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("distance: 3"));
    assert!(text.contains("logical_class:"));
}
```

- [ ] **Step 2: Run the focused CLI tests and verify they fail**

Run:

```bash
cargo test -p qec-code --test cli -v
```

Expected: FAIL because the crate does not yet provide a `qec-code` binary.

- [ ] **Step 3: Implement the CLI surface**

Create `qec-code/src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

use crate::codes::steane::Steane;
use crate::distance::compute_distance;
use crate::logical::extract_logical_basis;
use crate::QecError;

#[derive(Parser)]
#[command(name = "qec-code", version, about = "Algebraic quantum code inspection")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Code {
        code: String,
        #[command(subcommand)]
        command: CodeCommands,
    },
}

#[derive(Subcommand)]
pub enum CodeCommands {
    Summary,
    Stabilizers,
    Logicals,
    Distance,
}

pub fn run(cli: Cli) -> Result<String, QecError> {
    match cli.command {
        Commands::Code { code, command } if code == "steane" => run_steane(command),
        Commands::Code { .. } => Err(QecError::LogicalBasisNotFound),
    }
}

fn run_steane(command: CodeCommands) -> Result<String, QecError> {
    let steane = Steane::new()?;
    match command {
        CodeCommands::Summary => Ok(format!(
            "n: {}\nstabilizer_rank: {}\nk: {}\n",
            steane.code().n(),
            steane.code().stabilizer_rank(),
            steane.code().num_logical_qubits()
        )),
        CodeCommands::Stabilizers => {
            let mut out = String::new();
            for (idx, stabilizer) in steane.code().stabilizers().iter().enumerate() {
                out.push_str(&format!("s{idx}: {:?}\n", stabilizer.to_symplectic_row()));
            }
            Ok(out)
        }
        CodeCommands::Logicals => {
            let logicals = extract_logical_basis(steane.code())?;
            Ok(format!(
                "k: {}\nlogical_x: {:?}\nlogical_z: {:?}\n",
                logicals.k,
                logicals.logical_x[0].to_symplectic_row(),
                logicals.logical_z[0].to_symplectic_row()
            ))
        }
        CodeCommands::Distance => {
            let distance = compute_distance(steane.code())?;
            Ok(format!(
                "distance: {}\nlogical_class: {:?}\nweight: {}\n",
                distance.distance,
                distance.logical_class,
                distance.witness.weight()
            ))
        }
    }
}
```

Create `qec-code/src/main.rs`:

```rust
use clap::Parser;

fn main() {
    let cli = qec_code::cli::Cli::parse();
    match qec_code::cli::run(cli) {
        Ok(output) => {
            print!("{output}");
        }
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    }
}
```

Update `qec-code/src/lib.rs`:

```rust
pub mod binary;
pub mod cli;
pub mod code;
pub mod codes;
pub mod css;
pub mod distance;
pub mod error;
pub mod logical;
pub mod pauli;

pub use code::StabilizerCode;
pub use error::QecError;
pub use pauli::Pauli;
```

- [ ] **Step 4: Run the CLI smoke tests and the full crate suite**

Run:

```bash
cargo test -p qec-code --test cli -v
cargo test -p qec-code -v
```

Expected:

- first command: PASS with 2 passing CLI smoke tests
- second command: PASS for all `qec-code` tests and binary builds

- [ ] **Step 5: Commit the CLI checkpoint**

```bash
git add qec-code/src/lib.rs qec-code/src/cli.rs qec-code/src/main.rs qec-code/tests/cli.rs
git commit -m "feat: add qec-code steane inspection cli"
```

## Self-Review Checklist

- Spec coverage:
  - workspace crate boundary: Task 1
  - binary symplectic core + Pauli/GF(2): Task 1
  - validated `StabilizerCode`: Task 2
  - CSS convenience + `Steane::new()`: Task 2
  - validated logical basis: Task 3
  - exact small-code distance + witness: Task 3
  - minimal CLI for Steane: Task 4
  - focused tests at algebra / Steane / CLI layers: Tasks 1-4
- Placeholder scan:
  - no placeholder markers remain in executable steps
  - each code-changing step includes concrete code or exact file edits
  - each verification step includes exact commands and expected outcomes
- Type consistency:
  - public names are consistent across tasks: `StabilizerCode`, `CssCode`, `Steane`, `LogicalBasis`, `DistanceResult`
  - package name is `qec-code`, library crate path is `qec_code`, CLI command is `qec-code`

## Notes For The Implementer

- Keep the first implementation correctness-first. The distance search is allowed to be exponential because the only promised built-in example is Steane.
- Do not pull `rstim` or `rilpqec` into the new crate during this plan.
- If the exact `env!("CARGO_BIN_EXE_qec-code")` name is awkward on the local toolchain, rename the binary target explicitly in `qec-code/Cargo.toml` and update the CLI test in the same task before proceeding.
