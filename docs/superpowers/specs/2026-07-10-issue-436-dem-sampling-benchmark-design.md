# Issue 436 DEM Sampling Benchmark Design

## Context

Issue #436 adds checked `rstim` vs Stim detector-error-model sampling evidence for
the existing d11/r100 Stim-style surface fixture. The dependency from issue #433
is already merged: `run_speed_case.py` and `run_speed_suite.py` provide the
current build, environment, raw JSONL, summary JSON, and Markdown report pattern
for selected benchmark artifacts.

No repository-specific `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md` instructions
were present in this worktree. The live issue has no comments, and no pull
request exists yet for this worker branch.

## Approaches Considered

1. Extend Rust `PerfWorkload` with `SampleDem`.
   This would make DEM sampling part of the core `rstim perf` registry, but it
   requires broad changes across case definitions, variant discovery, summary
   logic, reports, and tests for a single checked external artifact.

2. Add a Python-side DEM runner that mirrors the selected-case artifact bundle.
   This keeps the work scoped to the benchmark package, invokes `stim
   sample_dem` and `rstim sample_dem` directly on the same pinned `.dem` input,
   and emits the same `raw.jsonl`, `summary.json`, `report.md`, and
   `environment.json` shape expected by issue #437.

3. Generate ad hoc artifacts without a reusable runner.
   This would satisfy one local run but would not give future checkers or users
   a reproducible command.

The selected approach is option 2. It matches the issue recommendation, avoids
unrelated Rust perf changes, and still creates checked, reproducible artifacts.

## Fixture And Metadata

Add a pinned DEM fixture beside the checked d11/r100 `.stim` fixture:

- `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem`
- `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json`

The metadata records the source circuit path, generation command, source circuit
SHA-256, DEM SHA-256, expected detectors, expected observables, shots, and
fixture provenance. The runner validates that metadata before timing and rejects
disagreements with a `DEM metadata mismatch` message. The required counts are
`expected_detectors = 12000` and `expected_observables = 1`, matching
`cases.full.toml`.

## Runner

Add `benchmarks.rstim_vs_stim_simulator.run_dem_speed_case` with this interface:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_dem_speed_case \
  --profile release \
  --case stim-style-surface-dem-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
```

The runner builds `rstim` with the existing `run_speed_case.build_rstim`
helper, validates the DEM metadata, and times two variants:

- `stim-sample-dem`: `stim sample_dem --shots 1024`
- `rstim-sample-dem`: `<rstim> sample_dem --shots 1024`

Each process receives the same DEM text on stdin and sends stdout to
`subprocess.DEVNULL` so file output cost is not included. The raw JSONL records
include completed status, workload `sample_dem`, report-only tier, detector and
observable counts, shot count, wall time, metadata hashes, and variant identity.

The summary groups raw records by case and variant, records medians, exposes
completed variants, and records issues only for malformed or missing evidence.
No speed threshold or performance claim is added.

## Checker

Add `tools/check_rstim_vs_stim_release_dem_speed_case.py` so issue #437 can
consume the checked directory. The checker validates:

- `raw.jsonl`, `summary.json`, `report.md`, and `environment.json` exist.
- `summary.json` contains the requested case.
- required variants `stim-sample-dem` and `rstim-sample-dem` are present and
  completed.
- the environment records profile, case label, command line, DEM path, DEM
  hash, source circuit hash, and metadata counts.

On success it prints:

```text
PASS release DEM speed case stim-style-surface-dem-sample-d11-r100-b1024
```

## Tests

Add Python unit coverage for:

- metadata validation, including a bad metadata fixture rejected with `DEM
  metadata mismatch`;
- runner command construction, build-once behavior, artifact writes, and
  environment fields;
- checker success and missing/failed variant negative controls.

The required verification commands are:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_dem_speed_case --profile release --case stim-style-surface-dem-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
python3 tools/check_rstim_vs_stim_release_dem_speed_case.py --results-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample --case stim-style-surface-dem-sample-d11-r100-b1024 --required-variants stim-sample-dem,rstim-sample-dem
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_dem_speed_case -q
python3 -m unittest tools.test_check_rstim_vs_stim_release_dem_speed_case -q
cargo test
```

## Self Review

- No placeholders remain.
- The design uses one runner and one checker, with no broad Rust perf changes.
- The artifact path and required variants match issue #436 and issue #437.
- The negative control is explicit and uses the required `DEM metadata mismatch`
  message.
