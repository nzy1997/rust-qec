# Issue 321 QEC-Code Random-Window Manifests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add validated smoke and full benchmark case manifests for `qec-code code css-distance random-window-upper-bound`.

**Architecture:** Create a new Python package under `benchmarks/qec_code_random_window/` that owns static TOML case manifests and a standard-library validator. The validator checks the manifest contract without invoking Cargo, while the existing qec-code CLI remains unchanged.

**Tech Stack:** Python 3.11+ standard library (`argparse`, `pathlib`, `subprocess`, `sys`, `tomllib`, `unittest`), TOML manifests, Cargo workspace verification.

## Global Constraints

- Create exactly `benchmarks/qec_code_random_window/cases.smoke.toml` and `benchmarks/qec_code_random_window/cases.full.toml`.
- Future runner command must work: `python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/cases.smoke.toml`.
- Valid manifests exit 0 and print exactly `PASS`.
- Duplicate `case_id` values must be rejected with a nonzero exit and the duplicated ID in stderr.
- `baseline_required = true` must be rejected when `baseline_key` is empty, `none`, or starts with `unmapped:`.
- Use `baseline_required = false` for cases with no defensible paper row.
- Use `baseline_required = true` only for `bb72` and `bb144` bivariate-bicycle cases.
- Include at least `steane`, one small rotated surface code, one small toric code, `bb72`, and one larger BB/APM case already available through qec-code. The larger case is `bb144` through the existing parameterized bivariate-bicycle code-id interface.
- Do not run or reimplement external paper algorithms.
- Do not claim exact distances from randomized upper-bound results.
- Required verification includes `cargo run -q -p qec-code -- code css-distance random-window-upper-bound --help` and `cargo test`.

---

### Task 1: Add Failing Validator Tests And Invalid Fixtures

**Files:**
- Create: `benchmarks/qec_code_random_window/__init__.py`
- Create: `benchmarks/qec_code_random_window/tests/__init__.py`
- Create: `benchmarks/qec_code_random_window/tests/test_validate_cases.py`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/duplicate_case_id.toml`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/strict_baseline_missing_key.toml`

**Interfaces:**
- Consumes: future module `benchmarks.qec_code_random_window.validate_cases`.
- Produces: unittest coverage for CLI success, duplicate-case rejection, strict-baseline rejection, and pinned smoke/full contents.

- [ ] **Step 1: Create package markers**

Create `benchmarks/qec_code_random_window/__init__.py`:

```python
"""QEC-code random-window benchmark case manifests."""
```

Create `benchmarks/qec_code_random_window/tests/__init__.py`:

```python
"""Tests for qec-code random-window benchmark manifests."""
```

- [ ] **Step 2: Create invalid duplicate-ID fixture**

Create `benchmarks/qec_code_random_window/tests/fixtures/duplicate_case_id.toml`:

```toml
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "duplicate_case"
code_id = "steane"
distance_side = "any"
iterations = 10
restarts = 1
seed = 7
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:steane"
baseline_required = false

[[cases]]
case_id = "duplicate_case"
code_id = "surface_rotated:d=3"
distance_side = "any"
iterations = 10
restarts = 1
seed = 7
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:surface_rotated_d3"
baseline_required = false
```

- [ ] **Step 3: Create invalid strict-baseline fixture**

Create `benchmarks/qec_code_random_window/tests/fixtures/strict_baseline_missing_key.toml`:

```toml
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "strict_missing_baseline"
code_id = "bb72"
distance_side = "any"
iterations = 10
restarts = 1
seed = 7
target_weight = 6
target_upper_bound = 6
baseline_key = ""
baseline_required = true
```

- [ ] **Step 4: Write failing tests**

Create `benchmarks/qec_code_random_window/tests/test_validate_cases.py`:

```python
from __future__ import annotations

import subprocess
import sys
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "qec_code_random_window"
SMOKE_MANIFEST = PACKAGE_DIR / "cases.smoke.toml"
FULL_MANIFEST = PACKAGE_DIR / "cases.full.toml"
FIXTURES = PACKAGE_DIR / "tests" / "fixtures"
BB144_CODE_ID = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.qec_code_random_window.validate_cases",
            str(path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def load_manifest(path: Path) -> dict[str, object]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def cases_by_id(path: Path) -> dict[str, dict[str, object]]:
    manifest = load_manifest(path)
    cases = manifest["cases"]
    assert isinstance(cases, list)
    return {case["case_id"]: case for case in cases}


class ValidateCasesTest(unittest.TestCase):
    def test_smoke_manifest_cli_prints_pass(self) -> None:
        result = run_validator(SMOKE_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS\n")
        self.assertEqual(result.stderr, "")

    def test_full_manifest_cli_prints_pass(self) -> None:
        result = run_validator(FULL_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS\n")
        self.assertEqual(result.stderr, "")

    def test_duplicate_case_id_fixture_is_rejected_and_names_id(self) -> None:
        result = run_validator(FIXTURES / "duplicate_case_id.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate case_id", result.stderr)
        self.assertIn("duplicate_case", result.stderr)

    def test_strict_baseline_fixture_requires_usable_key(self) -> None:
        result = run_validator(FIXTURES / "strict_baseline_missing_key.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("strict_missing_baseline", result.stderr)
        self.assertIn("baseline_required = true", result.stderr)
        self.assertIn("baseline_key", result.stderr)

    def test_smoke_manifest_pins_required_cases_and_baseline_contract(self) -> None:
        cases = cases_by_id(SMOKE_MANIFEST)

        self.assertEqual(
            tuple(cases),
            (
                "steane_smoke",
                "surface_rotated_d3_smoke",
                "toric_d3_smoke",
                "bb72_smoke",
            ),
        )
        self.assertEqual(cases["steane_smoke"]["code_id"], "steane")
        self.assertEqual(cases["surface_rotated_d3_smoke"]["code_id"], "surface_rotated:d=3")
        self.assertEqual(cases["toric_d3_smoke"]["code_id"], "toric:d=3")
        self.assertEqual(cases["bb72_smoke"]["code_id"], "bb72")
        self.assertFalse(cases["steane_smoke"]["baseline_required"])
        self.assertFalse(cases["surface_rotated_d3_smoke"]["baseline_required"])
        self.assertFalse(cases["toric_d3_smoke"]["baseline_required"])
        self.assertTrue(cases["bb72_smoke"]["baseline_required"])
        self.assertEqual(
            cases["bb72_smoke"]["baseline_key"],
            "codeDistancePYPI:bivariate_bicycle:bb72",
        )

    def test_full_manifest_includes_larger_bb_case(self) -> None:
        cases = cases_by_id(FULL_MANIFEST)

        self.assertIn("bb144_full", cases)
        self.assertEqual(cases["bb144_full"]["code_id"], BB144_CODE_ID)
        self.assertEqual(cases["bb144_full"]["target_upper_bound"], 12)
        self.assertTrue(cases["bb144_full"]["baseline_required"])
        self.assertEqual(
            cases["bb144_full"]["baseline_key"],
            "codeDistancePYPI:bivariate_bicycle:bb144",
        )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 5: Run RED verification**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases -q
```

Expected: FAIL because `validate_cases.py`, `cases.smoke.toml`, and
`cases.full.toml` do not exist yet.

---

### Task 2: Implement Validator And Case Manifests

**Files:**
- Create: `benchmarks/qec_code_random_window/validate_cases.py`
- Create: `benchmarks/qec_code_random_window/cases.smoke.toml`
- Create: `benchmarks/qec_code_random_window/cases.full.toml`

**Interfaces:**
- Consumes: TOML files containing `manifest_version`, `suite`, and `[[cases]]`.
- Produces: `validate_manifest(manifest: dict[str, object]) -> list[str]`.
- Produces: CLI `python3 -m benchmarks.qec_code_random_window.validate_cases <manifest>` that prints `PASS` on success and errors on stderr on failure.

- [ ] **Step 1: Create validator module**

Create `benchmarks/qec_code_random_window/validate_cases.py`:

```python
from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path
from typing import Any


SUITE = "qec_code_random_window"
MANIFEST_VERSION = 1
DISTANCE_SIDES = {"any", "x", "z"}
REQUIRED_CASE_FIELDS = {
    "case_id",
    "code_id",
    "distance_side",
    "iterations",
    "restarts",
    "seed",
    "target_weight",
    "baseline_key",
    "baseline_required",
}


def _is_int(value: object) -> bool:
    return type(value) is int


def _usable_baseline_key(value: object) -> bool:
    if not isinstance(value, str):
        return False
    normalized = value.strip().lower()
    return normalized not in {"", "none", "null", "n/a"} and not normalized.startswith(
        "unmapped:"
    )


def _require_str(case: dict[str, Any], field: str, case_label: str, errors: list[str]) -> str | None:
    value = case.get(field)
    if not isinstance(value, str) or not value.strip():
        errors.append(f'{case_label} field "{field}" must be a non-empty string')
        return None
    return value


def _require_positive_int(
    case: dict[str, Any],
    field: str,
    case_label: str,
    errors: list[str],
) -> int | None:
    value = case.get(field)
    if not _is_int(value) or value <= 0:
        errors.append(f'{case_label} field "{field}" must be a positive integer')
        return None
    return value


def _require_nonnegative_int(
    case: dict[str, Any],
    field: str,
    case_label: str,
    errors: list[str],
) -> int | None:
    value = case.get(field)
    if not _is_int(value) or value < 0:
        errors.append(f'{case_label} field "{field}" must be a non-negative integer')
        return None
    return value


def validate_manifest(manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []

    if manifest.get("manifest_version") != MANIFEST_VERSION:
        errors.append('manifest_version must be 1')
    if manifest.get("suite") != SUITE:
        errors.append(f'suite must be "{SUITE}"')

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

        _require_str(raw_case, "code_id", case_label, errors)
        distance_side = _require_str(raw_case, "distance_side", case_label, errors)
        if distance_side is not None and distance_side not in DISTANCE_SIDES:
            errors.append(
                f'{case_label} field "distance_side" must be one of: any, x, z'
            )

        _require_positive_int(raw_case, "iterations", case_label, errors)
        _require_positive_int(raw_case, "restarts", case_label, errors)
        _require_nonnegative_int(raw_case, "seed", case_label, errors)
        target_weight = _require_positive_int(raw_case, "target_weight", case_label, errors)

        target_upper_bound = raw_case.get("target_upper_bound")
        if target_upper_bound is not None:
            if not _is_int(target_upper_bound) or target_upper_bound <= 0:
                errors.append(f'{case_label} field "target_upper_bound" must be a positive integer')
            elif target_weight is not None and target_weight > target_upper_bound:
                errors.append(
                    f'{case_label} target_weight must be <= target_upper_bound'
                )

        baseline_key = raw_case.get("baseline_key")
        if not isinstance(baseline_key, str):
            errors.append(f'{case_label} field "baseline_key" must be a string')

        baseline_required = raw_case.get("baseline_required")
        if type(baseline_required) is not bool:
            errors.append(f'{case_label} field "baseline_required" must be a boolean')
        elif baseline_required and not _usable_baseline_key(baseline_key):
            errors.append(
                f'{case_label} has baseline_required = true but no usable baseline_key'
            )

    return errors


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be a TOML table")
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate qec-code random-window case manifests.")
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

    print("PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Create smoke manifest**

Create `benchmarks/qec_code_random_window/cases.smoke.toml`:

```toml
manifest_version = 1
suite = "qec_code_random_window"
description = "Fast smoke cases for qec-code random-window upper-bound evidence."

[[cases]]
case_id = "steane_smoke"
code_id = "steane"
distance_side = "any"
iterations = 500
restarts = 4
seed = 7
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:steane"
baseline_required = false

[[cases]]
case_id = "surface_rotated_d3_smoke"
code_id = "surface_rotated:d=3"
distance_side = "any"
iterations = 500
restarts = 4
seed = 7
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:surface_rotated_d3"
baseline_required = false

[[cases]]
case_id = "toric_d3_smoke"
code_id = "toric:d=3"
distance_side = "any"
iterations = 500
restarts = 4
seed = 7
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:toric_d3"
baseline_required = false

[[cases]]
case_id = "bb72_smoke"
code_id = "bb72"
distance_side = "any"
iterations = 5000
restarts = 8
seed = 7
target_weight = 6
target_upper_bound = 6
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
baseline_required = true
```

- [ ] **Step 3: Create full manifest**

Create `benchmarks/qec_code_random_window/cases.full.toml`:

```toml
manifest_version = 1
suite = "qec_code_random_window"
description = "Representative full cases for qec-code random-window upper-bound evidence."

[[cases]]
case_id = "steane_full"
code_id = "steane"
distance_side = "any"
iterations = 5000
restarts = 8
seed = 7
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:steane"
baseline_required = false

[[cases]]
case_id = "surface_rotated_d5_full"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 5000
restarts = 8
seed = 7
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:surface_rotated_d5"
baseline_required = false

[[cases]]
case_id = "toric_d5_full"
code_id = "toric:d=5"
distance_side = "any"
iterations = 5000
restarts = 8
seed = 7
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:toric_d5"
baseline_required = false

[[cases]]
case_id = "bb72_full"
code_id = "bb72"
distance_side = "any"
iterations = 5000
restarts = 8
seed = 7
target_weight = 6
target_upper_bound = 6
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
baseline_required = true

[[cases]]
case_id = "bb144_full"
code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
distance_side = "any"
iterations = 5000
restarts = 8
seed = 7
target_weight = 12
target_upper_bound = 12
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
baseline_required = true
```

- [ ] **Step 4: Run focused GREEN verification**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases -q
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/cases.smoke.toml
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/cases.full.toml
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/tests/fixtures/duplicate_case_id.toml
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/tests/fixtures/strict_baseline_missing_key.toml
```

Expected: unittest passes. The smoke and full manifest commands exit 0 and
print `PASS`. The duplicate fixture command exits nonzero and names
`duplicate_case`. The strict-baseline fixture command exits nonzero and names
`strict_missing_baseline`.

- [ ] **Step 5: Commit**

Run:

```bash
git add benchmarks/qec_code_random_window docs/superpowers/plans/2026-06-29-issue-321-qec-code-random-window-manifests.md
git commit -m "benchmarks: add qec random-window manifests"
```

---

### Task 3: Final Verification

**Files:**
- Read: `benchmarks/qec_code_random_window/cases.smoke.toml`
- Read: `benchmarks/qec_code_random_window/cases.full.toml`
- Read: `benchmarks/qec_code_random_window/validate_cases.py`

**Interfaces:**
- Consumes: committed manifests and validator.
- Produces: verification evidence for the pull request.

- [ ] **Step 1: Run required manifest validation**

Run:

```bash
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/cases.smoke.toml
```

Expected: exit 0 and stdout exactly `PASS`.

- [ ] **Step 2: Run invalid fixture checks**

Run:

```bash
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/tests/fixtures/duplicate_case_id.toml
python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/tests/fixtures/strict_baseline_missing_key.toml
```

Expected: both commands exit nonzero. The first stderr names
`duplicate_case`; the second stderr names `strict_missing_baseline` and
`baseline_key`.

- [ ] **Step 3: Run focused unittest coverage**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases -q
```

Expected: all tests pass.

- [ ] **Step 4: Run qec-code help check**

Run:

```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --help
```

Expected: exit 0 and help output for `random-window-upper-bound`. If the
sandboxed command tries to fetch crates and fails because network is blocked,
rerun with `CARGO_NET_OFFLINE=true` and report both outcomes.

- [ ] **Step 5: Run required Cargo test**

Run:

```bash
cargo test
```

Expected: tests pass. If the sandboxed command tries to fetch crates and fails
because network is blocked, rerun with `CARGO_NET_OFFLINE=true` and report both
outcomes.

- [ ] **Step 6: Inspect final diff**

Run:

```bash
git diff --stat origin/master..HEAD
git status --short
```

Expected: branch contains only the committed design, plan, manifests,
validator, fixtures, and tests; working tree is clean before PR creation.
