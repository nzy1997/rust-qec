# Issue 142 APM P=96 rbposd Smoke Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic `rsinter` smoke test proving the P=96 APM-CSS fixtures compile into native `rbposd` BP/BP-OSD and decode selected seeded syndromes to residual zero.

**Architecture:** Keep the implementation as one integration test under `rsinter/tests` so no production API changes are required. The test parses the #141 fixture pair, builds an Hx-side `rbposd::ParityCheckMatrix`, generates fixed sparse Z-error supports from a local SplitMix64 seed, decodes their syndromes through `BpOsdDecoder`, and compares against an all-zero correction negative control.

**Tech Stack:** Rust 2024, Cargo workspace integration tests, `qec-code` sparse-row fixture parser, `rbposd::BpOsdDecoder`.

## Global Constraints

- The test must be named `apm_p96_rbposd_smoke_decodes_seeded_syndromes`.
- The verification command is `cargo test -p rsinter apm_p96_rbposd_smoke_decodes_seeded_syndromes -q`.
- The test must load `rsinter/tests/fixtures/css/apm_p96_hx.json` and `rsinter/tests/fixtures/css/apm_p96_hz.json`.
- The test must build native `rbposd` decoders for one CSS side.
- The test must generate at least three fixed nonzero syndromes from known sparse errors.
- The test must decode the selected known syndromes to residual-zero corrections.
- The negative control must run an all-zero correction on the same nonzero syndromes and assert at least one residual remains nonzero.
- Record BP config values explicitly: `max_bp_iterations = 96`, `early_stop = true`, `bp_variant = BpVariant::MinimumSum`, `schedule = Schedule::Parallel`, `osd_variant = OsdVariant::Osd0`, `osd_order = 0`.
- Use fixed seed `0xA9_6B_50_D5_EE_D5_14_2A`.
- Pin generated supports as `[223]`, `[780, 1033]`, and `[346, 632, 921]`.
- Do not attempt to reproduce plotted logical error rates.
- Do not implement relay-BP or MIP fallback.
- Run `cargo test`.

---

## File Structure

- Create: `rsinter/tests/apm_p96_rbposd_smoke.rs` with the deterministic smoke test.
- Create: `docs/superpowers/plans/2026-06-24-issue-142-apm-p96-rbposd-smoke.md` with this execution plan.

### Task 1: APM P=96 rbposd Native Smoke

**Files:**
- Create: `rsinter/tests/apm_p96_rbposd_smoke.rs`
- Modify: no production files

**Interfaces:**
- Consumes: `qec_code::css::sparse_rows_matrix_from_json_str(&str) -> Result<SparseRowsMatrix, QecError>`
- Consumes: `rbposd::ParityCheckMatrix::from_sparse_rows(num_checks, num_bits, rows) -> Result<ParityCheckMatrix, DecodeError>`
- Consumes: `rbposd::BpOsdDecoder::new(pcm, channel, config) -> Result<BpOsdDecoder, DecodeError>`
- Consumes: `rbposd::BpOsdDecoder::decode(&Syndrome) -> Result<DecodeResult, DecodeError>`
- Produces: integration test `apm_p96_rbposd_smoke_decodes_seeded_syndromes`

- [ ] **Step 1: Write the failing smoke test skeleton**

Create `rsinter/tests/apm_p96_rbposd_smoke.rs` with a test that loads the fixtures and intentionally panics at the missing implementation point:

```rust
use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};

const APM_P96_HX_JSON: &str = include_str!("fixtures/css/apm_p96_hx.json");
const APM_P96_HZ_JSON: &str = include_str!("fixtures/css/apm_p96_hz.json");

#[test]
fn apm_p96_rbposd_smoke_decodes_seeded_syndromes() {
    let hx = parse_sparse_rows(APM_P96_HX_JSON, "Hx");
    let hz = parse_sparse_rows(APM_P96_HZ_JSON, "Hz");
    assert_eq!(hx.num_cols(), hz.num_cols());

    panic!("APM P=96 rbposd smoke implementation is not wired yet");
}

fn parse_sparse_rows(input: &str, label: &str) -> SparseRowsMatrix {
    sparse_rows_matrix_from_json_str(input)
        .unwrap_or_else(|error| panic!("failed to parse APM P=96 {label} fixture: {error}"))
}
```

- [ ] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p rsinter apm_p96_rbposd_smoke_decodes_seeded_syndromes -q
```

Expected: FAIL with `APM P=96 rbposd smoke implementation is not wired yet`.

- [ ] **Step 3: Replace the skeleton with the full smoke implementation**

Replace `rsinter/tests/apm_p96_rbposd_smoke.rs` with:

```rust
use std::collections::BTreeSet;

use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};
use rbposd::{
    BpOsdDecoder, BpVariant, ChannelModel, Correction, DecoderConfig, OsdVariant,
    ParityCheckMatrix, Schedule, Syndrome,
};

const APM_P96_NUM_QUBITS: usize = 1152;
const APM_P96_HX_JSON: &str = include_str!("fixtures/css/apm_p96_hx.json");
const APM_P96_HZ_JSON: &str = include_str!("fixtures/css/apm_p96_hz.json");
const APM_P96_SEED: u64 = 0xA9_6B_50_D5_EE_D5_14_2A;
const APM_P96_ERROR_WEIGHTS: [usize; 3] = [1, 2, 3];
const APM_P96_EXPECTED_SUPPORTS: &[&[usize]] = &[&[223], &[780, 1033], &[346, 632, 921]];
const APM_P96_CHANNEL_ERROR_RATE: f64 = 0.02;

#[test]
fn apm_p96_rbposd_smoke_decodes_seeded_syndromes() {
    let hx = parse_sparse_rows(APM_P96_HX_JSON, "Hx");
    let hz = parse_sparse_rows(APM_P96_HZ_JSON, "Hz");
    assert_eq!(hx.num_cols(), APM_P96_NUM_QUBITS);
    assert_eq!(hz.num_cols(), APM_P96_NUM_QUBITS);
    assert!(!hz.rows().is_empty(), "APM P=96 Hz fixture should be loaded");

    let pcm = parity_check_from_sparse_rows(&hx);
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc {
            error_rate: APM_P96_CHANNEL_ERROR_RATE,
        },
        apm_p96_decoder_config(),
    )
    .unwrap_or_else(|error| panic!("failed to compile APM P=96 Hx rbposd decoder: {error}"));

    let supports = seeded_error_supports(
        APM_P96_SEED,
        &APM_P96_ERROR_WEIGHTS,
        APM_P96_NUM_QUBITS,
    );
    assert_eq!(supports, expected_supports());

    let mut zero_control_left_residual = false;
    for support in supports {
        let known_error = correction_from_support(APM_P96_NUM_QUBITS, &support);
        let syndrome = pcm.multiply(&known_error);
        assert!(
            syndrome.weight() > 0,
            "seeded support {support:?} should generate a nonzero syndrome"
        );

        zero_control_left_residual |=
            residual_weight(&pcm, &Correction::zero(APM_P96_NUM_QUBITS), &syndrome) > 0;

        let result = decoder.decode(&syndrome).unwrap_or_else(|error| {
            panic!("failed to decode seeded support {support:?}: {error}")
        });
        assert_eq!(
            residual_weight(&pcm, &result.correction, &syndrome),
            0,
            "decoded correction should satisfy the seeded support {support:?} syndrome"
        );
    }

    assert!(
        zero_control_left_residual,
        "all-zero correction should leave a residual on at least one nonzero seeded syndrome"
    );
}

fn apm_p96_decoder_config() -> DecoderConfig {
    DecoderConfig {
        max_bp_iterations: 96,
        early_stop: true,
        bp_variant: BpVariant::MinimumSum,
        schedule: Schedule::Parallel,
        osd_variant: OsdVariant::Osd0,
        osd_order: 0,
    }
}

fn parse_sparse_rows(input: &str, label: &str) -> SparseRowsMatrix {
    sparse_rows_matrix_from_json_str(input)
        .unwrap_or_else(|error| panic!("failed to parse APM P=96 {label} fixture: {error}"))
}

fn parity_check_from_sparse_rows(matrix: &SparseRowsMatrix) -> ParityCheckMatrix {
    ParityCheckMatrix::from_sparse_rows(
        matrix.rows().len(),
        matrix.num_cols(),
        matrix.rows().to_vec(),
    )
    .unwrap_or_else(|error| panic!("failed to build rbposd parity matrix: {error}"))
}

fn seeded_error_supports(seed: u64, weights: &[usize], num_bits: usize) -> Vec<Vec<usize>> {
    let mut state = seed;
    weights
        .iter()
        .map(|&weight| {
            let mut support = BTreeSet::new();
            while support.len() < weight {
                support.insert((splitmix64_next(&mut state) % num_bits as u64) as usize);
            }
            support.into_iter().collect()
        })
        .collect()
}

fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn expected_supports() -> Vec<Vec<usize>> {
    APM_P96_EXPECTED_SUPPORTS
        .iter()
        .map(|support| support.to_vec())
        .collect()
}

fn correction_from_support(num_bits: usize, support: &[usize]) -> Correction {
    let mut bits = vec![false; num_bits];
    for &bit in support {
        bits[bit] = true;
    }
    Correction::from(bits)
}

fn residual_weight(
    pcm: &ParityCheckMatrix,
    correction: &Correction,
    target_syndrome: &Syndrome,
) -> usize {
    let decoded_syndrome = pcm.multiply(correction);
    decoded_syndrome
        .as_slice()
        .iter()
        .zip(target_syndrome.as_slice())
        .filter(|(decoded, target)| decoded != target)
        .count()
}
```

- [ ] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p rsinter apm_p96_rbposd_smoke_decodes_seeded_syndromes -q
```

Expected: PASS.

- [ ] **Step 5: Run full workspace verification**

Run:

```sh
cargo test
```

Expected: PASS. Existing warnings from unrelated crates may still print, but the command must exit 0.

- [ ] **Step 6: Commit**

```sh
git add docs/superpowers/plans/2026-06-24-issue-142-apm-p96-rbposd-smoke.md \
  rsinter/tests/apm_p96_rbposd_smoke.rs
git commit -m "test: add apm p96 rbposd smoke"
```
