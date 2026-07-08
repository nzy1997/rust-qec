# Issue 412 Integer-Threshold Noise Masks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `FrameSimulator` simple Bernoulli event-mask generation, including `DEPOLARIZE1` and `DEPOLARIZE2` event selection, with an integer-threshold mask path.

**Architecture:** Keep the change localized to `rstim/src/sim/frame.rs` and `rstim/tests/frame_noise_masks.rs`. A shared helper converts `p` to a `u64` threshold and fills only valid shot bits; depolarizing channels consume the resulting event mask and keep their existing uniform Pauli choice semantics.

**Tech Stack:** Rust 2024, `rand` 0.8, Cargo integration tests, existing `FrameSimulator`, parser, and reference-sample APIs.

## Global Constraints

- `X_ERROR`, `Y_ERROR`, `Z_ERROR`, `CORRELATED_ERROR`, `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1` Bernoulli/herald masks must use the shared integer-threshold helper.
- `DEPOLARIZE1` and `DEPOLARIZE2` event selection must use the same shared helper or a scratch-filled variant of it.
- `DEPOLARIZE1` Pauli choice after an event fires remains uniformly distributed over X, Y, and Z.
- `DEPOLARIZE2` Pauli choice after an event fires remains uniformly distributed over the 15 non-identity two-qubit Paulis using `two_qubit_pauli`.
- `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`, and Pauli selection inside `HERALDED_PAULI_CHANNEL_1` are out of scope unless they are only consuming an already selected Bernoulli event mask.
- `p <= 0` returns an all-zero mask without consuming event bits.
- `p >= 1` returns a mask with every valid shot bit set.
- The final partial word must have out-of-range shot bits cleared when the batch size is not a multiple of 64.
- Tests must be probability-tolerant and must not assert exact stochastic masks across implementation changes beyond seeded reproducibility within the new implementation.
- Focused verification must include `cargo test -p rstim --test frame_noise_masks`.
- Final verification must include `cargo test`.
- Do not add timing thresholds.

---

### Task 1: Add Focused Noise Mask Tests

**Files:**
- Create: `rstim/tests/frame_noise_masks.rs`
- Modify: `docs/superpowers/plans/2026-07-08-issue-412-integer-threshold-noise-masks.md`

**Interfaces:**
- Consumes: `rstim::sim::frame::FrameSimulator`, `rstim::executor::reference_sample`, `rstim::parser::parse_lines`.
- Produces: Integration tests named `noise_mask_p_zero_is_empty`, `noise_mask_p_one_is_all_ones`, `low_probability_noise_mask_has_expected_frequency_bounds`, `noise_mask_is_reproducible_for_seeded_rng`, `depolarize1_event_mask_uses_integer_threshold_path`, and `depolarize2_event_mask_uses_integer_threshold_path`.

- [x] **Step 1: Write the failing tests**

Create `rstim/tests/frame_noise_masks.rs`:

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sim::frame::FrameSimulator;

fn measurement_words(program: &str, batch_size: usize, seed: u64) -> Vec<u64> {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(1, batch_size);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    frame.measurements(&ref_sample).row_words(0).to_vec()
}

fn count_ones(words: &[u64]) -> u32 {
    words.iter().map(|word| word.count_ones()).sum()
}

fn valid_word_mask(batch_size: usize, word: usize) -> u64 {
    let remaining = batch_size.saturating_sub(word * 64);
    if remaining >= 64 {
        !0u64
    } else if remaining == 0 {
        0
    } else {
        (1u64 << remaining) - 1
    }
}

fn frame_source() -> &'static str {
    include_str!("../src/sim/frame.rs")
}

fn match_arm(source: &str, start_marker: &str, end_marker: &str) -> String {
    let start = source.find(start_marker).expect("start marker present");
    let tail = &source[start..];
    let end = tail.find(end_marker).expect("end marker present");
    tail[..end].to_string()
}

#[test]
fn noise_mask_p_zero_is_empty() {
    let words = measurement_words("X_ERROR(0) 0\nM 0\n", 130, 7);
    assert_eq!(words, vec![0, 0, 0]);
}

#[test]
fn noise_mask_p_one_is_all_ones() {
    let batch_size = 130;
    let words = measurement_words("X_ERROR(1) 0\nM 0\n", batch_size, 7);
    assert_eq!(words.len(), 3);
    assert_eq!(words[0], valid_word_mask(batch_size, 0));
    assert_eq!(words[1], valid_word_mask(batch_size, 1));
    assert_eq!(words[2], valid_word_mask(batch_size, 2));
}

#[test]
fn low_probability_noise_mask_has_expected_frequency_bounds() {
    let batch_size = 65_536;
    let words = measurement_words("X_ERROR(0.01) 0\nM 0\n", batch_size, 123);
    let hits = count_ones(&words);
    assert!(
        (550..=760).contains(&hits),
        "expected roughly 655 hits for p=0.01 over {batch_size} shots, got {hits}"
    );
}

#[test]
fn noise_mask_is_reproducible_for_seeded_rng() {
    let program = "X_ERROR(0.037) 0\nM 0\n";
    let first = measurement_words(program, 257, 99);
    let second = measurement_words(program, 257, 99);
    assert_eq!(first, second);
}

#[test]
fn depolarize1_event_mask_uses_integer_threshold_path() {
    let arm = match_arm(frame_source(), "\"DEPOLARIZE1\"", "\"DEPOLARIZE2\"");
    assert!(arm.contains("random_bits_with_prob"), "{arm}");
    assert!(!arm.contains("gen::<f64>() < p"), "{arm}");
    assert!(!arm.contains("r#gen::<f64>() < p"), "{arm}");
}

#[test]
fn depolarize2_event_mask_uses_integer_threshold_path() {
    let arm = match_arm(frame_source(), "\"DEPOLARIZE2\"", "\"CORRELATED_ERROR\"");
    assert!(arm.contains("random_bits_with_prob"), "{arm}");
    assert!(!arm.contains("gen::<f64>() < p"), "{arm}");
    assert!(!arm.contains("r#gen::<f64>() < p"), "{arm}");
}
```

- [x] **Step 2: Run the tests to verify RED**

Run:

```sh
cargo test -p rstim --test frame_noise_masks
```

Expected: FAIL. Before implementation, the `DEPOLARIZE1` and `DEPOLARIZE2` source guards fail because those match arms still contain `rng.r#gen::<f64>() < p`; `noise_mask_p_one_is_all_ones` may also fail on the final partial word if the old helper leaves out-of-range bits set.

---

### Task 2: Implement Shared Integer-Threshold Masks

**Files:**
- Modify: `rstim/src/sim/frame.rs`
- Modify: `docs/superpowers/plans/2026-07-08-issue-412-integer-threshold-noise-masks.md`

**Interfaces:**
- Consumes: existing `random_bits_with_prob` call sites and `FrameSimulator::batch_size`.
- Produces: `random_bits_with_prob(words: usize, valid_bits: usize, p: f64, rng: &mut impl Rng) -> Vec<u64>` and depolarizing match arms that consume event masks from that helper.

- [ ] **Step 1: Update Bernoulli helper signature and call sites**

In `rstim/src/sim/frame.rs`, change every simple Bernoulli mask call from:

```rust
let noise = random_bits_with_prob(wpr, p, rng);
```

to:

```rust
let noise = random_bits_with_prob(wpr, self.batch_size, p, rng);
```

Apply the same `self.batch_size` argument for correlated, else-correlated, heralded erase, and heralded Pauli channel masks.

- [ ] **Step 2: Replace the helper implementation**

Replace `random_bits_with_prob` with:

```rust
fn random_bits_with_prob(words: usize, valid_bits: usize, p: f64, rng: &mut impl Rng) -> Vec<u64> {
    let mut result = vec![0u64; words];
    if p <= 0.0 {
        return result;
    }
    if p >= 1.0 {
        result.fill(!0u64);
        mask_unused_bits(&mut result, valid_bits);
        return result;
    }

    let threshold = probability_threshold_u64(p);
    if threshold == 0 {
        return result;
    }

    for (word_idx, word) in result.iter_mut().enumerate() {
        let valid_in_word = valid_bits.saturating_sub(word_idx * 64).min(64);
        for bit in 0..valid_in_word {
            if rng.r#gen::<u64>() < threshold {
                *word |= 1u64 << bit;
            }
        }
    }
    result
}

fn probability_threshold_u64(p: f64) -> u64 {
    (p * 18_446_744_073_709_551_616.0) as u64
}

fn mask_unused_bits(words: &mut [u64], valid_bits: usize) {
    if words.is_empty() {
        return;
    }
    let valid_in_last = valid_bits % 64;
    if valid_in_last != 0 {
        let mask = (1u64 << valid_in_last) - 1;
        if let Some(last) = words.last_mut() {
            *last &= mask;
        }
    }
}
```

- [ ] **Step 3: Route `DEPOLARIZE1` through the helper**

Replace the nested per-bit event-selection loop in the `DEPOLARIZE1` match arm with:

```rust
"DEPOLARIZE1" => {
    let p = args.first().copied().unwrap_or(0.0);
    if p > 0.0 {
        for q in qubits(targets)? {
            let events = random_bits_with_prob(wpr, self.batch_size, p, rng);
            let mut xf = vec![0u64; wpr];
            let mut zf = vec![0u64; wpr];
            for w in 0..wpr {
                let mut bits = events[w];
                while bits != 0 {
                    let bit = bits.trailing_zeros();
                    match rng.gen_range(0u8..3) {
                        0 => xf[w] |= 1u64 << bit,
                        1 => {
                            xf[w] |= 1u64 << bit;
                            zf[w] |= 1u64 << bit;
                        }
                        _ => zf[w] |= 1u64 << bit,
                    }
                    bits &= bits - 1;
                }
            }
            let x = self.x_table.row_words_mut(q);
            for w in 0..wpr { x[w] ^= xf[w]; }
            let z = self.z_table.row_words_mut(q);
            for w in 0..wpr { z[w] ^= zf[w]; }
        }
    }
}
```

- [ ] **Step 4: Route `DEPOLARIZE2` through the helper**

Replace the nested per-bit event-selection loop in the `DEPOLARIZE2` match arm with:

```rust
"DEPOLARIZE2" => {
    let p = args.first().copied().unwrap_or(0.0);
    if p > 0.0 {
        for (qa, qb) in qubit_pairs(targets)? {
            let events = random_bits_with_prob(wpr, self.batch_size, p, rng);
            let mut xa = vec![0u64; wpr];
            let mut za = vec![0u64; wpr];
            let mut xb = vec![0u64; wpr];
            let mut zb = vec![0u64; wpr];
            for w in 0..wpr {
                let mut bits = events[w];
                while bits != 0 {
                    let bit = bits.trailing_zeros();
                    let r = rng.gen_range(0u8..15);
                    let (pa, pb) = two_qubit_pauli(r);
                    apply_pauli_bits(pa, &mut xa, &mut za, w, bit);
                    apply_pauli_bits(pb, &mut xb, &mut zb, w, bit);
                    bits &= bits - 1;
                }
            }
            let x = self.x_table.row_words_mut(qa);
            for w in 0..wpr { x[w] ^= xa[w]; }
            let z = self.z_table.row_words_mut(qa);
            for w in 0..wpr { z[w] ^= za[w]; }
            let x = self.x_table.row_words_mut(qb);
            for w in 0..wpr { x[w] ^= xb[w]; }
            let z = self.z_table.row_words_mut(qb);
            for w in 0..wpr { z[w] ^= zb[w]; }
        }
    }
}
```

- [ ] **Step 5: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test frame_noise_masks
```

Expected: PASS, with all six issue-required tests passing.

- [ ] **Step 6: Run final verification**

Run:

```sh
cargo test
```

Expected: PASS for the workspace default test set.
