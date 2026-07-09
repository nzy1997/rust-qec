# Issue 434 Repetition Release Speed Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish checked release-profile speed evidence for `rep-sample-d13-r13` and add a checker command that validates the evidence directory.

**Architecture:** Add a narrow Python checker in `tools/` that validates one requested release speed case directory without enforcing timing thresholds. Generate the evidence through the merged #433 suite runner into a separate `results/release-repetition-sample/` directory, then verify the checked artifacts with the new checker and focused unit tests.

**Tech Stack:** Python 3 standard library (`argparse`, `json`, `subprocess`, `tempfile`, `unittest`, `pathlib`), existing `benchmarks.rstim_vs_stim_simulator.run_speed_suite`, existing Rust `rstim` perf CLI, Cargo workspace tests.

## Global Constraints

- Results directory is exactly `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/`.
- Checked files are exactly `summary.json`, `report.md`, and `environment.json` in that directory.
- Checker command is `python3 tools/check_rstim_vs_stim_release_speed_case.py --results-dir benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample --case rep-sample-d13-r13 --workload sample --required-variants stim-cli,rstim-interpreted,rstim-compiled`.
- Successful checker output is exactly `PASS release speed case rep-sample-d13-r13`.
- The checker must validate that the case is present once, has workload `sample`, records all three required variants as completed, and records profile/environment metadata.
- Missing required variant failures must contain `missing required variant stim-cli`.
- Keep this evidence separate from `results/full/` and `results/release/`.
- Do not claim this single case proves broad speed superiority over Stim.
- Do not modify sampler internals.

---

## File Structure

- Create `tools/check_rstim_vs_stim_release_speed_case.py`: command-line checker for one release speed evidence directory.
- Create `tools/test_check_rstim_vs_stim_release_speed_case.py`: focused unittest coverage and negative controls for the checker.
- Create `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/summary.json`: checked generated summary.
- Create `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/report.md`: checked generated report.
- Create `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/environment.json`: checked generated environment metadata, marked as issue #434 evidence.

### Task 1: Checker Tests

**Files:**
- Create: `tools/test_check_rstim_vs_stim_release_speed_case.py`

**Interfaces:**
- Consumes: CLI path `tools/check_rstim_vs_stim_release_speed_case.py`.
- Produces: subprocess tests that verify success and required rejection messages.

- [ ] **Step 1: Write the failing checker tests**

Create `tools/test_check_rstim_vs_stim_release_speed_case.py` with:

```python
#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_release_speed_case.py"
CASE_LABEL = "rep-sample-d13-r13"
REQUIRED_VARIANTS = "stim-cli,rstim-interpreted,rstim-compiled"


def valid_summary() -> dict[str, object]:
    return {
        "cases": [
            {
                "case_label": CASE_LABEL,
                "workload": "sample",
                "tier": "gating",
                "present_variants": ["rstim-compiled", "rstim-interpreted", "stim-cli"],
                "variants": [
                    {"tool_variant": "rstim-compiled", "status": "completed"},
                    {"tool_variant": "rstim-interpreted", "status": "completed"},
                    {"tool_variant": "stim-cli", "status": "completed"},
                ],
            }
        ],
        "issues": [],
    }


def valid_environment() -> dict[str, object]:
    return {
        "profile": "release",
        "case_labels": [CASE_LABEL],
        "case_count": 1,
        "rstim_binary_path": "/tmp/target/release/rstim",
        "rustc_version": "rustc 1.93.1",
        "cargo_version": "cargo 1.93.1",
        "stim_cli_status": "ok",
    }


class ReleaseSpeedCaseCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.results_dir = Path(self.tmpdir.name) / "results"
        self.results_dir.mkdir()
        self.write_bundle(valid_summary(), valid_environment())

    def write_bundle(self, summary: dict[str, object], environment: dict[str, object]) -> None:
        (self.results_dir / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
        (self.results_dir / "environment.json").write_text(json.dumps(environment), encoding="utf-8")
        (self.results_dir / "report.md").write_text(f"# Report\n\n### {CASE_LABEL}\n", encoding="utf-8")

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(CHECKER),
                "--results-dir",
                str(self.results_dir),
                "--case",
                CASE_LABEL,
                "--workload",
                "sample",
                "--required-variants",
                REQUIRED_VARIANTS,
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_accepts_valid_release_case(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"PASS release speed case {CASE_LABEL}", result.stdout)

    def test_rejects_missing_required_variant(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        case["present_variants"] = ["rstim-compiled", "rstim-interpreted"]
        case["variants"] = [
            {"tool_variant": "rstim-compiled", "status": "completed"},
            {"tool_variant": "rstim-interpreted", "status": "completed"},
        ]
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required variant stim-cli", result.stderr)

    def test_rejects_duplicate_requested_case(self) -> None:
        summary = valid_summary()
        summary["cases"].append(copy.deepcopy(summary["cases"][0]))  # type: ignore[attr-defined,index]
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("case rep-sample-d13-r13 must be present exactly once", result.stderr)

    def test_rejects_wrong_workload(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        case["workload"] = "detect"
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("case rep-sample-d13-r13 workload must be sample", result.stderr)

    def test_rejects_required_variant_not_completed(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        variants = case["variants"]
        assert isinstance(variants, list)
        variants[0]["status"] = "tool_failed"  # type: ignore[index]
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("required variant rstim-compiled status is not completed", result.stderr)

    def test_rejects_missing_environment_metadata(self) -> None:
        environment = valid_environment()
        del environment["rstim_binary_path"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json missing rstim_binary_path", result.stderr)

    def test_rejects_missing_report_file(self) -> None:
        (self.results_dir / "report.md").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required release file: report.md", result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_release_speed_case -q
```

Expected: nonzero failure because `tools/check_rstim_vs_stim_release_speed_case.py` does not exist yet.

### Task 2: Checker Implementation

**Files:**
- Create: `tools/check_rstim_vs_stim_release_speed_case.py`

**Interfaces:**
- Produces: CLI `main(argv: list[str] | None = None) -> int`.
- Produces: success line `PASS release speed case <case>`.
- Consumes: `summary.json`, `report.md`, and `environment.json` under `--results-dir`.

- [ ] **Step 1: Implement the minimal checker**

Create `tools/check_rstim_vs_stim_release_speed_case.py` with:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_RELEASE_FILES = ("summary.json", "report.md", "environment.json")
REQUIRED_ENVIRONMENT_FIELDS = (
    "rstim_binary_path",
    "rustc_version",
    "cargo_version",
    "stim_cli_status",
)


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def parse_required_variants(raw: str) -> list[str]:
    variants = [variant.strip() for variant in raw.split(",") if variant.strip()]
    if not variants:
        raise ValueError("no required variants requested")
    return variants


def validate_release_files(results_dir: Path) -> tuple[Path, Path, Path]:
    paths: list[Path] = []
    for filename in REQUIRED_RELEASE_FILES:
        path = results_dir / filename
        if not path.is_file():
            raise ValueError(f"missing required release file: {filename}")
        paths.append(path)
    return paths[0], paths[1], paths[2]


def variants_by_name(case: dict[str, Any]) -> dict[str, dict[str, Any]]:
    variants: dict[str, dict[str, Any]] = {}
    raw_variants = case.get("variants")
    if not isinstance(raw_variants, list):
        return variants
    for variant in raw_variants:
        if not isinstance(variant, dict):
            continue
        name = variant.get("tool_variant")
        if isinstance(name, str):
            variants[name] = variant
    return variants


def validate_case(summary: dict[str, Any], case_label: str, workload: str, required_variants: list[str]) -> None:
    cases = summary.get("cases")
    if not isinstance(cases, list):
        raise ValueError("summary.json missing cases")
    matches = [case for case in cases if isinstance(case, dict) and case.get("case_label") == case_label]
    if len(matches) != 1:
        raise ValueError(f"case {case_label} must be present exactly once")
    case = matches[0]
    if case.get("workload") != workload:
        raise ValueError(f"case {case_label} workload must be {workload}")

    present_variants = case.get("present_variants")
    if not isinstance(present_variants, list):
        raise ValueError("summary.json case missing present_variants")
    present_variant_set = {variant for variant in present_variants if isinstance(variant, str)}
    variants = variants_by_name(case)
    for required in required_variants:
        if required not in present_variant_set or required not in variants:
            raise ValueError(f"missing required variant {required}")
        if variants[required].get("status") != "completed":
            raise ValueError(f"required variant {required} status is not completed")


def validate_environment(environment: dict[str, Any], case_label: str) -> None:
    if environment.get("profile") != "release":
        raise ValueError("environment.json profile must be release")
    for field in REQUIRED_ENVIRONMENT_FIELDS:
        value = environment.get(field)
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"environment.json missing {field}")

    case_labels = environment.get("case_labels")
    case_label_value = environment.get("case_label")
    if isinstance(case_labels, list):
        if case_label not in case_labels:
            raise ValueError(f"environment.json missing case label {case_label}")
    elif case_label_value != case_label:
        raise ValueError(f"environment.json missing case label {case_label}")


def validate_report(report_path: Path, case_label: str) -> None:
    report = report_path.read_text(encoding="utf-8")
    if case_label not in report:
        raise ValueError(f"report.md missing case label {case_label}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--workload", required=True)
    parser.add_argument("--required-variants", required=True)
    args = parser.parse_args(argv)

    try:
        summary_path, report_path, environment_path = validate_release_files(args.results_dir)
        required_variants = parse_required_variants(args.required_variants)
        summary = require_dict(load_json(summary_path), "summary.json")
        environment = require_dict(load_json(environment_path), "environment.json")
        validate_case(summary, args.case, args.workload, required_variants)
        validate_environment(environment, args.case)
        validate_report(report_path, args.case)
    except Exception as exc:
        print(f"ERROR release speed case check failed: {exc}", file=sys.stderr)
        return 1

    print(f"PASS release speed case {args.case}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Run checker tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_release_speed_case -q
```

Expected: exit 0.

### Task 3: Checked Repetition Evidence

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/environment.json`

**Interfaces:**
- Consumes: `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite`.
- Produces: checked artifact directory accepted by the checker.

- [ ] **Step 1: Generate the release evidence bundle**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite \
  --profile release \
  --cases rep-sample-d13-r13 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample
```

Expected: exit 0 and the result directory contains `raw.jsonl`, `summary.json`,
`report.md`, and `environment.json`.

- [ ] **Step 2: Remove unrequested raw timing artifact**

Run:

```sh
rm -f benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/raw.jsonl
```

Expected: only `summary.json`, `report.md`, and `environment.json` remain in
the checked evidence directory.

- [ ] **Step 3: Mark the environment as published issue #434 evidence**

Update `environment.json` to add:

```json
{
  "evidence_kind": "repetition release speed evidence",
  "published_artifact": true,
  "source_issue": 434
}
```

Keep the runner-recorded `case_labels`, `command_line`, `profile`,
`rstim_binary_path`, Rust/Cargo versions, and Stim probe metadata unchanged.

- [ ] **Step 4: Validate the generated artifact content**

Run:

```sh
python3 tools/check_rstim_vs_stim_release_speed_case.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample \
  --case rep-sample-d13-r13 \
  --workload sample \
  --required-variants stim-cli,rstim-interpreted,rstim-compiled
```

Expected: `PASS release speed case rep-sample-d13-r13`.

### Task 4: Verification And Commit

**Files:**
- Modify: repository index only.

**Interfaces:**
- Consumes: all files from Tasks 1-3.
- Produces: committed implementation ready for PR.

- [ ] **Step 1: Run focused required verification**

Run:

```sh
python3 tools/check_rstim_vs_stim_release_speed_case.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample \
  --case rep-sample-d13-r13 \
  --workload sample \
  --required-variants stim-cli,rstim-interpreted,rstim-compiled
python3 -m unittest tools.test_check_rstim_vs_stim_release_speed_case -q
```

Expected: checker prints `PASS release speed case rep-sample-d13-r13`; unittest
exits 0.

- [ ] **Step 2: Run repository verification**

Run:

```sh
cargo test
```

Expected: exit 0.

- [ ] **Step 3: Check diff hygiene**

Run:

```sh
git diff --check
git status --short
```

Expected: no whitespace errors; status shows only intended files and commits.

- [ ] **Step 4: Commit implementation**

Run:

```sh
git add tools/check_rstim_vs_stim_release_speed_case.py \
  tools/test_check_rstim_vs_stim_release_speed_case.py \
  benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/summary.json \
  benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/report.md \
  benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/environment.json \
  docs/superpowers/plans/2026-07-10-issue-434-repetition-release-speed-evidence.md
git commit -m "feat: publish repetition release speed evidence"
```
