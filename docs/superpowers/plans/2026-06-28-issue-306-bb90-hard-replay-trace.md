# Issue 306 BB90 Hard-Replay Correction Trace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic `hard_replay_trace.json` artifact and verifier that preserve the BB90 hard-replay Rust-vs-Python correction divergence without changing decoder behavior.

**Architecture:** Extend the Rust comparison export to include collected-trial correction bit vectors, then have the Python hard-replay runner derive a compact two-decoder JSON trace from the existing Rust export and Python `BpOsdDecoder.decode(...)` correction. A new verifier validates trace completeness, pairing, residual evidence, and the recorded logical mismatch classification.

**Tech Stack:** Rust 2024 (`rsinter`, `rbposd`), Python 3 standard library, existing `ldpc.BpOsdDecoder` replay path, `unittest`, Cargo tests.

## Global Constraints

- Preserve existing `results.csv`, `summary.md`, and `verify_replay` behavior.
- Keep the trace deterministic and small: one case, basis `Z`, one syndrome.
- Use the pinned upstream settings: `bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, `osd_order=7`, and Python `ms_scaling_factor=0`.
- Do not change `rbposd` decoding behavior.
- Do not run or require any 50,000-trial campaign.
- The real trace verifier must accept `classification=logical_prediction_mismatch` for `case_id=bb90-p006-c10-seed12345-order7-hard-syndrome`.

---

## File Structure

- Modify `rsinter/src/bb_circuit_memory.rs`: add optional `z_correction` and `x_correction` fields to `ComparisonTrialExport`, populate them from `DecodeResult.correction` for collected trials, and extend existing unit coverage.
- Modify `rsinter/tests/bench_cli.rs`: assert the CLI JSON comparison export exposes correction arrays for decoded bases.
- Modify `benchmarks/bb_circuit_bposd_compare/run_compare.py`: add trace helpers, write `hard_replay_trace.json`, and preserve the existing CSV path.
- Modify `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`: add failing tests for the emitted trace artifact using the existing fake hard replay exporter.
- Create `benchmarks/bb_circuit_bposd_compare/verify_replay_trace.py`: validate trace schema, pairing, decoder fields, residual metadata, and classification.
- Create `benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay_trace.py`: unit-test valid mismatch, missing correction support, unpaired syndrome metadata, and CLI output.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md`: document the new hard replay trace command.

### Task 1: Rust Comparison Export Correction Vectors

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bench_cli.rs`

**Interfaces:**
- Consumes: `DecodeResult.correction.as_slice()` from `rbposd`.
- Produces: JSON fields `z_correction: Option<Vec<bool>>` and `x_correction: Option<Vec<bool>>` on each collected `ComparisonTrialExport`.

- [ ] **Step 1: Write the failing Rust tests**

In `rsinter/src/bb_circuit_memory.rs`, extend `simulation_case_export_records_z_failure_without_x_decode` with:

```rust
assert!(trial.z_correction.as_ref().is_some_and(|bits| !bits.is_empty()));
assert!(trial.x_correction.is_none());
```

In `rsinter/tests/bench_cli.rs`, extend `rsinter_bb_circuit_bposd_memory_json_compare_case_smoke` with:

```rust
assert!(trial["z_correction"].as_array().is_some());
assert_eq!(
    trial["z_correction"].as_array().unwrap().len(),
    json["z_model"]["num_bits"].as_u64().unwrap() as usize
);
assert!(trial["x_correction"].as_array().is_some());
assert_eq!(
    trial["x_correction"].as_array().unwrap().len(),
    json["x_model"]["num_bits"].as_u64().unwrap() as usize
);
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rsinter simulation_case_export_records_z_failure_without_x_decode rsinter_bb_circuit_bposd_memory_json_compare_case_smoke
```

Expected: fails because `ComparisonTrialExport` has no `z_correction` or `x_correction` fields.

- [ ] **Step 3: Add correction fields and populate them**

Change `ComparisonTrialExport`:

```rust
pub z_logical_prediction: Option<Vec<bool>>,
pub x_logical_prediction: Option<Vec<bool>>,
pub z_correction: Option<Vec<bool>>,
pub x_correction: Option<Vec<bool>>,
pub z_profile: Option<BbCircuitBposdProfile>,
pub x_profile: Option<BbCircuitBposdProfile>,
```

Initialize in `comparison_trial_export`:

```rust
z_correction: None,
x_correction: None,
```

After Z decode succeeds in `run_simulation_case_for_code_with_osd_variant`, before possible early continue:

```rust
if let Some(trial) = trial_export.as_mut() {
    trial.z_logical_prediction = Some(predicted_z.clone());
    trial.z_correction = Some(z_result.correction.as_slice().to_vec());
    trial.z_profile = Some(profile_from_decode_stats(
        ProfileReplayBasis::Z,
        z_decode_seconds,
        &z_result.stats,
    ));
}
```

After X decode:

```rust
if let Some(trial) = trial_export.as_mut() {
    trial.x_logical_prediction = Some(predicted_x.clone());
    trial.x_correction = Some(x_result.correction.as_slice().to_vec());
    trial.x_profile = Some(profile_from_decode_stats(
        ProfileReplayBasis::X,
        x_decode_seconds,
        &x_result.stats,
    ));
}
```

- [ ] **Step 4: Run focused Rust tests**

Run:

```bash
cargo test -p rsinter simulation_case_export_records_z_failure_without_x_decode rsinter_bb_circuit_bposd_memory_json_compare_case_smoke
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
git add rsinter/src/bb_circuit_memory.rs rsinter/tests/bench_cli.rs
git commit -m "feat: export bb replay corrections"
```

### Task 2: Hard Replay Trace Artifact Writer

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`

**Interfaces:**
- Consumes: `_hard_replay_bundle(...)`, Rust trial `z_correction`, Python `correction = decoder.decode(...)`, and existing `RUST_PROFILE_COUNTER_FIELDS`.
- Produces: `<output_dir>/hard_replay_trace.json`.

- [ ] **Step 1: Write the failing Python trace artifact test**

In `fake_hard_export`, add:

```python
"z_correction": [True, False, True, True],
"x_correction": None,
```

In `test_hard_replay_suite_writes_paired_prediction_rows`, after reading CSV rows, read the trace:

```python
trace = json.loads((Path(tmpdir) / "hard_replay_trace.json").read_text())
self.assertEqual(trace["case_id"], HARD_REPLAY_CASES[0].case_id)
self.assertEqual(trace["basis"], "Z")
self.assertEqual(trace["classification"], "matched")
self.assertEqual(trace["syndrome_support"], [0, 2, 3])
self.assertEqual([entry["decoder_impl"] for entry in trace["decoders"]], ["rbposd", "ldpc_bposd"])
rust_trace, python_trace = trace["decoders"]
self.assertEqual(rust_trace["correction_support"], [0, 2, 3])
self.assertEqual(rust_trace["correction_weight"], 3)
self.assertTrue(rust_trace["residual_syndrome_matches"])
self.assertEqual(rust_trace["profile"]["osd_candidate_count"], 4100)
self.assertEqual(python_trace["correction_support"], [0, 2, 3])
self.assertEqual(python_trace["correction_weight"], 3)
self.assertTrue(python_trace["residual_syndrome_matches"])
```

Add a second test by making Python decode produce a different logical prediction:

```python
class FakeHardMismatchDecoder(FakeHardDecoder):
    def decode(self, syndrome):
        return FakeHardVector([False, False, False, True])
```

Assert:

```python
self.assertEqual(trace["classification"], "logical_prediction_mismatch")
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare.RunCompareTest.test_hard_replay_suite_writes_paired_prediction_rows
```

Expected: fails because `hard_replay_trace.json` does not exist.

- [ ] **Step 3: Implement trace helpers**

Add helpers in `run_compare.py`:

```python
HARD_REPLAY_TRACE_FILENAME = "hard_replay_trace.json"


def _support_from_bools(bits: Sequence[Any]) -> list[int]:
    return [index for index, enabled in enumerate(bits) if bool(enabled)]


def _vector_to_bools(vector: Any) -> list[bool]:
    if hasattr(vector, "tolist"):
        values = vector.tolist()
    else:
        values = list(vector)
    return [bool(value) for value in values]


def _residual_syndrome_support(
    model: dict[str, Any],
    correction_bits: Sequence[bool],
    syndrome_bits: Sequence[bool],
) -> list[int]:
    residual: list[int] = []
    for row_index, sparse_columns in enumerate(model["sparse_rows"]):
        parity = False
        for column_index in sparse_columns:
            parity ^= bool(correction_bits[column_index])
        if parity != bool(syndrome_bits[row_index]):
            residual.append(row_index)
    return residual
```

Add builders:

```python
def _trace_classification(decoders: Sequence[dict[str, Any]]) -> str:
    if len(decoders) != 2 or any(entry.get("status") != "ok" for entry in decoders):
        return "incomplete"
    predictions = [entry.get("predicted_logical") for entry in decoders]
    return "matched" if predictions[0] == predictions[1] else "logical_prediction_mismatch"
```

```python
def _rust_trace_entry(case: CompareCase, bundle: dict[str, Any], trial: dict[str, Any]) -> dict[str, Any]:
    correction_bits = trial.get("z_correction")
    if correction_bits is None:
        raise RuntimeError("Rust hard replay export is missing z_correction")
    residual_support = _residual_syndrome_support(bundle["model"], correction_bits, bundle["syndrome"])
    profile = {
        field: bundle["rust_profile"][field]
        for field in RUST_PROFILE_COUNTER_FIELDS
        if field in bundle["rust_profile"]
    }
    return {
        "decoder_impl": "rbposd",
        "status": "ok",
        "case_id": case.case_id,
        "basis": bundle["basis"],
        "syndrome_support": list(bundle["syndrome_support"]),
        "expected_sampled_logical": list(bundle["expected_logical"]),
        "bp_osd_settings": {
            "bp_method": case.bp_method,
            "max_iter": case.max_iter,
            "osd_method": case.osd_method,
            "osd_order": case.osd_order,
        },
        "correction_support": _support_from_bools(correction_bits),
        "correction_weight": len(_support_from_bools(correction_bits)),
        "residual_syndrome_support": residual_support,
        "residual_syndrome_weight": len(residual_support),
        "residual_syndrome_matches": not residual_support,
        "predicted_logical": list(bundle["rust_prediction"]),
        "profile": profile,
    }
```

```python
def _python_trace_entry(
    case: CompareCase,
    bundle: dict[str, Any],
    correction: Any,
    logical_prediction: Sequence[bool],
) -> dict[str, Any]:
    correction_bits = _vector_to_bools(correction)
    residual_support = _residual_syndrome_support(bundle["model"], correction_bits, bundle["syndrome"])
    return {
        "decoder_impl": "ldpc_bposd",
        "status": "ok",
        "case_id": case.case_id,
        "basis": bundle["basis"],
        "syndrome_support": list(bundle["syndrome_support"]),
        "expected_sampled_logical": list(bundle["expected_logical"]),
        "bp_osd_settings": _python_bposd_decoder_kwargs(),
        "correction_support": _support_from_bools(correction_bits),
        "correction_weight": len(_support_from_bools(correction_bits)),
        "residual_syndrome_support": residual_support,
        "residual_syndrome_weight": len(residual_support),
        "residual_syndrome_matches": not residual_support,
        "predicted_logical": list(logical_prediction),
    }
```

Write:

```python
def _write_hard_replay_trace(output_dir: Path, case: CompareCase, bundle: dict[str, Any], decoders: list[dict[str, Any]]) -> None:
    trace = {
        "schema_version": 1,
        "case_id": case.case_id,
        "basis": bundle["basis"],
        "syndrome_support": list(bundle["syndrome_support"]),
        "syndrome_weight": len(bundle["syndrome_support"]),
        "expected_sampled_logical": list(bundle["expected_logical"]),
        "classification": _trace_classification(decoders),
        "decoders": decoders,
    }
    (output_dir / HARD_REPLAY_TRACE_FILENAME).write_text(
        json.dumps(trace, indent=2, sort_keys=True) + "\n"
    )
```

- [ ] **Step 4: Integrate with hard replay run**

In `_python_hard_replay_row`, store the correction bits and logical prediction in a trace entry by factoring a small helper if needed. In `run_hard_replay_suite`, collect the Rust trace entry immediately after `_hard_replay_bundle(...)`, append the Python trace entry after Python decode, and call `_write_hard_replay_trace(...)` when a bundle exists. If Python dependency is missing, write an `incomplete` trace with the Rust entry and a skipped Python entry carrying `status="skipped"` and `error`.

- [ ] **Step 5: Run focused Python tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare.RunCompareTest.test_hard_replay_suite_writes_paired_prediction_rows
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Expected: both pass.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/run_compare.py benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py
git commit -m "feat: write bb90 hard replay trace"
```

### Task 3: Trace Verifier, Negative Controls, and Docs

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/verify_replay_trace.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay_trace.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: `<output-dir>/hard_replay_trace.json`.
- Produces: CLI exit 0 with `case_id=... basis=Z classification=logical_prediction_mismatch` for a valid mismatch trace, or exit 1 with named validation errors.

- [ ] **Step 1: Write failing verifier tests**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay_trace.py` with a `make_trace()` helper containing two decoder entries. Tests:

```python
def test_verify_trace_accepts_logical_prediction_mismatch(self):
    errors = verify_trace(make_trace())
    self.assertEqual(errors, [])

def test_verify_trace_rejects_missing_python_correction_support(self):
    trace = make_trace()
    del trace["decoders"][1]["correction_support"]
    self.assertIn("ldpc_bposd missing correction_support", "\n".join(verify_trace(trace)))

def test_verify_trace_rejects_unpaired_syndrome_metadata(self):
    trace = make_trace()
    trace["decoders"][1]["syndrome_support"] = [5, 8, 15]
    self.assertIn("decoder entries are not paired on syndrome metadata", "\n".join(verify_trace(trace)))
```

Also test CLI output with `tempfile` and `contextlib.redirect_stdout`:

```python
self.assertEqual(main([str(path)]), 0)
self.assertIn("case_id=bb90-p006-c10-seed12345-order7-hard-syndrome", stdout.getvalue())
self.assertIn("basis=Z", stdout.getvalue())
self.assertIn("classification=logical_prediction_mismatch", stdout.getvalue())
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay_trace
```

Expected: fails because `verify_replay_trace.py` does not exist.

- [ ] **Step 3: Implement verifier**

Create `verify_replay_trace.py`:

```python
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_TOP_LEVEL = (
    "schema_version",
    "case_id",
    "basis",
    "syndrome_support",
    "syndrome_weight",
    "expected_sampled_logical",
    "classification",
    "decoders",
)
REQUIRED_DECODER_FIELDS = (
    "decoder_impl",
    "status",
    "case_id",
    "basis",
    "syndrome_support",
    "expected_sampled_logical",
    "bp_osd_settings",
    "correction_support",
    "correction_weight",
    "residual_syndrome_matches",
    "residual_syndrome_weight",
    "residual_syndrome_support",
    "predicted_logical",
)


def verify_trace(trace: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for field in REQUIRED_TOP_LEVEL:
        if field not in trace:
            errors.append(f"trace missing {field}")
    if errors:
        return errors

    decoders = trace.get("decoders")
    if not isinstance(decoders, list):
        return ["trace decoders is not a list"]
    by_impl = {
        entry.get("decoder_impl"): entry
        for entry in decoders
        if isinstance(entry, dict)
    }
    for impl in ("rbposd", "ldpc_bposd"):
        if impl not in by_impl:
            errors.append(f"trace missing decoder entry {impl}")
    if errors:
        return errors

    for impl in ("rbposd", "ldpc_bposd"):
        _verify_decoder_entry(trace, by_impl[impl], errors)

    if not errors:
        pair_fields = ("case_id", "basis", "syndrome_support", "expected_sampled_logical")
        for field in pair_fields:
            if by_impl["rbposd"].get(field) != by_impl["ldpc_bposd"].get(field):
                errors.append("decoder entries are not paired on syndrome metadata")
                break

    expected_classification = _expected_classification(by_impl)
    if trace.get("classification") != expected_classification:
        errors.append(
            "trace classification mismatch: "
            f"expected {expected_classification}, got {trace.get('classification')}"
        )
    return errors
```

Add helper validation:

```python
def _verify_decoder_entry(trace: dict[str, Any], entry: dict[str, Any], errors: list[str]) -> None:
    impl = str(entry.get("decoder_impl", "<unknown>"))
    if entry.get("status") != "ok":
        errors.append(f"{impl} decoder entry is not ok")
        return
    for field in REQUIRED_DECODER_FIELDS:
        if field not in entry:
            errors.append(f"{impl} missing {field}")
    if errors:
        return
    for field in ("case_id", "basis", "syndrome_support", "expected_sampled_logical"):
        if entry.get(field) != trace.get(field):
            errors.append(f"{impl} is not paired with top-level {field}")
    correction_support = entry.get("correction_support")
    if not isinstance(correction_support, list) or not correction_support:
        errors.append(f"{impl} missing correction_support")
    elif entry.get("correction_weight") != len(correction_support):
        errors.append(f"{impl} correction_weight does not match correction_support")
    residual_support = entry.get("residual_syndrome_support")
    if not isinstance(residual_support, list):
        errors.append(f"{impl} residual_syndrome_support is not a list")
    elif entry.get("residual_syndrome_weight") != len(residual_support):
        errors.append(f"{impl} residual_syndrome_weight does not match residual_syndrome_support")
    if not isinstance(entry.get("residual_syndrome_matches"), bool):
        errors.append(f"{impl} residual_syndrome_matches is not boolean")
    if not isinstance(entry.get("predicted_logical"), list) or not entry.get("predicted_logical"):
        errors.append(f"{impl} missing predicted_logical")
```

Add classification and CLI:

```python
def _expected_classification(by_impl: dict[str, dict[str, Any]]) -> str:
    if any(entry.get("status") != "ok" for entry in by_impl.values()):
        return "incomplete"
    return (
        "matched"
        if by_impl["rbposd"].get("predicted_logical") == by_impl["ldpc_bposd"].get("predicted_logical")
        else "logical_prediction_mismatch"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace_path", type=Path)
    args = parser.parse_args(argv)
    trace = json.loads(args.trace_path.read_text())
    errors = verify_trace(trace)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(
        f"case_id={trace['case_id']} basis={trace['basis']} "
        f"classification={trace['classification']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Document the command**

Update `README.md` under `BB90 Hard-Syndrome Replay` with:

```markdown
The hard replay also writes `hard_replay_trace.json`, a one-case correction
trace for the pinned Z-basis syndrome. Validate it with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay_trace \
  /tmp/rstim-bb90-hard-replay/hard_replay_trace.json
```
```

- [ ] **Step 5: Run focused verifier tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay_trace
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay_trace
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/verify_replay_trace.py \
  benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay_trace.py \
  benchmarks/bb_circuit_bposd_compare/README.md
git commit -m "feat: verify bb90 hard replay trace"
```

## Final Verification

Run:

```bash
cargo build --release -p rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-trace \
  --rust-binary target/release/rsinter
python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay_trace \
  /tmp/rstim-bb90-hard-trace/hard_replay_trace.json
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare \
  benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay_trace \
  benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay
cargo test
```

Negative control:

```bash
cp /tmp/rstim-bb90-hard-trace/hard_replay_trace.json /tmp/hard_replay_trace_bad.json
python3 - <<'PY'
import json
from pathlib import Path
path = Path("/tmp/hard_replay_trace_bad.json")
trace = json.loads(path.read_text())
del trace["decoders"][1]["correction_support"]
path.write_text(json.dumps(trace, indent=2, sort_keys=True) + "\n")
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay_trace \
  /tmp/hard_replay_trace_bad.json
```

Expected: the negative control exits nonzero and names `ldpc_bposd missing correction_support`.
