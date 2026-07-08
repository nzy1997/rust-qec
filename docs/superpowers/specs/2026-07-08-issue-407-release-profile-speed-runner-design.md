# Issue 407 Release-Profile Speed Runner Design

## Objective

Add a focused Python runner for the public
`stim-style-surface-sample-d11-r100-b1024` `rstim`-vs-Stim speed case. The
runner must separate debug-profile evidence from release-profile evidence by
building or selecting the requested `rstim` binary, running the existing
selected-case perf workflow, writing the existing raw/summary/report artifacts,
and recording environment metadata.

## Context

Issue #406 intentionally records the current checked debug-profile performance
gap. This issue must not rewrite those checked artifacts or optimize sampler
internals. Existing `rstim perf run`, `rstim perf summarize`, and
`rstim perf report` already provide the selected-case raw JSONL, summary JSON,
and Markdown report behavior, including the `report-only Stim comparison`
context required by the verification command.

The new code belongs beside the fixture catalog under
`benchmarks/rstim_vs_stim_simulator/`. That directory already contains Python
entrypoints and tests for manifest validation and correctness verification.

## Selected Approach

Create `benchmarks/rstim_vs_stim_simulator/run_speed_case.py` as a thin
subprocess orchestration layer. It will:

- parse `--profile debug|release`, `--case`, `--warmup-rounds`,
  `--measure-rounds`, and `--out-dir`;
- build `cargo build -p rstim --bin rstim` for debug or
  `cargo build --release -p rstim --bin rstim` for release;
- select `target/debug/rstim` or `target/release/rstim`;
- run that binary's `perf run --case ... --out raw.jsonl`;
- run `perf summarize --in raw.jsonl --out summary.json`;
- run `perf report --in summary.json --out report.md`;
- write `environment.json` after successful perf artifacts exist.

This is preferred over adding a new Rust CLI subcommand because the issue asks
for a Python module interface under the benchmark fixture package. It is also
preferred over duplicating benchmark logic in Python because the Rust perf
commands already own variant selection, summary filtering, and Markdown
rendering.

## Environment Metadata

`environment.json` will include:

- `profile`: selected `debug` or `release`;
- `case_label`: selected case label;
- `warmup_rounds` and `measure_rounds`;
- `rustc_version`: stdout from `rustc --version`, or a failure marker;
- `cargo_version`: stdout from `cargo --version`, or a failure marker;
- `rstim_binary_path`: resolved selected binary path;
- `rstim_version`: stdout from `<binary>` with no subcommand, or a failure
  marker;
- `stim_cli`: object with `command`, `version`, `status`, and `stderr` for
  `stim --version`.

The required top-level fields from the issue are present directly:
`profile`, `rustc_version`, `cargo_version`, and `rstim_binary_path`. Stim CLI
version or failure status is represented by `stim_cli.status` plus either
`stim_cli.version` or `stim_cli.stderr`.

## Error Handling

Argument parsing rejects any profile other than `debug` or `release` before
creating output files. This preserves the negative control's requirement that a
bogus profile exits nonzero and does not leave `summary.json`.

For valid profiles, the runner creates `out-dir` before writing artifacts. A
failed build or perf command exits nonzero through `subprocess.run(...,
check=True)` and preserves already-written files for diagnosis. The script does
not apply wall-clock pass/fail thresholds and does not suppress slow results.

## Tests

Add Python unit coverage in
`benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_case.py`:

- debug and release profile plans build the expected cargo command and select
  the expected binary path;
- the runner calls `perf run`, `perf summarize`, and `perf report` on the
  selected `rstim` binary with the requested case and round counts;
- `summary.json` and `report.md` are not written by the Python layer itself;
- `environment.json` contains profile, case label, Rust/Cargo version fields,
  the selected binary path, and Stim CLI status/version;
- invalid profile parsing exits nonzero before creating `summary.json`.

Run the issue verification command for the release profile with one measured
round, the bogus-profile negative control, focused Python tests, and the
repository `cargo test` gate.

## Scope Limits

Do not update `benchmarks/rstim_vs_stim_simulator/results/full/` or any checked
#406 evidence. Do not tune `rstim` sampling speed. Do not add pass/fail timing
thresholds. Do not broaden the runner beyond a selected case label.

## Self-Review

- No placeholder text remains.
- The design keeps benchmark logic in the existing Rust perf commands.
- The interface exactly matches the issue's Python module command.
- The selected profile affects the built and executed `rstim` binary.
- The environment metadata includes all fields required by the issue.
- Invalid profiles fail before `summary.json` can be written.
