# Issue 383 Site Build Provenance Check Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a provenance-specific PASS/FAIL area to the reviewer-readable built-site checker while delegating provenance schema, renderer wiring, copied artifact, and SHA-256 validation to `tools.check_site_manifest`.

**Architecture:** Keep `tools/check_site_build.py` as a wrapper-level checker. It will continue to run `tools.check_site_manifest.validate_manifest()` and `validate_site_root()`, then add a shallow provenance summary for checked evidence items only when delegated validation passes. Delegated provenance/hash/wiring failures will also surface through the new provenance area so reviewers get a direct PASS/FAIL line.

**Tech Stack:** Python 3 standard library, existing `tools.check_site_manifest`, Python `unittest`, existing static site build Makefile, Cargo workspace.

## Global Constraints

- Command interface remains `python3 tools/check_site_build.py --self-test` and `python3 tools/check_site_build.py _site`.
- Reuse `tools.check_site_manifest` for schema, copied artifact, hash, and minimal renderer-wiring validation.
- Do not reimplement the full provenance schema or SHA-256 logic in `tools/check_site_build.py`.
- The checker summary includes a provenance PASS line when checked items expose valid provenance through the built site.
- The checker emits provenance FAIL when provenance is missing, malformed, hash-invalid, or not exposed by the renderer.
- PASS summary names checked evidence items such as `surface-decoder-full` and `bb-circuit-full`.
- PASS summary avoids implying that historical `not_recorded` fields are recorded.
- Self-test includes a mutation where `surface-decoder-full.provenance` is removed and the checker fails with a message naming `surface-decoder-full` and `provenance`.
- Self-test includes a mutation where a copied checked artifact under `_site/benchmarks/...` is changed without changing the manifest and the checker reports the delegated manifest/hash failure naming the artifact path.
- Out of scope: external link checking, browser automation, replacing the static site stack, running or regenerating benchmark campaigns.

---

## File Structure

- Modify `tools/test_check_site_build.py`: add red tests for the provenance PASS line and the two provenance/hash negative controls.
- Modify `tools/check_site_build.py`: add delegated-error plumbing, provenance summary helper, and self-test mutations.
- Commit this plan and the matching design spec because the active Superpowers workflow requires durable artifacts.

---

### Task 1: Provenance Summary And Negative Controls

**Files:**
- Modify: `tools/test_check_site_build.py`
- Modify: `tools/check_site_build.py`

**Interfaces:**
- Consumes:
  - `tools.check_site_manifest.validate_manifest(repo_root, manifest_path, site_root=site_root) -> list[str]`
  - `tools.check_site_manifest.validate_site_root(site_root, manifest_path) -> list[str]`
  - `tools.check_site_manifest.iter_checked_artifacts(manifest) -> list[tuple[dict[str, Any], str, str]]`
- Produces:
  - `check_checked_provenance(manifest: dict[str, object] | None, delegated_errors: list[str]) -> CheckResult`
  - `checked benchmark provenance` summary line in `format_summary(check_site_build(...))`

- [ ] **Step 1: Write the failing PASS-summary test**

In `tools/test_check_site_build.py`, extend `test_valid_fixture_prints_required_pass_summary_areas()` by adding `"PASS checked benchmark provenance"` to the marker list, then assert the output names both checked evidence items:

```python
self.assertIn("surface-decoder-full", output)
self.assertIn("bb-circuit-full", output)
self.assertIn("not_recorded", output)
self.assertIn("checked artifact hashes", output)
```

- [ ] **Step 2: Write the failing missing-provenance test**

Add this test to `SiteBuildCheckerTest`:

```python
def test_rejects_checked_item_missing_built_manifest_provenance(self) -> None:
    fixture = check_site_build.make_fixture_site()
    self.addCleanup(fixture.cleanup)
    manifest_path = fixture.site_root / "data/benchmark-site.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    del manifest["families"][0]["evidence_items"][0]["provenance"]
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

    self.assertTrue(
        any(
            result.status == "FAIL"
            and result.area == "checked benchmark provenance"
            and "surface-decoder-full" in result.detail
            and "provenance" in result.detail
            for result in results
        ),
        check_site_build.format_summary(results),
    )
```

Also import `json` at the top of the file.

- [ ] **Step 3: Write the failing copied-artifact hash mutation test**

Add this test to `SiteBuildCheckerTest`:

```python
def test_rejects_copied_checked_artifact_hash_mutation(self) -> None:
    fixture = check_site_build.make_fixture_site()
    self.addCleanup(fixture.cleanup)
    artifact_path = "benchmarks/surface_decoder_compare/results/full/results.csv"
    (fixture.site_root / artifact_path).write_text("mutated copied artifact\n", encoding="utf-8")

    results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

    self.assertTrue(
        any(
            result.status == "FAIL"
            and result.area in {"manifest and copied artifacts", "checked benchmark provenance"}
            and artifact_path in result.detail
            and "sha256" in result.detail
            for result in results
        ),
        check_site_build.format_summary(results),
    )
```

- [ ] **Step 4: Run focused tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_site_build -q
```

Expected before implementation: FAIL because no `checked benchmark provenance`
result exists.

- [ ] **Step 5: Plumb delegated manifest/site errors**

In `tools/check_site_build.py`, change:

```python
def check_manifest_and_artifacts(site_root: Path, repo_root: Path) -> tuple[list[CheckResult], dict[str, object] | None]:
```

to:

```python
def check_manifest_and_artifacts(site_root: Path, repo_root: Path) -> tuple[list[CheckResult], dict[str, object] | None, list[str]]:
```

Return `combined` as the third tuple value. Update the caller in
`check_site_build()`:

```python
manifest_results, manifest, manifest_site_errors = check_manifest_and_artifacts(site_root, repo_root)
results.extend(manifest_results)
results.append(check_checked_provenance(manifest, manifest_site_errors))
```

Place the provenance result immediately after `manifest and copied artifacts`
so the reviewer sees the delegated validation and the provenance summary
together.

- [ ] **Step 6: Implement shallow provenance summary helper**

Add helpers near `check_checked_artifacts()`:

```python
PROVENANCE_ERROR_MARKERS = (
    "provenance",
    "artifact_hashes",
    "sha256",
    "copied artifact",
)


def relevant_provenance_errors(errors: list[str]) -> list[str]:
    return [error for error in errors if any(marker in error for marker in PROVENANCE_ERROR_MARKERS)]


def plural(count: int, singular: str, plural_form: str | None = None) -> str:
    if count == 1:
        return f"{count} {singular}"
    return f"{count} {plural_form or singular + 's'}"


def summarize_provenance_item(item_id: str, item: dict[str, object], checked_paths: set[str]) -> str:
    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        return f"{item_id} (provenance missing from built manifest)"

    recorded_fields = 0
    not_recorded_fields = 0
    for field, value in provenance.items():
        if field == "schema_version" or not isinstance(value, dict):
            continue
        if value.get("status") == "recorded":
            recorded_fields += 1
        elif value.get("status") == "not_recorded":
            not_recorded_fields += 1

    artifact_hash_count = 0
    artifact_hashes = provenance.get("artifact_hashes")
    if isinstance(artifact_hashes, dict) and isinstance(artifact_hashes.get("value"), dict):
        artifact_hash_count = sum(
            1
            for path, entry in artifact_hashes["value"].items()
            if path in checked_paths and isinstance(entry, dict) and isinstance(entry.get("sha256"), str)
        )

    return (
        f"{item_id} ({plural(recorded_fields, 'recorded field')}, "
        f"{plural(not_recorded_fields, 'not_recorded field')}, "
        f"{plural(artifact_hash_count, 'checked artifact hash', 'checked artifact hashes')})"
    )
```

Then add:

```python
def check_checked_provenance(manifest: dict[str, object] | None, delegated_errors: list[str]) -> CheckResult:
    provenance_errors = relevant_provenance_errors(delegated_errors)
    if provenance_errors:
        return fail("checked benchmark provenance", "; ".join(provenance_errors))
    if manifest is None:
        return fail("checked benchmark provenance", "manifest could not be loaded")

    checked = check_site_manifest.iter_checked_artifacts(manifest)
    if not checked:
        return fail("checked benchmark provenance", "no checked evidence items are listed in the manifest")

    by_item: dict[str, tuple[dict[str, object], set[str]]] = {}
    for item, item_id, artifact_path in checked:
        current_item, paths = by_item.setdefault(item_id, (item, set()))
        del current_item
        paths.add(artifact_path)

    summaries = [summarize_provenance_item(item_id, item, paths) for item_id, (item, paths) in sorted(by_item.items())]
    return pass_("checked benchmark provenance", "; ".join(summaries) + " exposed through manifest-backed renderer")
```

- [ ] **Step 7: Extend checker self-test mutations**

In `run_self_test()`, add two mutations:

```python
(
    "missing_surface_provenance",
    lambda f: remove_surface_provenance(f.site_root / "data/benchmark-site.json"),
    "surface-decoder-full",
),
(
    "corrupt_copied_checked_artifact",
    lambda f: (f.site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").write_text(
        "mutated copied artifact\n",
        encoding="utf-8",
    ),
    "benchmarks/surface_decoder_compare/results/full/results.csv",
),
```

Add a small helper before `run_self_test()`:

```python
def remove_surface_provenance(manifest_path: Path) -> None:
    manifest = check_site_manifest.load_json(manifest_path)
    if isinstance(manifest, dict):
        families = manifest.get("families")
        if isinstance(families, list):
            for family in families:
                if isinstance(family, dict) and family.get("id") == "surface-decoder-comparison":
                    items = family.get("evidence_items")
                    if isinstance(items, list) and items and isinstance(items[0], dict):
                        items[0].pop("provenance", None)
                        break
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
```

Also make the self-test mutation assertion search the full formatted summary
instead of only individual result details:

```python
if f"FAIL" not in summary or marker not in summary:
    failures.append(...)
```

- [ ] **Step 8: Run focused tests to verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_site_build -q
```

Expected: PASS.

- [ ] **Step 9: Run issue verification**

Run:

```sh
make build-site
python3 tools/check_site_build.py --self-test
python3 tools/check_site_build.py _site
cargo test
```

Expected: all commands exit 0, and the final checker summary includes
`PASS checked benchmark provenance` naming `surface-decoder-full` and
`bb-circuit-full`.

- [ ] **Step 10: Commit**

Run:

```sh
git add tools/check_site_build.py tools/test_check_site_build.py docs/superpowers/specs/2026-07-08-issue-383-site-build-provenance-check-design.md docs/superpowers/plans/2026-07-08-issue-383-site-build-provenance-check.md
git commit -m "Add provenance to site build checker"
```

Expected: commit succeeds.

---

## Self Review

- Spec coverage: Task 1 covers the provenance PASS/FAIL summary, checked item
  names, recorded/not-recorded/hash counts, delegated validation, missing
  provenance mutation, copied artifact hash mutation, and required verification.
- Placeholder scan: no TBD/TODO/implement-later markers are present.
- Type consistency: helper names and return types match their usage in the plan.
