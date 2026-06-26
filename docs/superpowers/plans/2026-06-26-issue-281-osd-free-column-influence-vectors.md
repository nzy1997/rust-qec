# Issue 281 OSD free-column influence vectors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Precompute OSD free-column influence vectors and use them to assemble candidate corrections without repeated reduced-system back-substitution.

**Architecture:** Add a crate-private `FreeColumnInfluenceVectors` representation to `rbposd/src/gf2.rs`. Build it from a `ReducedLinearSystem`, the base OSD-0 solution, and the ordered free-column set, then use it in `rbposd/src/osd.rs` candidate loops to produce corrections by XORing precomputed toggle lists.

**Tech Stack:** Rust 2024 workspace; `rbposd` crate; existing GF(2), OSD, and decoder counter infrastructure; `cargo test`.

## Global Constraints

- Do not change candidate semantics.
- Do not change BP iteration behavior.
- Do not introduce campaign-level setup reuse.
- Keep the new influence-vector representation crate-private unless tests prove a public API is required.
- Candidate scoring and tie-breaking must remain unchanged.
- `osd_candidate_count` continues to count evaluated candidate sets.
- `gf2_solve_count` must no longer scale with evaluated candidate sets after influence vectors are used.

---

## File Structure

- Modify `rbposd/src/gf2.rs`: add `FreeColumnInfluenceVectors`, builder and candidate assembly methods, and direct unit tests for correctness and invalid forced columns.
- Modify `rbposd/src/osd.rs`: build influence vectors after the base reduced-system solve and use them in legacy, `ldpc`, and profiling candidate traversal.
- Modify `rbposd/tests/osd.rs`: update counter expectations that previously assumed one reduced solve per candidate.
- Modify `rsinter/tests/bb90_hard_syndrome_fixture.rs`: update BB90 profile counter expectations that previously assumed one reduced solve per candidate.

### Task 1: Add GF(2) Influence Vector Representation And Tests

**Files:**
- Modify: `rbposd/src/gf2.rs`

**Interfaces:**
- Consumes: `ReducedLinearSystem`, `DetailedSolution`, `DecodeError`, `Correction`.
- Produces: `FreeColumnInfluenceVectors::correction_for_forced_columns(&self, forced_true_columns: &[usize]) -> Result<Correction, DecodeError>` and `ReducedLinearSystem::free_column_influence_vectors(&self, base: &DetailedSolution, ordered_free_columns: &[usize]) -> Result<FreeColumnInfluenceVectors, DecodeError>`.

- [ ] **Step 1: Write the failing influence-vector correctness test**

Add this helper and test inside `#[cfg(test)] mod tests` in `rbposd/src/gf2.rs`:

```rust
    fn visit_test_combinations(
        columns: &[usize],
        target_len: usize,
        start: usize,
        forced: &mut Vec<usize>,
        visit: &mut impl FnMut(&[usize]),
    ) {
        if forced.len() == target_len {
            visit(forced);
            return;
        }
        let remaining = target_len - forced.len();
        for index in start..=columns.len() - remaining {
            forced.push(columns[index]);
            visit_test_combinations(columns, target_len, index + 1, forced, visit);
            forced.pop();
        }
    }

    #[test]
    fn osd_candidate_influence_vectors_match_back_substitution() {
        let pcm = ParityCheckMatrix::from_sparse_rows(
            2,
            5,
            vec![vec![0, 2, 3], vec![1, 3, 4]],
        )
        .unwrap();
        let syndrome = Syndrome::from(vec![true, false]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);
        let mut stats = super::Gf2SolveStats::default();
        let reduced = prepared
            .reduce_with_column_order_counting(&syndrome, &[0, 1, 2, 3, 4], &mut stats)
            .unwrap();
        let base = reduced
            .solve_with_forced_columns_counting(&[], &mut stats)
            .unwrap();
        let influences = reduced
            .free_column_influence_vectors(&base, &base.free_columns)
            .unwrap();

        let mut checked = 0usize;
        for order in 0..=2 {
            let mut forced = Vec::new();
            visit_test_combinations(
                &base.free_columns,
                order,
                0,
                &mut forced,
                &mut |columns| {
                    let expected = reduced
                        .solve_with_forced_columns_counting(
                            columns,
                            &mut super::Gf2SolveStats::default(),
                        )
                        .unwrap();
                    let assembled = influences
                        .correction_for_forced_columns(columns)
                        .unwrap();

                    assert_eq!(assembled, expected.correction);
                    assert_eq!(pcm.multiply(&assembled), syndrome);
                    checked += 1;
                },
            );
        }

        println!("candidate sets checked: {checked}");
        assert_eq!(checked, 7);
    }
```

Run:

```bash
cargo test -p rbposd osd_candidate_influence_vectors_match_back_substitution -- --nocapture
```

Expected: FAIL because `free_column_influence_vectors` does not exist yet.

- [ ] **Step 2: Write the failing invalid-column negative control**

Add this test beside the correctness test:

```rust
    #[test]
    fn osd_influence_vectors_reject_invalid_forced_columns() {
        let pcm = ParityCheckMatrix::from_sparse_rows(
            2,
            5,
            vec![vec![0, 2, 3], vec![1, 3, 4]],
        )
        .unwrap();
        let syndrome = Syndrome::from(vec![true, false]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);
        let mut stats = super::Gf2SolveStats::default();
        let reduced = prepared
            .reduce_with_column_order_counting(&syndrome, &[0, 1, 2, 3, 4], &mut stats)
            .unwrap();
        let base = reduced
            .solve_with_forced_columns_counting(&[], &mut stats)
            .unwrap();
        let influences = reduced
            .free_column_influence_vectors(&base, &[2, 3])
            .unwrap();

        assert_eq!(
            influences.correction_for_forced_columns(&[0]).unwrap_err(),
            DecodeError::SingularSystem
        );
        assert_eq!(
            influences.correction_for_forced_columns(&[5]).unwrap_err(),
            DecodeError::InvalidColumnIndex {
                column: 5,
                num_bits: 5,
            }
        );
        assert_eq!(
            influences.correction_for_forced_columns(&[4]).unwrap_err(),
            DecodeError::SingularSystem
        );
    }
```

Run:

```bash
cargo test -p rbposd osd_influence_vectors_reject_invalid_forced_columns -q
```

Expected: FAIL because the influence-vector API does not exist yet.

- [ ] **Step 3: Implement `FreeColumnInfluenceVectors`**

Add this type after `ReducedLinearSystem` in `rbposd/src/gf2.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FreeColumnInfluenceVectors {
    base_correction: Correction,
    ordered_free_columns: Vec<usize>,
    influence_toggles: Vec<Vec<usize>>,
    influence_by_column: Vec<Option<usize>>,
    num_bits: usize,
}
```

Add this method in `impl ReducedLinearSystem`:

```rust
    pub(crate) fn free_column_influence_vectors(
        &self,
        base: &DetailedSolution,
        ordered_free_columns: &[usize],
    ) -> Result<FreeColumnInfluenceVectors, DecodeError> {
        if base.correction.len() != self.num_bits {
            return Err(DecodeError::DimensionMismatch {
                what: "OSD base correction",
                expected: self.num_bits,
                actual: base.correction.len(),
            });
        }
        for &column in &self.free_columns {
            if base.correction.as_slice()[column] {
                return Err(DecodeError::SingularSystem);
            }
        }

        let mut influence_by_column = vec![None; self.num_bits];
        let mut influence_toggles = Vec::with_capacity(ordered_free_columns.len());
        for &free_column in ordered_free_columns {
            if free_column >= self.num_bits {
                return Err(DecodeError::InvalidColumnIndex {
                    column: free_column,
                    num_bits: self.num_bits,
                });
            }
            if !self.is_free[free_column] || influence_by_column[free_column].is_some() {
                return Err(DecodeError::SingularSystem);
            }

            let influence_index = influence_toggles.len();
            influence_by_column[free_column] = Some(influence_index);
            let mut toggles = vec![free_column];
            for (pivot_row, &pivot_column) in self.pivot_columns.iter().enumerate() {
                if self.rows[pivot_row][free_column] {
                    toggles.push(pivot_column);
                }
            }
            influence_toggles.push(toggles);
        }

        Ok(FreeColumnInfluenceVectors {
            base_correction: base.correction.clone(),
            ordered_free_columns: ordered_free_columns.to_vec(),
            influence_toggles,
            influence_by_column,
            num_bits: self.num_bits,
        })
    }
```

Add this `impl` after `impl ReducedLinearSystem`:

```rust
impl FreeColumnInfluenceVectors {
    pub(crate) fn correction_for_forced_columns(
        &self,
        forced_true_columns: &[usize],
    ) -> Result<Correction, DecodeError> {
        let mut correction = self.base_correction.as_slice().to_vec();
        let mut seen = vec![false; self.num_bits];
        for &column in forced_true_columns {
            if column >= self.num_bits {
                return Err(DecodeError::InvalidColumnIndex {
                    column,
                    num_bits: self.num_bits,
                });
            }
            let influence_index = self.influence_by_column[column].ok_or(DecodeError::SingularSystem)?;
            if seen[column] {
                continue;
            }
            seen[column] = true;
            for &physical in &self.influence_toggles[influence_index] {
                correction[physical] ^= true;
            }
        }
        Ok(Correction::from(correction))
    }
}
```

Run:

```bash
cargo test -p rbposd osd_candidate_influence_vectors_match_back_substitution -- --nocapture
cargo test -p rbposd osd_influence_vectors_reject_invalid_forced_columns -q
```

Expected: both PASS.

### Task 2: Route OSD Candidate Evaluation Through Influence Vectors

**Files:**
- Modify: `rbposd/src/osd.rs`
- Modify: `rbposd/tests/osd.rs`
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Consumes: `FreeColumnInfluenceVectors` from Task 1.
- Produces: OSD candidate loops that assemble candidate corrections without calling `ReducedLinearSystem::solve_with_forced_columns_counting` per candidate.

- [ ] **Step 1: Import the influence-vector type**

Update the `rbposd/src/osd.rs` GF(2) import to:

```rust
use crate::gf2::{
    DetailedSolution, FreeColumnInfluenceVectors, Gf2SolveStats, PreparedLinearSystem,
    ReducedLinearSystem,
};
```

- [ ] **Step 2: Change candidate comparison to use corrections**

Replace `is_better_solution` with:

```rust
fn is_better_correction(candidate: &Correction, best: &Correction, objective_weights: &[f64]) -> bool {
    let candidate_cost = residual_cost(candidate.as_slice(), objective_weights);
    let best_cost = residual_cost(best.as_slice(), objective_weights);
    if candidate_cost < best_cost - f64::EPSILON {
        return true;
    }
    if (candidate_cost - best_cost).abs() <= f64::EPSILON {
        return candidate.as_slice() < best.as_slice();
    }
    false
}
```

- [ ] **Step 3: Use influences in legacy candidate evaluation**

In `best_legacy_osd_candidate`, build influences from the legacy frontier and
replace each per-candidate reduced solve with influence assembly:

```rust
    let influences = reduced.free_column_influence_vectors(&base, &frontier)?;
```

Inside the visit closure:

```rust
            let candidate = influences.correction_for_forced_columns(columns);
            if let Ok(candidate) = candidate {
                if is_better_correction(&candidate, &best.correction, objective_weights) {
                    best.correction = candidate;
                }
            }
```

Remove the per-candidate `Gf2SolveStats` creation and `accumulate_gf2_stats`
call from that closure.

- [ ] **Step 4: Use influences in `ldpc_osd_cs` candidate evaluation**

In `best_ldpc_osd_candidate`, build influences for all free columns:

```rust
    let influences = reduced.free_column_influence_vectors(&base, &free_columns)?;
```

Replace each single-column and pair candidate solve with:

```rust
        let candidate = influences
            .correction_for_forced_columns(&[column])
            .expect("LDPC OSD-CS single-column candidates are selected from precomputed free columns");
        if is_better_correction(&candidate, &best.correction, objective_weights) {
            best.correction = candidate;
        }
```

and:

```rust
        let candidate = influences
            .correction_for_forced_columns(columns)
            .expect("LDPC OSD-CS pair candidates are selected from precomputed free columns");
        if is_better_correction(&candidate, &best.correction, objective_weights) {
            best.correction = candidate;
        }
```

Remove per-candidate GF(2) stats from both paths.

- [ ] **Step 5: Use influences in profile traversal**

Change `profile_legacy_osd_candidates` and `profile_ldpc_osd_candidates` to
build influences for the same candidate column sets they traverse. Replace each
`solve_with_forced_columns_counting` call with:

```rust
let _ = influences.correction_for_forced_columns(columns);
```

or for a single column:

```rust
let _ = influences.correction_for_forced_columns(&[column]);
```

Keep `osd_candidate_count` and limit handling unchanged.

- [ ] **Step 6: Update integration counter expectations**

In `rbposd/tests/osd.rs`, update these assertions:

```rust
assert_eq!(ldpc_decode.stats.gf2_solve_count, 1);
```

```rust
assert_eq!(result.stats.gf2_solve_count, 1);
assert!(result.stats.osd_candidate_count > result.stats.gf2_solve_count);
```

```rust
assert_eq!(stats.gf2_solve_count, 1);
```

```rust
assert_eq!(limited_stats.gf2_solve_count, 1);
```

```rust
assert_eq!(stats.gf2_solve_count, 1);
```

In `rsinter/tests/bb90_hard_syndrome_fixture.rs`, update the three profile
assertions that require `profile.gf2_solve_count >= profile.osd_candidate_count
+ 1` to:

```rust
assert_eq!(profile.gf2_solve_count, 1);
```

Run:

```bash
cargo test -p rbposd osd_order7_reuses_factorization_without_changing_correction -q
cargo test -p rbposd ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs -- --nocapture
cargo test -p rsinter --test bb90_hard_syndrome_fixture syndrome_profile_replay_reports_nontrivial_osd_counts -q
```

Expected: all pass and candidate count remains positive while `gf2_solve_count` is one on OSD candidate paths.

### Task 3: Final Verification And Cleanup

**Files:**
- All touched files.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: formatted, verified branch ready for PR.

- [ ] **Step 1: Format touched Rust files**

Run:

```bash
rustfmt --edition 2024 rbposd/src/gf2.rs rbposd/src/osd.rs rbposd/tests/osd.rs --check
```

Expected: PASS. If it fails, run the same command without `--check`, review the diff, and rerun the check.

- [ ] **Step 2: Run issue-required verification**

Run:

```bash
cargo test -p rbposd osd_candidate_influence_vectors_match_back_substitution -- --nocapture
cargo test -p rbposd osd_influence_vectors_reject_invalid_forced_columns -q
```

Expected: both PASS, and the positive test prints `candidate sets checked: 7`.

- [ ] **Step 3: Run broader verification**

Run:

```bash
cargo test -p rbposd
cargo test
```

Expected: both PASS.

- [ ] **Step 4: Review the diff**

Run:

```bash
git diff --check
git diff --stat
```

Expected: no whitespace errors, and changes limited to the design/plan plus `rbposd` influence-vector implementation and tests.

## Self-Review

- Spec coverage: the plan adds reusable influence vectors, invalid forced-column rejection, OSD integration, counter updates, and required verification commands.
- Placeholder scan: no placeholder terms are used as work items.
- Type consistency: `FreeColumnInfluenceVectors`, `free_column_influence_vectors`, and `correction_for_forced_columns` are named consistently across all tasks.

## Execution Choice

Standing answer policy selects **Subagent-Driven (recommended)** because it is the recommended option in the writing-plans handoff. This Agent Desk run will use the recommended execution workflow where tools are available; if subagent tooling is unavailable, the same task order will be executed inline and recorded in the final decision log.
