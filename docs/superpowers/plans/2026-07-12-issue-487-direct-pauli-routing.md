# Issue 487 Direct Pauli Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route packed reference `M`, `MX`, `MY`, `MR`, `MRX`, `MRY`, `R`, `RX`, and `RY` through direct packed-inverse collapse without canonical materialization.

**Architecture:** Keep `collapse_z_many_biased` as the only production collapse primitive. Add X/Y batch wrappers that apply unique-target basis transforms, call the Z batch once, and undo transforms; reset-style duplicate targets stay sequential.

**Tech Stack:** Rust 2024 Cargo workspace, `rstim` integration tests, Python unittest benchmark helpers.

## Global Constraints

- Production packed reference sampling must not call `canonical_rows` or `replace_from_canonical_rows`.
- Z operations use the direct batch primitive.
- X/Y operations apply basis transforms to the batch, collapse once, and undo them.
- Unique targets share one transposed view.
- Duplicate measurement-reset targets use sequential semantics.
- Old canonical strategy may remain internal for a later benchmark-only baseline.
- Do not implement repeat-cycle detection, publish checked performance evidence, broaden gate support, or remove legacy fallback.

---

### Task 1: Add Routing And Profile Regression Tests

**Files:**
- Modify: `rstim/tests/packed_reference_routing.rs`
- Modify: `rstim/tests/rstim_reference_build_worker.rs`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`

**Interfaces:**
- Consumes: `build_reference_sample_with_decision(&[StimInstr]) -> ReferenceSampleResult`
- Produces: tests that fail while production still uses canonical materialization.

- [ ] **Step 1: Write failing packed-reference routing tests**

Add tests that assert the required direct counters and duplicate reset behavior:

```rust
#[test]
fn supported_pauli_measurements_use_direct_inverse_collapse() {
    for (circuit, expected_bits, expected_pivots) in [
        ("H 0\nM 0\n", vec![false], 1),
        ("H 0 1\nMX 0 1\n", vec![false, false], 2),
        ("H 0 1\nS 0 1\nMY 0 1\n", vec![false, false], 2),
    ] {
        let result = build_reference_sample_with_decision(&parse_circuit(circuit))
            .expect("reference sample builds");
        assert_packed_reference_decision(&result.decision);
        assert_eq!(result.bits, expected_bits, "circuit:\n{circuit}");
        assert_eq!(result.phase_counters.canonical_materializations, 0);
        assert_eq!(result.phase_counters.canonical_writebacks, 0);
        assert_eq!(result.phase_counters.direct_inverse_batches, 1);
        assert_eq!(result.phase_counters.transposed_collapse_batches, 1);
        assert_eq!(result.phase_counters.collapse_pivots, expected_pivots);
    }
}

#[test]
fn supported_pauli_resets_use_direct_inverse_collapse_and_preserve_duplicates() {
    let duplicate = build_reference_sample_with_decision(&parse_circuit("X 0\nMR 0 0\nM 0\n"))
        .expect("duplicate reset reference sample builds");
    assert_packed_reference_decision(&duplicate.decision);
    assert_eq!(duplicate.bits, vec![true, false, false]);
    assert_eq!(duplicate.phase_counters.canonical_materializations, 0);
    assert_eq!(duplicate.phase_counters.canonical_writebacks, 0);

    for (circuit, expected_bits, expected_batches) in [
        ("X 0 1\nMR 0 1\nM 0 1\n", vec![true, true, false, false], 2),
        ("H 0 1\nZ 0 1\nMRX 0 1\nMX 0 1\n", vec![true, true, false, false], 2),
        (
            "H 0 1\nS_DAG 0 1\nMRY 0 1\nMY 0 1\n",
            vec![true, true, false, false],
            2,
        ),
        ("X 0 1\nR 0 1\nM 0 1\n", vec![false, false], 2),
        ("H 0 1\nZ 0 1\nRX 0 1\nMX 0 1\n", vec![false, false], 2),
        (
            "H 0 1\nS_DAG 0 1\nRY 0 1\nMY 0 1\n",
            vec![false, false],
            2,
        ),
    ] {
        let result = build_reference_sample_with_decision(&parse_circuit(circuit))
            .expect("reference sample builds");
        assert_packed_reference_decision(&result.decision);
        assert_eq!(result.bits, expected_bits, "circuit:\n{circuit}");
        assert_eq!(result.phase_counters.canonical_materializations, 0);
        assert_eq!(result.phase_counters.canonical_writebacks, 0);
        assert_eq!(result.phase_counters.direct_inverse_batches, expected_batches);
    }
}
```

Update `canonical_surface_fixture_reports_current_reference_phase_work` and
`rstim_reference_build_worker_reports_canonical_surface_phase_counters` to
expect `canonical_materializations=0`, `canonical_writebacks=0`,
`direct_inverse_batches=103`, `transposed_collapse_batches=2`, and
`collapse_pivots=120`.

- [ ] **Step 2: Update Python profile unit expectation**

Set `DEFAULT_COUNTERS` to direct counters and expect the new pass line:

```python
DEFAULT_COUNTERS = {
    "measurement_reset_batches": 103,
    "canonical_materializations": 0,
    "canonical_writebacks": 0,
    "direct_inverse_batches": 103,
    "transposed_collapse_batches": 2,
    "collapse_pivots": 120,
    "expanded_repeat_iterations": 99,
    "measurement_bits": 12121,
}
```

Expected stdout:

```text
PASS reference phase profile batches=103 canonical=0 writebacks=0 transposed=2 pivots=120 repeats=99 bits=12121
```

- [ ] **Step 3: Run tests to verify RED**

Run:

```sh
cargo test -p rstim --test packed_reference_routing supported_pauli_measurements_use_direct_inverse_collapse
cargo test -p rstim --test rstim_reference_build_worker rstim_reference_build_worker_reports_canonical_surface_phase_counters
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build.ProfileReferenceBuildTest.test_profile_command_writes_json_and_pass_line
```

Expected: each command fails on the old canonical counters or missing profile fields.

### Task 2: Route Packed Tableau Operations Through Direct Z Collapse

**Files:**
- Modify: `rstim/src/sim/packed_inverse_tableau.rs`
- Modify: `rstim/src/data_path.rs`

**Interfaces:**
- Consumes: `collapse_z_many_biased(&[(usize, bool)], &mut ReferenceBuildPhaseCounters) -> Vec<bool>`
- Produces: batch methods for Z/X/Y measurements, measure-resets, and resets that use direct collapse for counter-bearing production paths.

- [ ] **Step 1: Implement direct optional-counter wrappers**

Change `measure_z_many_biased_with_optional_counters` to call direct collapse
when counters are present, and use a local default counter for the public
counter-free method:

```rust
fn measure_z_many_biased_with_optional_counters(
    &mut self,
    targets: &[(usize, bool)],
    counters: Option<&mut ReferenceBuildPhaseCounters>,
) -> Vec<bool> {
    match counters {
        Some(counters) => self.collapse_z_many_biased(targets, counters),
        None => {
            let mut counters = ReferenceBuildPhaseCounters::default();
            self.collapse_z_many_biased(targets, &mut counters)
        }
    }
}
```

- [ ] **Step 2: Add unique-target helpers and X/Y batch wrappers**

Add private helpers:

```rust
fn unique_target_qubits(targets: &[(usize, bool)]) -> Vec<usize> { ... }
fn unique_qubits(qubits: &[usize]) -> Vec<usize> { ... }
```

Add methods:

```rust
pub fn measure_x_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool>;
pub(crate) fn measure_x_many_biased_with_counters(...);
pub fn measure_y_many_biased(&mut self, targets: &[(usize, bool)]) -> Vec<bool>;
pub(crate) fn measure_y_many_biased_with_counters(...);
```

The X wrapper applies `h` to unique targets, calls
`measure_z_many_biased_with_optional_counters`, then applies `h` again. The Y
wrapper applies `s_dag` then `h`, calls the Z method, then applies `h` and `s`.

- [ ] **Step 3: Route measure-reset and reset batches**

Change Z reset paths to direct batch for unique targets and sequential direct
single-target calls for duplicate targets. Add X/Y batch reset wrappers that
use unique basis transforms around the Z reset batch.

For duplicates:

```rust
if has_duplicate_qubits(&qubits) {
    return targets.iter().map(|&(q, inverted)| {
        self.measure_reset_z_biased_with_optional_counters(q, inverted, counters.as_deref_mut())
    }).collect();
}
```

For unique Z measure-reset:

```rust
let reported = self.measure_z_many_biased_with_optional_counters(targets, counters);
for (&(q, inverted), &bit) in targets.iter().zip(&reported) {
    if bit ^ inverted {
        self.x_gate(q);
    }
}
reported
```

- [ ] **Step 4: Route data-path operation batches**

In `rstim/src/data_path.rs`, replace per-target loops for `MX`, `MY`, `MRX`,
`MRY`, `RX`, and `RY` with the new batch methods:

```rust
"MX" => {
    measurements.extend(tableau.measure_x_many_biased_with_counters(
        &qubits_with_inversion(targets)?,
        counters,
    ));
}
```

Use analogous calls for `MY`, `MRX`, `MRY`, `RX`, and `RY`.

- [ ] **Step 5: Run focused GREEN tests**

Run:

```sh
cargo test -p rstim --test packed_reference_routing supported_pauli_measurements_use_direct_inverse_collapse
cargo test -p rstim --test packed_reference_routing supported_pauli_resets_use_direct_inverse_collapse_and_preserve_duplicates
cargo test -p rstim --test packed_inverse_tableau_measurement
```

Expected: all pass.

### Task 3: Update Reference Profile Output

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/profile_reference_build.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py`

**Interfaces:**
- Consumes: `ReferenceBuildPhaseCounters` JSON keys.
- Produces: profile pass line containing `transposed` and `pivots`.

- [ ] **Step 1: Print transposed and pivot counters**

Change the pass-line print in `profile_reference_build.py`:

```python
print(
    "PASS reference phase profile "
    f"batches={counters['measurement_reset_batches']} "
    f"canonical={counters['canonical_materializations']} "
    f"writebacks={counters['canonical_writebacks']} "
    f"transposed={counters['transposed_collapse_batches']} "
    f"pivots={counters['collapse_pivots']} "
    f"repeats={counters['expanded_repeat_iterations']} "
    f"bits={counters['measurement_bits']}"
)
```

- [ ] **Step 2: Run Python GREEN test**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build
```

Expected: all tests pass.

### Task 4: Final Verification And PR

**Files:**
- No planned source edits.

**Interfaces:**
- Consumes: completed implementation.
- Produces: verified branch pushed as a pull request.

- [ ] **Step 1: Run issue focused tests**

Run:

```sh
cargo test -p rstim \
  --test packed_inverse_direct_collapse \
  --test packed_inverse_tableau_measurement \
  --test packed_reference_routing
```

Expected: all tests pass.

- [ ] **Step 2: Run required release profile**

Run:

```sh
cargo build --release -p rstim --bin rstim_reference_build_worker
python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --worker target/release/rstim_reference_build_worker \
  --out /tmp/rstim-direct-inverse-profile.json
```

Expected output:

```text
PASS reference phase profile batches=103 canonical=0 writebacks=0 transposed=2 pivots=120 repeats=99 bits=12121
```

- [ ] **Step 3: Run source-grounded distribution verifier**

Run:

```sh
cargo build --release -p rstim --bin rstim
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --rstim target/release/rstim --shots 10000 --seeds 7 \
  --out /tmp/rstim-direct-inverse-distributions.json
```

Expected: output begins `PASS distribution correctness`.

- [ ] **Step 4: Run repository gate**

Run:

```sh
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit, push, and create PR**

Run:

```sh
git status --short
git add docs/superpowers/plans/2026-07-12-issue-487-direct-pauli-routing.md rstim/src/sim/packed_inverse_tableau.rs rstim/src/data_path.rs rstim/tests/packed_reference_routing.rs rstim/tests/rstim_reference_build_worker.rs benchmarks/rstim_vs_stim_simulator/profile_reference_build.py benchmarks/rstim_vs_stim_simulator/tests/test_profile_reference_build.py
git commit -m "Route packed Pauli reference ops through direct collapse"
git push -u origin agent/issue-487-route-batched-pauli-measurement-and-reset-throug-run-1
gh pr create --base master --head agent/issue-487-route-batched-pauli-measurement-and-reset-throug-run-1 --title "Route batched Pauli reference ops through direct collapse" --body "<summary, tests, Closes #487>"
```

Expected: PR URL is returned.

## Self-Review

Spec coverage is complete: every operation named in issue #487 is assigned to
Task 2, profile output is assigned to Task 3, and verification commands match
the issue. The plan has no placeholder text, and method names are consistent
with the existing `PackedInverseTableau` naming pattern.
