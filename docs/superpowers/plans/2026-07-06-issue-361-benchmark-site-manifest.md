# Issue 361 Benchmark Site Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a checked benchmark documentation site manifest plus a validator that enforces checked-artifact, provenance, and claims-limit policy.

**Architecture:** `site/benchmark-site.json` is the static-site data source. `tools/check_site_manifest.py` validates the JSON schema and git-backed artifact policy using only the Python standard library, while `tools/test_check_site_manifest.py` gives focused TDD coverage for the validator and its self-test.

**Tech Stack:** JSON, Python standard library (`argparse`, `json`, `subprocess`, `tempfile`, `unittest`, `pathlib`), git CLI, existing benchmark Markdown docs and checked CSV/PNG/Markdown artifacts.

## Global Constraints

- Create `site/benchmark-site.json`.
- Create `tools/check_site_manifest.py`.
- Create `tools/test_check_site_manifest.py`.
- Use JSON and the Python standard library only for the manifest validator.
- Family fields must include `id`, `title`, `status`, `source_docs`, `claims_limit`, and `evidence_items`.
- Evidence item fields must include `id`, `title`, `status`, `tier`, `artifacts`, `commands`, `provenance_requirements`, `provenance_sources`, and `claims_limit`.
- Allowed family and evidence statuses: `existing`, `partial`, `future`, `local-only`.
- Required family IDs from #360: `surface-decoder-comparison`, `bb-circuit-bposd-comparison`, `qec-code-random-window`, `rstim-vs-stim-simulator`, and `internal-regression-evidence`.
- Checked artifact entries must have `path`, `kind`, and `checked: true`.
- Checked artifact paths must exist, be tracked by git, and not be ignored.
- Source docs must exist, be tracked by git, and not be ignored.
- Local-only and future evidence items must not list checked artifacts.
- Local-only and future entries may list commands and source docs, but must not present ignored generated outputs as checked artifacts.
- The self-test must mutate one manifest entry to reference `benchmarks/missing/results.csv`, one entry to omit `claims_limit`, and one checked artifact entry to point under `benchmarks/out/`.
- Each negative-control mutation must be rejected with an error naming the bad entry id and the violated rule.
- Do not implement site rendering.
- Do not run new benchmark campaigns.
- Do not commit generated benchmark outputs under `benchmarks/out/`.

---

### Task 1: Add Site Manifest Validator And Focused Tests

**Files:**
- Create: `tools/check_site_manifest.py`
- Create: `tools/test_check_site_manifest.py`

**Interfaces:**
- Consumes: `validate_manifest(repo_root: Path, manifest_path: Path) -> list[str]`
- Consumes: CLI `python3 tools/check_site_manifest.py --self-test`
- Consumes: CLI `python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json`
- Produces: stderr validation errors naming the bad family or evidence item id.
- Produces: stdout success lines including every accepted family id.

- [ ] **Step 1: Write the failing tests**

Create `tools/test_check_site_manifest.py` with a `SiteManifestValidatorTest` class. Build a temporary git repository helper that writes `.gitignore` with `/benchmarks/out/`, creates checked docs and artifacts, runs `git init -q`, and stages the checked fixture files with `git add`.

Use this valid manifest fixture shape in the test helper:

```python
{
    "schema_version": 1,
    "families": [
        {
            "id": "surface-decoder-comparison",
            "title": "Surface Decoder Comparison",
            "status": "existing",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "Checked full artifacts are committed-run evidence, not a general decoder ordering claim.",
            "evidence_items": [
                {
                    "id": "surface-decoder-full",
                    "title": "Checked surface-decoder full artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [
                        {
                            "path": "benchmarks/surface_decoder_compare/results/full/results.csv",
                            "kind": "csv",
                            "checked": True,
                        }
                    ],
                    "commands": ["make surface-decoder-compare-full"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "bb-circuit-bposd-comparison",
            "title": "BB Circuit BP-OSD Comparison",
            "status": "partial",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "BB72/BB144 only.",
            "evidence_items": [
                {
                    "id": "bb-circuit-full",
                    "title": "Checked BB full artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [],
                    "commands": ["make bb-circuit-bposd-compare-full"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "qec-code-random-window",
            "title": "qec-code Random Window",
            "status": "local-only",
            "source_docs": ["benchmarks/qec_code_random_window/README.md"],
            "claims_limit": "Generated outputs are ignored local evidence.",
            "evidence_items": [
                {
                    "id": "qec-code-smoke",
                    "title": "Local smoke command",
                    "status": "local-only",
                    "tier": "smoke",
                    "artifacts": [],
                    "commands": ["make qec-code-random-window-bench-smoke"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["benchmarks/qec_code_random_window/README.md"],
                    "claims_limit": "Local wiring check only.",
                }
            ],
        },
        {
            "id": "rstim-vs-stim-simulator",
            "title": "rstim versus Stim Simulator",
            "status": "future",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "No current site-facing benchmark artifacts.",
            "evidence_items": [
                {
                    "id": "rstim-stim-future",
                    "title": "Future simulator benchmark",
                    "status": "future",
                    "tier": "future",
                    "artifacts": [],
                    "commands": [],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Planning entry only.",
                }
            ],
        },
        {
            "id": "internal-regression-evidence",
            "title": "Internal Regression Evidence",
            "status": "partial",
            "source_docs": [".github/workflows/ci.yml"],
            "claims_limit": "Regression gate evidence only.",
            "evidence_items": [
                {
                    "id": "rstim-perf-ci",
                    "title": "rstim perf CI",
                    "status": "partial",
                    "tier": "regression-gate",
                    "artifacts": [],
                    "commands": ["cargo run -p rstim --bin rstim -- perf ci --out-dir perf-artifacts"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": [".github/workflows/ci.yml"],
                    "claims_limit": "Regression gate evidence only.",
                }
            ],
        },
    ],
}
```

Test cases:

```python
def test_accepts_valid_fixture_and_reports_families(self):
    repo, manifest_path = self.write_fixture_manifest()
    errors = check_site_manifest.validate_manifest(repo, manifest_path)
    self.assertEqual(errors, [])

def test_rejects_missing_required_family(self):
    repo, manifest_path = self.write_fixture_manifest(remove_family="rstim-vs-stim-simulator")
    errors = check_site_manifest.validate_manifest(repo, manifest_path)
    self.assertTrue(any("manifest" in error and "rstim-vs-stim-simulator" in error for error in errors))

def test_rejects_negative_control_mutations(self):
    for mutation, entry_id, rule in [
        ("missing_artifact", "surface-decoder-full", "does not exist"),
        ("missing_claims_limit", "surface-decoder-full", "claims_limit"),
        ("ignored_artifact", "surface-decoder-full", "ignored"),
    ]:
        repo, manifest_path = self.write_fixture_manifest(mutation=mutation)
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

def test_self_test_exercises_negative_controls(self):
    self.assertEqual(check_site_manifest.run_self_test(), [])
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: FAIL with `ImportError`, `AttributeError`, or missing file errors because `tools/check_site_manifest.py` does not exist yet.

- [ ] **Step 3: Implement the validator**

Create `tools/check_site_manifest.py` with:

```python
ALLOWED_STATUSES = {"existing", "partial", "future", "local-only"}
REQUIRED_FAMILY_IDS = {
    "surface-decoder-comparison",
    "bb-circuit-bposd-comparison",
    "qec-code-random-window",
    "rstim-vs-stim-simulator",
    "internal-regression-evidence",
}
FAMILY_REQUIRED_FIELDS = {"id", "title", "status", "source_docs", "claims_limit", "evidence_items"}
ITEM_REQUIRED_FIELDS = {
    "id",
    "title",
    "status",
    "tier",
    "artifacts",
    "commands",
    "provenance_requirements",
    "provenance_sources",
    "claims_limit",
}
```

Implement helpers:

```python
def git_ok(repo_root: Path, args: list[str]) -> bool:
    result = subprocess.run(["git", *args], cwd=repo_root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return result.returncode == 0

def path_is_tracked(repo_root: Path, relative: str) -> bool:
    return git_ok(repo_root, ["ls-files", "--error-unmatch", "--", relative])

def path_is_ignored(repo_root: Path, relative: str) -> bool:
    return git_ok(repo_root, ["check-ignore", "-q", "--", relative])
```

`validate_manifest(repo_root, manifest_path)` should parse JSON, validate required fields and IDs, validate source docs, validate evidence items, and return a list of error strings. For local-only and future evidence items, append an error if `artifacts` is non-empty. For checked artifacts, reject missing files, untracked files, ignored paths, missing `checked`, or `checked` values other than `True`.

Implement `run_self_test()` by creating a temporary git fixture, validating the good manifest, then applying the three issue-required mutations and checking that each mutation is rejected with the expected entry id and rule text.

Implement CLI parsing with mutually exclusive modes:

```bash
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
```

On success, print one line per family:

```text
ok: family surface-decoder-comparison status=existing items=2
```

On failure, print `error: ...` lines to stderr and exit 1.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
```

Expected: PASS. The self-test must print `ok: self-test`.

- [ ] **Step 5: Commit**

Run:

```bash
git add tools/check_site_manifest.py tools/test_check_site_manifest.py
git commit -m "test: add benchmark site manifest validator"
```

Expected: commit succeeds.

---

### Task 2: Add Benchmark Site Manifest Data

**Files:**
- Create: `site/benchmark-site.json`

**Interfaces:**
- Consumes: `docs/showcases/benchmark-evidence.md`
- Consumes: `docs/showcases/qec-code-random-window-benchmark.md`
- Consumes: `benchmarks/surface_decoder_compare/results/full/*`
- Consumes: `benchmarks/bb_circuit_bposd_compare/results/full/*`
- Consumes: `benchmarks/qec_code_random_window/README.md`
- Consumes: `.github/workflows/ci.yml`
- Consumes: `.github/workflows/rbposd-parity.yml`
- Consumes: `rstim/doc/performance_parity.md`
- Produces: `site/benchmark-site.json`

- [ ] **Step 1: Write the manifest**

Create `site/benchmark-site.json` with `schema_version: 1` and the five required family IDs.

For checked artifact entries, include only these tracked artifacts:

```json
[
  "benchmarks/surface_decoder_compare/results/full/results.csv",
  "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
  "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
  "benchmarks/bb_circuit_bposd_compare/results/full/summary.md",
  "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
  "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md"
]
```

For `qec-code-random-window`, list commands including:

```text
make qec-code-random-window-bench-smoke
make qec-code-random-window-bench-full
make qec-code-random-window-bench-no-target-smoke
make qec-code-random-window-bench-no-target-multiseed-smoke
make qec-code-random-window-bench-no-target-ladder-smoke
make qec-code-random-window-bench-issue225-readiness-smoke
```

For `rstim-vs-stim-simulator`, use `status: "future"` and no artifacts.

For `internal-regression-evidence`, include evidence items for `rstim-perf-ci` and `rbposd-parity-gate`, with claims limiting them to regression/compatibility evidence.

Use this provenance requirement list for checked benchmark evidence unless a narrower item-specific list is clearer:

```json
[
  "OS",
  "CPU model",
  "Rust version",
  "Python version",
  "dependency versions",
  "external repository commits",
  "command line",
  "seeds",
  "build profile",
  "shots or error budgets",
  "date"
]
```

- [ ] **Step 2: Run validator**

Run:

```bash
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
```

Expected: PASS and stdout names all five required families.

- [ ] **Step 3: Commit**

Run:

```bash
git add site/benchmark-site.json
git commit -m "docs: add benchmark site manifest"
```

Expected: commit succeeds.

---

### Task 3: Final Verification

**Files:**
- No new files.

**Interfaces:**
- Consumes all files from Tasks 1 and 2.
- Produces verification evidence for the PR description.

- [ ] **Step 1: Run required manifest checks**

Run:

```bash
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
```

Expected: both commands exit 0. The manifest validation output names all five required family IDs.

- [ ] **Step 2: Run focused Python unit tests**

Run:

```bash
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: PASS.

- [ ] **Step 3: Run repository Rust verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Check final diff**

Run:

```bash
git status --short
git diff --stat origin/master..HEAD
```

Expected: clean worktree and a diff containing only the Superpowers design/plan docs, the validator/test, and `site/benchmark-site.json`.
