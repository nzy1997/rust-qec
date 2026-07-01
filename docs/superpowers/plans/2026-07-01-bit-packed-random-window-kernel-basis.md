# Bit-Packed Random-Window Kernel Basis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace random-window kernel-basis permutation and elimination with bit-packed GF(2) rows while preserving the dense workspace API and exact output ordering.

**Architecture:** `RandomWindowKernelWorkspace` keeps its public methods and dense returned rows, but stores permuted working rows as `BitPackedRow`. Bit-packed elimination follows the existing dense pivot convention and reconstructs original-order dense candidates from reduced packed rows.

**Tech Stack:** Rust 2024, existing `qec-code` crate, existing `BitPackedRow` primitives, existing `QecError` validation types, Cargo tests.

## Global Constraints

- Keep `RandomWindowKernelWorkspace::try_kernel_basis_with_width` and `try_random_window_kernel_basis_with_width` available.
- Return the same logical candidate rows as the dense reference for valid inputs.
- Preserve deterministic pivot/order convention: first pivot row, columns left to right, eliminate every other row, xor only from the pivot column onward.
- Return candidate rows in original qubit order after applying the sampled permutation.
- Reject invalid permutations, non-binary entries, and row-width mismatches.
- Do not change random-window sampling, seeds, target-weight behavior, counters, returned upper-bound semantics, benchmark manifests, external dependencies, unsafe code, SIMD, M4RI, or span filtering.

---

## File Structure

- Modify `qec-code/src/gf2.rs`: add bit helpers to `BitPackedRow`; change `RandomWindowKernelWorkspace` internals from dense permuted rows to packed permuted rows; add issue-specific tests.
- Leave `qec-code/src/distance_bound.rs` behavior unchanged except through the existing workspace type already used by random-window search.
- Add no new dependencies.

### Task 1: Add Bit-Packed Random-Window Contract Tests

**Files:**
- Modify: `qec-code/src/gf2.rs`

**Interfaces:**
- Consumes: existing `RandomWindowKernelWorkspace::try_kernel_basis_with_width`, `try_random_window_kernel_basis_with_width`, dense test reference helper.
- Produces: failing issue-specific tests named by #352.

- [ ] **Step 1: Write the failing dense-parity test**

Add a test named `gf2_bitpacked_random_window_kernel_basis_matches_dense_workspace` that iterates over three matrix widths: empty width 3, hand-checkable width 5, and BB-like width 144. For each matrix, run at least three valid permutations. For every permutation, compare `RandomWindowKernelWorkspace::try_kernel_basis_with_width` and `try_random_window_kernel_basis_with_width` against `reference_random_window_kernel_basis_with_width`, assert each row length equals `width`, and call `assert_kernel_vector` for every candidate.

- [ ] **Step 2: Run the new test and verify RED**

Run:

```bash
cargo test -p qec-code gf2_bitpacked_random_window_kernel_basis_matches_dense_workspace -q
```

Expected before implementation: FAIL because the test should include an assertion that the workspace is using the bit-packed implementation marker added in Task 2, or because the renamed test does not yet exist before this step is applied.

- [ ] **Step 3: Write the reuse and negative-control tests**

Add tests named:

```rust
#[test]
fn gf2_bitpacked_random_window_kernel_workspace_reuse_resets_state() { /* reuse wide then narrow calls */ }

#[test]
fn gf2_bitpacked_random_window_kernel_basis_rejects_invalid_inputs() { /* duplicate permutation, short permutation, out-of-range permutation, non-binary row, width mismatch, valid narrow call after wider call */ }
```

The reuse test must compare every call to the dense reference and assert all returned rows have the current logical width. The negative-control test must assert exact `QecError` values for malformed inputs and then verify a valid narrow call returns the exact dense reference rows.

- [ ] **Step 4: Run the test names and verify RED**

Run:

```bash
cargo test -p qec-code gf2_bitpacked_random_window_kernel_workspace_reuse_resets_state -q
cargo test -p qec-code gf2_bitpacked_random_window_kernel_basis_rejects_invalid_inputs -q
```

Expected before implementation: FAIL until Task 2 swaps the workspace implementation to bit-packed and the tests are finalized.

### Task 2: Implement Packed Permutation and Elimination in the Workspace

**Files:**
- Modify: `qec-code/src/gf2.rs`

**Interfaces:**
- Consumes: `BitPackedRow::try_from_dense`, `word_count`, `tail_mask`, `validate_rows_with_width`, `validate_column_permutation_with_seen`.
- Produces: `RandomWindowKernelWorkspace` with packed working rows and unchanged returned `&[BinaryRow]`.

- [ ] **Step 1: Add private bit helpers to `BitPackedRow`**

Add methods equivalent to:

```rust
fn reset_zero_width(&mut self, width: usize) {
    self.width = width;
    self.words.clear();
    self.words.resize(word_count(width), 0);
}

fn bit(&self, index: usize) -> u8 {
    u8::from(((self.words[index / 64] >> (index % 64)) & 1) == 1)
}

fn set_bit(&mut self, index: usize) {
    self.words[index / 64] |= 1u64 << (index % 64);
}

fn xor_assign_from_col(&mut self, rhs: &Self, start_col: usize) {
    let start_word = start_col / 64;
    let offset = start_col % 64;
    if self.words.is_empty() {
        return;
    }
    if offset == 0 {
        for word in start_word..self.words.len() {
            self.words[word] ^= rhs.words[word];
        }
    } else {
        self.words[start_word] ^= rhs.words[start_word] & (u64::MAX << offset);
        for word in (start_word + 1)..self.words.len() {
            self.words[word] ^= rhs.words[word];
        }
    }
    self.clear_padding_bits();
}
```

- [ ] **Step 2: Change workspace storage**

Change `RandomWindowKernelWorkspace` so `permuted_rows` is `Vec<BitPackedRow>` instead of `Vec<BinaryRow>`. Keep `basis_rows: Vec<BinaryRow>` and `basis_len` unchanged so callers still receive dense rows.

- [ ] **Step 3: Pack permuted rows**

Replace dense `fill_permuted_rows` with direct packed construction:

```rust
self.permuted_len = matrix.len();
for (row_index, row) in matrix.iter().enumerate() {
    if row_index == self.permuted_rows.len() {
        self.permuted_rows.push(BitPackedRow::zeros(width));
    }
    let permuted_row = &mut self.permuted_rows[row_index];
    permuted_row.reset_zero_width(width);
    for (permuted_col, &original_col) in column_permutation.iter().enumerate() {
        if row[original_col] == 1 {
            permuted_row.set_bit(permuted_col);
        }
    }
}
```

- [ ] **Step 4: Implement packed RREF with dense-equivalent range xor**

In `reduce_permuted_rows`, find pivots with `rows[row].bit(col) == 1`. When eliminating another row, call `xor_assign_from_col` starting at `col`; do not xor whole words below `col` because that would mutate earlier free-column coefficients and change the dense convention.

- [ ] **Step 5: Keep original-order basis reconstruction explicit**

In `fill_original_order_basis`, continue to create dense `Vec<u8>` output rows. Read pivot coefficients with `self.permuted_rows[pivot_row].bit(free_col)` and write output bits through `column_permutation`.

- [ ] **Step 6: Run the focused GF(2) tests**

Run:

```bash
cargo test -p qec-code gf2_bitpacked_random_window_kernel_basis_matches_dense_workspace -q
cargo test -p qec-code gf2_bitpacked_random_window_kernel_workspace_reuse_resets_state -q
cargo test -p qec-code gf2_bitpacked_random_window_kernel_basis_rejects_invalid_inputs -q
```

Expected: all pass.

### Task 3: Verify Search Semantics and Branch

**Files:**
- No production files expected beyond `qec-code/src/gf2.rs`.

**Interfaces:**
- Consumes: existing random-window search path in `qec-code/src/distance_bound.rs`.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Run the pinned random-window distance test**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_finds_surface_and_toric_distance_under_pinned_options -q
```

Expected: PASS with the expected upper bounds.

- [ ] **Step 2: Run the no-target ladder smoke**

Run:

```bash
make qec-code-random-window-bench-no-target-ladder-smoke
```

Expected: PASS; summary still reports `surface_rotated_d5 = 5`, `toric_d5 = 5`, `bb72 = 6`, `bb144 = 12`; no-target output omits `--target-weight`, records `target_weight = null`, `target_reached = false`, and `search_stats.kernel_basis_generations = 1000` for each ladder case.

- [ ] **Step 3: Run the requested broad verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Final review and PR**

Use `superpowers:requesting-code-review`, then `superpowers:verification-before-completion`, then `superpowers:finishing-a-development-branch`. Select "Push and create a Pull Request" per the Standing Answer Policy.

## Self-Review

- Spec coverage: the tasks cover packed permutation/elimination, original-order reconstruction, dense parity checks, invalid inputs, reuse/tail-bit safety, random-window semantics, and benchmark smoke output.
- Placeholder scan: no deferred implementation markers are intentionally present.
- Type consistency: `RandomWindowKernelWorkspace` keeps returning `&[BinaryRow]`; only its internal permuted rows change to `BitPackedRow`.
