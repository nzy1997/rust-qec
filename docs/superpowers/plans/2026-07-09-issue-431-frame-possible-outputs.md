# Issue 431 Frame Possible Outputs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add integration tests that prove representative `rstim` frame-sampler rows are possible under an independent tableau-style replay helper.

**Architecture:** Add one test-only helper in `rstim/tests/frame_possible_outputs.rs`. The sampler under test uses existing `sample_batch_with_options` with the interpreted frame backend, while the oracle uses `StabilizerState` directly and rejects deterministic measurement mismatches.

**Tech Stack:** Rust 2024, `rstim` integration tests, `rand::rngs::StdRng`, `rstim::parser`, `rstim::sampler`, `rstim::sim::tableau::StabilizerState`.

## Global Constraints

- Create `rstim/tests/frame_possible_outputs.rs`.
- Use checked inline `.stim` snippets, not external fixtures.
- Prefer existing `rstim` APIs; do not add public API surface.
- Include small entangling circuits, measurement/reset cases, and a surface-code-shaped smoke circuit.
- Include `sampled_outputs_are_possible_for_entangling_circuits`.
- Include `impossible_output_is_rejected`.
- For `H 0; CNOT 0 1; M 0; M 1`, `00` and `11` are possible while `01` and `10` are impossible.
- Keep the test fast under `cargo test --workspace`.
- Do not compare runtime against Stim.
- Do not port Stim's full frame simulator test suite.

---

### Task 1: Frame Possible Output Integration Tests

**Files:**
- Create: `rstim/tests/frame_possible_outputs.rs`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines`, `rstim::sampler::sample_batch_with_options`, `rstim::sampler::SampleOptions`, `rstim::sampler::SamplingBackend`, `rstim::sim::tableau::StabilizerState`.
- Produces: local helper functions `assert_sampled_outputs_are_possible`, `is_output_possible`, and tests `sampled_outputs_are_possible_for_entangling_circuits` and `impossible_output_is_rejected`.

- [ ] **Step 1: Write the failing regression test with a deliberately permissive helper**

Create `rstim/tests/frame_possible_outputs.rs` with imports, test cases, and a temporary helper that returns `Ok(true)` from `is_output_possible`. The final helper is not implemented in this step; this makes the negative fixture fail for the right reason.

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use rstim::sampler::{
    sample_batch_with_options, SampleOptions, SampleOutputMode, SamplingBackend,
};

fn assert_sampled_outputs_are_possible(stim: &str, shots: usize) {
    let instrs = parse_lines(stim).unwrap();
    let mut rng = StdRng::seed_from_u64(0x431);
    let out = sample_batch_with_options(
        &instrs,
        shots,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            output_mode: SampleOutputMode::MeasurementsOnly,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    for shot in 0..shots {
        let row: Vec<bool> = (0..out.measurements.num_major())
            .map(|m| out.measurements.get(m, shot))
            .collect();
        assert!(
            is_output_possible(&instrs, &row).unwrap(),
            "shot {shot} produced impossible row {row:?}"
        );
    }
}

fn is_output_possible(_instrs: &[StimInstr], _row: &[bool]) -> Result<bool, String> {
    Ok(true)
}

#[test]
fn sampled_outputs_are_possible_for_entangling_circuits() {
    for stim in [
        "H 0\nCNOT 0 1\nM 0 1\n",
        "H 0\nCNOT 0 1\nMR 0\nM 1\n",
        "RX 0 1\nH 0\nCNOT 0 1\nMRX 0\nMX 1\n",
        "R 0 1 2\nR 3 4\nTICK\nH 0 1 2\nCNOT 0 3 1 4 1 3 2 4\nMR 3 4\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nTICK\nM 0 1 2\nDETECTOR rec[-3] rec[-2] rec[-5]\nDETECTOR rec[-2] rec[-1] rec[-4]\nOBSERVABLE_INCLUDE(0) rec[-3]\n",
    ] {
        assert_sampled_outputs_are_possible(stim, 32);
    }
}

#[test]
fn impossible_output_is_rejected() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0\nM 1\n").unwrap();

    assert!(is_output_possible(&instrs, &[false, false]).unwrap());
    assert!(is_output_possible(&instrs, &[true, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[false, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[true, false]).unwrap());
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
cargo test -p rstim --test frame_possible_outputs -- --nocapture
```

Expected: the command fails in `impossible_output_is_rejected` because the temporary helper accepts `[false, true]` or `[true, false]`.

- [ ] **Step 3: Implement the independent possible-output helper**

Replace the temporary helper with a narrow tableau replay implementation. Use `StabilizerState` for the reference state, a tiny RNG that returns the desired candidate measurement bit when a random tableau branch asks for entropy, and explicit instruction handling for only the operations used by the tests.

The implementation must include these local pieces:

```rust
use rand::RngCore;
use rstim::executor::max_qubit;
use rstim::ir::StimTarget;
use rstim::sim::tableau::StabilizerState;

#[derive(Default)]
struct ForcedBitRng {
    bit: bool,
}

impl ForcedBitRng {
    fn force(&mut self, bit: bool) {
        self.bit = bit;
    }
}

impl RngCore for ForcedBitRng {
    fn next_u32(&mut self) -> u32 {
        u32::from(self.bit)
    }

    fn next_u64(&mut self) -> u64 {
        u64::from(self.bit)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.fill(u8::from(self.bit));
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

struct PossibleOutputReplay<'a> {
    state: StabilizerState,
    row: &'a [bool],
    next_measurement: usize,
    rng: ForcedBitRng,
}
```

Implement `is_output_possible` by constructing `PossibleOutputReplay` with `max_qubit(instrs)?`, recursively running instructions including `REPEAT`, and returning `Ok(replay.next_measurement == row.len())`.

Implement replay operations with exact behavior:

```rust
fn run_op(&mut self, name: &str, targets: &[StimTarget]) -> Result<bool, String> {
    match name {
        "I" | "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => Ok(true),
        "H" => { for q in qubits(targets)? { self.state.h(q); } Ok(true) }
        "X" => { for q in qubits(targets)? { self.state.x_gate(q); } Ok(true) }
        "Z" => { for q in qubits(targets)? { self.state.z_gate(q); } Ok(true) }
        "CX" | "CNOT" => { for (c, t) in qubit_pairs(targets)? { self.state.cx(c, t); } Ok(true) }
        "CZ" => { for (a, b) in qubit_pairs(targets)? { self.state.cz(a, b); } Ok(true) }
        "R" | "RZ" => { for q in qubits(targets)? { self.state.reset_z_biased(q); } Ok(true) }
        "RX" => { for q in qubits(targets)? { self.state.reset_x_biased(q); } Ok(true) }
        "M" | "MZ" => self.measure_each(targets, MeasureBasis::Z, false),
        "MX" => self.measure_each(targets, MeasureBasis::X, false),
        "MR" | "MRZ" => self.measure_each(targets, MeasureBasis::Z, true),
        "MRX" => self.measure_each(targets, MeasureBasis::X, true),
        other => Err(format!("unsupported instruction in possible-output helper: {other}")),
    }
}
```

For `MeasureBasis::X`, wrap Z-basis measurement/reset with `H`; for `MeasureBasis::Z`, call `measure_z` and reset helpers directly. For inverted measurement targets, compare the candidate row bit after applying target inversion. If the requested row runs out of bits or leaves extra bits after replay, return `Ok(false)`.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```sh
cargo test -p rstim --test frame_possible_outputs -- --nocapture
```

Expected: exit 0 and output shows `sampled_outputs_are_possible_for_entangling_circuits ... ok` and `impossible_output_is_rejected ... ok`.

- [ ] **Step 5: Run rustfmt**

Run:

```sh
cargo fmt
```

Expected: exit 0 with no formatting errors.

- [ ] **Step 6: Run the required full gate**

Run:

```sh
cargo test
```

Expected: exit 0.

- [ ] **Step 7: Commit implementation**

Run:

```sh
git add rstim/tests/frame_possible_outputs.rs
git commit -m "test: add frame possible-output regressions"
```

Expected: commit succeeds with only the new integration test file staged.
