# Issue 461 Instruction-Wide One-Qubit Noise Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply `X_ERROR` and `DEPOLARIZE1` instruction-wide for low-probability interpreted and typed compiled frame execution.

**Architecture:** Add shared `FrameSimulator` helpers for `X_ERROR` and `DEPOLARIZE1`. Each helper records debug-only per-instruction telemetry, routes `p <= 0.02` through one flattened `RareErrorIterator`, and leaves `p > 0.02` on the existing dense mask path. The compiled sampler already executes through `FrameSimulator`, so the same helpers cover interpreted and compiled paths.

**Tech Stack:** Rust 2024, `rand` 0.8, `rstim::rare_error_iterator`, Cargo integration tests, existing `SampleOptions` interpreted/compiled backend selection.

## Global Constraints

- Only wire `X_ERROR` and `DEPOLARIZE1`; do not broaden to `Y_ERROR`, `Z_ERROR`, `PAULI_CHANNEL_1`, or `DEPOLARIZE2`.
- Keep `SPARSE_BERNOULLI_MAX_PROBABILITY = 0.02`.
- For `p <= SPARSE_BERNOULLI_MAX_PROBABILITY`, build one `RareErrorIterator` per instruction.
- For `p > SPARSE_BERNOULLI_MAX_PROBABILITY`, use the existing dense fallback and do not construct a rare iterator.
- Flatten and decode exactly as `event_index = target_index * shots + shot_index`, `target_index = event_index / shots`, and `shot_index = event_index % shots`.
- `X_ERROR` toggles X; each yielded `DEPOLARIZE1` event selects X, Y, or Z uniformly.
- Per-instruction test telemetry must expose `sampling_path`, `iterator_builds`, and `attempt_count`.
- Interpreted and typed compiled paths must share this telemetry contract.
- For 100 targets, 1024 shots, and `p = 0.001`, interpreted and compiled execution each report `sampling_path = sparse`, `iterator_builds = 1`, and `attempt_count = 102400`.
- A medium-probability case at `p = 0.3` reports `sampling_path = dense` and constructs no rare iterator.
- Distribution verification must preserve `stim_x_error_two_measured_qubits` probabilities `00=.81`, `01=.09`, `10=.09`, `11=.01`.
- Distribution verification must preserve `stim_depolarize1_two_measured_qubits` probabilities `00=.64`, `01=.16`, `10=.16`, `11=.04`.
- Verification must include:
  - `cargo test -p rstim --test frame_instruction_wide_one_qubit_noise -- --nocapture`
  - `cargo test -p rstim --test frame_noise_masks -- --nocapture`
  - `cargo build --release -p rstim --bin rstim`
  - `python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml --rstim target/release/rstim --shots 100000 --seeds 7 --out /tmp/rstim-instruction-wide-one-qubit.json`
  - `cargo test`

---

## File Structure

- Modify `rstim/src/rare_error_iterator.rs`: add a crate-visible `rng_mut()` accessor so `DEPOLARIZE1` can draw its branch from the same RNG stream between sparse iterator events.
- Modify `rstim/src/sim/frame.rs`: add debug-only one-qubit noise telemetry, the hidden decode helper, instruction-wide sparse helpers, dense helper split for `DEPOLARIZE1`, and shared calls from interpreted and compiled arms.
- Create `rstim/tests/frame_instruction_wide_one_qubit_noise.rs`: focused instruction-wide decode and telemetry acceptance tests for interpreted and compiled backends.
- Modify `rstim/tests/frame_noise_masks.rs`: keep existing mask behavior checks, but point the dense `DEPOLARIZE1` source oracle at the new dense helper.

### Task 1: Add Instruction-Wide One-Qubit Noise Tests and Implementation

**Files:**
- Modify: `rstim/src/rare_error_iterator.rs`
- Modify: `rstim/src/sim/frame.rs`
- Create: `rstim/tests/frame_instruction_wide_one_qubit_noise.rs`
- Modify: `rstim/tests/frame_noise_masks.rs`

**Interfaces:**
- Consumes: `rare_error_indices(probability: f64, attempt_count: usize, rng: &mut R)`, `SampleOptions { backend, output_mode, .. }`, `SamplingBackend::{Interpreted, Compiled}`.
- Produces: hidden `decode_instruction_wide_event_index(event_index: usize, shots: usize) -> Option<(usize, usize)>`; debug-only `OneQubitNoiseSamplingPath`, `OneQubitNoiseInstructionTelemetry`, `reset_one_qubit_noise_instruction_telemetry()`, and `one_qubit_noise_instruction_telemetry()`.

- [ ] **Step 1: Write the failing focused test**

Create `rstim/tests/frame_instruction_wide_one_qubit_noise.rs`:

```rust
#[cfg(debug_assertions)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rstim::parser::parse_lines;
    use rstim::sampler::{
        SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options,
    };
    use rstim::sim::frame::{
        OneQubitNoiseSamplingPath, decode_instruction_wide_event_index,
        one_qubit_noise_instruction_telemetry, reset_one_qubit_noise_instruction_telemetry,
    };

    fn targets(count: usize) -> String {
        (0..count)
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn program(instruction: &str, probability: f64, target_count: usize) -> String {
        let targets = targets(target_count);
        format!("{instruction}({probability}) {targets}\nM {targets}\n")
    }

    fn run_and_read_telemetry(
        instruction: &str,
        probability: f64,
        target_count: usize,
        shots: usize,
        backend: SamplingBackend,
    ) -> rstim::sim::frame::OneQubitNoiseInstructionTelemetry {
        reset_one_qubit_noise_instruction_telemetry();
        let instrs = parse_lines(&program(instruction, probability, target_count)).unwrap();
        let mut rng = StdRng::seed_from_u64(7);
        let output = sample_batch_with_options(
            &instrs,
            shots,
            &mut rng,
            SampleOptions {
                backend,
                output_mode: SampleOutputMode::MeasurementsOnly,
                ..SampleOptions::default()
            },
        )
        .unwrap();
        assert_eq!(output.measurements.num_major(), target_count);
        one_qubit_noise_instruction_telemetry()
    }

    #[test]
    fn instruction_wide_index_decoder_matches_known_answers() {
        let cases = [
            (0, (0, 0)),
            (1023, (0, 1023)),
            (1024, (1, 0)),
            (102399, (99, 1023)),
        ];
        for (event_index, expected) in cases {
            assert_eq!(
                decode_instruction_wide_event_index(event_index, 1024),
                Some(expected),
                "event_index={event_index}"
            );
        }
    }

    #[test]
    fn sparse_one_qubit_noise_uses_one_instruction_wide_iterator() {
        for instruction in ["X_ERROR", "DEPOLARIZE1"] {
            for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
                let telemetry = run_and_read_telemetry(instruction, 0.001, 100, 1024, backend);
                assert_eq!(
                    telemetry.sampling_path,
                    OneQubitNoiseSamplingPath::Sparse,
                    "{instruction} {backend:?}"
                );
                assert_eq!(telemetry.iterator_builds, 1, "{instruction} {backend:?}");
                assert_eq!(telemetry.attempt_count, 102400, "{instruction} {backend:?}");
            }
        }
    }

    #[test]
    fn medium_probability_one_qubit_noise_uses_dense_path() {
        for instruction in ["X_ERROR", "DEPOLARIZE1"] {
            for backend in [SamplingBackend::Interpreted, SamplingBackend::Compiled] {
                let telemetry = run_and_read_telemetry(instruction, 0.3, 100, 1024, backend);
                assert_eq!(
                    telemetry.sampling_path,
                    OneQubitNoiseSamplingPath::Dense,
                    "{instruction} {backend:?}"
                );
                assert_eq!(telemetry.iterator_builds, 0, "{instruction} {backend:?}");
                assert_eq!(telemetry.attempt_count, 102400, "{instruction} {backend:?}");
            }
        }
        println!("PASS instruction-wide one-qubit noise telemetry");
    }
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
cargo test -p rstim --test frame_instruction_wide_one_qubit_noise -- --nocapture
```

Expected: FAIL to compile because `OneQubitNoiseSamplingPath`,
`decode_instruction_wide_event_index`, `one_qubit_noise_instruction_telemetry`,
and `reset_one_qubit_noise_instruction_telemetry` do not exist yet.

- [ ] **Step 3: Add the rare iterator RNG accessor**

In `rstim/src/rare_error_iterator.rs`, add this method inside
`impl<'a, R: RngCore + ?Sized> RareErrorIterator<'a, R>`:

```rust
    pub(crate) fn rng_mut(&mut self) -> &mut R {
        self.rng
    }
```

- [ ] **Step 4: Add frame telemetry and decode helper**

In `rstim/src/sim/frame.rs`, add imports:

```rust
use crate::rare_error_iterator::rare_error_indices;
#[cfg(debug_assertions)]
use crate::rare_error_iterator::rare_error_telemetry;
#[cfg(debug_assertions)]
use std::cell::Cell;
```

Add these debug-only types and thread-local storage near the `FrameSimulator`
definition:

```rust
#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneQubitNoiseSamplingPath {
    None,
    Sparse,
    Dense,
}

#[cfg(debug_assertions)]
impl Default for OneQubitNoiseSamplingPath {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(debug_assertions)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OneQubitNoiseInstructionTelemetry {
    pub sampling_path: OneQubitNoiseSamplingPath,
    pub iterator_builds: usize,
    pub attempt_count: usize,
}

#[cfg(debug_assertions)]
thread_local! {
    static ONE_QUBIT_NOISE_SAMPLING_PATH: Cell<OneQubitNoiseSamplingPath> =
        const { Cell::new(OneQubitNoiseSamplingPath::None) };
    static ONE_QUBIT_NOISE_ITERATOR_BUILDS: Cell<usize> = const { Cell::new(0) };
    static ONE_QUBIT_NOISE_ATTEMPT_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn reset_one_qubit_noise_instruction_telemetry() {
    ONE_QUBIT_NOISE_SAMPLING_PATH.with(|path| path.set(OneQubitNoiseSamplingPath::None));
    ONE_QUBIT_NOISE_ITERATOR_BUILDS.with(|builds| builds.set(0));
    ONE_QUBIT_NOISE_ATTEMPT_COUNT.with(|attempts| attempts.set(0));
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn one_qubit_noise_instruction_telemetry() -> OneQubitNoiseInstructionTelemetry {
    OneQubitNoiseInstructionTelemetry {
        sampling_path: ONE_QUBIT_NOISE_SAMPLING_PATH.with(Cell::get),
        iterator_builds: ONE_QUBIT_NOISE_ITERATOR_BUILDS.with(Cell::get),
        attempt_count: ONE_QUBIT_NOISE_ATTEMPT_COUNT.with(Cell::get),
    }
}

#[cfg(debug_assertions)]
fn record_one_qubit_noise_instruction(
    sampling_path: OneQubitNoiseSamplingPath,
    iterator_builds: usize,
    attempt_count: usize,
) {
    ONE_QUBIT_NOISE_SAMPLING_PATH.with(|path| path.set(sampling_path));
    ONE_QUBIT_NOISE_ITERATOR_BUILDS.with(|builds| builds.set(iterator_builds));
    ONE_QUBIT_NOISE_ATTEMPT_COUNT.with(|attempts| attempts.set(attempt_count));
}
```

Add the hidden decode helper near the noise helpers:

```rust
#[doc(hidden)]
pub fn decode_instruction_wide_event_index(
    event_index: usize,
    shots: usize,
) -> Option<(usize, usize)> {
    if shots == 0 {
        return None;
    }
    Some((event_index / shots, event_index % shots))
}
```

- [ ] **Step 5: Route interpreted and compiled `X_ERROR` through a shared helper**

Replace the interpreted `X_ERROR` arm with:

```rust
            "X_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                let qubits = qubits(targets)?;
                self.exec_x_error_qubits(&qubits, p, wpr, rng)?;
            }
```

Replace the compiled `CompiledOp::XError` arm with:

```rust
            CompiledOp::XError {
                probability,
                qubits,
            } => {
                self.exec_x_error_qubits(qubits, *probability, wpr, rng)?;
            }
```

Add the helper methods to `impl FrameSimulator` near `exec_depolarize1`:

```rust
    fn exec_x_error_qubits(
        &mut self,
        qubits: &[usize],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        let attempt_count = instruction_wide_attempt_count(qubits.len(), self.batch_size)?;
        if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
            self.exec_x_error_qubits_sparse(qubits, p, attempt_count, rng);
        } else {
            self.exec_x_error_qubits_dense(qubits, p, wpr, rng);
        }
        Ok(())
    }

    fn exec_x_error_qubits_dense(
        &mut self,
        qubits: &[usize],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) {
        let attempt_count = qubits.len().saturating_mul(self.batch_size);
        #[cfg(debug_assertions)]
        record_one_qubit_noise_instruction(OneQubitNoiseSamplingPath::Dense, 0, attempt_count);
        for &q in qubits {
            let noise = random_bits_with_prob(wpr, self.batch_size, p, rng);
            let x = self.x_table.row_words_mut(q);
            for w in 0..wpr {
                x[w] ^= noise[w];
            }
        }
    }

    fn exec_x_error_qubits_sparse(
        &mut self,
        qubits: &[usize],
        p: f64,
        attempt_count: usize,
        rng: &mut impl Rng,
    ) {
        #[cfg(debug_assertions)]
        let builds_before = rare_error_telemetry().iterator_builds;
        for event_index in rare_error_indices(p, attempt_count, rng) {
            let Some((target_index, shot_index)) =
                decode_instruction_wide_event_index(event_index, self.batch_size)
            else {
                continue;
            };
            let q = qubits[target_index];
            toggle_row_bit(self.x_table.row_words_mut(q), shot_index);
        }
        #[cfg(debug_assertions)]
        {
            let builds_after = rare_error_telemetry().iterator_builds;
            record_one_qubit_noise_instruction(
                OneQubitNoiseSamplingPath::Sparse,
                builds_after.saturating_sub(builds_before),
                attempt_count,
            );
        }
    }
```

- [ ] **Step 6: Route `DEPOLARIZE1` through sparse and dense helpers**

Change `exec_depolarize1_qubits` to return `Result<(), String>` and branch:

```rust
    fn exec_depolarize1_qubits(
        &mut self,
        qubits: &[usize],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        let attempt_count = instruction_wide_attempt_count(qubits.len(), self.batch_size)?;
        if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
            self.exec_depolarize1_qubits_sparse(qubits, p, attempt_count, rng);
        } else {
            self.exec_depolarize1_qubits_dense(qubits, p, wpr, rng);
        }
        Ok(())
    }
```

Update callers to use `?`:

```rust
self.exec_depolarize1_qubits(&qubits, p, wpr, rng)?;
self.exec_depolarize1_qubits(qubits, *probability, wpr, rng)?;
```

Move the existing body into `exec_depolarize1_qubits_dense` and add sparse
execution:

```rust
    fn exec_depolarize1_qubits_sparse(
        &mut self,
        qubits: &[usize],
        p: f64,
        attempt_count: usize,
        rng: &mut impl Rng,
    ) {
        #[cfg(debug_assertions)]
        let builds_before = rare_error_telemetry().iterator_builds;
        let mut events = rare_error_indices(p, attempt_count, rng);
        while let Some(event_index) = events.next() {
            let Some((target_index, shot_index)) =
                decode_instruction_wide_event_index(event_index, self.batch_size)
            else {
                continue;
            };
            let q = qubits[target_index];
            match events.rng_mut().gen_range(0u8..3) {
                0 => toggle_row_bit(self.x_table.row_words_mut(q), shot_index),
                1 => {
                    toggle_row_bit(self.x_table.row_words_mut(q), shot_index);
                    toggle_row_bit(self.z_table.row_words_mut(q), shot_index);
                }
                _ => toggle_row_bit(self.z_table.row_words_mut(q), shot_index),
            }
        }
        #[cfg(debug_assertions)]
        {
            let builds_after = rare_error_telemetry().iterator_builds;
            record_one_qubit_noise_instruction(
                OneQubitNoiseSamplingPath::Sparse,
                builds_after.saturating_sub(builds_before),
                attempt_count,
            );
        }
    }
```

Add small free helpers near the noise helpers:

```rust
fn instruction_wide_attempt_count(
    target_count: usize,
    shots: usize,
) -> Result<usize, String> {
    target_count
        .checked_mul(shots)
        .ok_or_else(|| "instruction-wide noise attempt count overflowed usize".to_string())
}

fn toggle_row_bit(row: &mut [u64], shot_index: usize) {
    let word = shot_index / 64;
    let bit = shot_index % 64;
    row[word] ^= 1u64 << bit;
}
```

- [ ] **Step 7: Update the dense-mask source oracle**

In `rstim/tests/frame_noise_masks.rs`, update the `DEPOLARIZE1` source test:

```rust
#[test]
fn depolarize1_event_mask_uses_integer_threshold_path() {
    let helper = match_arm(
        frame_source(),
        "fn exec_depolarize1_qubits_dense",
        "fn exec_depolarize1_qubits_sparse",
    );
    assert!(helper.contains("random_bits_with_prob_into"), "{helper}");
    assert!(!helper.contains("gen::<f64>() < p"), "{helper}");
    assert!(!helper.contains("r#gen::<f64>() < p"), "{helper}");
}
```

- [ ] **Step 8: Run focused tests and fix compile errors**

Run:

```sh
cargo test -p rstim --test frame_instruction_wide_one_qubit_noise -- --nocapture
cargo test -p rstim --test frame_noise_masks -- --nocapture
```

Expected: both pass, and the first command prints
`PASS instruction-wide one-qubit noise telemetry`.

- [ ] **Step 9: Format and run required verification**

Run:

```sh
cargo fmt
cargo test -p rstim --test frame_instruction_wide_one_qubit_noise -- --nocapture
cargo test -p rstim --test frame_noise_masks -- --nocapture
cargo build --release -p rstim --bin rstim
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --rstim target/release/rstim --shots 100000 --seeds 7 \
  --out /tmp/rstim-instruction-wide-one-qubit.json
cargo test
```

Expected: tests and build pass. The Python command prints
`PASS distribution correctness cases=8 mismatch=0`.

- [ ] **Step 10: Commit**

Commit the implementation:

```sh
git add rstim/src/rare_error_iterator.rs rstim/src/sim/frame.rs \
  rstim/tests/frame_instruction_wide_one_qubit_noise.rs \
  rstim/tests/frame_noise_masks.rs
git commit -m "feat: apply one-qubit noise instruction-wide"
```

## Plan Self-Review

- Spec coverage: sparse path selection, dense fallback, exact flatten/decode,
  branch selection, telemetry, interpreted/compiled sharing, known answers,
  and distribution checks all map to task steps.
- Placeholder scan: no unresolved placeholder markers or vague test steps.
- Type consistency: helper and telemetry names are stable across test and
  implementation steps.
