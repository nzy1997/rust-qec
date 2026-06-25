# Issue 230 Random-Window GF(2) Kernel-Basis Design

Date: 2026-06-25
Status: Accepted automatically under the Agent Desk Standing Answer Policy
Scope: `qec-code` GF(2) helper for random-window / random-information-set search

## Summary

Issue #230 adds a deterministic GF(2) helper that accepts a binary check matrix
and a column permutation, computes a nullspace basis in the permuted column
frame, and maps each candidate row back to the original column order.

The helper stays crate-internal in `qec-code/src/gf2.rs`. Later distance-bound
search code can call it from inside `qec-code`, while this issue avoids a new
public `binary` API before the random-window result contract and CLI surface are
defined.

## Context

`qec-code/src/gf2.rs` already owns rectangular binary-row validation, RREF,
rank, row-span checks, independent-row selection, and width-aware nullspace
basis construction. The module is internal (`mod gf2`) but its helpers are
available across the crate through `pub(crate)`.

Issue #225 decomposes the random-window work into small follow-up issues. #228
and #229 are already merged on the current base; #230 should only provide the
GF(2) basis generator. It must not add CSS witness validation, CLI flags,
bit-packed storage, or external GF(2) dependencies.

## Approaches Considered

### 1. Crate-internal helper in `gf2.rs`

Add `try_random_window_kernel_basis_with_width(matrix, width, permutation)` next
to `try_nullspace_basis_with_width`. The function validates matrix shape and
binary entries, validates the permutation, permutes columns, calls the existing
nullspace helper, and unpermutes each basis vector.

This is the chosen approach. It keeps pure GF(2) logic near the existing RREF
and nullspace code, makes future crate-internal CSS search integration direct,
and satisfies the issue without expanding the public API.

### 2. Public wrapper in `binary.rs`

Expose the helper through `qec_code::binary` and test it through integration
tests. This would be useful if downstream crates need the helper immediately,
but the issue says public API visibility should be explicit and does not require
a public surface. Exposing it now would make later API changes more expensive.

### 3. Private helper in `distance_bound.rs`

Keep the helper local to the future random-window search implementation. This
would narrow visibility too much for a pure GF(2) operation and would mix matrix
algebra with distance-bound result validation.

## Design

Add a crate-internal function:

```rust
pub(crate) fn try_random_window_kernel_basis_with_width(
    matrix: &[BinaryRow],
    width: usize,
    column_permutation: &[usize],
) -> Result<Vec<BinaryRow>>
```

The permutation is interpreted as the permuted-frame column order:
`column_permutation[permuted_col] = original_col`. The helper constructs
`H_perm[:, permuted_col] = H[:, original_col]`, computes a nullspace basis for
`H_perm`, then maps each basis row back with
`original[original_col] = permuted[permuted_col]`.

Validation order:

1. Validate every matrix row has `width` entries and only binary values.
2. Validate the permutation length is exactly `width`.
3. Validate every permutation entry is in `0..width` and appears once.
4. Only after validation, allocate the permuted matrix and compute the basis.

Add a `QecError::InvalidColumnPermutation { reason: String }` variant so bad
permutations produce clear errors that name the invalid permutation.

No seeded permutation generator is added in this issue. The issue makes that
optional, and `qec-code` has no RNG dependency today. Fixed permutation input is
enough to guarantee deterministic byte-for-byte output.

## Testing

Add focused unit tests in `qec-code/src/gf2.rs`:

- `gf2_random_window_kernel_basis_contract` uses a fixed small matrix and fixed
  permutation, asserts exact unpermuted rows, verifies every row has width `n`,
  checks every candidate satisfies `H * v^T = 0 mod 2`, compares kernel rank to
  the existing nullspace helper, and calls the helper twice to confirm
  byte-for-byte determinism.
- `gf2_random_window_kernel_basis_rejects_bad_permutation` passes a duplicate
  column permutation and asserts the error is `InvalidColumnPermutation`.
- `random_window_kernel_basis_rejects_invalid_matrix_inputs` covers the
  existing non-binary and row-width validation path through the new helper.

Required issue verification commands:

```bash
cargo test -p qec-code gf2_random_window_kernel_basis_contract -q
cargo test -p qec-code gf2_random_window_kernel_basis_rejects_bad_permutation -q
```

Repository workflow verification remains `cargo test`.

## Out Of Scope

Do not add CSS witness validation. Do not add CLI flags. Do not add bit-packed
matrix storage. Do not add external GF(2) or RNG dependencies.
