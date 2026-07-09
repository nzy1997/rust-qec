# Issue 435 Surface Detect Release Speed Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish checked release-profile speed evidence for `surface-detect-d13-r13` and verify the checker rejects detector evidence mislabeled as a sample workload.

**Architecture:** Reuse the existing generic release-speed checker and the merged multi-case speed suite. Add one focused negative-control test and one checker wording change, then generate a separate one-case release bundle for the detector workload.

**Tech Stack:** Python 3 standard library (`argparse`, `json`, `subprocess`, `tempfile`, `unittest`, `pathlib`), existing `benchmarks.rstim_vs_stim_simulator.run_speed_suite`, existing Rust `rstim` perf CLI, Cargo workspace tests.

## Global Constraints

- Results directory is exactly `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/`.
- Checked files are exactly `summary.json`, `report.md`, and `environment.json` in that directory.
- Checker command is `python3 tools/check_rstim_vs_stim_release_speed_case.py --results-dir benchmarks/rstim_vs_stim_simulator/results/release-surface-detect --case surface-detect-d13-r13 --workload detect --required-variants stim-cli,rstim-interpreted,rstim-compiled`.
- Successful checker output is exactly `PASS release speed case surface-detect-d13-r13`.
- The checker must validate that the case is present once, has workload `detect`, records all three required variants as completed, and records profile/environment metadata.
- A fixture where `surface-detect-d13-r13` has `workload = "sample"` must fail with a message containing `workload mismatch for surface-detect-d13-r13`.
- The checker must accept detect evidence from wall-time data without requiring shots-per-second.
- Do not add a hard Stim ratio gate.
- Do not change detector semantics.
- Do not modify existing checked evidence under `benchmarks/rstim_vs_stim_simulator/results/full/`, `benchmarks/rstim_vs_stim_simulator/results/release/`, or `benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/`.

---

## File Structure

- Modify `tools/check_rstim_vs_stim_release_speed_case.py`: change workload mismatch errors to include the exact negative-control signal required by issue #435.
- Modify `tools/test_check_rstim_vs_stim_release_speed_case.py`: add the detector-as-sample negative control and update the existing wrong-workload assertion to the new wording.
- Create `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/summary.json`: checked generated summary.
- Create `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/report.md`: checked generated report.
- Create `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/environment.json`: checked generated environment metadata, marked as issue #435 evidence.

### Task 1: Checker Workload Mismatch Negative Control

**Files:**
- Modify: `tools/test_check_rstim_vs_stim_release_speed_case.py`
- Modify: `tools/check_rstim_vs_stim_release_speed_case.py`

**Interfaces:**
- Consumes: CLI path `tools/check_rstim_vs_stim_release_speed_case.py`.
- Produces: workload mismatch failures containing `workload mismatch for <case>`.

- [ ] **Step 1: Write the failing detector negative-control test**

In `tools/test_check_rstim_vs_stim_release_speed_case.py`, change `run_checker` to accept the requested case and workload:

```python
    def run_checker(
        self,
        *,
        case_label: str = CASE_LABEL,
        workload: str = "sample",
        required_variants: str = REQUIRED_VARIANTS,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(CHECKER),
                "--results-dir",
                str(self.results_dir),
                "--case",
                case_label,
                "--workload",
                workload,
                "--required-variants",
                required_variants,
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
```

Then update the existing wrong-workload assertion and add the detector fixture:

```python
    def test_rejects_wrong_workload(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        case["workload"] = "detect"
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"workload mismatch for {CASE_LABEL}", result.stderr)

    def test_rejects_surface_detect_labeled_as_sample(self) -> None:
        case_label = "surface-detect-d13-r13"
        summary: dict[str, object] = {
            "cases": [
                {
                    "case_label": case_label,
                    "workload": "sample",
                    "tier": "gating",
                    "present_variants": ["rstim-compiled", "rstim-interpreted", "stim-cli"],
                    "variants": [
                        {
                            "tool_variant": "rstim-compiled",
                            "status": "completed",
                            "median_wall_time_ns": 10,
                        },
                        {
                            "tool_variant": "rstim-interpreted",
                            "status": "completed",
                            "median_wall_time_ns": 20,
                        },
                        {
                            "tool_variant": "stim-cli",
                            "status": "completed",
                            "median_wall_time_ns": 30,
                        },
                    ],
                }
            ],
            "issues": [],
        }
        environment = valid_environment()
        environment["case_labels"] = [case_label]
        self.write_bundle(summary, environment)
        self.write_report(f"# Report\n\n### {case_label}\n")
        result = self.run_checker(case_label=case_label, workload="detect")
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("workload mismatch for surface-detect-d13-r13", result.stderr)
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_release_speed_case -q
```

Expected: nonzero failure because the checker still reports `case surface-detect-d13-r13 workload must be detect`.

- [ ] **Step 3: Implement the checker wording change**

In `tools/check_rstim_vs_stim_release_speed_case.py`, replace the workload check with:

```python
    actual_workload = case.get("workload")
    if actual_workload != workload:
        raise ValueError(
            f"workload mismatch for {case_label}: expected {workload}, found {actual_workload!r}"
        )
```

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_release_speed_case -q
```

Expected: zero exit with all checker tests passing.

### Task 2: Checked Surface Detect Evidence

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/environment.json`

**Interfaces:**
- Consumes: `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite`.
- Produces: checked release evidence directory accepted by `tools/check_rstim_vs_stim_release_speed_case.py`.

- [ ] **Step 1: Generate one-case release evidence**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite \
  --profile release \
  --cases surface-detect-d13-r13 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/release-surface-detect
```

Expected: zero exit and the output directory contains `raw.jsonl`, `summary.json`, `report.md`, and `environment.json`.

- [ ] **Step 2: Remove transient raw timing stream from published evidence**

Delete:

```text
benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/raw.jsonl
```

Expected: the directory contains only `summary.json`, `report.md`, and `environment.json`.

- [ ] **Step 3: Add publication metadata**

Update `benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/environment.json` to include:

```json
{
  "evidence_kind": "surface detect release speed evidence",
  "published_artifact": true,
  "source_issue": 435
}
```

Keep all runner-generated environment fields intact.

- [ ] **Step 4: Run the issue-required checker command**

Run:

```sh
python3 tools/check_rstim_vs_stim_release_speed_case.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  --case surface-detect-d13-r13 \
  --workload detect \
  --required-variants stim-cli,rstim-interpreted,rstim-compiled
```

Expected stdout:

```text
PASS release speed case surface-detect-d13-r13
```

### Task 3: Final Verification and Commit

**Files:**
- Modify: files from Tasks 1 and 2.

**Interfaces:**
- Consumes: repository test commands.
- Produces: a scoped implementation commit ready for PR.

- [ ] **Step 1: Run all required verification commands**

Run:

```sh
python3 tools/check_rstim_vs_stim_release_speed_case.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  --case surface-detect-d13-r13 \
  --workload detect \
  --required-variants stim-cli,rstim-interpreted,rstim-compiled
python3 -m unittest tools.test_check_rstim_vs_stim_release_speed_case -q
cargo test
git diff --check
```

Expected: the checker prints `PASS release speed case surface-detect-d13-r13`; unit tests pass; `cargo test` passes; `git diff --check` exits zero.

- [ ] **Step 2: Inspect changed files**

Run:

```sh
git status --short
git diff --stat
```

Expected: changes are limited to the checker, checker tests, the new surface-detect evidence directory, and this Superpowers plan.

- [ ] **Step 3: Commit implementation**

Run:

```sh
git add tools/check_rstim_vs_stim_release_speed_case.py \
  tools/test_check_rstim_vs_stim_release_speed_case.py \
  benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  docs/superpowers/plans/2026-07-10-issue-435-surface-detect-release-speed-evidence.md
git commit -m "feat: publish surface detect release speed evidence"
```

Expected: a commit containing all implementation and evidence changes.
