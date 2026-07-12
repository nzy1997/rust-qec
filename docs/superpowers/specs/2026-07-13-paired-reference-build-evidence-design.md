# Paired Reference-Build Evidence Design

## Context

Issue #490 refreshes the existing
`benchmarks/rstim_vs_stim_simulator/results/reference-build-release` evidence
slot. The current slot has two variants and its `summary.json` SHA-256 is
`614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5`; that
file becomes `baseline-summary.json` before the refreshed evidence is written.

The dependency issue #489 is merged. The production packed-reference path now
folds the selected surface-code repeat and reports deterministic phase
counters: zero canonical materializations, one executed repeat iteration, and
98 skipped repeat iterations.

## Chosen Approach

Use the existing reference-build runner and bundle checker, but expand the
variant set from two to three:

- `stim-reference-b8`, backend `stim_reference`.
- `rstim-canonical-reference-b8`, backend `canonical_roundtrip`, benchmark-only.
- `rstim-direct-repeat-reference-b8`, backend `direct_inverse_repeat_folded`,
  production.

The canonical rstim variant is exposed only through a benchmark worker flag. It
does not change sampler routing or `build_reference_sample_with_decision`, so
production code continues to use the direct packed path from #489.

## Architecture

`rstim_reference_build_worker` gains a CLI strategy option used by the benchmark
runner. The default strategy is direct, preserving production semantics. The
canonical strategy builds the same reference bytes through the existing executor
roundtrip and returns phase counters that record canonical materialization work.

The Python runner runs all three variants in one process invocation with the
same fixture, two warmups, and seven measured rounds. It asks rstim workers for
phase counters on every build record so the checker can validate deterministic
direct-path telemetry from the artifact itself.

The checker validates semantic records before artifact hashes. It derives the
summary and report from raw records, enforces identical pinned byte digests and
1,516-byte packed output across all variants, checks direct phase counters, and
requires:

```text
rstim-canonical-reference-b8 median_elapsed_ns
/
rstim-direct-repeat-reference-b8 median_elapsed_ns >= 2.0
```

## Data Shape

`raw.jsonl` contains 27 records: three variants times nine rounds. Each record
keeps the existing payload fields and adds phase counters for rstim records. The
Stim records may omit phase counters.

`summary.json` contains three variants, 21 measured records, and a
`direct_speedup` number rounded from the same-run median ratio. Each variant
continues to include count, min, median, max, backend, parse count, final
reference-build count, packed byte length, measurement bits, and byte SHA-256.

`environment.json` records both executed worker argv and canonical logical argv
for all three variants. The canonical rstim argv includes the benchmark-only
strategy flag. The direct rstim argv uses the default production strategy.

`artifact-sha256.json` hashes `raw.jsonl`, `summary.json`, `baseline-summary.json`,
`report.md`, and `environment.json`.

## Error Handling

Runner validation fails before writing artifacts if any worker returns an
unexpected backend, parse count, measurement count, packed byte length, byte
digest, timer scope, or missing rstim phase counters.

Checker validation fails in this order:

1. Required files and raw semantic invariants.
2. Decoded packed bytes and pinned byte digests.
3. Variant backend, phase counters, round counts, and median speedup.
4. Derived summary and report.
5. Environment provenance.
6. Artifact hash manifest.

This order preserves the issue's negative controls: changed output bytes fail
semantically before artifact hashes; a canonicalized production strategy fails
on nonzero canonical materializations; speedup below 2 fails with the required
message.

## Testing

Update existing Python unit tests for the runner and checker to cover the
three-variant shape, direct speedup, direct phase counters, benchmark-only
canonical strategy, baseline-summary preservation, and the specified negative
controls.

Run focused verification first:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark
python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
```

Then run the required Rust verification:

```sh
cargo test
```

The final checker output must begin:

```text
PASS packed reference-build evidence variants=3 direct_speedup=
```
