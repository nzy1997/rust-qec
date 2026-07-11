# Instruction-Wide Frame Noise Evidence Design

## Goal

Publish release evidence for issue #463 proving the d11/r100 fixture executes sparse frame noise instruction-wide. The bundle must be generated from a real release `rstim` process built with `benchmark-telemetry`, checked against Stim `1.15.0`, and rejected when telemetry, fixture/load semantics, correctness semantics, or provenance hashes are fabricated or stale.

## Approach

Use the existing frame-simulator telemetry from issues #461 and #462 as the release data source, but compile it when either debug assertions or the `benchmark-telemetry` feature is enabled. The CLI gains a global `--benchmark-telemetry-json <path>` flag. After a successful sampling command, it writes the actual per-instruction telemetry collected during that process. Without a successful process run there is no telemetry artifact for the Python runner to consume.

The Python runner is a focused release-evidence workflow rather than a general speed suite. It resolves the manifest case, runs the independent fixture inspector, measures exactly one `rstim sample --out_format b8` process for the requested release fixture, requires the emitted telemetry file, aggregates it into operation rows, and runs a separate `detect` correctness comparison against Stim for the detector/observable output. The timer scope is end-to-end child process execution: spawn through complete stdout/stderr drain and observed exit.

## Files

- `rstim/Cargo.toml`: add `benchmark-telemetry`.
- `rstim/src/sim/frame.rs`: enable and accumulate telemetry for `X_ERROR`, `DEPOLARIZE1`, and `DEPOLARIZE2` under debug or the feature.
- `rstim/src/cli.rs`: parse `--benchmark-telemetry-json`, reset telemetry before commands that can sample, and write telemetry JSON after successful execution.
- `benchmarks/rstim_vs_stim_simulator/run_frame_instruction_wide_benchmark.py`: generate the seven-file evidence bundle.
- `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`: validate raw-summary-report consistency, fixture-load and correctness semantics, provenance, and mandatory hashes.
- `benchmarks/rstim_vs_stim_simulator/tests/test_run_frame_instruction_wide_benchmark.py` and `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py`: cover positive paths and required negative controls.
- `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/`: committed checked release artifacts.

## Data Contract

Measured raw rows are operation aggregates from real runtime telemetry. Each row records `operation`, `instructions`, `targets` or `pairs`, `iterator_builds`, and `attempt_count`. For the selected fixture and `shots=1024`, the required totals are 203 `X_ERROR` instructions and 24,946,688 attempts, 200 `DEPOLARIZE1` instructions and 12,288,000 attempts, 400 `DEPOLARIZE2` instructions and 45,056,000 attempts, and totals of 803 builds and 82,290,688 attempts.

`fixture-load.json` remains independent of telemetry and reports flattened fixture characteristics: 24,362 `X_ERROR` targets, 12,000 `DEPOLARIZE1` targets, and 44,000 `DEPOLARIZE2` pairs, totaling 80,362 legacy setups. The checker must reject any attempt to substitute this legacy setup count for runtime iterator builds.

`correctness-summary.json` is a separate detector-output comparison for the same fixture, seed, and shots. It must record a passing `detect` comparison with 12,000 detectors and one observable, not a sample-mode or failed report.

## Provenance And Hashes

`environment.json` records git commit and dirty state, fixture/manifest/rstim binary paths and SHA-256 digests, six artifact digests, Stim/rstim/rustc versions, OS/CPU, exact runner and child argv, seed, release profile, and timer scope. `artifact-sha256.json` hashes only `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, `fixture-load.json`, and `correctness-summary.json`.

The checker validates semantic consistency before mandatory hashes so negative controls fail for the causal problem first.

## Testing

Tests exercise the runner with fake binaries, including a fake binary that produces valid sample bytes but no telemetry. Checker tests mutate runtime iterator builds, correctness mode/status, fixture/manifest/binary/artifact hashes, and missing hash manifests. Final verification runs the issue commands plus `cargo test`.
