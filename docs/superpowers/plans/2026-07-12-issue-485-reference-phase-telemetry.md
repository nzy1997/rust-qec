# Issue 485 Reference Phase Telemetry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic packed-reference phase counters, expose them through an opt-in reference worker request, and provide a standalone profile command.

**Architecture:** `rstim/src/data_path.rs` owns the public counter struct and increments traversal-level counters while interpreting packed-reference instructions. `PackedInverseTableau` increments canonical materialization, writeback, and pivot counters at the current private collapse boundaries. The reference-build worker serializes counters only when `include_phase_counters` is requested, and `benchmarks.rstim_vs_stim_simulator.profile_reference_build` writes separate profiling JSON without changing checked evidence bundles.

**Tech Stack:** Rust 2024, serde JSON worker protocol, existing `rstim` integration tests, Python 3 standard library benchmark tooling.

## Global Constraints

- Add `ReferenceBuildPhaseCounters` with at least `measurement_reset_batches`, `canonical_materializations`, `canonical_writebacks`, `direct_inverse_batches`, `transposed_collapse_batches`, `collapse_pivots`, `expanded_repeat_iterations`, and `measurement_bits`.
- Existing `build_reference_sample` must remain a bits-only compatibility wrapper.
- Existing checked reference-build bundle rows must not gain phase counters unless a worker request opts in.
- `build_reference` requests opt in with `include_phase_counters: true`.
- `profile_reference_build` must write profiling output separately from the M1 checked bundle schema.
- The canonical d11/r100 profile must print exactly `PASS reference phase profile batches=103 canonical=103 writebacks=2 repeats=99 bits=12121`.
- Negative controls `X 0; M 0` and `H 0; M 0` must both report one batch, but only `H 0; M 0` may report collapse/writeback.
- Do not optimize tableau operations, fold repeats, publish new timing claims, or broaden packed-reference gate support.
- Required issue verification command: `cargo build --release -p rstim --bin rstim_reference_build_worker` followed by `python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim --worker target/release/rstim_reference_build_worker --out /tmp/rstim-reference-phase-profile.json`.
- Required final Agent Desk verification command: `cargo test`.

---

## File Structure

- Modify `rstim/src/data_path.rs`: define `ReferenceBuildPhaseCounters`, attach it to `ReferenceSampleResult`, and update packed-reference traversal counters.
- Modify `rstim/src/sim/packed_inverse_tableau.rs`: add instrumented measurement/reset variants that update canonical materialization, writeback, and pivot counters.
- Modify `rstim/src/bin/rstim_reference_build_worker.rs`: add optional request field and optional response payload for phase counters.
- Modify `rstim/tests/packed_reference_routing.rs`: add focused API-level counter tests including canonical fixture and negative controls.
- Modify `rstim/tests/rstim_reference_build_worker.rs`: add worker opt-in/default counter protocol tests.
- Create `benchmarks/rstim_vs_stim_simulator/profile_reference_build.py`: standalone profiler CLI.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`: Python profiler protocol and output tests.

### Task 1: Add Failing Counter Tests

**Files:**
- Modify: `rstim/tests/packed_reference_routing.rs`
- Modify: `rstim/tests/rstim_reference_build_worker.rs`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`

**Interfaces:**
- Consumes planned `ReferenceSampleResult.phase_counters`, planned worker `include_phase_counters`, and planned Python module `benchmarks.rstim_vs_stim_simulator.profile_reference_build`.
- Produces failing coverage for surface fixture counters, negative controls, worker opt-in behavior, and profiler output.

- [ ] **Step 1: Add Rust API counter assertions**

Append these imports to `rstim/tests/packed_reference_routing.rs` if needed:

```rust
use rstim::data_path::ReferenceBuildPhaseCounters;
```

Append these tests:

```rust
fn assert_zero_future_counters(counters: &ReferenceBuildPhaseCounters) {
    assert_eq!(counters.direct_inverse_batches, 0);
    assert_eq!(counters.transposed_collapse_batches, 0);
}

#[test]
fn canonical_surface_fixture_reports_current_reference_phase_work() {
    let instrs = parse_circuit(SURFACE_D11_R100);
    let result = build_reference_sample_with_decision(&instrs).expect("reference sample builds");
    assert_packed_reference_decision(&result.decision);

    let counters = result.phase_counters;
    assert_eq!(result.bits.len(), 12_121);
    assert_all_false(&result.bits, "surface d11 r100 reference");
    assert_eq!(counters.measurement_reset_batches, 103);
    assert_eq!(counters.canonical_materializations, 103);
    assert_eq!(counters.canonical_writebacks, 2);
    assert_eq!(counters.expanded_repeat_iterations, 99);
    assert_eq!(counters.measurement_bits, 12_121);
    assert!(counters.collapse_pivots > 0);
    assert_zero_future_counters(&counters);
}

#[test]
fn phase_counters_distinguish_deterministic_and_collapsing_measurements() {
    let deterministic = build_reference_sample_with_decision(&parse_circuit("X 0\nM 0\n"))
        .expect("deterministic reference builds")
        .phase_counters;
    let collapsing = build_reference_sample_with_decision(&parse_circuit("H 0\nM 0\n"))
        .expect("collapsing reference builds")
        .phase_counters;

    assert_eq!(deterministic.measurement_reset_batches, 1);
    assert_eq!(collapsing.measurement_reset_batches, 1);
    assert_eq!(deterministic.canonical_materializations, 1);
    assert_eq!(collapsing.canonical_materializations, 1);
    assert_eq!(deterministic.canonical_writebacks, 0);
    assert_eq!(collapsing.canonical_writebacks, 1);
    assert_eq!(deterministic.collapse_pivots, 0);
    assert_eq!(collapsing.collapse_pivots, 1);
    assert_eq!(deterministic.measurement_bits, 1);
    assert_eq!(collapsing.measurement_bits, 1);
    assert_zero_future_counters(&deterministic);
    assert_zero_future_counters(&collapsing);
}
```

- [ ] **Step 2: Add worker opt-in tests**

In `rstim/tests/rstim_reference_build_worker.rs`, extend the first build loop so its existing requests continue without `include_phase_counters`, and assert:

```rust
assert!(built.get("phase_counters").is_none());
```

Append a new test:

```rust
#[test]
fn rstim_reference_build_worker_reports_phase_counters_only_when_requested() {
    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "H 0\nM 0\n").expect("write fixture");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    let loaded = read_response(&mut reader, &mut line);
    assert_eq!(loaded["type"], "loaded");

    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"build_reference","request_id":0,"include_phase_counters":true})
    )
    .expect("send build");
    let built = read_response(&mut reader, &mut line);
    let counters = built
        .get("phase_counters")
        .and_then(serde_json::Value::as_object)
        .expect("phase counters object");
    assert_eq!(counters["measurement_reset_batches"], json!(1));
    assert_eq!(counters["canonical_materializations"], json!(1));
    assert_eq!(counters["canonical_writebacks"], json!(1));
    assert_eq!(counters["collapse_pivots"], json!(1));
    assert_eq!(counters["expanded_repeat_iterations"], json!(0));
    assert_eq!(counters["measurement_bits"], json!(1));

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
```

- [ ] **Step 3: Add Python profiler tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`:

```python
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
PROTOCOL = "reference-build-v1"


class ProfileReferenceBuildTest(unittest.TestCase):
    def _write_worker(self, directory: Path, *, include_counters: bool = True) -> Path:
        worker = directory / "worker.py"
        counters_literal = {
            "measurement_reset_batches": 103,
            "canonical_materializations": 103,
            "canonical_writebacks": 2,
            "direct_inverse_batches": 0,
            "transposed_collapse_batches": 0,
            "collapse_pivots": 120,
            "expanded_repeat_iterations": 99,
            "measurement_bits": 12121,
        }
        worker.write_text(
            textwrap.dedent(
                f"""\
                #!{sys.executable}
                import argparse
                import json
                import sys

                parser = argparse.ArgumentParser()
                parser.add_argument("--protocol", required=True)
                args = parser.parse_args()
                assert args.protocol == {PROTOCOL!r}
                load = json.loads(sys.stdin.readline())
                print(json.dumps({{"protocol": {PROTOCOL!r}, "type": "loaded", "parse_count": 1, "measurement_bits": 12121}}), flush=True)
                build = json.loads(sys.stdin.readline())
                if build.get("include_phase_counters") is not True:
                    raise SystemExit("missing opt-in")
                response = {{
                    "protocol": {PROTOCOL!r},
                    "type": "reference_built",
                    "request_id": 0,
                    "backend": "packed_inverse",
                    "parse_count": 1,
                    "reference_build_count": 1,
                    "measurement_bits": 12121,
                    "packed_bytes": 1516,
                    "packed_base64": "AA==",
                    "byte_sha256": "0" * 64,
                    "timer_scope": "reference_build_only",
                    "elapsed_ns": 1,
                }}
                if {include_counters!r}:
                    response["phase_counters"] = {counters_literal!r}
                print(json.dumps(response), flush=True)
                """
            ),
            encoding="utf-8",
        )
        worker.chmod(0o755)
        return worker

    def test_profile_command_writes_json_and_pass_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            fixture = directory / "fixture.stim"
            fixture.write_text("M 0\n", encoding="utf-8")
            worker = self._write_worker(directory)
            out = directory / "profile.json"

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "benchmarks.rstim_vs_stim_simulator.profile_reference_build",
                    "--fixture",
                    str(fixture),
                    "--worker",
                    str(worker),
                    "--out",
                    str(out),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                result.stdout.strip(),
                "PASS reference phase profile batches=103 canonical=103 writebacks=2 repeats=99 bits=12121",
            )
            payload = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(payload["protocol"], PROTOCOL)
            self.assertEqual(payload["backend"], "packed_inverse")
            self.assertEqual(payload["phase_counters"]["measurement_reset_batches"], 103)
            self.assertEqual(payload["phase_counters"]["canonical_writebacks"], 2)

    def test_profile_command_rejects_missing_phase_counters(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            fixture = directory / "fixture.stim"
            fixture.write_text("M 0\n", encoding="utf-8")
            worker = self._write_worker(directory, include_counters=False)
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "benchmarks.rstim_vs_stim_simulator.profile_reference_build",
                    "--fixture",
                    str(fixture),
                    "--worker",
                    str(worker),
                    "--out",
                    str(directory / "profile.json"),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("phase_counters", result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 4: Run tests to verify RED**

Run:

```sh
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test rstim_reference_build_worker
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
```

Expected before implementation: Rust fails because `phase_counters` and the opt-in response do not exist; Python fails because `profile_reference_build` does not exist.

- [ ] **Step 5: Commit failing tests**

```sh
git add rstim/tests/packed_reference_routing.rs \
  rstim/tests/rstim_reference_build_worker.rs \
  benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py
git commit -m "test: specify reference phase counters"
```

### Task 2: Implement Rust Phase Counters

**Files:**
- Modify: `rstim/src/data_path.rs`
- Modify: `rstim/src/sim/packed_inverse_tableau.rs`
- Modify: `rstim/src/bin/rstim_reference_build_worker.rs`

**Interfaces:**
- Consumes tests from Task 1.
- Produces `ReferenceBuildPhaseCounters`, populated `ReferenceSampleResult.phase_counters`, and opt-in worker `phase_counters`.

- [ ] **Step 1: Add the public counter struct**

In `rstim/src/data_path.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct ReferenceBuildPhaseCounters {
    pub measurement_reset_batches: usize,
    pub canonical_materializations: usize,
    pub canonical_writebacks: usize,
    pub direct_inverse_batches: usize,
    pub transposed_collapse_batches: usize,
    pub collapse_pivots: usize,
    pub expanded_repeat_iterations: usize,
    pub measurement_bits: usize,
}
```

Add `pub phase_counters: ReferenceBuildPhaseCounters` to
`ReferenceSampleResult`.

- [ ] **Step 2: Count packed-reference traversal phases**

Change `build_packed_reference_sample` to return `(Vec<bool>, ReferenceBuildPhaseCounters)` and initialize:

```rust
let mut counters = ReferenceBuildPhaseCounters {
    measurement_bits: crate::stats::num_measurements(instrs),
    ..ReferenceBuildPhaseCounters::default()
};
```

Thread `&mut counters` through `packed_reference_instrs` and
`packed_reference_op`. In `Repeat`, increment:

```rust
counters.expanded_repeat_iterations = counters
    .expanded_repeat_iterations
    .saturating_add(usize::try_from(*count).unwrap_or(usize::MAX));
```

Before executing any measurement/reset operation branch, increment
`measurement_reset_batches` once. Use a helper:

```rust
fn is_measurement_reset_operation(name: &str) -> bool {
    matches!(name, "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" | "R" | "RZ" | "RX" | "RY")
}
```

- [ ] **Step 3: Instrument current canonical-row boundaries**

Import the counter type in `rstim/src/sim/packed_inverse_tableau.rs`:

```rust
use crate::data_path::ReferenceBuildPhaseCounters;
```

Add instrumented private helpers beside the existing methods:

```rust
fn measure_z_raw_biased_with_counters(
    &mut self,
    q: usize,
    counters: Option<&mut ReferenceBuildPhaseCounters>,
) -> bool
```

and:

```rust
fn measure_z_raw_many_biased_with_counters(
    &mut self,
    qubits: &[usize],
    counters: Option<&mut ReferenceBuildPhaseCounters>,
) -> Vec<bool>
```

Increment `canonical_materializations` immediately before `self.canonical_rows()`.
Increment `collapse_pivots` inside the `if let Some(p) = pivot` branch of
`measure_z_raw_biased_in_rows`. Increment `canonical_writebacks` immediately
before `self.replace_from_canonical_rows(&rows)` when `changed` is true.

Keep the existing public methods as wrappers that call the instrumented helpers
with `None`. Add `pub(crate)` instrumented wrappers for data path use:

```rust
pub(crate) fn measure_z_many_biased_with_counters(
    &mut self,
    targets: &[(usize, bool)],
    counters: &mut ReferenceBuildPhaseCounters,
) -> Vec<bool>
```

Repeat this wrapper pattern for `measure_x_biased`, `measure_y_biased`,
`measure_reset_z_many_biased`, `measure_reset_x_biased`,
`measure_reset_y_biased`, `reset_z_many_biased`, `reset_x_biased`, and
`reset_y_biased` so basis-changing operations still count the underlying Z
canonical work.

- [ ] **Step 4: Route data path calls through instrumented wrappers**

In `packed_reference_op`, replace measurement/reset calls with the new
`*_with_counters` variants and pass the traversal `counters`.

When packed construction succeeds, return:

```rust
Ok(ReferenceSampleResult {
    bits,
    decision: ReferenceSampleDecision::PackedInverse,
    phase_counters: counters,
})
```

When legacy fallback is used, keep compatibility by returning default counters
with only `measurement_bits` populated.

- [ ] **Step 5: Add worker opt-in serialization**

In `rstim/src/bin/rstim_reference_build_worker.rs`, import the counter type,
add to `BuildReferenceRequest`:

```rust
#[serde(default)]
include_phase_counters: bool,
```

Add to `ReferenceBuiltResponse`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
phase_counters: Option<ReferenceBuildPhaseCounters>,
```

Set it to `Some(reference.phase_counters)` only when the request opts in.

- [ ] **Step 6: Run focused Rust tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test rstim_reference_build_worker
```

Expected: both commands pass.

- [ ] **Step 7: Commit Rust implementation**

```sh
git add rstim/src/data_path.rs \
  rstim/src/sim/packed_inverse_tableau.rs \
  rstim/src/bin/rstim_reference_build_worker.rs
git commit -m "feat: add reference phase counters"
```

### Task 3: Add Standalone Profile Command

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/profile_reference_build.py`

**Interfaces:**
- Consumes worker protocol from Task 2.
- Produces `python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build --fixture <path> --worker <path> --out <json>`.

- [ ] **Step 1: Implement profiler module**

Create `benchmarks/rstim_vs_stim_simulator/profile_reference_build.py` with:

```python
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


PROTOCOL = "reference-build-v1"
REPO_ROOT = Path(__file__).resolve().parents[2]
COUNTER_KEYS = (
    "measurement_reset_batches",
    "canonical_materializations",
    "canonical_writebacks",
    "direct_inverse_batches",
    "transposed_collapse_batches",
    "collapse_pivots",
    "expanded_repeat_iterations",
    "measurement_bits",
)
```

Implement a small `WorkerSession` that sets `PYTHONPATH` to `REPO_ROOT`, sends
JSONL requests, raises `ProfileError` on worker errors, and closes or aborts the
process. Send:

```python
{"protocol": PROTOCOL, "type": "load", "fixture_path": str(fixture)}
{"protocol": PROTOCOL, "type": "build_reference", "request_id": 0, "include_phase_counters": True}
```

Validate that `phase_counters` is a dictionary, contains every `COUNTER_KEYS`
entry, and each value is a nonnegative integer. Validate
`backend == "packed_inverse"` and `measurement_bits == counters["measurement_bits"]`.

Write this JSON shape to `--out`:

```python
{
    "protocol": PROTOCOL,
    "fixture_path": str(args.fixture),
    "worker_argv": [str(args.worker), "--protocol", PROTOCOL],
    "backend": response["backend"],
    "measurement_bits": response["measurement_bits"],
    "phase_counters": counters,
}
```

Print:

```python
print(
    "PASS reference phase profile "
    f"batches={counters['measurement_reset_batches']} "
    f"canonical={counters['canonical_materializations']} "
    f"writebacks={counters['canonical_writebacks']} "
    f"repeats={counters['expanded_repeat_iterations']} "
    f"bits={counters['measurement_bits']}"
)
```

- [ ] **Step 2: Run profiler tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
```

Expected: tests pass.

- [ ] **Step 3: Commit profiler implementation**

```sh
git add benchmarks/rstim_vs_stim_simulator/profile_reference_build.py
git commit -m "feat: add reference phase profile command"
```

### Task 4: Verify Issue Acceptance And Whole Repo

**Files:**
- No planned source edits unless verification reveals a defect.

**Interfaces:**
- Consumes all previous tasks.
- Produces verified branch ready for PR.

- [ ] **Step 1: Build release worker**

Run:

```sh
cargo build --release -p rstim --bin rstim_reference_build_worker
```

Expected: exit 0.

- [ ] **Step 2: Run required profile command**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --worker target/release/rstim_reference_build_worker \
  --out /tmp/rstim-reference-phase-profile.json
```

Expected stdout:

```text
PASS reference phase profile batches=103 canonical=103 writebacks=2 repeats=99 bits=12121
```

- [ ] **Step 3: Run focused tests**

Run:

```sh
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test rstim_reference_build_worker
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
```

Expected: all pass.

- [ ] **Step 4: Run required final test suite**

Run:

```sh
cargo test
```

Expected: exit 0.

- [ ] **Step 5: Commit any verification fixes**

If verification required changes, commit them with a focused message:

```sh
git add <changed-files>
git commit -m "fix: align reference phase telemetry verification"
```

If no changes are needed, do not create an empty commit.

## Self-Review

- Spec coverage: every issue field is implemented by Rust counters, worker opt-in, profile command, and tests.
- Placeholder scan: no placeholder markers or deferred implementation steps remain.
- Type consistency: `ReferenceBuildPhaseCounters`, `phase_counters`, and `include_phase_counters` are named consistently across Rust, JSON, and Python tests.
