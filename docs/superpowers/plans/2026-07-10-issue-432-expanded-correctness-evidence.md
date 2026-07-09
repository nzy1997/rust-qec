# Expanded Correctness Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish checked expanded `rstim`-vs-Stim correctness evidence that combines source-grounded distribution verification with the existing full fixture correctness summary.

**Architecture:** Keep the new evidence in `benchmarks/rstim_vs_stim_simulator/results/distributions/` and add a dedicated `tools/` checker that validates the distribution summary, rollup manifest, report, catalog hash, and existing full summary. Extend the existing distribution verifier summary metadata only enough for checked provenance.

**Tech Stack:** Python 3.11 stdlib (`argparse`, `hashlib`, `json`, `shutil`, `subprocess`, `sys`, `tomllib`, `unittest`, `pathlib`), existing `benchmarks.rstim_vs_stim_simulator` package, checked JSON/Markdown artifacts, Cargo workspace verification.

## Global Constraints

- Checker command path is `tools/check_rstim_vs_stim_expanded_correctness.py`.
- Required checker success output is exactly `PASS expanded rstim-vs-Stim correctness evidence`.
- Catalog path is `benchmarks/rstim_vs_stim_simulator/distribution_cases.toml`.
- Distribution evidence directory is `benchmarks/rstim_vs_stim_simulator/results/distributions`.
- Existing full fixture summary path is `benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`.
- Keep distribution evidence separate from `results/full/`; do not modify existing full fixture artifacts.
- Checker must verify every catalog case has passing distribution evidence and fail incomplete evidence with `missing distribution evidence for case`.
- Checker must verify checked evidence records the catalog SHA-256 hash.
- Checker must verify the full summary top-level `status` is `pass`.
- Do not update the public benchmark site.
- Do not claim all Stim workloads are covered or formal Stim parity.
- Do not bundle issue #431 frame possible-output regression tests.

---

## File Structure

- Modify `benchmarks/rstim_vs_stim_simulator/verify_distributions.py`: add catalog hash, command line, and environment metadata to emitted summaries.
- Modify `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`: add metadata coverage for the verifier summary.
- Create `tools/check_rstim_vs_stim_expanded_correctness.py`: checked evidence validator CLI.
- Create `tools/test_check_rstim_vs_stim_expanded_correctness.py`: subprocess tests and negative controls for the checker.
- Create `benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json`: checked distribution verifier output.
- Create `benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json`: rollup manifest linking distribution and full summary hashes.
- Create `benchmarks/rstim_vs_stim_simulator/results/distributions/report.md`: reviewer-readable report with scope limits and links.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document the expanded checker command.

---

### Task 1: Distribution Verifier Provenance Metadata

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/verify_distributions.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`

**Interfaces:**
- Produces: `sha256_file(path: Path) -> str`
- Produces: `collect_environment_metadata(stim_command: list[str], rstim_command: list[str]) -> dict[str, object]`
- Produces: verifier summaries with `catalog_sha256`, `environment`, and CLI-only `command_line`.
- Later tasks consume `summary.json["catalog_sha256"]` and `summary.json["environment"]`.

- [ ] **Step 1: Write the failing test**

Add this test method to `VerifyDistributionCliTest` in `benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py`:

```python
    def test_main_records_catalog_hash_command_line_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            cases = temp / "cases.toml"
            cases.write_text("manifest_version = 1\nsuite = \"unit\"\n[[cases]]\n", encoding="utf-8")
            out = temp / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [unit_case()]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.verify_case"
                ) as mocked_verify,
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.collect_environment_metadata",
                    return_value={
                        "stim_command": ["stim"],
                        "rstim_command": ["target/debug/rstim"],
                        "rstim_binary_path": "target/debug/rstim",
                        "stim_version": "stim test",
                        "rustc_version": "rustc test",
                        "cargo_version": "cargo test",
                    },
                ),
            ):
                mocked_verify.return_value = {
                    "case_id": "unit_bell",
                    "status": "pass",
                    "sample_count": 4,
                    "failure_reasons": [],
                    "expected_distribution": {"00": 0.5, "11": 0.5},
                    "source_url": "https://example.test/source",
                    "source_commit": "abc123",
                    "source_line_start": 10,
                    "source_line_end": 20,
                    "stim": {"status": "pass"},
                    "rstim": {"status": "pass"},
                }
                code = main(
                    [
                        "--cases",
                        str(cases),
                        "--rstim",
                        "target/debug/rstim",
                        "--shots",
                        "4",
                        "--out",
                        str(out),
                    ]
                )

            self.assertEqual(code, 0)
            data = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(data["catalog_sha256"], sha256_text(cases))
            self.assertEqual(data["command_line"][0], "python3")
            self.assertIn("--cases", data["command_line"])
            self.assertEqual(data["environment"]["rstim_binary_path"], "target/debug/rstim")
            self.assertEqual(data["environment"]["stim_version"], "stim test")
            self.assertEqual(data["environment"]["rustc_version"], "rustc test")
```

Also add this helper near the top of the test file:

```python
import hashlib


def sha256_text(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions.VerifyDistributionCliTest.test_main_records_catalog_hash_command_line_and_environment -q
```

Expected: fail because `catalog_sha256`, `environment`, or `command_line` is missing.

- [ ] **Step 3: Implement metadata helpers**

In `benchmarks/rstim_vs_stim_simulator/verify_distributions.py`, add imports:

```python
import hashlib
import shutil
```

Add helpers after `_command_from_arg`:

```python
def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _run_version_command(command: list[str]) -> dict[str, object]:
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return {
            "command": command,
            "status": "missing",
            "stdout": "",
            "stderr": str(error),
            "exit_code": None,
        }
    return {
        "command": command,
        "status": "ok" if completed.returncode == 0 else "failed",
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
        "exit_code": completed.returncode,
    }


def _direct_binary_path(command: list[str]) -> str | None:
    if not command:
        return None
    executable = command[0]
    if executable == "cargo":
        return None
    resolved = shutil.which(executable)
    if resolved is not None:
        return resolved
    if Path(executable).exists():
        return executable
    return None


def collect_environment_metadata(
    stim_command: list[str],
    rstim_command: list[str],
) -> dict[str, object]:
    stim_version = _run_version_command([stim_command[0], "--version"]) if stim_command else {
        "command": [],
        "status": "missing",
        "stdout": "",
        "stderr": "stim command is empty",
        "exit_code": None,
    }
    rustc_version = _run_version_command(["rustc", "--version"])
    cargo_version = _run_version_command(["cargo", "--version"])
    return {
        "stim_command": list(stim_command),
        "rstim_command": list(rstim_command),
        "rstim_binary_path": _direct_binary_path(rstim_command),
        "stim_version": stim_version["stdout"] if stim_version["status"] == "ok" else "",
        "stim_version_command": stim_version,
        "rustc_version": rustc_version["stdout"] if rustc_version["status"] == "ok" else "",
        "rustc_version_command": rustc_version,
        "cargo_version": cargo_version["stdout"] if cargo_version["status"] == "ok" else "",
        "cargo_version_command": cargo_version,
    }
```

- [ ] **Step 4: Add metadata to summaries**

In `build_summary`, after `stim_command`, `rstim_command`, and `seeds` are computed, add `catalog_sha256` and `environment` fields to the returned dict:

```python
        "catalog_sha256": sha256_file(args.cases),
        "environment": collect_environment_metadata(stim_command, rstim_command),
```

In `main`, after `summary = build_summary(args)`, add:

```python
    raw_argv = list(sys.argv[1:] if argv is None else argv)
    summary["command_line"] = [
        "python3",
        "-m",
        "benchmarks.rstim_vs_stim_simulator.verify_distributions",
        *raw_argv,
    ]
```

- [ ] **Step 5: Run focused verifier tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/verify_distributions.py benchmarks/rstim_vs_stim_simulator/tests/test_verify_distributions.py
git commit -m "feat: record distribution verifier provenance"
```

---

### Task 2: Expanded Correctness Checker

**Files:**
- Create: `tools/check_rstim_vs_stim_expanded_correctness.py`
- Create: `tools/test_check_rstim_vs_stim_expanded_correctness.py`

**Interfaces:**
- Produces: `main(argv: list[str] | None = None) -> int`
- Consumes: `summary.json`, `expanded-correctness.json`, `report.md`, catalog TOML, and the full correctness summary JSON.
- Later tasks use this checker against checked artifacts.

- [ ] **Step 1: Write failing checker tests**

Create `tools/test_check_rstim_vs_stim_expanded_correctness.py` with fixtures that write a minimal catalog, distribution summary, rollup manifest, report, and full summary in a temporary root. Include tests named:

```python
def test_accepts_complete_expanded_evidence(self) -> None: ...
def test_rejects_missing_distribution_catalog_case(self) -> None: ...
def test_rejects_distribution_case_that_did_not_pass(self) -> None: ...
def test_rejects_stale_catalog_hash(self) -> None: ...
def test_rejects_full_summary_without_pass_status(self) -> None: ...
def test_rejects_rollup_summary_hash_mismatch(self) -> None: ...
```

The missing-case test must assert stderr contains:

```python
"missing distribution evidence for case"
```

- [ ] **Step 2: Run checker tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_correctness -q
```

Expected: fail because the checker module does not exist.

- [ ] **Step 3: Implement the checker**

Create `tools/check_rstim_vs_stim_expanded_correctness.py` with:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any
import tomllib


PASS_LINE = "PASS expanded rstim-vs-Stim correctness evidence"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()
```

Implement small helpers `load_json`, `load_catalog_case_ids`, `require_dict`,
`validate_distribution_summary`, `validate_rollup`, `validate_report`, and
`validate_full_summary`. Raise `ValueError` with exact messages that include:

```text
missing distribution evidence for case <case_id>
distribution evidence for case <case_id> did not pass
distribution summary catalog hash mismatch
full correctness summary status is not pass
expanded rollup distribution summary hash mismatch
```

`main` prints `PASS_LINE` and returns 0 on success; on validation error it
prints the message to stderr and returns 1.

- [ ] **Step 4: Run checker tests to verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_correctness -q
```

Expected: all checker tests pass.

- [ ] **Step 5: Commit**

```sh
git add tools/check_rstim_vs_stim_expanded_correctness.py tools/test_check_rstim_vs_stim_expanded_correctness.py
git commit -m "feat: check expanded correctness evidence"
```

---

### Task 3: Checked Distribution Artifacts And Documentation

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/distributions/report.md`
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`

**Interfaces:**
- Produces checked artifacts consumed by `tools/check_rstim_vs_stim_expanded_correctness.py`.
- Consumes Task 1 verifier metadata and Task 2 checker.

- [ ] **Step 1: Generate checked distribution summary**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --shots 100000 \
  --out benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json
```

Expected: `PASS distribution correctness cases=8 mismatch=0`.

- [ ] **Step 2: Compute artifact hashes**

Run:

```sh
python3 - <<'PY'
from pathlib import Path
from benchmarks.rstim_vs_stim_simulator.verify_distributions import sha256_file
for path in [
    Path("benchmarks/rstim_vs_stim_simulator/distribution_cases.toml"),
    Path("benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json"),
    Path("benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json"),
]:
    print(path, sha256_file(path))
PY
```

Expected: three SHA-256 values to copy into `expanded-correctness.json`.

- [ ] **Step 3: Create rollup and report**

Create `expanded-correctness.json` with `status = "pass"`, relative artifact
paths, SHA-256 hashes, catalog case IDs, and scope text limited to the catalog
distributions plus the existing full fixture summary.

Create `report.md` with:

```markdown
# Expanded rstim-vs-Stim Correctness Evidence

Status: pass

## Checked Artifacts

- Distribution summary: `benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json`
- Expanded rollup: `benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json`
- Existing full fixture correctness summary: `benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`

## Scope

This evidence covers the source-grounded small-circuit distribution catalog and
the existing checked d11/r100 full fixture summary. It does not extend coverage
to issue #431 test artifacts or to unlisted Stim workloads.

## Verification

```sh
python3 tools/check_rstim_vs_stim_expanded_correctness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --distribution-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-summary benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
```
```

- [ ] **Step 4: Document checker in README**

Add an "Expanded Correctness Evidence" section to
`benchmarks/rstim_vs_stim_simulator/README.md` with the exact checker command
and expected PASS line.

- [ ] **Step 5: Run issue verification**

Run:

```sh
python3 tools/check_rstim_vs_stim_expanded_correctness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --distribution-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-summary benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_correctness -q
```

Expected: checker prints `PASS expanded rstim-vs-Stim correctness evidence`, and unit tests pass.

- [ ] **Step 6: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/results/distributions benchmarks/rstim_vs_stim_simulator/README.md
git commit -m "docs: publish expanded correctness evidence"
```

---

## Final Verification

Run:

```sh
python3 tools/check_rstim_vs_stim_expanded_correctness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --distribution-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-summary benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
python3 -m unittest tools.test_check_rstim_vs_stim_expanded_correctness -q
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_verify_distributions -q
cargo test
```

Expected: all commands exit 0. The checker prints exactly `PASS expanded rstim-vs-Stim correctness evidence`.

## Self-Review

- Spec coverage: Tasks cover verifier provenance, checker, checked artifacts, README, issue verification, and full Cargo verification.
- Placeholder scan: no placeholder markers remain.
- Type consistency: the checker consumes `summary.json`, `expanded-correctness.json`, `report.md`, catalog TOML, and the full summary paths named in the spec.
- Scope check: no public benchmark site updates and no issue #431 artifacts are included.
