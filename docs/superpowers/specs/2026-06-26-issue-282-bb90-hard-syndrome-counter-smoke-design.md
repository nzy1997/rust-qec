# Issue 282 BB90 Hard-Syndrome Counter Smoke Design

Issue: #282 Add a BB90 hard-syndrome performance smoke based on counters

Date: 2026-06-26

## Context

Issues #279, #280, and #281 are merged on this branch. The BB90 hard-syndrome
fixture can be replayed through the explicit `ldpc_osd_cs` planner, the replay
profile exposes structured BP/OSD/GF(2) counters, and OSD candidate evaluation
now uses precomputed influence vectors so `gf2_solve_count` no longer scales
with evaluated candidates.

The open gap is a cheap release-mode smoke that proves those intended paths are
still used on the hard BB fixture. It must not rely on wall-clock thresholds:
timing is useful reviewer evidence, but the pass/fail gate should be counter
based.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming gates use
the standing answer policy:

- No visual companion is needed because the work is release test and decoder
  counter behavior, not visual design.
- The design is approved from the issue text, the merged #279/#280/#281
  contracts, and #209 profiling comments.
- Keep the smoke in `rsinter/tests/bb90_hard_syndrome_fixture.rs` because that
  file already owns the hard fixture replay/profile assertions.
- Add a small validator helper in the test file rather than a public production
  API. The validator is test-only evidence and does not need to widen the
  `rsinter` public surface.
- Document the release smoke command in
  `benchmarks/bb_circuit_bposd_compare/README.md` so reviewers can compare the
  printed JSON with the #209 comments.

## Approaches Considered

1. Add focused integration tests in the existing BB90 fixture test file, with a
   local `ldpc_cs` counter-bound validator and a legacy negative control.
   This is recommended because it directly exercises the same fixture and helper
   paths added by #279/#280/#281 while avoiding public API churn.
2. Add a new `rsinter` CLI subcommand for the smoke profile.
   This would make manual execution explicit, but it adds user-facing CLI
   surface for one regression test and is unnecessary for the issue.
3. Extend `verify_replay.py` to enforce the bounds on CSV replay rows.
   This is useful later for comparison artifacts, but it depends on the Python
   compare workflow and the issue asks for a cheap Rust release-mode smoke.

## Design

The positive smoke test will be named
`bb90_hard_syndrome_release_profile_is_counter_bounded`. It will load the
checked-in BB90 hard-syndrome fixture, replay the sampled Z-basis syndrome
through `OsdVariant::LdpcCombinationSweep`, and collect a bounded profile with
`PROFILE_CANDIDATE_LIMIT = 16`.

The test will print one structured JSON object containing at least:

- `decode_seconds`
- `bp_seconds`
- `osd_seconds`
- `osd_candidate_count`
- `gf2_solve_count`
- `gf2_full_elimination_count`
- `decode_call_count`
- `z_decode_call_count`
- `x_decode_call_count`
- `planned_candidate_count`

The validator will assert:

- `osd_candidate_count` is nonzero and no larger than both the configured
  profile candidate limit and the `ldpc_osd_cs` diagnostic planned candidate
  count.
- `gf2_solve_count == 1`.
- `gf2_full_elimination_count == 1`.
- `decode_call_count == z_decode_call_count + x_decode_call_count`.
- the diagnostic planner is exactly `ldpc_osd_cs`.

The negative control will be named
`bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds`. It will profile the
same fixture through the legacy helper, run that profile through the same
`ldpc_cs` validator, and assert that validation fails with an error naming the
violating counter. The expected violation is the legacy exhaustive/frontier
planner count: the legacy diagnostic still reports `planned_candidate_count =
26332`, while the `ldpc_cs` bound is `free_column_count + C(7, 2)`.

Timing fields remain printed evidence only. The tests will assert finiteness and
non-negativity for timing fields, but they will not compare them against a
machine-dependent threshold.

## Testing

Use TDD:

1. Add the two issue-named tests and the local validator/profile JSON helpers in
   `rsinter/tests/bb90_hard_syndrome_fixture.rs`.
2. Run the positive test and confirm it fails before the validator/helper code is
   complete.
3. Implement the validator and JSON profile output.
4. Run:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_release_profile_is_counter_bounded -- --nocapture
cargo test --release -p rsinter bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds -q
cargo test
```

Out of scope: full campaign execution, CI-wide timing gates, changing the BB90
fixture, and replacing the existing compare smoke.
