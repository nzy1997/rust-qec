# Random-Window CSS Upper-Bound Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `random_window_css_upper_bound` so CSS component kernel candidates produce validated deterministic upper-bound witnesses.

**Architecture:** `CssCode` keeps validated dense `H_X` and `H_Z` rows at construction time. `distance_bound` uses those component rows with the GF(2) random-window kernel-basis helper, filters stabilizer-span component candidates, converts survivors into Pauli witnesses, and reuses the existing random-window result validator.

**Tech Stack:** Rust, qec-code crate, existing GF(2) helpers, existing SplitMix64 RNG, Cargo integration tests.

## Global Constraints

- Do not add CLI exposure.
- Do not replace `randomized_css_upper_bound`.
- Do not add external RW/RIS, `dist-m4ri`, or `codeDistancePYPI` code.
- Preserve deterministic output for the same CSS input and seed.
- Return `DistanceBoundResult` with `method = "random-window-upper-bound"` and `bound_type = "upper"`.
- Set `logical_class` to `x_like` or `z_like` for component candidates.
- Early exit when `target_weight` is reached or beaten.

---

## File Structure

- Modify `qec-code/src/css.rs`: store validated dense CSS component rows and expose `hx()` / `hz()` accessors.
- Modify `qec-code/src/gf2.rs`: remove dead-code suppression from the random-window kernel helper when it becomes used outside tests.
- Modify `qec-code/src/distance_bound.rs`: import `gf2`, add `random_window_css_upper_bound`, candidate conversion, component filtering, deterministic permutation generation, and random-window completion helper.
- Modify `qec-code/tests/distance_bound.rs`: import the new function and add positive and negative issue tests.

### Task 1: Preserve Validated CSS Component Rows

**Files:**
- Modify: `qec-code/src/css.rs`
- Test: `qec-code/tests/distance_bound.rs`

**Interfaces:**
- Consumes: existing `CssCode::from_hx_hz(hx: Vec<Vec<u8>>, hz: Vec<Vec<u8>>) -> Result<CssCode>`.
- Produces:
  - `pub fn hx(&self) -> &[Vec<u8>]`
  - `pub fn hz(&self) -> &[Vec<u8>]`

- [ ] **Step 1: Write a failing access test**

Add this test near the top-level helper tests in `qec-code/tests/distance_bound.rs`:

```rust
#[test]
fn css_code_preserves_dense_component_rows_for_search() {
    let css = CssCode::from_hx_hz(vec![vec![1, 1, 0]], vec![vec![0, 0, 1]]).unwrap();

    assert_eq!(css.hx(), &[vec![1, 1, 0]]);
    assert_eq!(css.hz(), &[vec![0, 0, 1]]);
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p qec-code css_code_preserves_dense_component_rows_for_search -q`

Expected: FAIL with methods `hx` and `hz` not found on `CssCode`.

- [ ] **Step 3: Implement component-row storage**

Change `CssCode` in `qec-code/src/css.rs` to this shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssCode {
    code: StabilizerCode,
    hx: Vec<Vec<u8>>,
    hz: Vec<Vec<u8>>,
}
```

At the start of `CssCode::from_hx_hz`, after validation and before consuming rows into symplectic stabilizers, clone the validated component rows:

```rust
let validated_hx = hx.clone();
let validated_hz = hz.clone();
```

Return the stored rows:

```rust
Ok(Self {
    code: StabilizerCode::from_stabilizers(n, stabilizers)?,
    hx: validated_hx,
    hz: validated_hz,
})
```

Add accessors below `code()`:

```rust
pub fn hx(&self) -> &[Vec<u8>] {
    &self.hx
}

pub fn hz(&self) -> &[Vec<u8>] {
    &self.hz
}
```

- [ ] **Step 4: Run the access test**

Run: `cargo test -p qec-code css_code_preserves_dense_component_rows_for_search -q`

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add qec-code/src/css.rs qec-code/tests/distance_bound.rs
git commit -m "feat: preserve css component checks"
```

### Task 2: Add Component Random-Window Search

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/src/gf2.rs`
- Modify: `qec-code/tests/distance_bound.rs`

**Interfaces:**
- Consumes:
  - `CssCode::hx() -> &[Vec<u8>]`
  - `CssCode::hz() -> &[Vec<u8>]`
  - `gf2::try_random_window_kernel_basis_with_width(matrix, width, permutation) -> Result<Vec<Vec<u8>>>`
- Produces:
  - `pub fn random_window_css_upper_bound(css: &CssCode, options: RandomWindowUpperBoundOptions) -> Result<DistanceBoundResult<RandomWindowUpperBoundOptions>>`

- [ ] **Step 1: Write the pinned positive search test**

Update the distance-bound imports:

```rust
use qec_code::distance_bound::{
    BoundType, BoundValidationContext, DistanceBoundMethod, DistanceBoundProvenance,
    DistanceBoundResult, DistanceBoundStatus, DistanceBoundWitness, Issue225LadderCase,
    RandomWindowUpperBoundOptions, RandomizedUpperBoundOptions, random_window_css_upper_bound,
    randomized_css_upper_bound, validate_random_window_upper_bound_result,
    validate_randomized_upper_bound_result, verify_issue_225_ladder_case,
};
```

Add this helper:

```rust
fn pinned_random_window_options() -> RandomWindowUpperBoundOptions {
    RandomWindowUpperBoundOptions {
        iterations: 5000,
        restarts: 8,
        seed: 7,
        target_weight: Some(5),
    }
}
```

Add this test:

```rust
#[test]
fn random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options() {
    for code_id in ["surface_rotated:d=5", "toric:d=5"] {
        let css = css_from_built_in_code_id(code_id);
        let options = pinned_random_window_options();

        let first = random_window_css_upper_bound(&css, options.clone()).unwrap();
        let second = random_window_css_upper_bound(&css, options).unwrap();

        assert_eq!(first, second, "{code_id} should be deterministic");
        assert_eq!(first.method, DistanceBoundMethod::RandomWindowUpperBound);
        assert_eq!(first.upper_bound, 5, "{code_id}");
        assert_eq!(first.witness.weight, 5, "{code_id}");
        assert!(matches!(
            first.logical_class,
            LogicalClass::XLike | LogicalClass::ZLike
        ));
        validate_random_window_upper_bound_result(
            &first,
            BoundValidationContext {
                code: css.code(),
                known_exact_distance: Some(5),
            },
        )
        .unwrap();
    }
}
```

- [ ] **Step 2: Run the failing positive test**

Run: `cargo test -p qec-code random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options -q`

Expected: FAIL with unresolved import `random_window_css_upper_bound`.

- [ ] **Step 3: Implement the public entry point and helpers**

In `qec-code/src/distance_bound.rs`, change the imports:

```rust
use crate::gf2;
```

Add the public function near `randomized_css_upper_bound`:

```rust
pub fn random_window_css_upper_bound(
    css: &CssCode,
    options: RandomWindowUpperBoundOptions,
) -> Result<DistanceBoundResult<RandomWindowUpperBoundOptions>> {
    options.validate()?;

    let code = css.code();
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    let width = code.n();
    let mut rng = SplitMix64::new(options.seed);
    let mut best_witness: Option<Pauli> = None;

    for _restart in 0..options.restarts {
        for _iteration in 0..options.iterations {
            let permutation = shuffled_columns(width, &mut rng);
            consider_component_candidates(
                css.hz(),
                css.hx(),
                ComponentKind::XLike,
                width,
                &permutation,
                code,
                &mut best_witness,
            )?;
            if target_reached(&best_witness, options.target_weight) {
                return completed_random_window_upper_bound_result(
                    code,
                    best_witness.unwrap(),
                    options,
                );
            }

            consider_component_candidates(
                css.hx(),
                css.hz(),
                ComponentKind::ZLike,
                width,
                &permutation,
                code,
                &mut best_witness,
            )?;
            if target_reached(&best_witness, options.target_weight) {
                return completed_random_window_upper_bound_result(
                    code,
                    best_witness.unwrap(),
                    options,
                );
            }
        }
    }

    let witness = best_witness.ok_or(QecError::RandomizedUpperBoundWitnessNotFound)?;
    completed_random_window_upper_bound_result(code, witness, options)
}
```

Add the private helpers:

```rust
#[derive(Debug, Clone, Copy)]
enum ComponentKind {
    XLike,
    ZLike,
}

fn consider_component_candidates(
    kernel_checks: &[Vec<u8>],
    stabilizer_component_rows: &[Vec<u8>],
    component: ComponentKind,
    width: usize,
    permutation: &[usize],
    code: &StabilizerCode,
    best_witness: &mut Option<Pauli>,
) -> Result<()> {
    let candidates =
        gf2::try_random_window_kernel_basis_with_width(kernel_checks, width, permutation)?;

    for candidate in candidates {
        if !candidate.iter().any(|bit| *bit == 1) {
            continue;
        }
        if gf2::try_in_row_span_with_width(stabilizer_component_rows, width, &candidate)? {
            continue;
        }

        let witness = component_candidate_to_pauli(component, candidate)?;
        if validate_witness_against_code(code, &witness).is_err() {
            continue;
        }
        if best_witness
            .as_ref()
            .is_none_or(|current| witness.weight() < current.weight())
        {
            *best_witness = Some(witness);
        }
    }

    Ok(())
}

fn component_candidate_to_pauli(component: ComponentKind, candidate: Vec<u8>) -> Result<Pauli> {
    let width = candidate.len();
    match component {
        ComponentKind::XLike => Pauli::from_xz_bits(candidate, vec![0; width]),
        ComponentKind::ZLike => Pauli::from_xz_bits(vec![0; width], candidate),
    }
}

fn shuffled_columns(width: usize, rng: &mut SplitMix64) -> Vec<usize> {
    let mut permutation = (0..width).collect::<Vec<_>>();
    for i in (1..width).rev() {
        let j = rng.next_usize(i + 1);
        permutation.swap(i, j);
    }
    permutation
}

fn target_reached(best_witness: &Option<Pauli>, target_weight: Option<usize>) -> bool {
    best_witness.as_ref().is_some_and(|witness| {
        target_weight.is_some_and(|target| witness.weight() <= target)
    })
}

fn completed_random_window_upper_bound_result(
    code: &StabilizerCode,
    witness: Pauli,
    options: RandomWindowUpperBoundOptions,
) -> Result<DistanceBoundResult<RandomWindowUpperBoundOptions>> {
    let result = DistanceBoundResult::completed_random_window_upper_bound(
        witness.weight(),
        classify_witness_support(&witness),
        DistanceBoundWitness::from_pauli(&witness),
        options,
    );
    validate_random_window_upper_bound_result(
        &result,
        BoundValidationContext {
            code,
            known_exact_distance: None,
        },
    )?;
    Ok(result)
}
```

Remove `#[allow(dead_code)]` from `try_random_window_kernel_basis_with_width` in `qec-code/src/gf2.rs`.

- [ ] **Step 4: Run the positive test**

Run: `cargo test -p qec-code random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options -q`

Expected: PASS with surface and toric upper bound `5`.

- [ ] **Step 5: Commit Task 2**

```bash
git add qec-code/src/distance_bound.rs qec-code/src/gf2.rs qec-code/tests/distance_bound.rs
git commit -m "feat: add random-window css upper-bound search"
```

### Task 3: Add Stabilizer-Span Negative Control And Final Verification

**Files:**
- Modify: `qec-code/tests/distance_bound.rs`

**Interfaces:**
- Consumes: `random_window_css_upper_bound` and existing validation helpers.
- Produces: the issue negative-control test.

- [ ] **Step 1: Write the negative control test**

Add this test:

```rust
#[test]
fn random_window_upper_bound_rejects_stabilizer_span_component_candidate() {
    let css = css_from_sparse_rows(3, vec![vec![0, 1], vec![1, 2]], vec![]);
    let result = random_window_css_upper_bound(
        &css,
        RandomWindowUpperBoundOptions {
            iterations: 20,
            restarts: 1,
            seed: 11,
            target_weight: Some(1),
        },
    )
    .unwrap();

    assert_eq!(result.upper_bound, 1);
    assert_eq!(result.logical_class, LogicalClass::XLike);
    assert_ne!(result.witness.x, vec![1, 1, 0]);
    assert_ne!(result.witness.x, vec![0, 1, 1]);
    validate_random_window_upper_bound_result(
        &result,
        BoundValidationContext {
            code: css.code(),
            known_exact_distance: Some(1),
        },
    )
    .unwrap();
}
```

This code has low-weight X-check rows `[1, 1, 0]` and `[0, 1, 1]` in `row_span(H_X)`. They satisfy the X-like kernel equation because `H_Z` is empty, but the search must reject them and return the valid logical X representative instead.

- [ ] **Step 2: Run the negative control**

Run: `cargo test -p qec-code random_window_upper_bound_rejects_stabilizer_span_component_candidate -q`

Expected: PASS and no stabilizer-span component row returned as the witness.

- [ ] **Step 3: Run issue verification**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options -q
cargo test -p qec-code random_window_upper_bound_rejects_stabilizer_span_component_candidate -q
```

Expected: both commands PASS.

- [ ] **Step 4: Run full verification**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add qec-code/tests/distance_bound.rs
git commit -m "test: cover random-window css component filtering"
```
