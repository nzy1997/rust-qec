# Issue 555 Deterministic Regular Classical Matrix Design

## Context

Issue #555 asks for a reproducible, versioned, pure-Rust generator for regular
binary parity-check matrices in `qec-code`. Random-HGP and random two-block
families will use this as stable sampling infrastructure, and future issue
#563 needs the same deterministic stream and bounded-index helper without
copying the algorithm.

The referenced design file
`docs/design/2026-07-26-qec-code-family-support.md` is not present in this
checkout. This design treats the issue body, existing `qec-code` conventions,
and `.AGENTS/AGENTS.md` as the binding source.

## Approaches Considered

1. Add a focused `qec_code::regular_classical` module with a versioned config,
   `SplitMix64V1`, unbiased bounded helper, generator, documentation, and
   integration tests.
   - Pros: keeps the new behavior in the owning crate, exposes reusable
     deterministic primitives, and avoids changing unrelated distance-bound
     behavior.
   - Cons: leaves the older private `distance_bound` random stream untouched in
     this PR.

2. Fold the new generator into `css.rs` beside `SparseRowsMatrix`.
   - Pros: the generated output can immediately feed CSS sparse-row matrices.
   - Cons: mixes parsing/validation concerns with deterministic sampling and
     gives future family generators a less focused dependency boundary.

3. Refactor all existing randomized distance-bound sampling to the new stream
   while adding the matrix generator.
   - Pros: one deterministic stream implementation across the crate.
   - Cons: expands the behavioral surface of the PR and risks changing existing
     randomized upper-bound fixtures that are not part of #555.

Chosen approach: approach 1. The module boundary is narrow, keeps the v1
algorithm stable, and leaves unrelated randomized distance behavior unchanged.

## Public Surface

Add `pub mod regular_classical` to `qec-code`.

The module provides:

- `REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1: u32 = 1`.
- `RegularClassicalMatrixConfig` with `column_count`, `row_count`,
  `column_weight`, `row_weight`, `seed`, `algorithm_version`, and
  `retry_limit`.
- `deterministic_regular_matrix(config) -> Result<Vec<Vec<usize>>>`.
- `SplitMix64V1` with `new(seed)`, `state()`, and `next_u64()`.
- `bounded_index_v1(stream, upper_bound) -> Option<u64>`.

Errors use `QecError` variants so callers can match invalid configuration,
stub-count mismatch, unsupported algorithm versions, and retry exhaustion.

## Version 1 Algorithm

Validation happens before sampling:

- `algorithm_version` must be 1.
- `column_count`, `row_count`, `column_weight`, `row_weight`, and
  `retry_limit` must be greater than zero.
- `column_count * column_weight` and `row_count * row_weight` must not overflow
  and must be equal before any random word is consumed.

`SplitMix64V1` starts with `state = seed`. Each `next_u64()` call updates:

```text
state = state + 0x9E3779B97F4A7C15 mod 2^64
z = state
z = (z xor (z >> 30)) * 0xBF58476D1CE4E5B9 mod 2^64
z = (z xor (z >> 27)) * 0x94D049BB133111EB mod 2^64
output = z xor (z >> 31)
```

For seed 7, the first golden output words are:

```text
0x63CBE1E459320DD7
0x044C3CD7F43C661C
0xE6984080BAB12A02
0x953AEB70673E29CB
0x73D33B666A1E21DA
0x3FDABE86CBBEAA11
0x77CBC4A133C2D0F6
0x53FCD6513D02BEFE
```

The bounded helper maps a stream word to `[0, upper_bound)` by rejection:

```text
threshold = 2^64 mod upper_bound
draw x until x >= threshold
return x mod upper_bound
```

An `upper_bound` of zero returns `None` without consuming the stream.

Each attempt builds row slots in row-major order:

```text
[0 repeated row_weight times, 1 repeated row_weight times, ...]
```

Columns are visited in ascending order. For each column, select
`column_weight` row slots. A row may be selected at most once for the current
column; candidate slots whose row is already selected for that column are
skipped. If no non-duplicate row slot remains, the entire attempt is rejected.

The stream is created once from `seed` before attempt 1. Retry attempts continue
from the current stream state; they do not reseed or rewind. Exhausting
`retry_limit` attempts returns the typed exhaustion error.

Successful rows are canonicalized by sorting each row's supports ascending and
then sorting the row list lexicographically. This makes row-order-equivalent
parity-check matrices compare by a stable canonical representation.

For `n=6`, `m=4`, `column_weight=2`, `row_weight=3`, `seed=7`, and version 1,
the canonical rows are exactly:

```text
[[0, 1, 2], [0, 3, 4], [1, 3, 5], [2, 4, 5]]
```

Seed 8 must produce a different valid canonical matrix.

## Documentation

Add `qec-code/doc/regular_classical.md` and include it in the module rustdoc so
the exact v1 contract is available from source and rendered docs. Link it from
`qec-code/README.md`.

The docs must explicitly cover:

- algorithm versioning policy
- stream transition and output mapping
- bounded-index rejection rule
- row-slot ordering and column traversal
- duplicate-edge rejection
- retry-state progression
- canonical row ordering
- seed-7 golden stream words

## Testing

Add `qec-code/tests/regular_classical.rs`.

Required tests:

- the seed-7 fixture matches exactly
- seed 8 produces a different valid matrix
- row and column weights match the requested degrees
- invalid degree inputs reject zero dimensions, zero weights, zero retry limit,
  unsupported algorithm version, and mismatched stub counts
- seed-7 `SplitMix64V1` output words match the golden values
- `bounded_index_v1` is tested independently, including a high-rejection-bound
  case that consumes multiple stream words
- `n=3`, `m=3`, `column_weight=2`, `row_weight=2`, `seed=1`, version 1,
  `retry_limit=1` returns the typed exhaustion error

Required verification:

```text
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_matches_v1_fixture -- --exact
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_rejects_invalid_degrees -- --exact
cargo test -p qec-code --test regular_classical splitmix64_v1_seed7_matches_golden_words -- --exact
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_retry_limit_one_returns_exhausted -- --exact
cargo test -p qec-code
cargo test
```
