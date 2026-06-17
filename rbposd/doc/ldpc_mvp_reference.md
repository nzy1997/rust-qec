# rbposd LDPC MVP Reference Contract

Date: 2026-04-22

This document locks the Task 1 MVP public surface for `rbposd`. The goal is to
give downstream crates a stable contract for initial integration while the
decoder internals remain under active development.

Included:
- `DecoderConfig` and its default contract:
  `max_bp_iterations=30`, `early_stop=true`, `bp_variant=MinimumSum`,
  `schedule=Parallel`, `osd_variant=Osd0`
- `LsdConfig` and its default contract:
  `method=LocalizedStatistics`, `lsd_order=0`
- `LsdMethod` with the first supported variant:
  `LocalizedStatistics`
- `ChannelModel` with:
  `Bsc { error_rate: f64 }` and `BitFlipProbabilities(Vec<f64>)`
- `DecodeError` variants:
  `EmptyMatrix`, `InvalidProbability`,
  `InvalidColumnIndex { column: usize, num_bits: usize }`,
  `DimensionMismatch { what: &'static str, expected: usize, actual: usize }`,
  `BpDidNotConverge`, `NoOsdSolution`,
  `UnsupportedLsdOrder { order: usize }`
- Error ergonomics for `DecodeError`:
  `Display` implementation and `impl std::error::Error`
- Crate exports:
  `pub mod config; pub mod error;`
  and top-level re-exports for
  `BpVariant, ChannelModel, DecoderConfig, LsdConfig, LsdMethod,
  OsdVariant, Schedule, DecodeError`

Excluded:
- Any decoding algorithm implementation (BP, OSD, or hybrid solver logic)
- Sparse/H matrix parsing, loading, or validation beyond public error typing
- Performance tuning, SIMD/parallel execution internals, and benchmarking hooks
- CLI/API integration outside this crate's foundational type contract

Reference fixtures:
- Repetition-style 4-check / 5-bit code with a single-flip syndrome that BP
  should solve without OSD.
- Small 2-check / 3-bit code that is solved by `OSD_0` when BP is disabled.
- Small 2-check / 3-bit code with equal reliability values that locks the OSD
  tie-break outcome.

## Parity Fixture Baseline

Static parity fixtures live in `rbposd/tests/fixtures/parity/`.

Seed cases:

- `bp_repetition_single_flip.json`
- `osd_small_sparse_code.json`
- `osd_equal_reliability_tiebreak.json`

Checked-in fixtures are the exact Rust stability baseline and are enforced by
reference tests in this repository.

Python `ldpc` is used by `rbposd/scripts/parity_harness.py` as a differential
comparison baseline for correction parity against the same cases.

Diagnostics drift (`diagnostics_mismatch`) is reported separately as
informational and does not fail parity on its own unless status or correction
parity also changes.

Cases with `max_bp_iterations=0` are also tracked separately when Rust and
Python `ldpc` both return valid residual-zero solutions but choose different
decode paths or corrections. These are reported as
`zero_iter_semantics_mismatch` and do not fail parity, because Rust treats
`max_bp_iterations=0` as "disable BP and run OSD" while Python `ldpc` does not
share that contract.

## LSD Public API Contract

Issue #88 adds `BpLsdDecoder` as a first-class public decoder family parallel
to `BpOsdDecoder`.

The supported construction path is:

```rust
let decoder = BpLsdDecoder::new(pcm, channel, LsdConfig::default())?;
let result = decoder.decode(&syndrome)?;
```

The issue #88 behavior is intentionally narrow:

- `LsdMethod::LocalizedStatistics` is the only public LSD method variant.
- `lsd_order=0` is the only supported order.
- `lsd_order>0` returns `DecodeError::UnsupportedLsdOrder`.
- successful decodes return `DecodeResult` and keep `used_osd=false`.
- the order-0 fallback is an API validity bridge, not the full LSD algorithm.

Full LSD post-BP search, nonzero-order behavior, borrowed LSD fixtures, and
Python `ldpc` differential harness coverage are owned by follow-on issues.
