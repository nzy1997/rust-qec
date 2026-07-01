# Issue 346 Random-Window GF(2) Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dense reusable GF(2) workspace for random-window kernel-basis generation and use it in the random-window CSS upper-bound hot path without changing candidate rows or result semantics.

**Architecture:** Implement `gf2::RandomWindowKernelWorkspace` as a crate-private dense scratch object that validates the same inputs as `try_random_window_kernel_basis_with_width`, reuses permuted rows and basis rows across calls, and returns borrowed original-order candidate rows. Keep the existing simple helper by delegating through a temporary workspace, then update `distance_bound.rs` to hold one workspace across random-window iterations and process borrowed candidate slices.

**Tech Stack:** Rust 2024, existing `qec-code` crate-private GF(2) helpers, existing random-window CSS upper-bound implementation, Cargo tests, existing benchmark Make targets.

## Global Constraints

- Do not introduce bit-packed matrix storage in this issue.
- Do not change random-window sampling, permutations, or seed semantics.
- Do not remove the existing simple GF(2) helper API.
- Do not add external GF(2), M4RI, `dist-m4ri`, `QDistRnd`, or `codeDistancePYPI` dependencies.
- Preserve `try_random_window_kernel_basis_with_width(matrix, width, column_permutation) -> Result<Vec<Vec<u8>>>`.
- Workspace output must exactly match `try_random_window_kernel_basis_with_width` byte-for-byte for the same matrix, width, and permutation.
- Invalid binary entries, row-width mismatches, and invalid permutations must return clear errors matching the existing helper.
- `search_stats.kernel_basis_generations` must keep the same counting semantics: one increment per component basis generation attempted.
- Use offline Cargo mode when the sandbox network proxy blocks registry access, and record both requested and offline command outcomes.

---

## File Structure

- Modify `qec-code/src/gf2.rs`: add `RandomWindowKernelWorkspace`, reusable permutation validation scratch, workspace-backed dense RREF/nullspace generation, and workspace tests.
- Modify `qec-code/src/distance_bound.rs`: allocate and thread one workspace through `random_window_css_upper_bound`, change candidate-row processing to borrow slices, and add a focused integration test that passes workspace output directly to the candidate loop.
- Create no new runtime dependencies.

### Task 1: GF(2) Random-Window Workspace

**Files:**
- Modify: `qec-code/src/gf2.rs`

**Interfaces:**
- Consumes: existing `BinaryRow`, `validate_rows_with_width`, `QecError`, `Result`, and current `try_random_window_kernel_basis_with_width` behavior.
- Produces: `pub(crate) struct RandomWindowKernelWorkspace` with `new()` and `try_kernel_basis_with_width(&mut self, matrix: &[BinaryRow], width: usize, column_permutation: &[usize]) -> Result<&[BinaryRow]>`; preserves `try_random_window_kernel_basis_with_width(...) -> Result<Vec<BinaryRow>>`.

- [ ] **Step 1: Write the failing workspace tests**

In `qec-code/src/gf2.rs`, update the test import block to include `RandomWindowKernelWorkspace`:

```rust
    use super::{
        try_in_reduced_row_span, try_in_row_span_with_width, try_nullspace_basis_with_width,
        try_random_window_kernel_basis_with_width, try_rank, try_select_independent_rows,
        RandomWindowKernelWorkspace,
    };
```

Then insert these tests after `gf2_random_window_kernel_basis_contract`:

```rust
    #[test]
    fn gf2_random_window_workspace_matches_existing_kernel_basis() {
        let cases = vec![
            (
                Vec::<Vec<u8>>::new(),
                3,
                vec![vec![0, 1, 2], vec![2, 1, 0], vec![1, 2, 0]],
            ),
            (
                vec![vec![1, 1, 0, 0], vec![0, 1, 1, 0]],
                4,
                vec![
                    vec![0, 1, 2, 3],
                    vec![3, 0, 2, 1],
                    vec![2, 3, 1, 0],
                ],
            ),
            (
                vec![
                    vec![1, 0, 1, 1, 0],
                    vec![0, 1, 1, 0, 1],
                    vec![1, 1, 0, 1, 1],
                ],
                5,
                vec![
                    vec![0, 1, 2, 3, 4],
                    vec![4, 2, 0, 3, 1],
                    vec![1, 3, 4, 0, 2],
                ],
            ),
        ];
        let mut workspace = RandomWindowKernelWorkspace::new();

        for (matrix, width, permutations) in cases {
            for permutation in permutations {
                let expected =
                    try_random_window_kernel_basis_with_width(&matrix, width, &permutation)
                        .unwrap();
                let actual = workspace
                    .try_kernel_basis_with_width(&matrix, width, &permutation)
                    .unwrap()
                    .to_vec();

                assert_eq!(actual, expected, "width {width} permutation {permutation:?}");
                assert!(actual.iter().all(|row| row.len() == width));
                for vector in &actual {
                    assert_kernel_vector(&matrix, vector);
                }
            }
        }
    }

    #[test]
    fn gf2_random_window_workspace_reuse_resets_state() {
        let mut workspace = RandomWindowKernelWorkspace::new();

        let wide = vec![
            vec![1, 0, 1, 0, 1],
            vec![0, 1, 1, 1, 0],
            vec![1, 1, 0, 0, 1],
        ];
        let wide_permutation = vec![4, 2, 0, 3, 1];
        let expected_wide =
            try_random_window_kernel_basis_with_width(&wide, 5, &wide_permutation).unwrap();
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&wide, 5, &wide_permutation)
                .unwrap(),
            expected_wide.as_slice()
        );

        let narrow = vec![vec![1, 1], vec![0, 0]];
        let narrow_permutation = vec![1, 0];
        let expected_narrow =
            try_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation).unwrap();
        let actual_narrow = workspace
            .try_kernel_basis_with_width(&narrow, 2, &narrow_permutation)
            .unwrap()
            .to_vec();
        assert_eq!(actual_narrow, expected_narrow);
        assert!(actual_narrow.iter().all(|row| row.len() == 2));
        for vector in &actual_narrow {
            assert_kernel_vector(&narrow, vector);
        }

        let larger = vec![vec![1, 0, 0, 1], vec![0, 1, 1, 0]];
        let larger_permutation = vec![2, 0, 3, 1];
        let expected_larger =
            try_random_window_kernel_basis_with_width(&larger, 4, &larger_permutation).unwrap();
        let actual_larger = workspace
            .try_kernel_basis_with_width(&larger, 4, &larger_permutation)
            .unwrap()
            .to_vec();
        assert_eq!(actual_larger, expected_larger);
        assert!(actual_larger.iter().all(|row| row.len() == 4));
        for vector in &actual_larger {
            assert_kernel_vector(&larger, vector);
        }
    }

    #[test]
    fn gf2_random_window_workspace_rejects_stale_or_invalid_inputs() {
        let mut workspace = RandomWindowKernelWorkspace::new();
        let previous_wide = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 1]];
        let previous_permutation = vec![3, 0, 2, 1];
        workspace
            .try_kernel_basis_with_width(&previous_wide, 4, &previous_permutation)
            .unwrap();

        let duplicate_permutation = vec![0, 1, 1, 3];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&previous_wide, 4, &duplicate_permutation)
                .unwrap_err(),
            try_random_window_kernel_basis_with_width(
                &previous_wide,
                4,
                &duplicate_permutation,
            )
            .unwrap_err()
        );

        let invalid_binary = vec![vec![1, 2, 0, 0]];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&invalid_binary, 4, &[0, 1, 2, 3])
                .unwrap_err(),
            try_random_window_kernel_basis_with_width(&invalid_binary, 4, &[0, 1, 2, 3])
                .unwrap_err()
        );

        let mismatched_width = vec![vec![1, 0, 0, 1], vec![1, 0]];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&mismatched_width, 4, &[0, 1, 2, 3])
                .unwrap_err(),
            try_random_window_kernel_basis_with_width(&mismatched_width, 4, &[0, 1, 2, 3])
                .unwrap_err()
        );

        let narrow = vec![vec![1, 1]];
        let narrow_permutation = vec![1, 0];
        let expected_narrow =
            try_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation).unwrap();
        let actual_narrow = workspace
            .try_kernel_basis_with_width(&narrow, 2, &narrow_permutation)
            .unwrap()
            .to_vec();
        assert_eq!(actual_narrow, expected_narrow);
        assert!(actual_narrow.iter().all(|row| row.len() == 2));
        for vector in &actual_narrow {
            assert_kernel_vector(&narrow, vector);
        }
    }
```

- [ ] **Step 2: Run the new tests to verify RED**

Run:

```bash
cargo test -p qec-code gf2_random_window_workspace_matches_existing_kernel_basis -q
cargo test -p qec-code gf2_random_window_workspace_reuse_resets_state -q
cargo test -p qec-code gf2_random_window_workspace_rejects_stale_or_invalid_inputs -q
```

Expected: each command fails to compile because `RandomWindowKernelWorkspace` is not defined yet. If Cargo tries to update the registry and fails through the sandbox proxy, rerun the same commands with `--offline`.

- [ ] **Step 3: Implement the reusable workspace**

In `qec-code/src/gf2.rs`, insert this struct and implementation after `ReducedRows`:

```rust
#[derive(Debug, Default)]
pub(crate) struct RandomWindowKernelWorkspace {
    permuted_rows: Vec<BinaryRow>,
    permuted_len: usize,
    pivot_cols: Vec<usize>,
    pivot_seen: Vec<bool>,
    permutation_seen: Vec<bool>,
    basis_rows: Vec<BinaryRow>,
    basis_len: usize,
}

impl RandomWindowKernelWorkspace {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn try_kernel_basis_with_width(
        &mut self,
        matrix: &[BinaryRow],
        width: usize,
        column_permutation: &[usize],
    ) -> Result<&[BinaryRow]> {
        self.reset_logical_state();
        validate_rows_with_width(matrix, width)?;
        validate_column_permutation_with_seen(
            column_permutation,
            width,
            &mut self.permutation_seen,
        )?;

        self.fill_permuted_rows(matrix, width, column_permutation);
        self.reduce_permuted_rows(width);
        self.fill_original_order_basis(width, column_permutation);

        Ok(&self.basis_rows[..self.basis_len])
    }

    fn reset_logical_state(&mut self) {
        self.permuted_len = 0;
        self.basis_len = 0;
        self.pivot_cols.clear();
    }

    fn fill_permuted_rows(
        &mut self,
        matrix: &[BinaryRow],
        width: usize,
        column_permutation: &[usize],
    ) {
        self.permuted_len = matrix.len();
        for (row_index, row) in matrix.iter().enumerate() {
            if row_index == self.permuted_rows.len() {
                self.permuted_rows.push(Vec::new());
            }
            let permuted_row = &mut self.permuted_rows[row_index];
            permuted_row.clear();
            permuted_row.resize(width, 0);
            for (permuted_col, &original_col) in column_permutation.iter().enumerate() {
                permuted_row[permuted_col] = row[original_col];
            }
        }
    }

    fn reduce_permuted_rows(&mut self, width: usize) {
        let rows = &mut self.permuted_rows[..self.permuted_len];
        let mut pivot_row = 0;

        for col in 0..width {
            let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row][col] == 1) else {
                continue;
            };
            rows.swap(pivot_row, pivot);

            for row in 0..rows.len() {
                if row != pivot_row && rows[row][col] == 1 {
                    for k in col..width {
                        rows[row][k] ^= rows[pivot_row][k];
                    }
                }
            }

            self.pivot_cols.push(col);
            pivot_row += 1;
            if pivot_row == rows.len() {
                break;
            }
        }
    }

    fn fill_original_order_basis(&mut self, width: usize, column_permutation: &[usize]) {
        self.pivot_seen.clear();
        self.pivot_seen.resize(width, false);
        for &pivot_col in &self.pivot_cols {
            self.pivot_seen[pivot_col] = true;
        }

        let mut basis_len = 0;
        for free_col in 0..width {
            if self.pivot_seen[free_col] {
                continue;
            }
            if basis_len == self.basis_rows.len() {
                self.basis_rows.push(Vec::new());
            }
            let vector = &mut self.basis_rows[basis_len];
            vector.clear();
            vector.resize(width, 0);
            vector[column_permutation[free_col]] = 1;
            for (pivot_row, &pivot_col) in self.pivot_cols.iter().enumerate() {
                if self.permuted_rows[pivot_row][free_col] == 1 {
                    vector[column_permutation[pivot_col]] = 1;
                }
            }
            basis_len += 1;
        }

        self.basis_len = basis_len;
    }
}
```

Replace `try_random_window_kernel_basis_with_width` with:

```rust
pub(crate) fn try_random_window_kernel_basis_with_width(
    matrix: &[BinaryRow],
    width: usize,
    column_permutation: &[usize],
) -> Result<Vec<BinaryRow>> {
    let mut workspace = RandomWindowKernelWorkspace::new();
    let basis = workspace.try_kernel_basis_with_width(matrix, width, column_permutation)?;
    Ok(basis.to_vec())
}
```

Replace `validate_column_permutation` with this reusable-scratch implementation:

```rust
#[allow(dead_code)]
fn validate_column_permutation(column_permutation: &[usize], width: usize) -> Result<()> {
    let mut seen = Vec::new();
    validate_column_permutation_with_seen(column_permutation, width, &mut seen)
}

fn validate_column_permutation_with_seen(
    column_permutation: &[usize],
    width: usize,
    seen: &mut Vec<bool>,
) -> Result<()> {
    if column_permutation.len() != width {
        return Err(QecError::InvalidColumnPermutation {
            reason: format!("expected length {width}, got {}", column_permutation.len()),
        });
    }

    seen.clear();
    seen.resize(width, false);
    for &column in column_permutation {
        if column >= width {
            return Err(QecError::InvalidColumnPermutation {
                reason: format!("column {column} out of range for width {width}"),
            });
        }
        if seen[column] {
            return Err(QecError::InvalidColumnPermutation {
                reason: format!("duplicate column {column}"),
            });
        }
        seen[column] = true;
    }

    Ok(())
}
```

- [ ] **Step 4: Run the workspace tests to verify GREEN**

Run:

```bash
cargo test -p qec-code gf2_random_window_workspace_matches_existing_kernel_basis -q
cargo test -p qec-code gf2_random_window_workspace_reuse_resets_state -q
cargo test -p qec-code gf2_random_window_workspace_rejects_stale_or_invalid_inputs -q
cargo test -p qec-code gf2_random_window -q
```

Expected: all commands pass. If registry access is blocked, rerun with `--offline` before the test name.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add qec-code/src/gf2.rs
git commit -m "feat: add reusable random-window gf2 workspace"
```

Expected: commit succeeds with only `qec-code/src/gf2.rs` staged.

### Task 2: Random-Window Search Workspace Integration

**Files:**
- Modify: `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: `gf2::RandomWindowKernelWorkspace::try_kernel_basis_with_width(...) -> Result<&[BinaryRow]>`.
- Produces: `consider_component_candidates(..., kernel_workspace: &mut gf2::RandomWindowKernelWorkspace, ...) -> Result<()>`, `consider_component_candidate_rows(candidates: &[Vec<u8>], ...) -> Result<()>`, and `component_candidate_to_pauli(component: ComponentKind, candidate: &[u8]) -> Result<Pauli>`.

- [ ] **Step 1: Write the failing integration test**

In `qec-code/src/distance_bound.rs`, insert this test after `random_window_pruning_does_not_skip_strictly_better_candidate`:

```rust
    #[test]
    fn random_window_candidate_rows_accepts_workspace_output_without_stale_rows() {
        let width = 3;
        let component_span = empty_reduced_rows(width);
        let mut workspace = gf2::RandomWindowKernelWorkspace::new();
        let permutation = vec![2, 0, 1];
        let candidates = workspace
            .try_kernel_basis_with_width(&[], width, &permutation)
            .unwrap();
        assert_eq!(
            candidates,
            &[
                vec![0, 0, 1],
                vec![1, 0, 0],
                vec![0, 1, 0],
            ]
        );

        let mut best_witness = Some(x_pauli(width, &[0, 1]));
        let mut search_stats = RandomWindowSearchStats::default();
        consider_component_candidate_rows(
            candidates,
            &[],
            &component_span,
            ComponentKind::XLike,
            &mut best_witness,
            &mut search_stats,
        )
        .unwrap();

        let best = best_witness.expect("workspace candidate should update the best witness");
        assert_eq!(best.weight(), 1);
        assert_eq!(search_stats.component_candidates_generated, 3);
        assert_eq!(search_stats.weight_pruned_candidates, 2);
        assert_eq!(search_stats.valid_witnesses_found, 1);
        assert_eq!(search_stats.best_witness_updates, 1);
    }
```

- [ ] **Step 2: Run the integration test to verify RED**

Run:

```bash
cargo test -p qec-code random_window_candidate_rows_accepts_workspace_output_without_stale_rows -q
```

Expected: the command fails to compile because `consider_component_candidate_rows` still expects an owned `Vec<Vec<u8>>`, not a borrowed workspace slice.

- [ ] **Step 3: Thread the workspace through the search path**

In `random_window_css_upper_bound`, add the workspace before `search_started`:

```rust
    let mut kernel_workspace = gf2::RandomWindowKernelWorkspace::new();
```

Update both `consider_component_candidates` calls to pass `&mut kernel_workspace` immediately after `permutation`:

```rust
                &permutation,
                &mut kernel_workspace,
                &mut best_witness,
                &mut search_stats,
```

Change the `consider_component_candidates` signature and kernel-basis call:

```rust
fn consider_component_candidates(
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    width: usize,
    permutation: &[usize],
    kernel_workspace: &mut gf2::RandomWindowKernelWorkspace,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
    search_stats.kernel_basis_generations += 1;
    let kernel_started = Instant::now();
    let candidates =
        kernel_workspace.try_kernel_basis_with_width(kernel_checks, width, permutation);
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

Change the candidate rows signature:

```rust
fn consider_component_candidate_rows(
    candidates: &[Vec<u8>],
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
```

Change `component_candidate_to_pauli` to borrow the candidate:

```rust
fn component_candidate_to_pauli(component: ComponentKind, candidate: &[u8]) -> Result<Pauli> {
    let width = candidate.len();
    match component {
        ComponentKind::XLike => Pauli::from_xz_bits(candidate.to_vec(), vec![0; width]),
        ComponentKind::ZLike => Pauli::from_xz_bits(vec![0; width], candidate.to_vec()),
    }
}
```

In the candidate loop, keep the call as:

```rust
        let witness = component_candidate_to_pauli(component, candidate)?;
```

- [ ] **Step 4: Update existing candidate-row test call sites**

In `qec-code/src/distance_bound.rs`, add `&` before every hand-built candidate vector passed to `consider_component_candidate_rows`. The resulting calls should look like these examples:

```rust
        consider_component_candidate_rows(
            &[vec![0, 0, 0], vec![1, 1, 0], vec![1, 1, 1], vec![0, 0, 1]],
            &[],
            &component_span,
            ComponentKind::XLike,
            &mut best_witness,
            &mut search_stats,
        )
        .unwrap();
```

```rust
        consider_component_candidate_rows(
            &[vec![1, 1, 1, 0, 0]],
            &[],
            &component_span,
            ComponentKind::XLike,
            &mut best_witness,
            &mut search_stats,
        )
        .unwrap();
```

```rust
        consider_component_candidate_rows(
            &[vec![0, 0, 1], vec![1, 1, 0]],
            css.hz(),
            &hx_span,
            ComponentKind::XLike,
            &mut x_best,
            &mut x_stats,
        )
        .unwrap();
```

```rust
        consider_component_candidate_rows(
            &[vec![1, 0, 0], vec![0, 0, 1]],
            css.hx(),
            &hz_span,
            ComponentKind::ZLike,
            &mut z_best,
            &mut z_stats,
        )
        .unwrap();
```

- [ ] **Step 5: Run the integration and regression tests to verify GREEN**

Run:

```bash
cargo test -p qec-code random_window_candidate_rows_accepts_workspace_output_without_stale_rows -q
cargo test -p qec-code random_window_prunes_candidates_that_cannot_improve_best -q
cargo test -p qec-code random_window_pruning_does_not_skip_strictly_better_candidate -q
cargo test -p qec-code random_window_component_filter_matches_full_witness_validation -q
cargo test -p qec-code random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options -q
```

Expected: all commands pass. If registry access is blocked, rerun with `--offline` before the test name.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add qec-code/src/distance_bound.rs
git commit -m "feat: reuse gf2 workspace in random-window search"
```

Expected: commit succeeds with only `qec-code/src/distance_bound.rs` staged.

### Task 3: Formatting And Verification

**Files:**
- Modify if needed: `qec-code/src/gf2.rs`, `qec-code/src/distance_bound.rs`

**Interfaces:**
- Consumes: implementation from Tasks 1 and 2.
- Produces: formatted Rust code, verified issue commands, final implementation commit if formatting changes files.

- [ ] **Step 1: Format Rust code**

Run:

```bash
cargo fmt -- qec-code/src/gf2.rs qec-code/src/distance_bound.rs
```

Expected: formatting completes successfully.

- [ ] **Step 2: Commit formatting if needed**

Run:

```bash
git status --short
```

If formatting changed Rust files, run:

```bash
git add qec-code/src/gf2.rs qec-code/src/distance_bound.rs
git commit -m "style: format random-window gf2 workspace"
```

Expected: no commit is needed when Task 1 and Task 2 snippets were already formatted; otherwise the formatting commit contains only Rust formatting changes.

- [ ] **Step 3: Run issue-required positive tests**

Run:

```bash
cargo test -p qec-code gf2_random_window_workspace_matches_existing_kernel_basis -q
cargo test -p qec-code gf2_random_window_workspace_reuse_resets_state -q
```

Expected: both commands pass.

- [ ] **Step 4: Run issue-required negative control**

Run:

```bash
cargo test -p qec-code gf2_random_window_workspace_rejects_stale_or_invalid_inputs -q
```

Expected: command passes and would fail if stale columns from the previous wider call appeared in the narrow output.

- [ ] **Step 5: Run crate and workspace verification**

Run:

```bash
cargo test -p qec-code -q
cargo test
```

Expected: both commands pass. If `cargo test` attempts to access the network and the sandbox proxy blocks it, rerun:

```bash
cargo test --offline
```

Record the requested command failure and the offline outcome.

- [ ] **Step 6: Run the no-target ladder smoke**

Run:

```bash
make qec-code-random-window-bench-no-target-ladder-smoke
```

Expected: command passes and the summary still reports `surface_rotated_d5 = 5`, `toric_d5 = 5`, `bb72 = 6`, and `bb144 = 12` as best upper bounds. `search_stats.kernel_basis_generations` remains consistent with the fixed smoke budget. If the command attempts to access the registry and the sandbox proxy blocks it, rerun the underlying Cargo build in offline mode only if the Make target supports the cached toolchain without editing semantics; otherwise record the exact failure as a residual risk.

- [ ] **Step 7: Inspect benchmark evidence**

Run:

```bash
rg -n "surface_rotated_d5|toric_d5|bb72|bb144|kernel_basis_generations|kernel=" benchmarks/out/qec_code_random_window/no-target-ladder-smoke/summary/summary.md
```

Expected: the summary evidence includes the four required case IDs, upper bounds 5/5/6/12, search-stat kernel generation totals, and a kernel timing note. Record whether `kernel_basis_time_ns` appears lower, similar, or higher than prior local output if prior output exists in the workspace; do not treat timing as a hard pass/fail threshold.

- [ ] **Step 8: Commit any verification-induced tracked changes only if intentional**

Run:

```bash
git status --short
```

Expected: benchmark output directories remain ignored or untracked outside the commit. Do not commit generated benchmark output unless it is already tracked and intentionally updated by the repository workflow.
