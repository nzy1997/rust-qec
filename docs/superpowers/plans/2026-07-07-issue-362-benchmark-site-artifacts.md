# Issue 362 Benchmark Site Artifact Publishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `make build-site` publish the benchmark site manifest and checked benchmark artifacts into stable `_site/` paths while preserving QP101 resources.

**Architecture:** Keep the site build as Makefile plus Python. Extend `tools/check_site_manifest.py` with shared checked-artifact iteration and optional copied-site validation, add `tools/copy_site_benchmark_data.py` as the build helper, and call the helper from `make build-site` after the existing QP101 gallery build.

**Tech Stack:** GNU Make-compatible Makefile, Python standard library (`argparse`, `json`, `pathlib`, `shutil`, `subprocess`, `tempfile`, `unittest`), git CLI, existing checked benchmark artifacts.

## Global Constraints

- Preserve existing QP101 outputs from `make build-site`: `_site/qp101.schema.json`, `_site/QP101-ZY.md`, `_site/examples/*`, and `_site/gallery/*`.
- Copy `site/benchmark-site.json` to `_site/data/benchmark-site.json`.
- Copy checked artifact paths listed in the manifest to `_site/<artifact path>`, preserving repository-relative paths under `_site/`.
- Validate the source manifest before copying.
- Validate the copied site paths after copying.
- Reuse `tools/check_site_manifest.py` rules for source validation and copied-site validation.
- Do not copy ignored local-only benchmark outputs as checked evidence.
- A checked artifact path under `benchmarks/out/` must be rejected before a broken site is accepted.
- A missing checked artifact path must be rejected before a broken site is accepted.
- Do not redesign the page layout.
- Do not generate fresh benchmark results.
- Keep dependencies to Makefile and Python standard library.
- Required verification:
  - `make build-site`
  - `python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json`
  - `python3 -m unittest tools.test_check_site_manifest -q`
  - `python3 tools/check_site_manifest.py --self-test`
  - `cargo test`

---

## File Structure

- Modify `tools/check_site_manifest.py`: shared checked-artifact iteration, optional copied-site validation, `--site-root` CLI support.
- Create `tools/copy_site_benchmark_data.py`: validates source manifest, copies manifest and checked artifacts, validates copied site paths.
- Modify `tools/test_check_site_manifest.py`: focused TDD coverage for copied-site validation and the copy helper.
- Modify `Makefile`: invoke the copy helper from `build-site` without changing existing QP101 copy commands.

### Task 1: Add Failing Tests For Site Artifact Publishing

**Files:**
- Modify: `tools/test_check_site_manifest.py`

**Interfaces:**
- Consumes: `check_site_manifest.validate_manifest(repo_root: Path, manifest_path: Path, site_root: Path | None = None) -> list[str]`
- Consumes: `copy_site_benchmark_data.copy_benchmark_site_data(repo_root: Path, manifest_path: Path, site_root: Path) -> list[str]`
- Produces: tests proving copied-site validation rejects missing copied artifacts and the helper copies only checked manifest artifacts.

- [ ] **Step 1: Write the failing tests**

Add this import near the existing `tools.check_site_manifest` import:

```python
import tools.copy_site_benchmark_data as copy_site_benchmark_data
```

Extend `write_fixture_manifest` so the fixture includes a second checked artifact and a local ignored output file:

```python
(root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").write_text("png\n", encoding="utf-8")
(root / "benchmarks/out/local-only.csv").write_text("local\n", encoding="utf-8")
manifest["families"][0]["evidence_items"][0]["artifacts"].append(
    {
        "path": "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
        "kind": "image",
        "checked": True,
    }
)
```

Add the new checked PNG to the fixture `git add` list:

```python
"benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
```

Add this test method:

```python
def test_site_root_validation_rejects_missing_copied_checked_artifact(self) -> None:
    repo, manifest_path = self.write_fixture_manifest()
    site_root = repo / "_site"
    (site_root / "data").mkdir(parents=True)
    site_manifest = site_root / "data/benchmark-site.json"
    site_manifest.write_text(manifest_path.read_text(encoding="utf-8"), encoding="utf-8")

    errors = check_site_manifest.validate_manifest(repo, site_manifest, site_root=site_root)

    self.assertTrue(
        any(
            "surface-decoder-full" in error
            and "benchmarks/surface_decoder_compare/results/full/results.csv" in error
            and "not copied" in error
            for error in errors
        ),
        errors,
    )
```

Add this test method:

```python
def test_copy_helper_copies_manifest_and_checked_artifacts_only(self) -> None:
    repo, manifest_path = self.write_fixture_manifest()
    site_root = repo / "_site"

    errors = copy_site_benchmark_data.copy_benchmark_site_data(repo, manifest_path, site_root)

    self.assertEqual(errors, [])
    self.assertTrue((site_root / "data/benchmark-site.json").is_file())
    self.assertTrue((site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").is_file())
    self.assertTrue((site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").is_file())
    self.assertFalse((site_root / "benchmarks/out/local-only.csv").exists())
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: FAIL because `tools.copy_site_benchmark_data` does not exist and `validate_manifest` does not yet accept `site_root`.

- [ ] **Step 3: Commit the failing tests**

Run:

```sh
git add tools/test_check_site_manifest.py
git commit -m "test: require benchmark site artifact publishing"
```

Expected: commit succeeds with only the test update staged.

### Task 2: Implement Site Artifact Publishing

**Files:**
- Modify: `tools/check_site_manifest.py`
- Create: `tools/copy_site_benchmark_data.py`
- Modify: `Makefile`

**Interfaces:**
- Consumes: source manifest `site/benchmark-site.json`
- Consumes: copied manifest `_site/data/benchmark-site.json`
- Produces: `check_site_manifest.validate_manifest(..., site_root=Path("_site"))`
- Produces: `copy_site_benchmark_data.copy_benchmark_site_data(repo_root, manifest_path, site_root) -> list[str]`
- Produces: `python3 tools/copy_site_benchmark_data.py --repo-root . --site-root _site site/benchmark-site.json`

- [ ] **Step 1: Extend the manifest checker**

In `tools/check_site_manifest.py`, add `site_root` support without changing existing call sites:

```python
def iter_checked_artifact_paths(manifest: dict[str, Any]) -> list[tuple[str, str]]:
    paths: list[tuple[str, str]] = []
    for family in manifest.get("families", []):
        if not isinstance(family, dict):
            continue
        for item in family.get("evidence_items", []):
            if not isinstance(item, dict):
                continue
            item_id = item.get("id", "<missing>")
            for artifact in item.get("artifacts", []):
                if not isinstance(artifact, dict):
                    continue
                if artifact.get("checked") is True and isinstance(artifact.get("path"), str):
                    paths.append((item_id, artifact["path"]))
    return paths
```

Add copied-site validation:

```python
def validate_site_paths(site_root: Path, manifest: dict[str, Any], errors: list[str]) -> None:
    for item_id, artifact_path in iter_checked_artifact_paths(manifest):
        copied = site_root / artifact_path
        if not copied.is_file():
            add_error(errors, f"evidence item {item_id}", f"checked artifact {artifact_path} was not copied to {site_root}")
```

Change the validator signature and call `validate_site_paths` after normal validation:

```python
def validate_manifest(repo_root: Path, manifest_path: Path, site_root: Path | None = None) -> list[str]:
    ...
    if site_root is not None and not errors:
        validate_site_paths(site_root, manifest, errors)
    return errors
```

Extend `parse_args()`:

```python
parser.add_argument("--site-root", type=Path, help="Validate checked artifact copies under this built site root")
```

Pass the value from `main()`:

```python
errors = validate_manifest(args.repo_root, args.manifest, site_root=args.site_root)
```

- [ ] **Step 2: Add the copy helper**

Create `tools/copy_site_benchmark_data.py`:

```python
#!/usr/bin/env python3
"""Copy benchmark site manifest data and checked artifacts into _site."""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

try:
    from tools import check_site_manifest
except ModuleNotFoundError:
    import check_site_manifest  # type: ignore[no-redef]


def copy_benchmark_site_data(repo_root: Path, manifest_path: Path, site_root: Path) -> list[str]:
    errors = check_site_manifest.validate_manifest(repo_root, manifest_path)
    if errors:
        return errors

    manifest = check_site_manifest.load_json(manifest_path)
    data_dir = site_root / "data"
    data_dir.mkdir(parents=True, exist_ok=True)
    site_manifest = data_dir / "benchmark-site.json"
    shutil.copy2(manifest_path, site_manifest)

    for _, artifact_path in check_site_manifest.iter_checked_artifact_paths(manifest):
        source = repo_root / artifact_path
        destination = site_root / artifact_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    return check_site_manifest.validate_manifest(repo_root, site_manifest, site_root=site_root)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Copy benchmark manifest and checked artifacts into _site.")
    parser.add_argument("--repo-root", type=Path, default=Path("."), help="Repository root for git checks")
    parser.add_argument("--site-root", type=Path, required=True, help="Built site root, usually _site")
    parser.add_argument("manifest", type=Path, help="Source site/benchmark-site.json")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    errors = copy_benchmark_site_data(args.repo_root, args.manifest, args.site_root)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"ok: copied benchmark site data to {args.site_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: Hook the helper into the Makefile**

Add the copy helper to the end of the `build-site` target, after the gallery
build:

```make
	python3 tools/copy_site_benchmark_data.py --repo-root . --site-root _site site/benchmark-site.json
```

- [ ] **Step 4: Run focused tests**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
```

Expected: both commands exit 0.

- [ ] **Step 5: Run issue site verification**

Run:

```sh
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
```

Expected: both commands exit 0. The following files exist:

```text
_site/data/benchmark-site.json
_site/qp101.schema.json
_site/QP101-ZY.md
_site/benchmarks/surface_decoder_compare/results/full/results.csv
_site/benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png
_site/benchmarks/bb_circuit_bposd_compare/results/full/results.csv
_site/benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png
_site/benchmarks/bb_circuit_bposd_compare/results/full/summary.md
_site/benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
```

Also verify no ignored local-only benchmark output is copied:

```sh
test ! -e _site/benchmarks/out
```

- [ ] **Step 6: Commit the implementation**

Run:

```sh
git add Makefile tools/check_site_manifest.py tools/copy_site_benchmark_data.py
git commit -m "fix: publish checked benchmark site artifacts"
```

Expected: commit succeeds with implementation files staged.
