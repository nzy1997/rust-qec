# Issue 307 BB90 LDPC OSD-CS Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Rust `rbposd` `ldpc_osd_cs` / `osd_cs` produce the same BB90 hard-replay Z logical prediction as Python `ldpc.BpOsdDecoder`.

**Architecture:** Keep the existing BP-first decode flow. When BP does not converge and the selected OSD variant is `LdpcCombinationSweep`, hand OSD the original syndrome, signed BP posterior log-probability ratios for column ordering, and `log(1 / p_i)` channel weights for candidate scoring. Leave legacy combination sweep and explicit OSD-0 residual semantics unchanged.

**Tech Stack:** Rust 2024 workspace (`rbposd`, `rsinter`), Cargo tests, Python hard-replay benchmark verifier.

## Global Constraints

- Preserve bounded `ldpc_osd_cs` candidate planning: singles over all non-pivot columns plus pairs among the first `osd_order` non-pivot columns.
- Preserve `osd_cs` and `ldpc_osd_cs` as aliases for `OsdVariant::LdpcCombinationSweep`.
- Preserve legacy combination sweep and explicit OSD-0 behavior.
- Preserve Rust hard replay counters: nonzero OSD use, finite counter fields, and one reused GF(2) elimination for LDPC OSD-CS.
- The pinned BB90 hard replay must produce Rust logical prediction `[true, true, false, true, true, false, false, false]`, matching Python `ldpc.BpOsdDecoder`.
- Do not run the full BB72/BB144 campaign.

---

## File Structure

- Modify `rsinter/tests/bb90_hard_syndrome_fixture.rs`: add the failing fixture-level regression that pins the Python hard-replay logical prediction for `LdpcCombinationSweep`.
- Modify `rbposd/src/decoder_core.rs`: expose `log(1 / p_i)` channel objective weights alongside existing prior LLRs.
- Modify `rbposd/src/decoder.rs`: route only `LdpcCombinationSweep` OSD through original-syndrome all-zero-base solving, signed BP posterior ordering, and `log(1 / p_i)` objective weights.
- Modify `rbposd/tests/osd.rs`: update/add focused unit coverage proving LDPC OSD-CS ignores the BP hard-decision residual base while legacy OSD keeps it.

### Task 1: LDPC OSD-CS Upstream Handoff

**Files:**
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`
- Modify: `rbposd/src/decoder_core.rs`
- Modify: `rbposd/src/decoder.rs`
- Modify: `rbposd/tests/osd.rs`

**Interfaces:**
- Consumes: `BpWorkspace.posterior_llr`, `BpWorkspace.hard_decision_bits`, `BpCore` channel probabilities, existing `decode_osd_with_workspace`.
- Produces: `BpCore::channel_probability_objective_weights(&self) -> &[f64]` and LDPC OSD-CS decode/profile/diagnostic calls that use all-zero base correction bits and signed posterior ordering.

- [ ] **Step 1: Add the failing BB90 regression**

In `rsinter/tests/bb90_hard_syndrome_fixture.rs`, extend the import from
`rsinter::bb_circuit_memory` to include
`export_comparison_case_for_code_with_osd_variant`.

Add this test after `bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded`:

```rust
#[test]
fn bb90_hard_syndrome_ldpc_cs_matches_python_logical_prediction() {
    let fixture = load_fixture();
    let export = export_comparison_case_for_code_with_osd_variant(
        &fixture.code_id,
        SimulationConfig {
            physical_error_rate: fixture.physical_error_rate,
            num_cycles: fixture.num_cycles,
            num_trials: 1,
            seed: Some(fixture.seed),
            max_bp_iterations: fixture.max_bp_iterations,
            osd_order: fixture.osd_order,
        },
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();
    let trial = export.trials.first().unwrap();
    let prediction = trial.z_logical_prediction.as_ref().unwrap();

    assert_eq!(
        prediction,
        &vec![true, true, false, true, true, false, false, false]
    );
    assert_eq!(trial.z_logical.as_slice(), fixture.expected_sampled_logical.as_slice());
    let profile = trial.z_profile.as_ref().unwrap();
    assert_eq!(profile.osd_use_count, 1);
    assert!(profile.osd_candidate_count > 0);
    assert_eq!(profile.gf2_solve_count, 1);
    assert_eq!(profile.gf2_full_elimination_count, 1);
}
```

- [ ] **Step 2: Run the regression and verify RED**

Run:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_ldpc_cs_matches_python_logical_prediction -- --nocapture
```

Expected before implementation: test fails because the current Rust prediction
is `[true, true, false, true, true, false, false, true]`.

- [ ] **Step 3: Add a focused rbposd unit test for LDPC original-syndrome semantics**

In `rbposd/tests/osd.rs`, add this test near the other LDPC OSD-CS tests:

```rust
#[test]
fn ldpc_osd_cs_solves_original_syndrome_instead_of_bp_residual() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1]]).unwrap();
    let channel = ChannelModel::BitFlipProbabilities(vec![0.9, 0.1, 0.2]);
    let syndrome = Syndrome::from(vec![true]);
    let config = DecoderConfig {
        max_bp_iterations: 0,
        osd_variant: OsdVariant::LdpcCombinationSweep,
        osd_order: 1,
        ..DecoderConfig::default()
    };

    let result = BpOsdDecoder::new(pcm.clone(), channel.clone(), config)
        .unwrap()
        .decode(&syndrome)
        .unwrap();

    assert_eq!(result.correction, Correction::from(vec![false, true, false]));
    assert_eq!(pcm.multiply(&result.correction), syndrome);

    let legacy = BpOsdDecoder::new(
        pcm,
        channel,
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LegacyCombinationSweep,
            osd_order: 1,
            ..DecoderConfig::default()
        },
    )
    .unwrap()
    .decode(&syndrome)
    .unwrap();

    assert_eq!(legacy.correction, Correction::from(vec![true, false, false]));
}
```

- [ ] **Step 4: Run the focused unit test and verify RED**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_solves_original_syndrome_instead_of_bp_residual
```

Expected before implementation: test fails because LDPC OSD-CS currently keeps
the BP hard-decision residual base and returns `[true, false, false]`.

- [ ] **Step 5: Implement channel-probability objective weights**

In `rbposd/src/decoder_core.rs`, change `BpCore` to store both prior LLRs and
channel objective weights:

```rust
#[derive(Debug, Clone)]
pub(crate) struct BpCore {
    graph: CompiledGraph,
    prior_llrs: Vec<f64>,
    channel_probability_objective_weights: Vec<f64>,
}
```

In `BpCore::new`, compute both vectors:

```rust
let prior_llrs = compute_prior_llrs(pcm, channel)?;
let channel_probability_objective_weights =
    compute_channel_probability_objective_weights(pcm, channel)?;
Ok(Self {
    graph: CompiledGraph::from_pcm(pcm),
    prior_llrs,
    channel_probability_objective_weights,
})
```

Add:

```rust
pub(crate) fn channel_probability_objective_weights(&self) -> &[f64] {
    &self.channel_probability_objective_weights
}
```

Add helpers:

```rust
pub(crate) fn compute_channel_probability_objective_weights(
    pcm: &ParityCheckMatrix,
    channel: &ChannelModel,
) -> Result<Vec<f64>, DecodeError> {
    match channel {
        ChannelModel::Bsc { error_rate } => {
            let probability = validate_probability(*error_rate)?;
            Ok(vec![probability_to_inverse_log_weight(probability); pcm.num_bits()])
        }
        ChannelModel::BitFlipProbabilities(probabilities) => {
            if probabilities.len() != pcm.num_bits() {
                return Err(DecodeError::DimensionMismatch {
                    what: "channel probabilities",
                    expected: pcm.num_bits(),
                    actual: probabilities.len(),
                });
            }
            probabilities
                .iter()
                .map(|&probability| {
                    validate_probability(probability).map(probability_to_inverse_log_weight)
                })
                .collect()
        }
    }
}

fn probability_to_inverse_log_weight(probability: f64) -> f64 {
    (1.0 / probability).ln()
}
```

Add a unit test in `decoder_core.rs` beside the prior-LLR tests:

```rust
#[test]
fn bp_core_exposes_channel_probability_objective_weights() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
    let core = BpCore::new(
        &pcm,
        &ChannelModel::BitFlipProbabilities(vec![0.2, 0.4, 0.8]),
    )
    .unwrap();

    let expected = vec![(1.0_f64 / 0.2).ln(), (1.0_f64 / 0.4).ln(), (1.0_f64 / 0.8).ln()];
    assert_eq!(core.channel_probability_objective_weights(), expected.as_slice());
}
```

- [ ] **Step 6: Route LDPC OSD-CS through original-syndrome all-zero-base solving**

In `rbposd/src/decoder.rs`, add a private helper:

```rust
fn osd_inputs_for_variant<'a>(
    planner: crate::config::OsdVariant,
    core: &'a BpCore,
    bp_workspace: &'a BpWorkspace,
    zero_base: &'a [bool],
) -> (&'a [bool], &'a [f64], &'a [f64]) {
    match planner {
        crate::config::OsdVariant::LdpcCombinationSweep => (
            zero_base,
            &bp_workspace.posterior_llr,
            core.channel_probability_objective_weights(),
        ),
        crate::config::OsdVariant::Osd0
        | crate::config::OsdVariant::LegacyCombinationSweep => (
            &bp_workspace.hard_decision_bits,
            &bp_workspace.reliability,
            &bp_workspace.reliability,
        ),
    }
}
```

In `decode`, replace the LDPC objective-only match with:

```rust
let zero_base;
let (base_correction_bits, ordering_reliability, objective_weights) = {
    zero_base = vec![false; self.pcm.num_bits()];
    osd_inputs_for_variant(effective_planner, &self.core, &bp_workspace, &zero_base)
};
decode_osd_with_workspace(
    &self.pcm,
    syndrome,
    base_correction_bits,
    ordering_reliability,
    objective_weights,
    &mut osd_workspace,
    effective_planner,
    self.config.osd_order,
)?
```

In `diagnose_osd_path` and `profile_decode_with_osd_candidate_limit`, use the
same helper and pass `base_correction_bits` / `ordering_reliability` into
`diagnose_osd_candidate_search_with_workspace` and `profile_osd_with_workspace`.

- [ ] **Step 7: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_solves_original_syndrome_instead_of_bp_residual
cargo test --release -p rsinter bb90_hard_syndrome_ldpc_cs_matches_python_logical_prediction -- --nocapture
```

Expected: both pass.

- [ ] **Step 8: Run broader required checks**

Run:

```bash
cargo test -p rbposd osd
cargo test --release -p rsinter bb90_hard_syndrome
```

Expected: both pass.

- [ ] **Step 9: Commit**

```bash
git add rbposd/src/decoder_core.rs rbposd/src/decoder.rs rbposd/tests/osd.rs rsinter/tests/bb90_hard_syndrome_fixture.rs
git commit -m "fix: align ldpc osd cs hard replay"
```
