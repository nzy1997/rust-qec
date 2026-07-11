# Issue 454 Compiled Steady Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a checked compiled steady-state evidence bundle and checker that derive lifecycle and measured timing claims from raw #453 worker telemetry.

**Architecture:** Add one Python checker that parses raw #453 records, maps worker labels to #454 release labels, derives canonical summary/report from measured records, validates environment provenance against derived values, then verifies artifact hashes last. Add unit tests that build valid synthetic bundles and mutate raw records or artifacts to prove semantic failures are caught before hash mismatches.

**Tech Stack:** Python 3 standard library, existing `benchmarks.rstim_vs_stim_simulator.run_compiled_steady`, existing `benchmarks.rstim_vs_stim_simulator.fair_cli_contract`, repository Cargo workspace tests.

## Global Constraints

- Publish under `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/`.
- Required files are exactly `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, and `artifact-sha256.json`.
- `artifact-sha256.json` maps the other four files to lowercase SHA-256 digests.
- Validate semantics before artifact hashes.
- For each release variant `stim-compiled-steady-b8` and `rstim-compiled-steady-b8`, require ready compile/reference/sample counts `1/1/0`, two warmup and seven measured sample records with request IDs `0-8`, response sample-call counts `1-9`, final lifecycle `1/1/9`, 1024 shots, 12,121 measurements, `b8`, 1,516 bytes/shot, and 1,552,384 response bytes per sample.
- Cross-check ready, request, response, final, environment, provenance, summary, and report records.
- `environment.json` must agree with derived lifecycle values but cannot source them.
- Regenerate canonical `summary.json` and `report.md` from raw measured records and require byte-for-byte equality.
- Require #453 provenance fields and verify fixture, fair/source manifest, worker binary/module, Python, and Stim-extension hashes before verifying `artifact-sha256.json`.
- Do not update the site manifest, overwrite earlier evidence, create a cross-machine wall-clock gate, or claim lifecycle behavior not present in raw worker telemetry.

---

### Task 1: Checker And Unit Tests

**Files:**
- Create: `tools/check_rstim_vs_stim_compiled_steady_evidence.py`
- Create: `tools/test_check_rstim_vs_stim_compiled_steady_evidence.py`

**Interfaces:**
- Consumes: #453 raw records and environment schema from `benchmarks.rstim_vs_stim_simulator.run_compiled_steady`.
- Produces: CLI `tools/check_rstim_vs_stim_compiled_steady_evidence.py --dir <path>` with success text `PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9`.

- [ ] **Step 1: Write failing tests**

Add `tools/test_check_rstim_vs_stim_compiled_steady_evidence.py` with helpers to write a valid synthetic bundle and tests for:

```python
def test_accepts_valid_bundle(self) -> None: ...
def test_rejects_missing_raw_request_even_when_environment_claims_lifecycle(self) -> None: ...
def test_rejects_duplicate_request_id(self) -> None: ...
def test_rejects_changed_cumulative_call_count(self) -> None: ...
def test_rejects_final_compile_count_semantically_before_hashes(self) -> None: ...
def test_rejects_rehashed_summary_not_derived_from_raw(self) -> None: ...
def test_rejects_missing_hash_manifest(self) -> None: ...
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
```

Expected: fail because `tools/check_rstim_vs_stim_compiled_steady_evidence.py` does not exist.

- [ ] **Step 3: Implement checker**

Implement these functions in `tools/check_rstim_vs_stim_compiled_steady_evidence.py`:

```python
sha256_file(path: Path) -> str
load_json_object(path: Path, label: str) -> dict[str, Any]
load_raw_records(path: Path) -> list[dict[str, Any]]
validate_required_files(results_dir: Path) -> None
validate_raw_semantics(records: list[dict[str, Any]]) -> dict[str, Any]
derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]
render_report(summary: dict[str, Any]) -> str
validate_environment(environment: dict[str, Any], derived: dict[str, Any], records: list[dict[str, Any]]) -> None
validate_artifact_hashes(results_dir: Path) -> None
validate_bundle(results_dir: Path) -> tuple[int, int, str]
main(argv: list[str] | None = None) -> int
```

Use existing `run_compiled_steady._summary()` and `_render_report()` formats only if they can preserve byte-for-byte canonical output from raw measured records. Otherwise keep the same simple report shape and validate the committed bundle against that renderer.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
```

Expected: pass.

- [ ] **Step 5: Commit**

```sh
git add tools/check_rstim_vs_stim_compiled_steady_evidence.py tools/test_check_rstim_vs_stim_compiled_steady_evidence.py
git commit -m "test: check compiled steady evidence semantics"
```

### Task 2: Publish Release Bundle

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/raw.jsonl`
- Create: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/environment.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/artifact-sha256.json`

**Interfaces:**
- Consumes: #453 runner command and checker from Task 1.
- Produces: checked release-profile compiled steady-state evidence bundle.

- [ ] **Step 1: Generate release-profile evidence**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_compiled_steady \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 --profile release \
  --warmup-rounds 2 --measure-rounds 7 --seed 0 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
```

Expected stdout contains:

```text
PASS compiled steady-state lifecycle variants=2 compile=1 reference=1 calls=9 measured=14
```

- [ ] **Step 2: Write artifact hash manifest**

Write `artifact-sha256.json` with lowercase SHA-256 digests for:

```text
raw.jsonl
summary.json
report.md
environment.json
```

- [ ] **Step 3: Verify bundle with checker**

Run:

```sh
python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
```

Expected:

```text
PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9
```

- [ ] **Step 4: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
git commit -m "data: publish compiled steady evidence"
```

### Task 3: Final Verification And PR

**Files:**
- Modify only if verification exposes a real issue in Task 1 or Task 2 files.

**Interfaces:**
- Consumes: committed checker and bundle.
- Produces: pushed worker branch and pull request.

- [ ] **Step 1: Run required checker**

```sh
python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
```

Expected:

```text
PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9
```

- [ ] **Step 2: Run required unit tests**

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
```

Expected: all tests pass.

- [ ] **Step 3: Run Cargo verification**

```sh
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 4: Commit workflow docs if kept in scope**

```sh
git add docs/superpowers/specs/2026-07-11-issue-454-compiled-steady-evidence-design.md docs/superpowers/plans/2026-07-11-issue-454-compiled-steady-evidence.md
git commit -m "docs: plan compiled steady evidence"
```

- [ ] **Step 5: Push and create PR**

```sh
git push -u origin agent/issue-454-publish-compiled-steady-state-sampling-evidence-run-1
gh pr create --base master --head agent/issue-454-publish-compiled-steady-state-sampling-evidence-run-1 --title "Publish compiled steady-state sampling evidence" --body-file <generated-body>
```

Expected: PR URL is printed.
