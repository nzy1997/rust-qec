# Issue 322 QEC-Code Random-Window Local Runner Design

## Context

Issue #322 asks for a reproducible local benchmark runner for the manifests
added by issue #321. The runner executes only the current local
`qec-code code css-distance random-window-upper-bound` implementation. It does
not import, vendor, run, or compare paper algorithms.

The #321 manifest contract is already present under
`benchmarks/qec_code_random_window/`. Each case contains `case_id`, `code_id`,
`distance_side`, `iterations`, `restarts`, `seed`, `target_weight`,
`target_upper_bound`, `baseline_key`, and `baseline_required`. The runner will
reuse the existing manifest loader and validator before running any cases.

GitHub issue API reads were blocked by the local sandbox proxy during this run.
The design uses the provided issue body, the merged #321 PR metadata, and the
local merged #321 artifacts.

## Approaches Considered

1. Add a standard-library Python runner that shells out to a local `qec-code`
   executable and records JSONL rows.
   - Pros: matches the requested interface, keeps measurement around the actual
     CLI subprocess, and needs no new dependencies.
   - Cons: users must build or install `qec-code` before running a successful
     benchmark.

2. Run `cargo run -p qec-code -- ...` from the Python runner.
   - Pros: works without installing the binary.
   - Cons: measures Cargo startup/build behavior, not just the local CLI run,
     and conflicts with the requested `qec-code ...` command.

3. Call Rust library functions directly from Python.
   - Pros: avoids subprocess handling.
   - Cons: does not measure the CLI path, skips the JSON contract the CLI
     intentionally requires, and would add an unnecessary FFI or wrapper layer.

Chosen approach: standard-library Python runner with subprocess execution. By
default it uses `QEC_CODE_BIN` when provided, otherwise `target/debug/qec-code`
when present, otherwise `qec-code` from `PATH`. This keeps the documented smoke
command usable after `cargo build -p qec-code` while still invoking the local
CLI binary rather than `cargo run`.

## Design

Add `benchmarks/qec_code_random_window/run_local.py`. The module provides:

- CLI entry point:
  `python3 -m benchmarks.qec_code_random_window.run_local --cases <manifest> --out <jsonl>`.
- Manifest validation through `validate_cases.load_manifest` and
  `validate_cases.validate_manifest`.
- Command-line overrides for `--seeds`, `--iterations`, `--restarts`, and
  `--target-weight`.
- Optional `--qec-code-bin` for tests and custom local installations.
- One subprocess per case and seed.
- Wall-clock measurement with `time.perf_counter()` immediately around
  `subprocess.run()`.
- One JSON object per case/seed written as JSONL.

Each subprocess command uses:

```text
qec-code code css-distance random-window-upper-bound \
  --code-id <code_id> \
  --iterations <iterations> \
  --restarts <restarts> \
  --seed <seed> \
  --target-weight <target_weight> \
  --json
```

The runner does not use `distance_side` to select a CLI mode because the current
random-window CLI has no side selector. The field is copied into result rows so
the manifest identity remains visible.

## Output Contract

Every row includes at least:

- `case_id`
- `code_id`
- `seed`
- `iterations`
- `restarts`
- `target_weight`
- `upper_bound`
- `elapsed_s`
- `status`
- `raw_cli_json`

Rows also include diagnostic fields such as `command`, `returncode`,
`distance_side`, `target_upper_bound`, `stdout_context`, `stderr_context`, and
`error` when useful.

The runner writes `status = "ok"` only when all of these are true:

- the subprocess exits with status 0
- stdout parses as JSON
- parsed JSON has `status = "completed"`
- parsed JSON has `method = "random-window-upper-bound"`
- parsed JSON has a positive integer `upper_bound`

Any failure writes a non-`ok` status and enough clipped stdout/stderr context to
diagnose the problem. The runner exits 0 only if every emitted row is `ok`; it
exits nonzero if manifest validation fails, subprocess launch fails, CLI output
is invalid, or any case/seed row is not successful.

## Testing

Add `benchmarks/qec_code_random_window/tests/test_run_local.py`.

The tests use real subprocess execution with a temporary fake `qec-code`
executable. This proves the runner builds the correct CLI command and handles
real process exit codes, stdout, and stderr without requiring slow benchmark
runs during unit tests.

Required test coverage:

- successful rows are emitted for each manifest case and requested seed
- overrides for seed, iterations, restarts, and target weight are reflected in
  both subprocess arguments and JSONL rows
- invalid `code_id` exits nonzero and does not emit an `ok` row
- non-JSON stdout exits nonzero and does not emit an `ok` row
- completed CLI JSON missing `upper_bound` exits nonzero and does not emit an
  `ok` row
- focused real smoke command writes one successful row per smoke case and seed

Required verification:

- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_run_local -q`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_run_local -q`
- the issue smoke command with `--seeds 7 --iterations 50 --restarts 1`
- `cargo test -p qec-code`
- `cargo test`
