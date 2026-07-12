# Issue 484 Checked Evidence Portability Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a clean-checkout aggregate checked-evidence gate that validates the portable evidence catalog and all four committed bundles without Stim, Cargo, or `target/` products.

**Architecture:** Keep the existing bundle checkers as the source of semantic truth, but make the frame-noise checker importable without its Stim-dependent runner module. Add one aggregate CLI with an explicit bundle-id registry that loads the catalog, validates it, dispatches checkers in catalog order, preserves each checker PASS line, and prints a final aggregate PASS line.

**Tech Stack:** Python standard library (`argparse`, `dataclasses`, `hashlib`, `importlib`, `json`, `pathlib`, `shutil`, `subprocess`, `sys`, `tempfile`, `tomllib`, `unittest`), existing portable provenance helpers, GitHub Actions YAML, Rust workspace verified by Cargo.

## Global Constraints

- Aggregate interface is exactly `python3 tools/check_all_portable_evidence.py --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`.
- The aggregate command invokes the catalog validator and every registered checker.
- The aggregate command preserves each PASS line.
- Bundle failures print `FAIL portable checked evidence bundle=<bundle-id>`.
- The successful final aggregate line is exactly `PASS portable checked evidence bundles=4`.
- CI job name is `checked-evidence-portability`.
- CI job runs after checkout with standard-library Python only.
- CI job does not install Stim, configure Rust, use Cargo cache/build steps, or create a `target/` directory.
- The negative control uses a fair CLI bundle with a rehashed absolute fixture path and must fail through the aggregate command with `FAIL portable checked evidence bundle=fair-cli-release`.
- Do not build benchmark binaries, rerun timings, modify site provenance, or add cross-machine performance thresholds.
- Run `cargo test --workspace` before completion.

---

## File Structure

- Modify `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`: remove the import-time dependency on `run_frame_instruction_wide_benchmark` and define the constants, summary derivation, report rendering, and `RunnerError` needed for checked-evidence validation locally.
- Create `tools/check_all_portable_evidence.py`: aggregate CLI, catalog validation, explicit checker registry, PASS output, bundle failure output.
- Create `tools/test_check_all_portable_evidence.py`: aggregate success tests, no-Stim import smoke test, registry coverage, and fair CLI negative control.
- Modify `.github/workflows/ci.yml`: add `checked-evidence-portability` job with checkout plus aggregate command only.

### Task 1: Make Frame-Noise Checker Importable Without Stim

**Files:**
- Modify: `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`
- Create: `tools/test_check_all_portable_evidence.py`

**Interfaces:**
- Produces: `tools.check_rstim_vs_stim_instruction_wide_noise_evidence.validate_bundle(results_dir: Path, verify_runtime_binary: Path | None = None) -> tuple[int, int, int]`
- Produces: local `derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]`
- Produces: local `render_report(summary: dict[str, Any]) -> str`
- Produces: local `RunnerError(RuntimeError)`

- [ ] **Step 1: Write the failing import smoke test**

Create `tools/test_check_all_portable_evidence.py` with this initial content:

```python
from __future__ import annotations

import importlib
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class BlockStimImports:
    def find_spec(self, fullname: str, path: object | None = None, target: object | None = None) -> None:
        if fullname == "stim" or fullname.startswith("stim."):
            raise ModuleNotFoundError("blocked stim import during portability smoke test")
        return None


class AllPortableEvidenceCheckerTest(unittest.TestCase):
    def test_aggregate_and_frame_checker_import_without_stim(self) -> None:
        for module_name in (
            "tools.check_all_portable_evidence",
            "tools.check_rstim_vs_stim_instruction_wide_noise_evidence",
            "benchmarks.rstim_vs_stim_simulator.run_frame_instruction_wide_benchmark",
            "benchmarks.rstim_vs_stim_simulator.inspect_fixture_load",
            "benchmarks.rstim_vs_stim_simulator.validate_cases",
        ):
            sys.modules.pop(module_name, None)

        blocker = BlockStimImports()
        sys.meta_path.insert(0, blocker)
        try:
            importlib.import_module("tools.check_rstim_vs_stim_instruction_wide_noise_evidence")
        finally:
            sys.meta_path.remove(blocker)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_all_portable_evidence.AllPortableEvidenceCheckerTest.test_aggregate_and_frame_checker_import_without_stim -q
```

Expected: FAIL with `blocked stim import during portability smoke test`.

- [ ] **Step 3: Replace the frame checker runner import with local validation constants**

In `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`, remove:

```python
from benchmarks.rstim_vs_stim_simulator import run_frame_instruction_wide_benchmark as runner
```

Add these definitions below `REPO_ROOT` and above `REQUIRED_FILES`:

```python
EXPECTED_CASE_ID = "stim_surface_d11_r100"
EXPECTED_STIM_VERSION = "1.15.0"
EXPECTED_FIXTURE_SHA256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
EXPECTED_MANIFEST_SHA256 = "9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921"
TIMER_SCOPE = "process_spawn_stdout_stderr_drain_exit"
OUTPUT_FORMAT = "b8"
CORRECTNESS_MODE = "detect"
CORRECTNESS_OUTPUT_FORMAT = "01"
EXPECTED_OUTPUT_BITS = 12_121
EXPECTED_BYTES_PER_SHOT = 1_516
EXPECTED_OUTPUT_BYTES = 1_552_384
EXPECTED_DETECTORS = 12_000
EXPECTED_OBSERVABLES = 1
EXPECTED_OPERATION_TOTALS = {
    "X_ERROR": {"instructions": 203, "targets": 24_362, "iterator_builds": 203, "attempt_count": 24_946_688},
    "DEPOLARIZE1": {"instructions": 200, "targets": 12_000, "iterator_builds": 200, "attempt_count": 12_288_000},
    "DEPOLARIZE2": {"instructions": 400, "pairs": 44_000, "iterator_builds": 400, "attempt_count": 45_056_000},
}
OPERATION_ORDER = tuple(EXPECTED_OPERATION_TOTALS)


class RunnerError(RuntimeError):
    pass
```

Replace all `runner.` references in the checker:

```text
runner.OPERATION_ORDER -> OPERATION_ORDER
runner.EXPECTED_CASE_ID -> EXPECTED_CASE_ID
runner.TIMER_SCOPE -> TIMER_SCOPE
runner.OUTPUT_FORMAT -> OUTPUT_FORMAT
runner.EXPECTED_OUTPUT_BITS -> EXPECTED_OUTPUT_BITS
runner.EXPECTED_BYTES_PER_SHOT -> EXPECTED_BYTES_PER_SHOT
runner.EXPECTED_OUTPUT_BYTES -> EXPECTED_OUTPUT_BYTES
runner.EXPECTED_DETECTORS -> EXPECTED_DETECTORS
runner.EXPECTED_OBSERVABLES -> EXPECTED_OBSERVABLES
runner.EXPECTED_OPERATION_TOTALS -> EXPECTED_OPERATION_TOTALS
runner.EXPECTED_STIM_VERSION -> EXPECTED_STIM_VERSION
runner.EXPECTED_FIXTURE_SHA256 -> EXPECTED_FIXTURE_SHA256
runner.EXPECTED_MANIFEST_SHA256 -> EXPECTED_MANIFEST_SHA256
runner.RunnerError -> RunnerError
```

Replace the existing `validate_summary_and_report` function with local derivation:

```python
def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    if not records:
        raise RunnerError("cannot derive summary from empty raw records")
    first = records[0]
    operations: list[dict[str, Any]] = []
    for operation in OPERATION_ORDER:
        matches = [record for record in records if record.get("operation") == operation]
        if len(matches) != 1:
            raise RunnerError(f"raw records must contain exactly one {operation} row")
        row = dict(matches[0])
        operation_summary = {
            "operation": operation,
            "sampling_path": row["sampling_path"],
            "instructions": row["instructions"],
            "iterator_builds": row["iterator_builds"],
            "attempt_count": row["attempt_count"],
        }
        if operation == "DEPOLARIZE2":
            operation_summary["pairs"] = row["pairs"]
        else:
            operation_summary["targets"] = row["targets"]
        operations.append(operation_summary)
    return {
        "case_id": first["case_id"],
        "seed": first["seed"],
        "shots": 1024,
        "phase": "measured",
        "round_index": 0,
        "operations": operations,
        "totals": {
            "instructions": sum(row["instructions"] for row in operations),
            "iterator_builds": sum(row["iterator_builds"] for row in operations),
            "attempt_count": sum(row["attempt_count"] for row in operations),
        },
        "measurement": {
            "timer_scope": TIMER_SCOPE,
            "output_format": OUTPUT_FORMAT,
            "output_bits": first["output_bits"],
            "bytes_per_shot": first["bytes_per_shot"],
            "expected_output_bytes": first["expected_output_bytes"],
            "actual_output_bytes": first["actual_output_bytes"],
            "stdout_sha256": first["stdout_sha256"],
            "elapsed_ns": first["elapsed_ns"],
        },
    }


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Instruction-Wide Frame-Noise Evidence",
        "",
        f"Case: `{summary['case_id']}`",
        f"Seed: `{summary['seed']}`",
        f"Timer scope: `{summary['measurement']['timer_scope']}`",
        "",
        "| Operation | Instructions/builds | Targets/pairs | Attempts |",
        "|---|---:|---:|---:|",
    ]
    for row in summary["operations"]:
        target_value = row.get("pairs", row.get("targets"))
        lines.append(
            f"| `{row['operation']}` | {row['iterator_builds']} | {target_value} | {row['attempt_count']} |"
        )
    totals = summary["totals"]
    target_total = sum(row.get("pairs", row.get("targets", 0)) for row in summary["operations"])
    lines.extend(
        [
            f"| **Total** | **{totals['iterator_builds']}** | **{target_total}** | **{totals['attempt_count']}** |",
            "",
            "Measurement output:",
            f"- bits per shot: {summary['measurement']['output_bits']}",
            f"- bytes per shot: {summary['measurement']['bytes_per_shot']}",
            f"- bytes for run: {summary['measurement']['actual_output_bytes']}",
            "",
        ]
    )
    return "\n".join(lines)


def validate_summary_and_report(records: list[dict[str, Any]], summary: dict[str, Any], report: str) -> None:
    expected_summary = derive_summary(records)
    if summary != expected_summary:
        raise ValueError("summary.json does not match summary derived from raw.jsonl")
    expected_report = render_report(expected_summary)
    if report != expected_report:
        raise ValueError("report.md does not match report derived from raw.jsonl")
```

- [ ] **Step 4: Run the focused import smoke test and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_all_portable_evidence.AllPortableEvidenceCheckerTest.test_aggregate_and_frame_checker_import_without_stim -q
```

Expected: PASS.

- [ ] **Step 5: Run the existing frame-noise checker tests**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence -q
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```sh
git add tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py tools/test_check_all_portable_evidence.py
git commit -m "fix: keep frame evidence checker stim-free"
```

### Task 2: Add Aggregate Checker Success Path

**Files:**
- Create: `tools/check_all_portable_evidence.py`
- Modify: `tools/test_check_all_portable_evidence.py`

**Interfaces:**
- Consumes: `portable_provenance.load_catalog(path: Path) -> dict[str, Any]`
- Consumes: `portable_provenance.validate_catalog(catalog: dict[str, Any], catalog_path: Path) -> list[str]`
- Produces: `main(argv: list[str] | None = None) -> int`
- Produces: CLI success output ending with `PASS portable checked evidence bundles=4`

- [ ] **Step 1: Extend aggregate tests for help, registry coverage, and success output**

Append these imports to `tools/test_check_all_portable_evidence.py`:

```python
import subprocess
```

Add these constants below `REPO_ROOT`:

```python
CHECKER = REPO_ROOT / "tools" / "check_all_portable_evidence.py"
CATALOG = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
```

Add this helper and tests to `AllPortableEvidenceCheckerTest`:

```python
    def run_aggregate(self, *extra_args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), *extra_args],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_direct_script_help_imports_without_stim(self) -> None:
        result = self.run_aggregate("--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--catalog", result.stdout)

    def test_registry_covers_required_bundle_ids(self) -> None:
        checker = importlib.import_module("tools.check_all_portable_evidence")

        self.assertEqual(
            set(checker.CHECKERS),
            {
                "fair-cli-release",
                "compiled-steady-release",
                "reference-build-release",
                "frame-instruction-wide-release",
            },
        )

    def test_cli_accepts_committed_catalog_and_all_bundles(self) -> None:
        result = self.run_aggregate("--catalog", str(CATALOG))

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "\n".join(
                [
                    "PASS portable evidence catalog bundles=4 schema=2",
                    "PASS fair CLI sampling evidence variants=2 measured=14",
                    "PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9",
                    "PASS packed reference-build evidence",
                    "PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362",
                    "PASS portable checked evidence bundles=4",
                    "",
                ]
            ),
        )
        self.assertEqual(result.stderr, "")
```

In `test_aggregate_and_frame_checker_import_without_stim`, add the aggregate
module import immediately after the frame checker import:

```python
            importlib.import_module("tools.check_rstim_vs_stim_instruction_wide_noise_evidence")
            importlib.import_module("tools.check_all_portable_evidence")
```

- [ ] **Step 2: Run aggregate tests and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_all_portable_evidence -q
```

Expected: FAIL because `tools/check_all_portable_evidence.py` does not exist.

- [ ] **Step 3: Implement aggregate checker**

Create `tools/check_all_portable_evidence.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
import tomllib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator.portable_provenance import (  # noqa: E402
    SCHEMA_VERSION,
    load_catalog,
    validate_catalog,
)
from tools import check_rstim_vs_stim_compiled_steady_evidence as compiled_steady  # noqa: E402
from tools import check_rstim_vs_stim_fair_cli_evidence as fair_cli  # noqa: E402
from tools import check_rstim_vs_stim_instruction_wide_noise_evidence as instruction_wide  # noqa: E402
from tools import check_rstim_vs_stim_reference_build_evidence as reference_build  # noqa: E402


@dataclass(frozen=True)
class BundleChecker:
    validate: Callable[[Path], Any]
    pass_line: Callable[[Any], str]


def _fair_cli_pass_line(result: Any) -> str:
    variants, measured = result
    return f"PASS fair CLI sampling evidence variants={variants} measured={measured}"


def _compiled_steady_pass_line(result: Any) -> str:
    variants, measured, lifecycle = result
    return f"PASS compiled steady-state sampling evidence variants={variants} measured={measured} lifecycle={lifecycle}"


def _reference_build_pass_line(result: Any) -> str:
    return "PASS packed reference-build evidence"


def _instruction_wide_pass_line(result: Any) -> str:
    builds, attempts, legacy_setups = result
    return (
        "PASS instruction-wide frame-noise evidence "
        f"builds={builds} attempts={attempts} legacy_setups={legacy_setups}"
    )


CHECKERS: dict[str, BundleChecker] = {
    "fair-cli-release": BundleChecker(fair_cli.validate_bundle, _fair_cli_pass_line),
    "compiled-steady-release": BundleChecker(compiled_steady.validate_bundle, _compiled_steady_pass_line),
    "reference-build-release": BundleChecker(reference_build.validate_bundle, _reference_build_pass_line),
    "frame-instruction-wide-release": BundleChecker(instruction_wide.validate_bundle, _instruction_wide_pass_line),
}


def _repo_root_from_catalog(catalog_path: Path) -> Path:
    return catalog_path.resolve().parents[2]


def _bundle_path(repo_root: Path, bundle: dict[str, Any]) -> Path:
    raw_path = bundle["bundle_path"]
    return repo_root / PurePosixPath(raw_path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate all portable checked evidence bundles.")
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

    bundles = catalog["bundles"]
    print(f"PASS portable evidence catalog bundles={len(bundles)} schema={SCHEMA_VERSION}")
    repo_root = _repo_root_from_catalog(args.catalog)
    for bundle in bundles:
        bundle_id = bundle["id"]
        checker = CHECKERS.get(bundle_id)
        if checker is None:
            print(
                f"FAIL portable checked evidence bundle={bundle_id}: no registered checker",
                file=sys.stderr,
            )
            return 1
        try:
            result = checker.validate(_bundle_path(repo_root, bundle))
        except Exception as error:
            print(f"FAIL portable checked evidence bundle={bundle_id}: {error}", file=sys.stderr)
            return 1
        print(checker.pass_line(result))

    print(f"PASS portable checked evidence bundles={len(bundles)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run aggregate tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_all_portable_evidence -q
```

Expected: PASS.

- [ ] **Step 5: Run the issue command and verify final line**

Run:

```sh
python3 tools/check_all_portable_evidence.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected final line:

```text
PASS portable checked evidence bundles=4
```

- [ ] **Step 6: Commit Task 2**

Run:

```sh
git add tools/check_all_portable_evidence.py tools/test_check_all_portable_evidence.py
git commit -m "feat: add aggregate checked evidence gate"
```

### Task 3: Add Fair CLI Aggregate Negative Control

**Files:**
- Modify: `tools/test_check_all_portable_evidence.py`

**Interfaces:**
- Consumes: aggregate CLI from Task 2.
- Produces: an integration test that mutates a copied fair CLI bundle, rewrites bundle artifact hashes, updates catalog artifact digests, and asserts aggregate failure naming `fair-cli-release`.

- [ ] **Step 1: Add imports and hash helpers**

Append these imports to `tools/test_check_all_portable_evidence.py`:

```python
import hashlib
import json
import shutil
import tempfile
from typing import Any
```

Add these helpers above the test class:

```python
FAIR_CLI_ARTIFACTS = ("raw.jsonl", "summary.json", "report.md", "environment.json")
FIXTURE_REPO_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rewrite_fair_cli_hashes(bundle: Path) -> None:
    write_json(
        bundle / "artifact-sha256.json",
        {filename: sha256_file(bundle / filename) for filename in FAIR_CLI_ARTIFACTS},
    )


def rewrite_catalog_fair_artifact_hashes(catalog_path: Path, fair_bundle: Path) -> None:
    text = catalog_path.read_text(encoding="utf-8")
    catalog = importlib.import_module("benchmarks.rstim_vs_stim_simulator.portable_provenance").load_catalog(catalog_path)
    fair_entry = next(bundle for bundle in catalog["bundles"] if bundle["id"] == "fair-cli-release")
    for artifact in fair_entry["artifacts"]:
        old_digest = artifact["sha256"]
        new_digest = sha256_file(fair_bundle / artifact["path"])
        text = text.replace(f'sha256 = "{old_digest}"', f'sha256 = "{new_digest}"', 1)
    catalog_path.write_text(text, encoding="utf-8")
```

- [ ] **Step 2: Add the negative-control test**

Add this test to `AllPortableEvidenceCheckerTest`:

```python
    def test_fair_cli_rehashed_absolute_fixture_path_fails_with_bundle_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            temp_repo = Path(tmp) / "repo"
            shutil.copytree(REPO_ROOT / "benchmarks", temp_repo / "benchmarks")
            catalog = temp_repo / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
            fair_bundle = temp_repo / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release"
            absolute_fixture = str((REPO_ROOT / FIXTURE_REPO_PATH).resolve())

            records = [json.loads(line) for line in (fair_bundle / "raw.jsonl").read_text(encoding="utf-8").splitlines()]
            for record in records:
                argv = record["argv"]
                argv[argv.index("--in") + 1] = absolute_fixture
            (fair_bundle / "raw.jsonl").write_text(
                "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
                encoding="utf-8",
            )

            environment = json.loads((fair_bundle / "environment.json").read_text(encoding="utf-8"))
            for round_argv in environment["round_argv"]:
                argv = round_argv["argv"]
                argv[argv.index("--in") + 1] = absolute_fixture
            write_json(fair_bundle / "environment.json", environment)
            rewrite_fair_cli_hashes(fair_bundle)
            rewrite_catalog_fair_artifact_hashes(catalog, fair_bundle)

            result = self.run_aggregate("--catalog", str(catalog))

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("PASS portable evidence catalog bundles=4 schema=2", result.stdout)
        self.assertIn("FAIL portable checked evidence bundle=fair-cli-release", result.stderr)
        self.assertIn("stim-cli-b8 argv contains a host-absolute path", result.stderr)
```

- [ ] **Step 3: Run the aggregate tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_all_portable_evidence -q
```

Expected: PASS.

- [ ] **Step 4: Commit Task 3**

Run:

```sh
git add tools/test_check_all_portable_evidence.py
git commit -m "test: cover aggregate evidence failure names"
```

### Task 4: Add CI Portability Job

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: required GitHub Actions job `checked-evidence-portability`.
- Produces: no Stim install, no Rust install/cache/build, no `target/` directory.

- [ ] **Step 1: Add CI job**

Add this job to `.github/workflows/ci.yml` between `test` and `perf-gate`:

```yaml
  checked-evidence-portability:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Check portable evidence bundles
        run: |
          test ! -d target
          python3 tools/check_all_portable_evidence.py \
            --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
          test ! -d target
```

- [ ] **Step 2: Run issue verification**

Run:

```sh
python3 tools/check_all_portable_evidence.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest tools.test_check_all_portable_evidence -q
```

Expected:

```text
PASS portable checked evidence bundles=4
```

and unit tests PASS.

- [ ] **Step 3: Run repository-required verification**

Run:

```sh
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 4: Confirm no target directory is required by the aggregate command**

Run:

```sh
test ! -e target/release/rstim
```

Expected: PASS. If `target/` exists from local Cargo verification, this command still proves the checked evidence command does not require the release binary path.

- [ ] **Step 5: Commit Task 4**

Run:

```sh
git add .github/workflows/ci.yml
git commit -m "ci: gate portable checked evidence"
```

### Task 5: Final Quality Gate and PR

**Files:**
- Inspect: full branch diff
- Create: GitHub pull request

**Interfaces:**
- Produces: pushed worker branch.
- Produces: pull request against `master` for issue #484.

- [ ] **Step 1: Run final verification**

Run:

```sh
python3 tools/check_all_portable_evidence.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest tools.test_check_all_portable_evidence -q
cargo test --workspace
```

Expected: aggregate final line `PASS portable checked evidence bundles=4`, unittest PASS, Cargo PASS.

- [ ] **Step 2: Inspect git status and diff**

Run:

```sh
git status --short
git diff --stat origin/master HEAD
```

Expected: only issue #484 files changed.

- [ ] **Step 3: Push branch and open PR**

Run:

```sh
git push -u origin agent/issue-484-gate-all-checked-performance-evidence-in-a-clean-run-1
gh pr create --repo nzy1997/rstim --base master --head agent/issue-484-gate-all-checked-performance-evidence-in-a-clean-run-1 --title "Gate portable checked evidence in CI" --body "Closes #484"
```

Expected: PR URL printed.
