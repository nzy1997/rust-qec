# Issue 344 CSS Component Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace random-window hot-loop full Pauli witness validation with explicit algebraic CSS component checks while preserving final result validation.

**Architecture:** Keep the change inside `qec-code/src/distance_bound.rs`. Add a private component-filter verdict helper that proves kernel membership and same-side stabilizer component nonmembership for pure CSS candidates, then update the row-processing loop to use that helper before constructing a Pauli. The final `completed_random_window_upper_bound_result` path still calls `validate_random_window_upper_bound_result`.

**Tech Stack:** Rust 2024, existing `gf2` row-span helpers, existing `CssCode` fixtures, Cargo tests, existing benchmark Make targets.

## Global Constraints

- Do not change random-window sampling semantics.
- Do not remove final result validation.
- Do not change `randomized-upper-bound`.
- Do not add bit-packed GF(2) storage or a reusable kernel workspace.
- Keep helper API private to `qec-code/src/distance_bound.rs`.
- Keep current-best weight pruning before component span checks.
- Keep `target_weight` unset in no-target ladder benchmark output.
- Use the existing `witness_validation_time_ns` field only for work that remains after the cheap component filters; the bucket may drop to zero for normal random-window runs.

---

## File Structure

- Modify `qec-code/src/distance_bound.rs`
  - Add `CssComponentCandidateVerdict`.
  - Add `css_component_candidate_verdict(...)`.
  - Update `consider_component_candidates(...)` and `consider_component_candidate_rows(...)` to use algebraic component checks instead of `validate_witness_against_code_with_span(...)`.
  - Add private unit tests for filter/full-validator parity and negative controls.
- No public Rust API changes.
- No benchmark schema changes.

### Task 1: Component Filter Unit Tests

**Files:**
- Modify: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: existing private `ComponentKind`, `component_candidate_to_pauli`, `validate_witness_against_code_with_span`, `gf2::try_random_window_kernel_basis_with_width`, `gf2::try_rref_with_width`.
- Produces: failing tests named `random_window_component_filter_matches_full_witness_validation` and `random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates`.

- [ ] **Step 1: Add test imports and helpers**

Inside `#[cfg(test)] mod tests` in `qec-code/src/distance_bound.rs`, extend the test module with these imports and helpers below `use super::*;`:

```rust
    use crate::codes::built_in_css::built_in_css_checks;
    use crate::css::{CssCode, SparseRowsMatrix};

    fn css_from_sparse_rows(num_cols: usize, hx: Vec<Vec<usize>>, hz: Vec<Vec<usize>>) -> CssCode {
        let hx = SparseRowsMatrix::new(num_cols, hx).unwrap().to_dense_rows();
        let hz = SparseRowsMatrix::new(num_cols, hz).unwrap().to_dense_rows();
        CssCode::from_hx_hz(hx, hz).unwrap()
    }

    fn css_from_built_in_code_id(code_id: &str) -> CssCode {
        let checks = built_in_css_checks(code_id).unwrap();
        css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz)
    }

    fn first_non_kernel_candidate(checks: &[Vec<u8>], width: usize) -> Vec<u8> {
        for column in 0..width {
            if checks.iter().any(|row| row[column] == 1) {
                let mut candidate = vec![0; width];
                candidate[column] = 1;
                return candidate;
            }
        }
        panic!("expected at least one nonzero check column");
    }

    fn full_validator_component_verdict(
        code: &StabilizerCode,
        stabilizer_span: &gf2::ReducedRows,
        component: ComponentKind,
        candidate: &[u8],
    ) -> Result<CssComponentCandidateVerdict> {
        let witness = component_candidate_to_pauli(component, candidate.to_vec())?;
        match validate_witness_against_code_with_span(code, stabilizer_span, &witness) {
            Ok(()) => Ok(CssComponentCandidateVerdict::Accepted),
            Err(QecError::DistanceBoundValidationFailed(message))
                if message == "witness must be non-identity" =>
            {
                Ok(CssComponentCandidateVerdict::Zero)
            }
            Err(QecError::DistanceBoundValidationFailed(message))
                if message == "witness does not commute with stabilizers" =>
            {
                Ok(CssComponentCandidateVerdict::NonKernel)
            }
            Err(QecError::DistanceBoundValidationFailed(message))
                if message == "witness lies in stabilizer span" =>
            {
                Ok(CssComponentCandidateVerdict::StabilizerSpan)
            }
            Err(error) => Err(error),
        }
    }
```

- [ ] **Step 2: Add the parity test**

Add this test after the existing random-window pruning tests:

```rust
    #[test]
    fn random_window_component_filter_matches_full_witness_validation() {
        for code_id in ["surface_rotated:d=3", "bb72"] {
            let css = css_from_built_in_code_id(code_id);
            let width = css.code().n();
            let stabilizer_span = gf2::try_rref_with_width(&css.code().stabilizer_rows(), width * 2)
                .unwrap();
            let identity_permutation = (0..width).collect::<Vec<_>>();

            for (component, kernel_checks, component_span_rows) in [
                (ComponentKind::XLike, css.hz(), css.hx()),
                (ComponentKind::ZLike, css.hx(), css.hz()),
            ] {
                let component_span = gf2::try_rref_with_width(component_span_rows, width).unwrap();
                let mut candidates = Vec::new();
                candidates.push(vec![0; width]);
                candidates.push(first_non_kernel_candidate(kernel_checks, width));
                if let Some(span_row) = component_span_rows.first() {
                    candidates.push(span_row.clone());
                }
                candidates.extend(
                    gf2::try_random_window_kernel_basis_with_width(
                        kernel_checks,
                        width,
                        &identity_permutation,
                    )
                    .unwrap(),
                );

                let mut accepted = 0;
                let mut non_kernel_rejected = 0;
                let mut stabilizer_span_rejected = 0;
                for candidate in candidates {
                    let component_verdict = css_component_candidate_verdict(
                        kernel_checks,
                        &component_span,
                        &candidate,
                    )
                    .unwrap();
                    let full_verdict = full_validator_component_verdict(
                        css.code(),
                        &stabilizer_span,
                        component,
                        &candidate,
                    )
                    .unwrap();

                    assert_eq!(
                        component_verdict, full_verdict,
                        "{code_id} {component:?} candidate {candidate:?}"
                    );
                    match component_verdict {
                        CssComponentCandidateVerdict::Accepted => accepted += 1,
                        CssComponentCandidateVerdict::NonKernel => non_kernel_rejected += 1,
                        CssComponentCandidateVerdict::StabilizerSpan => {
                            stabilizer_span_rejected += 1
                        }
                        CssComponentCandidateVerdict::Zero => {}
                    }
                }

                assert!(accepted > 0, "{code_id} {component:?} should have accepted rows");
                assert!(
                    non_kernel_rejected > 0,
                    "{code_id} {component:?} should exercise non-kernel rejection"
                );
                assert!(
                    stabilizer_span_rejected > 0,
                    "{code_id} {component:?} should exercise stabilizer-span rejection"
                );
            }
        }
    }
```

- [ ] **Step 3: Add the negative-control test**

Add this test after the parity test:

```rust
    #[test]
    fn random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates() {
        let css = css_from_sparse_rows(3, vec![vec![0, 1]], vec![vec![2]]);
        let width = css.code().n();

        let hx_span = gf2::try_rref_with_width(css.hx(), width).unwrap();
        let mut x_best = None;
        let mut x_stats = RandomWindowSearchStats::default();
        consider_component_candidate_rows(
            vec![vec![0, 0, 1], vec![1, 1, 0]],
            css.hz(),
            &hx_span,
            ComponentKind::XLike,
            &mut x_best,
            &mut x_stats,
        )
        .unwrap();
        assert!(x_best.is_none());
        assert_eq!(x_stats.component_candidates_generated, 2);
        assert_eq!(x_stats.witness_validation_candidates_rejected, 1);
        assert_eq!(x_stats.stabilizer_span_candidates_rejected, 1);
        assert_eq!(x_stats.valid_witnesses_found, 0);
        assert_eq!(x_stats.best_witness_updates, 0);

        let hz_span = gf2::try_rref_with_width(css.hz(), width).unwrap();
        let mut z_best = None;
        let mut z_stats = RandomWindowSearchStats::default();
        consider_component_candidate_rows(
            vec![vec![1, 0, 0], vec![0, 0, 1]],
            css.hx(),
            &hz_span,
            ComponentKind::ZLike,
            &mut z_best,
            &mut z_stats,
        )
        .unwrap();
        assert!(z_best.is_none());
        assert_eq!(z_stats.component_candidates_generated, 2);
        assert_eq!(z_stats.witness_validation_candidates_rejected, 1);
        assert_eq!(z_stats.stabilizer_span_candidates_rejected, 1);
        assert_eq!(z_stats.valid_witnesses_found, 0);
        assert_eq!(z_stats.best_witness_updates, 0);
    }
```

- [ ] **Step 4: Verify the tests fail before implementation**

Run:

```bash
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q
```

Expected: both fail to compile because `CssComponentCandidateVerdict` and `css_component_candidate_verdict` do not exist, and `consider_component_candidate_rows` does not yet accept `kernel_checks`.

### Task 2: Algebraic Component Filter

**Files:**
- Modify: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: `gf2::validate_rows_with_width`, `gf2::validate_target`, `gf2::try_in_reduced_row_span`.
- Produces: private `CssComponentCandidateVerdict` and `css_component_candidate_verdict(opposite_checks: &[Vec<u8>], stabilizer_component_span: &gf2::ReducedRows, candidate: &[u8]) -> Result<CssComponentCandidateVerdict>`.

- [ ] **Step 1: Add the verdict enum and helper**

Insert this code below `enum ComponentKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssComponentCandidateVerdict {
    Accepted,
    Zero,
    NonKernel,
    StabilizerSpan,
}

fn css_component_candidate_verdict(
    opposite_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    candidate: &[u8],
) -> Result<CssComponentCandidateVerdict> {
    let width = stabilizer_component_span.width;
    gf2::validate_rows_with_width(opposite_checks, width)?;
    gf2::validate_target(candidate)?;
    if candidate.len() != width {
        return Err(QecError::RowWidthMismatch {
            expected: width,
            actual: candidate.len(),
        });
    }
    if !candidate.iter().any(|bit| *bit == 1) {
        return Ok(CssComponentCandidateVerdict::Zero);
    }

    for check in opposite_checks {
        let parity = check
            .iter()
            .zip(candidate)
            .fold(0, |acc, (&check_bit, &candidate_bit)| {
                acc ^ (check_bit & candidate_bit)
            });
        if parity != 0 {
            return Ok(CssComponentCandidateVerdict::NonKernel);
        }
    }

    if gf2::try_in_reduced_row_span(stabilizer_component_span, candidate)? {
        return Ok(CssComponentCandidateVerdict::StabilizerSpan);
    }

    Ok(CssComponentCandidateVerdict::Accepted)
}
```

- [ ] **Step 2: Update candidate function signatures**

Change `consider_component_candidates` to remove `code` and `stabilizer_span`, and pass `kernel_checks` into `consider_component_candidate_rows`:

```rust
fn consider_component_candidates(
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    width: usize,
    permutation: &[usize],
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
    search_stats.kernel_basis_generations += 1;
    let kernel_started = Instant::now();
    let candidates =
        gf2::try_random_window_kernel_basis_with_width(kernel_checks, width, permutation);
    add_elapsed_ns(&mut search_stats.kernel_basis_time_ns, kernel_started);
    let candidates = candidates?;

    consider_component_candidate_rows(
        candidates,
        kernel_checks,
        stabilizer_component_span,
        component,
        best_witness,
        search_stats,
    )
}
```

Change `consider_component_candidate_rows` to:

```rust
fn consider_component_candidate_rows(
    candidates: Vec<Vec<u8>>,
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
```

- [ ] **Step 3: Replace full witness validation in the row loop**

Inside `consider_component_candidate_rows`, keep the existing zero and weight-pruning checks. Replace the span check plus full witness validation block with:

```rust
        let component_verdict = css_component_candidate_verdict(
            kernel_checks,
            stabilizer_component_span,
            &candidate,
        )?;
        add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
        match component_verdict {
            CssComponentCandidateVerdict::Accepted => {}
            CssComponentCandidateVerdict::Zero => {
                search_stats.zero_candidates_rejected += 1;
                continue;
            }
            CssComponentCandidateVerdict::NonKernel => {
                search_stats.witness_validation_candidates_rejected += 1;
                continue;
            }
            CssComponentCandidateVerdict::StabilizerSpan => {
                search_stats.stabilizer_span_candidates_rejected += 1;
                continue;
            }
        }

        let validation_started = Instant::now();
        let witness = component_candidate_to_pauli(component, candidate)?;
        add_elapsed_ns(
            &mut search_stats.witness_validation_time_ns,
            validation_started,
        );
        search_stats.valid_witnesses_found += 1;
```

Do not call `validate_witness_against_code_with_span` in this loop.

- [ ] **Step 4: Remove random-window hot-loop stabilizer span setup**

In `random_window_css_upper_bound`, remove:

```rust
    let stabilizer_span = gf2::try_rref_with_width(&code.stabilizer_rows(), width * 2)?;
```

Update both `consider_component_candidates(...)` calls by removing the `code` and `&stabilizer_span` arguments.

- [ ] **Step 5: Update existing private unit tests**

In the existing tests `random_window_prunes_candidates_that_cannot_improve_best` and `random_window_pruning_does_not_skip_strictly_better_candidate`, remove the `code` and `stabilizer_span` local variables and pass `&[]` as `kernel_checks` to `consider_component_candidate_rows`.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q
cargo test -p qec-code random_window_prunes_candidates_that_cannot_improve_best -q
cargo test -p qec-code random_window_pruning_does_not_skip_strictly_better_candidate -q
```

Expected: all pass.

### Task 3: Verification And Commit

**Files:**
- Modify: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: completed Task 1 and Task 2 implementation.
- Produces: verified implementation commit.

- [ ] **Step 1: Run issue verification commands**

Run:

```bash
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options -q
cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q
make qec-code-random-window-bench-no-target-ladder-smoke
cargo test
```

Expected: all pass. The no-target ladder smoke reports `surface_rotated_d5 = 5`, `toric_d5 = 5`, `bb72 = 6`, and `bb144 = 12`, and generated no-target output keeps `target_weight` unset.

- [ ] **Step 2: Inspect no-target summary timing**

Run:

```bash
rg -n "surface_rotated_d5|toric_d5|bb72|bb144|witness=" benchmarks/out/qec_code_random_window/no-target-ladder-smoke/summary/summary.md
```

Expected: `summary.md` includes the timing note with a `witness=` bucket for each no-target ladder case.

- [ ] **Step 3: Run formatting and diff checks**

Run:

```bash
cargo fmt --check
git diff --check
git status --short
```

Expected: formatting and diff checks pass; status shows only the intended implementation files before commit.

- [ ] **Step 4: Commit implementation**

Run:

```bash
git add qec-code/src/distance_bound.rs
git commit -m "feat: use CSS component checks in random-window search"
```

Expected: one implementation commit after the design and plan commits.
