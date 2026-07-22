# Issue 521 rsmp Fixture Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one machine-checked `rsmp` verification catalog with the seven required semantic roles, four independent known-answer cases, and deterministic corruption recipes.

**Architecture:** Keep fixture truth in `rstim/tests/fixtures/rsmp/catalog.json` and validate it with a standard-library Python checker. The checker treats `valid_cases=7` as required semantic-role coverage, validates all committed file hashes and b8 shapes, and validates corruption recipe taxonomy without materializing archive bytes.

**Tech Stack:** JSON fixtures, Python 3 standard library (`argparse`, `hashlib`, `json`, `pathlib`, `subprocess`, `tempfile`, `unittest`), existing Rust workspace tests via `cargo test`.

## Global Constraints

- Catalog path is exactly `rstim/tests/fixtures/rsmp/catalog.json`.
- Checker path is exactly `tools/check_rsmp_fixture_catalog.py`.
- Unit test path is exactly `tools/test_check_rsmp_fixture_catalog.py`.
- Checker PASS line is exactly `PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12`.
- Required roles are exactly `nonzero_reference`, `rank_zero`, `dependent_detectors`, `repeat_records`, `observable_recovery`, `loss_visible_measurements`, and `surface_d11_r100`.
- Required known-answer case IDs are exactly `known_mpad_multi`, `known_mpp_multi_product`, `known_heralded_erase`, and `known_heralded_pauli_channel_1`.
- Benchmark case `surface_d11_r100` must reference `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim` and must not copy that fixture under `rstim/tests/fixtures/rsmp/`.
- Public error codes are the #520 taxonomy: `RSMP_BAD_MAGIC`, `RSMP_UNSUPPORTED_VERSION`, `RSMP_UNSUPPORTED_FEATURE`, `RSMP_UNSUPPORTED_SWEEP`, `RSMP_CIRCUIT_MISMATCH`, `RSMP_SHAPE_MISMATCH`, `RSMP_LIMIT_EXCEEDED`, `RSMP_TRUNCATED`, `RSMP_MALFORMED_ARCHIVE`, `RSMP_DECOMPRESSION_FAILED`, `RSMP_CHECKSUM_MISMATCH`, `RSMP_LOGICAL_DIGEST_MISMATCH`, `RSMP_TRAILING_DATA`, and `RSMP_IO`.
- Unknown required feature recipes must map to `RSMP_UNSUPPORTED_FEATURE`.
- Malformed fields, order, padding, canonical encoding, or unknown IDs must map to `RSMP_MALFORMED_ARCHIVE`.
- Zstandard frame, decode, or frame-checksum failure recipes must map to `RSMP_DECOMPRESSION_FAILED`.
- Canonical logical payload mismatch recipes must map to `RSMP_LOGICAL_DIGEST_MISMATCH`.
- Do not implement transforms, archive writers/readers, CLI archive commands, compression, or corruption materialization.

---

### Task 1: Add Failing Checker Tests

**Files:**
- Create: `tools/test_check_rsmp_fixture_catalog.py`

**Interfaces:**
- Consumes: future checker CLI `python3 tools/check_rsmp_fixture_catalog.py --repo-root <root> --catalog <catalog>`.
- Produces: unit tests that assert the required PASS line and required rejection diagnostics.

- [ ] **Step 1: Write the failing unittest module**

Create `tools/test_check_rsmp_fixture_catalog.py` with:

```python
#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rsmp_fixture_catalog.py"
CATALOG = REPO_ROOT / "rstim" / "tests" / "fixtures" / "rsmp" / "catalog.json"
PASS_LINE = "PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12"


class RsmpFixtureCatalogCheckerTest(unittest.TestCase):
    def run_checker(self, *, repo_root: Path = REPO_ROOT, catalog: Path = CATALOG) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--repo-root", str(repo_root), "--catalog", str(catalog)],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def load_catalog_copy(self) -> tuple[tempfile.TemporaryDirectory[str], Path, dict[str, object]]:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        catalog_copy = Path(tmpdir.name) / "catalog.json"
        catalog_data = json.loads(CATALOG.read_text(encoding="utf-8"))
        catalog_copy.write_text(json.dumps(catalog_data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return tmpdir, catalog_copy, catalog_data

    def write_catalog(self, path: Path, catalog_data: dict[str, object]) -> None:
        path.write_text(json.dumps(catalog_data, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def cases(self, catalog_data: dict[str, object]) -> list[dict[str, object]]:
        cases = catalog_data["cases"]
        assert isinstance(cases, list)
        return cases  # type: ignore[return-value]

    def recipes(self, catalog_data: dict[str, object]) -> list[dict[str, object]]:
        recipes = catalog_data["corruption_recipes"]
        assert isinstance(recipes, list)
        return recipes  # type: ignore[return-value]

    def find_case(self, catalog_data: dict[str, object], case_id: str) -> dict[str, object]:
        for case in self.cases(catalog_data):
            if case.get("id") == case_id:
                return case
        raise AssertionError(f"missing case {case_id}")

    def find_recipe(self, catalog_data: dict[str, object], recipe_id: str) -> dict[str, object]:
        for recipe in self.recipes(catalog_data):
            if recipe.get("id") == recipe_id:
                return recipe
        raise AssertionError(f"missing recipe {recipe_id}")

    def test_accepts_repository_catalog(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, PASS_LINE + "\n")

    def test_rejects_valid_case_with_incorrect_measurement_count(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["measurement_count"] = 4
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.measurement_count", result.stderr)

    def test_rejects_changed_committed_fixture_sha256(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["circuit_sha256"] = "0" * 64
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.circuit_sha256", result.stderr)

    def test_rejects_removed_required_semantic_role(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "rank_zero")
        case["semantic_roles"] = []
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing semantic role rank_zero", result.stderr)

    def test_rejects_corruption_recipe_without_expected_code(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "bad_magic")["expected_code"] = ""
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bad_magic.expected_code", result.stderr)

    def test_rejects_raw_byte_offset_recipe_selector(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "bad_magic")["mutation"] = "set(byte_offset:0, 0)"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bad_magic.mutation", result.stderr)

    def test_rejects_wrong_unknown_required_feature_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "unknown_required_feature")["expected_code"] = "RSMP_MALFORMED_ARCHIVE"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown_required_feature.expected_code must be RSMP_UNSUPPORTED_FEATURE", result.stderr)

    def test_rejects_benchmark_duplicate_fixture_path(self) -> None:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        temp_root = Path(tmpdir.name)
        shutil.copytree(REPO_ROOT / "rstim" / "tests" / "fixtures" / "rsmp", temp_root / "rstim" / "tests" / "fixtures" / "rsmp")
        benchmark_src = REPO_ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / "stim_surface_code_rotated_memory_z_d11_r100.stim"
        benchmark_dst = temp_root / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / benchmark_src.name
        benchmark_dst.parent.mkdir(parents=True)
        shutil.copy2(benchmark_src, benchmark_dst)
        duplicate = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "surface_d11_r100_duplicate.stim"
        shutil.copy2(benchmark_src, duplicate)
        catalog_path = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "catalog.json"
        catalog_data = json.loads(catalog_path.read_text(encoding="utf-8"))
        case = self.find_case(catalog_data, "surface_d11_r100")
        case["circuit_path"] = "rstim/tests/fixtures/rsmp/surface_d11_r100_duplicate.stim"
        case["circuit_sha256"] = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
        self.write_catalog(catalog_path, catalog_data)
        result = self.run_checker(repo_root=temp_root, catalog=catalog_path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("surface_d11_r100.circuit_path must reference existing benchmark fixture", result.stderr)

    def test_rejects_removed_required_known_answer(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        catalog_data["cases"] = [case for case in self.cases(catalog_data) if case.get("id") != "known_mpp_multi_product"]
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing known-answer case known_mpp_multi_product", result.stderr)

    def test_rejects_changed_known_answer_expected_sha256(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "known_heralded_erase")
        expected_files = case["expected_files"]
        assert isinstance(expected_files, dict)
        measurements = expected_files["measurements_b8"]
        assert isinstance(measurements, dict)
        measurements["sha256"] = "f" * 64
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_heralded_erase.expected_files.measurements_b8.sha256", result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Verify RED**

Run:

```bash
python3 -m unittest tools.test_check_rsmp_fixture_catalog
```

Expected: FAIL because `tools/check_rsmp_fixture_catalog.py` and/or `rstim/tests/fixtures/rsmp/catalog.json` do not exist yet.

- [ ] **Step 3: Commit**

```bash
git add tools/test_check_rsmp_fixture_catalog.py
git commit -m "test: require rsmp fixture catalog checker"
```

### Task 2: Add rsmp Fixture Files and Catalog

**Files:**
- Create: `rstim/tests/fixtures/rsmp/README.md`
- Create: `rstim/tests/fixtures/rsmp/catalog.json`
- Create: `rstim/tests/fixtures/rsmp/*.stim`
- Create: `rstim/tests/fixtures/rsmp/*.b8`

**Interfaces:**
- Consumes: test expectations from Task 1.
- Produces: catalog data and committed source/expected files for the checker to validate.

- [ ] **Step 1: Add the source circuits**

Create the four exact known-answer circuits plus small semantic circuits:

```text
known_mpad_multi.stim
known_mpp_multi_product.stim
known_heralded_erase.stim
known_heralded_pauli_channel_1.stim
nonzero_reference.stim
rank_zero.stim
dependent_detectors.stim
repeat_records.stim
observable_recovery.stim
loss_visible_measurements.stim
```

Use the exact circuits from issue #521 for the four known-answer files.

- [ ] **Step 2: Add binary expected b8 files**

Create the known-answer b8 files with the exact bytes from the issue:

```text
known_mpad_multi.measurements.b8        02 03 06 07
known_mpad_multi.detectors.b8           00 01 02 03
known_mpad_multi.observables.b8         00 01 01 00
known_mpp_multi_product.measurements.b8 00 03 05 06
known_mpp_multi_product.detectors.b8    00 03 05 06
known_mpp_multi_product.observables.b8  00 01 01 00
known_heralded_erase.measurements.b8    00 01 01 00
known_heralded_erase.detectors.b8       00 01 01 00
known_heralded_erase.observables.b8     00 01 01 00
known_heralded_pauli_channel_1.measurements.b8 00 01 00 01
known_heralded_pauli_channel_1.detectors.b8    00 01 00 01
known_heralded_pauli_channel_1.observables.b8  00 01 00 01
```

- [ ] **Step 3: Add the README**

Document the parity calculations:

```markdown
# rsmp Fixture Catalog

This directory contains the small committed fixtures for `rstim/tests/fixtures/rsmp/catalog.json`.
The four known-answer b8 vectors are independent oracles for later rsmp work.

## known_mpad_multi

Measurements per shot are `[m0, m1, m2]` from `MPAD 0 1 0`. Detector bits are
`d0 = m0 xor m1` and `d1 = m1 xor m2`; observable bit is `l0 = m0 xor m2`.
For measurement bytes `02 03 06 07`, the detector bytes are `00 01 02 03` and
the observable bytes are `00 01 01 00`.

## known_mpp_multi_product

Measurements per shot are `[m0, m1, m2]` from `MPP Z0*Z1 Z0 Z1`. Detector bits
copy the three measurement bits, and the observable bit is `m1 xor m2`. For
measurement bytes `00 03 05 06`, the detector bytes are identical and the
observable bytes are `00 01 01 00`.

## known_heralded_erase

`HERALDED_ERASE` produces one herald measurement. The detector and observable
both reference `rec[-1]`, so measurements, detectors, and observables are all
`00 01 01 00`.

## known_heralded_pauli_channel_1

`HERALDED_PAULI_CHANNEL_1` produces one herald measurement. The detector and
observable both reference `rec[-1]`, so measurements, detectors, and
observables are all `00 01 00 01`.

## Stim Cross-Check

The independent command family is pinned to Stim 1.15.0:

```console
python3 -c 'import stim; print(stim.__version__)'
stim m2d --circuit <case>.stim --in <case>.measurements.b8 --in_format b8 --out <case>.detectors.check.b8 --out_format b8 --obs_out <case>.observables.check.b8 --obs_out_format b8
```
```

- [ ] **Step 4: Add `catalog.json`**

Use a top-level object with `schema_version`, `format`, `cases`, and
`corruption_recipes`. Include at least 12 recipes; prefer 19 recipes to cover
every listed corruption family independently.

- [ ] **Step 5: Verify intermediate RED**

Run:

```bash
python3 -m unittest tools.test_check_rsmp_fixture_catalog
```

Expected: still FAIL because the checker is not implemented.

- [ ] **Step 6: Commit**

```bash
git add rstim/tests/fixtures/rsmp
git commit -m "test: add rsmp fixture catalog data"
```

### Task 3: Implement the Catalog Checker

**Files:**
- Create: `tools/check_rsmp_fixture_catalog.py`

**Interfaces:**
- Consumes: `catalog.json` from Task 2.
- Produces: CLI `main(argv: list[str] | None = None) -> int`.

- [ ] **Step 1: Implement checker helpers**

Implement these standard-library helpers with the described behavior:

```python
PASS_LINE = "PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12"
REQUIRED_ROLES = {
    "nonzero_reference",
    "rank_zero",
    "dependent_detectors",
    "repeat_records",
    "observable_recovery",
    "loss_visible_measurements",
    "surface_d11_r100",
}
REQUIRED_KNOWN_ANSWERS = {
    "known_mpad_multi",
    "known_mpp_multi_product",
    "known_heralded_erase",
    "known_heralded_pauli_channel_1",
}
EXPECTED_BENCHMARK_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
ERROR_CODES = {
    "RSMP_BAD_MAGIC",
    "RSMP_UNSUPPORTED_VERSION",
    "RSMP_UNSUPPORTED_FEATURE",
    "RSMP_UNSUPPORTED_SWEEP",
    "RSMP_CIRCUIT_MISMATCH",
    "RSMP_SHAPE_MISMATCH",
    "RSMP_LIMIT_EXCEEDED",
    "RSMP_TRUNCATED",
    "RSMP_MALFORMED_ARCHIVE",
    "RSMP_DECOMPRESSION_FAILED",
    "RSMP_CHECKSUM_MISMATCH",
    "RSMP_LOGICAL_DIGEST_MISMATCH",
    "RSMP_TRAILING_DATA",
    "RSMP_IO",
}

def sha256_file(path: Path) -> str:
    """Return the lowercase SHA-256 hex digest of a committed file."""


def repo_path(repo_root: Path, value: object, label: str) -> Path:
    """Return repo_root/value after rejecting absolute paths and '..' components."""


def b8_len(shots: int, bit_count: int) -> int:
    """Return the bytes for Stim-style byte-aligned b8 rows."""
    return shots * ((bit_count + 7) // 8)


def validate_b8(path: Path, shots: int, bit_count: int, label: str) -> None:
    """Validate byte length and zero unused final padding bits."""
```

`repo_path` rejects absolute paths and any `..` component.

- [ ] **Step 2: Implement case validation**

Validate duplicate IDs, required strings, consumer lists, source SHA-256,
shape integers, `0 <= rank_H <= min(M, D)`, committed measurement input lengths,
and known-answer expected file hashes and lengths. Use labels like
`known_mpad_multi.measurement_count` and
`known_heralded_erase.expected_files.measurements_b8.sha256` in diagnostics.

- [ ] **Step 3: Implement recipe validation**

Validate duplicate recipe IDs, non-empty source roles, mutation strings,
expected error codes, recomputation lists, and validation boundaries. Reject
raw byte offset selectors by matching `byte_offset`, `offset(`, `@`, or
`[number]` patterns in mutation strings and recomputation selectors. Enforce
the normalized recipe mapping rules from the Global Constraints. For decoded
payload mutations, require recomputation metadata to include the affected
Zstandard frame checksum plus enclosing compressed-length and archive-digest
fields needed to reach the catalogued validation boundary.

- [ ] **Step 4: Implement CLI entrypoint**

Default to `Path(__file__).resolve().parents[1]` as the repository root and
`rstim/tests/fixtures/rsmp/catalog.json` as the catalog path. Print diagnostics
to stderr and return 1 on `ValueError`. Print the exact PASS line and return 0
on success.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
python3 tools/check_rsmp_fixture_catalog.py
python3 -m unittest tools.test_check_rsmp_fixture_catalog
```

Expected: checker prints the exact PASS line; unittest reports `OK`.

- [ ] **Step 6: Commit**

```bash
git add tools/check_rsmp_fixture_catalog.py tools/test_check_rsmp_fixture_catalog.py rstim/tests/fixtures/rsmp
git commit -m "feat: check rsmp fixture catalog"
```

### Task 4: Final Verification and Review

**Files:**
- Modify only if verification or review finds issues.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Run issue verification**

```bash
python3 tools/check_rsmp_fixture_catalog.py
python3 -m unittest tools.test_check_rsmp_fixture_catalog
```

Expected: both commands exit 0; the checker prints the exact PASS line and
unittest reports `OK`.

- [ ] **Step 2: Run Rust workspace verification**

```bash
cargo test
```

Expected: exits 0. Existing warnings are acceptable if present in the baseline.

- [ ] **Step 3: Run final review**

Use `superpowers:requesting-code-review` with the merge base against `master`
and the current `HEAD`.

- [ ] **Step 4: Fix review findings if needed**

Fix any Critical or Important findings and rerun the covering verification.

- [ ] **Step 5: Finish branch**

Use `superpowers:finishing-a-development-branch`, choose `Push and create a Pull Request`, push
`agent/issue-521-build-the-shared-rsmp-verification-catalog-run-1`, and create a PR targeting `master` with `Closes #521`.
