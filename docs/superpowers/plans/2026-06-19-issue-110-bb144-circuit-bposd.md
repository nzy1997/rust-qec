# Issue #110 BB144 Circuit BP-OSD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a circuit-level BP-OSD memory reproduction path for the upstream `[[144,12,12]]` bivariate-bicycle code.

**Architecture:** Add a focused `rsinter::bb_circuit_memory` module that owns BB144 construction, the upstream syndrome schedule, effective single-fault decoder construction, stochastic circuit sampling, and the two-stage BP-OSD trial loop. Expose it through a small `rsinter bb-circuit-bposd-memory` CLI subcommand that prints the upstream four-column result line.

**Tech Stack:** Rust 2024, `rsinter` CLI/library, existing `rbposd` decoder API, `rand`, `cargo test`.

## Global Constraints

- Default code parameters must be `ell = 12`, `m = 6`, `a1 = 3`, `a2 = 1`, `a3 = 2`, `b1 = 3`, `b2 = 1`, `b3 = 2`.
- Default run parameters must be `physical_error_rate = 0.003`, `num_cycles = 12`, `num_trials = 50_000`, `max_bp_iterations = 10000`, and `osd_order = 7`.
- CLI output must be one tab-separated line with `physical_error_rate`, `num_cycles`, `num_trials`, `num_failed_trials`.
- The noisy memory circuit must use `num_cycles * cycle`; effective decoder construction and trial logical extraction must append two additional noiseless cycles.
- The schedule arrays must be `sX = [idle, 1, 4, 3, 5, 0, 2]` and `sZ = [3, 5, 0, 1, 2, 4, idle]`.
- The circuit-level error model must match upstream IDLE, CNOT, PrepX, PrepZ, MeasX, and MeasZ insertion rules.
- Effective model single-fault probabilities must be `p`, `2p/3`, and `4p/15` as specified in issue #110.
- Decode Z faults first using X-check syndrome history; decode X faults second only if Z decoding predicts the correct logical vector.
- CI tests must use small smoke configurations; do not run the 50,000-trial default in tests.
- Keep changes scoped to `rsinter` and workflow docs unless a compile error proves a shared crate change is required.

---

## File Structure

- Create: `rsinter/src/bb_circuit_memory.rs`
  - Fixed BB144 code construction, GF(2) helpers, syndrome cycle builder, effective model builder, sampler, BP-OSD trial runner, and public config/result types.
- Modify: `rsinter/src/lib.rs`
  - Export the new module as `pub mod bb_circuit_memory;`.
- Modify: `rsinter/src/bin/rsinter.rs`
  - Add the `bb-circuit-bposd-memory` CLI subcommand and print the four-field result line.
- Create: `rsinter/tests/bb_circuit_memory.rs`
  - Library tests for BB144 shape, schedule counts, effective model smoke construction, and no-fault smoke trials.
- Modify: `rsinter/tests/bench_cli.rs`
  - Add a CLI smoke test for the new subcommand's output format.

## Task 1: BB144 Code And Syndrome Cycle

**Files:**
- Create: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/src/lib.rs`
- Create: `rsinter/tests/bb_circuit_memory.rs`

**Interfaces:**
- Produces: `BbCode`, `SyndromeCycle`, `Operation`, `OperationKind`, `BivariateBicycleParams::upstream_default()`, and `build_upstream_code() -> Result<BbCode, String>`.
- Produces: `build_syndrome_cycle(&BbCode) -> SyndromeCycle`.

- [ ] **Step 1: Write failing BB144 shape and cycle tests**

Create `rsinter/tests/bb_circuit_memory.rs` with these tests:

```rust
use rsinter::bb_circuit_memory::{
    build_syndrome_cycle, build_upstream_code, OperationKind,
};

#[test]
fn upstream_bb144_code_has_expected_shape() {
    let code = build_upstream_code().unwrap();

    assert_eq!(code.ell(), 12);
    assert_eq!(code.m(), 6);
    assert_eq!(code.n2(), 72);
    assert_eq!(code.n(), 144);
    assert_eq!(code.k(), 12);
    assert_eq!(code.x_checks().len(), 72);
    assert_eq!(code.z_checks().len(), 72);
    assert_eq!(code.data_qubits().len(), 144);
    assert_eq!(code.num_circuit_qubits(), 288);

    assert!(code.hx_rows().iter().all(|row| row.len() == 6));
    assert!(code.hz_rows().iter().all(|row| row.len() == 6));
    assert_eq!(code.logical_x_rows().len(), 12);
    assert_eq!(code.logical_z_rows().len(), 12);
}

#[test]
fn upstream_syndrome_cycle_has_expected_schedule_counts() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);

    assert_eq!(cycle.operations().len(), 1440);
    assert_eq!(cycle.count(OperationKind::Cnot), 864);
    assert_eq!(cycle.count(OperationKind::Idle), 288);
    assert_eq!(cycle.count(OperationKind::PrepX), 72);
    assert_eq!(cycle.count(OperationKind::PrepZ), 72);
    assert_eq!(cycle.count(OperationKind::MeasX), 72);
    assert_eq!(cycle.count(OperationKind::MeasZ), 72);
    assert_eq!(cycle.sx_labels(), ["idle", "1", "4", "3", "5", "0", "2"]);
    assert_eq!(cycle.sz_labels(), ["3", "5", "0", "1", "2", "4", "idle"]);
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run:

```sh
cargo test -p rsinter --test bb_circuit_memory upstream_bb144_code_has_expected_shape upstream_syndrome_cycle_has_expected_schedule_counts
```

Expected: FAIL because `rsinter::bb_circuit_memory` does not exist.

- [ ] **Step 3: Implement the fixed code and cycle surface**

Create `rsinter/src/bb_circuit_memory.rs` with:

- `pub struct BivariateBicycleParams { ell, m, a1, a2, a3, b1, b2, b3 }`
- `impl BivariateBicycleParams { pub fn upstream_default() -> Self }`
- compact qubit ids using `usize` indices in upstream order:
  `0..n2` for X checks, `n2..2*n2` for left data, `2*n2..3*n2` for right data, and `3*n2..4*n2` for Z checks.
- `BbCode` getters used by the tests.
- private cyclic-shift row helpers for A and B terms; row `i = x * m + y`, `x^a` maps to `((x + a) % ell) * m + y`, and `y^a` maps to `x * m + ((y + a) % m)`.
- sparse `hx = [A, B]` and `hz = [B^T, A^T]`.
- private GF(2) `rank`, `rref`, `nullspace`, `in_row_span`, and `select_logical_rows` helpers.
- pure X logical rows from `nullspace(hz)` modulo `rowspace(hx)` and pure Z logical rows from `nullspace(hx)` modulo `rowspace(hz)`.
- `OperationKind` and `Operation` enums/structs for `Idle`, `Cnot`, `PrepX`, `PrepZ`, `MeasX`, and `MeasZ`.
- `SyndromeCycle` with `operations()`, `count(kind)`, `sx_labels()`, and `sz_labels()` methods.
- `build_syndrome_cycle(&BbCode)` matching the issue #110 round structure.

Add `pub mod bb_circuit_memory;` to `rsinter/src/lib.rs`.

- [ ] **Step 4: Run Task 1 tests**

Run:

```sh
cargo test -p rsinter --test bb_circuit_memory upstream_bb144_code_has_expected_shape upstream_syndrome_cycle_has_expected_schedule_counts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

Run:

```sh
git add rsinter/src/bb_circuit_memory.rs rsinter/src/lib.rs rsinter/tests/bb_circuit_memory.rs
git commit -m "feat: construct bb144 circuit memory model"
```

## Task 2: Effective Decoder Model And Smoke Trial Runner

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bb_circuit_memory.rs`

**Interfaces:**
- Produces: `SimulationConfig`, `SimulationResult`, `EffectiveDecoderModel`, `build_effective_models(&BbCode, &SyndromeCycle, &SimulationConfig) -> Result<EffectiveModels, String>`, and `run_simulation(SimulationConfig) -> Result<SimulationResult, String>`.

- [ ] **Step 1: Add failing effective model and no-fault smoke tests**

Append to `rsinter/tests/bb_circuit_memory.rs`:

```rust
use rsinter::bb_circuit_memory::{
    build_effective_models, run_simulation, SimulationConfig,
};

#[test]
fn one_cycle_effective_models_have_expected_syndrome_rows() {
    let code = build_upstream_code().unwrap();
    let cycle = build_syndrome_cycle(&code);
    let config = SimulationConfig {
        physical_error_rate: 0.003,
        num_cycles: 1,
        num_trials: 1,
        seed: Some(7),
        max_bp_iterations: 10,
        osd_order: 0,
    };

    let models = build_effective_models(&code, &cycle, &config).unwrap();

    assert_eq!(models.z_faults.decoder.num_checks(), 72 * 3);
    assert_eq!(models.x_faults.decoder.num_checks(), 72 * 3);
    assert_eq!(models.z_faults.first_logical_row, 72 * 3);
    assert_eq!(models.x_faults.first_logical_row, 72 * 3);
    assert!(!models.z_faults.channel_probs.is_empty());
    assert!(!models.x_faults.channel_probs.is_empty());
    assert_eq!(
        models.z_faults.decoder.num_bits(),
        models.z_faults.channel_probs.len()
    );
    assert_eq!(
        models.x_faults.decoder.num_bits(),
        models.x_faults.channel_probs.len()
    );
}

#[test]
fn tiny_seeded_smoke_run_reports_zero_failures_without_sampled_faults() {
    let result = run_simulation(SimulationConfig {
        physical_error_rate: 1.0e-12,
        num_cycles: 1,
        num_trials: 2,
        seed: Some(1),
        max_bp_iterations: 10,
        osd_order: 0,
    })
    .unwrap();

    assert_eq!(result.physical_error_rate, 1.0e-12);
    assert_eq!(result.num_cycles, 1);
    assert_eq!(result.num_trials, 2);
    assert_eq!(result.num_failed_trials, 0);
}
```

- [ ] **Step 2: Run the new tests and confirm they fail**

Run:

```sh
cargo test -p rsinter --test bb_circuit_memory one_cycle_effective_models_have_expected_syndrome_rows tiny_seeded_smoke_run_reports_zero_failures_without_sampled_faults
```

Expected: FAIL because effective models and simulation are not implemented.

- [ ] **Step 3: Implement fault propagation and effective models**

In `rsinter/src/bb_circuit_memory.rs`:

- Add `SimulationConfig` with the exact public fields used by tests and `impl Default` with upstream defaults.
- Add `SimulationResult` with `physical_error_rate`, `num_cycles`, `num_trials`, and `num_failed_trials`.
- Add `EffectiveModels { z_faults, x_faults }` and `EffectiveDecoderModel { decoder: rbposd::ParityCheckMatrix, augmented_columns: Vec<Vec<usize>>, channel_probs: Vec<f64>, first_logical_row: usize }`.
- Add a small internal `PauliFault` enum for single-qubit and two-qubit X/Z/Y faults.
- Add propagation functions for Z-error state and X-error state:
  - Z state CNOT propagation: `state[control] ^= state[target]`.
  - X state CNOT propagation: `state[target] ^= state[control]`.
  - PrepX resets Z state on that qubit; PrepZ resets X state on that qubit.
  - MeasX records X-check Z-state syndrome; MeasZ records Z-check X-state syndrome.
  - Fault insertion toggles the relevant X or Z marginal bits using the upstream Pauli cases.
- After collecting each check's `num_cycles + 2` measurement positions, replace each round after the first with its difference from the previous round.
- Build grouped columns with a `BTreeMap<Vec<usize>, f64>` keyed by the augmented sparse support.
- Convert grouped columns to `rbposd::ParityCheckMatrix::from_sparse_columns(num_syndrome_rows, num_columns, decoder_columns)`.
- Clamp channel probabilities into `(0, 1)` with a small lower bound only if needed for `rbposd` probability validation; keep valid issue #110 smoke/default probabilities unchanged.

- [ ] **Step 4: Implement `run_simulation`**

In `rsinter/src/bb_circuit_memory.rs`:

- Validate config fields before building models.
- Build the code, cycle, and effective models.
- Create `rbposd::BpOsdDecoder` for Z faults from `models.z_faults.decoder` and `ChannelModel::BitFlipProbabilities(models.z_faults.channel_probs.clone())`.
- Create `rbposd::BpOsdDecoder` for X faults from `models.x_faults.decoder` and `ChannelModel::BitFlipProbabilities(models.x_faults.channel_probs.clone())`.
- Use `StdRng::seed_from_u64(seed)` when a seed is provided and `StdRng::from_entropy()` otherwise.
- Sample noisy memory operations from `num_cycles * cycle` with the upstream probabilities and 15 CNOT error order.
- For each trial, simulate Z faults through the sampled noisy circuit plus two noiseless cycles, decode the syndrome, multiply the augmented sparse columns selected by the correction to get the predicted logical vector, and compare with the actual logical vector.
- If Z succeeds, repeat for X faults.
- Count a failed trial when either logical vector differs.

- [ ] **Step 5: Run Task 2 tests**

Run:

```sh
cargo test -p rsinter --test bb_circuit_memory
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```sh
git add rsinter/src/bb_circuit_memory.rs rsinter/tests/bb_circuit_memory.rs
git commit -m "feat: simulate bb144 circuit bposd memory"
```

## Task 3: CLI Surface And Verification

**Files:**
- Modify: `rsinter/src/bin/rsinter.rs`
- Modify: `rsinter/tests/bench_cli.rs`

**Interfaces:**
- Produces: `rsinter bb-circuit-bposd-memory` CLI command.

- [ ] **Step 1: Add failing CLI smoke test**

Append to `rsinter/tests/bench_cli.rs`:

```rust
#[test]
fn rsinter_bb_circuit_bposd_memory_prints_four_column_result_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--physical-error-rate",
            "0.000000000001",
            "--num-cycles",
            "1",
            "--num-trials",
            "1",
            "--seed",
            "1",
            "--max-bp-iterations",
            "10",
            "--osd-order",
            "0",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fields: Vec<_> = stdout.trim().split('\t').collect();
    assert_eq!(fields, vec!["0.000000000001", "1", "1", "0"]);
}
```

- [ ] **Step 2: Run the CLI test and confirm it fails**

Run:

```sh
cargo test -p rsinter --test bench_cli rsinter_bb_circuit_bposd_memory_prints_four_column_result_line
```

Expected: FAIL because the subcommand does not exist.

- [ ] **Step 3: Implement CLI command**

In `rsinter/src/bin/rsinter.rs`:

- Import `rsinter::bb_circuit_memory::{run_simulation, SimulationConfig};`.
- Add a top-level `BbCircuitBposdMemory` variant to `Commands` with clap long flags:
  `physical_error_rate: f64`, `num_cycles: usize`, `num_trials: u64`,
  `seed: Option<u64>`, `max_bp_iterations: usize`, and `osd_order: usize`.
- Set clap defaults to `0.003`, `12`, `50000`, `10000`, and `7` for the non-seed fields.
- In `run()`, call `run_simulation(config)` and print:

```rust
println!(
    "{}\t{}\t{}\t{}",
    result.physical_error_rate,
    result.num_cycles,
    result.num_trials,
    result.num_failed_trials
);
```

- [ ] **Step 4: Run CLI and rsinter focused tests**

Run:

```sh
cargo test -p rsinter --test bench_cli rsinter_bb_circuit_bposd_memory_prints_four_column_result_line
cargo test -p rsinter --test bb_circuit_memory
cargo test -p rsinter
```

Expected: all commands pass.

- [ ] **Step 5: Commit Task 3**

Run:

```sh
git add rsinter/src/bin/rsinter.rs rsinter/tests/bench_cli.rs
git commit -m "feat: expose bb144 circuit bposd cli"
```

## Final Verification

- [ ] Run `cargo test -p rsinter`.
- [ ] Run the required workspace verification command: `cargo test`.
- [ ] Run `git status --short` and ensure only intentional committed changes remain.
- [ ] Use `superpowers:requesting-code-review` for a final review.
- [ ] Use `superpowers:finishing-a-development-branch`, choose "Push and create a Pull Request", and stop after PR creation.

## Self-Review

- Spec coverage: Tasks cover code/cycle construction, effective model construction, trial simulation, CLI output, smoke tests, and required verification.
- Marker scan: No open-ended markers are present.
- Type consistency: Public names used by tests are defined in task interfaces and implementation steps.
