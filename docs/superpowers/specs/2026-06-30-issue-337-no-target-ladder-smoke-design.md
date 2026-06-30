# Issue 337 No-Target Ladder Smoke Design

Issue #337 adds a release-built, fixed-budget no-target profiling surface for
`qec-code code css-distance random-window-upper-bound`. The suite must extend
the BB-only no-target smoke path from #335 without replacing it.

## Context

The repository already has the random-window benchmark modules under
`benchmarks/qec_code_random_window/`:

- `validate_cases.py` validates the generic manifest schema.
- `run_local.py` executes `qec-code` and writes JSONL rows containing the
  command, elapsed wall time, build profile, target weight, status, upper bound,
  and raw CLI JSON.
- `summarize.py` writes CSV and Markdown summaries.
- The root `Makefile` already exposes smoke, full, and BB-only no-target smoke
  targets.

The issue-225 ladder fixture at
`qec-code/tests/fixtures/distance/issue_225_ladder.json` is the source for the
required case IDs, code IDs, and expected upper-bound provenance. The required
no-target ladder smoke cases are `surface_rotated_d5`, `toric_d5`, `bb72`, and
`bb144`.

## Chosen Approach

Add a sibling no-target ladder smoke manifest and Make target that reuse the
existing runner and summarizer. This keeps #335 intact and avoids adding another
execution path. The new target will:

1. Validate the manifest with the generic schema plus a no-target ladder
   contract.
2. Build `target/release/qec-code`.
3. Run the existing local runner with `--build-profile release` and no
   `--target-weight` option.
4. Write `local-runs.jsonl` and `summary/summary.csv` under
   `benchmarks/out/qec_code_random_window/no-target-ladder-smoke/`.

The manifest will use exact issue-225 case IDs so downstream profiling rows can
join directly against ladder provenance. Every case omits `target_weight`.
Budgets are deliberately smoke-sized: one seed, one restart, and 500 iterations
per case, matching the existing no-target BB smoke budget while adding the
surface and toric d5 cases.

## Validator Contract

Extend `validate_cases.py` with a reusable no-target ladder validation mode.
The mode keeps the existing generic checks and adds:

- all required case IDs are present;
- no case contains a `target_weight` field.

The Make target will call this mode before building and running. The focused
test will mutate a manifest copy in memory to prove both negative controls:
adding `target_weight` names the offending field, and removing `bb144` names
the missing case.

## Documentation

Update the benchmark README, showcase page, and Makefile help text to document
the new no-target ladder smoke target, its output directory, and its distinction
from the BB-only no-target smoke target.

## Tests

Add `benchmarks/qec_code_random_window/tests/test_no_target_ladder_suite.py`
with coverage for:

- manifest validation and required issue-225 provenance;
- Makefile target wiring, release build, no `--target-weight`, and output path;
- negative controls for target-weight and missing-case rejection;
- generated JSONL and summary artifacts when the Make target has run.

Run the issue verification commands plus `cargo test`.

## Risks And Limits

This issue does not optimize the random-window algorithm and does not certify
exact distance. Successful rows remain upper bounds. The generated-output test
checks artifacts when they exist; the Make target remains the authoritative
end-to-end producer of those artifacts.
