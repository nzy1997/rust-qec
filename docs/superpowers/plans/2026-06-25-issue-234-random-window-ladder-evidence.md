# Issue 234 Random-Window Ladder Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add issue-225 ladder evidence tests for `random-window-upper-bound`, including smoke, ignored full acceptance, and current-sampler negative-control coverage.

**Architecture:** Extend the existing `qec-code/tests/distance_bound.rs` integration test file because it already owns the random-window method tests, issue-225 manifest loader, CSS builders, and method-aware ladder verifier. The new helpers run production library methods and validate their returned witnesses through `verify_issue_225_ladder_case`. Reuse GF(2) reduced row-span forms inside the random-window candidate loop so the full ignored evidence command remains within the issue-225 per-case cap.

**Tech Stack:** Rust 2024, Cargo workspace, `qec-code` integration tests, existing issue-225 manifest JSON, existing distance-bound verifier.

## Global Constraints

- Use the issue-225 manifest `tier` field to choose smoke cases.
- Smoke cases are `surface_rotated_d5`, `toric_d5`, and `bb72`.
- Full acceptance covers all eight issue-225 manifest cases.
- Full acceptance is out of default CI and must be runnable with `cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture`.
- Random-window ladder options use `iterations = 5000`, `restarts = 8`, fixed seed `7`, and `target_weight` equal to each case's expected upper bound.
- Every random-window result must satisfy `upper_bound <= expected_upper_bound`.
- Every random-window witness must pass full stabilizer validation through the ladder verifier.
- Negative control runs the existing `randomized-upper-bound` baseline for `surface_rotated_d5` and passes only because the ladder verifier rejects the loose result against expected upper bound `5`.
- Do not require external `codeDistancePYPI`, Gurobi, or `dist-m4ri`.
- Do not claim exact distance certification; the method reports upper bounds.

---

### Task 1: Add Issue-225 Ladder Evidence Tests

**Files:**
- Modify: `qec-code/src/gf2.rs`
- Modify: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/tests/distance_bound.rs`

**Interfaces:**
- Consumes: `issue_225_ladder_cases() -> Vec<Issue225LadderCase>`, `issue_225_case(&str) -> Issue225LadderCase`, `css_from_built_in_code_id(&str) -> CssCode`, `random_window_css_upper_bound`, `randomized_css_upper_bound`, `verify_issue_225_ladder_case`, `gf2::try_rref_with_width`.
- Produces: `gf2::try_in_reduced_row_span(&ReducedRows, &[u8]) -> Result<bool>`.
- Produces: test functions `issue_225_random_window_upper_bound_smoke_ladder`, `issue_225_random_window_upper_bound_full_ladder`, and `issue_225_current_randomized_upper_bound_ladder_negative_control`.

- [ ] **Step 1: Write the failing ladder tests**

Add the time import at the top of `qec-code/tests/distance_bound.rs`:

```rust
use std::time::{Duration, Instant};
```

Add these constants and helpers after `pinned_random_window_options()`:

```rust
const ISSUE_225_RANDOM_WINDOW_SEED: u64 = 7;
const ISSUE_225_RANDOMIZED_NEGATIVE_CONTROL_SEED: u64 = 225;
const ISSUE_225_PER_CASE_CAP: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct Issue225LadderEvidenceRow {
    case_id: String,
    expected_upper_bound: usize,
    observed_upper_bound: usize,
    method: DistanceBoundMethod,
    seed: u64,
    elapsed: Duration,
}

fn issue_225_random_window_options(
    case: &Issue225LadderCase,
) -> RandomWindowUpperBoundOptions {
    RandomWindowUpperBoundOptions {
        iterations: 5000,
        restarts: 8,
        seed: ISSUE_225_RANDOM_WINDOW_SEED,
        target_weight: Some(case.target_weight),
    }
}

fn issue_225_randomized_negative_control_options(
    case: &Issue225LadderCase,
) -> RandomizedUpperBoundOptions {
    RandomizedUpperBoundOptions {
        iterations: 5000,
        restarts: 8,
        seed: ISSUE_225_RANDOMIZED_NEGATIVE_CONTROL_SEED,
        target_weight: Some(case.target_weight),
    }
}

fn run_issue_225_random_window_case(case: &Issue225LadderCase) -> Issue225LadderEvidenceRow {
    let css = css_from_built_in_code_id(&case.code_id);
    let options = issue_225_random_window_options(case);
    let started = Instant::now();
    let result = random_window_css_upper_bound(&css, options).unwrap_or_else(|error| {
        panic!(
            "{} random-window-upper-bound failed: {error}",
            case.case_id
        )
    });
    let elapsed = started.elapsed();

    verify_issue_225_ladder_case(
        case,
        &result,
        &css,
        DistanceBoundMethod::RandomWindowUpperBound,
    )
    .unwrap_or_else(|error| {
        panic!(
            "{} random-window ladder verifier rejected result: {error}",
            case.case_id
        )
    });

    assert!(
        elapsed <= ISSUE_225_PER_CASE_CAP,
        "{} exceeded issue-225 per-case cap: elapsed {:.3}s > 300s",
        case.case_id,
        elapsed.as_secs_f64()
    );

    Issue225LadderEvidenceRow {
        case_id: case.case_id.clone(),
        expected_upper_bound: case.expected_upper_bound,
        observed_upper_bound: result.upper_bound,
        method: result.method,
        seed: ISSUE_225_RANDOM_WINDOW_SEED,
        elapsed,
    }
}

fn run_issue_225_random_window_ladder<'a>(
    cases: impl IntoIterator<Item = &'a Issue225LadderCase>,
) -> Vec<Issue225LadderEvidenceRow> {
    println!("case_id\texpected\tobserved\tmethod\tseed\telapsed_s");
    let mut rows = Vec::new();
    for case in cases {
        let row = run_issue_225_random_window_case(case);
        println!(
            "{}\t{}\t{}\t{}\t{}\t{:.3}",
            row.case_id,
            row.expected_upper_bound,
            row.observed_upper_bound,
            row.method.label(),
            row.seed,
            row.elapsed.as_secs_f64()
        );
        rows.push(row);
    }
    rows
}
```

Add the three tests near the existing issue-225 verifier tests:

```rust
#[test]
fn issue_225_random_window_upper_bound_smoke_ladder() {
    let cases = issue_225_ladder_cases();
    let smoke_cases = cases
        .iter()
        .filter(|case| case.tier == "smoke")
        .collect::<Vec<_>>();
    let smoke_ids = smoke_cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        smoke_ids,
        ["surface_rotated_d5", "toric_d5", "bb72"],
        "issue-225 smoke tier changed: {smoke_ids:?}"
    );

    let rows = run_issue_225_random_window_ladder(smoke_cases.into_iter());
    assert_eq!(rows.len(), 3, "issue-225 smoke ladder checked row count");
}

#[test]
#[ignore = "full issue-225 ladder: cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture"]
fn issue_225_random_window_upper_bound_full_ladder() {
    let cases = issue_225_ladder_cases();
    assert_eq!(cases.len(), 8, "issue-225 full ladder must include all eight cases");

    let rows = run_issue_225_random_window_ladder(cases.iter());
    assert_eq!(rows.len(), 8, "issue-225 full ladder checked row count");
}

#[test]
fn issue_225_current_randomized_upper_bound_ladder_negative_control() {
    let case = issue_225_case("surface_rotated_d5");
    let css = css_from_built_in_code_id(&case.code_id);
    let result = randomized_css_upper_bound(
        &css,
        issue_225_randomized_negative_control_options(&case),
    )
    .unwrap();

    assert_eq!(result.method, DistanceBoundMethod::RandomizedUpperBound);
    assert!(
        result.upper_bound > case.expected_upper_bound,
        "{} negative control is no longer loose: expected upper_bound > {}, got {}",
        case.case_id,
        case.expected_upper_bound,
        result.upper_bound
    );

    let error = verify_issue_225_ladder_case(
        &case,
        &result,
        &css,
        DistanceBoundMethod::RandomizedUpperBound,
    )
    .expect_err("expected current randomized baseline to fail issue-225 ladder target");

    assert_eq!(
        error,
        QecError::DistanceBoundValidationFailed(format!(
            "{} expected upper_bound <= {}, got {}",
            case.case_id, case.expected_upper_bound, result.upper_bound
        ))
    );
}
```

- [ ] **Step 2: Run RED verification**

Run:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture
cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture
cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q
```

Expected: the test names are initially missing or fail to compile before the code is added.

- [ ] **Step 3: Implement the tests**

Apply the exact helper and test additions from Step 1 to `qec-code/tests/distance_bound.rs`, then run:

```bash
rustfmt qec-code/tests/distance_bound.rs
```

- [ ] **Step 4: Run focused GREEN verification**

Run:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture
cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture
cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q
```

Expected: all three commands pass. The smoke output names `surface_rotated_d5`, `toric_d5`, and `bb72`; the full output prints eight rows; the negative control passes because the verifier rejects a loose `surface_rotated_d5` randomized result.

- [ ] **Step 5: Commit**

```bash
git add qec-code/tests/distance_bound.rs docs/superpowers/plans/2026-06-25-issue-234-random-window-ladder-evidence.md
git commit -m "test: add issue 225 random-window ladder evidence"
```
