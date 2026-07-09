# Issue 436 DEM Sampling Benchmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a checked d11/r100 detector-error-model sampling benchmark comparing `stim sample_dem` with `rstim sample_dem`.

**Architecture:** Keep the DEM benchmark as a Python-side runner and checker that mirror the existing selected speed artifact shape without expanding the Rust perf registry. The runner validates pinned DEM provenance, builds the selected `rstim` profile, times the two CLI variants against the same DEM stdin, and writes `raw.jsonl`, `summary.json`, `report.md`, and `environment.json`.

**Tech Stack:** Python standard library `argparse`, `hashlib`, `json`, `statistics`, `subprocess`, `tempfile`, `unittest`; existing `benchmarks.rstim_vs_stim_simulator.run_speed_case` helpers; existing `rstim sample_dem` CLI; Stim CLI.

## Global Constraints

- Use case label `stim-style-surface-dem-sample-d11-r100-b1024`.
- Required variants are exactly visible as `stim-sample-dem` and `rstim-sample-dem`.
- Required workload label is `sample_dem`, separate from circuit `sample` and `detect`.
- Expected detector count is `12000`.
- Expected observable count is `1`.
- Shot count is `1024`.
- Checked result directory is `benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/`.
- Reject bad DEM metadata with a message containing `DEM metadata mismatch`.
- Do not add speed thresholds.
- Do not optimize DEM sampling.
- Do not claim this circuit-sampled DEM benchmark represents all `sample_dem` workloads.

---

## File Structure

- Create `benchmarks/rstim_vs_stim_simulator/run_dem_speed_case.py`: CLI runner, DEM metadata validator, raw record emitter, summary/report/environment writers.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_dem_speed_case.py`: unit tests for metadata validation, command construction, artifact shape, and negative controls.
- Create `tools/check_rstim_vs_stim_release_dem_speed_case.py`: focused checker for issue #437 consumption.
- Create `tools/test_check_rstim_vs_stim_release_dem_speed_case.py`: unit tests for checker success and failure modes.
- Create `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem`: pinned DEM generated from the checked circuit.
- Create `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json`: provenance and hash metadata.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document the DEM runner/checker command.
- Create checked output files under `benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/`: `raw.jsonl`, `summary.json`, `report.md`, `environment.json`.

## Task 1: DEM Metadata Validation Contract

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_dem_speed_case.py`
- Create: `benchmarks/rstim_vs_stim_simulator/run_dem_speed_case.py`
- Create: `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json`

**Interfaces:**
- Produces: `DemCase` dataclass with fields `label: str`, `dem_path: Path`, `metadata_path: Path`, `shots: int`, `expected_detectors: int`, `expected_observables: int`.
- Produces: `case_by_label(label: str) -> DemCase`.
- Produces: `load_and_validate_dem_case(case: DemCase) -> tuple[str, dict[str, object]]`.
- Later tasks consume the returned DEM text and metadata dictionary.

- [ ] **Step 1: Write failing metadata validation tests**

Add tests that create a temporary DEM and metadata file, then assert that valid metadata loads and a mismatched detector or observable count raises `ValueError` containing `DEM metadata mismatch`.

```python
def test_load_and_validate_dem_case_rejects_bad_counts(self) -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        dem_path = root / "case.dem"
        metadata_path = root / "case.dem.metadata.json"
        dem_path.write_text("error(0.1) D0 L0\n")
        dem_hash = run_dem_speed_case.sha256_file(dem_path)
        metadata_path.write_text(json.dumps({
            "case_label": "stim-style-surface-dem-sample-d11-r100-b1024",
            "dem_path": str(dem_path),
            "dem_sha256": dem_hash,
            "expected_detectors": 11999,
            "expected_observables": 1,
            "shots": 1024,
            "source_circuit_path": "fixtures/source.stim",
            "source_circuit_sha256": "0" * 64,
            "generation_command": "stim analyze_errors --decompose_errors < source.stim > case.dem",
        }) + "\n")
        case = run_dem_speed_case.DemCase(
            label="stim-style-surface-dem-sample-d11-r100-b1024",
            dem_path=dem_path,
            metadata_path=metadata_path,
            shots=1024,
            expected_detectors=12000,
            expected_observables=1,
        )

        with self.assertRaisesRegex(ValueError, "DEM metadata mismatch"):
            run_dem_speed_case.load_and_validate_dem_case(case)
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_dem_speed_case -q
```

Expected before implementation: import or attribute failure for `run_dem_speed_case`.

- [ ] **Step 3: Implement the metadata validator**

Create `run_dem_speed_case.py` with the case constants, `sha256_file`, `case_by_label`, and `load_and_validate_dem_case`. The validator must compare metadata label, DEM path hash, source circuit hash when the source path exists, shot count, expected detector count, and expected observable count. Every metadata disagreement must raise `ValueError("DEM metadata mismatch: ...")`.

- [ ] **Step 4: Add the real metadata file after generating or locating the DEM**

Generate the DEM with:

```sh
stim analyze_errors --decompose_errors < benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim > benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem
```

Record the exact generation command, source circuit hash, DEM hash, detector count `12000`, observable count `1`, and shots `1024` in `stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json`.

- [ ] **Step 5: Run focused validation tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_dem_speed_case -q
```

Expected after implementation: tests pass.

- [ ] **Step 6: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/run_dem_speed_case.py benchmarks/rstim_vs_stim_simulator/tests/test_run_dem_speed_case.py benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json
git commit -m "feat: add DEM sampling metadata validation"
```

## Task 2: DEM Runner Artifacts

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_dem_speed_case.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_dem_speed_case.py`

**Interfaces:**
- Consumes: `load_and_validate_dem_case(case: DemCase) -> tuple[str, dict[str, object]]`.
- Produces: `run_dem_speed_case(args: argparse.Namespace, repo_root: Path = REPO_ROOT, command_line: list[str] | None = None) -> None`.
- Produces: `summarize_records(records: list[dict[str, object]], case: DemCase) -> dict[str, object]`.
- Produces: `render_report(summary: dict[str, object]) -> str`.
- The checker consumes the produced JSON fields.

- [ ] **Step 1: Write failing runner workflow tests**

Mock `build_rstim` and `subprocess.run`. Assert the runner invokes:

```python
["stim", "sample_dem", "--shots", "1024"]
[str(binary), "sample_dem", "--shots", "1024"]
```

with DEM text on stdin, `stdout=subprocess.DEVNULL`, and writes `raw.jsonl`, `summary.json`, `report.md`, and `environment.json`. Assert the summary case has `present_variants == ["rstim-sample-dem", "stim-sample-dem"]` after sorting and every variant status is `completed`.

- [ ] **Step 2: Run the focused runner tests and confirm failure**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_dem_speed_case -q
```

Expected before implementation: missing `run_dem_speed_case` or missing artifact fields.

- [ ] **Step 3: Implement timed variant execution**

Add a helper `run_timed_command(command: list[str], dem_text: str) -> tuple[int, str, int]` that measures `time.perf_counter_ns()`, runs the command with `input=dem_text`, `text=True`, `stdout=subprocess.DEVNULL`, `stderr=subprocess.PIPE`, and returns `(returncode, stderr, elapsed_ns)`. Convert nonzero return codes into raw records with status `tool_failed`; completed commands have status `completed`.

- [ ] **Step 4: Implement raw, summary, report, and environment writes**

Write one raw JSONL record per variant per warmup or measured round. Summary must include:

```json
{
  "cases": [{
    "case_label": "stim-style-surface-dem-sample-d11-r100-b1024",
    "workload": "sample_dem",
    "tier": "report_only",
    "expected_variants": ["stim-sample-dem", "rstim-sample-dem"],
    "present_variants": ["rstim-sample-dem", "stim-sample-dem"],
    "variants": [{
      "tool_variant": "stim-sample-dem",
      "status": "completed",
      "sample_count": 1
    }]
  }],
  "issues": []
}
```

Environment must include `profile`, `case_label`, `case_labels`, `case_count`,
`command_line`, `dem_path`, `dem_sha256`, `source_circuit_path`,
`source_circuit_sha256`, `expected_detectors`, `expected_observables`, and
existing base environment fields from `run_speed_case.collect_suite_environment`.

- [ ] **Step 5: Run focused tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_dem_speed_case -q
```

Expected after implementation: tests pass.

- [ ] **Step 6: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/run_dem_speed_case.py benchmarks/rstim_vs_stim_simulator/tests/test_run_dem_speed_case.py
git commit -m "feat: add DEM sampling speed runner"
```

## Task 3: Release DEM Checker

**Files:**
- Create: `tools/check_rstim_vs_stim_release_dem_speed_case.py`
- Create: `tools/test_check_rstim_vs_stim_release_dem_speed_case.py`

**Interfaces:**
- Consumes: runner artifacts under a results directory.
- Produces: CLI `main(argv: list[str] | None = None) -> int`.
- Produces success line `PASS release DEM speed case <case>`.

- [ ] **Step 1: Write failing checker tests**

Create temporary artifact directories. A success fixture includes all four files, a summary with completed `stim-sample-dem` and `rstim-sample-dem`, and environment metadata. A negative fixture removes one variant and must exit nonzero with `missing required variant rstim-sample-dem`.

- [ ] **Step 2: Run checker tests and confirm failure**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_release_dem_speed_case -q
```

Expected before implementation: import failure.

- [ ] **Step 3: Implement checker validation**

Parse `--results-dir`, `--case`, and `--required-variants`. Check required files, parse `summary.json` and `environment.json`, find the requested case, require the variants with status `completed`, require `issues == []`, and require environment fields for profile, case label, hashes, detector count, and observable count. Return `1` with a direct error message for failures.

- [ ] **Step 4: Run checker tests**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_release_dem_speed_case -q
```

Expected after implementation: tests pass.

- [ ] **Step 5: Commit**

```sh
git add tools/check_rstim_vs_stim_release_dem_speed_case.py tools/test_check_rstim_vs_stim_release_dem_speed_case.py
git commit -m "feat: add release DEM speed checker"
```

## Task 4: Publish Checked DEM Results And Docs

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/raw.jsonl`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/environment.json`

**Interfaces:**
- Consumes: runner CLI and checker CLI from Tasks 2 and 3.
- Produces: checked artifacts committed in the required result directory.

- [ ] **Step 1: Add README command block**

Document the DEM runner and checker command exactly enough for a user to reproduce the checked evidence directory. State that it is report-only evidence with no speed threshold.

- [ ] **Step 2: Run the release DEM runner**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_dem_speed_case --profile release --case stim-style-surface-dem-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
```

Expected: command exits `0` and writes the four artifact files.

- [ ] **Step 3: Run the release DEM checker**

Run:

```sh
python3 tools/check_rstim_vs_stim_release_dem_speed_case.py --results-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample --case stim-style-surface-dem-sample-d11-r100-b1024 --required-variants stim-sample-dem,rstim-sample-dem
```

Expected: prints `PASS release DEM speed case stim-style-surface-dem-sample-d11-r100-b1024`.

- [ ] **Step 4: Commit docs and checked results**

```sh
git add benchmarks/rstim_vs_stim_simulator/README.md benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
git commit -m "docs: publish DEM sampling speed evidence"
```

## Task 5: Final Verification

**Files:**
- Uses the complete branch.

**Interfaces:**
- Confirms all required issue commands and repository tests pass.

- [ ] **Step 1: Run required issue commands**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_dem_speed_case --profile release --case stim-style-surface-dem-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
python3 tools/check_rstim_vs_stim_release_dem_speed_case.py --results-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample --case stim-style-surface-dem-sample-d11-r100-b1024 --required-variants stim-sample-dem,rstim-sample-dem
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_dem_speed_case -q
python3 -m unittest tools.test_check_rstim_vs_stim_release_dem_speed_case -q
cargo test
```

Expected: the runner exits `0`, the checker prints `PASS release DEM speed case stim-style-surface-dem-sample-d11-r100-b1024`, both unit test modules pass, and `cargo test` passes.

- [ ] **Step 2: Inspect git state**

Run:

```sh
git status --short
```

Expected: only intended checked result files changed by the final runner rerun, or a clean tree after committing those deterministic updates.

- [ ] **Step 3: Commit final deterministic artifact refresh when needed**

If the final runner changed checked artifact files, run:

```sh
git add benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
git commit -m "docs: refresh DEM sampling speed evidence"
```

Expected: a commit is created only when the final run changed committed artifacts.

## Self Review

- Spec coverage: fixture metadata, runner, checker, checked result directory, docs, required positive commands, and required negative metadata control all have tasks.
- Placeholder scan: no placeholder markers, incomplete steps, or deferred edge cases remain.
- Type consistency: `DemCase`, `case_by_label`, `load_and_validate_dem_case`, `run_dem_speed_case`, `summarize_records`, and `render_report` are named consistently across tasks.
