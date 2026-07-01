# Bit-Packed Random-Window Kernel Basis Design

Issue: #352

## Context

The random-window upper-bound search repeatedly builds a kernel basis after
applying a sampled column permutation to a binary check matrix. The existing
`RandomWindowKernelWorkspace` keeps reusable dense `Vec<u8>` rows and returns
candidate rows in original qubit order. Issue #351 added reusable
`BitPackedRow` primitives, and issue #352 asks the random-window kernel-basis
path to use those primitives without changing the search contract.

GitHub issue fetch was not available in this Agent Desk sandbox, so this design
uses the issue body supplied in the run prompt as the authoritative context.

## Requirements

- Keep `RandomWindowKernelWorkspace::try_kernel_basis_with_width` and
  `try_random_window_kernel_basis_with_width` available to current callers.
- Use bit-packed rows for random-window permutation and elimination work.
- Preserve the dense reference pivot convention: scan columns left to right,
  choose the first available pivot row, eliminate every other row, and only xor
  from the pivot column onward.
- Return dense logical candidate rows in original qubit order after applying the
  caller's column permutation.
- Reject invalid permutations, non-binary entries, and row-width mismatches.
- Do not change random-window sampling, seed semantics, target-weight behavior,
  counters, returned upper bounds, dependencies, unsafe code, or SIMD/runtime
  dependencies.

## Approaches Considered

1. **Recommended: keep the existing workspace API and change its internal row
   storage to bit-packed rows.** This gives the random-window search path the
   packed elimination speedup because it already uses
   `RandomWindowKernelWorkspace`, while preserving all caller-facing types and
   result semantics.
2. **Add a second `BitPackedRandomWindowKernelWorkspace` and update only the
   search path to use it.** This is more explicit, but it duplicates lifecycle
   and validation behavior and creates two workspace APIs to keep in sync.
3. **Make `try_random_window_kernel_basis_with_width` bit-packed and leave the
   reusable workspace dense.** This helps simple helper callers but misses the
   main random-window search path, which reuses `RandomWindowKernelWorkspace`.

## Design

`RandomWindowKernelWorkspace` will continue to return `&[BinaryRow]`, but its
permuted working rows will become `Vec<BitPackedRow>`. Each call will validate
matrix rows and the permutation before packing. Packing maps
`row[original_col]` into bit `permuted_col`, so the elimination operates in the
sampled window order.

The bit-packed elimination will mirror the dense implementation exactly:

- scan `col` from `0..width`;
- find the first pivot row at or after `pivot_row` whose bit at `col` is one;
- swap that row into `pivot_row`;
- for every other row with bit `col` set, xor the pivot row into it starting at
  `col`, not at bit zero;
- record the pivot column and advance.

The "xor from column" detail is required for deterministic parity with the
dense helper because earlier free columns are part of the basis convention.

Basis reconstruction will remain explicit and dense. For each non-pivot
permuted column, the workspace creates a dense output row, places the free bit
at `column_permutation[free_col]`, reads pivot-row coefficients from the
bit-packed reduced rows, and writes those bits back through the same
permutation. This keeps returned candidates in original qubit order.

## Testing

The GF(2) tests will add issue-specific checks with the exact requested names:

- `gf2_bitpacked_random_window_kernel_basis_matches_dense_workspace`
- `gf2_bitpacked_random_window_kernel_workspace_reuse_resets_state`
- `gf2_bitpacked_random_window_kernel_basis_rejects_invalid_inputs`

The match test will cover an empty matrix, small hand-checkable matrices, and a
BB-like width case, each with at least three permutations. Every returned row
will be compared to a dense reference and checked against the original matrix
kernel equation. Reuse tests will run the same workspace across wider and
narrower calls to catch stale rows and tail-bit leaks. Negative-control tests
will assert invalid permutation, non-binary input, and width-mismatch errors.

The existing pinned random-window distance test and no-target ladder smoke will
verify that search semantics and diagnostics remain stable.

## Approved Scope

This PR only changes random-window kernel-basis generation and its tests. It
does not alter sampling, span filtering, benchmark manifests, CLI flags, or
external dependencies.
