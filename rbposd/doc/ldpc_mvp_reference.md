# rbposd LDPC MVP Reference Contract

Date: 2026-04-22

This document locks the Task 1 MVP public surface for `rbposd`. The goal is to
give downstream crates a stable contract for initial integration while the
decoder internals remain under active development.

Included:
- `DecoderConfig` and its default contract:
  `max_bp_iterations=30`, `early_stop=true`, `bp_variant=MinimumSum`,
  `schedule=Parallel`, `osd_variant=Osd0`
- `ChannelModel` with:
  `Bsc { error_rate: f64 }` and `BitFlipProbabilities(Vec<f64>)`
- `DecodeError` variants:
  `EmptyMatrix`, `InvalidProbability`,
  `InvalidColumnIndex { column: usize, num_bits: usize }`,
  `DimensionMismatch { what: &'static str, expected: usize, actual: usize }`,
  `BpDidNotConverge`, `NoOsdSolution`
- Error ergonomics for `DecodeError`:
  `Display` implementation and `impl std::error::Error`
- Crate exports:
  `pub mod config; pub mod error;`
  and top-level re-exports for
  `BpVariant, ChannelModel, DecoderConfig, OsdVariant, Schedule, DecodeError`

Excluded:
- Any decoding algorithm implementation (BP, OSD, or hybrid solver logic)
- Sparse/H matrix parsing, loading, or validation beyond public error typing
- Performance tuning, SIMD/parallel execution internals, and benchmarking hooks
- CLI/API integration outside this crate's foundational type contract

Reference fixtures:
- Repetition-style 4-check / 5-bit code with a single-flip syndrome that BP
  should solve without OSD.
- Small 2-check / 3-bit code that is solved by `OSD_0` when BP is disabled.
- Small sparse non-identity matrix built from sparse columns to verify
  constructor symmetry.

## Parity Fixture Baseline

Static parity fixtures live in `rbposd/tests/fixtures/parity/`.

Seed cases:

- `bp_repetition_single_flip.json`
- `osd_small_sparse_code.json`
- `osd_equal_reliability_tiebreak.json`

Checked-in fixtures must include an `expected` outcome copied from the Python
`ldpc` reference behavior. Dynamically generated scan cases may omit
`expected`.
