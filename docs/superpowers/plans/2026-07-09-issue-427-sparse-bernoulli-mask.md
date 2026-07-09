# Issue 427 Sparse Bernoulli Frame Noise Mask Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a sparse low-probability Bernoulli path to frame noise mask generation while keeping the existing dense integer-threshold path for medium and high probabilities.

**Architecture:** Keep the change private to `rstim/src/sim/frame.rs`. `random_bits_with_prob_into` will preserve exact `p <= 0`, `p >= 1`, and zero-threshold behavior, then dispatch to a sparse geometric-skip helper for `p <= 0.02` and the current dense integer-threshold helper for larger probabilities.

**Tech Stack:** Rust 2024, `rand` 0.8, Cargo integration tests, existing `FrameSimulator`, parser, and reference-sample APIs.

## Global Constraints

- Do not change public CLI formats or checked benchmark artifacts.
- Do not overwrite the checked #406 artifact.
- Do not publish new checked speed artifacts.
- Do not set a CI wall-clock regression gate based on cross-machine timing.
- Preserve Stim compatibility and public output formats.
- Preserve exact fast paths for `p <= 0.0` and `p >= 1.0`.
- Keep medium and high probabilities on the dense integer-threshold path.
- Low-probability sparse masks must mask unused tail bits.
- Seeded RNG output must be reproducible for the same implementation and inputs.
- Focused verification must include `cargo test -p rstim --test frame_noise_masks`.
- Final verification must include `cargo test`.
- Release smoke verification records positive rates and ratio only; it must not enforce a wall-clock speed threshold.

---

## File Structure

- Modify `rstim/tests/frame_noise_masks.rs`: add runtime RNG-call path-selection tests with a counting RNG while keeping existing mask correctness tests.
- Modify `rstim/src/sim/frame.rs`: split dense mask filling into a helper and add sparse geometric-skip filling for low probabilities.

### Task 1: Add Sparse Path Selection Tests

**Files:**
- Modify: `rstim/tests/frame_noise_masks.rs`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines`, `rstim::executor::reference_sample`, `rstim::sim::frame::FrameSimulator`, `rand::RngCore`
- Produces: integration tests named `low_probability_noise_mask_uses_sparse_path` and `medium_probability_noise_mask_keeps_dense_path`

- [ ] **Step 1: Extend imports and add counting RNG**

Add `RngCore` to the rand imports and add the counting RNG helper below `measurement_words`.

```rust
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sim::frame::FrameSimulator;
```

```rust
struct CountingRng {
    inner: StdRng,
    core_calls: usize,
}

impl CountingRng {
    fn seed_from_u64(seed: u64) -> Self {
        Self {
            inner: StdRng::seed_from_u64(seed),
            core_calls: 0,
        }
    }

    fn core_calls(&self) -> usize {
        self.core_calls
    }
}

impl RngCore for CountingRng {
    fn next_u32(&mut self) -> u32 {
        self.core_calls += 1;
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.core_calls += 1;
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_ne_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

fn measurement_words_with_counting_rng(
    program: &str,
    batch_size: usize,
    seed: u64,
) -> (Vec<u64>, usize) {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = CountingRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(1, batch_size);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    (
        frame.measurements(&ref_sample).row_words(0).to_vec(),
        rng.core_calls(),
    )
}
```

- [ ] **Step 2: Add sparse and dense path-selection tests**

Append these tests after `noise_mask_is_reproducible_for_seeded_rng`.

```rust
#[test]
fn low_probability_noise_mask_uses_sparse_path() {
    let batch_size = 65_536;
    let (words, core_calls) =
        measurement_words_with_counting_rng("X_ERROR(0.001) 0\nM 0\n", batch_size, 123);
    let hits = count_ones(&words);
    assert!(
        (30..=110).contains(&hits),
        "expected roughly 66 hits for p=0.001 over {batch_size} shots, got {hits}"
    );
    assert!(
        core_calls < 8_192,
        "low-probability mask should jump between events instead of drawing once per bit; saw {core_calls} RNG core calls for {batch_size} shots"
    );
}

#[test]
fn medium_probability_noise_mask_keeps_dense_path() {
    let batch_size = 4_096;
    let (words, core_calls) =
        measurement_words_with_counting_rng("X_ERROR(0.3) 0\nM 0\n", batch_size, 321);
    let hits = count_ones(&words);
    assert!(
        (1_100..=1_350).contains(&hits),
        "expected roughly 1229 hits for p=0.3 over {batch_size} shots, got {hits}"
    );
    assert!(
        core_calls >= batch_size,
        "medium-probability mask should stay on the dense path; saw {core_calls} RNG core calls for {batch_size} shots"
    );
}
```

- [ ] **Step 3: Run test to verify RED**

Run:

```sh
cargo test -p rstim --test frame_noise_masks
```

Expected: FAIL. `low_probability_noise_mask_uses_sparse_path` fails because the current integer-threshold implementation draws one `u64` per valid shot bit for `p = 0.001`, so `core_calls` is at least `65_536`.

- [ ] **Step 4: Commit the failing tests**

```bash
git add rstim/tests/frame_noise_masks.rs
git commit -m "test: cover sparse frame noise masks"
```

### Task 2: Implement Sparse Bernoulli Mask Filling

**Files:**
- Modify: `rstim/src/sim/frame.rs`

**Interfaces:**
- Consumes: `random_bits_with_prob_into(result: &mut [u64], valid_bits: usize, p: f64, rng: &mut impl Rng)`, `probability_threshold_u64`, `mask_unused_bits`
- Produces: private constants/helpers `SPARSE_BERNOULLI_MAX_PROBABILITY`, `random_dense_bits_with_threshold_into`, and `random_sparse_bits_with_prob_into`

- [ ] **Step 1: Add the sparse threshold constant**

Add the constant near the noise helper section, before `random_bits_with_prob`.

```rust
const SPARSE_BERNOULLI_MAX_PROBABILITY: f64 = 0.02;
```

- [ ] **Step 2: Dispatch helper filling by probability**

Replace the loop at the end of `random_bits_with_prob_into` with the path split below.

```rust
    if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
        random_sparse_bits_with_prob_into(result, valid_bits, p, rng);
    } else {
        random_dense_bits_with_threshold_into(result, valid_bits, threshold, rng);
    }
```

The full function should read:

```rust
fn random_bits_with_prob_into(result: &mut [u64], valid_bits: usize, p: f64, rng: &mut impl Rng) {
    result.fill(0);
    if p <= 0.0 {
        return;
    }
    if p >= 1.0 {
        result.fill(!0u64);
        mask_unused_bits(result, valid_bits);
        return;
    }

    let threshold = probability_threshold_u64(p);
    if threshold == 0 {
        return;
    }

    if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
        random_sparse_bits_with_prob_into(result, valid_bits, p, rng);
    } else {
        random_dense_bits_with_threshold_into(result, valid_bits, threshold, rng);
    }
}
```

- [ ] **Step 3: Extract the dense helper**

Add this helper below `random_bits_with_prob_into`.

```rust
fn random_dense_bits_with_threshold_into(
    result: &mut [u64],
    valid_bits: usize,
    threshold: u64,
    rng: &mut impl Rng,
) {
    for (word_idx, word) in result.iter_mut().enumerate() {
        let valid_in_word = valid_bits.saturating_sub(word_idx * 64).min(64);
        for bit in 0..valid_in_word {
            if rng.r#gen::<u64>() < threshold {
                *word |= 1u64 << bit;
            }
        }
    }
}
```

- [ ] **Step 4: Add the sparse helper**

Add this helper below the dense helper.

```rust
fn random_sparse_bits_with_prob_into(
    result: &mut [u64],
    valid_bits: usize,
    p: f64,
    rng: &mut impl Rng,
) {
    let log_one_minus_p = (-p).ln_1p();
    let mut shot = 0usize;
    while shot < valid_bits {
        let u = loop {
            let candidate = rng.r#gen::<f64>();
            if candidate > 0.0 {
                break candidate;
            }
        };
        let skip = (u.ln() / log_one_minus_p).floor() as usize;
        shot = match shot.checked_add(skip + 1) {
            Some(next) => next,
            None => break,
        };
        if shot >= valid_bits {
            break;
        }
        result[shot / 64] |= 1u64 << (shot % 64);
    }
}
```

- [ ] **Step 5: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test frame_noise_masks
```

Expected: PASS. The focused test binary reports 8 passing tests, including `low_probability_noise_mask_uses_sparse_path` and `medium_probability_noise_mask_keeps_dense_path`.

- [ ] **Step 6: Commit the implementation**

```bash
git add rstim/src/sim/frame.rs
git commit -m "feat: add sparse frame noise masks"
```

### Task 3: Final Verification And Smoke Evidence

**Files:**
- No source files should be modified by this task.

**Interfaces:**
- Consumes: issue #427 verification commands
- Produces: command output evidence for PR description

- [ ] **Step 1: Run full workspace tests**

Run:

```sh
cargo test
```

Expected: PASS. Existing unrelated warnings from `rmatching/tests/coverage.rs` may appear, matching the current baseline.

- [ ] **Step 2: Run release smoke**

Run:

```sh
rm -rf /tmp/rstim-speed-sparse-mask
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case \
  --profile release \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 3 \
  --out-dir /tmp/rstim-speed-sparse-mask
python3 - <<'PY'
import json
from pathlib import Path
summary = json.loads(Path('/tmp/rstim-speed-sparse-mask/summary.json').read_text())
case = next(c for c in summary['cases'] if c['case_label'] == 'stim-style-surface-sample-d11-r100-b1024')
rates = {v['tool_variant']: v['median_shots_per_second'] for v in case['variants']}
ratio = case['rstim_compiled_vs_stim_cli_ratio']['ratio']
assert rates['rstim-compiled'] > 0
assert rates['stim-cli'] > 0
assert ratio > 0
print(f"PASS sparse-mask release smoke: rstim-compiled={rates['rstim-compiled']:.3f} shots/s, stim-cli={rates['stim-cli']:.3f} shots/s, ratio={ratio:.3f}")
PY
```

Expected: PASS. The final line begins with `PASS sparse-mask release smoke:` and reports positive `rstim-compiled`, `stim-cli`, and ratio values.

- [ ] **Step 3: Run formatting and diff checks**

Run:

```sh
cargo fmt --check
git diff --check
```

Expected: PASS with no formatting or whitespace errors.

- [ ] **Step 4: Review changed files**

Run:

```sh
git status --short
git diff --stat master...HEAD
```

Expected: only the issue #427 design/plan docs plus `rstim/src/sim/frame.rs` and `rstim/tests/frame_noise_masks.rs` are changed relative to `master`.

## Self-Review

- Spec coverage: Task 1 covers statistical validity, reproducibility, path-selection negative controls, and tail-mask regression coverage through existing tests. Task 2 implements sparse low-probability generation and preserves dense medium-probability generation. Task 3 covers the required focused test, full `cargo test`, formatting checks, and release smoke.
- Placeholder scan: no placeholder instructions remain; each code step includes exact code and each verification step has expected output.
- Type consistency: helper names and signatures match the current private frame simulator style and remain confined to `rstim/src/sim/frame.rs`.
