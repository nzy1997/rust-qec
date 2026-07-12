# Issue 494 Sampler Performance Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a reviewer-readable sampler-performance readiness JSON/Markdown pair and one PASS/FAIL gate command.

**Architecture:** Add one Python checker that reuses existing portable, semantic, correctness, and historical evidence validators, then derives deterministic JSON and Markdown artifacts from their results. Keep committed artifacts under the existing benchmark results tree and root docs without changing site metadata or benchmark measurements.

**Tech Stack:** Python 3 standard library, unittest, existing `tools.check_all_portable_evidence` registry, existing rstim-vs-Stim checkers, Cargo focused Rust integration tests.

## Global Constraints

- Add `tools/check_sampler_performance_readiness.py`.
- Add `benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json`.
- Add `sampler-performance-readiness.md`.
- Require all four portable bundle checkers.
- Require reference direct/canonical speedup `>= 2.0`.
- Require zero production canonical materializations and one executed d11 repeat iteration in direct reference-build records.
- Require a complete and honestly worded fair CLI comparison.
- Require frame ratio `<= 1.05`.
- Require frame/distribution correctness.
- Require unchanged historical #406 evidence.
- State explicitly that site-facing #379 remains separate.
- Do not change milestone state, close #406/#379, update the site, add board wiring, rerun benchmarks, or claim broad Stim parity.
- Required success line is exactly `PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05`.
- Required focused Rust command is exactly:

```sh
cargo test -p rstim \
  --test reusable_compiled_measurement_sampler \
  --test packed_inverse_tableau_storage \
  --test packed_inverse_tableau_clifford \
  --test packed_inverse_tableau_measurement \
  --test packed_inverse_direct_collapse \
  --test packed_reference_routing \
  --test reference_sample_tree \
  --test repeat_aware_reference_sample \
  --test rare_error_iterator \
  --test frame_instruction_wide_one_qubit_noise \
  --test frame_instruction_wide_depolarize2
```

---

### Task 1: Readiness Checker Tests

**Files:**
- Create: `tools/test_check_sampler_performance_readiness.py`

**Interfaces:**
- Consumes: committed `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`, existing bundle/correctness artifacts, and a future checker module named `tools.check_sampler_performance_readiness`.
- Produces: failing tests for CLI success, derived Markdown, absolute provenance rejection, low reference speedup, high frame ratio, and mocked GitHub milestone failure.

- [ ] **Step 1: Write the failing tests**

Create `tools/test_check_sampler_performance_readiness.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_sampler_performance_readiness.py"
CATALOG = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
COMMITTED_JSON = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json"
COMMITTED_MD = REPO_ROOT / "sampler-performance-readiness.md"
PASS_LINE = "PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05\n"


class SamplerPerformanceReadinessCheckerTest(unittest.TestCase):
    def run_checker(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), *args],
            cwd=REPO_ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_help_imports_without_side_effects(self) -> None:
        result = self.run_checker("--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--catalog", result.stdout)
        self.assertIn("--verify-github", result.stdout)

    def test_cli_accepts_committed_catalog_and_writes_derived_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "readiness.json"
            markdown = Path(tmp) / "readiness.md"

            result = self.run_checker("--catalog", str(CATALOG), "--out", str(out), "--markdown-out", str(markdown))

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, PASS_LINE)
            self.assertEqual(result.stderr, "")
            readiness = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(readiness["status"], "ready")
            self.assertEqual(readiness["bundle_count"], 4)
            self.assertGreaterEqual(readiness["reference_build"]["direct_speedup"], 2.0)
            self.assertEqual(readiness["reference_build"]["direct_canonical_materializations"], 0)
            self.assertEqual(readiness["reference_build"]["direct_executed_repeat_iterations"], 1)
            self.assertLessEqual(readiness["frame_noise"]["candidate_over_baseline"], 1.05)
            self.assertEqual(readiness["frame_noise"]["correctness_status"], "pass")
            self.assertEqual(readiness["distribution_correctness"]["status"], "pass")
            self.assertEqual(readiness["historical_406"]["status"], "preserved")
            self.assertIn("#379", "\n".join(readiness["claim_limits"]))
            text = markdown.read_text(encoding="utf-8")
            for required in ("fair-cli-release", "compiled-steady-release", "reference-build-release", "frame-instruction-wide-release", "#38", "#406", "#379"):
                self.assertIn(required, text)

    def test_committed_markdown_is_derived_from_committed_json(self) -> None:
        checker = __import__("tools.check_sampler_performance_readiness", fromlist=["render_markdown"])
        readiness = json.loads(COMMITTED_JSON.read_text(encoding="utf-8"))

        self.assertEqual(COMMITTED_MD.read_text(encoding="utf-8"), checker.render_markdown(readiness))

    def test_absolute_catalog_provenance_reports_not_ready(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            temp_repo = Path(tmp) / "repo"
            shutil.copytree(REPO_ROOT / "benchmarks", temp_repo / "benchmarks")
            catalog = temp_repo / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
            text = catalog.read_text(encoding="utf-8")
            text = text.replace(
                'value = { case_id = "stim_surface_d11_r100", profile = "release"',
                'host_path = "/tmp/provenance.json"\nvalue = { case_id = "stim_surface_d11_r100", profile = "release"',
                1,
            )
            catalog.write_text(text, encoding="utf-8")
            out = Path(tmp) / "readiness.json"

            result = self.run_checker("--catalog", str(catalog), "--out", str(out))

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("not ready", result.stderr)
            self.assertIn("checked provenance contains host-absolute path", result.stderr)

    def test_reference_speedup_below_two_reports_not_ready(self) -> None:
        checker = __import__("tools.check_sampler_performance_readiness", fromlist=["ReadinessError", "build_readiness"])
        with mock.patch.object(checker.reference_build, "validate_bundle", return_value={"direct_speedup": 1.99}):
            with self.assertRaisesRegex(checker.ReadinessError, "not ready: reference direct/canonical speedup"):
                checker.build_readiness(CATALOG)

    def test_frame_ratio_above_limit_reports_not_ready(self) -> None:
        checker = __import__("tools.check_sampler_performance_readiness", fromlist=["ReadinessError", "build_readiness"])
        replacement = {
            "builds": 803,
            "attempts": 82290688,
            "legacy_setups": 80362,
            "candidate_over_baseline": 1.06,
            "outcome": "regressed",
        }
        with mock.patch.object(checker.instruction_wide, "validate_bundle", return_value=replacement):
            with self.assertRaisesRegex(checker.ReadinessError, "not ready: frame candidate/baseline ratio"):
                checker.build_readiness(CATALOG)

    def test_mocked_open_github_milestone_fails_with_title(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            github_json = Path(tmp) / "issues.json"
            github_json.write_text(
                json.dumps([
                    {
                        "number": 999,
                        "title": "Operational sampler-performance milestone closure",
                        "state": "OPEN",
                        "milestone": {"title": "M4: Measured Optimization Closure"},
                    }
                ]),
                encoding="utf-8",
            )
            out = Path(tmp) / "readiness.json"

            result = self.run_checker(
                "--catalog", str(CATALOG),
                "--out", str(out),
                "--verify-github", "nzy1997/rstim",
                "--github-json", str(github_json),
            )

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("not ready", result.stderr)
            self.assertIn("Operational sampler-performance milestone closure", result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_sampler_performance_readiness -q
```

Expected: FAIL because `tools/check_sampler_performance_readiness.py`, the committed JSON, and the committed Markdown do not exist yet.

- [ ] **Step 3: Commit the test red state**

Do not commit while tests are red. Keep the failing test file staged only after implementation passes in Task 2.

### Task 2: Readiness Checker Implementation

**Files:**
- Create: `tools/check_sampler_performance_readiness.py`
- Create: `benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json`
- Create: `sampler-performance-readiness.md`
- Modify: `tools/test_check_sampler_performance_readiness.py`

**Interfaces:**
- Consumes: `load_catalog`, `validate_catalog`, `tools.check_all_portable_evidence.CHECKERS`, `tools.check_rstim_vs_stim_reference_build_evidence.validate_bundle`, `tools.check_rstim_vs_stim_fair_cli_evidence.validate_bundle`, `tools.check_rstim_vs_stim_instruction_wide_noise_evidence.validate_bundle`, `tools.check_rstim_vs_stim_expanded_correctness`, and `tools.check_rstim_vs_stim_gap_artifact`.
- Produces:
  - `class ReadinessError(RuntimeError)`.
  - `build_readiness(catalog_path: Path, verify_github: str | None = None, github_json: Path | None = None) -> dict[str, object]`.
  - `render_markdown(readiness: dict[str, object]) -> str`.
  - CLI success line `PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05`.

- [ ] **Step 1: Implement the checker**

Create `tools/check_sampler_performance_readiness.py` with these concrete behaviors:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator.portable_provenance import EXPECTED_BUNDLE_IDS, load_catalog, validate_catalog
from tools import check_all_portable_evidence as portable
from tools import check_rstim_vs_stim_expanded_correctness as expanded_correctness
from tools import check_rstim_vs_stim_fair_cli_evidence as fair_cli
from tools import check_rstim_vs_stim_gap_artifact as gap_artifact
from tools import check_rstim_vs_stim_instruction_wide_noise_evidence as instruction_wide
from tools import check_rstim_vs_stim_reference_build_evidence as reference_build

PASS_LINE = "PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05"
```

Implement helpers to:

- hash files with SHA-256;
- load JSON objects and JSONL records;
- resolve catalog bundle paths relative to `catalog_path.resolve().parents[2]`;
- run all portable bundle checkers and collect pass lines;
- extract direct reference phase counters from
  `reference-build-release/raw.jsonl`;
- validate expanded correctness by calling the existing module validators using
  the committed relative paths;
- validate historical #406 by calling `gap_artifact.validate_default_hash` and
  `gap_artifact.validate_case`;
- read mocked GitHub JSON or run:

```sh
gh issue list --repo <owner/repo> --state open --milestone "M4: Measured Optimization Closure" --json number,title,state,milestone --limit 100
```

and fail if any returned issue is open.

The `build_readiness` function must raise `ReadinessError` with messages that
start `not ready:` for all readiness failures, including:

```python
if reference_result["direct_speedup"] < 2.0:
    raise ReadinessError("not ready: reference direct/canonical speedup below 2.0")
if direct_canonical_materializations != 0:
    raise ReadinessError("not ready: direct reference path recorded production canonical materializations")
if direct_executed_repeat_iterations != 1:
    raise ReadinessError("not ready: direct reference path did not execute exactly one d11 repeat iteration")
if frame_result["candidate_over_baseline"] > 1.05:
    raise ReadinessError("not ready: frame candidate/baseline ratio exceeds 1.05")
```

The JSON object must contain `status`, `catalog_path`, `catalog_sha256`,
`bundle_count`, `bundle_ids`, `portable_bundles`, `reference_build`,
`fair_cli`, `frame_noise`, `distribution_correctness`, `historical_406`,
`focused_rust_tests`, `claim_limits`, and `issues`.

- [ ] **Step 2: Implement Markdown rendering**

`render_markdown(readiness)` must build the document from JSON fields. It must
include:

```markdown
# Sampler Performance Readiness

Status: **ready**

## Evidence Bundles
...
## Readiness Checks
...
## Claim Limits
...
## Issue Links
...
```

The bundle list must link all four bundle directories. The issue list must link
`#38`, `#406`, and `#379` to `https://github.com/nzy1997/rstim/issues/<number>`.
The claim-limits section must include the exact sentence:

```text
Site-facing #379 remains separate; this readiness artifact does not update the site or close #379.
```

- [ ] **Step 3: Run tests to verify GREEN for the implementation**

Run:

```sh
python3 -m unittest tools.test_check_sampler_performance_readiness -q
```

Expected: PASS.

- [ ] **Step 4: Generate committed readiness artifacts**

Run:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --out benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json
```

Expected stdout:

```text
PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05
```

This writes `benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json`
and root `sampler-performance-readiness.md`.

- [ ] **Step 5: Re-run tests after artifact generation**

Run:

```sh
python3 -m unittest tools.test_check_sampler_performance_readiness -q
```

Expected: PASS, including the committed Markdown derivation check.

- [ ] **Step 6: Commit checker, tests, and artifacts**

Run:

```sh
git add tools/check_sampler_performance_readiness.py \
  tools/test_check_sampler_performance_readiness.py \
  benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json \
  sampler-performance-readiness.md
git commit -m "feat: add sampler performance readiness gate"
```

### Task 3: Verification And Pull Request

**Files:**
- No new files.
- Verify: whole branch.

**Interfaces:**
- Consumes: committed checker/artifacts from Task 2.
- Produces: verification evidence and a pushed pull request.

- [ ] **Step 1: Run the issue verification command**

Run:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --out /tmp/rstim-sampler-readiness.json
```

Expected stdout:

```text
PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05
```

- [ ] **Step 2: Run Python readiness tests**

Run:

```sh
python3 -m unittest tools.test_check_sampler_performance_readiness -q
```

Expected: PASS.

- [ ] **Step 3: Run the focused Rust suites required by issue #494**

Run the exact command from Global Constraints.

Expected: PASS.

- [ ] **Step 4: Run the required broad Cargo verification**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 5: Inspect branch diff for scope**

Run:

```sh
git status --short
git diff --stat origin/master..HEAD
git diff --check origin/master..HEAD
```

Expected: only the design, plan, checker, test, readiness JSON, and readiness
Markdown are changed; diff check reports no whitespace errors.

- [ ] **Step 6: Create the pull request**

Run:

```sh
git push -u origin agent/issue-494-publish-sampler-performance-readiness-evidence-run-1
gh pr create --repo nzy1997/rstim --base master --head agent/issue-494-publish-sampler-performance-readiness-evidence-run-1 --title "Publish sampler performance readiness evidence" --body "## Summary
- add a sampler-performance readiness gate that reuses the four portable bundle checkers plus reference, fair CLI, frame-noise, distribution, and #406 guards
- publish the derived readiness JSON and Markdown artifacts with #38/#406/#379 claim limits
- document the non-interactive Superpowers design and plan

## Verification
- python3 tools/check_sampler_performance_readiness.py --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml --out /tmp/rstim-sampler-readiness.json
- python3 -m unittest tools.test_check_sampler_performance_readiness -q
- cargo test -p rstim --test reusable_compiled_measurement_sampler --test packed_inverse_tableau_storage --test packed_inverse_tableau_clifford --test packed_inverse_tableau_measurement --test packed_inverse_direct_collapse --test packed_reference_routing --test reference_sample_tree --test repeat_aware_reference_sample --test rare_error_iterator --test frame_instruction_wide_one_qubit_noise --test frame_instruction_wide_depolarize2
- cargo test

Closes #494"
```

Expected: PR URL is printed.

## Self-Review

- Spec coverage: Task 1 covers all required negative controls; Task 2 covers the checker and committed artifacts; Task 3 covers the exact issue verification, focused Rust suites, `cargo test`, and PR creation.
- Placeholder scan: no `TBD`, `TODO`, `fill in`, or open-ended implementation steps remain.
- Type consistency: `build_readiness`, `render_markdown`, `ReadinessError`, and artifact paths are named consistently across tasks.
