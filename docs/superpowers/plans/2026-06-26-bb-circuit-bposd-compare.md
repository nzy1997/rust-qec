# BB Circuit BP-OSD Compare Smoke Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal smoke command that compares Rust `rbposd` and Python upstream `ldpc`/`bposd` on shared BB72 and BB90 circuit BP-OSD diagnostic cases.

**Architecture:** Rust owns the production BB circuit model, sampling, and `rbposd` profile export. Python shells out to the Rust exporter, replays the exported effective models and syndromes through `ldpc.BpOsdDecoder`, writes paired CSV rows, and validates them with a narrow smoke verifier.

**Tech Stack:** Rust 2024, `rsinter` CLI, `serde_json`, Python standard library, optional Python `ldpc`/`bposd`, Makefile.

## Global Constraints

- Preserve the existing default four-column `rsinter bb-circuit-bposd-memory` output.
- Smoke rows must include `case_id`, `runner`, `decoder_impl`, `code_id`, `p`, `num_cycles`, `num_trials`, `seed`, `setup_seconds`, `decode_seconds`, `run_seconds`, `logical_error_rate`, and `status`.
- Python upstream settings must record min-sum / `ms`, `max_iter = 10000`, `osd_method = osd_cs`, `osd_order = 7`, and seed `12345`.
- Missing Python `ldpc`/`bposd` dependencies must be explicit and must not produce a green Rust-only comparison unless `--allow-missing-python` is passed, and the verifier must still reject that CSV.
- Keep the comparison thin: no full 50,000-trial sweep, no dashboard, and no broad benchmark framework migration.

---

## File Structure

- Modify `rsinter/src/bb_circuit_memory.rs`: add BB72 code selection, serializable comparison export structs, and `export_comparison_case_for_code`.
- Modify `rsinter/src/bin/rsinter.rs`: add `--json-compare-case` to the existing BB circuit CLI.
- Modify `rsinter/tests/bb_circuit_memory.rs`: cover BB72 shape and JSON export contents.
- Modify `rsinter/tests/bench_cli.rs`: cover the new JSON CLI mode while preserving the four-column default.
- Create `benchmarks/bb_circuit_bposd_compare/__init__.py`: package marker.
- Create `benchmarks/bb_circuit_bposd_compare/cases.py`: smoke case manifest and CSV header.
- Create `benchmarks/bb_circuit_bposd_compare/summary.py`: Markdown timing summary artifact.
- Create `benchmarks/bb_circuit_bposd_compare/verify_smoke.py`: CSV contract verifier and CLI.
- Create `benchmarks/bb_circuit_bposd_compare/run_compare.py`: Rust exporter orchestration, Python upstream replay, CSV writing, summary writing, dependency handling.
- Create `benchmarks/bb_circuit_bposd_compare/README.md`: usage and dependency notes.
- Create `benchmarks/bb_circuit_bposd_compare/tests/`: Python unit tests for verifier, summary, and dependency handling.
- Modify `Makefile`: add `bb-circuit-bposd-compare-smoke`.

---

### Task 1: Rust BB72 Selector and Shared Case Export

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/src/bin/rsinter.rs`
- Modify: `rsinter/tests/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bench_cli.rs`

**Interfaces:**
- Produces: `pub fn export_comparison_case_for_code(code_id: &str, config: SimulationConfig) -> Result<BbCircuitBposdComparisonExport, String>`
- Produces: CLI flag `rsinter bb-circuit-bposd-memory --json-compare-case`
- Consumes: existing `build_code`, `build_effective_models`, `simulate_trial`, `decode_logicals`, and profile aggregation helpers.

- [ ] **Step 1: Write failing Rust tests**

Add this test to `rsinter/tests/bb_circuit_memory.rs` near the existing code selection tests:

```rust
#[test]
fn build_code_supports_bb72_smoke_shape() {
    let bb72 = build_code("bb72").unwrap();
    assert_eq!(bb72.ell(), 6);
    assert_eq!(bb72.m(), 6);
    assert_eq!(bb72.n2(), 36);
    assert_eq!(bb72.n(), 72);
    assert_eq!(bb72.k(), 12);
    assert!(bb72.hx_rows().iter().all(|row| row.len() == 6));
    assert!(bb72.hz_rows().iter().all(|row| row.len() == 6));
}
```

Add `export_comparison_case_for_code` to the imports in the same file, then add:

```rust
#[test]
fn comparison_case_export_contains_models_samples_and_profile() {
    let export = export_comparison_case_for_code(
        "bb72",
        SimulationConfig {
            physical_error_rate: 1.0e-12,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(12345),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    )
    .unwrap();

    assert_eq!(export.code_id, "bb72");
    assert_eq!(export.num_trials, 1);
    assert_eq!(export.seed, Some(12345));
    assert_eq!(export.z_model.num_checks, 36 * 3);
    assert_eq!(export.x_model.num_checks, 36 * 3);
    assert_eq!(export.trials.len(), 1);
    assert_eq!(export.trials[0].z_syndrome.len(), export.z_model.num_checks);
    assert_eq!(export.trials[0].x_syndrome.len(), export.x_model.num_checks);
    assert!(export.rust_result.profile.setup_seconds.is_finite());
    assert!(export.rust_result.profile.decode_seconds.is_finite());
}
```

Add this test to `rsinter/tests/bench_cli.rs`:

```rust
#[test]
fn rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--code-id",
            "bb72",
            "--physical-error-rate",
            "0.000000000001",
            "--num-cycles",
            "1",
            "--num-trials",
            "1",
            "--seed",
            "12345",
            "--max-bp-iterations",
            "10",
            "--osd-order",
            "0",
            "--json-compare-case",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["code_id"], "bb72");
    assert_eq!(json["num_trials"], 1);
    assert_eq!(json["trials"].as_array().unwrap().len(), 1);
    assert!(json["rust_result"]["profile"]["setup_seconds"].is_number());
    assert!(json["z_model"]["sparse_rows"].as_array().unwrap().len() > 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
```

Expected: FAIL across the focused commands with unknown `export_comparison_case_for_code`, missing CLI flag, or unsupported `bb72`.

- [ ] **Step 3: Implement BB72 support and export structs**

In `rsinter/src/bb_circuit_memory.rs`, add `use serde::Serialize;`, derive `Serialize` on `SimulationConfig`, `SimulationResult`, and `BbCircuitBposdProfile`, and add:

```rust
    pub fn bb72() -> Self {
        Self {
            ell: 6,
            m: 6,
            a1: 3,
            a2: 1,
            a3: 2,
            b1: 3,
            b2: 1,
            b3: 2,
        }
    }
```

Extend `build_code`:

```rust
        "bb72" => BivariateBicycleParams::bb72(),
        "bb90" => BivariateBicycleParams::bb90(),
        "bb144" => BivariateBicycleParams::bb144(),
        _ => {
            return Err(format!(
                "unknown bb code id {code_id:?}; supported ids: bb72, bb90, bb144"
            ));
        }
```

Add these structs near `SampledTrial`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct BbCircuitBposdComparisonExport {
    pub code_id: String,
    pub physical_error_rate: f64,
    pub num_cycles: usize,
    pub num_trials: usize,
    pub seed: Option<u64>,
    pub max_bp_iterations: usize,
    pub osd_order: usize,
    pub rust_result: SimulationResult,
    pub z_model: ComparisonModelExport,
    pub x_model: ComparisonModelExport,
    pub trials: Vec<ComparisonTrialExport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonModelExport {
    pub num_checks: usize,
    pub num_bits: usize,
    pub sparse_rows: Vec<Vec<usize>>,
    pub augmented_columns: Vec<Vec<usize>>,
    pub channel_probs: Vec<f64>,
    pub first_logical_row: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonTrialExport {
    pub z_syndrome: Vec<bool>,
    pub x_syndrome: Vec<bool>,
    pub z_logical: Vec<bool>,
    pub x_logical: Vec<bool>,
}
```

Implement helpers:

```rust
fn comparison_model_export(model: &EffectiveDecoderModel) -> ComparisonModelExport {
    ComparisonModelExport {
        num_checks: model.decoder.num_checks(),
        num_bits: model.decoder.num_bits(),
        sparse_rows: (0..model.decoder.num_checks())
            .map(|row| model.decoder.row_neighbors(row).to_vec())
            .collect(),
        augmented_columns: model.augmented_columns.clone(),
        channel_probs: model.channel_probs.clone(),
        first_logical_row: model.first_logical_row,
    }
}

fn comparison_trial_export(sample: &SampledTrial) -> ComparisonTrialExport {
    ComparisonTrialExport {
        z_syndrome: sample.z_syndrome.clone(),
        x_syndrome: sample.x_syndrome.clone(),
        z_logical: sample.z_logical.clone(),
        x_logical: sample.x_logical.clone(),
    }
}
```

Implement `export_comparison_case_for_code` by matching the sampling and decode loop in `run_simulation_for_code`, pushing `comparison_trial_export(&sample)` for each sampled trial, and returning the result plus `comparison_model_export(&models.z_faults)` and `comparison_model_export(&models.x_faults)`.

- [ ] **Step 4: Implement CLI JSON mode**

In `rsinter/src/bin/rsinter.rs`, import `export_comparison_case_for_code`, add `json_compare_case: bool` to the `BbCircuitBposdMemory` command, and route:

```rust
if json_compare_case {
    let export = export_comparison_case_for_code(&code_id, config)?;
    serde_json::to_writer_pretty(std::io::stdout(), &export).map_err(|e| e.to_string())?;
    println!();
} else {
    let result = run_simulation_for_code(&code_id, config)?;
    println!(
        "{}\t{}\t{}\t{}",
        result.physical_error_rate,
        result.num_cycles,
        result.num_trials,
        result.num_failed_trials
    );
}
```

- [ ] **Step 5: Run focused Rust tests**

Run:

```bash
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
git add rsinter/src/bb_circuit_memory.rs rsinter/src/bin/rsinter.rs rsinter/tests/bb_circuit_memory.rs rsinter/tests/bench_cli.rs
git commit -m "feat: export bb bposd compare cases"
```

---

### Task 2: CSV Schema, Summary, and Verifier

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/__init__.py`
- Create: `benchmarks/bb_circuit_bposd_compare/cases.py`
- Create: `benchmarks/bb_circuit_bposd_compare/summary.py`
- Create: `benchmarks/bb_circuit_bposd_compare/verify_smoke.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/__init__.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_summary.py`

**Interfaces:**
- Produces: `CSV_HEADER: list[str]`
- Produces: `SMOKE_CASES: tuple[CompareCase, ...]`
- Produces: `verify_rows(rows: list[dict[str, str]]) -> list[str]`
- Produces: `write_summary(rows: list[dict[str, str]], out_path: Path) -> None`

- [ ] **Step 1: Write verifier and summary tests**

Create tests that build rows with helper dictionaries containing both `rbposd` and `ldpc_bposd` for `case_id=bb72-p0005-c1-t1-seed12345` and `case_id=bb90-p0005-c1-t1-seed12345`.

Required assertions:

```python
self.assertEqual(verify_rows(rows), [])
self.assertIn("upstream ldpc/bposd comparison row is missing", "\n".join(verify_rows(no_python_rows)))
self.assertIn("no paired Rust/Python diagnostic case is present", "\n".join(verify_rows(unpaired_rows)))
self.assertIn("completed row missing required timing/logical/status field", "\n".join(verify_rows(missing_timing_rows)))
```

For `summary.py`, assert that a generated `summary.md` contains both `rbposd`, `ldpc_bposd`, and `decode_seconds`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary`

Expected: FAIL because the package and functions do not exist.

- [ ] **Step 3: Implement cases and CSV header**

Create `cases.py` with:

```python
from __future__ import annotations

from dataclasses import dataclass

CSV_HEADER = [
    "case_id",
    "runner",
    "decoder_impl",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
    "error",
]

@dataclass(frozen=True)
class CompareCase:
    case_id: str
    code_id: str
    p: float
    num_cycles: int
    num_trials: int
    seed: int = 12345
    bp_method: str = "ms"
    max_iter: int = 10000
    osd_method: str = "osd_cs"
    osd_order: int = 7

SMOKE_CASES = (
    CompareCase("bb72-p0005-c1-t1-seed12345", "bb72", 0.0005, 1, 1),
    CompareCase("bb90-p0005-c1-t1-seed12345", "bb90", 0.0005, 1, 1),
)
```

- [ ] **Step 4: Implement verifier**

`verify_smoke.py` should load CSV rows, call `verify_rows`, print each error to stderr, and exit `1` when errors exist. `verify_rows` must check required columns, at least one `decoder_impl=rbposd` row, at least one `decoder_impl=ldpc_bposd` row, at least one completed paired `case_id`, required BB72 and BB90 coverage, and required fields on `status=ok` rows.

- [ ] **Step 5: Implement summary**

`summary.py` should filter `status=ok` rows and write a Markdown table with columns `case_id`, `decoder_impl`, `setup_seconds`, `decode_seconds`, `run_seconds`, and `logical_error_rate`.

- [ ] **Step 6: Run tests**

Run: `python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary`

Expected: PASS.

- [ ] **Step 7: Commit Task 2**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare
git commit -m "test: verify bb bposd compare smoke rows"
```

---

### Task 3: Python Comparison Runner

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py`

**Interfaces:**
- Consumes: `SMOKE_CASES`, `CSV_HEADER`, `write_summary`, and `verify_rows`.
- Produces: `run_suite(output_dir: Path, allow_missing_python: bool = False, cases: Sequence[CompareCase] = SMOKE_CASES, rust_exporter: Callable[[CompareCase], dict[str, Any]] | None = None) -> int`
- Produces: CLI `python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke [--allow-missing-python]`

- [ ] **Step 1: Write runner tests**

Add tests using a fake Rust export and a fake missing Python dependency path:

```python
def fake_export(case):
    return {
        "code_id": case.code_id,
        "physical_error_rate": case.p,
        "num_cycles": case.num_cycles,
        "num_trials": case.num_trials,
        "seed": case.seed,
        "max_bp_iterations": case.max_iter,
        "osd_order": case.osd_order,
        "rust_result": {
            "num_failed_trials": 0,
            "profile": {"setup_seconds": 0.1, "decode_seconds": 0.2},
        },
        "z_model": {"num_checks": 1, "num_bits": 1, "sparse_rows": [[]], "augmented_columns": [[]], "channel_probs": [0.1], "first_logical_row": 1},
        "x_model": {"num_checks": 1, "num_bits": 1, "sparse_rows": [[]], "augmented_columns": [[]], "channel_probs": [0.1], "first_logical_row": 1},
        "trials": [{"z_syndrome": [False], "x_syndrome": [False], "z_logical": [False], "x_logical": [False]}],
    }
```

Assert that missing Python dependencies produce skipped `ldpc_bposd` rows, a nonzero return without `allow_missing_python`, and a zero return with `allow_missing_python` while `verify_rows` still rejects the skipped CSV.

- [ ] **Step 2: Run tests to verify they fail**

Run: `python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare`

Expected: FAIL because `run_compare.py` does not exist.

- [ ] **Step 3: Implement Rust exporter orchestration**

Add `_run_rust_export(case: CompareCase) -> dict[str, Any]` that invokes:

```python
[
    "cargo",
    "run",
    "-q",
    "-p",
    "rsinter",
    "--bin",
    "rsinter",
    "--",
    "bb-circuit-bposd-memory",
    "--code-id",
    case.code_id,
    "--physical-error-rate",
    str(case.p),
    "--num-cycles",
    str(case.num_cycles),
    "--num-trials",
    str(case.num_trials),
    "--seed",
    str(case.seed),
    "--max-bp-iterations",
    str(case.max_iter),
    "--osd-order",
    str(case.osd_order),
    "--json-compare-case",
]
```

Parse stdout as JSON and raise a `RuntimeError` that includes stdout/stderr if the command fails.

- [ ] **Step 4: Implement row conversion**

Add `_rust_row(case, export)` using `export["rust_result"]["profile"]` and `num_failed_trials / num_trials`.

Add `_skipped_python_row(case, error)` with `status="skipped"` and the dependency message.

- [ ] **Step 5: Implement Python upstream replay**

Import `numpy` and `from ldpc import BpOsdDecoder` inside `_python_row`. Convert each exported sparse row list into a `numpy.uint8` dense parity-check matrix. Construct decoders with:

```python
BpOsdDecoder(
    matrix,
    error_channel=model["channel_probs"],
    max_iter=case.max_iter,
    bp_method="ms",
    osd_method="osd_cs",
    osd_order=case.osd_order,
    input_vector_type="syndrome",
)
```

Decode Z first, compute predicted logical bits from `augmented_columns` entries at or after `first_logical_row`, skip X decode when Z already fails, and compute `logical_error_rate`.

- [ ] **Step 6: Implement `run_suite` and CLI**

`run_suite` should write `results.csv`, call `write_summary`, and return `0` only when no Rust errors occurred and no Python rows were skipped unless `allow_missing_python=True`.

The CLI should support only `--tier smoke`, `--output-dir benchmarks/bb_circuit_bposd_compare/results`, and `--allow-missing-python`.

- [ ] **Step 7: Run runner tests**

Run: `python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare`

Expected: PASS.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/run_compare.py benchmarks/bb_circuit_bposd_compare/tests
git commit -m "feat: run bb bposd compare smoke"
```

---

### Task 4: Make Target, Docs, and End-to-End Verification

**Files:**
- Modify: `Makefile`
- Create: `benchmarks/bb_circuit_bposd_compare/README.md`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py`

**Interfaces:**
- Produces: `make bb-circuit-bposd-compare-smoke`
- Produces: `benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv`
- Produces: `benchmarks/bb_circuit_bposd_compare/results/smoke/summary.md`

- [ ] **Step 1: Add Makefile target and README**

Add `bb-circuit-bposd-compare-smoke` to `.PHONY`, `help`, and:

```make
bb-circuit-bposd-compare-smoke:
	python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke
	python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv
```

Create `README.md` documenting the target, dependency install hint (`python3 -m pip install 'ldpc>=2.4.1' bposd numpy`), output paths, and missing-dependency behavior.

- [ ] **Step 2: Run Python unit tests**

Run: `python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary benchmarks.bb_circuit_bposd_compare.tests.test_run_compare`

Expected: PASS.

- [ ] **Step 3: Run focused Rust tests**

Run:

```bash
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
```

Expected: PASS.

- [ ] **Step 4: Run smoke command**

Run: `make bb-circuit-bposd-compare-smoke`

Expected when Python dependencies are installed: PASS and writes `results.csv` plus `summary.md`.

Expected when Python dependencies are absent: FAIL with explicit missing dependency text, while `results.csv` contains skipped `ldpc_bposd` rows and `summary.md` exists.

- [ ] **Step 5: Run verifier directly**

Run: `python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv`

Expected: PASS only if `ldpc`/`bposd` rows completed; FAIL if dependencies were missing and rows were skipped.

- [ ] **Step 6: Run negative controls**

Create a copy with Python rows removed:

```bash
python3 - <<'PY'
import csv
from pathlib import Path
src = Path("benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv")
dst = Path("/tmp/bb-compare-missing-ldpc.csv")
rows = list(csv.DictReader(src.open()))
with dst.open("w", newline="") as f:
    writer = csv.DictWriter(f, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows([r for r in rows if r["decoder_impl"] != "ldpc_bposd"])
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke /tmp/bb-compare-missing-ldpc.csv
```

Expected: FAIL and stderr contains `upstream ldpc/bposd comparison row is missing`.

Create a copy with unpaired case IDs:

```bash
python3 - <<'PY'
import csv
from pathlib import Path
src = Path("benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv")
dst = Path("/tmp/bb-compare-unpaired-cases.csv")
rows = list(csv.DictReader(src.open()))
for row in rows:
    if row["decoder_impl"] == "ldpc_bposd":
        row["case_id"] = row["case_id"] + "-python-only"
with dst.open("w", newline="") as f:
    writer = csv.DictWriter(f, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows(rows)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke /tmp/bb-compare-unpaired-cases.csv
```

Expected: FAIL and stderr contains `no paired Rust/Python diagnostic case is present`.

- [ ] **Step 7: Run required broad Rust verification**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 8: Commit Task 4**

Run:

```bash
git add Makefile benchmarks/bb_circuit_bposd_compare
git commit -m "docs: add bb bposd compare smoke target"
```

---

## Self-Review

- Spec coverage: Tasks cover BB72/BB90 rows, paired case IDs, Rust and Python decoder implementations, dependency failure behavior, result CSV, summary artifact, verifier acceptance, and negative controls.
- Placeholder scan: The plan contains no incomplete markers or open-ended implementation steps.
- Type consistency: `CompareCase`, `CSV_HEADER`, `write_summary`, `verify_rows`, and `export_comparison_case_for_code` names are defined before later tasks consume them.

## Execution Choice

Plan complete and saved to `docs/superpowers/plans/2026-06-26-bb-circuit-bposd-compare.md`.

Two execution options:

1. Subagent-Driven (recommended) - dispatch a fresh subagent per task, review between tasks, fast iteration.
2. Inline Execution - execute tasks in this session using executing-plans, batch execution with checkpoints.
