# rstim-vs-Stim Simulator Evidence

This showcase is the reviewer-facing map for the `rstim`-vs-Stim simulator
evidence family. It explains the checked workload, the statistical correctness
workflow, the selected-case speed workflow, and the limits on any claim made
from those artifacts.

## What This Shows

The workload is the Stim-style surface-code sample case
`stim-style-surface-sample-d11-r100-b1024`. Its canonical circuit input is the
checked Stim-generated `.stim` fixture introduced by issue
[#385](https://github.com/nzy1997/rust-qec/issues/385), not a circuit regenerated
by `rstim`. The full fixture is
[`benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`](benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim).

The evidence has two independent tracks:

- statistical sample-correctness checks compare Stim and `rstim` observable
  rates on shared checked circuit text;
- speed evidence reruns only the selected simulator comparison case and then
  summarizes raw records into reviewer-readable shots/s and report-only
  `rstim`-vs-Stim ratios.

The older umbrella issue
[#38](https://github.com/nzy1997/rust-qec/issues/38) is historical context for the
surface-code benchmark direction. This page narrows that umbrella to the
recorded simulator workloads, commands, and environments below.

## Run It

Validate the smoke fixture catalog:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml
```

Run the smoke correctness verifier:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \
  --shots 20000 \
  --out /tmp/rstim-vs-stim-correctness.json
```

Run the selected speed case:

```sh
cargo run -p rstim --bin rstim -- perf run \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out /tmp/rstim-vs-stim-speed.jsonl
```

Summarize and render the speed evidence:

```sh
cargo run -p rstim --bin rstim -- perf summarize \
  --in /tmp/rstim-vs-stim-speed.jsonl \
  --out /tmp/rstim-vs-stim-summary.json
cargo run -p rstim --bin rstim -- perf report \
  --in /tmp/rstim-vs-stim-summary.json \
  --out /tmp/rstim-vs-stim-report.md
```

## Expected Result

The catalog validation command exits 0 and confirms that the smoke manifest
matches the checked fixtures.

The correctness verifier prints `PASS correctness smoke` for the current smoke
suite and writes `/tmp/rstim-vs-stim-correctness.json`. That JSON records each
case, selected rates and pair correlations, tolerances, sample counts, tool
status, stderr, and failure reasons. A future `WARN` or `FAIL` result should be
read as correctness evidence for that run, not hidden as a documentation
failure.

The selected speed run writes `/tmp/rstim-vs-stim-speed.jsonl` with records for
only `stim-style-surface-sample-d11-r100-b1024`. Available variants include
`stim-cli`, `rstim-interpreted`, and `rstim-compiled`; failed or unavailable
variants are represented with explicit statuses such as `tool_failed`,
`timed_out`, or `missing_variant`.

The summary JSON reports `median_shots_per_second` for completed sample
variants. The Markdown report contains the selected case label, `shots/s`, and
the phrase `report-only Stim comparison`. If `rstim` is slower than Stim, Stim
is unavailable, or a result is incomplete, that is still evidence when the
status and environment are recorded plainly.

Checked historical #406 speed evidence remains at
[`benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`](benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json)
with its companion report
[`benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md`](benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md).
That artifact records the earlier debug-profile selected-case gap and is kept
separate from later release-profile evidence.

Checked post-optimization release evidence is published separately as
[`benchmarks/rstim_vs_stim_simulator/results/release/summary.json`](benchmarks/rstim_vs_stim_simulator/results/release/summary.json),
[`benchmarks/rstim_vs_stim_simulator/results/release/report.md`](benchmarks/rstim_vs_stim_simulator/results/release/report.md),
and
[`benchmarks/rstim_vs_stim_simulator/results/release/environment.json`](benchmarks/rstim_vs_stim_simulator/results/release/environment.json).
This release-profile run records only
`stim-style-surface-sample-d11-r100-b1024` with 0 warmup rounds, 1 measured
round, and the environment metadata captured by the selected-case runner.

## Expanded Checked Evidence

### Correctness evidence

The checked correctness area combines the existing d11/r100 full-fixture
summary with eight source-grounded small-circuit distribution cases. The
recorded distribution run used 100,000 shots per case and seed 12345; all eight
catalogued cases and the checked d11/r100 summary record a passing status. This
is evidence for those cases, commands, seeds, and recorded tool versions only.

- [Full-fixture correctness summary](benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json)
- [Distribution summary](benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json)
- [Expanded correctness rollup](benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json)
- [Expanded correctness report](benchmarks/rstim_vs_stim_simulator/results/distributions/report.md)

### Release speed evidence

The following observations are scoped to the checked release-profile runs,
their single measured round, and their recorded environments:

| Case | Workload | `rstim` variant | `rstim` median wall time | Stim variant | Stim median wall time | Throughput | Case-scoped result |
|---|---:|---|---:|---|---:|---:|---|
| [`stim-style-surface-sample-d11-r100-b1024`](benchmarks/rstim_vs_stim_simulator/results/release/summary.json) | sample | `rstim-compiled` | 586.794 ms | `stim-cli` | 182.963 ms | 1,745 vs 5,597 shots/s | `rstim` recorded 3.21x slower |
| [`rep-sample-d13-r13`](benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/summary.json) | sample | `rstim-compiled` | 1.738 ms | `stim-cli` | 86.111 ms | 11.51M vs 232k shots/s | `rstim` recorded 49.6x faster |
| [`surface-detect-d13-r13`](benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/summary.json) | detect | `rstim-compiled` | 34.339 ms | `stim-cli` | 266.172 ms | n/a | `rstim` recorded 7.75x faster |
| [`stim-style-surface-dem-sample-d11-r100-b1024`](benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/summary.json) | sample_dem | `rstim-sample-dem` | 3,112.766 ms | `stim-sample-dem` | 383.724 ms | 329 vs 2,669 shots/s | `rstim` recorded 8.11x slower |

Companion reports and environments are checked at
[`results/release/`](benchmarks/rstim_vs_stim_simulator/results/release/report.md),
[`results/release-repetition-sample/`](benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/report.md),
[`results/release-surface-detect/`](benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/report.md),
and
[`results/release-dem-sample/`](benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/report.md).

These relationships describe only the named cases under the captured release
profiles and environments. They are not timing thresholds or cross-machine
gates.

### Historical debug gap

Issue #406's debug-profile selected d11/r100 sample remains a historical gap
record. Its checked [speed summary](benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json)
and [speed report](benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md)
are kept separate from the correctness area and all release-profile cards.

## Code

Fixture catalog and canonical circuit input:

- [`benchmarks/rstim_vs_stim_simulator/README.md`](benchmarks/rstim_vs_stim_simulator/README.md)
- [`benchmarks/rstim_vs_stim_simulator/cases.smoke.toml`](benchmarks/rstim_vs_stim_simulator/cases.smoke.toml)
- [`benchmarks/rstim_vs_stim_simulator/cases.full.toml`](benchmarks/rstim_vs_stim_simulator/cases.full.toml)
- [`benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`](benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim)
- [`benchmarks/rstim_vs_stim_simulator/validate_cases.py`](benchmarks/rstim_vs_stim_simulator/validate_cases.py)

Correctness evidence:

- [`benchmarks/rstim_vs_stim_simulator/verify_correctness.py`](benchmarks/rstim_vs_stim_simulator/verify_correctness.py)
- [`benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`](benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py)
- [`rstim/tests/sample_correctness_contract.rs`](rstim/tests/sample_correctness_contract.rs)

Speed evidence:

- [`rstim/src/perf/cases.rs`](rstim/src/perf/cases.rs)
- [`rstim/src/perf/runner.rs`](rstim/src/perf/runner.rs)
- [`rstim/src/perf/summary.rs`](rstim/src/perf/summary.rs)
- [`rstim/tests/cli_perf.rs`](rstim/tests/cli_perf.rs)
- [`rstim/tests/perf_summary.rs`](rstim/tests/perf_summary.rs)
- [`rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl`](rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl)
- [`benchmarks/rstim_vs_stim_simulator/run_speed_case.py`](benchmarks/rstim_vs_stim_simulator/run_speed_case.py)
- [`benchmarks/rstim_vs_stim_simulator/results/release/summary.json`](benchmarks/rstim_vs_stim_simulator/results/release/summary.json)
- [`benchmarks/rstim_vs_stim_simulator/results/release/report.md`](benchmarks/rstim_vs_stim_simulator/results/release/report.md)
- [`benchmarks/rstim_vs_stim_simulator/results/release/environment.json`](benchmarks/rstim_vs_stim_simulator/results/release/environment.json)

Issue context:

- [`#38 Performance Benchmarks on Surface Codes`](https://github.com/nzy1997/rust-qec/issues/38)
- [`#385 Add a shared rstim-vs-Stim simulator fixture catalog`](https://github.com/nzy1997/rust-qec/issues/385)
- [`#386 Add a statistical sample-correctness verifier against Stim`](https://github.com/nzy1997/rust-qec/issues/386)
- [`#390 Report shots/s and rstim-vs-Stim ratios for sample speed evidence`](https://github.com/nzy1997/rust-qec/issues/390)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-vs-stim-simulator.md
```

Expected result: the command exits 0, and this page links to the speed command
and correctness command.

Negative controls for this page:

- removing the `Limits` section must fail with `missing required section:
  Limits`;
- removing `python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness`
  must fail with `missing rstim-vs-Stim correctness command link`;
- removing `cargo run -p rstim --bin rstim -- perf run` must fail with
  `missing rstim-vs-Stim speed command link`.

Run the checker self-test for the negative-control fixtures:

```sh
python3 tools/check_showcase_docs.py --self-test
```

## Limits

Evidence applies to recorded workloads and recorded environments only. A local
run on another machine, toolchain, Stim installation, or thermal state can
produce different timings and availability statuses.

The smoke correctness command is a statistical wiring and evidence check. It
does not prove all possible circuits, seeds, detector paths, or simulator
features agree with Stim.

The selected speed command is report-only `rstim`-vs-Stim context. It does not
make broad `rstim` performance parity claims, and it does not turn a
cross-machine Stim ratio into a CI gate. The same-run `rstim`
compiled-vs-interpreted comparisons remain the gating candidate.

The canonical fixture is checked Stim-generated circuit text. This page does
not claim that an `rstim` generator reproduces Stim's generator output.

Slow, bad, failed, or incomplete benchmark output is still valid evidence when
the raw record, summary, report, and environment make that status visible. This
documentation should publish that status plainly instead of blocking on
optimization work.
