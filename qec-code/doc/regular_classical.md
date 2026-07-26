# Deterministic Regular Classical Matrices

`qec_code::regular_classical` provides the repository-owned deterministic
sampler for regular binary parity-check matrices. It is intended for random
code families that need reproducible sampling across Rust versions and
platforms.

## Versioning Policy

The `algorithm_version` field is part of every generation request. Version 1 is
immutable: a behavioral change to the stream, bounded-index rule, stub
ordering, retry progression, duplicate-edge handling, or canonicalization must
use a new algorithm version rather than changing version 1.

## SplitMix64V1

`SplitMix64V1` starts with:

```text
state = seed
```

Each `next_u64()` call advances and maps the state with wrapping `u64`
arithmetic:

```text
state = state + 0x9E3779B97F4A7C15 mod 2^64
z = state
z = (z xor (z >> 30)) * 0xBF58476D1CE4E5B9 mod 2^64
z = (z xor (z >> 27)) * 0x94D049BB133111EB mod 2^64
output = z xor (z >> 31)
```

For seed 7, the first eight output words are:

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

## Bounded Index

`bounded_index_v1(stream, upper_bound)` maps stream words to the unbiased range
`[0, upper_bound)`.

For `upper_bound == 0`, it returns `None` and does not consume the stream. For
positive bounds:

```text
threshold = 2^64 mod upper_bound
draw x = stream.next_u64()
if x < threshold, reject x and draw again
return x mod upper_bound
```

The accepted interval has a length divisible by `upper_bound`, so each returned
index is selected by the same number of accepted `u64` words.

## Matrix Generation

Validation happens before the stream is created and before any random word is
consumed:

- `algorithm_version` must be 1.
- `column_count`, `row_count`, `column_weight`, `row_weight`, and
  `retry_limit` must be greater than zero.
- `column_weight` must be at most `row_count`.
- `row_weight` must be at most `column_count`.
- `column_count * column_weight` and `row_count * row_weight` must not overflow
  `usize`.
- The checked column and row stub counts must be equal.

Each attempt starts with row slots in row-major order:

```text
[0 repeated row_weight times, 1 repeated row_weight times, ...]
```

Columns are visited in ascending order. For each column, version 1 selects
`column_weight` row slots with `bounded_index_v1`. A row may appear at most once
for the current column. Slots whose row is already selected for the current
column are skipped before the bounded index is applied. If no non-duplicate row
slot remains for the current column, the attempt is rejected.

The stream is created once from `seed` before attempt 1. Retry attempts continue
from the current stream state; they do not reseed or rewind. Exhausting
`retry_limit` attempts returns `RegularClassicalMatrixGenerationExhausted`.

On success, every row support is sorted ascending, and then the rows are sorted
lexicographically. This canonical row ordering makes row-order-equivalent
parity-check matrices compare identically.

For:

```text
column_count = 6
row_count = 4
column_weight = 2
row_weight = 3
seed = 7
algorithm_version = 1
```

the canonical rows are:

```text
[[0, 1, 2], [0, 3, 4], [1, 3, 5], [2, 4, 5]]
```
