# Issue 353 Bit-Packed CSS Span Filter Design

## Context

Issue #353 asks the random-window CSS upper-bound search to use the bit-packed
GF(2) row primitives from issue #351 for CSS component kernel and stabilizer
span checks. Issue #344 already replaced hot-loop full Pauli validation with a
dense algebraic component filter that returns `Accepted`, `Zero`, `NonKernel`,
or `StabilizerSpan`. This issue must preserve that component-filter semantics,
keep the final `validate_random_window_upper_bound_result` call, and avoid
changes to sampling, seeds, target-weight behavior, benchmark manifests, or
`randomized-upper-bound`.

Local history shows #344 and #351 merged into `master`. No #352 branch or merge
is visible locally, and GitHub CLI lookup is blocked by the Agent Desk sandbox
proxy, so this design uses the full Agent Desk issue body plus local merge
history as the source of issue and PR context.

## Design Options

1. Add a packed reduced-row-span representation in `qec-code/src/gf2.rs`, then
   use a packed CSS component filter in `qec-code/src/distance_bound.rs`.
   Opposite CSS checks and same-side stabilizer spans are packed once per
   search, while each dense candidate from the existing kernel workspace is
   packed at the filter boundary. This is the chosen approach because #352 is
   not present, it keeps GF(2) membership logic near the GF(2) helpers, and it
   leaves the current dense filter available as a reference path.

2. Implement packed span membership directly inside `distance_bound.rs`. This
   would be slightly quicker to write, but it would scatter pivot-row algebra
   through the random-window search code and make future packed kernel-basis
   work harder to reuse.

3. Rewrite `RandomWindowKernelWorkspace` to emit packed candidate rows now.
   This could remove candidate packing at the boundary, but the issue says this
   is optional only if #352 has landed. It would broaden the change from span
   filtering into kernel-basis generation and increase regression risk.

Chosen approach: option 1.

## Packed GF(2) Span Contract

Add `pub(crate) struct PackedReducedRows` in `qec-code/src/gf2.rs` with the same
logical content as `ReducedRows`, but with each reduced row stored as a
`BitPackedRow`. Construct it from an existing `ReducedRows` value:

- validate that every dense reduced row has `reduced.width`;
- pack rows with `BitPackedRow::try_from_dense`;
- clone `pivot_cols` and `width`;
- keep all fields private except crate-private accessors needed by the filter.

Add `try_in_packed_reduced_row_span(reduced, target)` that mirrors
`try_in_reduced_row_span` exactly:

1. reject target width mismatch with `QecError::RowWidthMismatch`;
2. clone the packed target into a mutable remainder;
3. for each pivot column in order, XOR the corresponding packed reduced row
   into the remainder if the pivot bit is set;
4. return true only when the logical remainder is zero.

The helper must use logical bit access and `BitPackedRow` operations so final
word padding never affects equality, zero detection, parity, or membership.

## CSS Component Filter Contract

Keep the dense `css_component_candidate_verdict` helper as the reference path.
Add a private packed filter representation in `qec-code/src/distance_bound.rs`:

- `PackedCssComponentFilter` owns packed opposite-check rows and a
  `PackedReducedRows` stabilizer component span.
- X-like candidates use opposite checks `H_Z` and stabilizer span `row_span(H_X)`.
- Z-like candidates use opposite checks `H_X` and stabilizer span `row_span(H_Z)`.

Add `bitpacked_css_component_candidate_verdict(filter, candidate)` returning
the same enum as the dense helper:

- `Zero` when the logical candidate row is zero;
- `NonKernel` when any packed opposite-check dot parity is one;
- `StabilizerSpan` when packed reduced-row-span membership is true;
- `Accepted` otherwise.

`consider_component_candidate_rows` should pack each dense candidate once,
derive weight from the packed row, run the packed verdict, and leave existing
search-stat counters unchanged:

- zero rows still increment `zero_candidates_rejected`;
- current-best weight pruning still increments `weight_pruned_candidates`;
- non-kernel verdicts still increment
  `witness_validation_candidates_rejected`;
- stabilizer-span verdicts still increment
  `stabilizer_span_candidates_rejected`;
- accepted rows still construct the same pure X-like or Z-like Pauli witness,
  increment `valid_witnesses_found`, and update the best witness only when
  strictly lighter.

## Tests

Add focused unit tests where the private helpers are visible:

- `random_window_bitpacked_component_filter_matches_dense_filter` compares
  dense and packed verdicts for both X-like and Z-like candidates on
  `surface_rotated:d=3` and `bb72`, including zero, non-kernel, stabilizer-span,
  and sampled kernel-basis rows.
- Update `random_window_component_filter_matches_full_witness_validation` so
  every packed-filter verdict still matches the full validator mapping.
- Keep and update
  `random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates`
  so the real candidate loop rejects hand-built non-kernel and span rows before
  best-witness updates.
- Add
  `random_window_bitpacked_component_filter_rejects_tail_bit_and_span_false_positive_cases`
  to prove logical padding bits cannot create an accepted row, opposite-check
  kernel failure still rejects, and packed span membership does not accept a
  nonmember merely because it shares storage words with a span row.

Run the issue verification commands, `cargo test -p qec-code gf2 -q`, and the
workspace `cargo test`.

## Out Of Scope

Do not change random-window sampling, seed semantics, target-weight behavior,
benchmark manifests, no-target output semantics, final result validation,
`randomized-upper-bound`, or external dependencies. Do not add SIMD, unsafe
code, M4RI, `dist-m4ri`, `QDistRnd`, or `codeDistancePYPI`.

## Self-Review

- Placeholder scan: no unresolved marker entries.
- Consistency: packed verdicts intentionally share the existing dense enum and
  preserve the #344 rejection mapping.
- Scope: the change is limited to packed GF(2) row-span membership and
  random-window CSS component filtering.
- Ambiguity: because #352 is absent locally, dense candidates are packed at the
  filter boundary.
