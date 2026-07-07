# Issue 380 Canonical Provenance Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Require checked benchmark evidence items in `site/benchmark-site.json` to carry canonical provenance metadata and reject malformed provenance in the manifest checker.

**Architecture:** Extend the existing Python manifest validator with a small provenance schema checker that runs only for evidence items containing `checked: true` artifacts. Keep the canonical provenance data in `site/benchmark-site.json`; keep `provenance_requirements` and `provenance_sources` as descriptive legacy/methodology fields that do not satisfy the canonical schema.

**Tech Stack:** JSON manifest data, Python standard-library validation and `unittest`, Rust repository verification through `cargo test`.

## Global Constraints

- Only modify `site/benchmark-site.json`, `tools/check_site_manifest.py`, `tools/test_check_site_manifest.py`, and Superpowers documentation artifacts for this issue.
- Require canonical provenance only for evidence items whose `artifacts` list contains at least one object with `"checked": true`.
- Required provenance keys: `schema_version`, `artifact_date`, `source_commit`, `commands`, `os`, `cpu_model`, `rust_version`, `python_version`, `dependency_versions`, `external_repository_commits`, `seed_policy`, `build_profile`, `shots_or_error_budget`, and `artifact_hashes`.
- `provenance.schema_version` must be the number `1`.
- Every non-`schema_version` provenance field must be either `{ "status": "recorded", "value": ... }` or `{ "status": "not_recorded", "reason": "..." }`.
- `not_recorded.reason` must be a non-empty string.
- `recorded` entries must include the `value` key.
- Do not let `provenance_requirements` or `provenance_sources` satisfy canonical provenance validation.
- Do not run benchmark campaigns.
- Do not change benchmark artifact contents.
- Do not implement hash-content verification.
- Unit tests must cover missing whole `provenance`, missing `provenance.cpu_model`, `provenance.cpu_model` as `{ "status": "not_recorded" }` without reason, and unsupported or wrong-type `provenance.schema_version`.

---

### Task 1: Canonical Provenance Data And Validator

**Files:**
- Modify: `tools/test_check_site_manifest.py`
- Modify: `tools/check_site_manifest.py`
- Modify: `site/benchmark-site.json`
- Modify: `docs/superpowers/plans/2026-07-08-issue-380-canonical-provenance-validation.md`

**Interfaces:**
- Consumes: `check_site_manifest.validate_manifest(repo_root: Path, manifest_path: Path, site_root: Path | None = None) -> list[str]`
- Consumes: evidence item fields `id`, `artifacts`, `provenance_requirements`, and `provenance_sources`.
- Produces: module constant `PROVENANCE_SCHEMA_VERSION: int = 1`.
- Produces: module constant `PROVENANCE_REQUIRED_FIELDS: tuple[str, ...]`.
- Produces: validator helper `validate_checked_item_provenance(scope: str, item: dict[str, Any], errors: list[str]) -> None`.
- Produces: canonical `provenance` objects on checked manifest items `surface-decoder-full` and `bb-circuit-full`.

- [x] **Step 1: Add fixture provenance helper and failing tests**

In `tools/test_check_site_manifest.py`, add this helper after the imports:

```python
PROVENANCE_NOT_RECORDED_REASON = "historical fixture predates canonical provenance capture"


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
        "artifact_hashes": {"status": "not_recorded", "reason": PROVENANCE_NOT_RECORDED_REASON},
    }
```

Update `VALID_MANIFEST["families"][0]["evidence_items"][0]` to include:

```python
"provenance": fixture_provenance(["make surface-decoder-compare-full"]),
```

Update `VALID_MANIFEST["families"][1]["evidence_items"][0]` to include:

```python
"provenance": fixture_provenance(["make bb-circuit-bposd-compare-full"]),
```

Extend the `mutation` block inside `write_fixture_manifest()` with these cases:

```python
elif mutation == "missing_provenance":
    del manifest["families"][0]["evidence_items"][0]["provenance"]
elif mutation == "missing_provenance_cpu_model":
    del manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"]
elif mutation == "provenance_cpu_model_missing_reason":
    manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"] = {"status": "not_recorded"}
elif mutation == "bad_provenance_schema_version":
    manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = 2
elif mutation == "bad_provenance_schema_version_type":
    manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = "1"
```

Add these tests after `test_rejects_empty_source_and_provenance_sources`:

```python
    def test_rejects_checked_item_without_canonical_provenance(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_provenance")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("surface-decoder-full" in error and "provenance" in error for error in errors),
            errors,
        )

    def test_rejects_checked_item_missing_provenance_key(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="missing_provenance_cpu_model")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("surface-decoder-full" in error and "cpu_model" in error for error in errors),
            errors,
        )

    def test_rejects_not_recorded_provenance_without_reason(self) -> None:
        repo, manifest_path, _ = self.write_fixture_manifest(mutation="provenance_cpu_model_missing_reason")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "cpu_model" in error
                and "reason" in error
                for error in errors
            ),
            errors,
        )

    def test_rejects_unsupported_provenance_schema_version(self) -> None:
        for mutation in ("bad_provenance_schema_version", "bad_provenance_schema_version_type"):
            repo, manifest_path, _ = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(
                any("surface-decoder-full" in error and "schema_version" in error for error in errors),
                errors,
            )
```

- [x] **Step 2: Run the new tests and verify the RED state**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest -q
```

Expected before implementation: FAIL because the new negative controls still validate successfully without canonical provenance enforcement.

- [x] **Step 3: Implement canonical provenance validation**

In `tools/check_site_manifest.py`, add these constants after `CHECKED_ARTIFACT_REFERENCE_RE`:

```python
PROVENANCE_SCHEMA_VERSION = 1
PROVENANCE_REQUIRED_FIELDS = (
    "schema_version",
    "artifact_date",
    "source_commit",
    "commands",
    "os",
    "cpu_model",
    "rust_version",
    "python_version",
    "dependency_versions",
    "external_repository_commits",
    "seed_policy",
    "build_profile",
    "shots_or_error_budget",
    "artifact_hashes",
)
```

Add these helpers after `validate_artifact()`:

```python
def item_has_checked_artifacts(item: dict[str, Any]) -> bool:
    artifacts = item.get("artifacts")
    if not isinstance(artifacts, list):
        return False
    return any(isinstance(artifact, dict) and artifact.get("checked") is True for artifact in artifacts)


def validate_provenance_status_field(scope: str, provenance: dict[str, Any], field: str, errors: list[str]) -> None:
    if field not in provenance:
        add_error(errors, scope, f"provenance missing required field {field}")
        return

    entry = provenance[field]
    if not isinstance(entry, dict):
        add_error(errors, scope, f"provenance.{field} must be an object")
        return

    status = entry.get("status")
    if status == "recorded":
        if "value" not in entry:
            add_error(errors, scope, f"provenance.{field} recorded entry must include value")
        return

    if status == "not_recorded":
        if not is_non_empty_string(entry.get("reason")):
            add_error(errors, scope, f"provenance.{field} not_recorded entry must include non-empty reason")
        return

    add_error(errors, scope, f"provenance.{field} status must be 'recorded' or 'not_recorded'")


def validate_checked_item_provenance(scope: str, item: dict[str, Any], errors: list[str]) -> None:
    if not item_has_checked_artifacts(item):
        return

    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        add_error(errors, scope, "provenance must be an object")
        return

    if provenance.get("schema_version") != PROVENANCE_SCHEMA_VERSION:
        add_error(errors, scope, f"provenance.schema_version must be {PROVENANCE_SCHEMA_VERSION}")

    for field in PROVENANCE_REQUIRED_FIELDS:
        if field == "schema_version":
            continue
        validate_provenance_status_field(scope, provenance, field, errors)
```

Call `validate_checked_item_provenance(scope, item, errors)` in `validate_item()` after the `artifacts` validation block so checked artifacts still get their existing path checks.

In `make_fixture_repo()`, add a local helper or inline provenance objects for the surface checked fixture and the `bb-circuit-full` fixture. Use the same field shapes as the unit-test helper.

In `run_self_test()`, add these mutation checks to the `mutations` list and mutation writer:

```python
("missing_provenance", "surface-decoder-full", "provenance"),
("missing_provenance_cpu_model", "surface-decoder-full", "cpu_model"),
("provenance_cpu_model_missing_reason", "surface-decoder-full", "reason"),
("bad_provenance_schema_version", "surface-decoder-full", "schema_version"),
```

Mutation implementations:

```python
elif mutation == "missing_provenance":
    del manifest["families"][0]["evidence_items"][0]["provenance"]
elif mutation == "missing_provenance_cpu_model":
    del manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"]
elif mutation == "provenance_cpu_model_missing_reason":
    manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"] = {"status": "not_recorded"}
elif mutation == "bad_provenance_schema_version":
    manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = 2
```

- [x] **Step 4: Add canonical provenance to the real manifest**

In `site/benchmark-site.json`, add `provenance` to `surface-decoder-full` after `commands` or after `provenance_sources`. Use this object:

```json
"provenance": {
  "schema_version": 1,
  "artifact_date": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "source_commit": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "commands": {
    "status": "recorded",
    "value": [
      "make surface-decoder-compare-full",
      "make bench-surface-full"
    ]
  },
  "os": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "cpu_model": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "rust_version": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "python_version": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "dependency_versions": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "external_repository_commits": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "seed_policy": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "build_profile": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "shots_or_error_budget": {
    "status": "not_recorded",
    "reason": "historical checked artifact predates canonical provenance capture"
  },
  "artifact_hashes": {
    "status": "not_recorded",
    "reason": "hash-content verification is out of scope for this historical artifact"
  }
}
```

In `site/benchmark-site.json`, add `provenance` to `bb-circuit-full` after `commands` or after `provenance_sources`. Use the same object except set `commands.value` to:

```json
[
  "make bb-circuit-bposd-compare-full"
]
```

- [x] **Step 5: Run focused GREEN verification**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
```

Expected: all commands exit 0. The manifest checker should print one `ok: family ...` line per benchmark family.

- [x] **Step 6: Run manual negative controls**

Create temporary files without modifying the repository and run these checks:

```sh
tmp=$(mktemp /tmp/bad-benchmark-site.XXXXXX.json)
cp site/benchmark-site.json "$tmp"
python3 -c 'import json,sys; path=sys.argv[1]; data=json.load(open(path)); del data["families"][0]["evidence_items"][0]["provenance"]["cpu_model"]; open(path,"w").write(json.dumps(data, indent=2)+"\n")' "$tmp"
python3 tools/check_site_manifest.py --repo-root . "$tmp"
```

Expected: nonzero exit; output names `surface-decoder-full` and `cpu_model`.

Restore the temporary copy and run:

```sh
cp site/benchmark-site.json "$tmp"
python3 -c 'import json,sys; path=sys.argv[1]; data=json.load(open(path)); data["families"][0]["evidence_items"][0]["provenance"]["cpu_model"]={"status":"not_recorded"}; open(path,"w").write(json.dumps(data, indent=2)+"\n")' "$tmp"
python3 tools/check_site_manifest.py --repo-root . "$tmp"
```

Expected: nonzero exit; output names `surface-decoder-full`, `cpu_model`, and `reason`.

Restore the temporary copy and run:

```sh
cp site/benchmark-site.json "$tmp"
python3 -c 'import json,sys; path=sys.argv[1]; data=json.load(open(path)); del data["families"][0]["evidence_items"][0]["provenance"]; open(path,"w").write(json.dumps(data, indent=2)+"\n")' "$tmp"
python3 tools/check_site_manifest.py --repo-root . "$tmp"
```

Expected: nonzero exit; output names `surface-decoder-full` and `provenance`.

Remove the temporary file:

```sh
rm -f "$tmp"
```

- [x] **Step 7: Run repository verification**

Run:

```sh
cargo test
```

Expected: exit 0.

- [x] **Step 8: Final diff review and commit**

Run:

```sh
git diff --check
git diff --stat
git status --short
```

Expected: no whitespace errors; only the planned files are modified.

Commit:

```sh
git add site/benchmark-site.json tools/check_site_manifest.py tools/test_check_site_manifest.py docs/superpowers/plans/2026-07-08-issue-380-canonical-provenance-validation.md
git commit -m "feat: validate checked benchmark provenance"
```

## Self Review

- Spec coverage: Task 1 covers all #380 required manifest keys, shape validation, real manifest data, unit negative controls, self-test updates, manual negative controls, and `cargo test`.
- Marker scan: no unresolved open markers.
- Type consistency: helper names, mutation names, and expected error fields match across the unit tests, validator, and self-test plan.
