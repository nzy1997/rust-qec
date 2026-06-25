# Issue 229 CSS Upper-Bound Ladder Verifier Design

Date: 2026-06-25
Status: Accepted automatically under the Agent Desk Standing Answer Policy
Scope: `qec-code` reusable verifier support for the issue-225 CSS distance ladder

## Summary

Issue #229 adds a reusable, method-aware verifier for completed CSS distance
upper-bound results. The verifier checks a `DistanceBoundResult`, the matching
`CssCode`, and one manifest row from the issue-225 ladder. It accepts only
validated logical witnesses whose method label, bound type, witness weight, and
upper-bound value satisfy the manifest target.

This PR does not implement random-window search or change the CLI. It preserves
the existing loose `randomized-upper-bound` command as normal production
behavior, while letting tests reject that method when it is judged against the
issue-225 acceptance ladder.

## Context

Issue #228 added `qec-code/tests/fixtures/distance/issue_225_ladder.json` with
eight ladder cases. Issue #225 documents that the current
`randomized-upper-bound` sampler returns valid but loose witnesses, including
`surface_rotated:d=5` returning `7` where the ladder target is `5`.

`qec-code/src/distance_bound.rs` already owns the shared distance-bound result
types and `validate_randomized_upper_bound_result`, which validates witness
shape, non-identity, stabilizer commutation, stabilizer row-span exclusion,
logical class, and randomized method metadata.

## Design

Use a production helper in `qec-code/src/distance_bound.rs`, not test-only
support. Later issues can reuse the same method-aware validation for
`random-window-upper-bound` without copying witness checks into fixture tests.

Add:

- `Issue225LadderCase`: a deserializable manifest-row type with the fields from
  #228.
- `DistanceBoundValidationOptions`: a method-aware validation context carrying
  the expected method label and optional known exact distance.
- `validate_distance_bound_result`: a method-aware wrapper that validates common
  completed upper-bound invariants and delegates witness validation to the same
  path used by `validate_randomized_upper_bound_result`.
- `verify_issue_225_ladder_case`: the ladder-level check that includes case IDs
  in all errors and enforces `upper_bound <= expected_upper_bound`.

The existing `validate_randomized_upper_bound_result` remains available and is
rewired through the shared method-aware helper with
`expected_method = RandomizedUpperBound`.

## Alternatives Considered

### 1. Test-only verifier under `qec-code/tests/support/`

This is small, but it would duplicate production witness validation or force
future implementation issues to import test code. It also makes #231 and #234
more likely to copy checks.

### 2. Production verifier in `distance_bound.rs`

This keeps result validation next to the bound result types and reuses existing
witness checks. It is the chosen approach because it avoids method coupling and
keeps future methods behind the same API.

### 3. Extend only `validate_randomized_upper_bound_result`

This is too method-specific. The issue explicitly requires parameterizing by
expected method so future `random-window-upper-bound` checks do not undo the
current coupling.

## Error Contract

Verifier errors should be `QecError::DistanceBoundValidationFailed` strings.
Ladder-level failures must name the manifest `case_id`, for example:

- `surface_rotated_d5 expected upper_bound <= 5, got 7`
- `surface_rotated_d5 expected method random-window-upper-bound, got randomized-upper-bound`

Witness validation errors can keep the existing message text but should be
prefixed by the ladder case when reported through `verify_issue_225_ladder_case`.

## Testing

Add focused tests to `qec-code/tests/distance_bound.rs`:

- `issue_225_ladder_verifier_accepts_exact_upper_bounds_and_rejects_loose_bounds`
  builds a valid `surface_rotated_d5` witness of weight `5`, accepts it, then
  rejects a valid but loose weight-`7` witness with an error naming the case,
  expected `5`, and observed `7`.
- `issue_225_ladder_verifier_rejects_unvalidated_witness` rejects a stabilizer
  span witness and a mismatched serialized witness weight.
- `issue_225_ladder_verifier_rejects_wrong_method_label` rejects a valid
  witness wrapped in the wrong method label and names the expected method.

Run the required focused commands and then `cargo test`.

## Out Of Scope

Do not implement random-window search. Do not change the CLI. Do not make the
existing `randomized-upper-bound` command fail in normal use.
