# Issue 409 Fixture Load Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic fixture inspection command that reports expanded operation load for `stim_surface_d11_r100`.

**Architecture:** Implement a small Python CLI in `benchmarks/rstim_vs_stim_simulator` that loads the existing TOML manifest, validates it, parses the selected `.stim` fixture with Python `stim`, and counts flattened instructions plus logical repeat expansion markers. Tests exercise direct report generation, CLI output, missing-case failure, and nested repeat accounting.

**Tech Stack:** Python 3.11+, `argparse`, `json`, `tomllib`, `unittest`, Python `stim`, existing `validate_cases.py` and `verify_correctness.py` helpers.

## Global Constraints

- Use `benchmarks/rstim_vs_stim_simulator/cases.full.toml` as the default manifest.
- Support `python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load --case stim_surface_d11_r100`.
- Support `--manifest`, `--format text|json`, and `--out`.
- Output must be deterministic and independent of timing.
- JSON report must include `case_id`, `expected_measurements`, `expected_detectors`, `expected_observables`, `expanded_operation_count`, and `operations`.
- For `stim_surface_d11_r100`, report `expected_measurements = 12121`, `expected_detectors = 12000`, `expected_observables = 1`, `expanded_operation_count = 14547`, `operations["DEPOLARIZE2"]["target_count"] = 88000`, and `operations["DETECTOR"]["operation_count"] = 12000`.
- Missing case `no_such_case` must be rejected with a nonzero exit code and an error message naming `no_such_case`.
- Do not run timing benchmarks.
- Do not optimize simulator code in this issue.

---

### Task 1: Add Inspector Tests and CLI Implementation

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/inspect_fixture_load.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_inspect_fixture_load.py`

**Interfaces:**
- Consumes: `load_manifest(path: Path) -> dict[str, Any]` and `validate_manifest(manifest: dict[str, Any], base_dir: Path) -> list[str]` from `benchmarks.rstim_vs_stim_simulator.validate_cases`.
- Consumes: `resolve_case_input_path(raw_path: str, base_dir: Path) -> Path` from `benchmarks.rstim_vs_stim_simulator.verify_correctness`.
- Produces: `find_case(manifest: dict[str, Any], case_id: str) -> dict[str, object] | None`.
- Produces: `summarize_circuit(circuit: stim.Circuit) -> dict[str, object]`.
- Produces: `build_report(case: dict[str, object], *, manifest_path: Path, base_dir: Path) -> dict[str, object]`.
- Produces: `format_text_report(report: dict[str, object]) -> str`.
- Produces: `summary_line(report: dict[str, object]) -> str`.
- Produces: `main(argv: list[str] | None = None) -> int`.

- [ ] **Step 1: Write the failing tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_inspect_fixture_load.py` with:

```python
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import stim

from benchmarks.rstim_vs_stim_simulator.inspect_fixture_load import (
    build_report,
    summarize_circuit,
)
from benchmarks.rstim_vs_stim_simulator.validate_cases import load_manifest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
FULL_MANIFEST = PACKAGE_DIR / "cases.full.toml"


def run_inspector(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.inspect_fixture_load",
            *args,
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


class InspectFixtureLoadReportTest(unittest.TestCase):
    def test_full_fixture_report_matches_issue_contract(self) -> None:
        manifest = load_manifest(FULL_MANIFEST)
        case = manifest["cases"][0]

        report = build_report(case, manifest_path=FULL_MANIFEST, base_dir=FULL_MANIFEST.parent)

        self.assertEqual(report["case_id"], "stim_surface_d11_r100")
        self.assertEqual(report["expected_measurements"], 12121)
        self.assertEqual(report["expected_detectors"], 12000)
        self.assertEqual(report["expected_observables"], 1)
        self.assertEqual(report["actual_measurements"], 12121)
        self.assertEqual(report["actual_detectors"], 12000)
        self.assertEqual(report["actual_observables"], 1)
        self.assertEqual(report["flattened_operation_count"], 14448)
        self.assertEqual(report["repeat_depth"], 1)
        self.assertEqual(report["repeat_expansion_count"], 99)
        self.assertEqual(report["expanded_operation_count"], 14547)
        self.assertEqual(report["operations"]["DEPOLARIZE2"]["target_count"], 88000)
        self.assertEqual(report["operations"]["DETECTOR"]["operation_count"], 12000)
        self.assertEqual(report["operations"]["REPEAT"]["operation_count"], 99)

    def test_cli_writes_json_report_and_prints_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "load.json"

            result = run_inspector(
                "--case",
                "stim_surface_d11_r100",
                "--manifest",
                str(FULL_MANIFEST),
                "--format",
                "json",
                "--out",
                str(out),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("PASS fixture load stim_surface_d11_r100", result.stdout)
            self.assertEqual(result.stderr, "")
            report = json.loads(out.read_text())
            self.assertEqual(report["expanded_operation_count"], 14547)
            self.assertEqual(report["operations"]["DEPOLARIZE2"]["target_count"], 88000)

    def test_missing_case_is_rejected_with_requested_id(self) -> None:
        result = run_inspector(
            "--case",
            "no_such_case",
            "--manifest",
            str(FULL_MANIFEST),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no_such_case", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_nested_repeat_summary_counts_depth_and_expansion_markers(self) -> None:
        circuit = stim.Circuit(
            """
            REPEAT 2 {
                M 0
                REPEAT 3 {
                    DETECTOR rec[-1]
                }
            }
            """
        )

        summary = summarize_circuit(circuit)

        self.assertEqual(summary["flattened_operation_count"], 8)
        self.assertEqual(summary["repeat_block_count"], 2)
        self.assertEqual(summary["repeat_depth"], 2)
        self.assertEqual(summary["repeat_expansion_count"], 8)
        self.assertEqual(summary["expanded_operation_count"], 16)
        self.assertEqual(summary["operations"]["M"]["operation_count"], 2)
        self.assertEqual(summary["operations"]["DETECTOR"]["operation_count"], 6)
        self.assertEqual(summary["operations"]["REPEAT"]["operation_count"], 8)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests to verify they fail for the missing module**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_inspect_fixture_load
```

Expected: FAIL or ERROR because `benchmarks.rstim_vs_stim_simulator.inspect_fixture_load` does not exist yet.

- [ ] **Step 3: Implement the inspector**

Create `benchmarks/rstim_vs_stim_simulator/inspect_fixture_load.py` with the CLI and helper functions described in the Interfaces block. Important implementation details:

```python
MEASUREMENT_GATES = {"M", "MX", "MY", "MR", "MRX", "MRY", "MRZ", "MXX", "MYY", "MZZ", "MPP", "MPAD"}
DEFAULT_MANIFEST = Path("benchmarks/rstim_vs_stim_simulator/cases.full.toml")
```

Use `stim.Circuit(input_path.read_text())` to parse the fixture. Count concrete operations from `circuit.flattened()`. Count repeat expansion markers recursively with a helper that multiplies nested repeat invocations by the parent expansion factor:

```python
def _collect_repeat_stats(circuit: stim.Circuit, *, multiplier: int = 1, depth: int = 0) -> RepeatStats:
    stats = RepeatStats()
    for instruction in circuit:
        if isinstance(instruction, stim.CircuitRepeatBlock):
            repeat_count = int(instruction.repeat_count)
            expanded_invocations = multiplier * repeat_count
            stats.repeat_block_count += 1
            stats.repeat_expansion_count += expanded_invocations
            stats.repeat_depth = max(stats.repeat_depth, depth + 1)
            stats += _collect_repeat_stats(
                instruction.body_copy(),
                multiplier=expanded_invocations,
                depth=depth + 1,
            )
    return stats
```

Set `expanded_operation_count = flattened_operation_count + repeat_expansion_count`. Add a synthetic `REPEAT` entry to `operations` when `repeat_expansion_count > 0` so the per-operation totals add up to `expanded_operation_count`.

Serialize JSON with `json.dumps(report, indent=2, sort_keys=True) + "\n"`. When `--out` is present, write the report body to that path and print only `summary_line(report)` to stdout. When `--format json` is used without `--out`, print JSON to stdout and the summary to stderr. Missing cases and validation errors print to stderr and return `1`.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_inspect_fixture_load
```

Expected: PASS.

- [ ] **Step 5: Run the issue verification command**

Run:

```sh
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
```

Expected: both PASS lines appear, including `PASS selected fixture load report matches checked d11/r100 workload`.

- [ ] **Step 6: Run the missing-case negative control**

Run:

```sh
if python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load \
  --case no_such_case \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml; then
  echo 'unexpected missing-case success' >&2
  exit 1
fi
```

Expected: command exits 0 overall because the inner inspector rejects `no_such_case`.

- [ ] **Step 7: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/inspect_fixture_load.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_inspect_fixture_load.py
git commit -m "feat: add fixture load inspector"
```

### Task 2: Document the Inspector Command

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`

**Interfaces:**
- Consumes: `python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load --case stim_surface_d11_r100 --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml --format json --out /tmp/stim-surface-load.json`.
- Produces: A README section named `Inspect Fixture Load` with the command and the key checked counts.

- [ ] **Step 1: Add README documentation**

Add this section after `## Validate` and before `## Correctness Verification`:

````markdown
## Inspect Fixture Load

Inspect the expanded operation load for the checked full fixture:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.inspect_fixture_load \
  --case stim_surface_d11_r100 \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  --format json \
  --out /tmp/stim-surface-load.json
```

The expected summary is `PASS fixture load stim_surface_d11_r100`. The JSON
report records `expanded_operation_count = 14547`,
`operations.DEPOLARIZE2.target_count = 88000`,
`operations.DETECTOR.operation_count = 12000`,
`expected_measurements = 12121`, `expected_detectors = 12000`, and
`expected_observables = 1`.
````

- [ ] **Step 2: Run documentation-sensitive tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_inspect_fixture_load
```

Expected: PASS.

- [ ] **Step 3: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/README.md
git commit -m "docs: document fixture load inspector"
```

## Plan Self-Review

- Spec coverage: Task 1 covers the CLI, JSON/text report, required counts, repeat depth, repeat expansion, PASS-style summary, missing-case rejection, and issue verification. Task 2 documents the reviewer-facing command.
- Marker scan: No banned marker strings are present.
- Type consistency: Function names in Task 1 match the Interfaces block and the test imports.
