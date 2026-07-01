# Issue 353 Bit-Packed CSS Span Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Use bit-packed reduced-row-span checks in the random-window CSS component filter while preserving the dense filter verdict semantics and final witness validation.

**Architecture:** Add a reusable packed reduced-row-span representation to `qec-code/src/gf2.rs`. Build packed CSS component filters once in `random_window_css_upper_bound`, pack dense candidates at the filter boundary because #352 is not present, and keep the dense verdict helper as the private reference path for tests.

**Tech Stack:** Rust 2024, existing `qec-code` GF(2) helpers, `BitPackedRow`, Cargo tests, Makefile benchmark smoke target.

## Global Constraints

- Accepted and rejected component candidates must match the current dense `css_component_candidate_verdict` exactly.
- The verdict path must return `Accepted`, `Zero`, `NonKernel`, or `StabilizerSpan`.
- Keep `css_component_candidate_verdict` available as a dense private reference path for tests.
- Preserve `search_stats.stabilizer_span_candidates_rejected`, `witness_validation_candidates_rejected`, `valid_witnesses_found`, and `weight_pruned_candidates` semantics.
- Preserve final `validate_random_window_upper_bound_result` validation before serialization.
- Do not change random-window sampling, seed semantics, target-weight behavior, benchmark manifests, no-target output semantics, upper-bound semantics, or `randomized-upper-bound`.
- Do not add external GF(2), M4RI, `dist-m4ri`, `QDistRnd`, or `codeDistancePYPI` dependencies.
- Because #352 is not visible locally, pack dense candidates at the component-filter boundary.
- Run `cargo test -p qec-code random_window_bitpacked_component_filter_matches_dense_filter -q`.
- Run `cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q`.
- Run `cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q`.
- Run `cargo test -p qec-code random_window_bitpacked_component_filter_rejects_tail_bit_and_span_false_positive_cases -q`.
- Run `make qec-code-random-window-bench-no-target-ladder-smoke`.
- Run `cargo test`.

---

## File Structure

- Modify `qec-code/src/gf2.rs`: add `PackedReducedRows`, logical bit access on `BitPackedRow`, and packed reduced-row-span membership tests.
- Modify `qec-code/src/distance_bound.rs`: add `PackedCssComponentFilter`, packed verdict helper, packed filter construction in `random_window_css_upper_bound`, and issue-required tests.
- No new modules, public APIs, dependencies, or benchmark manifest files are required.

### Task 1: Packed Reduced Row Span

**Files:**
- Modify: `qec-code/src/gf2.rs`
- Test: `qec-code/src/gf2.rs`

**Interfaces:**
- Consumes: `BitPackedRow`, `ReducedRows`, `QecError`, `Result`.
- Produces: `pub(crate) struct PackedReducedRows`, `pub(crate) fn try_in_packed_reduced_row_span(reduced: &PackedReducedRows, target: &BitPackedRow) -> Result<bool>`, and `BitPackedRow::bit(index: usize) -> Result<u8>`.

- [ ] **Step 1: Write failing tests**

Add the following tests inside `qec-code/src/gf2.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn packed_reduced_row_span_membership_matches_dense_membership() {
    let reduced = super::try_rref_with_width(
        &[
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        ],
        65,
    )
    .unwrap();
    let packed = PackedReducedRows::try_from_reduced_rows(&reduced).unwrap();
    let member = BitPackedRow::try_from_dense(&{
        let mut row = vec![0; 65];
        row[0] = 1;
        row[2] = 1;
        row
    }, 65).unwrap();
    let nonmember = BitPackedRow::try_from_dense(&{
        let mut row = vec![0; 65];
        row[3] = 1;
        row
    }, 65).unwrap();

    assert_eq!(try_in_packed_reduced_row_span(&packed, &member), Ok(true));
    assert_eq!(try_in_packed_reduced_row_span(&packed, &nonmember), Ok(false));
}

#[test]
fn packed_reduced_row_span_membership_ignores_target_padding_bits() {
    let reduced = super::try_rref_with_width(&[vec![1, 0, 0]], 3).unwrap();
    let packed = PackedReducedRows::try_from_reduced_rows(&reduced).unwrap();
    let mut zero = BitPackedRow::zeros(3);
    zero.set_storage_padding_for_test();

    assert_eq!(try_in_packed_reduced_row_span(&packed, &zero), Ok(true));
}
```

The exact dense row construction can be shortened with local helper functions if readability is better, but the tests must cover width 65 and dirty target padding.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p qec-code packed_reduced_row_span_membership_matches_dense_membership -q
cargo test -p qec-code packed_reduced_row_span_membership_ignores_target_padding_bits -q
```

Expected: compile failure because `PackedReducedRows`, `BitPackedRow::bit`, or `try_in_packed_reduced_row_span` is missing.

- [ ] **Step 3: Implement packed span membership**

Add the implementation in `qec-code/src/gf2.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackedReducedRows {
    rows: Vec<BitPackedRow>,
    pivot_cols: Vec<usize>,
    width: usize,
}

impl PackedReducedRows {
    pub(crate) fn try_from_reduced_rows(reduced: &ReducedRows) -> Result<Self> {
        let rows = reduced
            .rows
            .iter()
            .map(|row| BitPackedRow::try_from_dense(row, reduced.width))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            rows,
            pivot_cols: reduced.pivot_cols.clone(),
            width: reduced.width,
        })
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}

impl BitPackedRow {
    pub(crate) fn bit(&self, index: usize) -> Result<u8> {
        if index >= self.width {
            return Err(QecError::RowWidthMismatch {
                expected: self.width,
                actual: index + 1,
            });
        }
        Ok(u8::from(((self.words[index / 64] >> (index % 64)) & 1) == 1))
    }
}

pub(crate) fn try_in_packed_reduced_row_span(
    reduced: &PackedReducedRows,
    target: &BitPackedRow,
) -> Result<bool> {
    if target.width() != reduced.width {
        return Err(QecError::RowWidthMismatch {
            expected: reduced.width,
            actual: target.width(),
        });
    }

    let mut remainder = target.clone();
    for (pivot_row, pivot_col) in reduced.pivot_cols.iter().copied().enumerate() {
        if remainder.bit(pivot_col)? == 1 {
            remainder.xor_assign(&reduced.rows[pivot_row])?;
        }
    }

    Ok(remainder.is_zero())
}
```

- [ ] **Step 4: Run focused GREEN tests**

Run the two tests from Step 2. Expected: both pass.

### Task 2: Packed CSS Component Filter

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Test: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: `gf2::BitPackedRow`, `gf2::PackedReducedRows`, `gf2::try_in_packed_reduced_row_span`, `CssComponentCandidateVerdict`.
- Produces: `PackedCssComponentFilter::try_new`, `bitpacked_css_component_candidate_verdict`, and a candidate loop using the packed verdict path.

- [ ] **Step 1: Write failing tests**

Add or update tests in `qec-code/src/distance_bound.rs`:

```rust
#[test]
fn random_window_bitpacked_component_filter_matches_dense_filter() {
    for code_id in ["surface_rotated:d=3", "bb72"] {
        let css = css_from_built_in_code_id(code_id);
        let width = css.code().n();
        let identity_permutation = (0..width).collect::<Vec<_>>();

        for (component, kernel_checks, component_span_rows) in [
            (ComponentKind::XLike, css.hz(), css.hx()),
            (ComponentKind::ZLike, css.hx(), css.hz()),
        ] {
            let component_span = gf2::try_rref_with_width(component_span_rows, width).unwrap();
            let packed_filter =
                PackedCssComponentFilter::try_new(kernel_checks, &component_span).unwrap();
            let mut candidates = component_filter_reference_candidates(
                kernel_checks,
                component_span_rows,
                width,
                &identity_permutation,
            );

            for candidate in candidates.drain(..) {
                let dense_verdict =
                    css_component_candidate_verdict(kernel_checks, &component_span, &candidate)
                        .unwrap();
                let packed_candidate = gf2::BitPackedRow::try_from_dense(&candidate, width).unwrap();
                let packed_verdict =
                    bitpacked_css_component_candidate_verdict(&packed_filter, &packed_candidate)
                        .unwrap();

                assert_eq!(
                    packed_verdict, dense_verdict,
                    "{code_id} {component:?} candidate {candidate:?}"
                );
            }
        }
    }
}

#[test]
fn random_window_bitpacked_component_filter_rejects_tail_bit_and_span_false_positive_cases() {
    let span = gf2::try_rref_with_width(&[{
        let mut row = vec![0; 65];
        row[0] = 1;
        row
    }], 65).unwrap();
    let packed_filter = PackedCssComponentFilter::try_new(&[], &span).unwrap();

    let mut dirty_zero = gf2::BitPackedRow::zeros(65);
    dirty_zero.set_storage_padding_for_test();
    assert_eq!(
        bitpacked_css_component_candidate_verdict(&packed_filter, &dirty_zero).unwrap(),
        CssComponentCandidateVerdict::Zero
    );

    let nonmember_same_word = gf2::BitPackedRow::try_from_dense(&{
        let mut row = vec![0; 65];
        row[1] = 1;
        row
    }, 65).unwrap();
    assert_eq!(
        bitpacked_css_component_candidate_verdict(&packed_filter, &nonmember_same_word).unwrap(),
        CssComponentCandidateVerdict::Accepted
    );

    let nonkernel_filter = PackedCssComponentFilter::try_new(&[{
        let mut row = vec![0; 65];
        row[1] = 1;
        row
    }], &gf2::try_rref_with_width(&[], 65).unwrap()).unwrap();
    assert_eq!(
        bitpacked_css_component_candidate_verdict(&nonkernel_filter, &nonmember_same_word).unwrap(),
        CssComponentCandidateVerdict::NonKernel
    );
}
```

Add a private test helper if needed:

```rust
fn component_filter_reference_candidates(
    kernel_checks: &[Vec<u8>],
    component_span_rows: &[Vec<u8>],
    width: usize,
    permutation: &[usize],
) -> Vec<Vec<u8>> {
    let mut candidates = Vec::new();
    candidates.push(vec![0; width]);
    candidates.push(first_non_kernel_candidate(kernel_checks, width));
    if let Some(span_row) = component_span_rows.first() {
        candidates.push(span_row.clone());
    }
    candidates.extend(
        gf2::try_random_window_kernel_basis_with_width(kernel_checks, width, permutation).unwrap(),
    );
    candidates
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p qec-code random_window_bitpacked_component_filter_matches_dense_filter -q
cargo test -p qec-code random_window_bitpacked_component_filter_rejects_tail_bit_and_span_false_positive_cases -q
```

Expected: compile failure because `PackedCssComponentFilter` and `bitpacked_css_component_candidate_verdict` are missing.

- [ ] **Step 3: Implement packed filter path**

Add a private filter and verdict helper near `css_component_candidate_verdict`:

```rust
struct PackedCssComponentFilter {
    opposite_checks: Vec<gf2::BitPackedRow>,
    stabilizer_component_span: gf2::PackedReducedRows,
}

impl PackedCssComponentFilter {
    fn try_new(
        opposite_checks: &[Vec<u8>],
        stabilizer_component_span: &gf2::ReducedRows,
    ) -> Result<Self> {
        let width = stabilizer_component_span.width;
        gf2::validate_rows_with_width(opposite_checks, width)?;
        let opposite_checks = opposite_checks
            .iter()
            .map(|row| gf2::BitPackedRow::try_from_dense(row, width))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            opposite_checks,
            stabilizer_component_span: gf2::PackedReducedRows::try_from_reduced_rows(
                stabilizer_component_span,
            )?,
        })
    }

    fn width(&self) -> usize {
        self.stabilizer_component_span.width()
    }
}

fn bitpacked_css_component_candidate_verdict(
    filter: &PackedCssComponentFilter,
    candidate: &gf2::BitPackedRow,
) -> Result<CssComponentCandidateVerdict> {
    if candidate.width() != filter.width() {
        return Err(QecError::RowWidthMismatch {
            expected: filter.width(),
            actual: candidate.width(),
        });
    }
    if candidate.is_zero() {
        return Ok(CssComponentCandidateVerdict::Zero);
    }
    for check in &filter.opposite_checks {
        if check.dot_parity(candidate)? != 0 {
            return Ok(CssComponentCandidateVerdict::NonKernel);
        }
    }
    if gf2::try_in_packed_reduced_row_span(&filter.stabilizer_component_span, candidate)? {
        return Ok(CssComponentCandidateVerdict::StabilizerSpan);
    }
    Ok(CssComponentCandidateVerdict::Accepted)
}
```

Construct one packed filter for X-like and one for Z-like rows in
`random_window_css_upper_bound`, then pass the relevant filter into
`consider_component_candidates` and `consider_component_candidate_rows`.
Inside `consider_component_candidate_rows`, replace dense weight and dense
verdict checks with:

```rust
let packed_candidate = gf2::BitPackedRow::try_from_dense(candidate, component_filter.width())?;
let candidate_weight = packed_candidate.weight();
...
let component_verdict =
    bitpacked_css_component_candidate_verdict(component_filter, &packed_candidate)?;
```

Keep dense `css_component_candidate_verdict` unchanged for reference tests.

- [ ] **Step 4: Run focused GREEN tests**

Run the two tests from Step 2. Expected: both pass.

### Task 3: Preserve Full Validation Semantics And Search Stats

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Test: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: task 2 packed filter path.
- Produces: updated existing tests proving packed accepted rows match full witness validation and hand-built rejected rows cannot update best witnesses.

- [ ] **Step 1: Update focused tests**

Update `random_window_component_filter_matches_full_witness_validation` to call
the packed filter and compare against the full validator for every candidate
returned by `component_filter_reference_candidates`.

Update `random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates`
so its calls to `consider_component_candidate_rows` pass packed component
filters and continue asserting:

```rust
assert!(x_best.is_none());
assert_eq!(x_stats.witness_validation_candidates_rejected, 1);
assert_eq!(x_stats.stabilizer_span_candidates_rejected, 1);
assert_eq!(x_stats.valid_witnesses_found, 0);
assert_eq!(x_stats.best_witness_updates, 0);

assert!(z_best.is_none());
assert_eq!(z_stats.witness_validation_candidates_rejected, 1);
assert_eq!(z_stats.stabilizer_span_candidates_rejected, 1);
assert_eq!(z_stats.valid_witnesses_found, 0);
assert_eq!(z_stats.best_witness_updates, 0);
```

- [ ] **Step 2: Run tests to verify RED or affected compile failure**

Run:

```bash
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q
```

Expected: after task 2 signature changes, tests fail to compile until all call sites are updated.

- [ ] **Step 3: Finish call-site updates**

Update all `consider_component_candidates` and `consider_component_candidate_rows`
call sites to pass `&PackedCssComponentFilter`. Preserve existing counter
increments and final result validation.

- [ ] **Step 4: Run issue-specific GREEN tests**

Run:

```bash
cargo test -p qec-code random_window_bitpacked_component_filter_matches_dense_filter -q
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q
cargo test -p qec-code random_window_bitpacked_component_filter_rejects_tail_bit_and_span_false_positive_cases -q
```

Expected: all four tests pass.

### Task 4: Verification And Commit

**Files:**
- Modify: `qec-code/src/gf2.rs`
- Modify: `qec-code/src/distance_bound.rs`
- Create: `docs/superpowers/specs/2026-07-01-issue-353-bitpacked-css-span-filter-design.md`
- Create: `docs/superpowers/plans/2026-07-01-issue-353-bitpacked-css-span-filter.md`

**Interfaces:**
- Consumes: all earlier tasks.
- Produces: verified implementation ready for PR.

- [ ] **Step 1: Run focused and benchmark verification**

Run:

```bash
cargo test -p qec-code random_window_bitpacked_component_filter_matches_dense_filter -q
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates -q
cargo test -p qec-code random_window_bitpacked_component_filter_rejects_tail_bit_and_span_false_positive_cases -q
cargo test -p qec-code gf2 -q
make qec-code-random-window-bench-no-target-ladder-smoke
cargo test
```

Expected: all commands exit 0. The no-target ladder smoke summary should still report surface_rotated_d5 = 5, toric_d5 = 5, bb72 = 6, and bb144 = 12; target fields remain no-target/null; summary markdown still includes the human-readable `span=` timing bucket.

- [ ] **Step 2: Inspect outputs and git diff**

Run:

```bash
git status --short
git diff -- qec-code/src/gf2.rs qec-code/src/distance_bound.rs docs/superpowers/specs/2026-07-01-issue-353-bitpacked-css-span-filter-design.md docs/superpowers/plans/2026-07-01-issue-353-bitpacked-css-span-filter.md
```

Expected: only scoped files are modified or created.

- [ ] **Step 3: Commit**

Run:

```bash
git add qec-code/src/gf2.rs qec-code/src/distance_bound.rs docs/superpowers/specs/2026-07-01-issue-353-bitpacked-css-span-filter-design.md docs/superpowers/plans/2026-07-01-issue-353-bitpacked-css-span-filter.md
git commit -m "feat: use bit-packed CSS span filtering"
```

Expected: commit succeeds.

## Plan Self-Review

- Spec coverage: packed GF(2) span membership, packed component filter, dense equivalence, full validator equivalence, stats preservation, no-target ladder smoke, and no external dependency constraints all map to tasks.
- Placeholder scan: no unresolved marker entries.
- Type consistency: `PackedReducedRows`, `PackedCssComponentFilter`, and `bitpacked_css_component_candidate_verdict` signatures are consistent across tasks.
- Scope check: the plan does not alter sampling, seeds, manifests, final validation, or `randomized-upper-bound`.
