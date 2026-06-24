# Task 1 Report: P=192 Registry Support and Acceptance Coverage

## What Changed

- Registered `apm_kasai:p=192` in `qec-code/src/codes/built_in_css.rs`.
- Added the pinned Table A1 P=192 affine coefficients:
  - `f`: `(71, 127)`, `(97, 80)`, `(67, 117)`, `(163, 165)`, `(25, 60)`, `(187, 33)`
  - `g`: `(163, 165)`, `(55, 183)`, `(167, 79)`, `(139, 41)`, `(109, 78)`, `(31, 27)`
- Generalized the APM Kasai registry helper so P=96 and P=192 share the same manifest-entry construction path.
- Updated unsupported `apm_kasai:p=<value>` diagnostics to list `supported: 96, 192` with note `available Table A1 APM-CSS instances`.
- Added P=192 structural acceptance coverage through the built-in registry entrypoint.
- Added a crate-private coefficient-level P=192 negative control in `qec-code/src/codes/apm.rs` because the APM builder module is `pub(crate)` and should not be made public solely for integration tests.
- Updated CLI coverage so `code css list` and sparse-row export accept both `apm_kasai:p=96` and `apm_kasai:p=192`.

## TDD RED

Command:

```sh
cargo test -p qec-code apm_p192_builds_paper_stats -q
```

Outcome: failed as expected before implementation.

Summary:

- The test compiled and ran.
- `apm_p192_builds_paper_stats` failed at the catalog assertion.
- Failure reason: `built_in_css_catalog()` listed `apm_kasai:p=96` but not `apm_kasai:p=192`.
- This was the expected missing-registry failure, not a compile error or malformed test failure.

## GREEN / Focused Tests

Commands and outcomes after implementation:

```sh
cargo test -p qec-code apm_p192_builds_paper_stats -q
```

Passed: 1 test passed.

```sh
cargo test -p qec-code --test cli apm_kasai_css_export -q
```

Passed: 1 test passed.

```sh
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs -q
```

Passed: 1 test passed.

```sh
cargo test -p qec-code --test cli code_css_list_output_matches_catalog_width -q
```

Ran successfully but matched 0 tests in this branch. The actual exact catalog-output test is named `run_code_css_list_returns_catalog_without_newline`, so I also ran:

```sh
cargo test -p qec-code --test cli run_code_css_list_returns_catalog_without_newline -q
```

Passed: 1 test passed.

Additional private coefficient-level negative-control check:

```sh
cargo test -p qec-code apm_p192_verifier_rejects_one_p96_affine_coefficient -q
```

Passed: 1 test passed.

## Full Test

Command:

```sh
cargo test
```

Outcome: passed for the full workspace with exit code 0.

Observed non-failing warnings:

- Existing `rmatching/tests/coverage.rs` warnings about `saw_same_tree` being assigned/read unused.

## Files Changed

- `qec-code/src/codes/built_in_css.rs`
- `qec-code/src/codes/apm.rs`
- `qec-code/tests/code.rs`
- `qec-code/tests/cli.rs`
- `.agent-desk/sdd/task-1-report.md`

## Self-Review Findings / Concerns

- No functional concerns found.
- The coefficient-level negative control is intentionally in `apm.rs` tests because `qec_code::codes::apm` is crate-private to integration tests.
- The integration P=192 negative control remains structural support mutation coverage through the public built-in entrypoint.
- No P=192 fixtures or decoder benchmarks were added, matching the task scope.
- I accidentally ran workspace-wide `cargo fmt`; unrelated formatting diffs were restored before commit. Final `git status` after commit shows only the pre-existing untracked `.agent-desk/sdd/task-1-brief.md`.
