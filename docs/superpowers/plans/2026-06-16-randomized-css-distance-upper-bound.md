# Randomized CSS Distance Upper Bound Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable `qec-code` API and JSON CLI command for seeded randomized CSS distance upper bounds that can never be mistaken for exact distance.

**Architecture:** Add a new `qec-code/src/distance_bound.rs` module beside the existing exact `distance.rs` module. Keep exact `compute_distance` untouched, parse CSS sparse-row JSON through `css.rs`, and route the new CLI command through `qec-code/src/cli.rs`.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, `clap`, existing `qec-code` GF(2)/CSS/stabilizer utilities, Cargo integration tests.

---

## File Structure

- Create `qec-code/src/distance_bound.rs`
  - owns randomized upper-bound options, result/provenance/witness JSON types, validation helpers, deterministic PRNG, and search implementation
  - depends on `CssCode`, `StabilizerCode`, `Pauli`, `LogicalClass`, and `binary::try_in_row_span`
- Modify `qec-code/src/lib.rs`
  - exposes `pub mod distance_bound`
- Modify `qec-code/src/distance.rs`
  - derives `Serialize`/`Deserialize` for `LogicalClass` so result JSON can emit `x_like`, `z_like`, or `mixed`
- Modify `qec-code/src/css.rs`
  - adds sparse-row JSON parsing and dense conversion helpers
- Modify `qec-code/src/error.rs`
  - adds explicit errors for bound options, CSS matrix JSON, JSON-only CLI output, and no witness found
- Modify `qec-code/src/cli.rs`
  - adds `code css-distance randomized-upper-bound`
  - supports `--code-id` and `--hx`/`--hz`
  - emits JSON on success and returns errors before stdout on failure
- Create `qec-code/tests/distance_bound.rs`
  - tests result validation, option validation, reproducibility, pinned Steane equality, and no exact-labeled randomized result
- Create `qec-code/tests/css_matrix_input.rs`
  - tests sparse-row JSON parsing and dense conversion
- Modify `qec-code/tests/cli.rs`
  - tests new CLI built-in/file modes and invalid-option behavior

---

### Task 1: Add Bound Result Types And Validation

**Files:**
- Create: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/distance.rs`
- Modify: `qec-code/src/error.rs`
- Test: `qec-code/tests/distance_bound.rs`

- [ ] **Step 1: Write failing result-shape and validation tests**

Create `qec-code/tests/distance_bound.rs` with:

```rust
use qec_code::distance::LogicalClass;
use qec_code::distance_bound::{
    BoundType, BoundValidationContext, DistanceBoundMethod, DistanceBoundProvenance,
    DistanceBoundResult, DistanceBoundStatus, DistanceBoundWitness,
    RandomizedUpperBoundOptions, validate_randomized_upper_bound_result,
};
use qec_code::{Pauli, QecError, StabilizerCode};

fn trivial_one_qubit_code() -> StabilizerCode {
    StabilizerCode::from_stabilizers(1, vec![]).unwrap()
}

fn one_qubit_x_witness() -> DistanceBoundWitness {
    let pauli = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();
    DistanceBoundWitness::from_pauli(&pauli)
}

fn valid_result() -> DistanceBoundResult {
    DistanceBoundResult::completed(
        1,
        LogicalClass::XLike,
        one_qubit_x_witness(),
        RandomizedUpperBoundOptions {
            iterations: 10,
            restarts: 1,
            seed: 7,
            target_weight: None,
        },
    )
}

#[test]
fn completed_bound_result_serializes_with_upper_bound_contract() {
    let result = valid_result();

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 1);
    assert_eq!(json["logical_class"], "x_like");
    assert_eq!(json["witness"]["x"], serde_json::json!([1]));
    assert_eq!(json["witness"]["z"], serde_json::json!([0]));
    assert_eq!(json["witness"]["weight"], 1);
    assert_eq!(json["options"]["iterations"], 10);
    assert_eq!(json["options"]["restarts"], 1);
    assert_eq!(json["options"]["seed"], 7);
    assert_eq!(json["options"]["target_weight"], serde_json::Value::Null);
    assert_eq!(json["provenance"]["tool"], "qec-code");
    assert_eq!(json["provenance"]["method_revision"], 1);
}

#[test]
fn randomized_upper_bound_options_reject_zero_iterations_restarts_and_target() {
    assert_eq!(
        RandomizedUpperBoundOptions {
            iterations: 0,
            restarts: 1,
            seed: 7,
            target_weight: None,
        }
        .validate(),
        Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        })
    );

    assert_eq!(
        RandomizedUpperBoundOptions {
            iterations: 1,
            restarts: 0,
            seed: 7,
            target_weight: None,
        }
        .validate(),
        Err(QecError::InvalidDistanceBoundOption {
            option: "restarts",
            reason: "must be greater than zero".to_owned(),
        })
    );

    assert_eq!(
        RandomizedUpperBoundOptions {
            iterations: 1,
            restarts: 1,
            seed: 7,
            target_weight: Some(0),
        }
        .validate(),
        Err(QecError::InvalidDistanceBoundOption {
            option: "target_weight",
            reason: "must be greater than zero when provided".to_owned(),
        })
    );
}

#[test]
fn validator_accepts_valid_upper_bound_result() {
    let code = trivial_one_qubit_code();
    let result = valid_result();

    validate_randomized_upper_bound_result(
        &result,
        BoundValidationContext {
            code: &code,
            known_exact_distance: Some(1),
        },
    )
    .unwrap();
}

#[test]
fn validator_rejects_exact_labeled_randomized_result() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.bound_type = BoundType::Exact;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(1),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "randomized-upper-bound results must use bound_type upper".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_wrong_method() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.method = DistanceBoundMethod::Exact;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(1),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "distance bound method must be randomized-upper-bound".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_mocked_underestimate_against_known_exact_distance() {
    let code = trivial_one_qubit_code();
    let result = valid_result();

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(2),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "upper_bound 1 is below known exact distance 2".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_witness_weight_mismatch() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.upper_bound = 2;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "upper_bound must equal witness weight".to_owned(),
        ))
    );
}

#[test]
fn validator_rejects_stabilizer_span_witness() {
    let x0 = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();
    let code = StabilizerCode::from_stabilizers(1, vec![x0]).unwrap();
    let result = valid_result();

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "witness lies in stabilizer span".to_owned(),
        ))
    );
}

#[test]
fn provenance_uses_current_package_version_and_method_revision() {
    let provenance = DistanceBoundProvenance::current();

    assert_eq!(provenance.tool, "qec-code");
    assert_eq!(provenance.tool_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(provenance.method_revision, 1);
}

#[test]
fn completed_status_is_serialized_as_completed() {
    assert_eq!(
        serde_json::to_value(DistanceBoundStatus::Completed).unwrap(),
        "completed"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
cargo test -p qec-code --test distance_bound
```

Expected: FAIL because `qec_code::distance_bound` does not exist.

- [ ] **Step 3: Add distance-bound errors**

Modify `qec-code/src/error.rs` by adding these variants to `QecError` after `DistanceWitnessNotFound`:

```rust
    #[error("invalid distance bound option {option}: {reason}")]
    InvalidDistanceBoundOption {
        option: &'static str,
        reason: String,
    },
    #[error("randomized upper-bound witness not found")]
    RandomizedUpperBoundWitnessNotFound,
    #[error("distance bound validation failed: {0}")]
    DistanceBoundValidationFailed(String),
```

- [ ] **Step 4: Make logical class serializable**

Modify the top of `qec-code/src/distance.rs`:

```rust
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use crate::Pauli;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalClass {
    XLike,
    ZLike,
    Mixed,
}
```

Leave the rest of `distance.rs` unchanged.

- [ ] **Step 5: Expose the new module**

Modify `qec-code/src/lib.rs` to add `distance_bound` beside `distance`:

```rust
pub mod binary;
pub mod cli;
pub mod code;
pub mod codes;
pub mod css;
pub mod distance;
pub mod distance_bound;
pub mod error;
#[cfg(feature = "distance-ilp-highs")]
pub mod distance_ilp;
mod gf2;
pub mod logical;
pub mod pauli;
mod symplectic;

pub use code::StabilizerCode;
pub use error::QecError;
pub use pauli::Pauli;
```

- [ ] **Step 6: Add `qec-code/src/distance_bound.rs` with result and validation code**

Create `qec-code/src/distance_bound.rs`:

```rust
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::distance::LogicalClass;
use crate::error::{QecError, Result};
use crate::Pauli;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistanceBoundMethod {
    RandomizedUpperBound,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoundType {
    Upper,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceBoundStatus {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizedUpperBoundOptions {
    pub iterations: usize,
    pub restarts: usize,
    pub seed: u64,
    pub target_weight: Option<usize>,
}

impl RandomizedUpperBoundOptions {
    pub fn validate(&self) -> Result<()> {
        if self.iterations == 0 {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "iterations",
                reason: "must be greater than zero".to_owned(),
            });
        }
        if self.restarts == 0 {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "restarts",
                reason: "must be greater than zero".to_owned(),
            });
        }
        if self.target_weight == Some(0) {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "target_weight",
                reason: "must be greater than zero when provided".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundWitness {
    pub x: Vec<u8>,
    pub z: Vec<u8>,
    pub weight: usize,
}

impl DistanceBoundWitness {
    pub fn from_pauli(pauli: &Pauli) -> Self {
        Self {
            x: pauli.x_bits().to_vec(),
            z: pauli.z_bits().to_vec(),
            weight: pauli.weight(),
        }
    }

    pub fn to_pauli(&self) -> Result<Pauli> {
        Pauli::from_xz_bits(self.x.clone(), self.z.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundProvenance {
    pub tool: String,
    pub tool_version: String,
    pub method_revision: u32,
}

impl DistanceBoundProvenance {
    pub fn current() -> Self {
        Self {
            tool: "qec-code".to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            method_revision: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundResult {
    pub status: DistanceBoundStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: RandomizedUpperBoundOptions,
    pub provenance: DistanceBoundProvenance,
}

impl DistanceBoundResult {
    pub fn completed(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomizedUpperBoundOptions,
    ) -> Self {
        Self {
            status: DistanceBoundStatus::Completed,
            method: DistanceBoundMethod::RandomizedUpperBound,
            bound_type: BoundType::Upper,
            upper_bound,
            logical_class,
            witness,
            options,
            provenance: DistanceBoundProvenance::current(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundValidationContext<'a> {
    pub code: &'a StabilizerCode,
    pub known_exact_distance: Option<usize>,
}

pub fn validate_randomized_upper_bound_result(
    result: &DistanceBoundResult,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    if result.method != DistanceBoundMethod::RandomizedUpperBound {
        return Err(QecError::DistanceBoundValidationFailed(
            "distance bound method must be randomized-upper-bound".to_owned(),
        ));
    }
    if result.bound_type != BoundType::Upper {
        return Err(QecError::DistanceBoundValidationFailed(
            "randomized-upper-bound results must use bound_type upper".to_owned(),
        ));
    }
    if result.upper_bound == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "completed upper_bound must be positive".to_owned(),
        ));
    }
    if result.upper_bound != result.witness.weight {
        return Err(QecError::DistanceBoundValidationFailed(
            "upper_bound must equal witness weight".to_owned(),
        ));
    }

    let witness = result.witness.to_pauli()?;
    validate_witness_against_code(context.code, &witness)?;

    if let Some(known_exact_distance) = context.known_exact_distance {
        if result.upper_bound < known_exact_distance {
            return Err(QecError::DistanceBoundValidationFailed(format!(
                "upper_bound {} is below known exact distance {}",
                result.upper_bound, known_exact_distance
            )));
        }
    }

    Ok(())
}

fn validate_witness_against_code(code: &StabilizerCode, witness: &Pauli) -> Result<()> {
    if witness.weight() == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ));
    }
    for stabilizer in code.stabilizers() {
        if !witness.try_commutes_with(stabilizer)? {
            return Err(QecError::DistanceBoundValidationFailed(
                "witness does not commute with stabilizers".to_owned(),
            ));
        }
    }
    if try_in_row_span(&code.stabilizer_rows(), &witness.to_symplectic_row())? {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness lies in stabilizer span".to_owned(),
        ));
    }
    Ok(())
}
```

- [ ] **Step 7: Run tests to verify Task 1 passes**

Run:

```sh
cargo test -p qec-code --test distance_bound
```

Expected: PASS.

- [ ] **Step 8: Commit Task 1**

Run:

```sh
git add qec-code/src/distance_bound.rs qec-code/src/lib.rs qec-code/src/distance.rs qec-code/src/error.rs qec-code/tests/distance_bound.rs
git commit -m "feat: add distance bound result validation"
```

---

### Task 2: Add Sparse-Rows CSS Matrix JSON Input

**Files:**
- Modify: `qec-code/src/css.rs`
- Modify: `qec-code/src/error.rs`
- Test: `qec-code/tests/css_matrix_input.rs`

- [ ] **Step 1: Write failing sparse-rows input tests**

Create `qec-code/tests/css_matrix_input.rs`:

```rust
use qec_code::QecError;
use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};

#[test]
fn sparse_rows_json_parses_and_converts_to_dense_rows() {
    let matrix = sparse_rows_matrix_from_json_str(
        r#"{"format":"sparse_rows","num_cols":5,"rows":[[0,3],[1,4],[]]}"#,
    )
    .unwrap();

    assert_eq!(matrix.num_cols(), 5);
    assert_eq!(
        matrix.to_dense_rows(),
        vec![
            vec![1, 0, 0, 1, 0],
            vec![0, 1, 0, 0, 1],
            vec![0, 0, 0, 0, 0],
        ]
    );
}

#[test]
fn sparse_rows_json_rejects_missing_format() {
    assert_eq!(
        sparse_rows_matrix_from_json_str(r#"{"num_cols":3,"rows":[[0]]}"#),
        Err(QecError::MissingCssMatrixFormat)
    );
}

#[test]
fn sparse_rows_json_rejects_dense_matrix_shape_as_unsupported_format() {
    assert_eq!(
        sparse_rows_matrix_from_json_str(r#"{"format":"dense","rows":[[1,0,1]]}"#),
        Err(QecError::UnsupportedCssMatrixFormat {
            format: "dense".to_owned(),
        })
    );
}

#[test]
fn sparse_rows_json_rejects_malformed_json() {
    let err = sparse_rows_matrix_from_json_str(r#"{"format":"sparse_rows","num_cols":"bad","rows":[]}"#)
        .unwrap_err();

    assert!(
        err.to_string().contains("invalid CSS matrix JSON"),
        "error was: {err}"
    );
}

#[test]
fn sparse_rows_json_reuses_sparse_row_validation() {
    assert_eq!(
        sparse_rows_matrix_from_json_str(
            r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,3]]}"#,
        ),
        Err(QecError::SparseRowSupportOutOfRange {
            row: 0,
            support: 3,
            num_cols: 3,
        })
    );
}

#[test]
fn sparse_rows_matrix_dense_conversion_preserves_empty_rows() {
    let matrix = SparseRowsMatrix::new(3, vec![vec![], vec![0, 2]]).unwrap();

    assert_eq!(
        matrix.to_dense_rows(),
        vec![vec![0, 0, 0], vec![1, 0, 1]]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
cargo test -p qec-code --test css_matrix_input
```

Expected: FAIL because `sparse_rows_matrix_from_json_str`, `num_cols`, and `to_dense_rows` do not exist.

- [ ] **Step 3: Add CSS matrix JSON errors**

Modify `qec-code/src/error.rs` by adding these variants near the sparse-row errors:

```rust
    #[error("missing CSS matrix format")]
    MissingCssMatrixFormat,
    #[error("unsupported CSS matrix format: {format}")]
    UnsupportedCssMatrixFormat { format: String },
    #[error("invalid CSS matrix JSON: {0}")]
    InvalidCssMatrixJson(String),
```

- [ ] **Step 4: Add sparse-row parsing and dense conversion helpers**

Modify `qec-code/src/css.rs` imports:

```rust
use crate::Pauli;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use serde::{Deserialize, Serialize};
```

Add this implementation below `impl SparseRowsMatrix`'s existing `to_json_string` method:

```rust
    pub fn num_cols(&self) -> usize {
        self.num_cols
    }

    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }

    pub fn to_dense_rows(&self) -> Vec<Vec<u8>> {
        self.rows
            .iter()
            .map(|row| {
                let mut dense = vec![0; self.num_cols];
                for &support in row {
                    dense[support] = 1;
                }
                dense
            })
            .collect()
    }
```

Add these types and function below the `SparseRowsMatrix` impl:

```rust
#[derive(Debug, Deserialize)]
struct SparseRowsMatrixJson {
    format: String,
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

pub fn sparse_rows_matrix_from_json_str(input: &str) -> Result<SparseRowsMatrix> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| QecError::InvalidCssMatrixJson(err.to_string()))?;

    let format = value
        .get("format")
        .and_then(serde_json::Value::as_str)
        .ok_or(QecError::MissingCssMatrixFormat)?;

    if format != "sparse_rows" {
        return Err(QecError::UnsupportedCssMatrixFormat {
            format: format.to_owned(),
        });
    }

    let parsed: SparseRowsMatrixJson = serde_json::from_value(value)
        .map_err(|err| QecError::InvalidCssMatrixJson(err.to_string()))?;

    SparseRowsMatrix::new(parsed.num_cols, parsed.rows)
}
```

- [ ] **Step 5: Run tests to verify Task 2 passes**

Run:

```sh
cargo test -p qec-code --test css_matrix_input --test css_export --test code
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```sh
git add qec-code/src/css.rs qec-code/src/error.rs qec-code/tests/css_matrix_input.rs
git commit -m "feat: parse CSS sparse rows matrices"
```

---

### Task 3: Implement Seeded Randomized Upper-Bound Search

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Test: `qec-code/tests/distance_bound.rs`

- [ ] **Step 1: Add failing API search tests**

Append these tests to `qec-code/tests/distance_bound.rs`:

```rust
use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance_bound::randomized_css_upper_bound;

fn css_from_sparse_rows(num_cols: usize, hx: Vec<Vec<usize>>, hz: Vec<Vec<usize>>) -> CssCode {
    let hx = SparseRowsMatrix::new(num_cols, hx).unwrap().to_dense_rows();
    let hz = SparseRowsMatrix::new(num_cols, hz).unwrap().to_dense_rows();
    CssCode::from_hx_hz(hx, hz).unwrap()
}

#[test]
fn randomized_upper_bound_reproducible_for_same_seed() {
    let checks = built_in_css_checks("steane").unwrap();
    let css = css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz);
    let options = RandomizedUpperBoundOptions {
        iterations: 200,
        restarts: 2,
        seed: 7,
        target_weight: None,
    };

    let first = randomized_css_upper_bound(&css, options.clone()).unwrap();
    let second = randomized_css_upper_bound(&css, options).unwrap();

    assert_eq!(first.upper_bound, second.upper_bound);
    assert_eq!(first.witness, second.witness);
}

#[test]
fn randomized_upper_bound_finds_steane_distance_under_pinned_options() {
    let checks = built_in_css_checks("steane").unwrap();
    let css = css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz);
    let options = RandomizedUpperBoundOptions {
        iterations: 500,
        restarts: 4,
        seed: 7,
        target_weight: Some(3),
    };

    let result = randomized_css_upper_bound(&css, options).unwrap();

    assert_eq!(result.upper_bound, 3);
    validate_randomized_upper_bound_result(
        &result,
        BoundValidationContext {
            code: css.code(),
            known_exact_distance: Some(3),
        },
    )
    .unwrap();
}

#[test]
fn randomized_upper_bound_finds_repetition_css_distance_under_pinned_options() {
    let css = css_from_sparse_rows(
        5,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
        vec![],
    );
    let options = RandomizedUpperBoundOptions {
        iterations: 200,
        restarts: 2,
        seed: 11,
        target_weight: Some(1),
    };

    let result = randomized_css_upper_bound(&css, options).unwrap();

    assert_eq!(result.upper_bound, 1);
    validate_randomized_upper_bound_result(
        &result,
        BoundValidationContext {
            code: css.code(),
            known_exact_distance: Some(1),
        },
    )
    .unwrap();
}

#[test]
fn randomized_upper_bound_rejects_invalid_options_before_search() {
    let css = css_from_sparse_rows(1, vec![], vec![]);

    assert_eq!(
        randomized_css_upper_bound(
            &css,
            RandomizedUpperBoundOptions {
                iterations: 0,
                restarts: 1,
                seed: 7,
                target_weight: None,
            },
        ),
        Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        })
    );
}

#[test]
fn randomized_upper_bound_rejects_zero_logical_qubit_code() {
    let css = css_from_sparse_rows(1, vec![vec![0]], vec![]);

    assert_eq!(
        randomized_css_upper_bound(
            &css,
            RandomizedUpperBoundOptions {
                iterations: 10,
                restarts: 1,
                seed: 7,
                target_weight: None,
            },
        ),
        Err(QecError::DistanceWitnessNotFound)
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
cargo test -p qec-code --test distance_bound
```

Expected: FAIL because `randomized_css_upper_bound` does not exist.

- [ ] **Step 3: Add search helpers and public API**

Append this code to `qec-code/src/distance_bound.rs`:

```rust
use crate::css::CssCode;

pub fn randomized_css_upper_bound(
    css: &CssCode,
    options: RandomizedUpperBoundOptions,
) -> Result<DistanceBoundResult> {
    options.validate()?;

    let code = css.code();
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    let basis = code.canonical_logical_basis()?;
    let logical_rows = basis
        .logical_x
        .iter()
        .chain(&basis.logical_z)
        .map(Pauli::to_symplectic_row)
        .collect::<Vec<_>>();
    let stabilizer_rows = code.stabilizer_rows();
    let mut rng = SplitMix64::new(options.seed);
    let mut best: Option<Pauli> = None;

    for _restart in 0..options.restarts {
        for _iteration in 0..options.iterations {
            let candidate_row =
                sample_candidate_row(&logical_rows, &stabilizer_rows, code.n() * 2, &mut rng);
            let candidate = Pauli::from_symplectic_row(candidate_row)?;

            if validate_witness_against_code(code, &candidate).is_err() {
                continue;
            }

            let replace = best
                .as_ref()
                .map(|current| candidate.weight() < current.weight())
                .unwrap_or(true);
            if replace {
                best = Some(candidate);
            }

            if let (Some(target), Some(current)) = (options.target_weight, best.as_ref()) {
                if current.weight() <= target {
                    return result_from_witness(code, current.clone(), options);
                }
            }
        }
    }

    let witness = best.ok_or(QecError::RandomizedUpperBoundWitnessNotFound)?;
    result_from_witness(code, witness, options)
}

fn result_from_witness(
    code: &StabilizerCode,
    witness: Pauli,
    options: RandomizedUpperBoundOptions,
) -> Result<DistanceBoundResult> {
    validate_witness_against_code(code, &witness)?;
    let result = DistanceBoundResult::completed(
        witness.weight(),
        classify_logical_for_bound(&witness),
        DistanceBoundWitness::from_pauli(&witness),
        options,
    );
    validate_randomized_upper_bound_result(
        &result,
        BoundValidationContext {
            code,
            known_exact_distance: None,
        },
    )?;
    Ok(result)
}

fn sample_candidate_row(
    logical_rows: &[Vec<u8>],
    stabilizer_rows: &[Vec<u8>],
    width: usize,
    rng: &mut SplitMix64,
) -> Vec<u8> {
    let mut row = vec![0; width];
    let mut selected_logical = false;

    for logical in logical_rows {
        if rng.next_bool() {
            xor_into(&mut row, logical);
            selected_logical = true;
        }
    }

    if !selected_logical {
        let index = rng.next_usize(logical_rows.len());
        xor_into(&mut row, &logical_rows[index]);
    }

    for stabilizer in stabilizer_rows {
        if rng.next_bool() {
            xor_into(&mut row, stabilizer);
        }
    }

    row
}

fn xor_into(target: &mut [u8], row: &[u8]) {
    for (target_bit, row_bit) in target.iter_mut().zip(row) {
        *target_bit ^= *row_bit;
    }
}

fn classify_logical_for_bound(pauli: &Pauli) -> LogicalClass {
    let has_x = pauli.x_bits().contains(&1);
    let has_z = pauli.z_bits().contains(&1);

    match (has_x, has_z) {
        (true, false) => LogicalClass::XLike,
        (false, true) => LogicalClass::ZLike,
        (true, true) => LogicalClass::Mixed,
        (false, false) => unreachable!("validated witnesses are non-identity"),
    }
}

#[derive(Debug, Clone)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        (self.next_u64() as usize) % upper
    }
}
```

- [ ] **Step 4: Run tests to verify Task 3 passes**

Run:

```sh
cargo test -p qec-code --test distance_bound
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

Run:

```sh
git add qec-code/src/distance_bound.rs qec-code/tests/distance_bound.rs
git commit -m "feat: add randomized CSS upper-bound search"
```

---

### Task 4: Add CLI Command And JSON Output

**Files:**
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/src/error.rs`
- Test: `qec-code/tests/cli.rs`

- [ ] **Step 1: Add failing CLI tests**

Append these tests to `qec-code/tests/cli.rs`:

```rust
#[test]
fn css_distance_randomized_upper_bound_code_id_outputs_json() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "500",
        "--restarts",
        "4",
        "--seed",
        "7",
        "--target-weight",
        "3",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 3);
    assert_eq!(json["options"]["seed"], 7);
}

#[test]
fn css_distance_randomized_upper_bound_hx_hz_files_output_json() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "randomized-upper-bound",
            "--hx",
        ])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .args([
            "--iterations",
            "500",
            "--restarts",
            "4",
            "--seed",
            "7",
            "--target-weight",
            "3",
            "--json",
        ])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 3);
}

#[test]
fn css_distance_randomized_upper_bound_requires_json_flag() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "10",
        "--seed",
        "7",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("JSON output is required for code css-distance randomized-upper-bound"),
        "stderr was: {stderr}"
    );
}

#[test]
fn css_distance_randomized_upper_bound_rejects_zero_iterations_without_stdout() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "randomized-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "0",
        "--seed",
        "7",
        "--json",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid distance bound option iterations"),
        "stderr was: {stderr}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
cargo test -p qec-code --test cli css_distance_randomized_upper_bound
```

Expected: FAIL because the `css-distance` command does not exist.

- [ ] **Step 3: Add CLI error variants**

Modify `qec-code/src/error.rs` by adding these variants near the CLI/input errors:

```rust
    #[error("JSON output is required for {command}")]
    JsonOutputRequired { command: &'static str },
    #[error("invalid CSS distance input: {0}")]
    InvalidCssDistanceInput(String),
    #[error("failed to read CSS matrix {path}: {source}")]
    CssMatrixReadFailed { path: String, source: String },
```

- [ ] **Step 4: Add CLI imports and command types**

Modify the imports at the top of `qec-code/src/cli.rs`:

```rust
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::QecError;
use crate::codes::built_in_css::built_in_css_checks;
use crate::codes::steane::Steane;
use crate::css::{CssCode, SparseRowsMatrix, sparse_rows_matrix_from_json_str};
use crate::distance::compute_distance;
use crate::distance_bound::{RandomizedUpperBoundOptions, randomized_css_upper_bound};
```

Modify `CodeCommands`:

```rust
#[derive(Debug, Subcommand)]
pub enum CodeCommands {
    Steane {
        #[command(subcommand)]
        command: SteaneCommands,
    },
    Css {
        code_id: String,
        matrix: CssMatrixKind,
    },
    CssDistance {
        #[command(subcommand)]
        command: CssDistanceCommands,
    },
}
```

Add these CLI structs after `CssMatrixKind`:

```rust
#[derive(Debug, Subcommand)]
pub enum CssDistanceCommands {
    RandomizedUpperBound(RandomizedUpperBoundCli),
}

#[derive(Debug, Args)]
pub struct RandomizedUpperBoundCli {
    #[arg(long)]
    pub code_id: Option<String>,
    #[arg(long)]
    pub hx: Option<PathBuf>,
    #[arg(long)]
    pub hz: Option<PathBuf>,
    #[arg(long)]
    pub iterations: usize,
    #[arg(long, default_value_t = 1)]
    pub restarts: usize,
    #[arg(long)]
    pub seed: u64,
    #[arg(long)]
    pub target_weight: Option<usize>,
    #[arg(long)]
    pub json: bool,
}
```

- [ ] **Step 5: Route the new command**

Modify `run_code`:

```rust
fn run_code(command: CodeCommands) -> Result<String, QecError> {
    match command {
        CodeCommands::Steane { command } => run_steane(command),
        CodeCommands::Css { code_id, matrix } => run_css(&code_id, matrix),
        CodeCommands::CssDistance { command } => run_css_distance(command),
    }
}
```

Add these functions below `run_css`:

```rust
fn run_css_distance(command: CssDistanceCommands) -> Result<String, QecError> {
    match command {
        CssDistanceCommands::RandomizedUpperBound(args) => {
            run_css_randomized_upper_bound(args)
        }
    }
}

fn run_css_randomized_upper_bound(args: RandomizedUpperBoundCli) -> Result<String, QecError> {
    if !args.json {
        return Err(QecError::JsonOutputRequired {
            command: "code css-distance randomized-upper-bound",
        });
    }

    let css = load_css_input(args.code_id.as_deref(), args.hx.as_ref(), args.hz.as_ref())?;
    let result = randomized_css_upper_bound(
        &css,
        RandomizedUpperBoundOptions {
            iterations: args.iterations,
            restarts: args.restarts,
            seed: args.seed,
            target_weight: args.target_weight,
        },
    )?;

    serde_json::to_string(&result)
        .map_err(|err| QecError::DistanceBoundValidationFailed(err.to_string()))
}

fn load_css_input(
    code_id: Option<&str>,
    hx_path: Option<&PathBuf>,
    hz_path: Option<&PathBuf>,
) -> Result<CssCode, QecError> {
    match (code_id, hx_path, hz_path) {
        (Some(code_id), None, None) => css_from_built_in_code_id(code_id),
        (None, Some(hx_path), Some(hz_path)) => css_from_matrix_files(hx_path, hz_path),
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "use either --code-id or --hx/--hz, not both".to_owned(),
        )),
        (None, Some(_), None) => Err(QecError::InvalidCssDistanceInput(
            "--hz is required when --hx is provided".to_owned(),
        )),
        (None, None, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "--hx is required when --hz is provided".to_owned(),
        )),
        (None, None, None) => Err(QecError::InvalidCssDistanceInput(
            "provide --code-id or --hx/--hz".to_owned(),
        )),
    }
}

fn css_from_built_in_code_id(code_id: &str) -> Result<CssCode, QecError> {
    let checks = built_in_css_checks(code_id)?;
    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx)?.to_dense_rows();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz)?.to_dense_rows();
    CssCode::from_hx_hz(hx, hz)
}

fn css_from_matrix_files(hx_path: &PathBuf, hz_path: &PathBuf) -> Result<CssCode, QecError> {
    let hx_text = read_css_matrix_file(hx_path)?;
    let hz_text = read_css_matrix_file(hz_path)?;
    let hx = sparse_rows_matrix_from_json_str(&hx_text)?;
    let hz = sparse_rows_matrix_from_json_str(&hz_text)?;

    if hx.num_cols() != hz.num_cols() {
        return Err(QecError::RowWidthMismatch {
            expected: hx.num_cols(),
            actual: hz.num_cols(),
        });
    }

    CssCode::from_hx_hz(hx.to_dense_rows(), hz.to_dense_rows())
}

fn read_css_matrix_file(path: &PathBuf) -> Result<String, QecError> {
    std::fs::read_to_string(path).map_err(|err| QecError::CssMatrixReadFailed {
        path: path.display().to_string(),
        source: err.to_string(),
    })
}
```

- [ ] **Step 6: Run CLI tests to verify Task 4 passes**

Run:

```sh
cargo test -p qec-code --test cli css_distance_randomized_upper_bound
```

Expected: PASS.

- [ ] **Step 7: Commit Task 4**

Run:

```sh
git add qec-code/src/cli.rs qec-code/src/error.rs qec-code/tests/cli.rs
git commit -m "feat: expose randomized CSS upper bounds in CLI"
```

---

### Task 5: Final Regression, Negative Controls, And Issue Evidence

**Files:**
- Modify: `qec-code/tests/distance_bound.rs`
- Modify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Add final negative-control tests**

Append this test to `qec-code/tests/distance_bound.rs`:

```rust
#[test]
fn validator_rejects_identity_witness_even_with_positive_upper_bound() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.witness = DistanceBoundWitness {
        x: vec![0],
        z: vec![0],
        weight: 1,
    };
    result.upper_bound = 1;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: None,
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ))
    );
}
```

Append this test to `qec-code/tests/cli.rs`:

```rust
#[test]
fn css_distance_randomized_upper_bound_rejects_code_id_and_file_input_together() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "randomized-upper-bound",
            "--code-id",
            "steane",
            "--hx",
        ])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .args(["--iterations", "10", "--seed", "7", "--json"])
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("use either --code-id or --hx/--hz, not both"),
        "stderr was: {stderr}"
    );
}
```

- [ ] **Step 2: Run focused test set**

Run:

```sh
cargo test -p qec-code --test distance_bound --test css_matrix_input --test cli
```

Expected: PASS.

- [ ] **Step 3: Run full `qec-code` tests**

Run:

```sh
cargo test -p qec-code
```

Expected: PASS.

- [ ] **Step 4: Run feature-gated exact-distance regression**

Run:

```sh
cargo test -p qec-code --features distance-ilp-highs --test logical_distance
```

Expected: PASS. If the local machine lacks the HiGHS backend or the backend fails to build, record the exact error and rerun `cargo test -p qec-code --test logical_distance` without the feature before finishing.

- [ ] **Step 5: Capture CLI evidence for issue 76**

Run:

```sh
cargo run -p qec-code -- code css-distance randomized-upper-bound --code-id steane --iterations 500 --restarts 4 --seed 7 --target-weight 3 --json
```

Expected: PASS and stdout contains:

```json
{"status":"completed","method":"randomized-upper-bound","bound_type":"upper","upper_bound":3
```

- [ ] **Step 6: Commit final test and verification polish**

Run:

```sh
git add qec-code/tests/distance_bound.rs qec-code/tests/cli.rs
git commit -m "test: cover randomized CSS bound controls"
```

---

## Completion Checklist

- [ ] `cargo test -p qec-code --test distance_bound --test css_matrix_input --test cli` passes.
- [ ] `cargo test -p qec-code` passes.
- [ ] `cargo test -p qec-code --features distance-ilp-highs --test logical_distance` passes or the backend-specific blocker is recorded with a non-feature fallback result.
- [ ] CLI evidence for `--code-id steane` shows `method: "randomized-upper-bound"`, `bound_type: "upper"`, and `upper_bound: 3`.
- [ ] `git status --short` shows only intentional changes.
