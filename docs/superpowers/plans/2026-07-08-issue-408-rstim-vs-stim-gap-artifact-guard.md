# Issue 408 rstim-vs-Stim Gap Artifact Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `python3 tools/check_rstim_vs_stim_gap_artifact.py [speed-summary.json]`, a focused guard that preserves the checked issue #406 speed-gap artifact identity.

**Architecture:** Keep the checker as a small Python standard-library script with a literal semantic fingerprint for the selected checked case. Unit tests write synthetic summaries under temporary directories, call the script through `subprocess`, and verify both the passing checked artifact and negative controls. The default artifact path additionally compares its SHA-256 to the recorded hash in `site/benchmark-site.json`; explicit fixture paths skip that hash check.

**Tech Stack:** Python 3 standard library, `unittest`, JSON, SHA-256, existing `site/benchmark-site.json`, committed `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`, Cargo workspace verification.

## Global Constraints

- Selected case label is exactly `stim-style-surface-sample-d11-r100-b1024`.
- Expected workload is exactly `sample`.
- Expected tier is exactly `report_only`.
- Expected present variants are exactly `["rstim-compiled", "rstim-interpreted", "stim-cli"]`.
- Required completed variants for the gap are exactly `stim-cli` and `rstim-compiled`.
- Expected `stim-cli` sample count is `1`.
- Expected `rstim-compiled` sample count is `1`.
- Expected `stim-cli` median shots/s is `5690.64878525516`.
- Expected `rstim-compiled` median shots/s is `21.774891038227285`.
- Expected ratio range is inclusive lower/upper bounds `200.0` and `300.0`.
- Default checked artifact path is `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`.
- Default manifest path is `site/benchmark-site.json`.
- Default path validation compares SHA-256 only when the manifest has a recorded hash entry for the default artifact.
- Explicit fixture paths skip the manifest hash check.
- PASS text must be `PASS checked #406 gap is preserved: stim-cli is 261.34x faster than rstim-compiled`.
- Reject equal-speed, missing-variant, changed-rate, and overwritten-artifact synthetic summaries.
- Do not modify sampler code or regenerate checked benchmark artifacts.
- Final verification must include `cargo test`.

---

### Task 1: Add Gap Artifact Checker And Tests

**Files:**
- Create: `tools/test_check_rstim_vs_stim_gap_artifact.py`
- Create: `tools/check_rstim_vs_stim_gap_artifact.py`
- Modify: `docs/superpowers/plans/2026-07-08-issue-408-rstim-vs-stim-gap-artifact-guard.md`

**Interfaces:**
- Consumes: `site/benchmark-site.json`, `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`.
- Produces: CLI `python3 tools/check_rstim_vs_stim_gap_artifact.py [speed-summary.json]` with exit code `0` for preserved checked artifact and nonzero for rejected summaries.

- [x] **Step 1: Write the failing tests**

Create `tools/test_check_rstim_vs_stim_gap_artifact.py` with tests that call the checker as a subprocess:

```python
#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_gap_artifact.py"
DEFAULT_SUMMARY = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"


def selected_case(
    *,
    present_variants: list[str] | None = None,
    stim_rate: float = 5690.64878525516,
    rstim_rate: float = 21.774891038227285,
    stim_status: str = "completed",
    rstim_status: str = "completed",
    stim_samples: int = 1,
    rstim_samples: int = 1,
) -> dict[str, object]:
    return {
        "case_label": SELECTED_CASE_LABEL,
        "workload": "sample",
        "tier": "report_only",
        "present_variants": present_variants
        if present_variants is not None
        else ["rstim-compiled", "rstim-interpreted", "stim-cli"],
        "variants": [
            {
                "tool_variant": "stim-cli",
                "sample_count": stim_samples,
                "median_shots_per_second": stim_rate,
                "status": stim_status,
            },
            {
                "tool_variant": "rstim-compiled",
                "sample_count": rstim_samples,
                "median_shots_per_second": rstim_rate,
                "status": rstim_status,
            },
        ],
    }


class RstimVsStimGapArtifactCheckerTest(unittest.TestCase):
    def run_checker(self, path: Path | None = None) -> subprocess.CompletedProcess[str]:
        args = ["python3", str(CHECKER)]
        if path is not None:
            args.append(str(path))
        return subprocess.run(args, cwd=REPO_ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)

    def write_summary(self, case: dict[str, object]) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        path = Path(tmpdir.name) / "speed-summary.json"
        path.write_text(json.dumps({"cases": [case]}), encoding="utf-8")
        return tmpdir, path

    def test_default_checked_artifact_passes(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "PASS checked #406 gap is preserved: stim-cli is 261.34x faster than rstim-compiled",
            result.stdout,
        )

    def test_rejects_equal_speed_fixture(self) -> None:
        _, path = self.write_summary(selected_case(stim_rate=100.0, rstim_rate=100.0))
        result = self.run_checker(path)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("ratio outside 200-300", result.stderr)

    def test_rejects_changed_large_gap_fixture(self) -> None:
        _, path = self.write_summary(selected_case(stim_rate=6000.0, rstim_rate=24.0))
        result = self.run_checker(path)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("selected-case rate changed", result.stderr)

    def test_rejects_missing_rstim_compiled_fixture(self) -> None:
        case = selected_case(present_variants=["stim-cli"])
        case["variants"] = [case["variants"][0]]  # type: ignore[index]
        _, path = self.write_summary(case)
        result = self.run_checker(path)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing rstim-compiled", result.stderr)

    def test_rejects_default_path_copy_with_manifest_hash_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo = Path(tmpdir) / "repo"
            summary_path = repo / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
            manifest_path = repo / "site/benchmark-site.json"
            summary_path.parent.mkdir(parents=True)
            manifest_path.parent.mkdir(parents=True)
            summary_path.write_text(json.dumps({"cases": [selected_case(stim_rate=6000.0, rstim_rate=23.0)]}), encoding="utf-8")
            manifest_path.write_text((REPO_ROOT / "site/benchmark-site.json").read_text(encoding="utf-8"), encoding="utf-8")
            result = subprocess.run(
                ["python3", str(CHECKER)],
                cwd=repo,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("checked artifact hash differs from site manifest", result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [x] **Step 2: Run the tests to verify RED**

Run:

```bash
python3 -m unittest tools.test_check_rstim_vs_stim_gap_artifact -v
```

Expected: FAIL because `tools/check_rstim_vs_stim_gap_artifact.py` does not exist yet.

- [x] **Step 3: Implement the checker**

Create `tools/check_rstim_vs_stim_gap_artifact.py` with these concrete elements:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any


DEFAULT_SUMMARY_PATH = Path("benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json")
DEFAULT_MANIFEST_PATH = Path("site/benchmark-site.json")
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"
EXPECTED_WORKLOAD = "sample"
EXPECTED_TIER = "report_only"
EXPECTED_PRESENT_VARIANTS = ["rstim-compiled", "rstim-interpreted", "stim-cli"]
EXPECTED_RATES = {
    "stim-cli": 5690.64878525516,
    "rstim-compiled": 21.774891038227285,
}
EXPECTED_SAMPLE_COUNTS = {
    "stim-cli": 1,
    "rstim-compiled": 1,
}
RATIO_MIN = 200.0
RATIO_MAX = 300.0
RATE_REL_TOL = 1e-12
RATE_ABS_TOL = 1e-9
```

Implement:

- `load_json(path: Path) -> Any`
- `sha256_file(path: Path) -> str`
- `find_selected_case(summary: dict[str, Any]) -> dict[str, Any]`
- `variants_by_name(case: dict[str, Any]) -> dict[str, dict[str, Any]]`
- `validate_case(summary: dict[str, Any]) -> float`
- `recorded_manifest_sha256(manifest: dict[str, Any], artifact_path: str) -> str | None`
- `validate_default_hash(summary_path: Path) -> None`
- `main(argv: list[str] | None = None) -> int`

`validate_case()` must raise `ValueError` with these reason substrings:

- `missing selected case`
- `selected-case workload changed`
- `selected-case tier changed`
- `selected-case present variants changed`
- `missing stim-cli`
- `missing rstim-compiled`
- `stim-cli status is not completed`
- `rstim-compiled status is not completed`
- `stim-cli sample count changed`
- `rstim-compiled sample count changed`
- `selected-case rate changed`
- `ratio outside 200-300`

`validate_default_hash()` must compute the summary file hash and compare it to
the manifest hash only when `summary_path == Path.cwd() / DEFAULT_SUMMARY_PATH`
after resolving paths. It must raise `ValueError("checked artifact hash differs from site manifest")`
when the manifest has a recorded SHA-256 for the default artifact and it differs.

`main()` must print `ERROR checked #406 gap is not preserved: <reason>` to stderr
and return `1` on validation failure. On success it must print:

```python
print(f"PASS checked #406 gap is preserved: stim-cli is {ratio:.2f}x faster than rstim-compiled")
```

- [x] **Step 4: Run the focused tests to verify GREEN**

Run:

```bash
python3 -m unittest tools.test_check_rstim_vs_stim_gap_artifact -v
```

Expected: PASS.

- [x] **Step 5: Run the requested positive issue verification**

Run:

```bash
python3 tools/check_rstim_vs_stim_gap_artifact.py \
  benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json
```

Expected output includes:

```text
PASS checked #406 gap is preserved: stim-cli is 261.34x faster than rstim-compiled
```

- [x] **Step 6: Run the requested equal-speed negative control**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
case = {
    'case_label': 'stim-style-surface-sample-d11-r100-b1024',
    'workload': 'sample',
    'tier': 'report_only',
    'present_variants': ['rstim-compiled', 'stim-cli'],
    'variants': [
        {'tool_variant': 'stim-cli', 'sample_count': 1, 'median_shots_per_second': 100.0, 'status': 'completed'},
        {'tool_variant': 'rstim-compiled', 'sample_count': 1, 'median_shots_per_second': 100.0, 'status': 'completed'},
    ],
}
Path('/tmp/equal-speed-summary.json').write_text(json.dumps({'cases': [case]}))
PY
if python3 tools/check_rstim_vs_stim_gap_artifact.py /tmp/equal-speed-summary.json; then
  echo 'unexpected equal-speed pass' >&2
  exit 1
fi
```

Expected: command exits 0 overall because the checker rejects the fixture.

- [x] **Step 7: Run the requested changed-large-gap negative control**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
case = {
    'case_label': 'stim-style-surface-sample-d11-r100-b1024',
    'workload': 'sample',
    'tier': 'report_only',
    'present_variants': ['rstim-compiled', 'stim-cli'],
    'variants': [
        {'tool_variant': 'stim-cli', 'sample_count': 1, 'median_shots_per_second': 6000.0, 'status': 'completed'},
        {'tool_variant': 'rstim-compiled', 'sample_count': 1, 'median_shots_per_second': 24.0, 'status': 'completed'},
    ],
}
Path('/tmp/changed-large-gap-summary.json').write_text(json.dumps({'cases': [case]}))
PY
if python3 tools/check_rstim_vs_stim_gap_artifact.py /tmp/changed-large-gap-summary.json; then
  echo 'unexpected changed-large-gap pass' >&2
  exit 1
fi
```

Expected: command exits 0 overall because the checker rejects the fixture.

- [x] **Step 8: Run repository verification**

Run:

```bash
cargo test
git diff --check
```

Expected: both commands exit 0.

- [x] **Step 9: Commit the implementation**

Run:

```bash
git add tools/check_rstim_vs_stim_gap_artifact.py \
  tools/test_check_rstim_vs_stim_gap_artifact.py \
  docs/superpowers/plans/2026-07-08-issue-408-rstim-vs-stim-gap-artifact-guard.md
git commit -m "test: guard checked rstim-vs-stim speed gap artifact"
```

Expected: commit succeeds.

## Self-Review

- Spec coverage: Task 1 implements the checker, default path, explicit fixture handling, semantic fingerprint, manifest hash check, PASS output, and required negative controls.
- Placeholder scan: no `TBD`, `TODO`, or incomplete implementation steps remain.
- Type consistency: all referenced function names and constants are defined in Task 1.
