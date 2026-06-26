# Issue 280 BB hard-syndrome ldpc_cs routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route BB90 hard-syndrome diagnostics and bounded profile helpers through the explicit `ldpc_cs` candidate planner while preserving a legacy negative control.

**Architecture:** Keep existing BB helper wrappers legacy-compatible, add explicit `OsdVariant` helper variants, and surface `osd_planner` in `SyndromeReplayDiagnostic`. The BB90 fixture tests use the explicit `LdpcCombinationSweep` variant for the bounded count and the existing wrapper for the legacy `26332` count.

**Tech Stack:** Rust 2024 workspace; `rsinter` BB circuit memory helpers; `rbposd::OsdVariant`; existing BB90 hard-syndrome JSON fixture; `cargo test`.

## Global Constraints

- Do not change the BB90 hard-syndrome sampled syndrome.
- `max_bp_iterations` remains `10000` and `osd_order` remains `7` for the issue tests.
- `ldpc_cs` diagnostics must report `planned_candidate_count = free_column_count + C(7, 2)` for the BB90 fixture.
- Legacy diagnostics must remain separately identifiable and must still report the stored `26332` frontier count.
- Do not implement influence-vector optimization or run Monte Carlo trials.
- Use TDD: write the issue tests, run them red, then implement production code.

---

## File Structure

- Modify `rsinter/tests/bb90_hard_syndrome_fixture.rs`: add the issue positive test and legacy negative control.
- Modify `rsinter/src/bb_circuit_memory.rs`: expose `osd_planner` in replay diagnostics and add explicit `OsdVariant` replay/profile helper variants.
- Add `docs/superpowers/specs/2026-06-26-issue-280-bb-hard-syndrome-ldpc-cs-design.md`: brainstorming design artifact.
- Add `docs/superpowers/plans/2026-06-26-issue-280-bb-hard-syndrome-ldpc-cs.md`: implementation plan artifact.

### Task 1: Add Failing BB90 Hard-Syndrome Planner Tests

**Files:**
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Consumes: `HardSyndromeFixture`, `compute_fixture_replay`, `profile_replay_basis`, existing replay/profile helpers.
- Produces: failing tests named `bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded` and `bb90_hard_syndrome_legacy_osd_plan_still_reports_exhaustive_frontier`.

- [ ] **Step 1: Import `OsdVariant` and the explicit helper names**

Change the top imports to include:

```rust
use rbposd::{OsdVariant, ParityCheckMatrix};
use rsinter::bb_circuit_memory::{
    /* existing imports */,
    profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant,
    replay_syndrome_diagnostic_with_osd_variant,
};
```

- [ ] **Step 2: Add the positive `ldpc_cs` diagnostic/profile test**

Add this test after `bb90_hard_syndrome_reports_osd_profile_counters`:

```rust
#[test]
fn bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded() {
    let fixture = load_fixture();
    let computed = compute_fixture_replay(&fixture).unwrap();
    let diagnostic = replay_syndrome_diagnostic_with_osd_variant(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();

    let expected_pair_count = 21u128;
    assert_eq!(diagnostic.osd_planner, "ldpc_osd_cs");
    assert_eq!(diagnostic.osd_order, 7);
    assert!(diagnostic.free_column_count > 0);
    assert_eq!(diagnostic.candidate_search_frontier_size, 7);
    assert_eq!(diagnostic.max_candidate_order, 2);
    assert_eq!(
        diagnostic.planned_candidate_count,
        diagnostic.free_column_count as u128 + expected_pair_count
    );
    assert!(diagnostic.planned_candidate_count < fixture.expected_planned_candidate_count);

    let profile = profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant(
        profile_replay_basis(fixture.basis),
        &computed.model,
        &computed.sampled_syndrome,
        fixture.max_bp_iterations,
        fixture.osd_order,
        PROFILE_CANDIDATE_LIMIT,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();
    assert_eq!(profile.osd_candidate_count, PROFILE_CANDIDATE_LIMIT);
    assert_eq!(profile.gf2_solve_count, PROFILE_CANDIDATE_LIMIT + 1);
}
```

- [ ] **Step 3: Add the legacy negative control**

Add this test after the positive control:

```rust
#[test]
fn bb90_hard_syndrome_legacy_osd_plan_still_reports_exhaustive_frontier() {
    let fixture = load_fixture();
    let computed = compute_fixture_replay(&fixture).unwrap();
    let legacy = replay_syndrome_diagnostic(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
    )
    .unwrap();
    let ldpc = replay_syndrome_diagnostic_with_osd_variant(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();

    assert_eq!(legacy.osd_planner, "legacy_combination_sweep");
    assert_eq!(
        legacy.planned_candidate_count,
        fixture.expected_planned_candidate_count
    );
    assert_eq!(legacy.planned_candidate_count, 26_332);
    assert_ne!(legacy.osd_planner, ldpc.osd_planner);
    assert_ne!(legacy.planned_candidate_count, ldpc.planned_candidate_count);
}
```

- [ ] **Step 4: Run the positive test red**

Run:

```bash
cargo test -p rsinter bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded -- --nocapture
```

Expected: FAIL before implementation because the explicit helper functions and `SyndromeReplayDiagnostic::osd_planner` do not exist.

### Task 2: Add Explicit BB Replay/Profile Planner Routing

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`

**Interfaces:**
- Consumes: failing tests from Task 1 and `rbposd::OsdVariant`.
- Produces: `SyndromeReplayDiagnostic::osd_planner`, `replay_syndrome_diagnostic_with_osd_variant`, `profile_syndrome_replay_for_basis_with_osd_variant`, and `profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant`.

- [ ] **Step 1: Import `OsdVariant` and add `osd_planner`**

Change the `rbposd` import to include `OsdVariant`. Add this field to `SyndromeReplayDiagnostic` after `residual_syndrome_weight`:

```rust
pub osd_planner: &'static str,
```

- [ ] **Step 2: Add explicit diagnostic helper**

Refactor `replay_syndrome_diagnostic` to call a new helper:

```rust
pub fn replay_syndrome_diagnostic(...) -> Result<SyndromeReplayDiagnostic, String> {
    replay_syndrome_diagnostic_with_osd_variant(
        model,
        syndrome_bits,
        num_logicals,
        max_bp_iterations,
        osd_order,
        OsdVariant::Osd0,
    )
}
```

Implement `replay_syndrome_diagnostic_with_osd_variant` with the same body as the existing function, but set `DecoderConfig { max_bp_iterations, osd_variant, osd_order, ..DecoderConfig::default() }` for the diagnostic decoder and copy `diagnostic.osd_planner` into `SyndromeReplayDiagnostic`.

- [ ] **Step 3: Add explicit profile helper variants**

Refactor `profile_syndrome_replay_for_basis` to call:

```rust
profile_syndrome_replay_for_basis_with_osd_variant(
    basis,
    model,
    syndrome_bits,
    max_bp_iterations,
    osd_order,
    OsdVariant::Osd0,
)
```

Implement `profile_syndrome_replay_for_basis_with_osd_variant` with the existing body and an explicit `osd_variant` in `DecoderConfig`.

Refactor `profile_syndrome_replay_with_candidate_limit_for_basis` to call:

```rust
profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant(
    basis,
    model,
    syndrome_bits,
    max_bp_iterations,
    osd_order,
    osd_candidate_limit,
    OsdVariant::Osd0,
)
```

Implement `profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant` with the existing body and an explicit `osd_variant` in `DecoderConfig`.

- [ ] **Step 4: Run issue tests green**

Run:

```bash
cargo test -p rsinter bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded -- --nocapture
cargo test -p rsinter bb90_hard_syndrome_legacy_osd_plan_still_reports_exhaustive_frontier -q
```

Expected: both PASS. The positive test reports `ldpc_osd_cs` and `planned_candidate_count = free_column_count + 21`; the negative test reports `legacy_combination_sweep` and `26332`.

### Task 3: Final Verification

**Files:**
- All touched files.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: formatted, verified PR-ready branch.

- [ ] **Step 1: Format/check touched Rust files**

Run:

```bash
rustfmt --edition 2024 rsinter/src/bb_circuit_memory.rs rsinter/tests/bb90_hard_syndrome_fixture.rs --check
```

Expected: PASS. If it fails, run the same command without `--check`, inspect the diff, and rerun with `--check`.

- [ ] **Step 2: Run issue verification**

Run:

```bash
cargo test -p rsinter bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded -- --nocapture
cargo test -p rsinter bb90_hard_syndrome_legacy_osd_plan_still_reports_exhaustive_frontier -q
```

Expected: both PASS.

- [ ] **Step 3: Run full workspace verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Review diff hygiene**

Run:

```bash
git diff --check
git diff --stat
```

Expected: no whitespace errors and changes limited to the Superpowers artifacts, BB helper routing, and BB90 fixture tests.

## Self-Review

- Spec coverage: the plan covers explicit `ldpc_cs` diagnostics, bounded candidate-count assertion, profile helper routing, and legacy negative control.
- Placeholder scan: no placeholder instructions remain.
- Type consistency: helper names and planner names match the design and existing `rbposd::OsdVariant` contract.

## Execution Choice

Standing answer policy selects **Subagent-Driven (recommended)** because it is the recommended option in the writing-plans handoff. This run records any deviation caused by Agent Desk/tooling constraints in the final decision log.
