# Issue 234 Random-Window Ladder Evidence Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add smoke, full, and negative-control evidence tests for the issue-225 random-window CSS distance ladder.

**Architecture:** Extend `qec-code/tests/distance_bound.rs` with test-only helpers that load the existing manifest, generate each built-in CSS code, run the selected upper-bound method with issue-225 options, and validate results through `verify_issue_225_ladder_case`. The smoke test filters `tier = "smoke"`, the full test runs every manifest row under `#[ignore]`, and the negative control runs the current randomized baseline on `surface_rotated_d5`.

**Tech Stack:** Rust 2024, qec-code integration tests, existing `Issue225LadderCase`, `random_window_css_upper_bound`, `randomized_css_upper_bound`, and `verify_issue_225_ladder_case`.

## Global Constraints

- Use `qec-code/tests/fixtures/distance/issue_225_ladder.json` as the source of ladder cases.
- Use the manifest `tier` field for smoke versus full-only selection.
- Smoke cases are `surface_rotated_d5`, `toric_d5`, and `bb72`.
- Full acceptance covers all eight issue-225 cases.
- Random-window ladder runs use `iterations = 5000`, `restarts = 8`, fixed `seed = 7`, and `target_weight = expected_upper_bound`.
- Every random-window result must satisfy `upper_bound <= expected_upper_bound`.
- Every random-window witness must pass full stabilizer validation through the ladder verifier.
- Full acceptance must print case ID, expected upper bound, observed upper bound, method, seed, and elapsed time.
- No full-ladder case may exceed the issue-225 300 s per-case cap.
- Negative control must show `randomized-upper-bound` is rejected for `surface_rotated_d5` against expected upper bound `5`.
- Do not require external `codeDistancePYPI`, Gurobi, or `dist-m4ri`.
- Do not claim exact distance certification; methods report upper bounds.

---

## File Structure

- Modify `qec-code/tests/distance_bound.rs`: add ladder-run helpers, smoke/full random-window tests, and the randomized baseline negative control.
- No production files are changed.

### Task 1: Add Issue-225 Ladder Evidence Tests

**Files:**
- Modify: `qec-code/tests/distance_bound.rs`

**Interfaces:**
- Consumes:
  - `fn issue_225_ladder_cases() -> Vec<Issue225LadderCase>`
  - `fn issue_225_case(case_id: &str) -> Issue225LadderCase`
  - `fn css_from_built_in_code_id(code_id: &str) -> CssCode`
  - `random_window_css_upper_bound(css: &CssCode, options: RandomWindowUpperBoundOptions) -> Result<DistanceBoundResult<RandomWindowUpperBoundOptions>>`
  - `randomized_css_upper_bound(css: &CssCode, options: RandomizedUpperBoundOptions) -> Result<DistanceBoundResult<RandomizedUpperBoundOptions>>`
  - `verify_issue_225_ladder_case(case, result, css, expected_method) -> Result<()>`
- Produces:
  - `issue_225_random_window_upper_bound_smoke_ladder`
  - `issue_225_random_window_upper_bound_full_ladder`
  - `issue_225_current_randomized_upper_bound_ladder_negative_control`

- [ ] **Step 1: Write the failing evidence tests**

Add these imports at the top of `qec-code/tests/distance_bound.rs`:

```rust
use std::time::{Duration, Instant};
```

Add these helpers after `pinned_random_window_options()`:

```rust
const ISSUE_225_LADDER_SEED: u64 = 7;
const ISSUE_225_LADDER_ITERATIONS: usize = 5000;
const ISSUE_225_LADDER_RESTARTS: usize = 8;
const ISSUE_225_PER_CASE_CAP: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct Issue225LadderRunSummary {
    case_id: String,
    expected_upper_bound: usize,
    observed_upper_bound: usize,
    method: &'static str,
    seed: u64,
    elapsed: Duration,
}

fn issue_225_ladder_cases_for_tier(tier: &str) -> Vec<Issue225LadderCase> {
    issue_225_ladder_cases()
        .into_iter()
        .filter(|case| case.tier == tier)
        .collect()
}

fn issue_225_random_window_options(case: &Issue225LadderCase) -> RandomWindowUpperBoundOptions {
    RandomWindowUpperBoundOptions {
        iterations: ISSUE_225_LADDER_ITERATIONS,
        restarts: ISSUE_225_LADDER_RESTARTS,
        seed: ISSUE_225_LADDER_SEED,
        target_weight: Some(case.target_weight),
    }
}

fn run_issue_225_random_window_ladder_case(
    case: &Issue225LadderCase,
) -> Issue225LadderRunSummary {
    let css = css_from_built_in_code_id(&case.code_id);
    let start = Instant::now();
    let result = random_window_css_upper_bound(&css, issue_225_random_window_options(case))
        .unwrap_or_else(|error| panic!("{} random-window run failed: {error}", case.case_id));
    let elapsed = start.elapsed();

    verify_issue_225_ladder_case(
        case,
        &result,
        &css,
        DistanceBoundMethod::RandomWindowUpperBound,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(
        elapsed <= ISSUE_225_PER_CASE_CAP,
        "{} exceeded {:?} cap with {:?}",
        case.case_id,
        ISSUE_225_PER_CASE_CAP,
        elapsed
    );

    Issue225LadderRunSummary {
        case_id: case.case_id.clone(),
        expected_upper_bound: case.expected_upper_bound,
        observed_upper_bound: result.upper_bound,
        method: result.method.label(),
        seed: ISSUE_225_LADDER_SEED,
        elapsed,
    }
}

fn print_issue_225_ladder_table(summaries: &[Issue225LadderRunSummary]) {
    println!("case_id expected observed method seed elapsed_ms");
    for summary in summaries {
        println!(
            "{} {} {} {} {} {}",
            summary.case_id,
            summary.expected_upper_bound,
            summary.observed_upper_bound,
            summary.method,
            summary.seed,
            summary.elapsed.as_millis()
        );
    }
}
```

Add these tests after the existing issue-225 ladder verifier tests:

```rust
#[test]
fn issue_225_random_window_upper_bound_smoke_ladder() {
    let cases = issue_225_ladder_cases_for_tier("smoke");
    let case_ids = cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        case_ids,
        ["surface_rotated_d5", "toric_d5", "bb72"],
        "smoke ladder case selection changed"
    );

    let summaries = cases
        .iter()
        .map(run_issue_225_random_window_ladder_case)
        .collect::<Vec<_>>();

    print_issue_225_ladder_table(&summaries);
}

#[test]
#[ignore = "full issue-225 random-window ladder acceptance run"]
fn issue_225_random_window_upper_bound_full_ladder() {
    let cases = issue_225_ladder_cases();
    assert_eq!(cases.len(), 8, "full ladder should include all issue-225 cases");

    let summaries = cases
        .iter()
        .map(run_issue_225_random_window_ladder_case)
        .collect::<Vec<_>>();

    print_issue_225_ladder_table(&summaries);
}

#[test]
fn issue_225_current_randomized_upper_bound_ladder_negative_control() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);
    let result = randomized_css_upper_bound(
        &css,
        RandomizedUpperBoundOptions {
            iterations: ISSUE_225_LADDER_ITERATIONS,
            restarts: ISSUE_225_LADDER_RESTARTS,
            seed: ISSUE_225_LADDER_SEED,
            target_weight: Some(case.target_weight),
        },
    )
    .unwrap();

    assert_eq!(result.method, DistanceBoundMethod::RandomizedUpperBound);
    assert_eq!(
        result.upper_bound, 7,
        "{} randomized baseline should reproduce the known loose issue-225 result",
        case.case_id
    );
    assert!(
        result.upper_bound > case.expected_upper_bound,
        "{} randomized baseline must be loose for the negative control",
        case.case_id
    );

    let error = verify_issue_225_ladder_case(
        &case,
        &result,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected randomized baseline rejection");

    assert_eq!(
        error,
        QecError::DistanceBoundValidationFailed(
            "surface_rotated_d5 expected upper_bound <= 5, got 7".to_owned(),
        )
    );
}
```

- [ ] **Step 2: Run the focused smoke command and verify RED**

Run:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture
```

Expected before implementation: FAIL to compile because the new test helpers and tests do not exist yet.

- [ ] **Step 3: Implement the tests**

Keep the code from Step 1 in `qec-code/tests/distance_bound.rs`. If the focused run exposes ordering or formatting issues, fix only the test helper code. Do not change production code or the manifest.

- [ ] **Step 4: Run targeted verification**

Run:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture
cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture
cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q
```

Expected: smoke covers `surface_rotated_d5`, `toric_d5`, and `bb72`; full covers all eight cases; negative control observes randomized upper bound `7` and passes because the verifier rejects it.

- [ ] **Step 5: Run broad verification and commit**

Run:

```bash
cargo test
```

Expected: PASS.

Commit:

```bash
git add docs/superpowers/specs/2026-06-25-issue-234-random-window-ladder-evidence-tests-design.md docs/superpowers/plans/2026-06-25-issue-234-random-window-ladder-evidence-tests.md qec-code/tests/distance_bound.rs
git commit -m "test: add issue-225 random-window ladder evidence"
```
