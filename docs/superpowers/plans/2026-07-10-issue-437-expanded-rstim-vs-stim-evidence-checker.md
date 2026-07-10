# Issue 437 Expanded rstim-vs-Stim Evidence Checker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one command that verifies the complete checked rstim-vs-Stim correctness and release-speed evidence pack, including stable missing-case failures and issue #406 debug-summary reuse protection.

**Architecture:** A new Python umbrella checker imports the three existing focused checker modules, adds an immutable manifest for the required circuit speed cases, and performs coverage/provenance preflights before delegating detailed validation. Subprocess unit tests copy only checked artifacts into temporary directories and mutate one contract at a time. No benchmark artifacts are regenerated or modified.

**Tech Stack:** Python 3 standard library (`argparse`, `dataclasses`, `json`, `pathlib`, `unittest`, `subprocess`, `tempfile`, `shutil`), existing repository checker modules, Markdown documentation, Cargo workspace verification.

## Global Constraints

- The success line is exactly `PASS expanded rstim-vs-Stim evidence`.
- Missing required circuit or DEM cases fail with `missing required evidence case <case-label>`.
- The DEM case is `stim-style-surface-dem-sample-d11-r100-b1024` with variants `stim-sample-dem` and `rstim-sample-dem`.
- Circuit speed checks are threshold-free and require release metadata plus completed variants.
- Do not create, refresh, or modify benchmark artifacts under `benchmarks/rstim_vs_stim_simulator/results/`.
- Do not update public benchmark site metadata.

---

### Task 1: Add failing umbrella-checker integration tests

**Files:**
- Create: `tools/test_check_rstim_vs_stim_expanded_evidence.py`
- Test: `tools/test_check_rstim_vs_stim_expanded_evidence.py`

**Interfaces:**
- Consumes: the checked dependency artifacts already committed under `benchmarks/rstim_vs_stim_simulator/results/`.
- Produces: subprocess acceptance tests for `tools/check_rstim_vs_stim_expanded_evidence.py` and its exact CLI/error contract.

- [ ] **Step 1: Create the complete subprocess test fixture**

Create `tools/test_check_rstim_vs_stim_expanded_evidence.py` with repository path constants, a `TemporaryDirectory`, copied release directories, JSON mutation helpers, and this checker invocation shape:

```python
class ExpandedEvidenceCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.root = Path(self.tmpdir.name)
        self.speed_dirs: list[Path] = []
        for source in DEFAULT_SPEED_DIRS:
            destination = self.root / source.name
            shutil.copytree(source, destination)
            self.speed_dirs.append(destination)
        self.dem_speed_dir = self.root / DEFAULT_DEM_SPEED_DIR.name
        shutil.copytree(DEFAULT_DEM_SPEED_DIR, self.dem_speed_dir)

    def run_checker(
        self,
        *,
        correctness_dir: Path = DEFAULT_CORRECTNESS_DIR,
        full_correctness: Path = DEFAULT_FULL_CORRECTNESS,
        speed_dirs: list[Path] | None = None,
        dem_speed_dir: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(CHECKER),
                "--correctness-dir",
                str(correctness_dir),
                "--full-correctness",
                str(full_correctness),
                "--speed-dirs",
                ",".join(str(path) for path in (speed_dirs or self.speed_dirs)),
                "--dem-speed-dir",
                str(dem_speed_dir or self.dem_speed_dir),
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
```

Add a `rewrite_json(path, mutate)` helper that loads an object, applies the
callable, and writes sorted, indented JSON with a trailing newline.

- [ ] **Step 2: Add the positive and required negative controls**

Add tests with these exact assertions:

```python
def test_accepts_committed_expanded_evidence(self) -> None:
    result = self.run_checker()
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertEqual("PASS expanded rstim-vs-Stim evidence\n", result.stdout)

def test_rejects_missing_surface_detect_case(self) -> None:
    summary_path = self.speed_dirs[2] / "summary.json"
    rewrite_json(
        summary_path,
        lambda data: data.__setitem__(
            "cases",
            [case for case in data["cases"] if case.get("case_label") != SURFACE_CASE],
        ),
    )
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn(f"missing required evidence case {SURFACE_CASE}", result.stderr)

def test_rejects_dem_directory_without_summary(self) -> None:
    (self.dem_speed_dir / "summary.json").unlink()
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn(f"missing required evidence case {DEM_CASE}", result.stderr)

def test_rejects_dem_summary_without_required_case(self) -> None:
    rewrite_json(
        self.dem_speed_dir / "summary.json",
        lambda data: data["cases"][0].__setitem__("case_label", "other-dem-case"),
    )
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn(f"missing required evidence case {DEM_CASE}", result.stderr)
```

Add the remaining focused tests:

```python
def test_rejects_missing_required_speed_variant(self) -> None:
    summary_path = self.speed_dirs[1] / "summary.json"

    def remove_stim_cli(data: dict[str, object]) -> None:
        case = data["cases"][0]
        case["present_variants"].remove("stim-cli")
        case["variants"] = [
            variant for variant in case["variants"]
            if variant.get("tool_variant") != "stim-cli"
        ]

    rewrite_json(summary_path, remove_stim_cli)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("missing required variant stim-cli", result.stderr)

def test_rejects_missing_speed_environment_metadata(self) -> None:
    rewrite_json(
        self.speed_dirs[2] / "environment.json",
        lambda data: data.pop("cargo_version"),
    )
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("environment.json missing cargo_version", result.stderr)

def test_rejects_old_debug_summary_as_release_evidence(self) -> None:
    shutil.copyfile(OLD_DEBUG_SUMMARY, self.speed_dirs[0] / "summary.json")
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("release evidence reuses old #406 debug summary", result.stderr)

def test_rejects_missing_distribution_correctness_case(self) -> None:
    correctness_dir = self.root / "distributions"
    shutil.copytree(DEFAULT_CORRECTNESS_DIR, correctness_dir)
    summary_path = correctness_dir / "summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    missing_case = summary["cases"][0]["case_id"]
    summary["cases"] = summary["cases"][1:]
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    result = self.run_checker(correctness_dir=correctness_dir)
    self.assertNotEqual(result.returncode, 0)
    self.assertIn(f"missing distribution evidence for case {missing_case}", result.stderr)
```

- [ ] **Step 3: Run the new tests and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_evidence -q
```

Expected: FAIL because `tools/check_rstim_vs_stim_expanded_evidence.py` does
not exist; the positive test reports a nonzero subprocess result.

- [ ] **Step 4: Commit the failing tests**

```sh
git add tools/test_check_rstim_vs_stim_expanded_evidence.py
git commit -m "test: specify expanded rstim-vs-Stim evidence checker"
```

---

### Task 2: Implement the umbrella evidence checker

**Files:**
- Create: `tools/check_rstim_vs_stim_expanded_evidence.py`
- Test: `tools/test_check_rstim_vs_stim_expanded_evidence.py`

**Interfaces:**
- Consumes: `--catalog`, `--correctness-dir`, `--full-correctness`, comma-separated `--speed-dirs`, and `--dem-speed-dir`.
- Produces: exit code 0 plus the exact PASS line, or exit code 1 plus one stderr validation message.

- [ ] **Step 1: Add imports, constants, and the immutable circuit speed manifest**

Create the checker with repository import bootstrapping and this manifest:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_rstim_vs_stim_expanded_correctness as correctness_checker
from tools import check_rstim_vs_stim_release_dem_speed_case as dem_checker
from tools import check_rstim_vs_stim_release_speed_case as speed_checker

PASS_LINE = "PASS expanded rstim-vs-Stim evidence"
DEFAULT_CATALOG = Path("benchmarks/rstim_vs_stim_simulator/distribution_cases.toml")
OLD_DEBUG_SUMMARY = Path("benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json")
DEM_CASE = "stim-style-surface-dem-sample-d11-r100-b1024"
DEM_VARIANTS = ("stim-sample-dem", "rstim-sample-dem")

@dataclass(frozen=True)
class SpeedEvidenceSpec:
    case_label: str
    workload: str
    required_variants: tuple[str, ...]
    source_issue: int
    evidence_kind_fragment: str

STANDARD_VARIANTS = ("stim-cli", "rstim-interpreted", "rstim-compiled")
SPEED_EVIDENCE_SPECS = (
    SpeedEvidenceSpec("stim-style-surface-sample-d11-r100-b1024", "sample", STANDARD_VARIANTS, 416, "post-optimization"),
    SpeedEvidenceSpec("rep-sample-d13-r13", "sample", STANDARD_VARIANTS, 434, "repetition"),
    SpeedEvidenceSpec("surface-detect-d13-r13", "detect", STANDARD_VARIANTS, 435, "surface detect"),
)
```

- [ ] **Step 2: Implement correctness and case-coverage validation**

Add JSON object loading, extraction of case labels from `summary.json`, and an
order-independent index from each required case to `(directory, summary)`:

```python
def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.load(handle)
    except FileNotFoundError as exc:
        raise ValueError(f"missing required file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc.msg}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def summary_case_labels(summary: dict[str, Any], label: str) -> list[str]:
    cases = summary.get("cases")
    if not isinstance(cases, list):
        raise ValueError(f"{label} missing cases")
    return [
        case_label
        for case in cases
        if isinstance(case, dict)
        and isinstance((case_label := case.get("case_label")), str)
    ]


def index_speed_evidence(
    speed_dirs: list[Path],
) -> dict[str, tuple[Path, dict[str, Any]]]:
    expected = {spec.case_label for spec in SPEED_EVIDENCE_SPECS}
    indexed: dict[str, tuple[Path, dict[str, Any]]] = {}
    unmatched_dirs: list[Path] = []
    for results_dir in speed_dirs:
        summary = load_json_object(results_dir / "summary.json", "summary.json")
        matched = False
        for case_label in summary_case_labels(summary, "summary.json"):
            if case_label not in expected:
                continue
            matched = True
            if case_label in indexed:
                raise ValueError(f"duplicate required evidence case {case_label}")
            indexed[case_label] = (results_dir, summary)
        if not matched:
            unmatched_dirs.append(results_dir)
    for spec in SPEED_EVIDENCE_SPECS:
        if spec.case_label not in indexed:
            raise ValueError(f"missing required evidence case {spec.case_label}")
    if unmatched_dirs:
        raise ValueError(
            f"speed directory contains no required evidence case: {unmatched_dirs[0]}"
        )
    return indexed
```

Implement `validate_correctness(catalog_path, correctness_dir,
full_correctness_path)` by loading the distribution summary, expanded rollup,
and full summary:

```python
def validate_correctness(
    catalog_path: Path,
    correctness_dir: Path,
    full_correctness_path: Path,
) -> None:
    summary_path = correctness_dir / "summary.json"
    rollup_path = correctness_dir / "expanded-correctness.json"
    summary = load_json_object(summary_path, "distribution summary")
    rollup = load_json_object(rollup_path, "expanded rollup")
    full_summary = load_json_object(full_correctness_path, "full correctness summary")
    correctness_checker.validate_distribution_summary(summary, catalog_path)
    correctness_checker.validate_rollup(
        rollup,
        summary,
        catalog_path=catalog_path,
        summary_path=summary_path,
        full_summary_path=full_correctness_path,
    )
    correctness_checker.validate_report(
        correctness_dir / "report.md",
        summary_path,
        rollup_path,
        full_correctness_path,
    )
    correctness_checker.validate_full_summary(full_summary)
```

- [ ] **Step 3: Delegate detailed circuit speed validation and add provenance guards**

Add the provenance and delegated validation functions:

```python
def validate_speed_provenance(
    environment: dict[str, Any],
    spec: SpeedEvidenceSpec,
) -> None:
    if environment.get("published_artifact") is not True:
        raise ValueError("environment.json published_artifact must be true")
    if environment.get("source_issue") != spec.source_issue:
        raise ValueError(
            f"environment.json source_issue must be {spec.source_issue}"
        )
    evidence_kind = environment.get("evidence_kind")
    if (
        not isinstance(evidence_kind, str)
        or spec.evidence_kind_fragment not in evidence_kind.lower()
    ):
        raise ValueError(
            "environment.json evidence_kind missing "
            f"{spec.evidence_kind_fragment}"
        )


def validate_speed_evidence(speed_dirs: list[Path]) -> None:
    if not speed_dirs:
        raise ValueError("no speed directories requested")
    indexed = index_speed_evidence(speed_dirs)
    old_debug_summary = load_json_object(OLD_DEBUG_SUMMARY, "old #406 summary")
    for spec in SPEED_EVIDENCE_SPECS:
        results_dir, summary = indexed[spec.case_label]
        _, report_path, environment_path = speed_checker.validate_release_files(
            results_dir
        )
        environment = load_json_object(environment_path, "environment.json")
        speed_checker.validate_case(
            summary,
            spec.case_label,
            spec.workload,
            list(spec.required_variants),
        )
        speed_checker.validate_environment(environment, spec.case_label)
        speed_checker.validate_report(report_path, spec.case_label)
        validate_speed_provenance(environment, spec)
        if spec.source_issue == 416 and summary == old_debug_summary:
            raise ValueError("release evidence reuses old #406 debug summary")
```

- [ ] **Step 4: Delegate detailed DEM validation and add umbrella metadata checks**

Before loading other DEM files, require exactly one matching DEM summary case,
then delegate the existing validations and add report/toolchain checks:

```python
def validate_dem_evidence(results_dir: Path) -> None:
    summary_path = results_dir / "summary.json"
    if not summary_path.is_file():
        raise ValueError(f"missing required evidence case {DEM_CASE}")
    summary = load_json_object(summary_path, "DEM summary.json")
    matches = [
        label
        for label in summary_case_labels(summary, "DEM summary.json")
        if label == DEM_CASE
    ]
    if not matches:
        raise ValueError(f"missing required evidence case {DEM_CASE}")
    if len(matches) != 1:
        raise ValueError(f"duplicate required evidence case {DEM_CASE}")

    dem_checker.validate_pinned_metadata()
    dem_checker.validate_required_files(results_dir)
    dem_checker.validate_raw_records(
        results_dir,
        case_label=DEM_CASE,
        required_variants=list(DEM_VARIANTS),
    )
    environment = load_json_object(
        results_dir / "environment.json", "environment.json"
    )
    dem_checker.validate_summary(
        summary,
        case_label=DEM_CASE,
        required_variants=list(DEM_VARIANTS),
    )
    dem_checker.validate_environment(environment, case_label=DEM_CASE)
    report = (results_dir / "report.md").read_text(encoding="utf-8")
    if DEM_CASE not in report:
        raise ValueError(f"report.md missing case label {DEM_CASE}")
    for field in speed_checker.REQUIRED_ENVIRONMENT_FIELDS:
        value = environment.get(field)
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"environment.json missing {field}")
```

- [ ] **Step 5: Add argument parsing and one-message failure handling**

Parse the documented required arguments, split `--speed-dirs` on commas, and
run correctness, circuit speed, and DEM validation in that order:

```python
def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog", default=str(DEFAULT_CATALOG))
    parser.add_argument("--correctness-dir", required=True)
    parser.add_argument("--full-correctness", required=True)
    parser.add_argument("--speed-dirs", required=True)
    parser.add_argument("--dem-speed-dir", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    speed_dirs = [
        Path(raw.strip()) for raw in args.speed_dirs.split(",") if raw.strip()
    ]
    try:
        validate_correctness(
            Path(args.catalog),
            Path(args.correctness_dir),
            Path(args.full_correctness),
        )
        validate_speed_evidence(speed_dirs)
        validate_dem_evidence(Path(args.dem_speed_dir))
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1
    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 6: Run the focused tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_evidence -q
```

Expected: all tests pass.

- [ ] **Step 7: Run the public checker command**

Run:

```sh
python3 tools/check_rstim_vs_stim_expanded_evidence.py \
  --correctness-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-correctness benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json \
  --speed-dirs benchmarks/rstim_vs_stim_simulator/results/release,benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample,benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  --dem-speed-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
```

Expected: `PASS expanded rstim-vs-Stim evidence`.

- [ ] **Step 8: Commit the checker**

```sh
git add tools/check_rstim_vs_stim_expanded_evidence.py
git commit -m "feat: check expanded rstim-vs-Stim evidence"
```

---

### Task 3: Document and verify the completed evidence command

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`
- Test: `tools/test_check_rstim_vs_stim_expanded_evidence.py`

**Interfaces:**
- Consumes: the public checker CLI from Task 2.
- Produces: a discoverable repository command plus final repository verification evidence.

- [ ] **Step 1: Document the umbrella command**

Add an `## Expanded Evidence Pack` section after the focused correctness/DEM
material with the exact four-argument command from Task 2, the exact PASS line,
and one sentence stating that it validates case coverage and metadata without
enforcing timing thresholds.

- [ ] **Step 2: Run formatting and focused verification**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_evidence -q
python3 tools/check_rstim_vs_stim_expanded_evidence.py \
  --correctness-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-correctness benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json \
  --speed-dirs benchmarks/rstim_vs_stim_simulator/results/release,benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample,benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  --dem-speed-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
git diff --check
```

Expected: unit tests pass, checker prints the exact PASS line, and diff check is
silent with exit code 0.

- [ ] **Step 3: Run the workspace regression gate**

Run:

```sh
cargo test
```

Expected: the workspace test suite passes; existing warnings may remain but no
test fails.

- [ ] **Step 4: Commit documentation and any test refinements**

```sh
git add benchmarks/rstim_vs_stim_simulator/README.md tools/test_check_rstim_vs_stim_expanded_evidence.py
git commit -m "docs: describe expanded rstim-vs-Stim evidence check"
```

## Plan Self-Review

- Every design requirement maps to a task and an observable test.
- The plan starts with a failing subprocess test before implementation.
- Function/module names match the existing focused checker APIs.
- The exact issue PASS line and both missing-case controls appear in tests and
  implementation steps.
- The plan has no benchmark generation step, timing threshold, site update,
  placeholder, or unrelated Rust change.
