# Issue #214 rbposd OSD Forced-Column Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optimize OSD forced free-column candidate solving so order-7 candidate search reuses one GF(2) reduction instead of running full elimination per candidate.

**Architecture:** Add a reusable reduced-system object in `rbposd/src/gf2.rs`, route OSD base and forced candidates through back-substitution, and update `rbposd`/BB profile tests to assert full-elimination count no longer scales with candidate count. Public decoder semantics and deterministic OSD tie-breaking stay unchanged.

**Tech Stack:** Rust 2024, existing `rbposd` GF(2)/OSD modules, existing #212 decode counters, existing `rsinter` BB90 hard fixture.

## Global Constraints

- Preserve public BP-OSD semantics and existing correction/logical predictions.
- Preserve OSD candidate traversal, `OSD_FREE_COLUMN_FRONTIER = 16`, residual-cost comparison, and deterministic correction tie-break.
- `gf2_full_elimination_count <= 1` for an OSD decode that evaluates many forced candidates.
- `gf2_solve_count` may scale with base plus candidate back-substitutions.
- Forced columns must be rejected when they are pivots, out of range, or outside the ordered free set.
- Do not change BP scheduling or BP update rules.
- Required verification commands:
  - `cargo test -p rbposd osd_order7_reuses_factorization_without_changing_correction -- --nocapture`
  - `cargo test -p rbposd checked_in_parity_fixtures_match_exact_expected_outputs -q`
  - `cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture`
  - `cargo test -p rbposd osd_forced_pivot_columns_are_rejected_after_optimization -q`
  - `cargo test`

---

## File Structure

- Modify: `rbposd/src/gf2.rs`
  - Add reusable `ReducedLinearSystem`, keep existing solve helpers as wrappers, and expose crate-private reduced solve helpers for tests and OSD.
- Modify: `rbposd/src/osd.rs`
  - Factor once per OSD target and evaluate base/candidate solutions by back-substitution.
- Modify: `rbposd/tests/osd.rs`
  - Add the order-7 optimization regression and update existing counter expectations.
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`
  - Update profile counter expectations for one full elimination.

## Task 1: Red Tests For Optimized Counter Semantics

**Files:**
- Modify: `rbposd/tests/osd.rs`
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Consumes: existing `BpOsdDecoder`, `DecodeStats`, and BB90 profile helpers.
- Produces: failing tests that require one full elimination for many OSD candidates.

- [ ] **Step 1: Add rbposd regression tests**

In `rbposd/tests/osd.rs`, add `osd_order7_reuses_factorization_without_changing_correction` using a two-check, ten-bit sparse matrix:

```rust
#[test]
fn osd_order7_reuses_factorization_without_changing_correction() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        2,
        10,
        vec![vec![0, 2, 3, 4, 5, 6, 7, 8, 9], vec![1, 2, 3, 4, 5, 6, 7, 8, 9]],
    )
    .unwrap();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.2, 0.2, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08]),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true, true])).unwrap();

    assert_eq!(result.correction, Correction::from(vec![false, false, false, false, false, false, false, false, false, true]));
    assert_eq!(pcm.multiply(&result.correction), Syndrome::from(vec![true, true]));
    assert!(result.used_osd);
    assert!(result.stats.osd_candidate_count > 1);
    assert_eq!(result.stats.gf2_solve_count, result.stats.osd_candidate_count + 1);
    assert!(result.stats.gf2_full_elimination_count <= 1);
}
```

Also add `osd_forced_pivot_columns_are_rejected_after_optimization` in the same file using `rbposd::dev::gf2::PreparedLinearSystem` if needed, or a crate-private unit test in `rbposd/src/gf2.rs` if integration visibility is insufficient. The test must assert pivot, out-of-range, and outside-ordered-free forced columns return errors.

- [ ] **Step 2: Update existing counter expectations to the new target behavior**

Change existing OSD counter assertions so order > 0 profile/decode checks expect `gf2_solve_count >= osd_candidate_count + 1` and `gf2_full_elimination_count == 1`, not equality between solve count and full-elimination count.

- [ ] **Step 3: Run RED**

Run:

```sh
cargo test -p rbposd osd_order7_reuses_factorization_without_changing_correction -- --nocapture
cargo test -p rbposd osd_forced_pivot_columns_are_rejected_after_optimization -q
```

Expected: the order-7 test fails because full elimination still scales with candidate count. The negative-control test may pass for the legacy solver but must remain as a guard for the optimized path.

## Task 2: Reusable GF(2) Reduced System

**Files:**
- Modify: `rbposd/src/gf2.rs`

**Interfaces:**
- Produces: `PreparedLinearSystem::reduce_with_column_order_counting(...) -> Result<ReducedLinearSystem, DecodeError>`.
- Produces: `ReducedLinearSystem::solve_with_forced_columns_counting(...) -> Result<DetailedSolution, DecodeError>`.
- Keeps: existing `solve_with_column_order*` wrappers.

- [ ] **Step 1: Add the reduced-system type**

Add a crate-private `ReducedLinearSystem` storing `rows`, `rhs`, `pivot_columns`, `free_columns`, `is_free`, and `num_bits`. Derive `Debug` and `Clone`.

- [ ] **Step 2: Extract elimination into `reduce_with_column_order_counting`**

Move the current row-reduction part of `solve_with_column_order_detailed_counting` into a method that increments `full_elimination_count` once, populates scratch rows/RHS/pivots, checks singular trailing rows, builds ordered free columns from the supplied `column_order`, and returns `ReducedLinearSystem`.

- [ ] **Step 3: Add forced-column back-substitution**

Implement `ReducedLinearSystem::solve_with_forced_columns_counting` so it increments `solve_count` once, rejects invalid forced columns, assigns forced free variables, back-substitutes pivot variables in reverse pivot order, and returns `DetailedSolution`.

- [ ] **Step 4: Keep legacy solves behavior-compatible**

Make `solve_with_column_order_detailed_counting` call reduce once and solve once. Existing direct GF(2) unit tests should still report one solve and one full elimination for one detailed solve call.

- [ ] **Step 5: Run focused GF(2) tests**

Run:

```sh
cargo test -p rbposd gf2 -q
```

Expected: all GF(2) unit tests pass.

## Task 3: Route OSD Candidate Search Through One Reduction

**Files:**
- Modify: `rbposd/src/osd.rs`

**Interfaces:**
- Consumes: `ReducedLinearSystem`.
- Keeps: `OsdDecodeOutcome`, `OsdDecodeStats`, and `profile_osd_with_workspace`.

- [ ] **Step 1: Factor once in `decode_osd_with_workspace`**

After target syndrome and column ordering are computed, call `reduce_with_column_order_counting` once, accumulate full-elimination stats, solve the base assignment, then pass the reduced system into candidate search.

- [ ] **Step 2: Factor once in bounded profile helper**

Apply the same one-reduction pattern in `profile_osd_with_workspace` so the BB90 diagnostic path measures the optimized candidate behavior too.

- [ ] **Step 3: Change candidate evaluation**

Update `best_osd_candidate` to accept the reduced system and call `solve_with_forced_columns_counting` for each candidate instead of calling the full detailed solve helper.

- [ ] **Step 4: Run GREEN focused checks**

Run:

```sh
cargo test -p rbposd osd_order7_reuses_factorization_without_changing_correction -- --nocapture
cargo test -p rbposd osd_forced_pivot_columns_are_rejected_after_optimization -q
cargo test -p rbposd -q
```

Expected: all pass, with the order-7 test showing candidate count greater than one and full elimination count at most one.

## Task 4: BB90 Smoke And Final Verification

**Files:**
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`
- Review: all touched files.

**Interfaces:**
- Produces: PR-ready branch.

- [ ] **Step 1: Update BB90 profile assertions**

Change BB90 hard-fixture assertions so `gf2_solve_count >= osd_candidate_count + 1` remains true and `gf2_full_elimination_count <= 1` is required for the candidate-limited replay.

- [ ] **Step 2: Format**

Run:

```sh
cargo fmt
```

Expected: exits 0.

- [ ] **Step 3: Run required verification**

Run:

```sh
cargo test -p rbposd osd_order7_reuses_factorization_without_changing_correction -- --nocapture
cargo test -p rbposd checked_in_parity_fixtures_match_exact_expected_outputs -q
cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture
cargo test -p rbposd osd_forced_pivot_columns_are_rejected_after_optimization -q
cargo test
```

Expected: all pass.

- [ ] **Step 4: Review and finish**

Use `superpowers:requesting-code-review`, fix any Critical or Important findings, then use `superpowers:verification-before-completion` and `superpowers:finishing-a-development-branch`. Per standing policy, choose "Push and create a Pull Request" and stop after PR creation.

## Plan Self-Review

Spec coverage: all goals map to Tasks 1-4. Placeholder scan: no placeholder
requirements. Type consistency: `ReducedLinearSystem` and the counter semantics
are named consistently across tasks.
