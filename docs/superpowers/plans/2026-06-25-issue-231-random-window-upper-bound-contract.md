# Issue 231 Random-Window Upper-Bound Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the typed `random-window-upper-bound` result/options contract while preserving the existing randomized upper-bound JSON contract.

**Architecture:** Keep `DistanceBoundResult` as the shared result family, but make it generic over the method-specific options type with `RandomizedUpperBoundOptions` as the default. Add `RandomWindowUpperBoundOptions`, a random-window-specific constructor, and a random-window validator that reuses the existing method-aware witness validation path.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, existing `qec-code` distance-bound, CSS, stabilizer, and Pauli utilities, Cargo integration tests.

## Global Constraints

- Do not implement the random-window search algorithm.
- Do not change exact distance output.
- Do not add CLI flags or commands.
- Do not silently alias `randomized-upper-bound` to `random-window-upper-bound`.
- Completed random-window JSON must serialize `status = "completed"`, `method = "random-window-upper-bound"`, and `bound_type = "upper"`.
- Completed random-window JSON must include `options.iterations`, `options.restarts`, `options.seed`, `options.target_weight`, `witness`, `logical_class`, `upper_bound`, and provenance fields comparable to the existing randomized result.
- `upper_bound` must equal the serialized witness weight.
- Random-window validation must reject `method = "randomized-upper-bound"` with an error naming `random-window-upper-bound` as the expected method.
- Existing randomized validation must continue rejecting `method = "random-window-upper-bound"`.
- The existing randomized serialization contract must keep passing unchanged.
- Because this Agent Desk sandbox cannot write the local linked-worktree git index, local task commits may fail with `Operation not permitted`; preserve local diffs and let the controller create the final remote commit through the GitHub connector.

---

## File Structure

- Modify `qec-code/src/distance_bound.rs`
  - Add `RandomWindowUpperBoundOptions`.
  - Add a public options-validation trait used by the generic validator.
  - Share the existing randomized option validation logic with random-window options.
  - Change `DistanceBoundResult` to `DistanceBoundResult<Options = RandomizedUpperBoundOptions>`.
  - Add a private method-aware constructor helper.
  - Keep `DistanceBoundResult::completed(...)` as the randomized constructor.
  - Add `DistanceBoundResult::<RandomWindowUpperBoundOptions>::completed_random_window_upper_bound(...)`.
  - Add `validate_random_window_upper_bound_result(...)`.
  - Generalize `validate_distance_bound_result(...)` and `verify_issue_225_ladder_case(...)` over validated option types.
- Modify `qec-code/tests/distance_bound.rs`
  - Import `RandomWindowUpperBoundOptions` and `validate_random_window_upper_bound_result`.
  - Add a random-window result helper.
  - Add the issue-required serialization contract test.
  - Add the issue-required random-window wrong-method validator negative control.
  - Add a randomized validator negative control for the random-window label.

---

### Task 1: Random-Window Upper-Bound Result Contract

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/tests/distance_bound.rs`
- Track: `docs/superpowers/specs/2026-06-25-issue-231-random-window-upper-bound-contract-design.md`
- Track: `docs/superpowers/plans/2026-06-25-issue-231-random-window-upper-bound-contract.md`

**Interfaces:**
- Consumes:
  - Existing `DistanceBoundMethod::RandomWindowUpperBound`
  - Existing `BoundValidationContext`
  - Existing `MethodAwareBoundValidationContext`
  - Existing `validate_distance_bound_result` witness validation semantics
  - Existing `RandomizedUpperBoundOptions` JSON contract
- Produces:
  - `pub trait DistanceBoundOptions`
  - `pub struct RandomWindowUpperBoundOptions`
  - `impl DistanceBoundResult<RandomWindowUpperBoundOptions>::completed_random_window_upper_bound(...) -> Self`
  - `pub fn validate_random_window_upper_bound_result(result: &DistanceBoundResult<RandomWindowUpperBoundOptions>, context: BoundValidationContext<'_>) -> Result<()>`
  - Generic `DistanceBoundResult<Options = RandomizedUpperBoundOptions>`
  - Generic `validate_distance_bound_result<Options: DistanceBoundOptions>(...)`
  - Generic `verify_issue_225_ladder_case<Options: DistanceBoundOptions>(...)`

- [ ] **Step 1: Write failing random-window contract tests**

Update the `qec-code/tests/distance_bound.rs` import list to include the new options and validator:

```rust
use qec_code::distance_bound::{
    BoundType, BoundValidationContext, DistanceBoundMethod, DistanceBoundProvenance,
    DistanceBoundResult, DistanceBoundStatus, DistanceBoundWitness, Issue225LadderCase,
    RandomWindowUpperBoundOptions, RandomizedUpperBoundOptions, randomized_css_upper_bound,
    validate_random_window_upper_bound_result, validate_randomized_upper_bound_result,
    verify_issue_225_ladder_case,
};
```

Add this helper after `valid_result()`:

```rust
fn random_window_result() -> DistanceBoundResult<RandomWindowUpperBoundOptions> {
    DistanceBoundResult::completed_random_window_upper_bound(
        1,
        LogicalClass::XLike,
        one_qubit_x_witness(),
        RandomWindowUpperBoundOptions {
            iterations: 12,
            restarts: 2,
            seed: 99,
            target_weight: Some(1),
        },
    )
}
```

Append these tests near the existing serialization and validator tests:

```rust
#[test]
fn random_window_upper_bound_result_serializes_contract() {
    let result = random_window_result();

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "random-window-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 1);
    assert_eq!(json["logical_class"], "x_like");
    assert_eq!(json["witness"]["x"], serde_json::json!([1]));
    assert_eq!(json["witness"]["z"], serde_json::json!([0]));
    assert_eq!(json["witness"]["weight"], 1);
    assert_eq!(json["options"]["iterations"], 12);
    assert_eq!(json["options"]["restarts"], 2);
    assert_eq!(json["options"]["seed"], 99);
    assert_eq!(json["options"]["target_weight"], 1);
    assert_eq!(json["provenance"]["tool"], "qec-code");
    assert_eq!(json["provenance"]["method_revision"], 1);
}

#[test]
fn random_window_upper_bound_validator_rejects_wrong_method_label() {
    let code = trivial_one_qubit_code();
    let mut result = random_window_result();
    result.method = DistanceBoundMethod::RandomizedUpperBound;

    assert_eq!(
        validate_random_window_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(1),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "expected method random-window-upper-bound, got randomized-upper-bound".to_owned(),
        ))
    );
}

#[test]
fn randomized_upper_bound_validator_rejects_random_window_method_label() {
    let code = trivial_one_qubit_code();
    let mut result = valid_result();
    result.method = DistanceBoundMethod::RandomWindowUpperBound;

    assert_eq!(
        validate_randomized_upper_bound_result(
            &result,
            BoundValidationContext {
                code: &code,
                known_exact_distance: Some(1),
            },
        ),
        Err(QecError::DistanceBoundValidationFailed(
            "expected method randomized-upper-bound, got random-window-upper-bound".to_owned(),
        ))
    );
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_result_serializes_contract -q --offline
```

Expected: FAIL to compile because `RandomWindowUpperBoundOptions`, `DistanceBoundResult::completed_random_window_upper_bound`, and `validate_random_window_upper_bound_result` do not exist.

- [ ] **Step 3: Add random-window options and shared option validation**

In `qec-code/src/distance_bound.rs`, add this trait before `RandomizedUpperBoundOptions`:

```rust
pub trait DistanceBoundOptions {
    fn validate(&self) -> Result<()>;
}
```

Replace the body of `RandomizedUpperBoundOptions::validate` with:

```rust
validate_upper_bound_options(self.iterations, self.restarts, self.target_weight)
```

Add this trait implementation after the randomized options impl:

```rust
impl DistanceBoundOptions for RandomizedUpperBoundOptions {
    fn validate(&self) -> Result<()> {
        RandomizedUpperBoundOptions::validate(self)
    }
}
```

Add the random-window options type and validation:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomWindowUpperBoundOptions {
    pub iterations: usize,
    pub restarts: usize,
    pub seed: u64,
    pub target_weight: Option<usize>,
}

impl RandomWindowUpperBoundOptions {
    pub fn validate(&self) -> Result<()> {
        validate_upper_bound_options(self.iterations, self.restarts, self.target_weight)
    }
}

impl DistanceBoundOptions for RandomWindowUpperBoundOptions {
    fn validate(&self) -> Result<()> {
        RandomWindowUpperBoundOptions::validate(self)
    }
}
```

Move the shared validation checks into:

```rust
fn validate_upper_bound_options(
    iterations: usize,
    restarts: usize,
    target_weight: Option<usize>,
) -> Result<()> {
    if iterations == 0 {
        return Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        });
    }
    if restarts == 0 {
        return Err(QecError::InvalidDistanceBoundOption {
            option: "restarts",
            reason: "must be greater than zero".to_owned(),
        });
    }
    if target_weight == Some(0) {
        return Err(QecError::InvalidDistanceBoundOption {
            option: "target_weight",
            reason: "must be greater than zero when provided".to_owned(),
        });
    }
    Ok(())
}
```

- [ ] **Step 4: Make `DistanceBoundResult` generic and add constructors**

Change the result struct definition to:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundResult<Options = RandomizedUpperBoundOptions> {
    pub status: DistanceBoundStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: Options,
    pub provenance: DistanceBoundProvenance,
}
```

Replace the existing `impl DistanceBoundResult` block with:

```rust
impl<Options> DistanceBoundResult<Options> {
    fn completed_with_method(
        method: DistanceBoundMethod,
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: Options,
    ) -> Self {
        Self {
            status: DistanceBoundStatus::Completed,
            method,
            bound_type: BoundType::Upper,
            upper_bound,
            logical_class,
            witness,
            options,
            provenance: DistanceBoundProvenance::current(),
        }
    }
}

impl DistanceBoundResult<RandomizedUpperBoundOptions> {
    pub fn completed(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomizedUpperBoundOptions,
    ) -> Self {
        Self::completed_with_method(
            DistanceBoundMethod::RandomizedUpperBound,
            upper_bound,
            logical_class,
            witness,
            options,
        )
    }
}

impl DistanceBoundResult<RandomWindowUpperBoundOptions> {
    pub fn completed_random_window_upper_bound(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomWindowUpperBoundOptions,
    ) -> Self {
        Self::completed_with_method(
            DistanceBoundMethod::RandomWindowUpperBound,
            upper_bound,
            logical_class,
            witness,
            options,
        )
    }
}
```

- [ ] **Step 5: Add random-window validator and generic shared validation**

Add this wrapper after `validate_randomized_upper_bound_result`:

```rust
pub fn validate_random_window_upper_bound_result(
    result: &DistanceBoundResult<RandomWindowUpperBoundOptions>,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    validate_distance_bound_result(
        result,
        MethodAwareBoundValidationContext {
            code: context.code,
            expected_method: DistanceBoundMethod::RandomWindowUpperBound,
            known_exact_distance: context.known_exact_distance,
        },
    )
}
```

Change the shared validator signature to:

```rust
pub fn validate_distance_bound_result<Options: DistanceBoundOptions>(
    result: &DistanceBoundResult<Options>,
    context: MethodAwareBoundValidationContext<'_>,
) -> Result<()> {
```

Keep its body the same, including `result.options.validate()?`.

Change the ladder verifier signature to:

```rust
pub fn verify_issue_225_ladder_case<Options: DistanceBoundOptions>(
    case: &Issue225LadderCase,
    result: &DistanceBoundResult<Options>,
    css: &CssCode,
    expected_method: DistanceBoundMethod,
) -> Result<()> {
```

Keep its body the same.

- [ ] **Step 6: Run focused GREEN tests**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_result_serializes_contract -q --offline
cargo test -p qec-code random_window_upper_bound_validator_rejects_wrong_method_label -q --offline
cargo test -p qec-code completed_bound_result_serializes_with_upper_bound_contract -q --offline
cargo test -p qec-code randomized_upper_bound_validator_rejects_random_window_method_label -q --offline
```

Expected: all four commands pass.

- [ ] **Step 7: Run qec-code suite**

Run:

```bash
cargo test -p qec-code -q --offline
```

Expected: all qec-code tests pass.

- [ ] **Step 8: Record diff for controller review**

Run:

```bash
git diff -- qec-code/src/distance_bound.rs qec-code/tests/distance_bound.rs docs/superpowers/specs/2026-06-25-issue-231-random-window-upper-bound-contract-design.md docs/superpowers/plans/2026-06-25-issue-231-random-window-upper-bound-contract.md
```

Expected: diff contains only the issue #231 contract/docs changes.
