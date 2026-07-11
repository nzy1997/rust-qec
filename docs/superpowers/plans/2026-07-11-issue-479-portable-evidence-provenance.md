# Issue 479 Portable Evidence Provenance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add schema-v2 portable provenance catalog validation for the four checked rstim-vs-Stim simulator evidence bundles.

**Architecture:** `portable_provenance.py` owns the schema, path, SHA-256, runtime-identity, and recursive host-path validation. `validate_evidence_bundles.py` is a small CLI wrapper. `evidence_bundles.toml` is the portable contract and lists only repo-relative inputs, bundle-relative artifacts, logical executable roles, runtime identities, and portable checked command/provenance values.

**Tech Stack:** Python standard library (`argparse`, `hashlib`, `pathlib`, `re`, `tomllib`, `unittest`), TOML catalog, existing `benchmarks.rstim_vs_stim_simulator` package.

## Global Constraints

- Add exactly `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`, `benchmarks/rstim_vs_stim_simulator/portable_provenance.py`, and `benchmarks/rstim_vs_stim_simulator/validate_evidence_bundles.py`.
- Add tests at `benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py`.
- Initial catalog bundle IDs must be exactly `fair-cli-release`, `compiled-steady-release`, `reference-build-release`, and `frame-instruction-wide-release`.
- Schema version must be exactly `2`.
- Repository inputs must be repo-relative POSIX paths.
- Executable roles must use logical URIs such as `tool://stim`, `tool://rstim`, and `tool://python`.
- Runtime identities must contain role, version, basename, and SHA-256, and must not require a live path.
- Artifact paths must be bundle-relative.
- Checked command/provenance values must not contain host-absolute paths.
- Do not change benchmark measurements, site metadata, the historical #406 artifact, or bundle-specific semantic checkers.

---

### Task 1: Test The Portable Catalog Contract

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py`
- Create later in Task 2: `benchmarks/rstim_vs_stim_simulator/portable_provenance.py`
- Create later in Task 2: `benchmarks/rstim_vs_stim_simulator/validate_evidence_bundles.py`
- Create later in Task 3: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`

**Interfaces:**
- Consumes: future `load_catalog(path: Path) -> dict[str, Any]`, `validate_catalog(catalog: dict[str, Any], catalog_path: Path) -> list[str]`, and CLI module `benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles`.
- Produces: regression tests for success output, exact bundle IDs, repository absolute-path rejection, live-runtime-path rejection, and checked command host-path rejection.

- [ ] **Step 1: Write the failing test**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py` with tests equivalent to:

```python
from __future__ import annotations

import copy
import subprocess
import sys
import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator.portable_provenance import (
    EXPECTED_BUNDLE_IDS,
    load_catalog,
    validate_catalog,
)


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
CATALOG = PACKAGE_DIR / "evidence_bundles.toml"


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles",
            "--catalog",
            str(path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


class ValidateEvidenceBundlesTest(unittest.TestCase):
    def test_cli_accepts_committed_catalog(self) -> None:
        result = run_validator(CATALOG)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS portable evidence catalog bundles=4 schema=2\n")
        self.assertEqual(result.stderr, "")

    def test_catalog_pins_exact_schema_and_bundle_ids(self) -> None:
        catalog = load_catalog(CATALOG)
        bundles = catalog["bundles"]

        self.assertEqual(catalog["schema"], 2)
        self.assertEqual(tuple(bundle["id"] for bundle in bundles), EXPECTED_BUNDLE_IDS)

    def test_repository_inputs_reject_host_absolute_paths(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["repository_inputs"][0]["path"] = "/tmp/fixture.stim"

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("repository path must be relative" in error for error in errors), errors)

    def test_runtime_identity_rejects_required_live_path(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["runtime_identities"][0]["required_live_path"] = True

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(
            any("checked evidence must not require a live runtime path" in error for error in errors),
            errors,
        )

    def test_checked_commands_reject_host_absolute_paths(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["checked_commands"][0]["argv"] = [
            "tool://stim",
            "sample",
            "--in",
            "/tmp/fixture.stim",
        ]

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("checked command contains host-absolute path" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q`

Expected: FAIL or ERROR because `portable_provenance.py`, `validate_evidence_bundles.py`, and `evidence_bundles.toml` do not exist yet.

- [ ] **Step 3: Do not implement production code in this task**

Stop after the red test is observed. Task 2 supplies the implementation.

- [ ] **Step 4: Commit**

Run:

```bash
git add benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py
git commit -m "test: define portable evidence catalog contract"
```

### Task 2: Implement The Validator And CLI

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/portable_provenance.py`
- Create: `benchmarks/rstim_vs_stim_simulator/validate_evidence_bundles.py`
- Test: `benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py`

**Interfaces:**
- Consumes: the tests from Task 1.
- Produces:
  - `SCHEMA_VERSION: int = 2`
  - `EXPECTED_BUNDLE_IDS: tuple[str, ...]`
  - `load_catalog(path: Path) -> dict[str, Any]`
  - `validate_catalog(catalog: dict[str, Any], catalog_path: Path) -> list[str]`
  - CLI success line `PASS portable evidence catalog bundles=4 schema=2`

- [ ] **Step 1: Write minimal implementation**

Implement `portable_provenance.py` with these behaviors:

```python
SCHEMA_VERSION = 2
SUITE = "rstim_vs_stim_simulator"
EXPECTED_BUNDLE_IDS = (
    "fair-cli-release",
    "compiled-steady-release",
    "reference-build-release",
    "frame-instruction-wide-release",
)
```

Validation must:

- parse TOML through `tomllib`;
- compute `repo_root = catalog_path.resolve().parents[2]`;
- require `schema == 2` and `suite == "rstim_vs_stim_simulator"`;
- require `bundles` to be a list with exactly `EXPECTED_BUNDLE_IDS`;
- reject repo paths whose `PurePosixPath(path).is_absolute()` is true, whose drive is nonempty, whose parts contain `""`, `"."`, or `".."`, or whose string contains backslash;
- reject bundle paths with the same path rules;
- verify repository-input and artifact SHA-256 digests against file bytes;
- reject executable roles that do not start with `tool://` or whose suffix is empty;
- require runtime identity fields `role`, `version`, `basename`, and `sha256`;
- reject any runtime identity with `required_live_path is True`;
- reject runtime identity basenames containing `/` or `\`;
- recursively scan `checked_commands[*].argv` and `checked_provenance[*].value` for absolute host paths and report `checked command contains host-absolute path` or `checked provenance contains host-absolute path`;
- continue collecting all validation errors instead of raising on the first catalog error.

Implement `validate_evidence_bundles.py` with:

```python
def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate portable rstim-vs-Stim evidence bundles.")
    parser.add_argument("--catalog", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        catalog = load_catalog(args.catalog)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.catalog}: {error}", file=sys.stderr)
        return 1
    errors = validate_catalog(catalog, args.catalog)
    if errors:
        for error in errors:
            print(f"{args.catalog}: {error}", file=sys.stderr)
        return 1
    print(f"PASS portable evidence catalog bundles={len(catalog['bundles'])} schema={SCHEMA_VERSION}")
    return 0
```

- [ ] **Step 2: Run test to verify it still fails only because the catalog is missing**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q`

Expected: FAIL or ERROR references missing `evidence_bundles.toml`.

- [ ] **Step 3: Commit**

Run:

```bash
git add benchmarks/rstim_vs_stim_simulator/portable_provenance.py benchmarks/rstim_vs_stim_simulator/validate_evidence_bundles.py
git commit -m "feat: add portable evidence catalog validator"
```

### Task 3: Populate The Schema-v2 Catalog

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`
- Test: `benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py`

**Interfaces:**
- Consumes: `validate_catalog()` from Task 2.
- Produces: a committed catalog accepted by the CLI and unit tests.

- [ ] **Step 1: Create the catalog**

Create `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml` with:

- top-level `schema = 2`;
- top-level `suite = "rstim_vs_stim_simulator"`;
- four `[[bundles]]` tables in the required order;
- each bundle `bundle_path` under `benchmarks/rstim_vs_stim_simulator/results/<bundle-id>`;
- artifact entries for every committed file in that bundle with current SHA-256 digest;
- repository inputs for the canonical fixture and manifests with current SHA-256 digest;
- runtime identities copied from each bundle's `environment.json` SHA/version values, represented by role, version, basename, and sha256;
- portable checked command/provenance values using `tool://` roles and repo-relative paths.

Use these canonical repository input digests:

```text
benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229
benchmarks/rstim_vs_stim_simulator/cases.full.toml 9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921
benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml 863bde24be133bee991198220e9729af153595545dce640b093adac36ddf87cb
benchmarks/rstim_vs_stim_simulator/workers/stim_compiled_steady.py 2c8fd5c9ad1e72534e12641f6f241d3cb10a7d64b5a546264246b1c793c3bdaf
```

- [ ] **Step 2: Run the focused validator**

Run:

```bash
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected:

```text
PASS portable evidence catalog bundles=4 schema=2
```

- [ ] **Step 3: Run the focused unit tests**

Run:

```bash
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected: OK.

- [ ] **Step 4: Commit**

Run:

```bash
git add benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
git commit -m "data: add portable evidence bundle catalog"
```

### Task 4: Final Verification

**Files:**
- No new files. Verify all issue files plus the Rust workspace.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: evidence for PR body and final response.

- [ ] **Step 1: Run issue validator command**

Run:

```bash
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected:

```text
PASS portable evidence catalog bundles=4 schema=2
```

- [ ] **Step 2: Run issue unit test command**

Run:

```bash
python3 -m unittest \
  benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected: OK.

- [ ] **Step 3: Run repository-required Rust verification**

Run: `cargo test`

Expected: all tests pass.

- [ ] **Step 4: Inspect final diff**

Run:

```bash
git status -sb
git diff --stat origin/master...HEAD
```

Expected: only issue #479 files and workflow docs changed.
