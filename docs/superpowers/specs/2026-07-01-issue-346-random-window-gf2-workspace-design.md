# Issue 346 Random-Window GF(2) Workspace Design

## Context

Issue #346 asks for a reusable GF(2) workspace for random-window kernel-basis
generation. The current helper,
`gf2::try_random_window_kernel_basis_with_width`, validates dense binary rows and
a column permutation, materializes a permuted dense matrix, computes a nullspace
basis, remaps each basis vector back to original column order, and returns a
fresh `Vec<Vec<u8>>`.

The random-window CSS upper-bound search calls that helper twice per sampled
permutation, once for X-like candidates from `ker(H_Z)` and once for Z-like
candidates from `ker(H_X)`. The current branch already includes the #337
no-target ladder smoke target, #338 search counters, #343 timing diagnostics,
#345 weight pruning, and #344 CSS component filtering. The relevant acceptance
surface is therefore the existing dense GF(2) helper plus
`qec-code/src/distance_bound.rs`'s `consider_component_candidates` path.

Live GitHub issue and PR lookup is blocked by the sandbox proxy, so this design
uses the complete Agent Desk issue body and merged local Superpowers specs for
issues #337, #338, #343, #344, and #345.

## Design Options

1. Add a crate-private dense `RandomWindowKernelWorkspace` in
   `qec-code/src/gf2.rs`. The workspace owns reusable permuted rows, pivot
   metadata, validation scratch, and original-order basis rows. It returns a
   borrowed slice of original-order candidate rows for hot-loop callers, while
   the existing simple helper delegates to a temporary workspace and clones the
   returned slice for compatibility.

2. Keep the public helper shape and only factor out small validation utilities.
   This would reduce duplicated code but would still allocate a fresh permuted
   matrix and output basis for every random-window component, so it would not
   address the issue's allocation objective.

3. Introduce bit-packed GF(2) storage directly. This could improve later
   performance, but it is explicitly out of scope for this issue and would make
   byte-for-byte dense-row compatibility harder to review.

Chosen approach: option 1. It preserves dense-row semantics, keeps the existing
helper available, and gives the search loop a reusable path without changing
sampling, candidate ordering, or result meaning.

## Workspace Contract

Add `pub(crate) struct RandomWindowKernelWorkspace` with:

- `new() -> Self`;
- `try_kernel_basis_with_width(&mut self, matrix: &[BinaryRow], width: usize, column_permutation: &[usize]) -> Result<&[BinaryRow]>`.

The method validates inputs in the same order as the existing helper:

1. `validate_rows_with_width(matrix, width)` rejects row-width mismatches and
   non-binary entries.
2. `validate_column_permutation` rejects length mismatch, out-of-range columns,
   and duplicates with the same error strings as today.

After validation, the workspace clears its logical scratch lengths before
refilling rows. It may retain vector capacity across calls, but every returned
row must be resized to the current width and every unreturned stale row must be
outside the returned slice. Reusing one workspace for a wider matrix, then a
narrower matrix, then a different permutation must produce exactly the same
rows as the existing helper for each individual call.

The basis algorithm stays equivalent to the current helper:

- copy each input row into workspace-owned permuted column order;
- run the same dense RREF elimination over the permuted rows;
- enumerate free columns in increasing permuted-column order;
- construct original-order basis vectors directly by mapping each permuted free
  and pivot coordinate through `column_permutation`.

Direct construction avoids a separate permuted-basis allocation while preserving
the exact candidate row order produced by the old helper.

## Search Integration

In `random_window_css_upper_bound`, allocate one
`gf2::RandomWindowKernelWorkspace` before the restart/iteration loops and pass a
mutable reference into `consider_component_candidates`. The X-like component
uses the workspace, fully processes the returned slice, and then the Z-like
component reuses the same workspace. No candidate slice is retained across calls.

Change `consider_component_candidate_rows` to accept `&[Vec<u8>]` instead of
owning `Vec<Vec<u8>>`. The loop only needs borrowed candidate rows until a row
survives the cheap filters. At that point `component_candidate_to_pauli` clones
the accepted row into the Pauli support. Search counters keep their existing
semantics: `kernel_basis_generations` still increments once per component basis
generation, and `component_candidates_generated` still counts the returned rows.

The existing `try_random_window_kernel_basis_with_width` helper remains
crate-private and keeps its `Result<Vec<Vec<u8>>>` return type for simple callers
and tests. It delegates to a temporary workspace, then clones the borrowed slice.

## Tests

Add Rust unit tests in `qec-code/src/gf2.rs` with the issue-required names:

- `gf2_random_window_workspace_matches_existing_kernel_basis` compares workspace
  output byte-for-byte against `try_random_window_kernel_basis_with_width` for
  at least three small matrices and at least three permutations. It also checks
  every returned row satisfies the original matrix kernel equation.
- `gf2_random_window_workspace_reuse_resets_state` reuses one workspace across
  different matrix widths and permutations and proves returned rows match the
  helper with no stale-width rows.
- `gf2_random_window_workspace_rejects_stale_or_invalid_inputs` first populates
  the workspace with a wider matrix, then checks invalid permutations and
  invalid binary or width inputs return the same validation errors as the
  helper, and finally verifies a narrower valid call returns only narrow rows.

Keep existing random-window search tests as regression coverage for candidate
semantics. The no-target ladder smoke remains the end-to-end behavioral gate and
continues to validate best upper bounds and search-stat generation counts.

## Out Of Scope

Do not add bit-packed storage, external GF(2) libraries, or new random-window
sampling behavior. Do not remove the existing simple GF(2) helper. Do not change
benchmark result semantics, target handling, random seeds, or candidate
filtering rules.
