# Issue 139 APM Sparse Matrix Verifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable APM sparse-row verifier for rank, orthogonality, degree statistics, and girth status, then prove it on native/generated P=96 matrices.

**Architecture:** Keep the verifier test-visible under `qec-code/tests/support` so later APM tests can import it without changing the production public API. Include the same support file from the private `qec-code/src/codes/apm.rs` unit tests to verify the crate-private native builder from #138. Refactor existing APM fixture checks in `qec-code/tests/code.rs` to use the shared verifier instead of local ad hoc checks.

**Tech Stack:** Rust 2024, `qec-code` integration/unit tests, existing `SparseRowsMatrix` sparse-row shape, `qec_code::binary::try_binary_rank`, deterministic BFS for Tanner graph girth.

## Global Constraints

- Keep the verifier test-visible; do not add a public production verifier API.
- Do not register CLI or catalog support for `apm_kasai:p=96`.
- Input is `Hx` and `Hz` sparse-row matrices plus expected stats from the APM manifest/contract.
- Output must report `num_cols`, X/Z row counts, row-weight min/avg/max, column-weight min/avg/max, `rank_x`, `rank_z`, `k`, `orthogonal`, and a girth lower-bound/status for X and Z Tanner graphs.
- Reject duplicate row support entries and out-of-range support entries before reporting rank or girth.
- For P=96, assert `orthogonal == true`, `n=1152`, `mx=mz=288`, `k=580`, every X/Z row weight is `12`, every X/Z column weight is `3`, and both X/Z girth statuses satisfy the manifest lower bound `6`.
- Use `qec_code::binary::try_binary_rank` for rank computation.
- Keep girth computation bounded and deterministic; use BFS shortest-cycle checks for these regular fixtures.
- Run `cargo test -p qec-code apm_p96_verifier_reports_paper_stats -q`.
- Run `cargo test`.

---

## File Structure

- Create `qec-code/tests/support/mod.rs`: shared integration-test support module entrypoint.
- Create `qec-code/tests/support/apm_verifier.rs`: reusable APM sparse-row verifier and report structs.
- Modify `qec-code/tests/code.rs`: import the support module and replace local fixture structural checks with the shared verifier.
- Modify `qec-code/src/lib.rs`: add a test-only `extern crate self as qec_code;` alias so the support file can be included from unit tests and integration tests with the same `qec_code::...` paths.
- Modify `qec-code/src/codes/apm.rs`: include the support verifier in the private test module and add the required native P=96 verifier test.

### Task 1: Shared APM Verifier Support

**Files:**
- Create: `qec-code/tests/support/mod.rs`
- Create: `qec-code/tests/support/apm_verifier.rs`
- Modify: `qec-code/tests/code.rs`

**Interfaces:**
- Consumes: `qec_code::binary::try_binary_rank`.
- Produces: `ApmSparseMatrixView<'a> { name: &'static str, num_cols: usize, rows: &'a [Vec<usize>] }`.
- Produces: `WeightStats { min: usize, average: f64, max: usize }`.
- Produces: `GirthStatus::{Exact(usize), AtLeast(usize), Acyclic}` with `meets_lower_bound(&self, expected: usize) -> bool`.
- Produces: `ApmSparseMatrixReport { num_cols, num_rows, row_weight, column_weight, rank, girth }`.
- Produces: `ApmCssVerifierExpectations` with optional fields for `num_cols`, `mx`, `mz`, exact row/column weights, `k`, `orthogonal`, and `girth_lower_bound`.
- Produces: `ApmCssVerifierReport { num_cols, mx, mz, x, z, rank_x, rank_z, k, orthogonal }`.
- Produces: `verify_apm_css_matrices(hx, hz, expectations) -> Result<ApmCssVerifierReport, String>`.

- [ ] **Step 1: Write the failing integration refactor**

Add this near the top of `qec-code/tests/code.rs`:

```rust
mod support;

use support::apm_verifier::{
    ApmCssVerifierExpectations, ApmSparseMatrixView, GirthStatus, verify_apm_css_matrices,
};
```

Replace `assert_apm_p96_fixture_stats` with:

```rust
fn apm_p96_expectations() -> ApmCssVerifierExpectations {
    ApmCssVerifierExpectations {
        num_cols: Some(1152),
        mx: Some(288),
        mz: Some(288),
        row_weight_x: Some(12),
        row_weight_z: Some(12),
        column_weight_x: Some(3),
        column_weight_z: Some(3),
        k: Some(580),
        orthogonal: Some(true),
        girth_lower_bound: Some(6),
    }
}

fn verify_apm_p96_fixture_stats(
    hx: &ApmSparseFixture,
    hz: &ApmSparseFixture,
) -> std::result::Result<support::apm_verifier::ApmCssVerifierReport, String> {
    verify_apm_css_matrices(
        ApmSparseMatrixView {
            name: "Hx",
            num_cols: hx.num_cols,
            rows: &hx.rows,
        },
        ApmSparseMatrixView {
            name: "Hz",
            num_cols: hz.num_cols,
            rows: &hz.rows,
        },
        &apm_p96_expectations(),
    )
}
```

Update existing APM fixture tests to call `verify_apm_p96_fixture_stats`. In `apm_p96_fixture_matches_reference_stats`, assert the returned report fields:

```rust
let report = verify_apm_p96_fixture_stats(&hx, &hz).unwrap();
assert!(report.orthogonal);
assert_eq!(report.num_cols, 1152);
assert_eq!(report.mx, 288);
assert_eq!(report.mz, 288);
assert_eq!(report.k, 580);
assert_eq!(report.x.row_weight, support::apm_verifier::WeightStats { min: 12, average: 12.0, max: 12 });
assert_eq!(report.z.row_weight, support::apm_verifier::WeightStats { min: 12, average: 12.0, max: 12 });
assert_eq!(report.x.column_weight, support::apm_verifier::WeightStats { min: 3, average: 3.0, max: 3 });
assert_eq!(report.z.column_weight, support::apm_verifier::WeightStats { min: 3, average: 3.0, max: 3 });
assert!(matches!(report.x.girth, GirthStatus::Exact(girth) if girth >= 6));
assert!(matches!(report.z.girth, GirthStatus::Exact(girth) if girth >= 6));
```

Update negative-control calls from `assert_apm_p96_fixture_stats` to `verify_apm_p96_fixture_stats`.

- [ ] **Step 2: Run focused tests to verify the helper is missing**

Run:

```sh
cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q
```

Expected: FAIL at compile time because `qec-code/tests/support/mod.rs` and the verifier types do not exist yet.

- [ ] **Step 3: Add the support module entrypoint**

Create `qec-code/tests/support/mod.rs`:

```rust
pub mod apm_verifier;
```

- [ ] **Step 4: Add the verifier implementation**

Create `qec-code/tests/support/apm_verifier.rs` with these public types and functions:

```rust
use std::collections::VecDeque;

use qec_code::binary::try_binary_rank;

#[derive(Debug, Clone, Copy)]
pub struct ApmSparseMatrixView<'a> {
    pub name: &'static str,
    pub num_cols: usize,
    pub rows: &'a [Vec<usize>],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightStats {
    pub min: usize,
    pub average: f64,
    pub max: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GirthStatus {
    Exact(usize),
    AtLeast(usize),
    Acyclic,
}

impl GirthStatus {
    pub fn meets_lower_bound(self, expected: usize) -> bool {
        match self {
            Self::Exact(value) | Self::AtLeast(value) => value >= expected,
            Self::Acyclic => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApmSparseMatrixReport {
    pub num_cols: usize,
    pub num_rows: usize,
    pub row_weight: WeightStats,
    pub column_weight: WeightStats,
    pub rank: usize,
    pub girth: GirthStatus,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApmCssVerifierExpectations {
    pub num_cols: Option<usize>,
    pub mx: Option<usize>,
    pub mz: Option<usize>,
    pub row_weight_x: Option<usize>,
    pub row_weight_z: Option<usize>,
    pub column_weight_x: Option<usize>,
    pub column_weight_z: Option<usize>,
    pub k: Option<usize>,
    pub orthogonal: Option<bool>,
    pub girth_lower_bound: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApmCssVerifierReport {
    pub num_cols: usize,
    pub mx: usize,
    pub mz: usize,
    pub x: ApmSparseMatrixReport,
    pub z: ApmSparseMatrixReport,
    pub rank_x: usize,
    pub rank_z: usize,
    pub k: usize,
    pub orthogonal: bool,
}
```

Implement validation before rank/girth:

```rust
fn validate_sparse_matrix(matrix: ApmSparseMatrixView<'_>) -> Result<(), String> {
    if matrix.num_cols == 0 {
        return Err(format!("{} has invalid sparse-rows width 0", matrix.name));
    }
    for (row_index, row) in matrix.rows.iter().enumerate() {
        let mut sorted = row.clone();
        sorted.sort_unstable();
        for pair in sorted.windows(2) {
            if pair[0] == pair[1] {
                return Err(format!(
                    "{} row {row_index} contains duplicate support {}",
                    matrix.name, pair[0]
                ));
            }
        }
        for &support in row {
            if support >= matrix.num_cols {
                return Err(format!(
                    "{} row {row_index} contains out-of-range support {support} for width {}",
                    matrix.name, matrix.num_cols
                ));
            }
        }
    }
    Ok(())
}
```

Implement `verify_apm_css_matrices` so it calls `validate_sparse_matrix` on both inputs, checks shared width, computes matrix reports, checks sparse orthogonality, derives `k` with `checked_sub`, applies each expectation, and returns descriptive `Err(String)` values such as `"expected k=580, got 579"` or `"expected Hx row weight 12, got min/avg/max 11/11.99/12"`.

Implement helpers:

```rust
fn dense_rows(matrix: ApmSparseMatrixView<'_>) -> Vec<Vec<u8>>;
fn weight_stats(weights: &[usize]) -> WeightStats;
fn column_weights(matrix: ApmSparseMatrixView<'_>) -> Vec<usize>;
fn row_weight_report_matches(stats: WeightStats, expected: usize) -> bool;
fn sparse_rows_are_orthogonal(hx: ApmSparseMatrixView<'_>, hz: ApmSparseMatrixView<'_>) -> bool;
fn tanner_girth(matrix: ApmSparseMatrixView<'_>) -> GirthStatus;
fn shortest_cycle_from(start: usize, graph: &[Vec<usize>]) -> Option<usize>;
```

`tanner_girth` must build a bipartite graph with row nodes `0..rows.len()` and column nodes `rows.len()..rows.len()+num_cols`, run BFS from every node, and return `GirthStatus::Exact(best)` for the shortest cycle or `GirthStatus::Acyclic` when none exists.

- [ ] **Step 5: Run focused integration tests to verify green**

Run:

```sh
cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q
```

Expected: PASS.

Run:

```sh
cargo test -p qec-code apm_p96_fixture_rejects_mutated_support apm_p96_fixture_rejects_structural_stat_mismatches apm_p96_fixture_rejects_balanced_nonorthogonal_swap apm_p96_fixture_rejects_low_rank_shape -q
```

Expected: PASS, or if Cargo rejects multiple exact filters in one command, run the four named tests one at a time.

- [ ] **Step 6: Commit**

```sh
git add qec-code/tests/support/mod.rs qec-code/tests/support/apm_verifier.rs qec-code/tests/code.rs
git commit -m "test: add apm sparse verifier helper"
```

### Task 2: Native P=96 Verifier Test

**Files:**
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/codes/apm.rs`

**Interfaces:**
- Consumes: Task 1 `qec-code/tests/support/apm_verifier.rs`.
- Produces: unit test `apm_p96_verifier_reports_paper_stats`.

- [ ] **Step 1: Write the failing native-builder verifier test**

In `qec-code/src/codes/apm.rs` inside `#[cfg(test)] mod tests`, include the support helper:

```rust
    #[path = "../../tests/support/apm_verifier.rs"]
    mod apm_verifier;
```

Add:

```rust
    fn apm_p96_verifier_expectations() -> apm_verifier::ApmCssVerifierExpectations {
        apm_verifier::ApmCssVerifierExpectations {
            num_cols: Some(1152),
            mx: Some(288),
            mz: Some(288),
            row_weight_x: Some(12),
            row_weight_z: Some(12),
            column_weight_x: Some(3),
            column_weight_z: Some(3),
            k: Some(580),
            orthogonal: Some(true),
            girth_lower_bound: Some(6),
        }
    }

    #[test]
    fn apm_p96_verifier_reports_paper_stats() {
        let manifest: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/apm/table_a1_manifest.json"
        ))
        .unwrap();
        let entry = parse_p96_apm_manifest_entry(&manifest);
        let checks = build_apm_css_checks(&entry).unwrap();

        let report = apm_verifier::verify_apm_css_matrices(
            apm_verifier::ApmSparseMatrixView {
                name: "Hx",
                num_cols: checks.num_cols,
                rows: &checks.hx,
            },
            apm_verifier::ApmSparseMatrixView {
                name: "Hz",
                num_cols: checks.num_cols,
                rows: &checks.hz,
            },
            &apm_p96_verifier_expectations(),
        )
        .unwrap();

        assert!(report.orthogonal);
        assert_eq!(report.num_cols, 1152);
        assert_eq!(report.mx, 288);
        assert_eq!(report.mz, 288);
        assert_eq!(report.k, 580);
        assert_eq!(report.rank_x + report.rank_z, 572);
        assert_eq!(report.x.row_weight, apm_verifier::WeightStats { min: 12, average: 12.0, max: 12 });
        assert_eq!(report.z.row_weight, apm_verifier::WeightStats { min: 12, average: 12.0, max: 12 });
        assert_eq!(report.x.column_weight, apm_verifier::WeightStats { min: 3, average: 3.0, max: 3 });
        assert_eq!(report.z.column_weight, apm_verifier::WeightStats { min: 3, average: 3.0, max: 3 });
        assert!(report.x.girth.meets_lower_bound(6));
        assert!(report.z.girth.meets_lower_bound(6));

        let mut duplicate_hx = checks.hx.clone();
        duplicate_hx[0][1] = duplicate_hx[0][0];
        let duplicate_err = apm_verifier::verify_apm_css_matrices(
            apm_verifier::ApmSparseMatrixView { name: "Hx", num_cols: checks.num_cols, rows: &duplicate_hx },
            apm_verifier::ApmSparseMatrixView { name: "Hz", num_cols: checks.num_cols, rows: &checks.hz },
            &apm_p96_verifier_expectations(),
        )
        .unwrap_err();
        assert!(duplicate_err.contains("duplicate support"), "{duplicate_err}");

        let mut out_of_range_hz = checks.hz.clone();
        out_of_range_hz[0][0] = checks.num_cols;
        let range_err = apm_verifier::verify_apm_css_matrices(
            apm_verifier::ApmSparseMatrixView { name: "Hx", num_cols: checks.num_cols, rows: &checks.hx },
            apm_verifier::ApmSparseMatrixView { name: "Hz", num_cols: checks.num_cols, rows: &out_of_range_hz },
            &apm_p96_verifier_expectations(),
        )
        .unwrap_err();
        assert!(range_err.contains("out-of-range support"), "{range_err}");
    }
```

- [ ] **Step 2: Run focused test to verify red**

Run:

```sh
cargo test -p qec-code apm_p96_verifier_reports_paper_stats -q
```

Expected: FAIL at compile time because `qec_code::...` cannot be resolved when including the integration-test support file from a unit test.

- [ ] **Step 3: Add the test-only crate alias**

Add near the top of `qec-code/src/lib.rs`:

```rust
#[cfg(test)]
extern crate self as qec_code;
```

- [ ] **Step 4: Run focused test to verify green**

Run:

```sh
cargo test -p qec-code apm_p96_verifier_reports_paper_stats -q
```

Expected: PASS.

- [ ] **Step 5: Commit**

```sh
git add qec-code/src/lib.rs qec-code/src/codes/apm.rs
git commit -m "test: verify apm p96 sparse matrix stats"
```
