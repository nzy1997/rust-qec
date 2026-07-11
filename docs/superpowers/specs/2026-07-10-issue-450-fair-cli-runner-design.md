# Issue 450 Fair CLI Runner Design

Issue: #450

## Context

Issue #450 builds on the merged #449 fair CLI contract. The contract now pins
the canonical `stim_surface_d11_r100` manifest, fixture, Stim version,
bit-packed output format, expected byte count, seed policy, and symmetric argv
templates. This issue adds the executor that actually applies the same process
boundary to Stim and `rstim`.

No repository-specific `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md` files were
present in this worktree. Issue #450 has no comments. The dependency PR #467
for #449 is merged into `master` and added the fair manifest, validator, and
contract tests used by this runner.

## Approaches Considered

1. Add a standalone Python runner that imports `fair_cli_contract`, builds
   `rstim` once, validates the manifest, performs the known-answer preflight,
   times each CLI process with full stdout drain and successful exit, and
   writes the four requested artifacts.

2. Extend `run_speed_case.py` or `run_dem_speed_case.py` with a new mode. This
   would reuse existing output paths, but those runners have different command
   semantics and summarize existing `rstim perf` or DEM workloads rather than
   symmetric bit-packed CLI sampling.

3. Implement the timing runner in Rust. This would give strong typing, but it
   would duplicate the Python manifest contract and make fake CLI negative
   controls harder to express.

The selected approach is option 1. It keeps #450 scoped to the requested
Python module, reuses the #449 validator as the source of truth, and avoids
changing the simulator or existing speed runners.

## Runner

Create `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py` with this CLI:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_fair_cli \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 \
  --profile release \
  --warmup-rounds 2 \
  --measure-rounds 7 \
  --out-dir /tmp/rstim-fair-cli
```

The runner validates the manifest before creating benchmark artifacts or
starting measured processes. It builds `rstim` once through the existing
`run_speed_case.build_rstim` helper, resolves the Stim and `rstim` binaries,
and expands the canonical argv templates per round without shell evaluation.

Before timing, it writes a temporary circuit containing `X 0\nM 0\n`, runs both
expanded templates with `--shots 1` and `--in <temp circuit>`, and requires
stdout to equal the single byte `0x01`. A preflight failure returns a nonzero
exit before `raw.jsonl` is created.

For each variant, seeds are assigned in execution order from 0 through 8: two
warmups use round indexes 0 and 1, then seven measured rounds use indexes 0
through 6. Timing uses `time.perf_counter_ns()` around `subprocess.Popen`,
`communicate()`, and return-code validation so elapsed time covers spawn,
complete stdout and stderr drain, and process exit.

Each successful run must exit `0` and produce exactly `1552384` stdout bytes.
The runner records SHA-256 of stdout but does not persist the raw sampled data.

## Artifacts

The output directory contains exactly the requested benchmark artifacts:

- `raw.jsonl`: 18 records, ordered by variant and then seed within each
  variant. Each record contains `case_id`, `variant`, `phase`, `round_index`,
  `seed`, `argv`, `shots`, `measurement_count`, `output_format`,
  `timer_scope`, `elapsed_ns`, `actual_output_bytes`, `stdout_sha256`, and
  `exit_code`.
- `summary.json`: derived only from the 14 measured records. It records case
  metadata, per-variant sample counts, median and mean elapsed nanoseconds,
  min and max elapsed nanoseconds, total output bytes, and stdout hashes.
- `report.md`: rendered from `summary.json`, not from raw warmup records.
- `environment.json`: records git commit, OS, CPU model, profile, timer
  scope, seed policy, Stim and `rstim` versions, Rust version, fair manifest
  and source manifest paths plus SHA-256 values, fixture path plus SHA-256,
  resolved binary paths plus SHA-256 values, exact expanded argv for all
  rounds, warmup and measure round counts, and the known-answer preflight
  result.

Stim version must resolve to `1.15.0`. The runner accepts either `stim
--version` output such as `stim 1.15.0` or the existing Python-module fallback
used by `run_speed_case`, but it rejects any other version before timing.

## Tests

Add `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`.

Coverage:

- the artifact-writing workflow with fake Stim and `rstim` CLIs that emit the
  required byte count and pass the known-answer circuit;
- independent parsing of `raw.jsonl`, verifying 18 records, two variants, two
  warmups and seven measured records per variant, seeds 0 through 8 per
  variant, exact required fields, exit code `0`, byte count `1552384`, and
  recorded argv/provenance;
- independent derivation of measured sample counts and summary aggregates,
  proving warmups do not affect sample counts or aggregate values;
- environment provenance for manifest, source manifest, fixture, resolved
  binary paths, exact expanded argv, round counts, and preflight result;
- negative control where a fake CLI writes all but its final byte, waits at
  least 150 ms, writes the final byte, closes stdout, waits at least another
  150 ms, and exits `0`; accepted elapsed time must include both delays;
- negative control where the known-answer circuit returns `0x00`; the runner
  must fail before `raw.jsonl` exists.

Tests use temporary manifests and fake executables to avoid depending on local
Stim installation or release build speed for most behavior. The issue's
verification command still exercises the canonical manifest and real binaries.

## Scope Limits

- Do not cache samplers between CLI invocations.
- Do not publish checked timing evidence.
- Do not set a speed-ratio gate.
- Do not include build time in benchmark samples.
- Do not change existing historical #406 artifacts or checked results.
- Do not change the Rust sampler or CLI unless tests prove the runner cannot
  satisfy the contract against existing behavior.

## Verification

Run:

```sh
rm -rf /tmp/rstim-fair-cli
python3 -m benchmarks.rstim_vs_stim_simulator.run_fair_cli \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 \
  --profile release --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-fair-cli
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
cargo test
```

The runner command must print:

```text
PASS symmetric fair CLI runner variants=2 warmups=4 measured=14 bytes_per_run=1552384
```

## Self Review

- No incomplete requirements remain.
- The design reuses the #449 manifest and validator instead of duplicating the
  contract.
- The timing scope includes process spawn, stdout and stderr drain, and process
  exit.
- Warmup records are written to raw output but excluded from summary
  aggregates.
- Negative controls cover premature read/stdout-close timing and preflight
  failure before artifact creation.
