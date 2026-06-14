# Built-in CSS Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a built-in CSS registry in `qec-code` that exposes canonical raw `Hx`/`Hz` row supports by code id, rejects unknown ids, and makes `Steane::new()` lower from the same registry data.

**Architecture:** Keep the built-in code catalog separate from generic CSS validation. Add one small registry module under `qec-code/src/codes/` that owns canonical row-support data and lookup, then make `Steane::new()` consume that registry and lower into the existing dense `CssCode` API. The public surface stays small: one lookup function, one result type, one dedicated unknown-id error.

**Tech Stack:** Rust 2024, `qec-code`, `thiserror`, integration tests in `qec-code/tests/code.rs`, `cargo test`

---

### Task 1: Lock the public failure mode and registry tests

**Files:**
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/tests/code.rs`

- [ ] **Step 1: Add the dedicated unknown-id error variant**

```rust
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
    #[error("invalid symplectic row width: expected even width, got {width}")]
    InvalidSymplecticRowWidth { width: usize },
    #[error("non-binary matrix entry {value} at row {row}, column {col}")]
    InvalidBinaryEntry { row: usize, col: usize, value: u8 },
    #[error("invalid Pauli width: x has {x_width} bits, z has {z_width}")]
    InvalidPauliWidth { x_width: usize, z_width: usize },
    #[error("non-binary Pauli bit {value} in {which} support at index {index}")]
    InvalidPauliBit {
        which: &'static str,
        index: usize,
        value: u8,
    },
    #[error("stabilizers do not mutually commute")]
    NonCommutingStabilizers,
    #[error("stabilizers are linearly dependent")]
    DependentStabilizers,
    #[error("CSS X/Z checks are not orthogonal")]
    InvalidCssOrthogonality,
    #[error("unknown built-in CSS code: {code_id}")]
    UnknownBuiltInCssCode { code_id: String },
    #[error("logical basis extraction is unsupported for {k} logical qubits")]
    UnsupportedLogicalBasis { k: usize },
    #[error("exhaustive Pauli enumeration is unsupported for {n} qubits on this target")]
    UnsupportedExhaustiveEnumeration { n: usize },
    #[error("logical basis not found")]
    LogicalBasisNotFound,
    #[error("distance witness not found")]
    DistanceWitnessNotFound,
}
```

- [ ] **Step 2: Add the registry tests and canonical-support helper**

```rust
use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::codes::steane::Steane;
use qec_code::css::CssCode;
use qec_code::{Pauli, QecError, StabilizerCode};

fn assert_strictly_increasing_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|pair| pair[0] < pair[1]),
            "row is not canonical: {row:?}"
        );
    }
}

#[test]
fn built_in_css_registry_exposes_steane_checks() {
    let checks = built_in_css_checks("steane").unwrap();

    assert_eq!(checks.code_id, "steane");
    assert_eq!(checks.num_cols, 7);
    assert_eq!(
        checks.hx,
        vec![
            vec![0, 3, 5, 6],
            vec![1, 3, 4, 6],
            vec![2, 4, 5, 6],
        ]
    );
    assert_eq!(checks.hz, checks.hx);
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
}

#[test]
fn built_in_css_registry_rejects_unknown_code_id() {
    assert_eq!(
        built_in_css_checks("unknown"),
        Err(QecError::UnknownBuiltInCssCode {
            code_id: "unknown".to_owned(),
        })
    );
}
```

- [ ] **Step 3: Run the targeted registry test command and confirm it still fails on the missing registry API**

Run:

```bash
cargo test -p qec-code --test code built_in_css_registry_exposes_steane_checks built_in_css_registry_rejects_unknown_code_id
```

Expected: compile fails until the registry module exists, but the new error variant resolves cleanly and the failure is limited to the missing `built_in_css` API.

- [ ] **Step 4: Commit the test-and-error change**

```bash
git add qec-code/src/error.rs qec-code/tests/code.rs
git commit -m "test: add built-in css registry coverage"
```

### Task 2: Add the built-in CSS registry module and export it

**Files:**
- Create: `qec-code/src/codes/built_in_css.rs`
- Modify: `qec-code/src/codes/mod.rs`

- [ ] **Step 1: Add the registry module with canonical Steane row supports**

```rust
use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssChecks {
    pub code_id: &'static str,
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}

const STEANE_ROW_SUPPORTS: &[&[usize]] = &[
    &[0, 3, 5, 6],
    &[1, 3, 4, 6],
    &[2, 4, 5, 6],
];

pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks> {
    match code_id {
        "steane" => {
            let hx = STEANE_ROW_SUPPORTS
                .iter()
                .map(|row| row.to_vec())
                .collect::<Vec<_>>();

            Ok(BuiltInCssChecks {
                code_id: "steane",
                num_cols: 7,
                hx: hx.clone(),
                hz: hx,
            })
        }
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: code_id.to_owned(),
        }),
    }
}
```

- [ ] **Step 2: Export the new registry module from `codes/mod.rs`**

```rust
pub mod built_in_css;
pub mod steane;
```

- [ ] **Step 3: Run the focused registry tests and confirm the new API passes**

Run:

```bash
cargo test -p qec-code --test code built_in_css_registry_exposes_steane_checks built_in_css_registry_rejects_unknown_code_id
```

Expected: both registry tests pass, and the existing `css` / `stabilizer` tests in `qec-code/tests/code.rs` still compile and pass.

- [ ] **Step 4: Commit the registry module**

```bash
git add qec-code/src/codes/built_in_css.rs qec-code/src/codes/mod.rs
git commit -m "feat: add built-in css registry"
```

### Task 3: Rewire `Steane::new()` to lower from the registry

**Files:**
- Modify: `qec-code/src/codes/steane.rs`

- [ ] **Step 1: Replace the inline dense checks with a registry lookup and private lowering helper**

```rust
use crate::code::StabilizerCode;
use crate::codes::built_in_css::built_in_css_checks;
use crate::css::CssCode;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steane {
    code: StabilizerCode,
}

impl Steane {
    pub fn new() -> Result<Self> {
        let checks = built_in_css_checks("steane")?;
        let hx = row_supports_to_dense(checks.num_cols, &checks.hx);
        let hz = row_supports_to_dense(checks.num_cols, &checks.hz);
        let css = CssCode::from_hx_hz(hx, hz)?;

        Ok(Self {
            code: css.code().clone(),
        })
    }

    pub fn code(&self) -> &StabilizerCode {
        &self.code
    }
}

fn row_supports_to_dense(num_cols: usize, rows: &[Vec<usize>]) -> Vec<Vec<u8>> {
    let mut matrix = vec![vec![0; num_cols]; rows.len()];

    for (row_idx, row) in rows.iter().enumerate() {
        for &col in row {
            matrix[row_idx][col] = 1;
        }
    }

    matrix
}
```

- [ ] **Step 2: Run the code tests and the CLI smoke tests**

Run:

```bash
cargo test -p qec-code --test code
cargo test -p qec-code --test cli
```

Expected: all tests pass, including the new registry coverage and the existing Steane summary/distance/logicals checks.

- [ ] **Step 3: Run the full crate test suite**

Run:

```bash
cargo test -p qec-code
```

Expected: full `qec-code` test suite passes with no regressions.

- [ ] **Step 4: Commit the Steane refactor**

```bash
git add qec-code/src/codes/steane.rs
git commit -m "refactor: lower steane from built-in css registry"
```

### Task 4: Final verification and handoff

**Files:**
- No code changes; verify the working tree and test status

- [ ] **Step 1: Confirm the final state is clean except for unrelated user work**

Run:

```bash
git status --short
```

Expected: only unrelated pre-existing changes remain, if any.

- [ ] **Step 2: Re-run the issue-specific acceptance command**

Run:

```bash
cargo test -p qec-code --test code built_in_css_registry_exposes_steane_checks built_in_css_registry_rejects_unknown_code_id
```

Expected: pass.

- [ ] **Step 3: Report completion with the exact files that changed**

Mention:

- `qec-code/src/error.rs`
- `qec-code/src/codes/mod.rs`
- `qec-code/src/codes/built_in_css.rs`
- `qec-code/src/codes/steane.rs`
- `qec-code/tests/code.rs`
