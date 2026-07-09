# Issue 416 Post-Optimization Speed Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish checked post-optimization release-profile rstim-vs-Stim selected-case evidence without overwriting the old #406 debug-profile artifact.

**Architecture:** Keep the old #406 `results/full/` artifact immutable and add a separate checked `results/release/` evidence directory. Add one directory-based checker that reuses the #408 semantic guard for the old summary, validates the release directory and environment metadata, and confirms docs/site metadata link old and new evidence separately with narrow claims.

**Tech Stack:** Python 3.14 standard library, existing `benchmarks.rstim_vs_stim_simulator.run_speed_case` runner, existing Rust `rstim perf` CLI, JSON/Markdown checked artifacts, existing site manifest schema.

## Global Constraints

- Do not overwrite files under `benchmarks/rstim_vs_stim_simulator/results/full/`.
- Publish checked release artifacts under `benchmarks/rstim_vs_stim_simulator/results/release/`.
- The release evidence is only for `stim-style-surface-sample-d11-r100-b1024` with `--profile release`, `--warmup-rounds 0`, and `--measure-rounds 1`.
- Do not add CI wall-clock gates based on cross-machine Stim ratios.
- Do not claim broad `rstim`/Stim performance parity or all-workload parity.
- The checker command must accept `--old` and `--new-dir` and print `PASS post-optimization evidence is separate from the checked #406 artifact` on success.
- The negative control that copies the old summary into the new directory must fail.

---

## File Structure

- Create `tools/check_rstim_vs_stim_post_optimization_evidence.py`: CLI checker for old/new separation, release evidence completeness, docs links, site metadata, and narrow claims.
- Create `tools/test_check_rstim_vs_stim_post_optimization_evidence.py`: focused unittest coverage and negative controls for the checker.
- Create `benchmarks/rstim_vs_stim_simulator/results/release/summary.json`: checked promoted release summary from the #407 runner.
- Create `benchmarks/rstim_vs_stim_simulator/results/release/report.md`: checked promoted release report from the #407 runner.
- Create `benchmarks/rstim_vs_stim_simulator/results/release/environment.json`: checked promoted release environment metadata from the #407 runner plus a release evidence marker.
- Modify `docs/showcases/rstim-vs-stim-simulator.md`: link old and new artifacts separately, describe the release run narrowly, and preserve the limits.
- Modify `site/benchmark-site.json`: add a separate `rstim-vs-stim-release` evidence item with hashes for the three release files and existing docs/source provenance.
- Modify `site/app.js` and `site/index.html`: include `rstim-vs-stim-release` in the checked result cards.
- Modify `tools/check_site_manifest.py` and its fixture if needed so release results under `results/release/` are valid checked rstim-vs-Stim artifacts.
- Modify `tools/check_site_build.py` tests if checked-item expectations are hard-coded.

### Task 1: Add The Directory-Based Checker With Tests

**Files:**
- Create: `tools/check_rstim_vs_stim_post_optimization_evidence.py`
- Create: `tools/test_check_rstim_vs_stim_post_optimization_evidence.py`

**Interfaces:**
- Consumes: `tools.check_rstim_vs_stim_gap_artifact.validate_case(summary: dict[str, Any]) -> float`
- Produces: `python3 tools/check_rstim_vs_stim_post_optimization_evidence.py --old <path> --new-dir <dir>`

- [ ] **Step 1: Write failing checker tests**

Create `tools/test_check_rstim_vs_stim_post_optimization_evidence.py` with unittest helpers that copy the real old summary, build temporary release directories, create temporary docs/site metadata fixtures, and run the checker in subprocesses. Include tests for:

```python
def test_accepts_separate_release_fixture(self) -> None:
    release = self.write_valid_release_fixture()
    docs = self.write_docs_fixture()
    manifest = self.write_manifest_fixture(release)
    result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertIn(
        "PASS post-optimization evidence is separate from the checked #406 artifact",
        result.stdout,
    )

def test_rejects_reused_old_summary_as_new_evidence(self) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        release = Path(tmp) / "release"
        release.mkdir()
        shutil.copy(DEFAULT_OLD_SUMMARY, release / "summary.json")
        (release / "environment.json").write_text('{"profile":"release"}\n', encoding="utf-8")
        (release / "report.md").write_text("# pretend report\n", encoding="utf-8")
        result = self.run_checker(old=release / "summary.json", new_dir=release)
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("new summary reuses the checked #406 summary", result.stderr)

def test_rejects_missing_environment_metadata(self) -> None:
    release = self.write_valid_release_fixture()
    environment = json.loads((release / "environment.json").read_text(encoding="utf-8"))
    del environment["rstim_binary_path"]
    (release / "environment.json").write_text(json.dumps(environment), encoding="utf-8")
    result = self.run_checker(new_dir=release)
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("environment.json missing rstim_binary_path", result.stderr)
```

Also cover missing `report.md`, missing site item, missing docs link, and broad parity wording in docs. Use temporary fixture repos for docs/site mutations so the default real repo remains untouched.

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_post_optimization_evidence -v
```

Expected: FAIL because `tools/check_rstim_vs_stim_post_optimization_evidence.py` does not exist yet.

- [ ] **Step 3: Implement the checker**

Implement the new checker with these functions:

```python
DEFAULT_OLD_SUMMARY = Path("benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json")
DEFAULT_NEW_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/release")
DEFAULT_DOCS_PATH = Path("docs/showcases/rstim-vs-stim-simulator.md")
DEFAULT_MANIFEST_PATH = Path("site/benchmark-site.json")
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"
REQUIRED_RELEASE_FILES = ("summary.json", "report.md", "environment.json")
BROAD_CLAIM_PATTERNS = (
    "broad rstim/stim performance parity",
    "all-workload parity",
    "all workloads",
)
```

The checker should:

1. Load the old summary, call `check_rstim_vs_stim_gap_artifact.validate_case`, and preserve that function's strict 200-300 ratio/fingerprint behavior.
2. Require `summary.json`, `report.md`, and `environment.json` under `--new-dir`.
3. Reject when `sha256(old_summary) == sha256(new_dir / "summary.json")`.
4. Require the new summary to contain exactly one case with `case_label == SELECTED_CASE_LABEL`, `workload == "sample"`, `tier == "report_only"`, and `rstim-compiled` in `present_variants`.
5. Require `environment.json` fields `profile == "release"`, `evidence_kind` containing `post-optimization`, `rstim_binary_path`, `rustc_version`, `cargo_version`, `stim_cli_status`, and either `stim_cli_version` or `stim_cli.stderr`.
6. Require `report.md` to contain `report-only Stim comparison` and the selected case label.
7. Require `docs/showcases/rstim-vs-stim-simulator.md` to mention both `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json` and `benchmarks/rstim_vs_stim_simulator/results/release/summary.json`.
8. Require `site/benchmark-site.json` to contain separate evidence items `rstim-vs-stim-full` and `rstim-vs-stim-release`, with artifact entries for all three release files and recorded hashes matching repository files.
9. Reject broad parity wording in the docs by scanning lowercased docs text for the exact forbidden phrases above.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_post_optimization_evidence -v
```

Expected: PASS for fixture-based checker behavior. The real default checked-artifact command is verified in Task 4 after Task 2 and Task 3 create the checked release directory and docs/site links.

### Task 2: Generate And Promote Release Evidence

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/release/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release/environment.json`

**Interfaces:**
- Consumes: `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case --profile release --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir <dir>`
- Produces: checked release evidence directory accepted by the new checker.

- [ ] **Step 1: Generate staged release evidence**

Run:

```sh
rm -rf /tmp/rstim-speed-release-issue-416
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case \
  --profile release \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir /tmp/rstim-speed-release-issue-416
```

Expected: exit 0 and `/tmp/rstim-speed-release-issue-416/summary.json`,
`report.md`, and `environment.json` exist.

- [ ] **Step 2: Promote only the selected artifacts**

Create `benchmarks/rstim_vs_stim_simulator/results/release/` and copy only
`summary.json`, `report.md`, and `environment.json` from the staged directory.
Do not copy `raw.jsonl`.

- [ ] **Step 3: Mark the environment as post-optimization evidence**

Update `environment.json` to include:

```json
{
  "evidence_kind": "post-optimization release speed evidence",
  "published_artifact": true,
  "source_issue": 416
}
```

Keep the runner-recorded `profile`, `rstim_binary_path`, `rustc_version`,
`cargo_version`, and Stim CLI metadata.

- [ ] **Step 4: Sanity-check release evidence**

Run:

```sh
python3 - <<'PY'
import json
from pathlib import Path
root = Path("benchmarks/rstim_vs_stim_simulator/results/release")
for name in ("summary.json", "report.md", "environment.json"):
    assert (root / name).is_file(), name
summary = json.loads((root / "summary.json").read_text())
assert [case["case_label"] for case in summary["cases"]] == ["stim-style-surface-sample-d11-r100-b1024"]
env = json.loads((root / "environment.json").read_text())
assert env["profile"] == "release"
assert "post-optimization" in env["evidence_kind"]
assert "rstim_binary_path" in env
print("PASS release evidence promoted")
PY
```

Expected: `PASS release evidence promoted`.

### Task 3: Link Docs And Site Metadata Separately

**Files:**
- Modify: `docs/showcases/rstim-vs-stim-simulator.md`
- Modify: `site/benchmark-site.json`
- Modify: `site/app.js`
- Modify: `site/index.html`
- Modify: `tools/check_site_manifest.py`
- Modify: `tools/check_site_build.py` only if tests show hard-coded checked item expectations.

**Interfaces:**
- Consumes: release artifacts from Task 2
- Produces: docs/site metadata that the new checker and existing manifest checker accept.

- [ ] **Step 1: Update docs with separate checked artifacts**

Add a section under `## Expected Result` that names:

```markdown
Checked historical #406 speed evidence:
[`benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`](benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json)

Checked post-optimization release evidence:
[`benchmarks/rstim_vs_stim_simulator/results/release/summary.json`](benchmarks/rstim_vs_stim_simulator/results/release/summary.json),
[`benchmarks/rstim_vs_stim_simulator/results/release/report.md`](benchmarks/rstim_vs_stim_simulator/results/release/report.md),
and
[`benchmarks/rstim_vs_stim_simulator/results/release/environment.json`](benchmarks/rstim_vs_stim_simulator/results/release/environment.json).
```

Keep the Limits section wording narrow and retain the sentence that the selected speed command does not make broad `rstim` performance parity claims.

- [ ] **Step 2: Update site manifest policy for release paths**

In `tools/check_site_manifest.py`, extend `CHECKED_ARTIFACT_REFERENCE_RE` and the rstim-vs-Stim artifact policy to allow checked paths under `benchmarks/rstim_vs_stim_simulator/results/release/`. Keep all existing `results/full/` requirements unchanged.

- [ ] **Step 3: Add release evidence item to `site/benchmark-site.json`**

Add an evidence item:

```json
{
  "id": "rstim-vs-stim-release",
  "title": "Checked post-optimization release rstim versus Stim selected-case speed evidence",
  "status": "existing",
  "tier": "release",
  "artifacts": [
    {"path": "benchmarks/rstim_vs_stim_simulator/results/release/summary.json", "kind": "speed-summary", "checked": true},
    {"path": "benchmarks/rstim_vs_stim_simulator/results/release/report.md", "kind": "speed-report", "checked": true},
    {"path": "benchmarks/rstim_vs_stim_simulator/results/release/environment.json", "kind": "environment", "checked": true}
  ],
  "commands": [
    "python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case --profile release --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-speed-release-issue-416",
    "cp /tmp/rstim-speed-release-issue-416/summary.json benchmarks/rstim_vs_stim_simulator/results/release/summary.json",
    "cp /tmp/rstim-speed-release-issue-416/report.md benchmarks/rstim_vs_stim_simulator/results/release/report.md",
    "cp /tmp/rstim-speed-release-issue-416/environment.json benchmarks/rstim_vs_stim_simulator/results/release/environment.json"
  ],
  "claims_limit": "Checked post-optimization release evidence for one recorded d11/r100 selected-case workload and one recorded environment only; not broad rstim/Stim parity."
}
```

Fill `provenance` using the existing schema with recorded command, OS/CPU/toolchain/environment data from `environment.json`, and artifact hashes computed from the three release files.

- [ ] **Step 4: Add release item to checked result cards**

Update `site/app.js`:

```javascript
const checkedBenchmarkItems = ["surface-decoder-full", "bb-circuit-full", "rstim-vs-stim-full", "rstim-vs-stim-release"];
```

Update `site/index.html`:

```html
data-checked-items="surface-decoder-full bb-circuit-full rstim-vs-stim-full rstim-vs-stim-release"
```

- [ ] **Step 5: Run docs and site checks**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-vs-stim-simulator.md
python3 tools/check_site_manifest.py site/benchmark-site.json
python3 tools/check_site_build.py --self-test
python3 tools/check_site_manifest.py --self-test
```

Expected: all exit 0.

### Task 4: Verify The Publication Contract

**Files:**
- Modify: any files needed to fix failures from the verification commands.

**Interfaces:**
- Consumes: Tasks 1-3
- Produces: passing issue verification and a ready-to-PR branch.

- [ ] **Step 1: Run the issue verification command**

Run:

```sh
python3 tools/check_rstim_vs_stim_post_optimization_evidence.py \
  --old benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json \
  --new-dir benchmarks/rstim_vs_stim_simulator/results/release
```

Expected output includes:

```text
PASS post-optimization evidence is separate from the checked #406 artifact
```

- [ ] **Step 2: Run the issue negative control**

Run:

```sh
mkdir -p /tmp/pretend-release-evidence
cp benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json /tmp/pretend-release-evidence/summary.json
printf '{"profile":"release"}\n' > /tmp/pretend-release-evidence/environment.json
printf '# pretend report\n' > /tmp/pretend-release-evidence/report.md
if python3 tools/check_rstim_vs_stim_post_optimization_evidence.py \
  --old /tmp/pretend-release-evidence/summary.json \
  --new-dir /tmp/pretend-release-evidence; then
  echo 'unexpected overwritten-old-artifact success' >&2
  exit 1
fi
```

Expected: checker exits nonzero and the shell snippet exits 0.

- [ ] **Step 3: Run focused and global checks**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_post_optimization_evidence -v
python3 -m unittest tools.test_check_rstim_vs_stim_gap_artifact tools.test_check_site_manifest tools.test_check_site_build -v
git diff --check
cargo test
```

Expected: all exit 0.

- [ ] **Step 4: Commit and prepare PR**

Run:

```sh
git status --short
git add docs/superpowers/plans/2026-07-09-issue-416-post-optimization-speed-evidence.md \
  tools/check_rstim_vs_stim_post_optimization_evidence.py \
  tools/test_check_rstim_vs_stim_post_optimization_evidence.py \
  benchmarks/rstim_vs_stim_simulator/results/release/summary.json \
  benchmarks/rstim_vs_stim_simulator/results/release/report.md \
  benchmarks/rstim_vs_stim_simulator/results/release/environment.json \
  docs/showcases/rstim-vs-stim-simulator.md \
  site/benchmark-site.json \
  site/app.js \
  site/index.html \
  tools/check_site_manifest.py \
  tools/check_site_build.py
git commit -m "feat: publish post-optimization rstim-vs-stim evidence"
```

Expected: implementation commit created. Then use the finishing branch workflow and choose "Push and create a Pull Request".
