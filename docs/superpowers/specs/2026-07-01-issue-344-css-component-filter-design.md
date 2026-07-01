# Issue 344 CSS Component Filter Design

## Context

Issue #344 asks the random-window CSS upper-bound search to stop running full
Pauli witness validation for every component candidate in the hot loop. The
candidate source is already component-specific: X-like rows are drawn from the
kernel of `H_Z`, and Z-like rows are drawn from the kernel of `H_X`. The search
also already rejects rows in the same-side stabilizer component span before
constructing a Pauli witness.

The current branch starts from master after issues #343 and #345. Timing fields
are present on `RandomWindowSearchStats`, and candidate processing already lives
in `consider_component_candidate_rows`, with current-best weight pruning before
span filtering. The issue comment says dependencies #337 and #338 are complete
and implementation is approved; GitHub CLI issue fetching is blocked by the
sandbox proxy, so this design uses the Agent Desk issue body plus connector
comments and merged local PR context.

## Design Options

1. Add a focused algebraic CSS component filter in `qec-code/src/distance_bound.rs`
   and call it from the random-window candidate loop. The helper validates the
   kernel equation against the opposite CSS check matrix and nonmembership in
   the same-side component stabilizer row span, then the loop constructs a pure
   X-like or Z-like Pauli witness for accepted rows. This is the recommended
   option because it is local, explicit, and keeps final result validation.

2. Trust `try_random_window_kernel_basis_with_width` plus the existing span
   rejection without an explicit helper. This would remove the hot-loop full
   validator with fewer lines, but it would make the proof implicit and would
   not provide case-specific rejection coverage for malformed hand-built rows.

3. Move algebraic checking into the generic GF(2) kernel-basis helper. This
   would centralize some matrix logic, but it would mix CSS distance semantics
   into a generic linear-algebra utility and would not fit the existing module
   boundaries.

Chosen approach: option 1.

## Component Filter Contract

Add a private verdict enum and helper for random-window component rows:

- X-like candidate `v`: require `H_Z * v^T = 0 mod 2`, reject zero rows, and
  require `v` not in `row_span(H_X)`.
- Z-like candidate `v`: require `H_X * v^T = 0 mod 2`, reject zero rows, and
  require `v` not in `row_span(H_Z)`.

The helper should return a case-specific verdict for accepted, zero,
non-kernel, and stabilizer-span rows. The normal search loop will still keep the
existing zero, weight-pruning, stabilizer-span, valid-witness, and best-update
counters. Non-kernel rejections should increment
`witness_validation_candidates_rejected` because that counter already represents
rows that passed earlier cheap filters but failed the correctness gate; for real
kernel-basis candidates this should remain zero.

The helper is intentionally private. It documents the CSS proof in the owning
module without adding a public API surface.

## Search Flow

For each component candidate row:

1. Count Hamming weight.
2. Reject zero rows and increment `zero_candidates_rejected`.
3. If the row cannot improve the current best witness, increment
   `weight_pruned_candidates`.
4. Run the algebraic CSS component filter using the opposite check matrix and
   same-side reduced component stabilizer span.
5. Reject stabilizer-span rows with `stabilizer_span_candidates_rejected`.
6. Reject non-kernel rows with `witness_validation_candidates_rejected`.
7. Construct the pure Pauli witness only for accepted rows and update the best
   witness if it is strictly lighter.

This removes repeated calls to `validate_witness_against_code_with_span` from
the random-window inner loop. The final
`validate_random_window_upper_bound_result` call remains in
`completed_random_window_upper_bound_result`, preserving the serialized result
validation contract.

## Tests

Add Rust unit tests in `qec-code/src/distance_bound.rs` so the private helper
can be exercised without adding public API surface:

- `random_window_component_filter_matches_full_witness_validation` enumerates
  hand-built and sampled component rows for at least one surface-code fixture
  and one BB fixture. For both X-like and Z-like candidates, every row accepted
  by the component filter must be accepted by the full witness validator, and
  every full-validator kernel/span rejection must match a component-filter
  non-kernel or stabilizer-span verdict.
- `random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates`
  uses hand-built low-weight rows that fail the kernel equation or lie in the
  relevant component stabilizer span, then proves they cannot update the best
  witness.

Keep existing pinned random-window tests and the no-target ladder smoke as the
behavioral gate. If timing diagnostics are present, confirm the generated
summary keeps the witness-validation timing bucket and that no-target output
still omits `target_weight`.

## Out Of Scope

Do not change random-window sampling semantics. Do not remove final result
validation. Do not change `randomized-upper-bound`. Do not add bit-packed GF(2)
storage or a reusable kernel workspace.
