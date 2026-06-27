# Issue 308 Bravyi Model Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic BB72 Bravyi effective-model audit command and verifier that compare Rust model construction evidence against a pinned source-backed fixture.

**Architecture:** Add a model-only Rust export behind the existing `rsinter bb-circuit-bposd-memory` CLI, then make Python reduce that export to compact hashes/counts and verify it against a checked-in BB72 fixture. The audit command writes `/tmp/rstim-bb-model-audit/model_audit.json`; the verifier independently reloads the checked-in fixture so hand-edited audit JSON cannot self-certify.

**Tech Stack:** Rust 2024, `clap`, `serde`, Python 3 standard library, `pytest`, existing `benchmarks.bb_circuit_bposd_compare` package.

## Global Constraints

- Audit point: `code_id=bb72`, `physical_error_rate=0.003`, `num_cycles=6`.
- Bravyi contract source: `benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json`.
- Upstream commit: `sbravyi/BivariateBicycleCodes@fa77e3333d3ec44c79d8f914dd24c040d1da471b`.
- Tail convention: `noiseless_tail_cycles=2`, so `num_cycles_plus_tail=8` for the audit point.
- Decoder settings: `bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, `osd_order=7`, `ms_scaling_factor=0`.
- The audit path must not run Monte Carlo trials.
- The audit artifact must include status and mismatch details.
- The verifier must reject a changed tail-cycle count or changed schedule/model evidence with a nonzero exit and named mismatch.

---

## File Structure

- Modify `rsinter/src/bb_circuit_memory.rs`: add serializable model-audit export structs and `export_bravyi_model_audit_for_code()`.
- Modify `rsinter/src/bin/rsinter.rs`: add `--json-model-audit` to `bb-circuit-bposd-memory` and route it to the new export.
- Create `benchmarks/bb_circuit_bposd_compare/bravyi_model_audit.py`: CLI, Rust export call, normalization, comparison, and artifact writer.
- Create `benchmarks/bb_circuit_bposd_compare/verify_model_audit.py`: standalone verifier for audit artifacts.
- Create `benchmarks/bb_circuit_bposd_compare/reference/bravyi_model_audit_bb72_p003_c6.json`: expected normalized fixture.
- Create `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py`: pytest coverage and negative controls.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md`: document the audit and verifier commands.

### Task 1: Rust Model-Only Export

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/src/bin/rsinter.rs`

**Interfaces:**
- Consumes: `build_code()`, `build_syndrome_cycle()`, `build_effective_models()`, `comparison_model_export()`, `SimulationConfig`.
- Produces: `export_bravyi_model_audit_for_code(code_id: &str, config: SimulationConfig) -> Result<BbCircuitBravyiModelAuditExport, String>`.
- Produces CLI: `rsinter bb-circuit-bposd-memory --json-model-audit`.

- [ ] **Step 1: Write failing Rust test**

Add a unit test in `rsinter/src/bb_circuit_memory.rs`:

```rust
#[test]
fn bravyi_model_audit_export_is_model_only_and_reports_tail_rows() {
    let export = export_bravyi_model_audit_for_code(
        "bb72",
        SimulationConfig {
            physical_error_rate: 0.003,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(12345),
            max_bp_iterations: 10_000,
            osd_order: 7,
        },
    )
    .unwrap();

    assert_eq!(export.code_id, "bb72");
    assert_eq!(export.code.n2, 36);
    assert_eq!(export.code.n, 72);
    assert_eq!(export.code.k, 12);
    assert_eq!(export.noiseless_tail_cycles, 2);
    assert_eq!(export.num_cycles_plus_tail, 3);
    assert_eq!(export.z_model.first_logical_row, 36 * 3);
    assert_eq!(export.x_model.first_logical_row, 36 * 3);
    assert_eq!(export.z_model.num_checks, 36 * 3);
    assert_eq!(export.x_model.num_checks, 36 * 3);
    assert_eq!(export.schedule.sx_labels, ["idle", "1", "4", "3", "5", "0", "2"]);
    assert_eq!(export.schedule.sz_labels, ["3", "5", "0", "1", "2", "4", "idle"]);
    assert_eq!(export.schedule.operation_count_by_kind.get("cnot"), Some(&432));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rsinter bravyi_model_audit_export_is_model_only_and_reports_tail_rows
```

Expected: fail to compile because `export_bravyi_model_audit_for_code` and export structs do not exist.

- [ ] **Step 3: Implement export structs and function**

Add serializable structs near the existing comparison export structs:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BbCircuitBravyiModelAuditExport {
    pub code_id: String,
    pub physical_error_rate: f64,
    pub num_cycles: usize,
    pub noiseless_tail_cycles: usize,
    pub num_cycles_plus_tail: usize,
    pub code: BravyiModelAuditCodeExport,
    pub schedule: BravyiModelAuditScheduleExport,
    pub z_model: ComparisonModelExport,
    pub x_model: ComparisonModelExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BravyiModelAuditCodeExport {
    pub ell: usize,
    pub m: usize,
    pub n2: usize,
    pub n: usize,
    pub k: usize,
    pub x_check_count: usize,
    pub z_check_count: usize,
    pub data_qubit_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BravyiModelAuditScheduleExport {
    pub sx_labels: [&'static str; 7],
    pub sz_labels: [&'static str; 7],
    pub operation_count: usize,
    pub operation_count_by_kind: BTreeMap<&'static str, usize>,
}
```

Add helpers:

```rust
fn operation_kind_label(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::Idle => "idle",
        OperationKind::Cnot => "cnot",
        OperationKind::PrepX => "prep_x",
        OperationKind::PrepZ => "prep_z",
        OperationKind::MeasX => "meas_x",
        OperationKind::MeasZ => "meas_z",
    }
}

fn model_audit_code_export(code: &BbCode) -> BravyiModelAuditCodeExport {
    BravyiModelAuditCodeExport {
        ell: code.ell(),
        m: code.m(),
        n2: code.n2(),
        n: code.n(),
        k: code.k(),
        x_check_count: code.x_checks().len(),
        z_check_count: code.z_checks().len(),
        data_qubit_count: code.data_qubits().len(),
    }
}

fn model_audit_schedule_export(cycle: &SyndromeCycle) -> BravyiModelAuditScheduleExport {
    let mut operation_count_by_kind = BTreeMap::new();
    for operation in cycle.operations() {
        *operation_count_by_kind
            .entry(operation_kind_label(operation.kind()))
            .or_insert(0) += 1;
    }
    BravyiModelAuditScheduleExport {
        sx_labels: cycle.sx_labels(),
        sz_labels: cycle.sz_labels(),
        operation_count: cycle.operations().len(),
        operation_count_by_kind,
    }
}
```

Add public export:

```rust
pub fn export_bravyi_model_audit_for_code(
    code_id: &str,
    config: SimulationConfig,
) -> Result<BbCircuitBravyiModelAuditExport, String> {
    validate_model_config(&config)?;
    let code = build_code(code_id)?;
    let cycle = build_syndrome_cycle(&code);
    let models = build_effective_models(&code, &cycle, &config)?;
    Ok(BbCircuitBravyiModelAuditExport {
        code_id: code_id.to_owned(),
        physical_error_rate: config.physical_error_rate,
        num_cycles: config.num_cycles,
        noiseless_tail_cycles: BRAVYI_NOISELESS_TAIL_CYCLES,
        num_cycles_plus_tail: config.num_cycles + BRAVYI_NOISELESS_TAIL_CYCLES,
        code: model_audit_code_export(&code),
        schedule: model_audit_schedule_export(&cycle),
        z_model: comparison_model_export(&models.z_faults),
        x_model: comparison_model_export(&models.x_faults),
    })
}
```

- [ ] **Step 4: Wire CLI flag**

Update `rsinter/src/bin/rsinter.rs` imports and command fields:

```rust
use rsinter::bb_circuit_memory::{
    export_bravyi_model_audit_for_code, export_comparison_case_for_code,
    export_comparison_case_for_code_with_osd_variant, run_simulation_for_code,
    run_simulation_for_code_with_osd_variant, SimulationConfig,
};
```

Add the flag beside `json_compare_case`:

```rust
#[arg(long)]
json_model_audit: bool,
```

Destructure and handle before `json_compare_case`:

```rust
if json_model_audit {
    let export = export_bravyi_model_audit_for_code(&code_id, config)?;
    serde_json::to_writer_pretty(std::io::stdout(), &export).map_err(|e| e.to_string())?;
    println!();
} else if json_compare_case {
    let result = match osd_variant {
        Some(osd_variant) => export_comparison_case_for_code_with_osd_variant(
            &code_id,
            config,
            osd_variant,
        )?,
        None => export_comparison_case_for_code(&code_id, config)?,
    };
}
```

- [ ] **Step 5: Run Rust focused tests**

Run:

```bash
cargo test -p rsinter bravyi_model_audit_export_is_model_only_and_reports_tail_rows
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rsinter/src/bb_circuit_memory.rs rsinter/src/bin/rsinter.rs
git commit -m "feat: export bravyi bb model audit data"
```

### Task 2: Python Audit Command And Fixture

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/bravyi_model_audit.py`
- Create: `benchmarks/bb_circuit_bposd_compare/reference/bravyi_model_audit_bb72_p003_c6.json`
- Test: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py`

**Interfaces:**
- Consumes: Rust model-audit export JSON, `bravyi_contract.json`.
- Produces: `build_audit_artifact(code_id, physical_error_rate, num_cycles, rust_export) -> dict[str, object]`.
- Produces CLI: `python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit --code-id bb72 --physical-error-rate 0.003 --num-cycles 6 --out /tmp/rstim-bb-model-audit/model_audit.json`.

- [ ] **Step 1: Write failing Python tests for normalization and CLI**

Create `test_bravyi_model_audit.py` with fake Rust export data, then assert:

```python
def test_build_audit_artifact_passes_when_export_matches_expected_fixture() -> None:
    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    assert artifact["status"] == "pass"
    assert artifact["observed"]["syndrome_tail"]["num_cycles_plus_tail"] == 8
    assert artifact["observed"]["models"]["Z"]["first_logical_row"] == 288
```

and:

```python
def test_audit_cli_writes_json_with_mocked_rust_export(tmp_path, monkeypatch) -> None:
    out = tmp_path / "model_audit.json"
    monkeypatch.setattr(bravyi_model_audit, "_run_rust_model_audit_export", lambda *a, **k: FAKE_RUST_EXPORT)
    status = bravyi_model_audit.main([
        "--code-id", "bb72",
        "--physical-error-rate", "0.003",
        "--num-cycles", "6",
        "--out", str(out),
    ])
    assert status == 0
    assert json.loads(out.read_text())["status"] == "pass"
```

Use fake export values copied from the expected fixture once generated.

- [ ] **Step 2: Run pytest to verify tests fail**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
```

Expected: fail because `bravyi_model_audit.py` does not exist.

- [ ] **Step 3: Implement audit module**

Implement:

```python
REFERENCE_DIR = Path(__file__).resolve().parent / "reference"
CONTRACT_PATH = REFERENCE_DIR / "bravyi_contract.json"
EXPECTED_AUDIT_PATH = REFERENCE_DIR / "bravyi_model_audit_bb72_p003_c6.json"
AUDIT_VERSION = 1
```

Add `_run_rust_model_audit_export()`, `_hash_json()`, `_normalize_probability()`,
`_model_summary()`, `_observed_summary()`, `_compare_expected()`,
`build_audit_artifact()`, `write_audit()`, and `main()`.

The canonical hashes use:

```python
hashlib.sha256(
    json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
).hexdigest()
```

The model summary includes:

```python
{
    "decoder_rows": model["num_checks"],
    "decoder_columns": model["num_bits"],
    "first_logical_row": model["first_logical_row"],
    "grouped_column_count": len(model["augmented_columns"]),
    "sparse_rows_hash": _hash_json(model["sparse_rows"]),
    "augmented_columns_hash": _hash_json(model["augmented_columns"]),
    "channel_probabilities_hash": _hash_json([_format_float(p) for p in model["channel_probs"]]),
    "probability_total": _format_float(math.fsum(model["channel_probs"])),
    "probability_min": _format_float(min(model["channel_probs"])),
    "probability_max": _format_float(max(model["channel_probs"])),
}
```

Use basis mapping:

```python
"Z": rust_export["z_model"]
"X": rust_export["x_model"]
```

- [ ] **Step 4: Generate expected fixture from the real Rust export**

Run the real audit command once after Task 1:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit \
  --code-id bb72 \
  --physical-error-rate 0.003 \
  --num-cycles 6 \
  --out /tmp/rstim-bb-model-audit/model_audit.json
```

Copy only `expected`/normalized expected fields into
`reference/bravyi_model_audit_bb72_p003_c6.json`, with provenance:

```json
{
  "fixture_version": 1,
  "description": "Expected normalized BB72 Bravyi model-audit evidence for issue #308.",
  "provenance": {
    "upstream_repository": "sbravyi/BivariateBicycleCodes",
    "upstream_commit": "fa77e3333d3ec44c79d8f914dd24c040d1da471b",
    "contract_path": "benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json"
  },
  "expected": {
    "inputs": {
      "code_id": "bb72",
      "physical_error_rate": "0.003",
      "num_cycles": 6
    },
    "code": {
      "ell": 6,
      "m": 6,
      "n2": 36,
      "n": 72,
      "k": 12,
      "x_check_count": 36,
      "z_check_count": 36,
      "data_qubit_count": 72
    },
    "schedule": {
      "sx_labels": ["idle", "1", "4", "3", "5", "0", "2"],
      "sz_labels": ["3", "5", "0", "1", "2", "4", "idle"]
    },
    "syndrome_tail": {
      "configured_noisy_cycles": 6,
      "noiseless_tail_cycles": 2,
      "num_cycles_plus_tail": 8
    },
    "models": {
      "Z": {
        "first_logical_row": 288
      },
      "X": {
        "first_logical_row": 288
      }
    }
  }
}
```

- [ ] **Step 5: Run pytest**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/bravyi_model_audit.py \
  benchmarks/bb_circuit_bposd_compare/reference/bravyi_model_audit_bb72_p003_c6.json \
  benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
git commit -m "feat: add bravyi bb model audit"
```

### Task 3: Verifier And Negative Controls

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/verify_model_audit.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py`

**Interfaces:**
- Consumes audit artifact JSON from Task 2 and expected fixture.
- Produces CLI: `python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit <artifact>`.
- Produces function: `verify_audit_artifact(artifact: dict[str, object]) -> list[str]`.

- [ ] **Step 1: Write failing verifier tests**

Add:

```python
def test_verify_model_audit_accepts_good_artifact() -> None:
    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    assert verify_model_audit.verify_audit_artifact(artifact) == []
```

Add tail negative control:

```python
def test_verify_model_audit_rejects_tail_cycle_drift() -> None:
    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact["observed"]["syndrome_tail"]["noiseless_tail_cycles"] = 1
    errors = verify_model_audit.verify_audit_artifact(artifact)
    assert any("syndrome_tail.noiseless_tail_cycles" in error for error in errors)
```

Add schedule/model negative control:

```python
def test_verify_model_audit_rejects_schedule_hash_drift() -> None:
    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact["observed"]["schedule"]["sx_labels"][0] = "changed"
    errors = verify_model_audit.verify_audit_artifact(artifact)
    assert any("schedule.sx_labels" in error for error in errors)
```

- [ ] **Step 2: Run pytest to verify tests fail**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
```

Expected: fail because `verify_model_audit.py` does not exist or verifier functions are missing.

- [ ] **Step 3: Implement verifier**

Implement:

```python
def verify_audit_artifact(artifact: dict[str, object]) -> list[str]:
    expected = load_expected_summary()
    errors = []
    _expect(artifact, ("audit_version",), 1, errors)
    _expect(artifact, ("status",), "pass", errors)
    _compare_mapping("inputs", artifact["inputs"], expected["inputs"], errors)
    _compare_mapping("observed.code", artifact["observed"]["code"], expected["code"], errors)
    _compare_mapping("observed.schedule", artifact["observed"]["schedule"], expected["schedule"], errors)
    _compare_mapping("observed.syndrome_tail", artifact["observed"]["syndrome_tail"], expected["syndrome_tail"], errors)
    _compare_mapping("observed.models.Z", artifact["observed"]["models"]["Z"], expected["models"]["Z"], errors)
    _compare_mapping("observed.models.X", artifact["observed"]["models"]["X"], expected["models"]["X"], errors)
    return errors
```

Print a PASS line:

```python
return (
    "PASS Bravyi model audit "
    f"{inputs['code_id']} [[{code['n']},{code['k']}]] "
    f"schedule_ops={schedule['operation_count']} "
    f"num_cycles_plus_tail={tail['num_cycles_plus_tail']} "
    f"Z first_logical_row={z_model['first_logical_row']} "
    f"dims={z_model['decoder_rows']}x{z_model['decoder_columns']} "
    f"X first_logical_row={x_model['first_logical_row']} "
    f"dims={x_model['decoder_rows']}x{x_model['decoder_columns']} "
    f"grouped_probabilities=Z:{z_model['probability_total']} "
    f"X:{x_model['probability_total']}"
)
```

- [ ] **Step 4: Run pytest**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/verify_model_audit.py \
  benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
git commit -m "test: verify bravyi bb model audit"
```

### Task 4: Documentation And End-To-End Verification

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: commands from issue #308.
- Produces: README section describing audit and negative control.

- [ ] **Step 1: Add README section**

Add a section after the Bravyi contract or readiness sections:

```markdown
## Bravyi Effective Model Audit

The BB72 model audit builds the Rust effective decoder models without Monte
Carlo trials and verifies their source-backed contract evidence against the
pinned Bravyi fixture:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit \
  --code-id bb72 \
  --physical-error-rate 0.003 \
  --num-cycles 6 \
  --out /tmp/rstim-bb-model-audit/model_audit.json
python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit \
  /tmp/rstim-bb-model-audit/model_audit.json
```

The verifier checks shape, schedule counts, `num_cycles_plus_tail=8`, X/Z
`first_logical_row`, decoder dimensions, and grouped probability hashes.
```

- [ ] **Step 2: Run required positive verification**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit \
  --code-id bb72 \
  --physical-error-rate 0.003 \
  --num-cycles 6 \
  --out /tmp/rstim-bb-model-audit/model_audit.json
python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit \
  /tmp/rstim-bb-model-audit/model_audit.json
cargo test
```

Expected: every command exits 0.

- [ ] **Step 3: Run required negative control**

Run:

```bash
cp /tmp/rstim-bb-model-audit/model_audit.json /tmp/model_audit_bad.json
python3 - <<'PY'
import json
from pathlib import Path
path = Path("/tmp/model_audit_bad.json")
data = json.loads(path.read_text())
data["observed"]["syndrome_tail"]["noiseless_tail_cycles"] = 1
path.write_text(json.dumps(data, indent=2) + "\n")
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit \
  /tmp/model_audit_bad.json
```

Expected: verifier exits nonzero and names `syndrome_tail.noiseless_tail_cycles`.

- [ ] **Step 4: Commit docs and any verification fixes**

```bash
git add benchmarks/bb_circuit_bposd_compare/README.md
git commit -m "docs: document bravyi bb model audit"
```
