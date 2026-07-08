# Issue 414 MeasureRecordBatch Contiguous Storage Design

## Context

`MeasureRecordBatch` stores one measurement row per `Vec<u64>`. The selected
`stim_surface_d11_r100` fixture has 12121 measurement rows, so this layout pays
one allocation per row and later lookbacks chase row pointers when detectors and
observables materialize from `rec[-k]` references.

Issue #410 already added measurement-only output mode. This issue is limited to
the storage layout used when measurement rows are still recorded; sampler
semantics, detector semantics, and observable semantics must not change.

## Approaches Considered

1. Store rows in a row-major `Vec<u64>` plus `words_per_row` and an explicit
   row count. This is the recommended approach because it removes per-row
   allocations, preserves current slice-based lookback APIs, and still handles
   zero-shot rows where `words_per_row == 0`.
2. Reuse `BitTable` as the backing store. This would also be contiguous, but
   `MeasureRecordBatch` appends rows dynamically while `BitTable` is sized at
   construction, so it would add resizing behavior that the type does not need.
3. Keep `Vec<Vec<u64>>` and reserve row capacity. This reduces outer-vector
   reallocation but leaves the per-row allocation and pointer-chasing problem in
   place.

The design uses option 1.

## Design

Change `MeasureRecordBatch` to own:

- `batch_size: usize`
- `words_per_row: usize`
- `row_count: usize`
- `records: Vec<u64>`

Rows are addressed by `row_index * words_per_row`. `push_row` appends
`words_per_row` zero words, copies the supplied words up to the row width, and
increments `row_count`. `push_zeros` appends `words_per_row` zero words and
increments `row_count`. `lookback`, `lookback_words`, and
`xor_lookback_into` compute the row index as `row_count - k`, matching the
existing `records.len() - k` behavior.

Keep existing public methods stable and add a small read-only inspection method
that returns the contiguous row-major backing words. That method exists so the
new integration test can assert the storage shape directly without making
fields public.

Keep `row_count` explicit instead of deriving it from `records.len() /
words_per_row`; otherwise `batch_size == 0` would lose row identity because
zero-width rows append no words.

## Testing

Add `rstim/tests/measure_record_batch_storage.rs` with these tests:

- `contiguous_storage_reports_expected_shape` checks that pushed rows are
  visible in one row-major backing slice with `len * words_per_row` words.
- `lookback_words_match_pushed_rows` checks lookback order, truncation, and
  zero-padding against pushed rows.
- `xor_lookback_preserves_detector_parity_for_known_fixture` samples the
  checked d11/r100 fixture with a deterministic seed and asserts detector and
  observable fingerprints. Reversing row order or using an incorrect stride
  changes the fingerprints.
- `push_zeros_preserves_row_alignment` checks that zero rows keep their own
  contiguous slot between nonzero rows.
- Include a zero-shot row-count case so the explicit `row_count` requirement is
  covered.

Run the focused issue command and the broader repository test command:

- `cargo test -p rstim --test measure_record_batch_storage`
- `cargo test`

## Scope

This change is limited to `MeasureRecordBatch` storage and focused tests. It
does not change sampler output modes, detector algorithms, observable
algorithms, benchmark thresholds, or CLI behavior.

## Self-Review

- No placeholders remain.
- The design preserves zero-shot row semantics.
- The test plan checks both direct storage shape and detector parity for the
  selected fixture.
