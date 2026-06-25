# Issue 229 CSS Upper-Bound Ladder Verifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable, method-aware CSS upper-bound ladder verifier for the issue-225 manifest.

**Architecture:** Keep the verifier in `qec-code/src/distance_bound.rs` next to the result types and existing randomized validator. Add a method-aware validation wrapper that reuses the existing witness validation checks, then layer the issue-225 manifest upper-bound target check on top.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, existing `qec-code` CSS/stabilizer/Pauli utilities, Cargo integration tests.

## Global Constraints

- Do not implement the random-window search.
- Do not change the CLI.
- Do not make the existing `randomized-upper-bound` command fail in normal use.
- The verifier must be parameterized by expected method instead of hard-coding `randomized-upper-bound`.
- The verifier must check `method`, `bound_type == upper`, `upper_bound <= expected_upper_bound`, `upper_bound == witness.weight`, and witness validity.
- Error messages from ladder checks must name the case ID.

---

## File Structure

- Modify `qec-code/src/distance_bound.rs`
  - Add the `RandomWindowUpperBound` method label.
  - Add a public method-label helper for clear errors.
  - Add `Issue225LadderCase`.
  - Add a method-aware validation context and `validate_distance_bound_result`.
  - Rewire `validate_randomized_upper_bound_result` through the shared validator.
  - Add `verify_issue_225_ladder_case`.
- Modify `qec-code/tests/distance_bound.rs`
  - Add helpers for loading the issue-225 manifest row and building exact/loose `surface_rotated_d5` witness results.
  - Add the three issue-required verifier tests.

---

### Task 1: Method-Aware Ladder Verifier

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/tests/distance_bound.rs`

**Interfaces:**
- Consumes: `DistanceBoundResult`, `CssCode`, `DistanceBoundMethod`, existing `BoundValidationContext`, existing `validate_randomized_upper_bound_result` witness semantics, and the #228 manifest JSON.
- Produces:
  - `DistanceBoundMethod::RandomWindowUpperBound`
  - `DistanceBoundMethod::label(&self) -> &'static str`
  - `pub struct Issue225LadderCase`
  - `pub struct MethodAwareBoundValidationContext<'a>`
  - `pub fn validate_distance_bound_result(result: &DistanceBoundResult, context: MethodAwareBoundValidationContext<'_>) -> Result<()>`
  - `pub fn verify_issue_225_ladder_case(case: &Issue225LadderCase, result: &DistanceBoundResult, css: &CssCode, expected_method: DistanceBoundMethod) -> Result<()>`

- [ ] **Step 1: Write failing ladder verifier tests**

Add these imports to `qec-code/tests/distance_bound.rs`:

```rust
use qec_code::distance_bound::{
    BoundType, BoundValidationContext, DistanceBoundMethod, DistanceBoundProvenance,
    DistanceBoundResult, DistanceBoundStatus, DistanceBoundWitness, Issue225LadderCase,
    RandomizedUpperBoundOptions, randomized_css_upper_bound, validate_randomized_upper_bound_result,
    verify_issue_225_ladder_case,
};
```

Append these helpers near the existing test helpers:

```rust
fn issue_225_ladder_cases() -> Vec<Issue225LadderCase> {
    serde_json::from_str(include_str!("fixtures/distance/issue_225_ladder.json"))
        .expect("issue-225 ladder fixture should deserialize")
}

fn issue_225_case(case_id: &str) -> Issue225LadderCase {
    issue_225_ladder_cases()
        .into_iter()
        .find(|case| case.case_id == case_id)
        .expect("requested issue-225 ladder case should exist")
}

fn css_from_built_in_code_id(code_id: &str) -> CssCode {
    let checks = built_in_css_checks(code_id).unwrap();
    css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz)
}

fn x_only_witness(num_qubits: usize, support: &[usize]) -> DistanceBoundWitness {
    let mut x = vec![0; num_qubits];
    for &qubit in support {
        x[qubit] = 1;
    }
    let pauli = Pauli::from_xz_bits(x, vec![0; num_qubits]).unwrap();
    DistanceBoundWitness::from_pauli(&pauli)
}

fn surface_rotated_d5_result_with_x_support(support: &[usize]) -> DistanceBoundResult {
    DistanceBoundResult::completed(
        support.len(),
        LogicalClass::XLike,
        x_only_witness(25, support),
        RandomizedUpperBoundOptions {
            iterations: 5000,
            restarts: 8,
            seed: 225,
            target_weight: Some(5),
        },
    )
}
```

Append these tests:

```rust
#[test]
fn issue_225_ladder_verifier_accepts_exact_upper_bounds_and_rejects_loose_bounds() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);
    let exact = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4]);

    verify_issue_225_ladder_case(
        &case,
        &exact,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .unwrap();

    let loose = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4, 9, 14]);
    let error = verify_issue_225_ladder_case(
        &case,
        &loose,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected loose bound rejection");

    assert_eq!(
        error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 expected upper_bound <= 5, got 7".to_owned(),
        )
    );
}

#[test]
fn issue_225_ladder_verifier_rejects_unvalidated_witness() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);

    let stabilizer_span = surface_rotated_d5_result_with_x_support(&[0, 5]);
    let span_error = verify_issue_225_ladder_case(
        &case,
        &stabilizer_span,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected stabilizer-span witness rejection");
    assert_eq!(
        span_error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 witness lies in stabilizer span".to_owned(),
        )
    );

    let mut mismatched_weight = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4]);
    mismatched_weight.witness.weight = 4;
    let weight_error = verify_issue_225_ladder_case(
        &case,
        &mismatched_weight,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected serialized witness weight rejection");
    assert_eq!(
        weight_error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 upper_bound must equal witness weight".to_owned(),
        )
    );
}

#[test]
fn issue_225_ladder_verifier_rejects_wrong_method_label() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);
    let result = surface_rotated_d5_result_with_x_support(&[0, 1, 2, 3, 4]);

    let error = verify_issue_225_ladder_case(
        &case,
        &result,
        &css,
        DistanceBoundMethod::RandomWindowUpperBound,
    )
    .expect_err("expected method mismatch rejection");

    assert_eq!(
        error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 expected method random-window-upper-bound, got randomized-upper-bound"
                .to_owned(),
        )
    );
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p qec-code issue_225_ladder_verifier_accepts_exact_upper_bounds_and_rejects_loose_bounds -q --offline
```

Expected: FAIL to compile because `Issue225LadderCase`, `verify_issue_225_ladder_case`, and `DistanceBoundMethod::RandomWindowUpperBound` do not exist yet.

- [ ] **Step 3: Add method labels and shared verifier implementation**

In `qec-code/src/distance_bound.rs`, update `DistanceBoundMethod`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistanceBoundMethod {
    RandomizedUpperBound,
    RandomWindowUpperBound,
    Exact,
}

impl DistanceBoundMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RandomizedUpperBound => "randomized-upper-bound",
            Self::RandomWindowUpperBound => "random-window-upper-bound",
            Self::Exact => "exact",
        }
    }
}
```

Add these types after `BoundValidationContext`:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Issue225LadderCase {
    pub case_id: String,
    pub source_issue: u64,
    pub code_id: String,
    pub expected_upper_bound: usize,
    pub target_weight: usize,
    pub tier: String,
    pub run_mode: String,
}

#[derive(Debug, Clone, Copy)]
pub struct MethodAwareBoundValidationContext<'a> {
    pub code: &'a StabilizerCode,
    pub expected_method: DistanceBoundMethod,
    pub known_exact_distance: Option<usize>,
}
```

Replace the body of `validate_randomized_upper_bound_result` with:

```rust
pub fn validate_randomized_upper_bound_result(
    result: &DistanceBoundResult,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    validate_distance_bound_result(
        result,
        MethodAwareBoundValidationContext {
            code: context.code,
            expected_method: DistanceBoundMethod::RandomizedUpperBound,
            known_exact_distance: context.known_exact_distance,
        },
    )
}
```

Add the shared validator and ladder verifier:

```rust
pub fn validate_distance_bound_result(
    result: &DistanceBoundResult,
    context: MethodAwareBoundValidationContext<'_>,
) -> Result<()> {
    result.options.validate()?;

    if result.method != context.expected_method {
        return Err(QecError::DistanceBoundValidationFailed(format!(
            "expected method {}, got {}",
            context.expected_method.label(),
            result.method.label()
        )));
    }
    if result.bound_type != BoundType::Upper {
        return Err(QecError::DistanceBoundValidationFailed(
            "distance bound results must use bound_type upper".to_owned(),
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
    if witness.n() != context.code.n() {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness width must match code length".to_owned(),
        ));
    }
    if witness.weight() == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ));
    }
    if result.witness.weight != witness.weight() {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness weight field must equal Pauli weight".to_owned(),
        ));
    }
    if result.logical_class != classify_witness_support(&witness) {
        return Err(QecError::DistanceBoundValidationFailed(
            "logical_class must match witness support".to_owned(),
        ));
    }
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

pub fn verify_issue_225_ladder_case(
    case: &Issue225LadderCase,
    result: &DistanceBoundResult,
    css: &CssCode,
    expected_method: DistanceBoundMethod,
) -> Result<()> {
    if result.method != expected_method {
        return Err(QecError::DistanceBoundValidationFailed(format!(
            "{} expected method {}, got {}",
            case.case_id,
            expected_method.label(),
            result.method.label()
        )));
    }

    validate_distance_bound_result(
        result,
        MethodAwareBoundValidationContext {
            code: css.code(),
            expected_method,
            known_exact_distance: None,
        },
    )
    .map_err(|error| prefix_ladder_case_error(&case.case_id, error))?;

    if result.upper_bound > case.expected_upper_bound {
        return Err(QecError::DistanceBoundValidationFailed(format!(
            "{} expected upper_bound <= {}, got {}",
            case.case_id, case.expected_upper_bound, result.upper_bound
        )));
    }

    Ok(())
}

fn prefix_ladder_case_error(case_id: &str, error: QecError) -> QecError {
    match error {
        QecError::DistanceBoundValidationFailed(message) => {
            QecError::DistanceBoundValidationFailed(format!("{case_id} {message}"))
        }
        other => QecError::DistanceBoundValidationFailed(format!("{case_id} {other}")),
    }
}
```

- [ ] **Step 4: Update existing expected messages if necessary**

If `validator_rejects_exact_labeled_randomized_result` fails because the shared
validator message changed, update the expected string to:

```rust
"distance bound results must use bound_type upper".to_owned()
```

If `validator_rejects_wrong_method` fails because the shared validator message
changed, update the expected string to:

```rust
"expected method randomized-upper-bound, got exact".to_owned()
```

- [ ] **Step 5: Run focused GREEN tests**

Run:

```bash
cargo test -p qec-code issue_225_ladder_verifier_accepts_exact_upper_bounds_and_rejects_loose_bounds -q --offline
cargo test -p qec-code issue_225_ladder_verifier_rejects_unvalidated_witness -q --offline
cargo test -p qec-code issue_225_ladder_verifier_rejects_wrong_method_label -q --offline
```

Expected: all three pass.

- [ ] **Step 6: Run the crate test suite and commit**

Run:

```bash
cargo test -p qec-code -q --offline
```

Expected: all tests pass.

Commit:

```bash
git add qec-code/src/distance_bound.rs qec-code/tests/distance_bound.rs docs/superpowers/plans/2026-06-25-issue-229-css-upper-bound-ladder-verifier.md
git commit -m "feat: add CSS upper-bound ladder verifier"
```
