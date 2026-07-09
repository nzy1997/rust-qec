# Small-Circuit Distribution Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a source-grounded small-circuit distribution catalog and validator for issue #429.

**Architecture:** Add a focused TOML catalog and a separate Python validator beside the existing `rstim_vs_stim_simulator` fixture validator. The validator performs schema, pinned provenance, and probability-sum checks only; it does not invoke Stim or `rstim`.

**Tech Stack:** Python 3.11 stdlib (`argparse`, `math`, `re`, `subprocess`, `sys`, `tomllib`, `unittest`, `pathlib`), existing benchmark package layout, TOML manifests.

## Global Constraints

- Catalog path is `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml`.
- Validator module is `benchmarks.rstim_vs_stim_simulator.validate_distribution_cases`.
- Successful CLI output is exactly `PASS 8 distribution cases`.
- Include exactly the eight source-grounded cases listed in issue #429.
- Pin every case to Stim commit `9e225958f9ae1f9c33d1b9a012b7ec4392b43aef`.
- Record exact source line ranges from `src/stim/cmd/command_sample.test.cc`.
- Required fields per case: `case_id`, `source_url`, `source_commit`, `source_line_start`, `source_line_end`, `circuit`, `shots`, `expected_distribution`; include `tolerance` and optional `source_expression`.
- Expected probabilities must be numeric values that sum to 1.0 within documented tolerance, not only prose formulas.
- Negative control fixture path is `benchmarks/rstim_vs_stim_simulator/tests/fixtures/bad_distribution_sum.toml`.
- The validator must reject missing pinned `source_commit` and missing source line metadata.
- Do not run Stim or `rstim` in this issue.
- Do not publish benchmark evidence or generated output artifacts.

---

## File Structure

- Create `benchmarks/rstim_vs_stim_simulator/validate_distribution_cases.py`: CLI, TOML loading, validation helpers, manifest validation, success/failure reporting.
- Create `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml`: eight source-grounded cases from the pinned Stim source.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py`: validator unit and CLI tests.
- Create `benchmarks/rstim_vs_stim_simulator/tests/fixtures/bad_distribution_sum.toml`: invalid negative-control distribution.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document the new catalog validator command and scope.

---

### Task 1: Validator And Negative-Control Fixture

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/validate_distribution_cases.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/fixtures/bad_distribution_sum.toml`

**Interfaces:**
- Produces: `validate_manifest(manifest: dict[str, object]) -> list[str]`
- Produces: `load_manifest(path: Path) -> dict[str, object]`
- Produces: `main(argv: list[str] | None = None) -> int`
- Consumed later by Task 2's catalog tests and by the public CLI.

- [ ] **Step 1: Write failing validator tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py` with focused tests for the validator API and CLI. Use a helper that creates a minimal valid manifest in memory:

```python
from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator.validate_distribution_cases import validate_manifest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
FIXTURES = PACKAGE_DIR / "tests" / "fixtures"
PINNED_COMMIT = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
SOURCE_URL = (
    "https://github.com/quantumlib/Stim/blob/"
    f"{PINNED_COMMIT}/src/stim/cmd/command_sample.test.cc"
)


def minimal_manifest() -> dict[str, object]:
    return {
        "manifest_version": 1,
        "suite": "rstim_vs_stim_simulator",
        "description": "test distribution cases",
        "distribution_tolerance": 1e-9,
        "cases": [
            {
                "case_id": "unit_bell",
                "source_url": SOURCE_URL,
                "source_commit": PINNED_COMMIT,
                "source_line_start": 160,
                "source_line_end": 169,
                "circuit": "H 0\nCNOT 0 1\nM 0 1\n",
                "shots": 10000,
                "tolerance": 1e-9,
                "expected_distribution": {"00": 0.5, "11": 0.5},
            }
        ],
    }


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.validate_distribution_cases",
            str(path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


class ValidateDistributionCasesTest(unittest.TestCase):
    def test_validate_manifest_accepts_minimal_source_grounded_case(self) -> None:
        self.assertEqual(validate_manifest(minimal_manifest()), [])

    def test_validate_manifest_rejects_missing_source_commit(self) -> None:
        manifest = minimal_manifest()
        case = manifest["cases"][0]
        assert isinstance(case, dict)
        del case["source_commit"]

        errors = validate_manifest(manifest)

        self.assertTrue(any("source_commit" in error for error in errors), errors)

    def test_validate_manifest_rejects_missing_source_line_metadata(self) -> None:
        manifest = minimal_manifest()
        case = manifest["cases"][0]
        assert isinstance(case, dict)
        del case["source_line_start"]

        errors = validate_manifest(manifest)

        self.assertTrue(any("source_line_start" in error for error in errors), errors)

    def test_bad_distribution_sum_negative_control_cli_fails(self) -> None:
        result = run_validator(FIXTURES / "bad_distribution_sum.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("expected distribution probabilities must sum to 1", result.stderr)

    def test_cli_accepts_single_valid_distribution_case(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "one_case.toml"
            path.write_text(
                f'''\
manifest_version = 1
suite = "rstim_vs_stim_simulator"
description = "one valid case"
distribution_tolerance = 1e-9

[[cases]]
case_id = "unit_bell"
source_url = "{SOURCE_URL}"
source_commit = "{PINNED_COMMIT}"
source_line_start = 160
source_line_end = 169
circuit = """
H 0
CNOT 0 1
M 0 1
"""
shots = 10000
tolerance = 1e-9
expected_distribution = {{ "00" = 0.5, "11" = 0.5 }}
''',
                encoding="utf-8",
            )

            result = run_validator(path)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 1 distribution cases\n")
        self.assertEqual(result.stderr, "")
```

- [ ] **Step 2: Add the negative-control fixture**

Create `benchmarks/rstim_vs_stim_simulator/tests/fixtures/bad_distribution_sum.toml`:

```toml
manifest_version = 1
suite = "rstim_vs_stim_simulator"
description = "Negative-control distribution manifest whose probabilities do not sum to 1."
distribution_tolerance = 1e-9

[[cases]]
case_id = "bad_distribution_sum"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 160
source_line_end = 169
circuit = """
H 0
CNOT 0 1
M 0 1
"""
shots = 10000
tolerance = 1e-9
expected_distribution = { "00" = 0.5, "11" = 0.4 }
```

- [ ] **Step 3: Run tests to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_distribution_cases -q
```

Expected: fail or error because `validate_distribution_cases.py` does not exist yet.

- [ ] **Step 4: Implement the validator**

Create `benchmarks/rstim_vs_stim_simulator/validate_distribution_cases.py` with:

```python
from __future__ import annotations

import argparse
import math
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


SUITE = "rstim_vs_stim_simulator"
MANIFEST_VERSION = 1
PINNED_SOURCE_COMMIT = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
SOURCE_URL = (
    "https://github.com/quantumlib/Stim/blob/"
    f"{PINNED_SOURCE_COMMIT}/src/stim/cmd/command_sample.test.cc"
)
DEFAULT_DISTRIBUTION_TOLERANCE = 1e-9
REQUIRED_CASE_FIELDS = {
    "case_id",
    "source_url",
    "source_commit",
    "source_line_start",
    "source_line_end",
    "circuit",
    "shots",
    "expected_distribution",
}
BITSTRING_RE = re.compile(r"^[01]+$")


def _is_int(value: object) -> bool:
    return type(value) is int


def _require_str(case: dict[str, Any], field: str, case_label: str, errors: list[str]) -> str | None:
    value = case.get(field)
    if not isinstance(value, str) or not value.strip():
        errors.append(f'{case_label} field "{field}" must be a non-empty string')
        return None
    return value
```

Implement helper functions matching the existing `validate_cases.py` style:

```python
def _require_positive_int(case: dict[str, Any], field: str, case_label: str, errors: list[str]) -> int | None:
    value = case.get(field)
    if not _is_int(value) or value <= 0:
        errors.append(f'{case_label} field "{field}" must be a positive integer')
        return None
    return int(value)


def _require_tolerance(value: object, label: str, errors: list[str]) -> float | None:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        errors.append(f"{label} must be a positive numeric tolerance")
        return None
    tolerance = float(value)
    if not math.isfinite(tolerance) or tolerance <= 0:
        errors.append(f"{label} must be a positive numeric tolerance")
        return None
    return tolerance
```

Implement `validate_manifest`:

```python
def validate_manifest(manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if manifest.get("manifest_version") != MANIFEST_VERSION:
        errors.append("manifest_version must be 1")
    if manifest.get("suite") != SUITE:
        errors.append(f'suite must be "{SUITE}"')

    default_tolerance = DEFAULT_DISTRIBUTION_TOLERANCE
    if "distribution_tolerance" in manifest:
        parsed = _require_tolerance(
            manifest["distribution_tolerance"],
            "distribution_tolerance",
            errors,
        )
        if parsed is not None:
            default_tolerance = parsed

    cases = manifest.get("cases")
    if not isinstance(cases, list) or not cases:
        errors.append('manifest field "cases" must be a non-empty array')
        return errors

    seen: set[str] = set()
    for index, raw_case in enumerate(cases):
        case_label = f"case[{index}]"
        if not isinstance(raw_case, dict):
            errors.append(f"{case_label} must be a TOML table")
            continue

        missing = sorted(REQUIRED_CASE_FIELDS - set(raw_case))
        if missing:
            errors.append(f"{case_label} missing required field(s): {', '.join(missing)}")

        case_id = _require_str(raw_case, "case_id", case_label, errors)
        if case_id is not None:
            case_label = f'case "{case_id}"'
            if case_id in seen:
                errors.append(f'duplicate case_id "{case_id}"')
            seen.add(case_id)

        source_url = _require_str(raw_case, "source_url", case_label, errors)
        if source_url is not None and source_url != SOURCE_URL:
            errors.append(f'{case_label} field "source_url" must be the pinned Stim command_sample.test.cc URL')

        source_commit = _require_str(raw_case, "source_commit", case_label, errors)
        if source_commit is not None and source_commit != PINNED_SOURCE_COMMIT:
            errors.append(f'{case_label} field "source_commit" must be "{PINNED_SOURCE_COMMIT}"')

        line_start = _require_positive_int(raw_case, "source_line_start", case_label, errors)
        line_end = _require_positive_int(raw_case, "source_line_end", case_label, errors)
        if line_start is not None and line_end is not None and line_start > line_end:
            errors.append(f'{case_label} source_line_start must be <= source_line_end')

        _require_str(raw_case, "circuit", case_label, errors)
        _require_positive_int(raw_case, "shots", case_label, errors)

        case_tolerance = default_tolerance
        if "tolerance" in raw_case:
            parsed = _require_tolerance(raw_case["tolerance"], f'{case_label} field "tolerance"', errors)
            if parsed is not None:
                case_tolerance = parsed

        _validate_expected_distribution(raw_case, case_label, case_tolerance, errors)

    return errors
```

Implement `_validate_expected_distribution`:

```python
def _validate_expected_distribution(
    case: dict[str, Any],
    case_label: str,
    tolerance: float,
    errors: list[str],
) -> None:
    distribution = case.get("expected_distribution")
    if not isinstance(distribution, dict) or not distribution:
        errors.append(f'{case_label} field "expected_distribution" must be a non-empty table')
        return

    total = 0.0
    bit_width: int | None = None
    for outcome, raw_probability in distribution.items():
        if not isinstance(outcome, str) or BITSTRING_RE.fullmatch(outcome) is None:
            errors.append(f'{case_label} expected_distribution key "{outcome}" must be a non-empty 01 bitstring')
            continue
        if bit_width is None:
            bit_width = len(outcome)
        elif len(outcome) != bit_width:
            errors.append(f'{case_label} expected_distribution outcomes must all have the same bit width')

        if not isinstance(raw_probability, (int, float)) or isinstance(raw_probability, bool):
            errors.append(f'{case_label} expected_distribution["{outcome}"] must be a probability')
            continue
        probability = float(raw_probability)
        if not math.isfinite(probability) or not 0 <= probability <= 1:
            errors.append(f'{case_label} expected_distribution["{outcome}"] must be between 0 and 1')
            continue
        total += probability

    if not math.isclose(total, 1.0, rel_tol=0.0, abs_tol=tolerance):
        errors.append(
            f'{case_label} expected distribution probabilities must sum to 1 '
            f'within {tolerance:g}; got {total:.17g}'
        )
```

Add loader and CLI:

```python
def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be a TOML table")
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate rstim-vs-Stim distribution case manifests.")
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    errors = validate_manifest(manifest)
    if errors:
        for error in errors:
            print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    cases = manifest["cases"]
    print(f"PASS {len(cases)} distribution cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 5: Run tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_distribution_cases -q
```

Expected: all Task 1 tests pass.

- [ ] **Step 6: Commit validator layer**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/validate_distribution_cases.py benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py benchmarks/rstim_vs_stim_simulator/tests/fixtures/bad_distribution_sum.toml
git commit -m "test: add distribution catalog validator"
```

---

### Task 2: Source-Grounded Catalog And README Wiring

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`

**Interfaces:**
- Consumes: Task 1 `validate_distribution_cases` CLI and `validate_manifest`.
- Produces: the public catalog path and documented validation command.

- [ ] **Step 1: Add failing catalog tests**

Extend `ValidateDistributionCasesTest` in `benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py`:

```python
import tomllib


DISTRIBUTION_MANIFEST = PACKAGE_DIR / "distribution_cases.toml"


def load_manifest(path: Path) -> dict[str, object]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def distribution_cases_by_id() -> dict[str, dict[str, object]]:
    manifest = load_manifest(DISTRIBUTION_MANIFEST)
    cases = manifest["cases"]
    assert isinstance(cases, list)
    return {case["case_id"]: case for case in cases}
```

Add test methods:

```python
    def test_distribution_catalog_cli_prints_case_count(self) -> None:
        result = run_validator(DISTRIBUTION_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 8 distribution cases\n")
        self.assertEqual(result.stderr, "")

    def test_distribution_catalog_pins_expected_case_ids(self) -> None:
        cases = distribution_cases_by_id()

        self.assertEqual(
            tuple(cases),
            (
                "stim_bell_pair_basic_distribution",
                "stim_sqrt_x_transformed_pair",
                "stim_sqrt_y_transformed_pair",
                "stim_x_error_two_measured_qubits",
                "stim_z_error_h_conjugated_pair",
                "stim_y_error_two_measured_qubits",
                "stim_depolarize1_two_measured_qubits",
                "stim_depolarize2_two_measured_qubits",
            ),
        )

    def test_distribution_catalog_records_representative_probabilities(self) -> None:
        cases = distribution_cases_by_id()

        bell = cases["stim_bell_pair_basic_distribution"]["expected_distribution"]
        sqrt_x = cases["stim_sqrt_x_transformed_pair"]["expected_distribution"]
        depolarize1 = cases["stim_depolarize1_two_measured_qubits"]["expected_distribution"]
        depolarize2 = cases["stim_depolarize2_two_measured_qubits"]["expected_distribution"]
        self.assertEqual(bell, {"00": 0.5, "11": 0.5})
        self.assertEqual(sqrt_x, {"10": 0.5, "01": 0.5})
        self.assertEqual(depolarize1, {"00": 0.64, "01": 0.16, "10": 0.16, "11": 0.04})
        self.assertAlmostEqual(depolarize2["00"], 0.92)
        self.assertAlmostEqual(depolarize2["01"], 0.1 * 4 / 15)
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_distribution_cases -q
```

Expected: fail because `distribution_cases.toml` does not exist yet.

- [ ] **Step 3: Add the source-grounded catalog**

Create `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml` with these exact numeric probabilities and line ranges:

```toml
manifest_version = 1
suite = "rstim_vs_stim_simulator"
description = "Source-grounded small-circuit output distribution cases from Stim command_sample.test.cc."
distribution_tolerance = 1e-9

[[cases]]
case_id = "stim_bell_pair_basic_distribution"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 160
source_line_end = 169
circuit = """
H 0
CNOT 0 1
M 0 1
"""
shots = 10000
tolerance = 1e-9
source_expression = { "00" = "0.5", "11" = "0.5" }
expected_distribution = { "00" = 0.5, "11" = 0.5 }

[[cases]]
case_id = "stim_sqrt_x_transformed_pair"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 171
source_line_end = 180
circuit = """
H 0
CNOT 0 1
SQRT_X 0 1
M 0 1
"""
shots = 10000
tolerance = 1e-9
source_expression = { "10" = "0.5", "01" = "0.5" }
expected_distribution = { "10" = 0.5, "01" = 0.5 }

[[cases]]
case_id = "stim_sqrt_y_transformed_pair"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 182
source_line_end = 191
circuit = """
H 0
CNOT 0 1
SQRT_Y 0 1
M 0 1
"""
shots = 10000
tolerance = 1e-9
source_expression = { "00" = "0.5", "11" = "0.5" }
expected_distribution = { "00" = 0.5, "11" = 0.5 }

[[cases]]
case_id = "stim_x_error_two_measured_qubits"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 194
source_line_end = 202
circuit = """
X_ERROR(0.1) 0 1
M 0 1
"""
shots = 100000
tolerance = 1e-9
source_expression = { "00" = "0.9 * 0.9", "01" = "0.9 * 0.1", "10" = "0.9 * 0.1", "11" = "0.1 * 0.1" }
expected_distribution = { "00" = 0.81, "01" = 0.09, "10" = 0.09, "11" = 0.01 }

[[cases]]
case_id = "stim_z_error_h_conjugated_pair"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 216
source_line_end = 226
circuit = """
H 0 1
Z_ERROR(0.1) 0 1
H 0 1
M 0 1
"""
shots = 100000
tolerance = 1e-9
source_expression = { "00" = "0.9 * 0.9", "01" = "0.9 * 0.1", "10" = "0.9 * 0.1", "11" = "0.1 * 0.1" }
expected_distribution = { "00" = 0.81, "01" = 0.09, "10" = 0.09, "11" = 0.01 }

[[cases]]
case_id = "stim_y_error_two_measured_qubits"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 238
source_line_end = 246
circuit = """
Y_ERROR(0.1) 0 1
M 0 1
"""
shots = 100000
tolerance = 1e-9
source_expression = { "00" = "0.9 * 0.9", "01" = "0.9 * 0.1", "10" = "0.9 * 0.1", "11" = "0.1 * 0.1" }
expected_distribution = { "00" = 0.81, "01" = 0.09, "10" = 0.09, "11" = 0.01 }

[[cases]]
case_id = "stim_depolarize1_two_measured_qubits"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 260
source_line_end = 268
circuit = """
DEPOLARIZE1(0.3) 0 1
M 0 1
"""
shots = 100000
tolerance = 1e-9
source_expression = { "00" = "0.8 * 0.8", "01" = "0.8 * 0.2", "10" = "0.8 * 0.2", "11" = "0.2 * 0.2" }
expected_distribution = { "00" = 0.64, "01" = 0.16, "10" = 0.16, "11" = 0.04 }

[[cases]]
case_id = "stim_depolarize2_two_measured_qubits"
source_url = "https://github.com/quantumlib/Stim/blob/9e225958f9ae1f9c33d1b9a012b7ec4392b43aef/src/stim/cmd/command_sample.test.cc"
source_commit = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
source_line_start = 293
source_line_end = 301
circuit = """
DEPOLARIZE2(0.1) 0 1
M 0 1
"""
shots = 100000
tolerance = 1e-9
source_expression = { "00" = "0.1 * 3 / 15 + 0.9", "01" = "0.1 * 4 / 15", "10" = "0.1 * 4 / 15", "11" = "0.1 * 4 / 15" }
expected_distribution = { "00" = 0.92, "01" = 0.02666666666666667, "10" = 0.02666666666666667, "11" = 0.02666666666666667 }
```

- [ ] **Step 4: Document the validator in README**

Modify `benchmarks/rstim_vs_stim_simulator/README.md` after the existing `Validate` commands:

```markdown
Validate the source-grounded small-circuit distribution catalog:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_distribution_cases \
  benchmarks/rstim_vs_stim_simulator/distribution_cases.toml
```

The expected result is `PASS 8 distribution cases`. These cases are borrowed
from Stim's pinned `command_sample.test.cc` and record expected probabilities
only; this validator does not run Stim or `rstim`.
```

- [ ] **Step 5: Run catalog tests to verify GREEN**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_distribution_cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_distribution_cases -q
```

Expected: first command prints `PASS 8 distribution cases`; unit tests pass.

- [ ] **Step 6: Commit catalog layer**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/distribution_cases.toml benchmarks/rstim_vs_stim_simulator/tests/test_validate_distribution_cases.py benchmarks/rstim_vs_stim_simulator/README.md
git commit -m "feat: add source-grounded distribution catalog"
```

---

## Final Verification

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_distribution_cases \
  benchmarks/rstim_vs_stim_simulator/distribution_cases.toml
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_distribution_cases -q
cargo test
```

Expected:

- `PASS 8 distribution cases`
- unit tests pass
- `cargo test` passes

## Self-Review

- Plan covers every requirement from `docs/superpowers/specs/2026-07-09-issue-429-small-circuit-distribution-catalog-design.md`.
- No unresolved placeholder text remains.
- All new behavior has a RED/GREEN test path.
- Verification commands match the issue plus the required repository `cargo test` gate.
