# Issue 234 Random-Window Ladder Evidence Tests Design

Date: 2026-06-25
Status: Accepted automatically under the Agent Desk Standing Answer Policy
Scope: `qec-code` evidence tests for the issue-225 CSS distance ladder

## Summary

Issue #234 adds integration evidence that the new
`random-window-upper-bound` method recovers the issue-225 ladder targets while
still producing validated logical witnesses. The work is test-only: it reuses
the checked-in ladder manifest, the method-aware ladder verifier, and the
random-window library/CLI method already merged by the prerequisite issues.

The default test tier runs the manifest smoke cases. The full acceptance tier
is an ignored test that covers all eight ladder cases and prints a compact
table for manual acceptance runs. A negative-control test keeps the current
`randomized-upper-bound` sampler visible by proving that its deterministic
surface `d=5` result is rejected by the ladder verifier.

## Context

The dependency chain is already merged on `master`:

- #228 / PR #241 added `qec-code/tests/fixtures/distance/issue_225_ladder.json`.
- #229 / PR #254 added `verify_issue_225_ladder_case`.
- #232 / PR #261 added `random_window_css_upper_bound`.
- #233 / PR #266 exposed the CLI command and kept `randomized-upper-bound`.

Issue #225 documents the expected ladder targets and a loose baseline result:
`surface_rotated:d=5` with the existing randomized sampler returns an upper
bound of `7` under the issue-225 options while the ladder target is `5`.

## Approaches Considered

### 1. Library-level integration tests in `qec-code/tests/distance_bound.rs`

This is the selected approach. It avoids subprocess overhead for every full
ladder case, can pass each `DistanceBoundResult` directly into
`verify_issue_225_ladder_case`, and fits the existing tests for the manifest,
verifier, and random-window library path.

### 2. CLI-driven tests for every ladder case

This would exercise the public command path, but PR #266 already covers the CLI
contract. Running eight subprocesses in the full ladder adds noise without
improving witness validation because the verifier consumes typed results.

### 3. Replay stored JSON outputs

Replay would be fast, but it would not prove the current random-window method
still finds the witnesses. It is appropriate only as a fallback for the
negative control; the selected design runs the deterministic baseline directly.

## Design

Add test helpers to `qec-code/tests/distance_bound.rs`:

- `issue_225_ladder_cases_for_tier(tier)` filters the manifest by `tier`.
- `run_issue_225_random_window_ladder_case(case, seed)` builds the CSS code,
  runs `random_window_css_upper_bound` with `iterations = 5000`,
  `restarts = 8`, `seed = 7`, and `target_weight = case.target_weight`, then
  verifies the result with `expected_method = RandomWindowUpperBound`.
- A small summary struct records case ID, expected upper bound, observed upper
  bound, method label, seed, and elapsed time for printing.

The smoke test uses manifest rows with `tier = "smoke"`, which are
`surface_rotated_d5`, `toric_d5`, and `bb72`. The full test runs every manifest
row and is marked `#[ignore]`. Both tests print checked case IDs when run with
`-- --nocapture`, and all assertions include the case ID so captured-output
failures still identify the failing case.

The negative-control test runs `randomized_css_upper_bound` for
`surface_rotated_d5` with the issue-225 options and asserts that the observed
result is loose (`7`, and generally greater than `5`). It passes only when
`verify_issue_225_ladder_case` rejects the result against the manifest target.

## Error Handling

Test failure messages must name the manifest case ID. Ladder verifier errors
already prefix case IDs for validation failures, so the helper wraps algorithm
errors and unexpected missing cases with explicit case IDs as well.

The full acceptance helper measures elapsed time and asserts every case is
below the issue-225 300 s cap. This cap is enforced in the test so a run cannot
silently pass while violating the issue's acceptance boundary.

## Testing

Add focused tests in `qec-code/tests/distance_bound.rs`:

- `issue_225_random_window_upper_bound_smoke_ladder`
- `issue_225_random_window_upper_bound_full_ladder` with `#[ignore]`
- `issue_225_current_randomized_upper_bound_ladder_negative_control`

Run the issue verification commands:

- `cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture`
- `cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture`
- `cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q`

Also run `cargo test` before opening the PR.

## Out Of Scope

Do not require external `codeDistancePYPI`, Gurobi, or `dist-m4ri`. Do not
claim exact distance certification. Do not change the manifest, CLI surface, or
random-window algorithm behavior.
