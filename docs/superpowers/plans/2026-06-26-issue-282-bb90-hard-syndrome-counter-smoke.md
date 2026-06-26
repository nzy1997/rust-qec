# Issue 282 BB90 Hard-Syndrome Counter Smoke Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a release-mode BB90 hard-syndrome smoke that prints profile JSON and fails on counter regressions instead of wall-clock timing.

**Architecture:** Extend the existing BB90 fixture integration test with a local `ldpc_cs` counter-bound validator and a legacy negative control. The positive path profiles the hard fixture through `OsdVariant::LdpcCombinationSweep`; the negative path feeds legacy exhaustive/frontier diagnostics through the same validator and expects a named counter-bound error.

**Tech Stack:** Rust 2024 workspace; `rsinter` integration tests; existing BB90 hard-syndrome fixture; `serde_json`; `cargo test`.

## Global Constraints

- Do not change the BB90 hard-syndrome sampled syndrome or fixture JSON.
- `max_bp_iterations` remains `10000` and `osd_order` remains `7` for the issue tests.
- Positive profile mode must use `OsdVariant::LdpcCombinationSweep`.
- Keep assertions counter-based: candidate count, full-elimination count, solve count, and decode-call count.
- Treat wall-clock seconds as printed evidence only; do not fail solely on a timing threshold.
- `gf2_full_elimination_count == 1`.
- `decode_call_count == z_decode_call_count + x_decode_call_count`.
- Legacy exhaustive/frontier mode must be rejected by the `ldpc_cs` counter-bound validator, and the error must name the violating counter.

---

## File Structure

- Modify `rsinter/tests/bb90_hard_syndrome_fixture.rs`: add the release smoke, legacy negative control, profile JSON builder, and local counter-bound validator.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md`: document the release smoke and negative control commands.
- Add `docs/superpowers/specs/2026-06-26-issue-282-bb90-hard-syndrome-counter-smoke-design.md`: committed brainstorming design artifact.
- Add `docs/superpowers/plans/2026-06-26-issue-282-bb90-hard-syndrome-counter-smoke.md`: this implementation plan.

### Task 1: Add Failing Release Smoke Tests

**Files:**
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Consumes: `load_fixture`, `compute_fixture_replay`, `profile_replay_basis`, `profile_syndrome_replay_with_candidate_limit_for_basis`, `profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant`, `replay_syndrome_diagnostic`, `replay_syndrome_diagnostic_with_osd_variant`, `BbCircuitBposdProfile`, `SyndromeReplayDiagnostic`.
- Produces: failing tests named `bb90_hard_syndrome_release_profile_is_counter_bounded` and `bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds`.

- [ ] **Step 1: Import `BbCircuitBposdProfile`**

Change the `rsinter::bb_circuit_memory` import block in `rsinter/tests/bb90_hard_syndrome_fixture.rs` to include `BbCircuitBposdProfile`:

```rust
use rsinter::bb_circuit_memory::{
    BbCircuitBposdProfile, EffectiveDecoderModel, ProfileReplayBasis, SimulationConfig,
    SyndromeReplayDiagnostic, build_code, build_effective_models, build_syndrome_cycle,
    profile_syndrome_replay, profile_syndrome_replay_for_basis,
    profile_syndrome_replay_with_candidate_limit,
    profile_syndrome_replay_with_candidate_limit_for_basis,
    profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant,
    replay_syndrome_diagnostic, replay_syndrome_diagnostic_with_osd_variant, sample_seeded_trial,
};
```

- [ ] **Step 2: Add the positive release smoke test**

Add this test after `bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded`:

```rust
#[test]
fn bb90_hard_syndrome_release_profile_is_counter_bounded() {
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
    let bounds = LdpcCsCounterBounds::from_ldpc_diagnostic(&diagnostic, PROFILE_CANDIDATE_LIMIT);

    println!(
        "{}",
        serde_json::to_string_pretty(&profile_json(&fixture, &diagnostic, &profile, &bounds))
            .unwrap()
    );

    validate_ldpc_cs_counter_bounds(&profile, &diagnostic, &bounds).unwrap();
}
```

- [ ] **Step 3: Add the legacy negative control**

Add this test after the positive smoke:

```rust
#[test]
fn bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds() {
    let fixture = load_fixture();
    let computed = compute_fixture_replay(&fixture).unwrap();
    let ldpc_diagnostic = replay_syndrome_diagnostic_with_osd_variant(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();
    let legacy_diagnostic = replay_syndrome_diagnostic(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
    )
    .unwrap();
    let legacy_profile = profile_syndrome_replay_with_candidate_limit_for_basis(
        profile_replay_basis(fixture.basis),
        &computed.model,
        &computed.sampled_syndrome,
        fixture.max_bp_iterations,
        fixture.osd_order,
        PROFILE_CANDIDATE_LIMIT,
    )
    .unwrap();
    let bounds =
        LdpcCsCounterBounds::from_ldpc_diagnostic(&ldpc_diagnostic, PROFILE_CANDIDATE_LIMIT);

    let error =
        validate_ldpc_cs_counter_bounds(&legacy_profile, &legacy_diagnostic, &bounds).unwrap_err();

    assert!(
        error.contains("planned_candidate_count"),
        "error should name the violating counter: {error}"
    );
}
```

- [ ] **Step 4: Run the positive test red**

Run:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_release_profile_is_counter_bounded -- --nocapture
```

Expected: FAIL because `LdpcCsCounterBounds`, `profile_json`, and `validate_ldpc_cs_counter_bounds` do not exist yet.

### Task 2: Implement The Counter-Bound Validator And JSON Profile

**Files:**
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Consumes: failing tests from Task 1.
- Produces: `LdpcCsCounterBounds`, `profile_json`, and `validate_ldpc_cs_counter_bounds`.

- [ ] **Step 1: Add the bounds type and JSON helper**

Add these helpers before `load_fixture`:

```rust
struct LdpcCsCounterBounds {
    planned_candidate_count: u128,
    candidate_limit: usize,
}

impl LdpcCsCounterBounds {
    fn from_ldpc_diagnostic(
        diagnostic: &SyndromeReplayDiagnostic,
        candidate_limit: usize,
    ) -> Self {
        Self {
            planned_candidate_count: diagnostic.planned_candidate_count,
            candidate_limit,
        }
    }
}

fn profile_json(
    fixture: &HardSyndromeFixture,
    diagnostic: &SyndromeReplayDiagnostic,
    profile: &BbCircuitBposdProfile,
    bounds: &LdpcCsCounterBounds,
) -> serde_json::Value {
    serde_json::json!({
        "case_id": fixture.case_id,
        "basis": format!("{:?}", fixture.basis),
        "osd_planner": diagnostic.osd_planner,
        "osd_order": diagnostic.osd_order,
        "candidate_limit": bounds.candidate_limit,
        "planned_candidate_count": diagnostic.planned_candidate_count,
        "ldpc_cs_candidate_bound": bounds.planned_candidate_count,
        "decode_seconds": profile.decode_seconds,
        "bp_seconds": profile.bp_seconds,
        "osd_seconds": profile.osd_seconds,
        "decode_call_count": profile.decode_call_count,
        "z_decode_call_count": profile.z_decode_call_count,
        "x_decode_call_count": profile.x_decode_call_count,
        "bp_iteration_count": profile.bp_iteration_count,
        "osd_use_count": profile.osd_use_count,
        "osd_candidate_count": profile.osd_candidate_count,
        "gf2_solve_count": profile.gf2_solve_count,
        "gf2_full_elimination_count": profile.gf2_full_elimination_count,
    })
}
```

- [ ] **Step 2: Add finite timing validation**

Add this helper after `profile_json`:

```rust
fn expect_finite_nonnegative_seconds(label: &str, seconds: f64) -> Result<(), String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!("{label} must be finite and non-negative, got {seconds}"));
    }
    Ok(())
}
```

- [ ] **Step 3: Add the counter-bound validator**

Add this helper after `expect_finite_nonnegative_seconds`:

```rust
fn validate_ldpc_cs_counter_bounds(
    profile: &BbCircuitBposdProfile,
    diagnostic: &SyndromeReplayDiagnostic,
    bounds: &LdpcCsCounterBounds,
) -> Result<(), String> {
    expect_finite_nonnegative_seconds("decode_seconds", profile.decode_seconds)?;
    expect_finite_nonnegative_seconds("bp_seconds", profile.bp_seconds)?;
    expect_finite_nonnegative_seconds("osd_seconds", profile.osd_seconds)?;

    if diagnostic.planned_candidate_count > bounds.planned_candidate_count {
        return Err(format!(
            "planned_candidate_count violates ldpc_cs bound: expected <= {}, got {}",
            bounds.planned_candidate_count, diagnostic.planned_candidate_count
        ));
    }
    if diagnostic.osd_planner != "ldpc_osd_cs" {
        return Err(format!(
            "osd_planner violates ldpc_cs bound: expected ldpc_osd_cs, got {}",
            diagnostic.osd_planner
        ));
    }

    let planned_bound = usize::try_from(bounds.planned_candidate_count).unwrap_or(usize::MAX);
    let candidate_bound = bounds.candidate_limit.min(planned_bound);
    if profile.osd_candidate_count == 0 {
        return Err("osd_candidate_count violates ldpc_cs bound: expected > 0, got 0".into());
    }
    if profile.osd_candidate_count > candidate_bound {
        return Err(format!(
            "osd_candidate_count violates ldpc_cs bound: expected <= {candidate_bound}, got {}",
            profile.osd_candidate_count
        ));
    }
    if profile.gf2_solve_count != 1 {
        return Err(format!(
            "gf2_solve_count violates optimized OSD bound: expected 1, got {}",
            profile.gf2_solve_count
        ));
    }
    if profile.gf2_full_elimination_count != 1 {
        return Err(format!(
            "gf2_full_elimination_count violates GF(2) elimination bound: expected 1, got {}",
            profile.gf2_full_elimination_count
        ));
    }
    if profile.decode_call_count != profile.z_decode_call_count + profile.x_decode_call_count {
        return Err(format!(
            "decode_call_count violates basis sum bound: expected {} + {} = {}, got {}",
            profile.z_decode_call_count,
            profile.x_decode_call_count,
            profile.z_decode_call_count + profile.x_decode_call_count,
            profile.decode_call_count
        ));
    }
    Ok(())
}
```

- [ ] **Step 4: Run issue tests green**

Run:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_release_profile_is_counter_bounded -- --nocapture
cargo test --release -p rsinter bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds -q
```

Expected: both PASS. The positive test prints JSON containing `decode_seconds`, `bp_seconds`, `osd_seconds`, `osd_candidate_count`, `gf2_solve_count`, and `gf2_full_elimination_count`. The negative test fails validation with `planned_candidate_count` in the error.

### Task 3: Document The Release Smoke Command

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: issue test names from Tasks 1 and 2.
- Produces: reviewer-readable README section with positive and negative commands.

- [ ] **Step 1: Add a counter smoke subsection**

Append this subsection after the existing `BB90 Hard-Syndrome Replay` section:

````markdown
### Counter-Bounded Release Smoke

The hard-syndrome performance smoke is intentionally counter-gated rather than
wall-clock-gated. Run the positive release profile with:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_release_profile_is_counter_bounded -- --nocapture
```

The test prints profile JSON with `decode_seconds`, `bp_seconds`,
`osd_seconds`, `osd_candidate_count`, `gf2_solve_count`, and
`gf2_full_elimination_count`. The timing fields are evidence only; the pass/fail
checks assert that the BB90 fixture uses the bounded `ldpc_osd_cs` candidate
plan, one GF(2) full elimination, one optimized GF(2) solve, and consistent
per-basis decode-call counters.

The legacy exhaustive/frontier negative control is:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds -q
```

It verifies that the same validator rejects the legacy profile and names the
violating counter.
````

- [ ] **Step 2: Verify docs diff**

Run:

```bash
git diff -- benchmarks/bb_circuit_bposd_compare/README.md
```

Expected: the README only gains the counter-bounded release smoke subsection.

### Task 4: Final Verification And Cleanup

**Files:**
- All touched files.

**Interfaces:**
- Consumes: Tasks 1 through 3.
- Produces: formatted, verified PR-ready branch.

- [ ] **Step 1: Format/check touched Rust file**

Run:

```bash
rustfmt --edition 2024 rsinter/tests/bb90_hard_syndrome_fixture.rs --check
```

Expected: PASS. If it fails, run the same command without `--check`, review the diff, and rerun with `--check`.

- [ ] **Step 2: Run issue-required verification**

Run:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_release_profile_is_counter_bounded -- --nocapture
cargo test --release -p rsinter bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds -q
```

Expected: both PASS.

- [ ] **Step 3: Run required broader verification**

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

Expected: no whitespace errors and changes limited to the Superpowers artifacts, BB90 fixture tests, and BB compare README.

## Self-Review

- Spec coverage: the plan covers the positive release smoke, structured profile JSON, `ldpc_cs` candidate bound, one GF(2) full elimination, optimized solve count, decode-call sum, timing-as-evidence-only rule, legacy negative control, and README documentation.
- Placeholder scan: no placeholder instructions remain.
- Type consistency: `LdpcCsCounterBounds`, `profile_json`, and `validate_ldpc_cs_counter_bounds` are named consistently across all tasks.

## Execution Choice

Standing answer policy selects **Subagent-Driven (recommended)** because it is the recommended option in the writing-plans handoff. This Agent Desk run will use `superpowers:subagent-driven-development`; if a subagent cannot complete a task because of tool constraints, the controller will resolve the smallest necessary follow-up inline and record the deviation in the final decision log.
