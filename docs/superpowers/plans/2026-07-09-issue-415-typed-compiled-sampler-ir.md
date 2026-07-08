# Issue 415 Typed Compiled Sampler IR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lower supported compiled sampler instructions into typed operation variants and execute those variants without per-operation string dispatch or repeated target parsing.

**Architecture:** `CompiledOp` becomes a typed enum with predecoded qubit lists, qubit pairs, probabilities, measurement bases, observable indices, and `rec[-k]` lookbacks. `choose_sampler_path` rejects loss, feedback, and any `UnsupportedSamplerOp` marker before the compiled sampler can execute. `FrameSimulator::run_compiled_blocks` dispatches on typed variants directly while preserving the existing interpreted fallback and source-based analyzer behavior.

**Tech Stack:** Rust 2024, existing `rstim` compiled sampler modules, existing `FrameSimulator`, deterministic `StdRng` integration tests.

## Global Constraints

- Do not leave opaque string-dispatch operations on the compiled sampler fast path.
- Unsupported sampler operations must force `choose_sampler_path` fallback instead of executing through a generic operation escape hatch.
- Loss circuits and feedback circuits must continue to choose fallback.
- Keep the first typed IR scoped to the selected d11/r100 fixture and existing gating/smoke cases.
- Preserve compiled vs interpreted sample behavior for supported circuits.
- Do not remove the interpreted fallback.
- Do not claim broad Stim instruction coverage or performance parity.
- Verification command required by issue #415: `cargo test -p rstim --test compiled_sampler_ir`.
- Broader worker verification command required by Agent Desk: `cargo test`.

---

## File Structure

- Create `rstim/tests/compiled_sampler_ir.rs`: issue-level integration tests for typed lowering, supported behavior preservation, fallback gating, and unsupported-op negative control.
- Modify `rstim/src/compiled/circuit.rs`: replace the generic string `CompiledOp` struct with typed enum variants and lowering helpers.
- Modify `rstim/src/compiled/path.rs`: recursively reject unsupported sampler markers.
- Modify `rstim/src/compiled/mod.rs`: re-export new typed helper enums.
- Modify `rstim/src/sim/frame.rs`: execute typed compiled operations directly.

### Task 1: Add Failing Typed Sampler IR Tests

**Files:**
- Create: `rstim/tests/compiled_sampler_ir.rs`

**Interfaces:**
- Consumes: current public `rstim::compiled::{compile_circuit, choose_sampler_path, CompiledBlock, CompiledOp, CompiledPathDecision}`.
- Produces: four integration tests named `selected_surface_fixture_lowers_to_typed_sampler_ops`, `compiled_sampler_ir_preserves_sample_bits_on_smoke_fixture`, `loss_and_feedback_circuits_still_choose_fallback`, and `unsupported_sampler_ops_do_not_enter_typed_fast_path`.

- [ ] **Step 1: Write the failing test file**

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::compiled::{
    choose_sampler_path, compile_circuit, CompiledBlock, CompiledOp, CompiledPathDecision,
};
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch_with_options, SampleOptions, SamplingBackend};
use rstim::sim::bit_table::BitTable;

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);

#[derive(Default)]
struct VariantCounts {
    cx: usize,
    depolarize1: usize,
    depolarize2: usize,
    measure: usize,
    measure_reset: usize,
    detector: usize,
    observable: usize,
    unsupported: usize,
}

fn count_variants(blocks: &[CompiledBlock], counts: &mut VariantCounts) {
    for block in blocks {
        match block {
            CompiledBlock::Ops(ops) => {
                for op in ops {
                    match op {
                        CompiledOp::Cx { .. } => counts.cx += 1,
                        CompiledOp::Depolarize1 { .. } => counts.depolarize1 += 1,
                        CompiledOp::Depolarize2 { .. } => counts.depolarize2 += 1,
                        CompiledOp::Measure { .. } => counts.measure += 1,
                        CompiledOp::MeasureReset { .. } => counts.measure_reset += 1,
                        CompiledOp::Detector { .. } => counts.detector += 1,
                        CompiledOp::ObservableInclude { .. } => counts.observable += 1,
                        CompiledOp::UnsupportedSamplerOp { .. } => counts.unsupported += 1,
                        _ => {}
                    }
                }
            }
            CompiledBlock::Repeat(region) => count_variants(&region.body, counts),
        }
    }
}

fn bit_table_rows(table: &BitTable) -> Vec<Vec<bool>> {
    (0..table.num_major())
        .map(|major| {
            (0..table.num_minor())
                .map(|minor| table.get(major, minor))
                .collect()
        })
        .collect()
}

#[test]
fn selected_surface_fixture_lowers_to_typed_sampler_ops() {
    let instrs = parse_lines(SURFACE_D11_R100).expect("parse selected fixture");
    let compiled = compile_circuit(&instrs).expect("compile selected fixture");

    assert_eq!(choose_sampler_path(&compiled), CompiledPathDecision::FastPath);

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);

    assert!(counts.depolarize1 > 0, "fixture should lower DEPOLARIZE1");
    assert!(counts.depolarize2 > 0, "fixture should lower DEPOLARIZE2");
    assert!(counts.cx > 0, "fixture should lower CX");
    assert!(counts.measure_reset > 0, "fixture should lower MR");
    assert!(counts.measure > 0, "fixture should lower M");
    assert!(counts.detector > 0, "fixture should lower DETECTOR");
    assert!(counts.observable > 0, "fixture should lower OBSERVABLE_INCLUDE");
    assert_eq!(counts.unsupported, 0, "selected fixture must not contain fallback markers");
}

#[test]
fn compiled_sampler_ir_preserves_sample_bits_on_smoke_fixture() {
    let instrs = parse_lines(
        "R 0 1\n\
         X_ERROR(0.125) 0\n\
         H 0\n\
         CX 0 1\n\
         DEPOLARIZE1(0.125) 0\n\
         DEPOLARIZE2(0.125) 0 1\n\
         MR 1\n\
         X_ERROR(0.125) 0\n\
         M 0\n\
         DETECTOR rec[-1] rec[-2]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("parse smoke circuit");

    let compiled = compile_circuit(&instrs).expect("compile smoke circuit");
    assert_eq!(choose_sampler_path(&compiled), CompiledPathDecision::FastPath);

    let mut interpreted_rng = StdRng::seed_from_u64(20260709);
    let mut compiled_rng = StdRng::seed_from_u64(20260709);

    let interpreted = sample_batch_with_options(
        &instrs,
        32,
        &mut interpreted_rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            ..SampleOptions::default()
        },
    )
    .expect("interpreted sample");
    let compiled = sample_batch_with_options(
        &instrs,
        32,
        &mut compiled_rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .expect("compiled sample");

    assert_eq!(
        bit_table_rows(&compiled.measurements),
        bit_table_rows(&interpreted.measurements)
    );
    assert_eq!(
        bit_table_rows(&compiled.detections),
        bit_table_rows(&interpreted.detections)
    );
    assert_eq!(
        bit_table_rows(&compiled.observable_flips),
        bit_table_rows(&interpreted.observable_flips)
    );
}

#[test]
fn loss_and_feedback_circuits_still_choose_fallback() {
    let loss = compile_circuit(&parse_lines("LOSS(1) 0\nMRL 0\n").unwrap()).unwrap();
    let feedback = compile_circuit(&parse_lines("M 0\nCX rec[-1] 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&loss),
        CompiledPathDecision::Fallback("loss instructions require the interpreted path")
    );
    assert_eq!(
        choose_sampler_path(&feedback),
        CompiledPathDecision::Fallback("feedback instructions require the interpreted path")
    );
}

#[test]
fn unsupported_sampler_ops_do_not_enter_typed_fast_path() {
    let compiled = compile_circuit(&parse_lines("S 0\nM 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        CompiledPathDecision::Fallback(
            "unsupported sampler instructions require the interpreted path",
        )
    );

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);
    assert_eq!(counts.unsupported, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test compiled_sampler_ir`

Expected: FAIL at compile time because typed `CompiledOp` enum variants such as `Cx`, `Depolarize2`, `MeasureReset`, `Detector`, `ObservableInclude`, and `UnsupportedSamplerOp` do not exist yet.

- [ ] **Step 3: Commit the failing test**

```bash
git add rstim/tests/compiled_sampler_ir.rs
git commit -m "test: cover typed compiled sampler ir"
```

### Task 2: Lower Supported Instructions Into Typed CompiledOp Variants

**Files:**
- Modify: `rstim/src/compiled/circuit.rs`
- Modify: `rstim/src/compiled/path.rs`
- Modify: `rstim/src/compiled/mod.rs`

**Interfaces:**
- Consumes: Task 1 tests and existing `compile_circuit` callers.
- Produces: typed `CompiledOp` enum, `CompiledBasis` enum, predecoded targets/probabilities/record offsets, and sampler path fallback for unsupported markers.

- [ ] **Step 1: Replace the generic operation struct with typed enums**

In `rstim/src/compiled/circuit.rs`, replace `CompiledOp { name, args, targets }` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompiledBasis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompiledOp {
    Tick,
    QubitCoords,
    ShiftCoords,
    H { qubits: Vec<usize> },
    Reset { basis: CompiledBasis, qubits: Vec<usize> },
    XError { probability: f64, qubits: Vec<usize> },
    Depolarize1 { probability: f64, qubits: Vec<usize> },
    Cx { pairs: Vec<(usize, usize)> },
    Depolarize2 { probability: f64, pairs: Vec<(usize, usize)> },
    Measure { basis: CompiledBasis, qubits: Vec<usize> },
    MeasureReset { basis: CompiledBasis, qubits: Vec<usize> },
    Detector { rec_offsets: Vec<usize> },
    ObservableInclude { observable_index: usize, rec_offsets: Vec<usize> },
    UnsupportedSamplerOp { name: String },
}
```

- [ ] **Step 2: Lower `StimInstr::Op` through a typed helper**

Change the `pending_ops.push` block to call `compile_sampler_op(name, args, targets)` and push the returned `CompiledOp`.

- [ ] **Step 3: Add lowering helpers**

Add helpers in `rstim/src/compiled/circuit.rs`:

```rust
fn compile_sampler_op(name: &str, args: &[f64], targets: &[StimTarget]) -> CompiledOp {
    match name {
        "TICK" => CompiledOp::Tick,
        "QUBIT_COORDS" => CompiledOp::QubitCoords,
        "SHIFT_COORDS" => CompiledOp::ShiftCoords,
        "H" => qubits(targets)
            .map(|qubits| CompiledOp::H { qubits })
            .unwrap_or_else(|| unsupported(name)),
        "R" | "RZ" => qubits(targets)
            .map(|qubits| CompiledOp::Reset { basis: CompiledBasis::Z, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "RX" => qubits(targets)
            .map(|qubits| CompiledOp::Reset { basis: CompiledBasis::X, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "RY" => qubits(targets)
            .map(|qubits| CompiledOp::Reset { basis: CompiledBasis::Y, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "X_ERROR" => qubits(targets)
            .map(|qubits| CompiledOp::XError { probability: first_arg(args), qubits })
            .unwrap_or_else(|| unsupported(name)),
        "DEPOLARIZE1" => qubits(targets)
            .map(|qubits| CompiledOp::Depolarize1 { probability: first_arg(args), qubits })
            .unwrap_or_else(|| unsupported(name)),
        "CX" | "CNOT" | "ZCX" => qubit_pairs(targets)
            .map(|pairs| CompiledOp::Cx { pairs })
            .unwrap_or_else(|| unsupported(name)),
        "DEPOLARIZE2" => qubit_pairs(targets)
            .map(|pairs| CompiledOp::Depolarize2 { probability: first_arg(args), pairs })
            .unwrap_or_else(|| unsupported(name)),
        "M" | "MZ" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::Measure { basis: CompiledBasis::Z, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "MX" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::Measure { basis: CompiledBasis::X, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "MY" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::Measure { basis: CompiledBasis::Y, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "MR" | "MRZ" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::MeasureReset { basis: CompiledBasis::Z, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "MRX" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::MeasureReset { basis: CompiledBasis::X, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "MRY" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::MeasureReset { basis: CompiledBasis::Y, qubits })
            .unwrap_or_else(|| unsupported(name)),
        "DETECTOR" => rec_offsets(targets)
            .map(|rec_offsets| CompiledOp::Detector { rec_offsets })
            .unwrap_or_else(|| unsupported(name)),
        "OBSERVABLE_INCLUDE" => rec_offsets(targets)
            .map(|rec_offsets| CompiledOp::ObservableInclude {
                observable_index: first_arg(args) as usize,
                rec_offsets,
            })
            .unwrap_or_else(|| unsupported(name)),
        _ => unsupported(name),
    }
}
```

The helper functions must mirror existing executor target behavior:

```rust
fn unsupported(name: &str) -> CompiledOp {
    CompiledOp::UnsupportedSamplerOp { name: name.to_string() }
}

fn first_arg(args: &[f64]) -> f64 {
    args.first().copied().unwrap_or(0.0)
}

fn qubits(targets: &[StimTarget]) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Qubit(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {}
            _ => return None,
        }
    }
    Some(out)
}

fn qubits_ignoring_inv(targets: &[StimTarget]) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {}
            _ => return None,
        }
    }
    Some(out)
}

fn qubit_pairs(targets: &[StimTarget]) -> Option<Vec<(usize, usize)>> {
    if targets.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut iter = targets.iter();
    while let (Some(a), Some(b)) = (iter.next(), iter.next()) {
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        let StimTarget::Qubit(qa) = a else { return None };
        let StimTarget::Qubit(qb) = b else { return None };
        out.push((*qa as usize, *qb as usize));
    }
    Some(out)
}

fn rec_offsets(targets: &[StimTarget]) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Rec(offset) if *offset < 0 => out.push((-*offset) as usize),
            _ => return None,
        }
    }
    Some(out)
}
```

- [ ] **Step 4: Reject unsupported markers in `choose_sampler_path`**

In `rstim/src/compiled/path.rs`, import `CompiledOp` and add recursive helpers:

```rust
fn contains_unsupported_sampler_op(blocks: &[CompiledBlock]) -> bool {
    blocks.iter().any(|block| match block {
        CompiledBlock::Ops(ops) => ops
            .iter()
            .any(|op| matches!(op, CompiledOp::UnsupportedSamplerOp { .. })),
        CompiledBlock::Repeat(region) => contains_unsupported_sampler_op(&region.body),
    })
}
```

Then add this check after loss and feedback:

```rust
if contains_unsupported_sampler_op(&compiled.blocks) {
    return CompiledPathDecision::Fallback(
        "unsupported sampler instructions require the interpreted path",
    );
}
```

- [ ] **Step 5: Re-export typed helpers**

In `rstim/src/compiled/mod.rs`, export `CompiledBasis` alongside `CompiledOp`.

- [ ] **Step 6: Run the focused test**

Run: `cargo test -p rstim --test compiled_sampler_ir`

Expected: still FAIL because `FrameSimulator::run_compiled_blocks` still tries to read `op.name`, `op.args`, and `op.targets` from the old struct.

- [ ] **Step 7: Commit typed lowering and gating**

```bash
git add rstim/src/compiled/circuit.rs rstim/src/compiled/path.rs rstim/src/compiled/mod.rs
git commit -m "feat: lower compiled sampler ops into typed ir"
```

### Task 3: Execute Typed Compiled Operations

**Files:**
- Modify: `rstim/src/sim/frame.rs`

**Interfaces:**
- Consumes: `CompiledOp` typed variants from Task 2.
- Produces: `FrameSimulator::run_compiled_blocks` with no `exec_op(op.name.as_str(), ...)` path for compiled sampler execution.

- [ ] **Step 1: Import typed compiled operation enums**

Change the import to:

```rust
use crate::compiled::{CompiledBasis, CompiledBlock, CompiledOp};
```

- [ ] **Step 2: Route compiled ops through a typed executor**

Change the `CompiledBlock::Ops` arm in `run_compiled_blocks` to:

```rust
CompiledBlock::Ops(ops) => {
    for op in ops {
        self.exec_compiled_op(op, ref_sample, rng)?;
    }
}
```

- [ ] **Step 3: Add `exec_compiled_op`**

Add a private method inside `impl FrameSimulator` that matches each typed variant:

```rust
fn exec_compiled_op(
    &mut self,
    op: &CompiledOp,
    ref_sample: &[bool],
    rng: &mut impl Rng,
) -> Result<(), String> {
    let wpr = self.x_table.words_per_row();
    match op {
        CompiledOp::Tick | CompiledOp::QubitCoords | CompiledOp::ShiftCoords => {}
        CompiledOp::H { qubits } => {
            for &q in qubits {
                do_h(&mut self.x_table, &mut self.z_table, q);
            }
        }
        CompiledOp::Reset { basis, qubits } => self.exec_compiled_reset(*basis, qubits, rng),
        CompiledOp::XError { probability, qubits } => {
            for &q in qubits {
                let noise = random_bits_with_prob(wpr, self.batch_size, *probability, rng);
                let x = self.x_table.row_words_mut(q);
                for w in 0..wpr {
                    x[w] ^= noise[w];
                }
            }
        }
        CompiledOp::Depolarize1 { probability, qubits } => {
            self.exec_depolarize1_qubits(qubits, *probability, wpr, rng);
        }
        CompiledOp::Cx { pairs } => {
            for &(control, target) in pairs {
                do_cx(&mut self.x_table, &mut self.z_table, control, target);
            }
        }
        CompiledOp::Depolarize2 { probability, pairs } => {
            self.exec_depolarize2_pairs(pairs, *probability, wpr, rng);
        }
        CompiledOp::Measure { basis, qubits } => {
            self.exec_compiled_measure(*basis, qubits, wpr, rng);
        }
        CompiledOp::MeasureReset { basis, qubits } => {
            self.exec_compiled_measure_reset(*basis, qubits, wpr, rng);
        }
        CompiledOp::Detector { rec_offsets } => {
            self.exec_compiled_detector(rec_offsets, ref_sample);
        }
        CompiledOp::ObservableInclude {
            observable_index,
            rec_offsets,
        } => self.exec_compiled_observable_include(*observable_index, rec_offsets, ref_sample),
        CompiledOp::UnsupportedSamplerOp { name } => {
            return Err(format!("compiled sampler: unsupported instruction {name}"));
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Add typed measurement/reset helpers**

Add helpers that preserve the existing `M`, `MX`, `MY`, `MR`, `MRX`, and `MRY` behavior:

```rust
fn exec_compiled_measure(
    &mut self,
    basis: CompiledBasis,
    qubits: &[usize],
    wpr: usize,
    rng: &mut impl Rng,
) {
    for &q in qubits {
        match basis {
            CompiledBasis::Z => {
                self.m_record.push_row(self.x_table.row_words(q));
                self.x_table.clear_row(q);
                self.z_table.randomize_row(q, rng);
            }
            CompiledBasis::X => {
                self.m_record.push_row(self.z_table.row_words(q));
                self.z_table.clear_row(q);
                self.x_table.randomize_row(q, rng);
            }
            CompiledBasis::Y => {
                let mut tmp = vec![0u64; wpr];
                for (w, word) in tmp.iter_mut().enumerate() {
                    *word = self.x_table.row_words(q)[w] ^ self.z_table.row_words(q)[w];
                }
                self.m_record.push_row(&tmp);
                self.x_table.clear_row(q);
                self.z_table.clear_row(q);
                self.x_table.randomize_row(q, rng);
                self.z_table.randomize_row(q, rng);
            }
        }
    }
}
```

`exec_compiled_measure_reset` is the same except the Z-basis branch clears both
tables and randomizes Z, matching `MR`; the X-basis branch clears both tables
and randomizes X, matching `MRX`; the Y-basis branch matches `MRY`.

`exec_compiled_reset` clears/randomizes the same rows as `R`, `RX`, and `RY`
without pushing a measurement row.

Use these exact helper bodies:

```rust
fn exec_compiled_measure_reset(
    &mut self,
    basis: CompiledBasis,
    qubits: &[usize],
    wpr: usize,
    rng: &mut impl Rng,
) {
    for &q in qubits {
        match basis {
            CompiledBasis::Z => {
                self.m_record.push_row(self.x_table.row_words(q));
                self.x_table.clear_row(q);
                self.z_table.clear_row(q);
                self.z_table.randomize_row(q, rng);
            }
            CompiledBasis::X => {
                self.m_record.push_row(self.z_table.row_words(q));
                self.x_table.clear_row(q);
                self.z_table.clear_row(q);
                self.x_table.randomize_row(q, rng);
            }
            CompiledBasis::Y => {
                let mut tmp = vec![0u64; wpr];
                for (w, word) in tmp.iter_mut().enumerate() {
                    *word = self.x_table.row_words(q)[w] ^ self.z_table.row_words(q)[w];
                }
                self.m_record.push_row(&tmp);
                self.x_table.clear_row(q);
                self.z_table.clear_row(q);
                self.x_table.randomize_row(q, rng);
                self.z_table.randomize_row(q, rng);
            }
        }
    }
}

fn exec_compiled_reset(
    &mut self,
    basis: CompiledBasis,
    qubits: &[usize],
    rng: &mut impl Rng,
) {
    for &q in qubits {
        match basis {
            CompiledBasis::Z => {
                self.x_table.clear_row(q);
                self.z_table.randomize_row(q, rng);
            }
            CompiledBasis::X => {
                self.z_table.clear_row(q);
                self.x_table.randomize_row(q, rng);
            }
            CompiledBasis::Y => {
                self.x_table.clear_row(q);
                self.z_table.clear_row(q);
                self.x_table.randomize_row(q, rng);
                self.z_table.randomize_row(q, rng);
            }
        }
    }
}
```

- [ ] **Step 5: Add typed depolarizing helpers**

Keep existing `exec_depolarize1` and `exec_depolarize2` for string execution.
Add typed pair/list variants that reuse the same scratch and inner logic:

```rust
fn exec_depolarize1_qubits(
    &mut self,
    qubits: &[usize],
    p: f64,
    wpr: usize,
    rng: &mut impl Rng,
) {
    if p <= 0.0 {
        return;
    }
    for &q in qubits {
        {
            let scratch = &mut self.depolarize_scratch;
            scratch.prepare_one(wpr);
            random_bits_with_prob_into(&mut scratch.events, self.batch_size, p, rng);
            for w in 0..wpr {
                let mut bits = scratch.events[w];
                while bits != 0 {
                    let bit = bits.trailing_zeros();
                    match rng.gen_range(0u8..3) {
                        0 => scratch.x_a[w] |= 1u64 << bit,
                        1 => {
                            scratch.x_a[w] |= 1u64 << bit;
                            scratch.z_a[w] |= 1u64 << bit;
                        }
                        _ => scratch.z_a[w] |= 1u64 << bit,
                    }
                    bits &= bits - 1;
                }
            }
        }
        let scratch = &self.depolarize_scratch;
        let x = self.x_table.row_words_mut(q);
        for w in 0..wpr {
            x[w] ^= scratch.x_a[w];
        }
        let z = self.z_table.row_words_mut(q);
        for w in 0..wpr {
            z[w] ^= scratch.z_a[w];
        }
    }
}

fn exec_depolarize2_pairs(
    &mut self,
    pairs: &[(usize, usize)],
    p: f64,
    wpr: usize,
    rng: &mut impl Rng,
) {
    if p <= 0.0 {
        return;
    }
    for &(qa, qb) in pairs {
        {
            let scratch = &mut self.depolarize_scratch;
            scratch.prepare_two(wpr);
            random_bits_with_prob_into(&mut scratch.events, self.batch_size, p, rng);
            for w in 0..wpr {
                let mut bits = scratch.events[w];
                while bits != 0 {
                    let bit = bits.trailing_zeros();
                    let r = rng.gen_range(0u8..15);
                    let (pa, pb) = two_qubit_pauli(r);
                    apply_pauli_bits(pa, &mut scratch.x_a, &mut scratch.z_a, w, bit);
                    apply_pauli_bits(pb, &mut scratch.x_b, &mut scratch.z_b, w, bit);
                    bits &= bits - 1;
                }
            }
        }
        let scratch = &self.depolarize_scratch;
        let x = self.x_table.row_words_mut(qa);
        for w in 0..wpr {
            x[w] ^= scratch.x_a[w];
        }
        let z = self.z_table.row_words_mut(qa);
        for w in 0..wpr {
            z[w] ^= scratch.z_a[w];
        }
        let x = self.x_table.row_words_mut(qb);
        for w in 0..wpr {
            x[w] ^= scratch.x_b[w];
        }
        let z = self.z_table.row_words_mut(qb);
        for w in 0..wpr {
            z[w] ^= scratch.z_b[w];
        }
    }
}
```

Then make the existing string helpers call `qubits(targets)?` or
`qubit_pairs(targets)?` and delegate to these typed helpers. The random event
selection and Pauli choice loops must remain in the same target order as the
old helpers.

- [ ] **Step 6: Add typed detector/observable helpers**

Add `exec_compiled_detector` and `exec_compiled_observable_include` that take
predecoded positive lookbacks and preserve the existing reference-sample parity
logic:

```rust
fn exec_compiled_detector(&mut self, rec_offsets: &[usize], ref_sample: &[bool]) {
    if !self.materialize_detector_observable_outputs {
        return;
    }
    self.detector_materializations += 1;
    let wpr = self.m_record.words_per_row();
    let mut result = vec![0u64; wpr];
    let mut ref_parity = false;
    for &k in rec_offsets {
        self.m_record.xor_lookback_into(k, &mut result);
        let m_idx = self.m_record.len() - k;
        if m_idx < ref_sample.len() && ref_sample[m_idx] {
            ref_parity = !ref_parity;
        }
    }
    if ref_parity {
        for word in &mut result {
            *word ^= !0u64;
        }
    }
    self.det_records.push(result);
}
```

Add this observable helper:

```rust
fn exec_compiled_observable_include(
    &mut self,
    observable_index: usize,
    rec_offsets: &[usize],
    ref_sample: &[bool],
) {
    if !self.materialize_detector_observable_outputs {
        return;
    }
    self.observable_materializations += 1;
    let wpr = self.m_record.words_per_row();
    while self.obs_records.len() <= observable_index {
        self.obs_records.push(vec![0u64; wpr]);
    }
    let mut ref_parity = false;
    for &k in rec_offsets {
        self.m_record
            .xor_lookback_into(k, &mut self.obs_records[observable_index]);
        let m_idx = self.m_record.len() - k;
        if m_idx < ref_sample.len() && ref_sample[m_idx] {
            ref_parity = !ref_parity;
        }
    }
    if ref_parity {
        for word in &mut self.obs_records[observable_index] {
            *word ^= !0u64;
        }
    }
}
```

- [ ] **Step 7: Run the focused test**

Run: `cargo test -p rstim --test compiled_sampler_ir`

Expected: PASS, with all four issue-required tests passing.

- [ ] **Step 8: Run existing compiled sampler/routing tests**

Run: `cargo test -p rstim --test compiled_sampler`

Expected: PASS.

Run: `cargo test -p rstim --test compiled_routing`

Expected: PASS.

- [ ] **Step 9: Commit typed execution**

```bash
git add rstim/src/sim/frame.rs
git commit -m "feat: execute typed compiled sampler ops"
```

### Task 4: Final Verification And Review

**Files:**
- Verify repository state only; no planned source edits unless verification or review finds defects.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified worker branch ready for PR.

- [ ] **Step 1: Run issue verification**

Run: `cargo test -p rstim --test compiled_sampler_ir`

Expected: PASS.

- [ ] **Step 2: Run full worker verification**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 3: Inspect diff for generic compiled sampler dispatch**

Run: `rg "exec_op\\(op\\.name|name: String|UnsupportedSamplerOp|unsupported sampler instructions" rstim/src/compiled rstim/src/sim/frame.rs rstim/tests/compiled_sampler_ir.rs`

Expected: no `exec_op(op.name...)` match and no `CompiledOp` struct with `name: String`; `UnsupportedSamplerOp` appears only as fallback marker/test coverage.

- [ ] **Step 4: Commit any verification fixes**

If Step 1, Step 2, Step 3, or code review finds a defect, fix it with a focused
test-first change, run the covering command, and commit with:

```bash
git add <changed files>
git commit -m "fix: preserve typed compiled sampler behavior"
```

If no fixes are needed, do not create an empty commit.
