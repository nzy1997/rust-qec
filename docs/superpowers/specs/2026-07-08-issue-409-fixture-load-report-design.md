# Issue 409 Fixture Load Report Design

## Context

Issue #409 asks for a reviewer-friendly inspection command for the checked
`stim_surface_d11_r100` fixture in
`benchmarks/rstim_vs_stim_simulator/cases.full.toml`. The command must not run
timing benchmarks or optimize simulator code. It must deterministically report
the expanded operation load for the selected fixture, including operation
counts, target counts, measurement counts, detector counts, observable counts,
repeat depth, and repeat expansion.

The benchmark package already has small Python CLIs that load TOML manifests,
validate case metadata, resolve fixture paths relative to the benchmark package,
and print PASS-style human summaries. `validate_cases.py` already imports the
Python `stim` package to parse fixtures and compare manifest counts.

## Noninteractive Decisions

This Agent Desk run is noninteractive. The issue body provides complete
acceptance criteria, so the design is approved automatically under the standing
policy.

The selected design is a Stim-backed Python inspector. It is preferred because
the repository already depends on Python `stim` in this benchmark package, Stim
parses the fixture syntax correctly, and `stim.Circuit.flattened()` gives a
deterministic expanded instruction stream without writing a partial Stim
parser.

Rejected alternatives:

- A custom line walker could count this fixture, but it would need to duplicate
  Stim parsing details for arguments, targets, comments, and nested repeat
  blocks.
- A Rust `rstim` CLI subcommand would place benchmark-only reporting inside the
  simulator binary and would be larger than the issue needs.

## Interface

Create `benchmarks/rstim_vs_stim_simulator/inspect_fixture_load.py` with this
entry point:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load \
  --case stim_surface_d11_r100
```

Command options:

- `--case CASE_ID` selects exactly one case from the manifest and is required.
- `--manifest PATH` defaults to
  `benchmarks/rstim_vs_stim_simulator/cases.full.toml`.
- `--format text|json` defaults to `text`.
- `--out PATH` writes the deterministic report body to a file. Without
  `--out`, the report body is printed to stdout.

When `--format json --out PATH` is used, stdout still prints a short
PASS-style summary for humans while the JSON report is written to `PATH`.
Missing cases return a nonzero exit code and print an error naming the requested
case id.

## Report Model

The JSON report is a dictionary with stable key ordering when serialized:

- `case_id`, `manifest_path`, `input_path`, and case metadata from the selected
  manifest.
- `expected_measurements`, `expected_detectors`, and `expected_observables`
  copied from the selected case.
- `actual_measurements`, `actual_detectors`, and `actual_observables` from
  `stim.Circuit`.
- `expanded_operation_count`, defined as flattened instruction count plus one
  logical `REPEAT` expansion marker per repeat-body execution. For the selected
  fixture this is `14547`.
- `flattened_operation_count`, the number of concrete Stim instructions after
  `stim.Circuit.flattened()`. For the selected fixture this is `14448`.
- `repeat_block_count`, `repeat_depth`, and `repeat_expansion_count`, where
  `repeat_expansion_count` is the number of repeat-body executions materialized
  by expansion. For the selected fixture this is `99`.
- `operations`, a name-keyed dictionary. Each operation entry contains
  `operation_count`, `target_count`, and `measurement_count`.

`operations["REPEAT"]` reports the logical repeat expansion markers so
`sum(entry["operation_count"] for entry in operations.values())` equals
`expanded_operation_count`. Concrete Stim instruction counts are reported under
their own gate names from the flattened circuit.

## Error Handling

Manifest loading uses the existing `load_manifest` helper. The inspector runs
the existing `validate_manifest` checks before selecting a case so invalid
manifests fail with the same path-prefixed error style as `validate_cases.py`.

The selected case path is resolved with the same benchmark-package fallback
contract used by `verify_correctness.py`: paths under the manifest directory
win when they stay in the benchmark package; otherwise they are resolved under
`benchmarks/rstim_vs_stim_simulator`.

If the parsed Stim counts disagree with the manifest's expected measurement,
detector, or observable counts, the command fails before writing a report.

## Testing

Add `benchmarks/rstim_vs_stim_simulator/tests/test_inspect_fixture_load.py`.
The tests cover:

- JSON report generation for `stim_surface_d11_r100`, including the exact
  values required by the issue: `expected_measurements = 12121`,
  `expected_detectors = 12000`, `expected_observables = 1`,
  `expanded_operation_count = 14547`,
  `operations["DEPOLARIZE2"]["target_count"] = 88000`, and
  `operations["DETECTOR"]["operation_count"] = 12000`.
- CLI JSON output with `--out`, confirming the PASS summary remains on stdout.
- Missing-case rejection that names `no_such_case`.
- A compact nested-repeat fixture for repeat depth and repeat expansion.

Verification commands:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_inspect_fixture_load
python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load \
  --case stim_surface_d11_r100 \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  --format json \
  --out /tmp/stim-surface-load.json
python3 - <<'PY'
import json
from pathlib import Path
report = json.loads(Path('/tmp/stim-surface-load.json').read_text())
assert report['case_id'] == 'stim_surface_d11_r100'
assert report['expected_measurements'] == 12121
assert report['expected_detectors'] == 12000
assert report['expected_observables'] == 1
assert report['expanded_operation_count'] == 14547
assert report['operations']['DEPOLARIZE2']['target_count'] == 88000
assert report['operations']['DETECTOR']['operation_count'] == 12000
print('PASS selected fixture load report matches checked d11/r100 workload')
PY
if python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load \
  --case no_such_case \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml; then
  echo 'unexpected missing-case success' >&2
  exit 1
fi
cargo test
```
