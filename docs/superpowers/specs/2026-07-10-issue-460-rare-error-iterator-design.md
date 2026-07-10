# Issue 460 Rare Error Iterator Design

## Context

Issue #427 added sparse geometric skipping inside one frame-noise mask, but that
helper still operates on one mask at a time. The next optimization needs an
instruction-wide primitive that can walk a flattened opportunity domain
`[0, attempt_count)` without allocating a dense bitmap or vector whose size
tracks the number of opportunities.

This issue is intentionally limited to the internal iterator and its acceptance
tests. It must not wire noise instructions to the new iterator yet, and it must
leave the existing dense sampler available for probabilities above any future
sparse threshold.

## Approaches Considered

1. Add a focused `RareErrorIterator` module that owns only the flattened-index
   iterator and deterministic geometric skipping. This is the selected approach
   because it keeps the new primitive independent from current frame-noise
   wiring, gives integration tests a small surface to exercise, and avoids
   changing seeded simulator behavior outside this issue.
2. Refactor the existing frame-noise sparse helper into a shared iterator and
   route frame masks through it immediately. This would reduce duplication, but
   it would change an existing hot path and seeded mask behavior in a task whose
   out-of-scope section explicitly says not to wire instructions yet.
3. Implement rare events by precomputing a sparse `Vec<usize>` for an
   instruction and iterating that vector. This is simple, but it violates the
   memory contract when `attempt_count` is large enough for the sparse draw to
   still contain many events.

The design uses option 1.

## Design

Add `rstim/src/rare_error_iterator.rs` and expose it from `rstim/src/lib.rs` as
a hidden internal module. The module provides `RareErrorIterator<'a, R>` and a
small constructor function:

```rust
pub fn rare_error_indices<'a, R: RngCore + ?Sized>(
    probability: f64,
    attempt_count: usize,
    rng: &'a mut R,
) -> RareErrorIterator<'a, R>;
```

The iterator has three modes:

- Empty mode for `probability <= 0.0` or `attempt_count == 0`; it yields
  nothing and performs no RNG draws.
- Dense boundary mode for `probability >= 1.0`; it yields exactly
  `0..attempt_count` and performs no RNG draws.
- Sparse mode for `0.0 < probability < 1.0`; it computes `ln(1 - p)` once,
  draws one `u64` from the supplied `RngCore` per geometric skip, converts the
  high 53 bits into a uniform `f64` in `[0, 1)`, retries only the exact zero
  case, and advances by `floor(ln(u) / ln(1 - p)) + 1`.

Sparse mode stores only the current candidate index, the attempt bound, the
precomputed log value, and a mutable RNG reference. It never
stores a bitmap or vector of the opportunity domain. Each yielded index is
checked against `attempt_count` before it is returned, so `attempt_count` itself
is never yielded.

## Telemetry

Debug builds expose hidden module-level `reset_rare_error_telemetry()` and
`rare_error_telemetry()` functions, with thread-local iterator-build and core
RNG-draw counters. The counter updates occur at the iterator constructor and
the `RngCore::next_u64` call site. Release builds omit the telemetry type,
surface, storage, and counter updates entirely. Acceptance tests independently
compare the reported draw count with a counting `RngCore` wrapper.

## Testing

Add `rstim/tests/rare_error_iterator.rs` with the six required test names:

- `boundary_probabilities_and_zero_attempts`
- `indices_are_strictly_increasing_unique_and_in_range`
- `seeded_iterator_is_reproducible`
- `sparse_frequency_windows_and_gaps_are_non_periodic`
- `sparse_draw_count_is_bounded`
- `iterator_allocation_is_independent_of_attempt_count`

All seeded tests use `rand::rngs::StdRng::seed_from_u64(123)`. The sparse
acceptance case uses `attempt_count = 1_000_000` and `p = 0.001`, requires
800-1,200 events, fewer than 10,000 actual `RngCore` calls, at least one event in each
100,000-attempt window, non-identical window counts, and more than 100 distinct
gaps. The allocation test uses a thread-local counting global allocator in the
test binary to compare construction plus the first `next()` for
`attempt_count = 1_000_000` and `1_000_000_000` at `p = 1e-9`; the larger domain
may allocate at most 4 KiB more than the smaller one.

The focused verification command is:

```sh
cargo test -p rstim --test rare_error_iterator -- --nocapture
```

The acceptance output must include:

```text
PASS instruction-wide rare-error iterator
```

Final verification also runs:

```sh
cargo test
```

## Scope

This change is limited to the internal iterator module, library module export,
and focused integration tests. It does not change frame simulator noise
sampling, compiled sampling, CLI formats, benchmark artifacts, or public
end-user behavior.

## Self-Review

- No placeholders remain.
- The selected approach directly addresses the issue's flattened-domain
  iterator objective while respecting the out-of-scope wiring constraint.
- Boundary behavior, seeded determinism, draw-count limits, non-periodic gap
  checks, and allocation independence all have explicit tests.
- The negative controls named in the issue map to concrete assertions: fixed
  periodic gaps fail gap diversity, per-attempt draws fail draw bounds, dense
  bitsets fail allocation comparison, and duplicate/decreasing/out-of-range
  outputs fail ordering/range checks.
