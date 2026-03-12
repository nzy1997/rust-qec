# rstim Decompose Errors Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expose `decompose_errors` through `rstim analyze_errors --decompose_errors` and verify that the CLI and library preserve strict default semantics while producing graphlike DEM output when decomposition is explicitly requested.

**Architecture:** Reuse the existing `ErrorAnalyzer::circuit_to_dem_decomposed` / `decompose_errors` implementation instead of inventing a second decomposition path in the CLI. Keep default `analyze_errors` behavior unchanged and thread one new boolean flag through the existing CLI entrypoint so decomposition is opt-in, testable, and isolated from the Phase 1/2 option-parity logic.

**Tech Stack:** Rust, Cargo workspace tests, `clap`, `rstim::error_analyzer::ErrorAnalyzer`, existing CLI tests in `rstim/tests/cli_analyze.rs` and `rstim/tests/cli_coverage.rs`.

---

### Task 1: Add a library-level CLI entrypoint for decomposed analysis

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write the failing coverage test**

In `rstim/tests/cli_coverage.rs`, add:

```rust
#[test]
fn run_analyze_errors_with_decompose_errors_uses_decomposed_path() {
    let mut buf = Vec::new();
    cli::run_analyze_errors_with_flags(
        "R 0 1 2\nX_ERROR(0.1) 0 1\nM 0 1 2\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        false,
        false,
        true,
        &mut buf,
    )
    .unwrap();
    let dem = String::from_utf8(buf).unwrap();
    assert!(dem.contains("error("));
}
```

**Step 2: Run the test to verify it fails**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_with_decompose_errors_uses_decomposed_path -- --exact
```

Expected: FAIL because `run_analyze_errors_with_flags` does not exist yet.

**Step 3: Add a single flag-aware library entrypoint**

In `rstim/src/cli.rs`, replace the current `run_analyze_errors_with_options(...)` helper with:

```rust
pub fn run_analyze_errors_with_flags(
    circuit_text: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
    decompose_errors: bool,
    out: &mut dyn Write,
) -> Result<(), String>
```

Implementation:
- parse the circuit once
- if `decompose_errors` is `true`, call a new analyzer entrypoint that combines options + decomposition
- otherwise keep the current strict/default path

Add the matching analyzer-side entrypoint in `rstim/src/error_analyzer.rs`:

```rust
pub fn circuit_to_dem_with_options_decomposed(
    instrs: &[StimInstr],
    options: AnalyzeOptions,
) -> Result<DetectorErrorModel, String> {
    let mut dem = Self::circuit_to_dem_with_options(instrs, options)?;
    decompose_errors(&mut dem)?;
    Ok(dem)
}
```

Keep the existing public methods intact; this is an additive API.

**Step 4: Run the test to verify it passes**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_with_decompose_errors_uses_decomposed_path -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/src/error_analyzer.rs rstim/tests/cli_coverage.rs
git commit -m "refactor: add flag-aware analyze_errors entrypoint"
```

### Task 2: Expose `--decompose_errors` on the CLI

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Write failing CLI tests for the new flag**

In `rstim/tests/cli_analyze.rs`, add:

```rust
#[test]
fn analyze_errors_decompose_errors_flag_is_accepted() {
    let output = run_with_stdin(
        &["analyze_errors", "--decompose_errors"],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn analyze_errors_default_output_is_unchanged_without_decompose_flag() {
    let plain = run_with_stdin(
        &["analyze_errors"],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
    );
    let decomp = run_with_stdin(
        &["analyze_errors", "--decompose_errors"],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(plain.status.success());
    assert!(decomp.status.success());
    assert_eq!(plain.stdout, decomp.stdout);
}
```

**Step 2: Run the tests to verify they fail**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_decompose_errors_flag_is_accepted -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_default_output_is_unchanged_without_decompose_flag -- --exact
```

Expected: first test fails with `unexpected argument '--decompose_errors'`; second fails for the same reason.

**Step 3: Add the CLI flag and wire it into dispatch**

In `rstim/src/cli.rs`, extend `Commands::AnalyzeErrors`:

```rust
#[arg(long = "decompose_errors")]
decompose_errors: bool,
```

Update the dispatch arm and `run_analyze_errors(...)` to call `run_analyze_errors_with_flags(...)` with:
- default `false` for the existing wrapper
- the parsed `decompose_errors` value in the command dispatcher

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_decompose_errors_flag_is_accepted -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_default_output_is_unchanged_without_decompose_flag -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_analyze.rs
git commit -m "feat: add --decompose_errors to analyze_errors"
```

### Task 3: Add an end-to-end CLI regression that proves decomposition changes non-graphlike output

**Files:**
- Modify: `rstim/tests/cli_analyze.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write a failing CLI regression for actual decomposition**

In `rstim/tests/cli_analyze.rs`, add:

```rust
#[test]
fn analyze_errors_decompose_errors_flag_graphlike_decomposes_rep_code() {
    let circuit = "R 0 1 2 3 4\nX_ERROR(0.1) 0\nCX 0 1\nCX 1 2\nCX 2 3\nCX 3 4\nM 0 1 2 3 4\nDETECTOR rec[-5] rec[-4]\nDETECTOR rec[-4] rec[-3]\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]";
    let output = run_with_stdin(&["analyze_errors", "--decompose_errors"], circuit);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let dem = String::from_utf8(output.stdout).unwrap();
    assert_all_graphlike(&dem);
}
```

Add a small local helper in the test file:

```rust
fn assert_all_graphlike(text: &str) {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("error") {
            if let Some(targets_part) = line.split(')').nth(1) {
                for comp in targets_part.split('^') {
                    assert!(comp.matches('D').count() <= 2, "non-graphlike component in: {line}");
                }
            }
        }
    }
}
```

Also add one library-level regression in `rstim/tests/cli_coverage.rs`:

```rust
#[test]
fn run_analyze_errors_with_decompose_errors_preserves_default_for_graphlike_input() {
    let circuit = "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let mut plain = Vec::new();
    let mut decomp = Vec::new();
    cli::run_analyze_errors_with_flags(circuit, false, false, false, &mut plain).unwrap();
    cli::run_analyze_errors_with_flags(circuit, false, false, true, &mut decomp).unwrap();
    assert_eq!(plain, decomp);
}
```

**Step 2: Run the tests to verify they fail for the right reason**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_decompose_errors_flag_graphlike_decomposes_rep_code -- --exact
cargo test -p rstim --test cli_coverage run_analyze_errors_with_decompose_errors_preserves_default_for_graphlike_input -- --exact
```

Expected:
- the graphlike-preservation test may already pass
- if the rep-code test fails, failure should be about non-graphlike output or an unsuitable circuit, not CLI parsing

If the rep-code circuit does not reliably produce a non-graphlike error under the current analyzer, replace it with a direct circuit from an existing passing decomposition test instead of forcing through a brittle example.

**Step 3: Adjust the test fixture, not the production code, if needed**

This task is primarily about locking down behavior around the new CLI flag, not about rewriting decomposition internals. If the chosen circuit is brittle:
- borrow a stable decomposition-producing circuit from `rstim/tests/decompose_errors.rs`
- keep production code unchanged unless you find a real bug in the current decomposition path

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_decompose_errors_flag_graphlike_decomposes_rep_code -- --exact
cargo test -p rstim --test cli_coverage run_analyze_errors_with_decompose_errors_preserves_default_for_graphlike_input -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/tests/cli_analyze.rs rstim/tests/cli_coverage.rs
git commit -m "test: add analyze_errors decomposition regressions"
```

### Task 4: Verify decomposition and strict-option interactions

**Files:**
- Modify: `rstim/tests/cli_analyze.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`

**Step 1: Write failing interaction tests**

Add one CLI regression to `rstim/tests/cli_analyze.rs`:

```rust
#[test]
fn analyze_errors_decompose_errors_can_be_combined_with_phase2_flags() {
    let output = run_with_stdin(
        &["analyze_errors", "--approximate_disjoint_errors", "--decompose_errors"],
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

Add one library regression in `rstim/tests/stim_error_analyzer.rs`:

```rust
#[test]
fn circuit_to_dem_with_options_decomposed_respects_phase2_flags() {
    let instrs = parse_lines(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]"
    ).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_with_options_decomposed(
        &instrs,
        AnalyzeOptions {
            approximate_disjoint_errors: true,
            allow_gauge_detectors: false,
        },
    )
    .unwrap();
    assert!(dem.to_string().contains("error("));
}
```

**Step 2: Run the tests to verify they fail if the new path ignores options**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_decompose_errors_can_be_combined_with_phase2_flags -- --exact
cargo test -p rstim --test stim_error_analyzer circuit_to_dem_with_options_decomposed_respects_phase2_flags -- --exact
```

Expected: FAIL if the decompose path is not using the option-aware analyzer entrypoint.

**Step 3: Fix the option plumbing if needed**

If the tests fail, make the minimum change in `rstim/src/cli.rs` or `rstim/src/error_analyzer.rs` so the decomposed path always goes through `AnalyzeOptions`, never around them.

Do not add new decomposition semantics here; this task is only about preserving Phase 2 behavior when decomposition is turned on.

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_decompose_errors_can_be_combined_with_phase2_flags -- --exact
cargo test -p rstim --test stim_error_analyzer circuit_to_dem_with_options_decomposed_respects_phase2_flags -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/src/error_analyzer.rs rstim/tests/cli_analyze.rs rstim/tests/stim_error_analyzer.rs
git commit -m "fix: preserve analyze_errors options when decomposing dem output"
```

### Task 5: Run the preserved decomposition and DEM regressions

**Files:**
- No production changes required unless regressions appear

**Step 1: Run the direct decomposition tests**

```bash
cargo test -p rstim --test decompose_errors
```

Expected: PASS.

**Step 2: Run the CLI and analyzer suites touched by the new flag**

```bash
cargo test -p rstim --test cli_analyze
cargo test -p rstim --test cli_coverage
cargo test -p rstim --test stim_error_analyzer
```

Expected: PASS.

**Step 3: Run DEM-path regressions that should stay green**

```bash
cargo test -p rstim --test cross_validate_dem
cargo test -p rstim --test stim_dem
cargo test -p rstim --test dem_integration
cargo test -p rstim --test error_analyzer
```

Expected: PASS. Keep the ignored Stim CLI tests ignored.

**Step 4: Run workspace verification**

```bash
cargo test --workspace
```

Expected: PASS.

**Step 5: Commit the verification checkpoint**

```bash
git add -A
git commit -m "test: verify analyze_errors decompose_errors regressions"
```
