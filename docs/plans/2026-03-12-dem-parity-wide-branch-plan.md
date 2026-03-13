# DEM Parity Wide Branch Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining high-value DEM parity gaps against Stim on the current wide branch by tightening `decompose_errors` semantic parity, adding explicit `fold_loops` support, and verifying the combined mode matrix on hand-written and code-generated circuits.

**Architecture:** Keep one plain DEM analysis path as the source of truth in `rstim::error_analyzer`, then layer `decompose_errors` as a semantic transform and `fold_loops` as a structural transform. Preserve the current default flat CLI behavior and only add explicit folding opt-ins; use Stim cross-checks for semantic parity and internal flat-vs-folded equivalence checks for structural correctness.

**Tech Stack:** Rust, Cargo workspace tests, `rstim::error_analyzer::ErrorAnalyzer`, `rstim::dem::DetectorErrorModel`, `clap`, Stim CLI / Python API reference behavior, existing parity tests in `rstim/tests/`.

---

> **Worktree note:** The design called for a dedicated git worktree, but this sandbox blocks writes under `.git/refs` and `.git/worktrees`. Execute this plan in a dedicated worktree when the environment allows it; otherwise use the current clean branch as fallback.

### Task 1: Add missing Stim cross-checks for decomposed hand-written circuits

**Files:**
- Modify: `rstim/tests/cross_validate_dem.rs`
- Reference: `rstim/src/error_analyzer.rs`
- Reference: `docs/plans/2026-03-12-dem-parity-wide-branch-design.md`

**Step 1: Write the failing cross-check helper assertions**

In `rstim/tests/cross_validate_dem.rs`, add helpers that parse DEM text into:

```rust
fn parse_dem_errors_multi(dem_text: &str) -> BTreeMap<String, Vec<f64>> {
    let mut out = BTreeMap::new();
    for line in dem_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("error(") {
            if let Some(paren_end) = rest.find(')') {
                let prob: f64 = rest[..paren_end].parse().unwrap();
                let targets = rest[paren_end + 1..].trim().to_string();
                out.entry(targets).or_insert_with(Vec::new).push(prob);
            }
        }
    }
    for probs in out.values_mut() {
        probs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    out
}

fn assert_all_graphlike_dem_text(dem_text: &str) {
    for line in dem_text.lines() {
        let line = line.trim();
        if line.starts_with("error(") {
            let targets = line.split(')').nth(1).unwrap_or("").trim();
            for comp in targets.split('^') {
                let det_count = comp.split_whitespace().filter(|t| t.starts_with('D')).count();
                assert!(det_count <= 2, "non-graphlike component in: {line}");
            }
        }
    }
}
```

Add a failing test for a hand-written non-graphlike circuit:

```rust
#[test]
#[cfg(not(tarpaulin))]
fn cross_validate_decomposed_handwritten_non_graphlike_circuit() {
    let _guard = lock_stim_env();
    let circuit_text = "\
R 0 1 2
X_ERROR(0.1) 0
CX 0 1
CX 1 2
M 0 1 2
DETECTOR rec[-3]
DETECTOR rec[-2]
DETECTOR rec[-1]
";
    let instrs = parse_lines(circuit_text).unwrap();
    let stim_dem_text = stim_analyze_errors_flags(circuit_text, &["--decompose_errors"]);
    let rstim_dem = ErrorAnalyzer::circuit_to_dem_decomposed(&instrs).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    assert_all_graphlike_dem_text(&stim_dem_text);
    assert_all_graphlike_dem_text(&rstim_dem_text);
    assert_eq!(parse_dem_errors_multi(&stim_dem_text), parse_dem_errors_multi(&rstim_dem_text));
}
```

**Step 2: Run the new test to verify it fails**

```bash
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_handwritten_non_graphlike_circuit -- --exact
```

Expected: FAIL if current decomposition target sets, probabilities, or graphlike status diverge from Stim on this hand-written case.

**Step 3: Add a Stim helper that supports flags**

In `rstim/tests/cross_validate_dem.rs`, add:

```rust
fn stim_analyze_errors_flags(circuit_text: &str, flags: &[&str]) -> String {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let mut child = Command::new(stim_cmd)
        .arg("analyze_errors")
        .args(flags)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stim CLI not found");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(circuit_text.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stim failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}
```

Keep the existing no-flags helper delegating to this one.

**Step 4: Run the test to verify it now fails for semantic reasons or passes cleanly**

```bash
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_handwritten_non_graphlike_circuit -- --exact
```

Expected: PASS only if rstim already matches Stim semantically on the hand-written decomposed case. If it still fails, keep the failure for the next task.

**Step 5: Commit**

```bash
git add rstim/tests/cross_validate_dem.rs
git commit -m "test: add handwritten decomposed dem cross-check"
```

### Task 2: Add codegen decomposition parity tests before touching implementation

**Files:**
- Modify: `rstim/tests/cross_validate_dem.rs`
- Reference: `rstim/src/codegen/rep_code.rs`
- Reference: `rstim/src/codegen/surface_code.rs`
- Reference: `rstim/src/codegen/color_code.rs`

**Step 1: Write failing decomposition parity tests for codegen circuits**

Add three tests:

```rust
#[test]
#[cfg(not(tarpaulin))]
fn cross_validate_decomposed_rep_code_dem() {
    let _guard = lock_stim_env();
    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);
    let stim_dem_text = stim_analyze_errors_flags(&circuit_text, &["--decompose_errors"]);
    let rstim_dem_text = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap().to_string();
    assert_all_graphlike_dem_text(&stim_dem_text);
    assert_all_graphlike_dem_text(&rstim_dem_text);
    assert_eq!(parse_dem_errors_multi(&stim_dem_text), parse_dem_errors_multi(&rstim_dem_text));
}
```

Repeat the same pattern for:

- `rotated_memory_x(5, 3, 0.01)`
- one stable color-code generator already covered by `rstim/tests/gen_color_code.rs`

**Step 2: Run the tests to verify current state**

```bash
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_rep_code_dem -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_surface_code_dem -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_color_code_dem -- --exact
```

Expected: At least one test is likely to fail if decomposition parity is still incomplete. Capture the first failing target-set or probability mismatch before editing implementation.

**Step 3: Add a reusable semantic compare helper**

Factor the repeated comparison into:

```rust
fn assert_semantic_dem_parity(stim_dem_text: &str, rstim_dem_text: &str) {
    assert_eq!(parse_dem_errors_multi(stim_dem_text), parse_dem_errors_multi(rstim_dem_text));
    let stim_det_lines: Vec<_> = stim_dem_text.lines()
        .filter(|l| l.starts_with("detector") || l.starts_with("shift_detectors"))
        .collect();
    let rstim_det_lines: Vec<_> = rstim_dem_text.lines()
        .filter(|l| l.starts_with("detector") || l.starts_with("shift_detectors"))
        .collect();
    assert_eq!(stim_det_lines, rstim_det_lines);
}
```

Use it in both the old plain parity tests and the new decomposed tests.

**Step 4: Run the targeted tests again**

```bash
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_rep_code_dem -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_surface_code_dem -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_color_code_dem -- --exact
```

Expected: Failures now point directly at the semantic mismatch instead of helper drift.

**Step 5: Commit**

```bash
git add rstim/tests/cross_validate_dem.rs
git commit -m "test: add codegen decomposed dem parity checks"
```

### Task 3: Fix decomposition semantic mismatches one root cause at a time

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Test: `rstim/tests/cross_validate_dem.rs`
- Reference: `rstim/tests/decompose_errors.rs`
- Reference: `docs/plans/2026-03-12-dem-parity-wide-branch-design.md`

**Step 1: Capture the first failing semantic mismatch in a focused unit test**

Take the first failing Stim-vs-rstim case from Tasks 1-2 and reduce it into a direct unit test in `rstim/tests/decompose_errors.rs` or `rstim/tests/stim_error_analyzer.rs`. Example shape:

```rust
#[test]
fn decompose_errors_matches_stim_for_specific_non_graphlike_case() {
    let instrs = parse_lines("...minimal failing circuit...").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&instrs).unwrap();
    assert_eq!(dem.to_string(), "...expected semantic-normalized output...");
}
```

Do not add multiple failing cases at once. One root cause per test.

**Step 2: Run the focused test to verify it fails**

```bash
cargo test -p rstim --test decompose_errors decompose_errors_matches_stim_for_specific_non_graphlike_case -- --exact
```

Expected: FAIL with one concrete decomposition mismatch.

**Step 3: Implement the minimal semantic fix**

In `rstim/src/error_analyzer.rs`, adjust only the part of `decompose_errors(...)` needed to make the focused case match Stim. Candidate areas to inspect:

- graphlike component indexing in `graphlike_map`
- symmetric-difference reconstruction of larger target sets
- remnant-edge handling
- component ordering and separator placement
- preservation of observable targets during decomposition

Avoid changing plain analysis code in this task unless the failing case proves the root cause is upstream of decomposition itself.

**Step 4: Run the focused test and the cross-checks**

```bash
cargo test -p rstim --test decompose_errors decompose_errors_matches_stim_for_specific_non_graphlike_case -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_rep_code_dem -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_surface_code_dem -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_color_code_dem -- --exact
```

Expected: The focused test passes and at least one failing cross-check is eliminated. If another cross-check still fails for a different reason, start a new focused test in the next commit instead of bundling fixes.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/decompose_errors.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cross_validate_dem.rs
git commit -m "fix: align decompose_errors semantics with stim"
```

### Task 4: Add explicit `fold_loops` option surface without changing defaults

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_analyze.rs`
- Modify: `rstim/tests/cli_coverage.rs`

**Step 1: Write failing CLI and library option tests**

In `rstim/tests/cli_analyze.rs`, add:

```rust
#[test]
fn analyze_errors_fold_loops_flag_is_accepted() {
    let output = run_with_stdin(
        &["analyze_errors", "--fold_loops"],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
```

In `rstim/tests/cli_coverage.rs`, add:

```rust
#[test]
fn run_analyze_errors_with_fold_loops_preserves_default_for_non_repeat_input() {
    let circuit = "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]";
    let mut plain = Vec::new();
    let mut folded = Vec::new();
    cli::run_analyze_errors_with_all_flags(circuit, false, false, false, false, &mut plain).unwrap();
    cli::run_analyze_errors_with_all_flags(circuit, false, false, false, true, &mut folded).unwrap();
    assert_eq!(plain, folded);
}
```

**Step 2: Run the tests to verify they fail**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_fold_loops_flag_is_accepted -- --exact
cargo test -p rstim --test cli_coverage run_analyze_errors_with_fold_loops_preserves_default_for_non_repeat_input -- --exact
```

Expected: FAIL because the fold-loops flag surface does not exist yet.

**Step 3: Add the option plumbing only**

In `rstim/src/error_analyzer.rs`, extend `AnalyzeOptions`:

```rust
pub struct AnalyzeOptions {
    pub approximate_disjoint_errors: bool,
    pub allow_gauge_detectors: bool,
    pub fold_loops: bool,
}
```

In `rstim/src/cli.rs`, replace the current flag-aware helper with:

```rust
pub fn run_analyze_errors_with_all_flags(
    circuit_text: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
    decompose_errors: bool,
    fold_loops: bool,
    out: &mut dyn Write,
) -> Result<(), String>
```

Thread the new `fold_loops` boolean through `Commands::AnalyzeErrors` with:

```rust
#[arg(long = "fold_loops")]
fold_loops: bool,
```

Do not implement folding logic yet. Just preserve behavior for non-repeat circuits and for the default no-flag path.

**Step 4: Run the tests to verify option plumbing passes**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_fold_loops_flag_is_accepted -- --exact
cargo test -p rstim --test cli_coverage run_analyze_errors_with_fold_loops_preserves_default_for_non_repeat_input -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/src/cli.rs rstim/tests/cli_analyze.rs rstim/tests/cli_coverage.rs
git commit -m "feat: add fold_loops option surface for analyze_errors"
```

### Task 5: Implement a conservative loop-folding pass over DEM output

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Possibly create: `rstim/src/dem_fold.rs`
- Test: `rstim/tests/cross_validate_dem.rs`
- Test: `rstim/tests/cli_analyze.rs`
- Test: `rstim/tests/dem_ir.rs`

**Step 1: Write a failing folding regression on a repeated circuit**

In `rstim/tests/cli_analyze.rs`, add:

```rust
#[test]
fn analyze_errors_fold_loops_emits_repeat_for_repeated_detector_pattern() {
    let circuit = "R 0\nREPEAT 3 {\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nR 0\n}";
    let output = run_with_stdin(&["analyze_errors", "--fold_loops"], circuit);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let dem = String::from_utf8(output.stdout).unwrap();
    assert!(dem.contains(\"repeat 3 {\"));
}
```

In `rstim/tests/cross_validate_dem.rs`, add a Stim comparison against:

```bash
stim analyze_errors --fold_loops
```

on a hand-written repeated circuit. Compare:

- `repeat` presence
- semantic parity after expanding mentally via target maps
- detector / shift annotation lines

**Step 2: Run the tests to verify they fail**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_fold_loops_emits_repeat_for_repeated_detector_pattern -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_folded_handwritten_repeat_circuit -- --exact
```

Expected: FAIL because `rstim` currently returns flat DEM text only.

**Step 3: Implement a conservative folding pass**

Add a helper with a narrow contract, either in `rstim/src/error_analyzer.rs` or a new `rstim/src/dem_fold.rs`:

```rust
pub fn fold_dem_loops(dem: &DetectorErrorModel) -> DetectorErrorModel
```

Initial implementation requirements:

- only detect a repeated body when the detector and error pattern repeats with a fixed detector index offset
- preserve coordinates by emitting `shift_detectors(...)`
- fold only when the body repeats exactly for `N >= 2`
- otherwise return the original flat DEM unchanged

Apply this pass after plain or decomposed DEM generation when `options.fold_loops` is enabled.

**Step 4: Run the folding tests and regression matrix**

```bash
cargo test -p rstim --test cli_analyze analyze_errors_fold_loops_emits_repeat_for_repeated_detector_pattern -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_folded_handwritten_repeat_circuit -- --exact
cargo test -p rstim --test cli_coverage run_analyze_errors_with_fold_loops_preserves_default_for_non_repeat_input -- --exact
```

Expected: PASS on the supported repeated circuit, while non-repeat inputs remain unchanged.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/src/dem_fold.rs rstim/tests/cli_analyze.rs rstim/tests/cross_validate_dem.rs rstim/tests/cli_coverage.rs rstim/tests/dem_ir.rs
git commit -m "feat: fold repeated dem structure when requested"
```

### Task 6: Add flat-vs-folded semantic equivalence checks

**Files:**
- Modify: `rstim/tests/cross_validate_dem.rs`
- Modify: `rstim/tests/cli_coverage.rs`
- Reference: `rstim/src/dem.rs`

**Step 1: Write a failing equivalence test for rstim alone**

In `rstim/tests/cli_coverage.rs`, add a helper that counts semantic error targets plus annotation lines from DEM text. Then add:

```rust
#[test]
fn run_analyze_errors_folded_and_flat_outputs_are_semantically_equivalent() {
    let circuit = "R 0\nREPEAT 4 {\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nR 0\n}";
    let mut flat = Vec::new();
    let mut folded = Vec::new();
    cli::run_analyze_errors_with_all_flags(circuit, false, false, false, false, &mut flat).unwrap();
    cli::run_analyze_errors_with_all_flags(circuit, false, false, false, true, &mut folded).unwrap();
    assert_semantic_dem_text_equivalence(
        &String::from_utf8(flat).unwrap(),
        &String::from_utf8(folded).unwrap(),
    );
}
```

**Step 2: Run the test to verify it fails or is incomplete**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_folded_and_flat_outputs_are_semantically_equivalent -- --exact
```

Expected: FAIL until the equivalence helper handles repeat blocks properly or the folded output is correct.

**Step 3: Add a DEM semantic normalizer for tests**

In `rstim/tests/cross_validate_dem.rs` or `rstim/tests/cli_coverage.rs`, add a helper that expands repeat blocks by parsing DEM text with `DetectorErrorModel::parse(...)`, then recursively collects:

- error target strings with probabilities
- detector annotation lines after effective shifts
- detector / observable counts

Use this helper to compare folded and flat DEM outputs semantically instead of textually.

**Step 4: Run the equivalence test and a representative Stim cross-check**

```bash
cargo test -p rstim --test cli_coverage run_analyze_errors_folded_and_flat_outputs_are_semantically_equivalent -- --exact
cargo test -p rstim --test cross_validate_dem cross_validate_folded_handwritten_repeat_circuit -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/tests/cross_validate_dem.rs rstim/tests/cli_coverage.rs
git commit -m "test: verify folded dem is semantically equivalent to flat output"
```

### Task 7: Verify the full mode matrix on representative circuits

**Files:**
- Modify: `rstim/tests/cross_validate_dem.rs`
- Reference: `docs/plans/2026-03-12-dem-parity-wide-branch-design.md`

**Step 1: Write a single mode-matrix regression over representative circuits**

Add a table-driven test that runs each representative circuit through:

- plain
- decompose
- fold
- decompose + fold

For each mode:

- compare against Stim with the corresponding flags (`--decompose_errors`, `--fold_loops`)
- compare folded-vs-flat semantic equivalence inside rstim where folding is enabled

Use at least:

- one hand-written repeated circuit
- repetition code
- rotated surface code
- one stable color-code generator

**Step 2: Run the mode-matrix test to establish current status**

```bash
cargo test -p rstim --test cross_validate_dem mode_matrix_representative_circuits -- --exact
```

Expected: PASS only when both decomposition and folding parity are stable. If it fails, use the first failing circuit/mode pair to start a new focused unit test instead of broad edits.

**Step 3: Fix any remaining single-root-cause regressions**

Follow the same TDD loop:

- reduce the failure to one focused test
- implement one fix
- rerun the mode matrix

Do not bundle unrelated parity fixes into one commit.

**Step 4: Run the verification commands**

```bash
cargo test -p rstim --test cross_validate_dem
cargo test -p rstim --test cli_analyze
cargo test -p rstim --test cli_coverage
cargo test -p rstim
```

Expected: All commands pass.

**Step 5: Commit**

```bash
git add rstim/tests/cross_validate_dem.rs rstim/tests/cli_analyze.rs rstim/tests/cli_coverage.rs rstim/src/error_analyzer.rs rstim/src/cli.rs rstim/src/dem_fold.rs
git commit -m "test: verify dem parity mode matrix against stim"
```
