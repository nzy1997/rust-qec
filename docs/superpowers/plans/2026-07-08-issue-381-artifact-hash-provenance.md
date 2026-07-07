# Issue 381 Artifact Hash Provenance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate checked benchmark artifact SHA-256 hashes from canonical provenance against repository files and copied built-site files.

**Architecture:** Extend the existing manifest checker rather than adding a second validator. Keep checked-artifact traversal centralized by exposing item context from the existing checked-artifact helper, then use that context for provenance hash validation and site-copy hash verification.

**Tech Stack:** Python standard library (`json`, `hashlib`, `pathlib`, `unittest`), existing static-site manifest JSON, existing Makefile site build.

## Global Constraints

- Scope validation to checked artifact paths from `site/benchmark-site.json`.
- `provenance.artifact_hashes` for checked evidence must be `{ "status": "recorded", "value": { "<artifact_path>": { "sha256": "<64 lowercase hex digest>" } } }`.
- During normal `tools/check_site_manifest.py` validation, compute SHA-256 from `repo_root / artifact_path` and compare it to the recorded digest.
- When `--site-root` is supplied, also compute SHA-256 from `site_root / artifact_path` and compare it to the recorded digest.
- Errors for bad or missing hash data must name the evidence item, artifact path, and `sha256` when a digest is involved.
- Reuse the checked-artifact iteration helper instead of duplicating manifest traversal.
- Do not change benchmark artifact contents or generate fresh benchmark outputs.
- Do not require local-only or future benchmark entries to provide checked artifact hashes before promotion.

---

### Task 1: Tests For Checked Artifact Hash Validation

**Files:**
- Modify: `tools/test_check_site_manifest.py:13-240`
- Test: `tools/test_check_site_manifest.py`

**Interfaces:**
- Consumes: existing `check_site_manifest.validate_manifest(repo_root, manifest_path, site_root=None)`.
- Produces: failing tests that require recorded checked-artifact SHA-256 metadata, repository digest comparison, malformed shape rejection, missing hash entry rejection, and copied site digest comparison.

- [ ] **Step 1: Add fixture constants and recorded fixture hashes**

Insert these constants after `PROVENANCE_NOT_RECORDED_REASON`:

```python
SURFACE_RESULTS_PATH = "benchmarks/surface_decoder_compare/results/full/results.csv"
SURFACE_IMAGE_PATH = "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png"
SURFACE_RESULTS_SHA256 = "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
SURFACE_IMAGE_SHA256 = "33d8344a7135c42aa3876706b908f95b702d83ff53e05e4aaff17c07bf67a98e"

FIXTURE_ARTIFACT_HASHES = {
    SURFACE_RESULTS_PATH: {"sha256": SURFACE_RESULTS_SHA256},
    SURFACE_IMAGE_PATH: {"sha256": SURFACE_IMAGE_SHA256},
}
```

Replace `fixture_provenance()` with:

```python
def fixture_provenance(commands: list[str]) -> dict[str, object]:
    return {
        "schema_version": 1,
        "artifact_date": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "source_commit": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "commands": {"status": "recorded", "value": commands},
        "os": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "cpu_model": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "rust_version": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "python_version": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "dependency_versions": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "external_repository_commits": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "seed_policy": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "build_profile": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "shots_or_error_budget": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
        "artifact_hashes": {"status": "recorded", "value": FIXTURE_ARTIFACT_HASHES},
    }
```

- [ ] **Step 2: Use the path constants in the surface artifact fixture**

Replace the two literal checked surface artifact paths in `VALID_MANIFEST` with `SURFACE_RESULTS_PATH` and `SURFACE_IMAGE_PATH`.

- [ ] **Step 3: Add mutation branches for hash negative controls**

In `write_fixture_manifest()`, after the existing provenance schema-version mutations, add:

```python
        elif mutation == "bad_artifact_hash":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "sha256"
            ] = "0" * 64
        elif mutation == "missing_artifact_hash":
            del manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH]
        elif mutation == "artifact_hashes_not_recorded":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"] = {
                "status": "not_recorded",
                "reason": PROVENANCE_NOT_RECORDED_REASON,
            }
        elif mutation == "artifact_hash_entry_not_object":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                SURFACE_RESULTS_PATH
            ] = SURFACE_RESULTS_SHA256
        elif mutation == "artifact_hash_missing_sha256":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                SURFACE_RESULTS_PATH
            ] = {}
        elif mutation == "artifact_hash_sha256_not_string":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "sha256"
            ] = 42
        elif mutation == "artifact_hash_sha256_invalid_hex":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][SURFACE_RESULTS_PATH][
                "sha256"
            ] = "g" * 64
        elif mutation == "artifact_hash_extra_algorithm":
            manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                SURFACE_RESULTS_PATH
            ]["md5"] = "unsupported"
```

- [ ] **Step 4: Add a helper to copy checked artifacts into the fixture site root**

Inside `SiteManifestValidatorTest`, before the first test method, add:

```python
    def copy_checked_artifacts_to_site(self, repo: Path, site_root: Path) -> None:
        for artifact_path in (SURFACE_RESULTS_PATH, SURFACE_IMAGE_PATH):
            source = repo / artifact_path
            destination = site_root / artifact_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source.read_bytes())
```

- [ ] **Step 5: Add failing unit coverage for digest mismatch and missing hash entries**

Add these tests after `test_rejects_unsupported_provenance_schema_version`:

```python
    def test_rejects_checked_artifact_hash_digest_mismatch(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="bad_artifact_hash")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "results.csv" in error
                and "sha256" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_checked_artifact_missing_hash_entry(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_artifact_hash")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and SURFACE_RESULTS_PATH in error
                for error in errors
            ),
            errors,
        )
```

- [ ] **Step 6: Add failing unit coverage for malformed hash shapes**

Add this test after the missing-hash-entry test:

```python
    def test_rejects_unsupported_checked_artifact_hash_shapes(self) -> None:
        for mutation, rule in [
            ("artifact_hashes_not_recorded", "recorded"),
            ("artifact_hash_entry_not_object", "object"),
            ("artifact_hash_missing_sha256", "sha256"),
            ("artifact_hash_sha256_not_string", "sha256"),
            ("artifact_hash_sha256_invalid_hex", "sha256"),
            ("artifact_hash_extra_algorithm", "unsupported"),
        ]:
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(
                any(
                    "surface-decoder-full" in error
                    and "results.csv" in error
                    and rule in error
                    for error in errors
                ),
                (mutation, errors),
            )
```

- [ ] **Step 7: Add failing site-root copied artifact digest coverage**

Add this test after `test_site_root_validation_rejects_missing_copied_checked_artifact`:

```python
    def test_site_root_validation_rejects_copied_checked_artifact_hash_mismatch(self) -> None:
        repo, _, built_manifest_path = self.write_fixture_manifest()
        site_root = repo / "_site"
        self.copy_checked_artifacts_to_site(repo, site_root)
        (site_root / SURFACE_RESULTS_PATH).write_text("mutated copied artifact\n", encoding="utf-8")

        errors = check_site_manifest.validate_manifest(repo, built_manifest_path, site_root=site_root)

        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "results.csv" in error
                and "sha256" in error
                for error in errors
            ),
            errors,
        )
```

- [ ] **Step 8: Run the focused unit tests and verify RED**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: FAIL. The new tests should fail because production validation still accepts `not_recorded` hash metadata, does not compare repository digests, and does not compare copied `_site` digests.

- [ ] **Step 9: Commit the failing tests**

Do not commit this task until Step 8 shows the expected RED failure.

```bash
git add tools/test_check_site_manifest.py
git commit -m "test: cover checked artifact hash provenance"
```

### Task 2: Implement Repository And Site Hash Validation

**Files:**
- Modify: `tools/check_site_manifest.py:6-202`
- Modify: `tools/test_check_site_manifest.py` only if a test assertion needs a message adjustment for the final wording
- Test: `tools/test_check_site_manifest.py`

**Interfaces:**
- Consumes: tests from Task 1.
- Produces:
  - `sha256_file(path: Path) -> str`
  - `iter_checked_artifacts(manifest: dict[str, Any]) -> list[tuple[dict[str, Any], str, str]]`
  - existing `iter_checked_artifact_paths()` remains available for copy helper callers.
  - `validate_manifest(..., site_root=...)` rejects stale copied checked artifacts.

- [ ] **Step 1: Add `hashlib` and a SHA-256 digest pattern**

Modify imports and constants:

```python
import argparse
import hashlib
import json
```

Add after `CHECKED_ARTIFACT_REFERENCE_RE`:

```python
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
```

- [ ] **Step 2: Add the file hashing helper**

Add after `load_json()`:

```python
def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()
```

- [ ] **Step 3: Replace checked-artifact iteration with an item-aware helper**

Replace the existing `iter_checked_artifact_paths()` implementation with:

```python
def iter_checked_artifacts(manifest: dict[str, Any]) -> list[tuple[dict[str, Any], str, str]]:
    artifacts: list[tuple[dict[str, Any], str, str]] = []
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
                    artifacts.append((item, item_id, artifact["path"]))
    return artifacts


def iter_checked_artifact_paths(manifest: dict[str, Any]) -> list[tuple[str, str]]:
    return [(item_id, artifact_path) for _, item_id, artifact_path in iter_checked_artifacts(manifest)]
```

- [ ] **Step 4: Add per-item checked artifact path and recorded digest helpers**

Add after `item_has_checked_artifacts()`:

```python
def checked_artifact_paths_for_item(item: dict[str, Any]) -> list[str]:
    artifacts = item.get("artifacts")
    if not isinstance(artifacts, list):
        return []
    return [
        artifact["path"]
        for artifact in artifacts
        if isinstance(artifact, dict) and artifact.get("checked") is True and isinstance(artifact.get("path"), str)
    ]


def recorded_artifact_sha256(item: dict[str, Any], artifact_path: str) -> str | None:
    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        return None
    artifact_hashes = provenance.get("artifact_hashes")
    if not isinstance(artifact_hashes, dict) or artifact_hashes.get("status") != "recorded":
        return None
    value = artifact_hashes.get("value")
    if not isinstance(value, dict):
        return None
    entry = value.get(artifact_path)
    if not isinstance(entry, dict):
        return None
    digest = entry.get("sha256")
    return digest if isinstance(digest, str) else None
```

- [ ] **Step 5: Add checked artifact hash validation**

Add after `validate_provenance_status_field()`:

```python
def validate_checked_artifact_hashes(repo_root: Path, scope: str, item: dict[str, Any], provenance: dict[str, Any], errors: list[str]) -> None:
    artifact_paths = checked_artifact_paths_for_item(item)
    artifact_hashes = provenance.get("artifact_hashes")
    if not isinstance(artifact_hashes, dict):
        return
    if artifact_hashes.get("status") != "recorded":
        add_error(errors, scope, "provenance.artifact_hashes must be recorded for checked artifacts")
        return

    value = artifact_hashes.get("value")
    if not isinstance(value, dict):
        add_error(errors, scope, "provenance.artifact_hashes recorded entry value must be an object")
        return

    for artifact_path in artifact_paths:
        entry = value.get(artifact_path)
        if entry is None:
            add_error(errors, scope, f"provenance.artifact_hashes missing hash entry for {artifact_path}")
            continue
        if not isinstance(entry, dict):
            add_error(errors, scope, f"provenance.artifact_hashes entry for {artifact_path} must be an object with sha256")
            continue
        if set(entry) != {"sha256"}:
            add_error(errors, scope, f"provenance.artifact_hashes entry for {artifact_path} has unsupported hash shape; only sha256 is supported")
            continue

        recorded_digest = entry.get("sha256")
        if not isinstance(recorded_digest, str) or SHA256_RE.fullmatch(recorded_digest) is None:
            add_error(errors, scope, f"provenance.artifact_hashes entry for {artifact_path} must include sha256 as 64 lowercase hex characters")
            continue

        repo_file = repo_root / artifact_path
        if repo_file.is_file() and sha256_file(repo_file) != recorded_digest:
            add_error(errors, scope, f"artifact {artifact_path} sha256 digest does not match repository file")
```

- [ ] **Step 6: Call hash validation from checked provenance validation**

Change the signature:

```python
def validate_checked_item_provenance(repo_root: Path, scope: str, item: dict[str, Any], errors: list[str]) -> None:
```

After the existing `for field in PROVENANCE_REQUIRED_FIELDS` loop, add:

```python
    validate_checked_artifact_hashes(repo_root, scope, item, provenance, errors)
```

Change the call in `validate_item()` to:

```python
    validate_checked_item_provenance(repo_root, scope, item, errors)
```

- [ ] **Step 7: Verify copied site artifacts against recorded hashes**

Replace `validate_site_paths()` with:

```python
def validate_site_paths(repo_root: Path, site_root: Path, manifest: dict[str, Any], errors: list[str]) -> None:
    del repo_root
    for item, item_id, artifact_path in iter_checked_artifacts(manifest):
        copied = site_root / artifact_path
        scope = f"evidence item {item_id}"
        if not copied.is_file():
            add_error(
                errors,
                scope,
                f"checked artifact {artifact_path} was not copied to {site_root}",
            )
            continue

        recorded_digest = recorded_artifact_sha256(item, artifact_path)
        if recorded_digest is None:
            add_error(errors, scope, f"checked artifact {artifact_path} is missing recorded sha256 for site validation")
            continue
        if sha256_file(copied) != recorded_digest:
            add_error(errors, scope, f"copied artifact {artifact_path} sha256 digest does not match recorded hash")
```

Update the `validate_manifest()` site-root call to:

```python
    if site_root is not None and not errors:
        validate_site_paths(repo_root, site_root, manifest, errors)
```

- [ ] **Step 8: Update the self-test fixture provenance hashes**

In `make_fixture_repo()` inside `tools/check_site_manifest.py`, change `fixture_provenance()` so its `artifact_hashes` field is recorded:

```python
            "artifact_hashes": {
                "status": "recorded",
                "value": {
                    "benchmarks/surface_decoder_compare/results/full/results.csv": {
                        "sha256": "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
                    }
                },
            },
```

- [ ] **Step 9: Extend self-test hash mutations**

Add these entries to `run_self_test()` mutations:

```python
            ("bad_artifact_hash", "surface-decoder-full", "sha256"),
            ("missing_artifact_hash", "surface-decoder-full", "results.csv"),
```

Add matching mutation branches:

```python
            elif mutation == "bad_artifact_hash":
                manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                    "benchmarks/surface_decoder_compare/results/full/results.csv"
                ]["sha256"] = "0" * 64
            elif mutation == "missing_artifact_hash":
                del manifest["families"][0]["evidence_items"][0]["provenance"]["artifact_hashes"]["value"][
                    "benchmarks/surface_decoder_compare/results/full/results.csv"
                ]
```

- [ ] **Step 10: Run the focused tests and verify GREEN**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
```

Expected: both commands exit 0.

- [ ] **Step 11: Commit the validator implementation**

```bash
git add tools/check_site_manifest.py tools/test_check_site_manifest.py
git commit -m "feat: verify checked artifact hashes"
```

### Task 3: Record Real Manifest Hashes And Run Required Verification

**Files:**
- Modify: `site/benchmark-site.json:35-242`
- Test: `site/benchmark-site.json`, built `_site/data/benchmark-site.json`

**Interfaces:**
- Consumes: `tools/check_site_manifest.py` hash validation from Task 2.
- Produces: recorded SHA-256 provenance for all checked artifacts in the committed benchmark manifest.

- [ ] **Step 1: Update `surface-decoder-full` artifact hashes**

Replace the `surface-decoder-full` `provenance.artifact_hashes` object with:

```json
"artifact_hashes": {
  "status": "recorded",
  "value": {
    "benchmarks/surface_decoder_compare/results/full/results.csv": {
      "sha256": "e74ff135e130fc127f7dbfd41d2d431cc800bdb3ab0f038ce512bcf9ab06ccc9"
    },
    "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png": {
      "sha256": "ce597cd54265ca2116632e864a45f6aad2fc66f021915361a0b9ff8093817cec"
    }
  }
}
```

- [ ] **Step 2: Update `bb-circuit-full` artifact hashes**

Replace the `bb-circuit-full` `provenance.artifact_hashes` object with:

```json
"artifact_hashes": {
  "status": "recorded",
  "value": {
    "benchmarks/bb_circuit_bposd_compare/results/full/results.csv": {
      "sha256": "523ff316fa20ee21fca7f0b6ad38daec30898428d15e0385a98aa03c73530e25"
    },
    "benchmarks/bb_circuit_bposd_compare/results/full/summary.md": {
      "sha256": "e6d17c46cc5e5de99aa6e2d827094de954188cbd25a69513533c9821740c09ab"
    },
    "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png": {
      "sha256": "a7ccf8cac066c6bd22934028a999ffd68e74ef038060a3fda6be04e5c6e605b3"
    },
    "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md": {
      "sha256": "9afdea2f0bb45143bd6851bd80c4066735097891ceccf267d859911c645fc274"
    }
  }
}
```

- [ ] **Step 3: Run required verification commands**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cargo test
```

Expected: all commands exit 0.

- [ ] **Step 4: Run issue negative controls**

Use temporary files only. Run:

```bash
tmp_manifest="$(mktemp /tmp/bad-benchmark-site.XXXXXX.json)"
cp site/benchmark-site.json "$tmp_manifest"
python3 - "$tmp_manifest" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
manifest = json.loads(path.read_text(encoding="utf-8"))
item = manifest["families"][0]["evidence_items"][0]
item["provenance"]["artifact_hashes"]["value"]["benchmarks/surface_decoder_compare/results/full/results.csv"][
    "sha256"
] = "0" * 64
path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
python3 tools/check_site_manifest.py --repo-root . "$tmp_manifest"
```

Expected: nonzero exit naming `surface-decoder-full`, `results.csv`, and `sha256`.

Then run:

```bash
python3 - "$tmp_manifest" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
manifest = json.loads(Path("site/benchmark-site.json").read_text(encoding="utf-8"))
item = manifest["families"][0]["evidence_items"][0]
del item["provenance"]["artifact_hashes"]["value"]["benchmarks/surface_decoder_compare/results/full/results.csv"]
path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
python3 tools/check_site_manifest.py --repo-root . "$tmp_manifest"
```

Expected: nonzero exit naming `surface-decoder-full` and `benchmarks/surface_decoder_compare/results/full/results.csv`.

Then run:

```bash
cp _site/benchmarks/surface_decoder_compare/results/full/results.csv /tmp/rstim-site-results.csv.backup
printf '\nmutated copied artifact\n' >> _site/benchmarks/surface_decoder_compare/results/full/results.csv
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cp /tmp/rstim-site-results.csv.backup _site/benchmarks/surface_decoder_compare/results/full/results.csv
```

Expected: checker command exits nonzero and names `surface-decoder-full`, `results.csv`, and `sha256`; the final `cp` restores the generated site artifact.

- [ ] **Step 5: Commit the manifest update**

```bash
git add site/benchmark-site.json
git commit -m "data: record checked artifact hashes"
```

## Self-Review

- Spec coverage: Tasks 1 and 2 cover missing entries, malformed shapes, repository mismatch, and copied site mismatch; Task 3 records all real checked artifact digests and runs issue verification.
- Placeholder scan: no placeholder markers or deferred implementation steps.
- Type consistency: helper names and signatures match across tasks (`iter_checked_artifacts`, `iter_checked_artifact_paths`, `recorded_artifact_sha256`, and `validate_manifest(..., site_root=...)`).
