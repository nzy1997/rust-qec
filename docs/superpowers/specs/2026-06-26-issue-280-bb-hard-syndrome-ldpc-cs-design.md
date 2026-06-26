# Issue 280 BB hard-syndrome ldpc_cs routing design

Issue: #280 Route BB hard-syndrome decoding through the ldpc_cs candidate planner

Date: 2026-06-26

## Context

Issues #277 and #278 are merged. `rbposd` now exposes
`OsdVariant::LdpcCombinationSweep`, reports `osd_planner = "ldpc_osd_cs"` in
diagnostics, and scores that mode with channel-prior objective weights.

The BB90 hard-syndrome fixture in `rsinter` still exercises replay diagnostics
and bounded profiles through the legacy default: setting only `osd_order = 7`
maps to the Rust frontier planner. That path reports the stored legacy
`planned_candidate_count = 26332` from a 16-column exhaustive frontier, not the
upstream-compatible `ldpc` OSD-CS plan.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming gates use
the standing answer policy:

- No visual companion is needed because the work is API and decoder diagnostic
  behavior, not visual design.
- The design is approved from the issue text and the merged #277/#278 contracts.
- Preserve the existing legacy default helpers, and add explicit `OsdVariant`
  helper variants for the BB hard-syndrome replay/profile path. This is the
  conservative choice because the issue requires a legacy negative control.
- Commit the Superpowers spec and plan artifacts because this repository tracks
  prior issue specs/plans under `docs/superpowers`.

## Approaches Considered

1. Add explicit `OsdVariant` parameters to the BB replay/profile helper layer,
   keep legacy wrappers unchanged, and route the new BB90 hard-syndrome test
   through `LdpcCombinationSweep`.
   This is recommended because it makes the selected planner observable while
   preserving the legacy fixture contract.
2. Change all BB replay/profile helpers to default to `LdpcCombinationSweep`
   whenever `osd_order > 0`.
   This would satisfy the bounded count but would erase the current legacy-mode
   helper surface and weaken the negative control.
3. Add only tests that call `rbposd` directly from `rsinter/tests`.
   This would prove the planner count, but it would not wire the `rsinter`
   helper path that BB diagnostics and comparison code use.

## Design

`rsinter::bb_circuit_memory::SyndromeReplayDiagnostic` will include the
`osd_planner` name reported by `rbposd`. Existing fields stay unchanged so the
fixture can continue to assert the legacy frontier count.

The existing helper functions remain compatibility wrappers:

- `replay_syndrome_diagnostic`
- `profile_syndrome_replay_for_basis`
- `profile_syndrome_replay_with_candidate_limit_for_basis`

Those wrappers keep constructing the default `rbposd` config, where
`osd_order > 0` maps to the legacy combination sweep. New explicit helpers will
accept an `rbposd::OsdVariant` and use it when constructing the replay decoder:

- `replay_syndrome_diagnostic_with_osd_variant`
- `profile_syndrome_replay_for_basis_with_osd_variant`
- `profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant`

The BB90 hard-syndrome positive control will call the new diagnostic helper
with `OsdVariant::LdpcCombinationSweep`. It will assert:

- `osd_planner == "ldpc_osd_cs"`;
- `osd_order == 7`;
- `free_column_count > 0`;
- `candidate_search_frontier_size == 7`;
- `max_candidate_order == 2`;
- `planned_candidate_count == free_column_count + C(7, 2)`;
- the bounded count is far below the stored legacy `26332`.

The legacy negative control will call the existing helper and assert it still
reports `legacy_combination_sweep` and the stored `26332` count. It will compare
that output against the explicit `ldpc` output so future refactors cannot rename
the old frontier as `ldpc_cs`.

## Testing

Use TDD:

1. Add the two issue-named tests in
   `rsinter/tests/bb90_hard_syndrome_fixture.rs`.
2. Run the issue positive test and confirm it fails before production code is
   added because the explicit helper functions and `osd_planner` diagnostic
   field do not exist yet.
3. Implement the explicit helper variants and diagnostic field.
4. Run:

```bash
cargo test -p rsinter bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded -- --nocapture
cargo test -p rsinter bb90_hard_syndrome_legacy_osd_plan_still_reports_exhaustive_frontier -q
cargo test
```

Out of scope: influence-vector optimization, Monte Carlo trials, and changing
the sampled hard-syndrome fixture.
