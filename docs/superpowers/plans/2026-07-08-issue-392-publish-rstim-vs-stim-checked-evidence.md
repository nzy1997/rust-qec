# Issue 392 Publish rstim-vs-Stim Checked Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the checked `rstim-vs-stim-simulator` evidence family on the benchmarked site as partial evidence with copied artifacts, provenance, and limited claims.

**Architecture:** Keep `site/benchmark-site.json` as the evidence source of truth, with the existing copy helper and manifest validator enforcing checked artifact paths, hashes, and provenance. Update Python/Rust contract tests before changing production files, then update the site app/static copy and checker policy so `rstim-vs-stim-simulator` is accepted as partial only when checked artifacts are present and copied.

**Tech Stack:** Python 3 manifest/build checkers and unittest fixtures, JSON benchmark manifest, static HTML/JS site, Rust integration contract tests, existing `rstim` perf and correctness command outputs.

## Global Constraints

- Family ID is exactly `rstim-vs-stim-simulator`.
- Family status is `partial`; do not leave this family as `future`.
- Claim boundary is limited to recorded workloads and recorded environments only.
- Do not optimize benchmark results or claim broad `rstim`/Stim parity.
- Checked artifacts must not live under `benchmarks/out/`.
- Required listed artifacts are speed summary, speed report, correctness summary, fixture manifest, canonical `.stim` input, and showcase page.
- Provenance requirements must include OS, CPU model, Rust version, Stim version, command line, seeds, build profile, shot counts, and date.
- Final issue verification commands are `make build-site` and `python3 tools/check_site_build.py _site`.
- Negative control removes `_site/benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json` and expects `python3 tools/check_site_build.py _site` to fail.

---

### Task 1: Publish Checked rstim-vs-Stim Artifacts

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`

**Interfaces:**
- Consumes: `/tmp/rstim-vs-stim-perf-ci/summary.json`, `/tmp/rstim-vs-stim-perf-ci/report.md`, and `/tmp/rstim-vs-stim-correctness.json` from the documented commands.
- Produces: tracked checked artifacts for the manifest item `rstim-vs-stim-full`.

- [ ] **Step 1: Verify generated source artifacts exist**

Run:

```bash
test -s /tmp/rstim-vs-stim-perf-ci/summary.json
test -s /tmp/rstim-vs-stim-perf-ci/report.md
test -s /tmp/rstim-vs-stim-correctness.json
```

Expected: all commands exit 0.

- [ ] **Step 2: Create checked artifact directory**

Run:

```bash
mkdir -p benchmarks/rstim_vs_stim_simulator/results/full
```

Expected: directory exists.

- [ ] **Step 3: Copy generated checked artifacts**

Run:

```bash
cp /tmp/rstim-vs-stim-perf-ci/summary.json benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json
cp /tmp/rstim-vs-stim-perf-ci/report.md benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md
cp /tmp/rstim-vs-stim-correctness.json benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
```

Expected: the three destination files exist and are non-empty.

- [ ] **Step 4: Inspect artifact content**

Run:

```bash
python3 -m json.tool benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json >/tmp/issue392-speed-summary.pretty
python3 -m json.tool benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json >/tmp/issue392-correctness-summary.pretty
rg -n "stim-style-surface-sample-d11-r100-b1024|shots/s|report-only Stim comparison" benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md
```

Expected: both JSON files parse; the report contains the selected case, `shots/s`, and `report-only Stim comparison`.

- [ ] **Step 5: Commit artifact publication**

Run:

```bash
git add benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json \
  benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md \
  benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
git commit -m "docs: add rstim-vs-stim checked artifacts"
```

Expected: commit succeeds. Task 2 will compute manifest hashes from these
committed files.

### Task 2: Add Manifest Policy Tests And Manifest Entry

**Files:**
- Modify: `tools/test_check_site_manifest.py`
- Modify: `tools/check_site_manifest.py`
- Modify: `site/benchmark-site.json`

**Interfaces:**
- Consumes: checked artifact paths from Task 1.
- Produces: `rstim-vs-stim-full` checked evidence item with canonical provenance and validator coverage for partial status.

- [ ] **Step 1: Write failing manifest tests**

Add constants to `tools/test_check_site_manifest.py`:

```python
RSTIM_SPEED_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
RSTIM_SPEED_REPORT_PATH = "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md"
RSTIM_CORRECTNESS_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json"
RSTIM_CASES_FULL_PATH = "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
RSTIM_CANONICAL_STIM_PATH = (
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
)
RSTIM_SHOWCASE_PATH = "docs/showcases/rstim-vs-stim-simulator.md"
```

Update the fixture to create those files, add them to git, and replace the future family fixture with a partial family containing checked artifacts. Add a mutation named `rstim_partial_without_checked_artifacts` that removes the artifacts from the `rstim-vs-stim-full` item.

Add this assertion test:

```python
def test_rejects_rstim_partial_family_without_checked_artifacts(self) -> None:
    repo, manifest_path, _ = self.write_fixture_manifest(mutation="rstim_partial_without_checked_artifacts")
    errors = check_site_manifest.validate_manifest(repo, manifest_path)
    self.assertTrue(
        any(
            "rstim-vs-stim-simulator" in error
            and "partial" in error
            and "checked artifact" in error
            for error in errors
        ),
        errors,
    )
```

- [ ] **Step 2: Run the failing manifest test**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest.SiteManifestValidatorTest.test_rejects_rstim_partial_family_without_checked_artifacts -v
```

Expected: FAIL because the validator does not yet enforce partial checked artifacts for this family.

- [ ] **Step 3: Implement manifest policy**

In `tools/check_site_manifest.py`, add helpers:

```python
def family_has_checked_artifacts(family: dict[str, Any]) -> bool:
    items = family.get("evidence_items")
    if not isinstance(items, list):
        return False
    return any(isinstance(item, dict) and item_has_checked_artifacts(item) for item in items)


def validate_family_status_policy(scope: str, family: dict[str, Any], errors: list[str]) -> None:
    family_id = family.get("id")
    if family_id == "rstim-vs-stim-simulator" and family.get("status") == "partial":
        if not family_has_checked_artifacts(family):
            add_error(errors, scope, "partial rstim-vs-stim-simulator family must list checked artifacts")
```

Call `validate_family_status_policy(scope, family, errors)` after validating `family["status"]` in `validate_family`.

- [ ] **Step 4: Update manifest fixture data**

Update `VALID_MANIFEST` in `tools/test_check_site_manifest.py` so
`rstim-vs-stim-simulator` has status `partial`, source docs include
`docs/showcases/rstim-vs-stim-simulator.md` and
`benchmarks/rstim_vs_stim_simulator/README.md`, and item `rstim-vs-stim-full`
lists the six checked artifacts.

Use fixture hashes for fixture files; the real manifest hashes are added in
Step 6.

- [ ] **Step 5: Re-run manifest tests**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -v
```

Expected: PASS.

- [ ] **Step 6: Update site manifest with real hashes**

Compute hashes:

```bash
shasum -a 256 \
  benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json \
  benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md \
  benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json \
  benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  docs/showcases/rstim-vs-stim-simulator.md
```

Replace the `rstim-vs-stim-simulator` family in `site/benchmark-site.json` with a
partial family and item `rstim-vs-stim-full`. The item must include canonical
provenance fields, recorded commands, recorded environment values, recorded
artifact hashes, provenance requirements including Stim version and shot counts,
caveats that keep the claim narrow, and the checked artifacts with these kinds:
`speed-summary`, `speed-report`, `correctness-summary`, `fixture-manifest`,
`stim-fixture`, and `showcase`.

- [ ] **Step 7: Validate the real manifest**

Run:

```bash
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
```

Expected: exits 0 and prints `ok: family rstim-vs-stim-simulator status=partial`.

### Task 3: Update Site Build Policy And Visible Site Copy

**Files:**
- Modify: `tools/test_check_site_build.py`
- Modify: `tools/check_site_build.py`
- Modify: `rstim/tests/site_contract.rs`
- Modify: `site/index.html`
- Modify: `site/app.js`

**Interfaces:**
- Consumes: partial manifest policy and checked artifact paths from Task 2.
- Produces: built-site checker that accepts partial checked evidence and rejects missing/copy errors, plus visible checked-result site copy.

- [ ] **Step 1: Write failing build checker tests**

In `tools/test_check_site_build.py`, update the valid fixture's
`rstim-vs-stim-simulator` family to partial with checked artifacts and add the
copied fixture files to the repo and site fixture. Add a test:

```python
def test_rejects_missing_rstim_vs_stim_checked_artifact(self) -> None:
    fixture = check_site_build.make_fixture_site()
    self.addCleanup(fixture.cleanup)
    artifact_path = "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json"
    (fixture.site_root / artifact_path).unlink()

    results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

    self.assertTrue(
        any(
            result.status == "FAIL"
            and result.area == "checked benchmark artifacts"
            and "correctness-summary.json" in result.detail
            for result in results
        ),
        check_site_build.format_summary(results),
    )
```

- [ ] **Step 2: Run the failing build checker test**

Run:

```bash
python3 -m unittest tools.test_check_site_build.SiteBuildCheckerTest.test_valid_fixture_prints_required_pass_summary_areas -v
```

Expected: FAIL because `check_local_only_future` still requires `rstim-vs-stim` to be future.

- [ ] **Step 3: Implement build checker policy**

In `tools/check_site_build.py`:

- expand `CHECKED_ARTIFACT_REFERENCE_RE` to include `rstim_vs_stim_simulator`;
- replace the hard-coded `rstim_status != "future"` check with a policy accepting `future` or `partial`;
- require `partial` to have checked artifacts whose paths are under `benchmarks/rstim_vs_stim_simulator/` but not under `benchmarks/out/`;
- update the PASS detail to mention `rstim-vs-stim partial checked evidence`.

- [ ] **Step 4: Update site contract tests**

In `rstim/tests/site_contract.rs`, update
`qec_code_and_future_benchmarks_are_classified` so it expects:

```rust
assert_eq!(
    future_family["status"], "partial",
    "rstim versus Stim simulator family must be partial checked evidence"
);
```

Rename the local variable from `future_family` to `rstim_vs_stim_family` if that
makes the test clearer. Require item `rstim-vs-stim-full`, checked artifact
paths for speed/correctness/fixture/showcase, and copy text that includes
`Partial checked evidence`, `recorded workloads and recorded environments`, and
`not broad rstim/Stim parity`.

- [ ] **Step 5: Run the failing Rust site contract test**

Run:

```bash
cargo test -p rstim --test site_contract qec_code_and_future_benchmarks_are_classified -q
```

Expected: FAIL before updating `site/index.html` and `site/app.js`.

- [ ] **Step 6: Update visible site copy**

In `site/index.html`, replace the `future-simulator-benchmarks` article with
partial checked evidence copy. Keep the anchor stable or add
`id="rstim-vs-stim-simulator-benchmarks"` while preserving the old id only if
tests require it. Include:

- `Partial checked evidence`;
- `recorded workloads and recorded environments`;
- `not broad rstim/Stim parity`;
- links to the showcase and fixture README.

In `site/app.js`, add `rstim-vs-stim-full` to `checkedBenchmarkItems`.

- [ ] **Step 7: Re-run focused tests**

Run:

```bash
python3 -m unittest tools.test_check_site_build -v
cargo test -p rstim --test site_contract qec_code_and_future_benchmarks_are_classified -q
```

Expected: PASS.

### Task 4: End-to-End Verification, Commit, And PR

**Files:**
- Verify all files from Tasks 1-3.
- Commit implementation files and Superpowers docs.

**Interfaces:**
- Consumes: completed implementation.
- Produces: verified branch and pull request.

- [ ] **Step 1: Run manifest and build self-tests**

Run:

```bash
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_build.py --self-test
```

Expected: both exit 0.

- [ ] **Step 2: Run issue verification**

Run:

```bash
make build-site
python3 tools/check_site_build.py _site
```

Expected: checker output includes `PASS checked benchmark artifacts`, mentions
`rstim-vs-stim` as partial checked evidence, and reports `SUMMARY: PASS`.

- [ ] **Step 3: Run issue negative control**

Run:

```bash
rm _site/benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
python3 tools/check_site_build.py _site
```

Expected: exits nonzero and reports a missing checked artifact for
`correctness-summary.json`.

Rebuild the site after the negative control:

```bash
make build-site
```

- [ ] **Step 4: Run required cargo test**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 5: Review changed files**

Run:

```bash
git status --short
git diff --check
git diff --stat
```

Expected: no whitespace errors; changed files are scoped to issue #392.

- [ ] **Step 6: Commit**

Run:

```bash
git add \
  docs/superpowers/specs/2026-07-08-issue-392-publish-rstim-vs-stim-checked-evidence-design.md \
  docs/superpowers/plans/2026-07-08-issue-392-publish-rstim-vs-stim-checked-evidence.md \
  site/benchmark-site.json \
  site/index.html \
  site/app.js \
  tools/check_site_manifest.py \
  tools/check_site_build.py \
  tools/test_check_site_manifest.py \
  tools/test_check_site_build.py \
  rstim/tests/site_contract.rs
git commit -m "docs: publish rstim-vs-stim checked evidence"
```

Expected: commit succeeds.

- [ ] **Step 7: Finish branch**

Use `superpowers:verification-before-completion` before claiming completion.
Then use `superpowers:finishing-a-development-branch` and choose option 2,
`Push and create a Pull Request`, under the standing policy.

Expected: branch is pushed and a PR is created against `master`.

## Plan Self-Review

- Spec coverage: all issue #392 requirements map to Tasks 1-4.
- Placeholder scan: no TBD/TODO/fill-in placeholders remain.
- Type consistency: item id `rstim-vs-stim-full` and family id
  `rstim-vs-stim-simulator` are used consistently.
- The plan keeps artifact publication, manifest policy, visible site copy, and
  verification as independently checkable tasks.
