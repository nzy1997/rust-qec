# Issue 234 Random-Window Ladder Evidence Design

## Context

Issue #234 closes the evidence loop for issue #225. The repository already has the issue-225 ladder manifest, a method-aware ladder verifier, and the `random-window-upper-bound` library/CLI method. This change adds tests that run the new method against that manifest and keeps the current `randomized-upper-bound` sampler as an explicit negative control.

## Approaches Considered

1. Add the ladder evidence in `qec-code/tests/distance_bound.rs`.
   - Pros: reuses the existing manifest helpers, CSS constructors, verifier imports, and distance-bound test context.
   - Cons: the file grows larger, but the added tests exercise the same module surface as the existing verifier and method tests.

2. Add a new integration test file just for issue-225 evidence.
   - Pros: isolates acceptance evidence from lower-level distance-bound tests.
   - Cons: duplicates helper setup or requires moving private test helpers into support modules for a small, related addition.

3. Drive the ladder through the CLI only.
   - Pros: closest to a user invocation.
   - Cons: slower and less direct for witness validation, and the library verifier already validates the exact result structure this issue targets.

Chosen approach: add the evidence tests to `qec-code/tests/distance_bound.rs`. This keeps the change scoped to test evidence while using the production random-window function and the existing verifier directly.

## Design

Add shared test helpers that load issue-225 manifest rows, build CSS codes from each row's `code_id`, run `random_window_css_upper_bound` with pinned issue-225 options, and validate each result through `verify_issue_225_ladder_case` with expected method `RandomWindowUpperBound`.

The smoke test selects rows by `tier == "smoke"`, so it follows the manifest rather than duplicating the subset. The full test iterates all manifest rows and is marked ignored with the exact command in the ignore reason. Both tests print compact evidence rows under `-- --nocapture`, including case ID, expected upper bound, observed upper bound, method, seed, and elapsed time.

The negative control runs the existing `randomized_css_upper_bound` baseline on `surface_rotated_d5` with the issue-225 options and `target_weight = 5`. It passes only if the baseline result is loose and the ladder verifier rejects it against the expected upper bound.

## Error Handling

Every assertion that can fail for a case includes the case ID. Random-window ladder failures report the verifier error with the case prefix. The full test also checks that no individual case exceeds the 300 second issue-225 cap and reports the case ID if that happens.

## Testing

Required focused commands:

- `cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture`
- `cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture`
- `cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q`

Final repository verification also runs `cargo test`, using offline mode when the sandbox cannot reach crates.io.
