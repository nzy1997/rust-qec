# Issue 130 BB144 QEC-Code Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add BB144 CLI sparse-row smoke coverage and a scope note that the bivariate-bicycle MVP is `qec-code` construction/export support, not downstream #110/#124 circuit or benchmark work.

**Architecture:** Keep all executable regression coverage in `qec-code/tests/cli.rs`, where built-in CSS CLI export behavior is already tested. Reuse the existing `run_qec_code(...)` helper and `serde_json` parsing style, add a BB144 spec constant, and assert only JSON shape rather than introducing a new fixture.

**Tech Stack:** Rust 2024, `qec-code`, existing integration-test binary helper, `serde_json::Value`, existing built-in CSS sparse-row JSON contract.

## Global Constraints

- Keep this issue in `qec-code`; do not modify `rsinter`.
- Do not add circuit-level BB144 work, benchmark specs, logical observables, or fixture generation.
- Prove BB144 can be exported through CLI as sparse rows.
- Positive BB144 CLI smoke must parse stdout as compact `sparse_rows` JSON with `num_cols == 144` and `rows.length == 72`.
- Negative control `bb:lx=12,ly=6,a=3:0|,b=0:3` must exit non-zero and emit no JSON stdout.
- Add a short note that #110 and #124 remain downstream circuit/benchmark scopes.

---

## File Structure

- Modify `qec-code/tests/cli.rs`: add BB144 constants, one positive CLI sparse-row smoke test, one malformed-term negative CLI test, and a short scope note comment near the BB144 regression.
- Create `docs/superpowers/specs/2026-06-23-issue-130-bb144-qec-code-regression-design.md`: record the approved non-interactive design.
- Create `docs/superpowers/plans/2026-06-23-issue-130-bb144-qec-code-regression.md`: record this implementation plan.

### Task 1: Add BB144 CLI Smoke Regression

**Files:**
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: `run_qec_code(args: &[&str]) -> std::process::Output`, existing `qec-code code css <spec> hx` CLI path, and `serde_json::Value`.
- Produces: `code_css_bb144_parameterized_hx_prints_sparse_rows_shape()` and `code_css_bb_parameterized_malformed_shift_term_fails_without_json()` integration tests.

- [ ] **Step 1: Write failing BB144 CLI tests**

In `qec-code/tests/cli.rs`, add this constant after `BB72_PARAMETERIZED_SPEC`:

```rust
const BB144_PARAMETERIZED_SPEC: &str = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0";
```

Add this positive test after `code_css_bb_parameterized_hz_matches_bb72_fixture`:

```rust
#[test]
fn code_css_bb144_parameterized_hx_prints_sparse_rows_shape() {
    // This is qec-code construction/export coverage only; circuit-level
    // BB144 work and benchmark reproduction remain downstream in #110/#124.
    let output = run_qec_code(&["code", "css", BB144_PARAMETERIZED_SPEC, "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
    let rows = json["rows"]
        .as_array()
        .expect("sparse-row JSON should contain rows");

    assert_eq!(json["format"], "sparse_rows");
    assert_eq!(json["num_cols"], 144);
    assert_eq!(rows.len(), 72);
    assert!(
        rows.iter()
            .all(|row| row.as_array().is_some_and(|cols| cols.len() == 6)),
        "all BB144 hx rows should have weight 6: {rows:?}"
    );
}
```

Add this negative-control test after `code_css_bb_parameterized_invalid_lattice_dimension_fails_without_json`:

```rust
#[test]
fn code_css_bb_parameterized_malformed_shift_term_fails_without_json() {
    let output = run_qec_code(&["code", "css", "bb:lx=12,ly=6,a=3:0|,b=0:3", "hx"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("invalid built-in CSS integer parameter a for family bb"),
        "stderr was: {stderr}"
    );
}
```

- [ ] **Step 2: Run focused test to verify RED if possible**

Run:

```bash
cargo test -p qec-code --test cli bb144
```

Expected: If the tests have just been added, this filtered command may already pass because the implementation from #129 exists. If it passes immediately, confirm the test exercises the requested CLI path and proceed because this issue is adding regression coverage for existing behavior rather than changing production code.

- [ ] **Step 3: Keep implementation minimal**

No production code should be changed for this issue. If Step 2 fails because the CLI output shape differs from the issue contract, inspect `qec-code/src/codes/built_in_css.rs` and `qec-code/src/cli.rs`, then fix only the narrow path required to make `qec-code code css <BB144 spec> hx` emit valid `sparse_rows` JSON.

- [ ] **Step 4: Run focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code --test cli bb144
```

Expected: PASS with `code_css_bb144_parameterized_hx_prints_sparse_rows_shape`.

- [ ] **Step 5: Commit task**

Run:

```bash
git add qec-code/tests/cli.rs
git commit -m "test: cover bb144 css cli smoke"
```

### Task 2: Verify Issue Contract And Publish

**Files:**
- No additional source files.

**Interfaces:**
- Consumes: final branch state from Task 1.
- Produces: verified branch and pull request.

- [ ] **Step 1: Run qec-code package verification**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS for constructor, parser, CLI, existing `bb72`, fixture manifest, and BB144 smoke coverage.

- [ ] **Step 2: Run positive BB144 CLI command**

Run:

```bash
cargo run -p qec-code -- code css "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0" hx
```

Expected: exit 0 and stdout parses as compact `sparse_rows` JSON with `num_cols == 144` and `rows.length == 72`.

- [ ] **Step 3: Run negative control command**

Run:

```bash
cargo run -p qec-code -- code css "bb:lx=12,ly=6,a=3:0|,b=0:3" hx
```

Expected: non-zero exit and empty stdout.

- [ ] **Step 4: Run repository-level verification required by Agent Desk**

Run:

```bash
cargo test
```

Expected: PASS. If unrelated workspace warnings appear, record them without broadening this issue.

- [ ] **Step 5: Check formatting of committed diffs**

Run:

```bash
cargo fmt -p qec-code --check
git diff --check
```

Expected: both commands exit 0.

- [ ] **Step 6: Push and open PR**

Run:

```bash
git status --short --branch
git push -u origin agent/issue-130-add-bivariate-bicycle-regression-notes-and-bb144-run-1
```

Open a pull request against `master` with a summary that notes this is `qec-code`
construction/export support and that #110/#124 remain downstream.
