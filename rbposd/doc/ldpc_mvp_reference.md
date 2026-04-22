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
- `rbposd/tests/smoke.rs`:
  default configuration and channel model contract checks
- `rbposd/tests/smoke.rs`:
  decode error formatting and `std::error::Error` trait conformance check
- `rbposd/doc/ldpc_mvp_reference.md`:
  this written contract used as the task-level source of truth
