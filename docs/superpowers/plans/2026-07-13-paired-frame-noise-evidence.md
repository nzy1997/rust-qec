# Paired Frame-Noise Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish same-run baseline/candidate timing evidence for `frame-instruction-wide-release` and enforce the issue #493 non-regression gate.

**Architecture:** Extend the existing paired runner summary/report so the timing outcome is self-contained, then extend `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py` to validate `paired-*` artifacts beside the existing deterministic telemetry. Preserve old telemetry, correctness, environment, and runtime identity validation.

**Tech Stack:** Python standard library, `unittest`, existing `benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise`, existing portable evidence catalog.

## Global Constraints

- Preserve current `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, `fixture-load.json`, and `correctness-summary.json` semantics for `frame-instruction-wide-release`.
- Add `paired-raw.jsonl`, `paired-summary.json`, and `paired-report.md`.
- Derive `candidate_over_baseline = candidate_median / baseline_median`.
- Classify `improved` at ratio `<= 0.95`, `neutral` at `0.95 < ratio <= 1.05`, and `regressed` above `1.05`.
- Checked evidence requires ratio `<= 1.05`.
- Continue requiring 803 candidate iterator builds, 80,362 legacy setups, 82,290,688 attempts, correctness `pass`, complete 1,552,384-byte output, and baseline revision `f10d1ed024d3519318ed244c9095724074519595`.
- Do not claim a universal speedup, update site metadata, or compare different machines.

---

### Task 1: Runner Ratio and Outcome Fields

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_paired_frame_noise.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_paired_frame_noise.py`

**Interfaces:**
- Produces: `classify_candidate_ratio(ratio: float) -> str`
- Produces: summary fields `candidate_over_baseline: float` and `outcome: str`
- Consumes: existing paired runner summary variant records

- [ ] **Step 1: Write failing runner tests**

Add assertions to `test_runner_writes_paired_artifacts_and_alternates_order`:

```python
self.assertIn(summary["outcome"], {"improved", "neutral", "regressed"})
self.assertEqual(
    summary["candidate_over_baseline"],
    summary["variants"][1]["median_elapsed_ns"] / summary["variants"][0]["median_elapsed_ns"],
)
```

Add a direct classification test:

```python
def test_classifies_candidate_over_baseline_ratio(self) -> None:
    self.assertEqual(run_paired_frame_noise.classify_candidate_ratio(0.95), "improved")
    self.assertEqual(run_paired_frame_noise.classify_candidate_ratio(0.9500001), "neutral")
    self.assertEqual(run_paired_frame_noise.classify_candidate_ratio(1.05), "neutral")
    self.assertEqual(run_paired_frame_noise.classify_candidate_ratio(1.0500001), "regressed")
```

- [ ] **Step 2: Verify red**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_paired_frame_noise`

Expected: FAIL because `classify_candidate_ratio`, `candidate_over_baseline`, and `outcome` do not exist.

- [ ] **Step 3: Implement minimal runner support**

Add:

```python
def classify_candidate_ratio(ratio: float) -> str:
    if ratio <= 0.95:
        return "improved"
    if ratio <= 1.05:
        return "neutral"
    return "regressed"
```

In `_summary`, after constructing the two variant dictionaries:

```python
baseline_median = variants[0]["median_elapsed_ns"]
candidate_median = variants[1]["median_elapsed_ns"]
candidate_over_baseline = candidate_median / baseline_median
```

Add `candidate_over_baseline` and `outcome` to the returned summary.

Update `_report` to include:

```python
f"Candidate over baseline: `{summary['candidate_over_baseline']}`",
f"Outcome: `{summary['outcome']}`",
```

- [ ] **Step 4: Verify green**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_paired_frame_noise`

Expected: PASS.

### Task 2: Checker Validation for Paired Artifacts

**Files:**
- Modify: `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py`
- Modify: `tools/check_all_portable_evidence.py`
- Modify: `tools/test_check_all_portable_evidence.py`

**Interfaces:**
- Produces: `validate_paired_evidence(records, summary, report) -> dict[str, Any]`
- Produces: checker return value containing builds, attempts, legacy setups, outcome, and ratio
- Consumes: `paired-raw.jsonl`, `paired-summary.json`, `paired-report.md`

- [ ] **Step 1: Write failing checker tests**

Extend the test fixture to write paired artifacts. Add tests:

```python
def test_rejects_paired_classification_mismatch(self) -> None:
    rewrite_json(self.bundle / "paired-summary.json", lambda payload: payload.update({"outcome": "improved"}))
    rewrite_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("paired-summary outcome must be neutral", result.stderr)
```

```python
def test_rejects_paired_candidate_regression_limit_before_hash_error(self) -> None:
    rewrite_json(
        self.bundle / "paired-summary.json",
        lambda payload: (
            payload["variants"][1].update({"median_elapsed_ns": 1100, "mean_elapsed_ns": 1100}),
            payload.update({"candidate_over_baseline": 1.1, "outcome": "regressed"}),
        ),
    )
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("candidate frame-noise path exceeds 1.05 non-regression limit", result.stderr)
    self.assertNotIn("artifact", result.stderr.lower())
```

Keep the failed correctness negative control asserting no artifact hash error.

- [ ] **Step 2: Verify red**

Run: `python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence`

Expected: FAIL because paired files are not required or validated.

- [ ] **Step 3: Implement paired validation**

Update required artifact lists to include `paired-raw.jsonl`, `paired-summary.json`, and `paired-report.md`.

Add paired raw validation for:

- exactly 18 records
- variants `baseline-rstim-frame-noise-b8` and `candidate-rstim-frame-noise-b8`
- two warmup and seven measured records per variant
- seeds 0 through 8
- canonical `--skip_reference_sample` b8 command
- expected and actual output bytes `1_552_384`
- timer scope `process_spawn_stdout_stderr_drain_exit`
- baseline resolved revision `f10d1ed024d3519318ed244c9095724074519595`

Add paired summary/report derivation and return the ratio/outcome from `validate_bundle`.

- [ ] **Step 4: Update aggregate checker output**

Change `_instruction_wide_pass_line` to print:

```text
PASS instruction-wide frame-noise evidence outcome=<outcome> builds=803 attempts=82290688 legacy_setups=80362 candidate_over_baseline=<ratio>
```

Update aggregate checker tests to match.

- [ ] **Step 5: Verify green**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence tools.test_check_all_portable_evidence
```

Expected: PASS.

### Task 3: Publish Evidence Artifacts and Catalog Slot

**Files:**
- Add: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/paired-raw.jsonl`
- Add: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/paired-summary.json`
- Add: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/paired-report.md`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/artifact-sha256.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`

**Interfaces:**
- Consumes: `python3 -m benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise`
- Produces: committed paired artifacts accepted by `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`

- [ ] **Step 1: Generate paired evidence from the runner**

Run after code changes are committed:

```sh
rm -rf /tmp/rstim-paired-frame-noise
python3 -m benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise \
  --baseline-rev f10d1ed024d3519318ed244c9095724074519595 \
  --candidate-rev HEAD \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --shots 1024 --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-paired-frame-noise
```

Expected: `PASS paired frame-noise benchmark variants=2 measured=14 bytes=1552384`.

- [ ] **Step 2: Copy and rename runner artifacts**

Copy:

- `/tmp/rstim-paired-frame-noise/raw.jsonl` to `paired-raw.jsonl`
- `/tmp/rstim-paired-frame-noise/summary.json` to `paired-summary.json`
- `/tmp/rstim-paired-frame-noise/report.md` to `paired-report.md`

Do not copy paired environment files into this release slot.

- [ ] **Step 3: Refresh hash manifests**

Regenerate `artifact-sha256.json` to hash all required bundle artifacts, including the new `paired-*` files.

Update the `frame-instruction-wide-release` catalog artifacts to include the new files and refreshed SHA-256 digests. Add `benchmarks/rstim_vs_stim_simulator/run_paired_frame_noise.py` as a repository input.

- [ ] **Step 4: Verify published evidence**

Run:

```sh
python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release
```

Expected output begins with `PASS instruction-wide frame-noise evidence outcome=` and includes `builds=803 legacy_setups=80362 candidate_over_baseline=<ratio>` where ratio is at most `1.05`.

- [ ] **Step 5: Run full verification**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence tools.test_check_all_portable_evidence benchmarks.rstim_vs_stim_simulator.tests.test_run_paired_frame_noise benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles
cargo test
```

Expected: all pass.
