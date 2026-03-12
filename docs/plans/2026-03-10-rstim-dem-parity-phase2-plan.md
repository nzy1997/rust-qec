# rstim DEM Parity Phase 2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Phase 2 option parity to `rstim analyze_errors` by exposing `approximate_disjoint_errors` and `allow_gauge_detectors` as explicit opt-in analysis modes while preserving the stricter Phase 1 defaults.

**Architecture:** Keep the current `ErrorAnalyzer` reverse-walk logic and thread a small analysis-options struct from the CLI into the analyzer instead of adding global flags or duplicating code paths. Implement each option behind failing analyzer and CLI tests first, then reuse the existing Phase 1 guard helpers by making their rejection behavior conditional on the chosen options.

**Tech Stack:** Rust, Cargo workspace tests, `clap`, `rstim::error_analyzer::ErrorAnalyzer`, existing CLI entrypoints in `rstim/src/cli.rs`.

---

### Task 1: Add an explicit analysis-options surface

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write a failing unit test for the new default options**

In `rstim/tests/cli_coverage.rs`, add a focused test that exercises the library entrypoint instead of the subprocess wrapper:

```rust
#[test]
fn run_analyze_errors_with_default_options_still_rejects_gauge() {
    let mut buf = Vec::new();
    let err = cli::run_analyze_errors_with_options(
        "R 0\nH 0\nM 0\nDETECTOR rec[-1]",
        false,
        false,
        &mut buf,
    )
    .unwrap_err();
    assert!(err.contains("non-deterministic"));
}
```

**Step 2: Run the new test to verify it fails**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_with_default_options_still_rejects_gauge -- --exact
```

Expected: FAIL because `run_analyze_errors_with_options` does not exist yet.

**Step 3: Add a minimal options type and threaded entrypoint**

In `rstim/src/error_analyzer.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyzeOptions {
    pub approximate_disjoint_errors: bool,
    pub allow_gauge_detectors: bool,
}
```

Add a new public entrypoint:

```rust
pub fn circuit_to_dem_with_options(
    instrs: &[StimInstr],
    options: AnalyzeOptions,
) -> Result<DetectorErrorModel, String>
```

Make the existing `circuit_to_dem` delegate to it with `AnalyzeOptions::default()`.

In `rstim/src/cli.rs`, add:

```rust
pub fn run_analyze_errors_with_options(
    circuit_text: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
    out: &mut dyn Write,
) -> Result<(), String>
```

and have `run_analyze_errors` delegate to it with both flags `false`.

**Step 4: Run the new test to verify it passes**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_with_default_options_still_rejects_gauge -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/src/cli.rs rstim/tests/cli_coverage.rs
git commit -m "refactor: thread analyze_errors options through cli and analyzer"
```

### Task 2: Add CLI flags for Phase 2 option parity

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Write failing CLI tests for the new flags**

In `rstim/tests/cli_analyze.rs`, add:

```rust
#[test]
fn analyze_errors_allow_gauge_detectors_flag_accepts_gauge_circuit() {
    let output = run_with_stdin(
        &["analyze_errors", "--allow_gauge_detectors"],
        "R 0\nH 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn analyze_errors_approximate_disjoint_errors_flag_accepts_pauli_channel_2() {
    let output = run_with_stdin(
        &["analyze_errors", "--approximate_disjoint_errors"],
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

**Step 2: Run the new tests to verify they fail**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_allow_gauge_detectors_flag_accepts_gauge_circuit -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_approximate_disjoint_errors_flag_accepts_pauli_channel_2 -- --exact
```

Expected: FAIL because the CLI does not recognize the flags yet.

**Step 3: Add the flags to the `analyze_errors` command**

In `rstim/src/cli.rs`, extend `Commands::AnalyzeErrors`:

```rust
AnalyzeErrors {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long = "approximate_disjoint_errors")]
    approximate_disjoint_errors: bool,
    #[arg(long = "allow_gauge_detectors")]
    allow_gauge_detectors: bool,
},
```

Update the dispatch arm to call `run_analyze_errors_with_options(...)` with the parsed flags.

**Step 4: Run the new tests to verify they still fail for the right reason**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_allow_gauge_detectors_flag_accepts_gauge_circuit -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_approximate_disjoint_errors_flag_accepts_pauli_channel_2 -- --exact
```

Expected: FAIL with analyzer-side errors, not with `unexpected argument`.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_analyze.rs
git commit -m "feat: add analyze_errors option parity flags"
```

### Task 3: Implement `allow_gauge_detectors`

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Write failing analyzer tests for opt-in gauge acceptance**

In `rstim/tests/stim_error_analyzer.rs`, add a helper that calls the new analyzer entrypoint:

```rust
fn circuit_to_dem_with_options(
    circuit_str: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
) -> Result<DetectorErrorModel, String> {
    let instrs = parse_lines(circuit_str).unwrap();
    ErrorAnalyzer::circuit_to_dem_with_options(
        &instrs,
        rstim::error_analyzer::AnalyzeOptions {
            approximate_disjoint_errors,
            allow_gauge_detectors,
        },
    )
}
```

Add:

```rust
#[test]
fn stim_detect_gauge_detector_allowed_with_option() {
    let dem = circuit_to_dem_with_options(
        "R 0\nH 0\nM 0\nDETECTOR rec[-1]",
        false,
        true,
    )
    .unwrap();
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn stim_detect_gauge_observable_allowed_with_option() {
    let dem = circuit_to_dem_with_options(
        "R 0\nH 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        false,
        true,
    )
    .unwrap();
    assert_eq!(error_count(&dem), 0);
}
```

**Step 2: Run the new tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_detect_gauge_detector_allowed_with_option -- --exact
cargo test -p rstim --test stim_error_analyzer stim_detect_gauge_observable_allowed_with_option -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_allow_gauge_detectors_flag_accepts_gauge_circuit -- --exact
```

Expected: FAIL because Phase 1 always rejects gauge circuits.

**Step 3: Make gauge checks conditional on the option**

In `rstim/src/error_analyzer.rs`:

- Store `AnalyzeOptions` on `ErrorAnalyzer`.
- Skip `ensure_measurement_is_deterministic`, `ensure_reset_is_deterministic`, and `ensure_no_pending_gauge` rejection paths when `allow_gauge_detectors` is `true`.
- Do not weaken any default behavior when the option is `false`.

Keep the implementation narrow: the option only suppresses gauge rejection. It should not change target ordering, merged-probability logic, or exact supported-channel behavior.

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_detect_gauge_detector_allowed_with_option -- --exact
cargo test -p rstim --test stim_error_analyzer stim_detect_gauge_observable_allowed_with_option -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_allow_gauge_detectors_flag_accepts_gauge_circuit -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "feat: allow gauge detectors in analyze_errors when requested"
```

### Task 4: Implement `approximate_disjoint_errors` for `PAULI_CHANNEL_2`

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Write failing analyzer tests for `PAULI_CHANNEL_2` approximation**

In `rstim/tests/stim_error_analyzer.rs`, add:

```rust
#[test]
fn stim_pauli_channel_2_allowed_with_approximation_option() {
    let dem = circuit_to_dem_with_options(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
        true,
        false,
    )
    .unwrap();
    assert_eq!(error_count(&dem), 1);
    assert_has_error_approx(&dem, 0.01, 1e-12, &[DemTarget::Detector(0)]);
}
```

**Step 2: Run the new tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_pauli_channel_2_allowed_with_approximation_option -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_approximate_disjoint_errors_flag_accepts_pauli_channel_2 -- --exact
```

Expected: FAIL because the analyzer still rejects `PAULI_CHANNEL_2`.

**Step 3: Restore conditional `PAULI_CHANNEL_2` support behind the option**

In `rstim/src/error_analyzer.rs`, make the `PAULI_CHANNEL_2` branch:

- return the current Phase 1 rejection when `approximate_disjoint_errors` is `false`
- otherwise re-enable the prior approximate expansion logic for non-zero components

Do not broaden support beyond the existing approximated behavior. The point of Phase 2 is to expose that behavior explicitly, not to invent a new approximation scheme.

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_pauli_channel_2_allowed_with_approximation_option -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_approximate_disjoint_errors_flag_accepts_pauli_channel_2 -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "feat: expose pauli_channel_2 approximation behind analyze_errors flag"
```

### Task 5: Implement `approximate_disjoint_errors` for multi-branch correlated blocks

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Write failing tests for three-branch opt-in acceptance**

In `rstim/tests/stim_error_analyzer.rs`, add:

```rust
#[test]
fn stim_correlated_error_three_branch_block_allowed_with_approximation_option() {
    let dem = circuit_to_dem_with_options(
        "E(0.1) X0\nELSE_CORRELATED_ERROR(0.2) Z0\nELSE_CORRELATED_ERROR(0.3) Y0\nM 0\nDETECTOR rec[-1]",
        true,
        false,
    )
    .unwrap();
    assert_eq!(error_count(&dem), 1);
    assert_has_error_approx(&dem, 0.352, 1e-12, &[DemTarget::Detector(0)]);
}
```

In `rstim/tests/cli_analyze.rs`, add:

```rust
#[test]
fn analyze_errors_approximate_disjoint_errors_flag_accepts_correlated_block() {
    let output = run_with_stdin(
        &["analyze_errors", "--approximate_disjoint_errors"],
        "E(0.1) X0\nELSE_CORRELATED_ERROR(0.2) Z0\nELSE_CORRELATED_ERROR(0.3) Y0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

**Step 2: Run the new tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_correlated_error_three_branch_block_allowed_with_approximation_option -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_approximate_disjoint_errors_flag_accepts_correlated_block -- --exact
```

Expected: FAIL because Phase 1 rejects `>2` branches unconditionally.

**Step 3: Make multi-branch correlated-block handling option-aware**

In `rstim/src/error_analyzer.rs`:

- keep the exact existing two-branch handling for both modes
- reject blocks with more than two branches when `approximate_disjoint_errors` is `false`
- allow larger blocks when `approximate_disjoint_errors` is `true` by applying the same remaining-mass walk already used for two branches

This is a scoped approximation toggle. Do not rewrite the correlated-block collector, and do not change standalone `ELSE_CORRELATED_ERROR` rejection.

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_correlated_error_three_branch_block_allowed_with_approximation_option -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_approximate_disjoint_errors_flag_accepts_correlated_block -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "feat: allow approximate correlated blocks in analyze_errors"
```

### Task 6: Run parity regressions for both default and opt-in modes

**Files:**
- Modify: `rstim/tests/cli_coverage.rs`
- Modify: `rstim/tests/cli_analyze.rs`
- No production changes unless regressions appear

**Step 1: Add one explicit default-vs-opt-in regression pair**

In `rstim/tests/cli_coverage.rs`, add a focused library-level test proving the new flags do not weaken default behavior:

```rust
#[test]
fn run_analyze_errors_options_only_change_behavior_when_enabled() {
    let mut buf = Vec::new();
    let strict = cli::run_analyze_errors_with_options(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
        false,
        false,
        &mut buf,
    );
    assert!(strict.is_err());

    let mut buf = Vec::new();
    let relaxed = cli::run_analyze_errors_with_options(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
        true,
        false,
        &mut buf,
    );
    assert!(relaxed.is_ok());
}
```

**Step 2: Run the focused regression tests**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_options_only_change_behavior_when_enabled -- --exact
cargo test -p rstim --test cli_analyze
cargo test -p rstim --test stim_error_analyzer
```

Expected: PASS.

**Step 3: Run preserved DEM-path regressions**

```bash
cargo test -p rstim --test cross_validate_dem
cargo test -p rstim --test stim_dem
cargo test -p rstim --test dem_integration
cargo test -p rstim --test error_analyzer
```

Expected: PASS. Keep the ignored Stim CLI cases ignored; do not weaken the new helper tests.

**Step 4: Commit the verification checkpoint**

```bash
git add rstim/tests/cli_coverage.rs rstim/tests/cli_analyze.rs
git commit -m "test: verify analyze_errors option parity regressions"
```
