# rstim-vs-Stim Correctness Verifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness`, a statistical sample-correctness verifier that compares Stim and `rstim` on the checked fixture catalog and writes JSON evidence.

**Architecture:** Add one focused Python module beside `validate_cases.py`. The module loads and validates manifests, runs Stim and `rstim` CLIs, parses dense `01` samples, computes selected marginal and pair statistics, classifies each case, writes JSON, and prints a compact verdict.

**Tech Stack:** Python 3.11 stdlib (`argparse`, `json`, `math`, `random`, `subprocess`, `tempfile`, `time`, `tomllib`, `unittest.mock`), existing `benchmarks.rstim_vs_stim_simulator.validate_cases`, Stim CLI, `rstim` CLI.

## Global Constraints

- Consume canonical Stim-generated `.stim` fixtures from #385.
- Inputs are fixture manifest, Stim CLI path, `rstim` binary path, shot count, and seed list.
- Output is JSON summary plus PASS/WARN/FAIL text report with per-case rates, tolerances, sample counts, tool status, and failure reasons.
- Use `stim sample` / `rstim sample` for measurement-output cases and `stim detect` / `rstim detect` for detector-output cases.
- Include status fields such as `pass`, `statistical_mismatch`, `stim_failed`, and `rstim_failed`.
- Do not hide slow or failed runs; record failure status and command stderr in the summary.
- Do not optimize `rstim` or require all correctness cases to pass before writing a report.

---

## File Structure

- Create `benchmarks/rstim_vs_stim_simulator/verify_correctness.py`: CLI, manifest loading, command execution, sample parsing, statistics, evidence JSON, text verdicts.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`: unit tests and mocked CLI integration tests.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document verifier commands and expected smoke/negative-control outcomes.

---

### Task 1: Sample Parsing And Statistical Helpers

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/verify_correctness.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`

**Interfaces:**
- Produces: `parse_01_samples(stdout: str, *, expected_bits: int, expected_shots: int) -> list[list[int]]`
- Produces: `inject_bitflip(samples: list[list[int]], *, rate: float, seed: int) -> list[list[int]]`
- Produces: `select_columns(bit_count: int, *, observable_count: int, limit: int = 16) -> list[int]`
- Produces: `select_pairs(columns: list[int], *, bit_count: int, observable_count: int, limit: int = 16) -> list[tuple[int, int]]`
- Produces: `compare_sample_sets(stim_samples: list[list[int]], rstim_samples: list[list[int]], *, columns: list[int], pairs: list[tuple[int, int]], z_score: float = 6.0, floor: float = 0.01) -> dict[str, object]`

- [ ] **Step 1: Write failing helper tests**

Add these tests to `benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`:

```python
from __future__ import annotations

import unittest

from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    compare_sample_sets,
    inject_bitflip,
    parse_01_samples,
    select_columns,
    select_pairs,
)


class VerifyCorrectnessHelpersTest(unittest.TestCase):
    def test_parse_01_samples_requires_rectangular_output(self) -> None:
        self.assertEqual(
            parse_01_samples("01\n10\n", expected_bits=2, expected_shots=2),
            [[0, 1], [1, 0]],
        )
        with self.assertRaisesRegex(ValueError, "expected 2 bits"):
            parse_01_samples("0\n11\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "expected 2 shots"):
            parse_01_samples("01\n", expected_bits=2, expected_shots=2)

    def test_selectors_include_observable_tail_and_pairs(self) -> None:
        columns = select_columns(25, observable_count=2, limit=10)
        self.assertIn(0, columns)
        self.assertIn(23, columns)
        self.assertIn(24, columns)
        pairs = select_pairs(columns, bit_count=25, observable_count=2, limit=10)
        self.assertTrue(any(pair[1] >= 23 for pair in pairs))

    def test_compare_sample_sets_accepts_close_rates(self) -> None:
        stim = [[0, 1], [1, 1], [0, 0], [1, 0]]
        rstim = [[0, 1], [1, 1], [0, 0], [1, 0]]
        result = compare_sample_sets(stim, rstim, columns=[0, 1], pairs=[(0, 1)])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 4)

    def test_compare_sample_sets_flags_large_mismatch(self) -> None:
        stim = [[0], [0], [0], [0], [0], [0], [0], [0]]
        rstim = [[1], [1], [1], [1], [1], [1], [1], [1]]
        result = compare_sample_sets(stim, rstim, columns=[0], pairs=[])
        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertGreater(result["max_delta"], result["max_tolerance"])

    def test_inject_bitflip_is_deterministic_and_changes_bits(self) -> None:
        samples = [[0, 0], [1, 1]]
        self.assertEqual(
            inject_bitflip(samples, rate=1.0, seed=7),
            [[1, 1], [0, 0]],
        )
        self.assertEqual(samples, [[0, 0], [1, 1]])
```

- [ ] **Step 2: Run helper tests to verify they fail**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
```

Expected: FAIL or ERROR because `verify_correctness.py` and its helper functions do not exist yet.

- [ ] **Step 3: Implement helpers**

Create `verify_correctness.py` with constants and helper implementations:

```python
STATUS_PASS = "pass"
STATUS_MISMATCH = "statistical_mismatch"
STATUS_STIM_FAILED = "stim_failed"
STATUS_RSTIM_FAILED = "rstim_failed"
STATUS_SKIPPED = "skipped"


def parse_01_samples(stdout: str, *, expected_bits: int, expected_shots: int) -> list[list[int]]:
    lines = [line.strip() for line in stdout.splitlines() if line.strip()]
    if len(lines) != expected_shots:
        raise ValueError(f"expected {expected_shots} shots, got {len(lines)}")
    samples: list[list[int]] = []
    for shot_index, line in enumerate(lines):
        if len(line) != expected_bits:
            raise ValueError(
                f"shot {shot_index}: expected {expected_bits} bits, got {len(line)}"
            )
        if any(ch not in "01" for ch in line):
            raise ValueError(f"shot {shot_index}: output contains non-01 data")
        samples.append([1 if ch == "1" else 0 for ch in line])
    return samples
```

Implement `inject_bitflip` with `random.Random(seed)`, copy rows before editing, and reject rates outside `[0, 1]`.

Implement selectors with sorted unique indexes: first columns, observable tail, and evenly spaced middle columns; pairs are adjacent selected pairs plus first selected detector paired with each selected observable.

Implement `compare_sample_sets` by computing rates for each selected statistic and tolerance:

```python
pooled = (stim_hits + rstim_hits) / (stim_n + rstim_n)
tolerance = z_score * math.sqrt(pooled * (1 - pooled) * (1 / stim_n + 1 / rstim_n)) + floor
```

Return a JSON-ready dict containing `status`, `sample_count`, `marginals`, `pairs`, `max_delta`, `max_tolerance`, and `failure_reasons`.

- [ ] **Step 4: Run helper tests to verify they pass**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
```

Expected: PASS for all helper tests in this task.

- [ ] **Step 5: Commit helper layer**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/verify_correctness.py benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py
git commit -m "test: add correctness verifier statistics helpers"
```

---

### Task 2: Tool Runner And Case Evidence

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/verify_correctness.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`

**Interfaces:**
- Consumes: helper functions from Task 1.
- Produces: `resolve_case_input_path(raw_path: str, base_dir: Path) -> Path`
- Produces: `default_rstim_command() -> list[str]`
- Produces: `run_tool(command: list[str], *, input_path: Path) -> dict[str, object]`
- Produces: `verify_case(case: dict[str, object], *, base_dir: Path, stim_command: list[str], rstim_command: list[str], shots: int, seeds: list[int], inject_rstim_bitflip_rate: float) -> dict[str, object]`

- [ ] **Step 1: Write failing runner tests**

Add these tests:

```python
import subprocess
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    default_rstim_command,
    run_tool,
    verify_case,
)


class VerifyCorrectnessRunnerTest(unittest.TestCase):
    def test_default_rstim_command_uses_cargo_when_binary_is_absent(self) -> None:
        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.Path.exists", return_value=False):
            self.assertEqual(
                default_rstim_command(),
                ["cargo", "run", "--quiet", "-p", "rstim", "--bin", "rstim", "--"],
            )

    def test_run_tool_records_failure_stderr(self) -> None:
        completed = subprocess.CompletedProcess(["bad"], 2, "", "broken")
        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.subprocess.run", return_value=completed):
            result = run_tool(["bad"], input_path=Path("case.stim"))
        self.assertEqual(result["exit_code"], 2)
        self.assertEqual(result["stderr"], "broken")
        self.assertFalse(result["success"])

    def test_verify_case_records_stim_failure_before_statistics(self) -> None:
        case = {
            "case_id": "case_a",
            "tier": "smoke",
            "canonical_input_path": "fixtures/example.stim",
            "expected_measurements": 2,
            "expected_detectors": 0,
            "expected_observables": 0,
        }
        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.run_tool") as mocked:
            mocked.return_value = {
                "command": ["stim"],
                "exit_code": 1,
                "stdout": "",
                "stderr": "stim failed",
                "elapsed_s": 0.01,
                "success": False,
            }
            result = verify_case(
                case,
                base_dir=Path("benchmarks/rstim_vs_stim_simulator"),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )
        self.assertEqual(result["status"], "stim_failed")
        self.assertIn("stim failed", result["failure_reasons"][0])
```

- [ ] **Step 2: Run runner tests to verify they fail**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
```

Expected: FAIL because runner functions are missing.

- [ ] **Step 3: Implement runner and case evidence**

Implement command construction so Stim and `rstim` receive equivalent flags:

```python
def build_sample_command(tool_command: list[str], *, mode: str, shots: int, seed: int, input_path: Path) -> list[str]:
    command = [
        *tool_command,
        mode,
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        "01",
        "--in",
        str(input_path),
    ]
    if mode == "detect":
        command.insert(len(tool_command) + 1, "--append_observables")
    return command
```

Implement `run_tool` with `subprocess.run(..., capture_output=True, text=True, check=False)` and elapsed timing from `time.perf_counter()`. Store `command`, `exit_code`, `stdout`, `stderr`, `elapsed_s`, and `success`.

Implement `verify_case` to skip `tier == "documentation-only"`, infer mode from `expected_detectors > 0`, run both tools for every seed, parse outputs, optionally inject `rstim` bit flips with a deterministic seed derived from `case_id` and seed, merge seed samples, compare, and return per-case evidence. If parsing or command execution fails, return `stim_failed` or `rstim_failed` with stderr or parse error in `failure_reasons`.

- [ ] **Step 4: Run runner tests to verify they pass**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
```

Expected: PASS for helper and runner tests.

- [ ] **Step 5: Commit runner layer**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/verify_correctness.py benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py
git commit -m "feat: run Stim and rstim correctness samples"
```

---

### Task 3: CLI, JSON Summary, And Text Verdicts

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/verify_correctness.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`

**Interfaces:**
- Consumes: `verify_case` from Task 2.
- Produces: `build_summary(args: argparse.Namespace) -> dict[str, object]`
- Produces: `write_summary(path: Path, summary: dict[str, object]) -> None`
- Produces: `format_report(summary: dict[str, object]) -> tuple[int, str]`
- Produces: `main(argv: list[str] | None = None) -> int`

- [ ] **Step 1: Write failing CLI tests**

Add tests that mock `verify_case` and assert JSON/report behavior:

```python
import json
import tempfile

from benchmarks.rstim_vs_stim_simulator.verify_correctness import main


class VerifyCorrectnessCliTest(unittest.TestCase):
    def test_main_writes_json_and_prints_pass(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.verify_case") as mocked:
                mocked.side_effect = [
                    {"case_id": "case_a", "tier": "smoke", "status": "pass", "sample_count": 4, "max_delta": 0.0, "max_tolerance": 0.01, "failure_reasons": [], "selected_columns": [0], "selected_pairs": []},
                    {"case_id": "doc_case", "tier": "documentation-only", "status": "skipped", "sample_count": 0, "failure_reasons": ["documentation-only"]},
                ]
                code = main([
                    "--cases",
                    "benchmarks/rstim_vs_stim_simulator/cases.smoke.toml",
                    "--shots",
                    "4",
                    "--out",
                    str(out),
                ])
            self.assertEqual(code, 0)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "pass")

    def test_main_returns_nonzero_for_statistical_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.verify_case") as mocked:
                mocked.return_value = {"case_id": "case_a", "tier": "smoke", "status": "statistical_mismatch", "sample_count": 4, "max_delta": 0.2, "max_tolerance": 0.01, "failure_reasons": ["marginal c0 delta 0.2 > tolerance 0.01"], "selected_columns": [0], "selected_pairs": []}
                code = main([
                    "--cases",
                    "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                    "--shots",
                    "4",
                    "--out",
                    str(out),
                ])
            self.assertEqual(code, 1)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "statistical_mismatch")
```

- [ ] **Step 2: Run CLI tests to verify they fail**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
```

Expected: FAIL because CLI functions are missing or incomplete.

- [ ] **Step 3: Implement CLI and reporting**

Implement argument parsing:

```python
parser.add_argument("--cases", type=Path, required=True)
parser.add_argument("--stim", default="stim")
parser.add_argument("--rstim", default=None)
parser.add_argument("--shots", type=_positive_int, required=True)
parser.add_argument("--seeds", default="12345")
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--inject-rstim-bitflip-rate", type=float, default=0.0)
```

Load the manifest, run `validate_manifest`, build command prefixes with `shlex.split`, call `verify_case` for each case, and write sorted, indented JSON ending in a newline.

Implement verdict precedence:

- any `stim_failed` or `rstim_failed`: print first line `FAIL tool failure`, exit 1;
- any `statistical_mismatch`: print first line `FAIL statistical mismatch`, exit 1;
- otherwise print first line `PASS correctness smoke`, exit 0.

Per-case report lines include `case_id`, `status`, `samples`, selected statistic counts, `max_delta`, `tolerance`, and first failure reason when present.

- [ ] **Step 4: Run CLI tests to verify they pass**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
```

Expected: PASS for all verifier tests.

- [ ] **Step 5: Commit CLI layer**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/verify_correctness.py benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py
git commit -m "feat: add correctness verifier CLI"
```

---

### Task 4: Smoke Verification, Negative Control, And Docs

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py`

**Interfaces:**
- Consumes: full CLI from Task 3.
- Produces: documented user-facing smoke commands.

- [ ] **Step 1: Add documentation and any final regression assertions**

Append a README section:

````markdown
## Correctness Verification

Run the smoke correctness verifier:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \
  --shots 20000 \
  --out /tmp/rstim-vs-stim-correctness.json
```

The expected smoke verdict is `PASS correctness smoke`. The JSON report keeps
per-case selected marginal and pair rates, tolerances, sample counts, tool
status, stderr, and failure reasons.

Run the verifier negative control:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \
  --shots 20000 \
  --inject-rstim-bitflip-rate 0.20 \
  --out /tmp/rstim-vs-stim-correctness-bad.json
```

The expected negative-control verdict is `FAIL statistical mismatch`.
````

- [ ] **Step 2: Run package unit tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_correctness -v
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_cases -v
```

Expected: both commands pass.

- [ ] **Step 3: Run issue smoke command**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \
  --shots 20000 \
  --out /tmp/rstim-vs-stim-correctness.json
```

Expected: exits 0, prints `PASS correctness smoke`, and writes per-case rates to `/tmp/rstim-vs-stim-correctness.json`.

- [ ] **Step 4: Run issue negative control**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \
  --shots 20000 \
  --inject-rstim-bitflip-rate 0.20 \
  --out /tmp/rstim-vs-stim-correctness-bad.json
```

Expected: exits nonzero and prints `FAIL statistical mismatch`.

- [ ] **Step 5: Run repository verification**

Run:

```sh
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 6: Commit final verifier docs and fixes**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/README.md benchmarks/rstim_vs_stim_simulator/verify_correctness.py benchmarks/rstim_vs_stim_simulator/tests/test_verify_correctness.py
git commit -m "docs: document rstim-vs-Stim correctness verifier"
```

---

## Self-Review

- Spec coverage: Tasks 1-3 implement manifest-consuming verifier logic, statistical comparisons, tool status recording, JSON summary, PASS/FAIL text report, and negative-control injection; Task 4 covers explicit issue verification and docs.
- Marker scan: no unresolved marker text remains.
- Type consistency: helper and CLI function names are introduced before use and use JSON-ready Python dictionaries across task boundaries.
