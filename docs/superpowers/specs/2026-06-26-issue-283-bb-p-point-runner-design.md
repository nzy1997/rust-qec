# Issue 283 BB P-Point Runner Design

Issue: #283 Introduce a BB p-point runner that separates setup from the trial loop

Date: 2026-06-26

## Context

The BB circuit-memory Rust path already has the core building blocks needed for
a p-point runner: `build_code`, `build_syndrome_cycle`,
`build_effective_models`, `SimulationConfig`, and the BP/OSD profile fields in
`BbCircuitBposdProfile`. The current `run_simulation_for_code` implementation
does build code, cycle, effective models, and Z/X decoders before entering its
trial loop, but that separation is implicit. The profile does not count setup
construction phases, and sample/decode counts are not represented as separate
trial-loop counters.

Issue #282 is present on this branch through merge commit `634f37a`, so the
hard-syndrome counter work this issue depends on is available.

GitHub API access is unavailable in the Agent Desk sandbox, so this design uses
the issue body supplied by the manager as authoritative issue context.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming gates use
the standing answer policy:

- No visual companion is needed because the work is API/profile/test behavior,
  not visual design.
- The design is approved from the issue text and the existing
  `bb_circuit_memory` runner shape.
- Keep existing CLI behavior compatible by routing the current CLI and export
  functions through the new p-point runner instead of adding a new subcommand.
- Add public p-point config/result helpers in `rsinter::bb_circuit_memory`
  because the issue asks for an explicit BB p-point interface.
- Use additive profile fields rather than changing existing field meanings.
- Use a synthetic negative control by mutating a good profile into per-trial
  rebuild counts and feeding it through the production validator. This directly
  proves that the reported setup/model rebuild mismatch is rejected without
  adding a deliberately slow production code path.

## Approaches Considered

1. Add a first-class p-point runner that builds reusable setup once, runs all
   trials through it, and exposes additive setup/sample counters in the existing
   profile. This is recommended because it makes the intended boundary explicit
   while preserving the existing CLI and JSON export behavior.
2. Only add tests around the current `run_simulation_for_code` implementation.
   This is smaller, but it leaves the p-point interface implicit and does not
   satisfy the issue's requested input/output surface.
3. Add a new CLI command and benchmark CSV schema for p-point runs. This would
   make the mode highly visible, but it is unnecessary public surface for this
   issue and risks breaking existing comparison tooling.

## Design

Add `BbPPointConfig` with the explicit issue inputs:

- `code_id`
- `physical_error_rate`
- `num_cycles`
- `num_trials`
- `seed`
- `max_bp_iterations`
- `osd_order`
- `osd_variant`

The config will provide a small constructor from the existing
`SimulationConfig`, so current call sites can keep using `SimulationConfig` and
an optional OSD variant. Add `BbPPointResult` to pair `code_id` with the
existing `SimulationResult`.

Refactor the private simulation path into two phases:

1. Build one reusable p-point setup: BB code, syndrome cycle, effective models,
   decoder config, and Z/X BP/OSD decoders. Count this as one code build, one
   syndrome-cycle build, one effective-model build, and one decoder-set build.
2. Run the trial loop with that setup. Increment `sample_count` once per trial
   and keep the existing decode counters from BP/OSD stats, including separate
   Z and X decode call counts.

Extend `BbCircuitBposdProfile` additively with:

- `code_build_count`
- `syndrome_cycle_build_count`
- `effective_model_build_count`
- `decoder_build_count`
- `sample_count`

Existing timing and decode fields keep their meanings. Replay-only profile
helpers leave the setup/sample counters at zero because they do not build a
p-point or sample trials.

Add `validate_bb_p_point_result` to enforce the p-point accounting contract:

- setup/model/decoder construction counters are exactly one for a successful
  p-point result,
- `sample_count == num_trials`,
- `decode_call_count == z_decode_call_count + x_decode_call_count`.

The error for setup/model/decoder counter mismatches will contain
`setup/model rebuild count mismatch`, which gives the negative control a stable
failure string.

## Testing

Use TDD:

1. Add integration tests named
   `bb_p_point_runner_reuses_setup_across_trials` and
   `bb_p_point_runner_rejects_per_trial_setup_rebuild` in
   `rsinter/tests/bb_circuit_memory.rs`.
2. The positive test runs a small BB p-point with `num_trials = 8`, checks all
   setup construction counters are one, checks `sample_count == 8`, checks the
   Z/X/total decode counters, and validates the result.
3. The negative test starts from a valid p-point result, mutates the setup/model
   counters as if setup had been rebuilt per trial, and asserts that
   `validate_bb_p_point_result` rejects it with
   `setup/model rebuild count mismatch`.
4. Run the issue commands:

```bash
cargo test -p rsinter bb_p_point_runner_reuses_setup_across_trials -- --nocapture
cargo test -p rsinter bb_p_point_runner_rejects_per_trial_setup_rebuild -q
```

5. Run the wider required verification:

```bash
cargo test
```

Out of scope: full 50k-trial sweeps, plot/report generation, OSD semantic
changes, and benchmark CSV schema redesign.
