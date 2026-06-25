# Random-Window Upper-Bound CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the CSS random-window upper-bound search through `qec-code code css-distance random-window-upper-bound`.

**Architecture:** Add a sibling clap subcommand that mirrors `randomized-upper-bound` input selection and JSON-only output enforcement. The new route constructs `RandomWindowUpperBoundOptions`, calls the existing `random_window_css_upper_bound` library function, and leaves `randomized-upper-bound` unchanged.

**Tech Stack:** Rust 2024, clap derive CLI, qec-code crate, serde JSON, Cargo integration tests.

## Global Constraints

- Command name must be `code css-distance random-window-upper-bound`.
- Required inputs are exactly one of `--code-id <built-in-css-code>`, `--hx <path> --hz <path>`, or `--quantum-tanner-spec <path>`.
- Required options are `--iterations <n>`, `--seed <n>`, and `--json`.
- Optional options are `--restarts <n>` defaulting consistently with the library API and `--target-weight <n>`.
- Successful runs print one JSON object to stdout and nothing to stderr.
- JSON must have `method = "random-window-upper-bound"` and `bound_type = "upper"`.
- Invalid input or missing `--json` exits nonzero with no stdout.
- Missing `--json` stderr must contain `JSON output is required for code css-distance random-window-upper-bound`.
- Existing `randomized-upper-bound` command remains available unchanged.
- Do not make random-window the default for another command.
- Do not add benchmark report generation.

---

## File Structure

- Modify `qec-code/src/cli.rs`: add `RandomWindowUpperBoundCli`, a `CssDistanceCommands` variant, routing, option construction, and CSS input reuse for the new command.
- Modify `qec-code/tests/cli.rs`: add a focused CLI contract test covering built-in, `--hx/--hz`, `--quantum-tanner-spec`, and missing `--json`.

### Task 1: Add Random-Window Upper-Bound CLI Command

**Files:**
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes:
  - `css_distance_input_selection(...) -> Result<CssDistanceInputSelection<'_>, QecError>`
  - `css_code_from_built_in(code_id: &str) -> Result<CssCode, QecError>`
  - `css_code_from_files(hx_path: &PathBuf, hz_path: &PathBuf) -> Result<CssCode, QecError>`
  - `css_code_from_quantum_tanner_spec(path: &PathBuf) -> Result<CssCode, QecError>`
  - `random_window_css_upper_bound(css: &CssCode, options: RandomWindowUpperBoundOptions)`
- Produces:
  - `qec-code code css-distance random-window-upper-bound`
  - JSON result with `method = "random-window-upper-bound"` and `bound_type = "upper"`
  - Missing-JSON error tied to `code css-distance random-window-upper-bound`

- [ ] **Step 1: Write the failing CLI contract test**

Add this test after `css_distance_randomized_upper_bound_rejects_zero_iterations_without_stdout` in `qec-code/tests/cli.rs`:

```rust
#[test]
fn css_distance_random_window_upper_bound_cli_contract() {
    let built_in = run_qec_code(&[
        "code",
        "css-distance",
        "random-window-upper-bound",
        "--code-id",
        "surface_rotated:d=5",
        "--iterations",
        "5000",
        "--restarts",
        "8",
        "--seed",
        "7",
        "--target-weight",
        "5",
        "--json",
    ]);

    assert!(built_in.status.success());
    assert_eq!(built_in.stderr, b"");
    let stdout = String::from_utf8(built_in.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "random-window-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 5);
    assert_eq!(json["witness"]["weight"], 5);

    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let files = Command::new(qec_code_bin())
        .args(["code", "css-distance", "random-window-upper-bound", "--hx"])
        .arg(&hx)
        .arg("--hz")
        .arg(&hz)
        .args([
            "--iterations",
            "500",
            "--restarts",
            "4",
            "--seed",
            "7",
            "--target-weight",
            "3",
            "--json",
        ])
        .output()
        .expect("qec-code binary should run");

    assert!(files.status.success());
    assert_eq!(files.stderr, b"");
    let stdout = String::from_utf8(files.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "random-window-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["upper_bound"], 3);
    assert_eq!(json["witness"]["weight"], 3);

    let spec = quantum_tanner_fixture_path("toric_d4.json");
    let quantum_tanner = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "random-window-upper-bound",
            "--quantum-tanner-spec",
        ])
        .arg(&spec)
        .args([
            "--iterations",
            "1000",
            "--restarts",
            "8",
            "--seed",
            "7",
            "--target-weight",
            "4",
            "--json",
        ])
        .output()
        .expect("qec-code binary should run");

    assert!(quantum_tanner.status.success());
    assert_eq!(quantum_tanner.stderr, b"");
    let stdout = String::from_utf8(quantum_tanner.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "random-window-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert!(json["upper_bound"].as_u64().unwrap() <= 4);
    assert!(json["witness"]["weight"].as_u64().unwrap() <= 4);

    let missing_json = run_qec_code(&[
        "code",
        "css-distance",
        "random-window-upper-bound",
        "--code-id",
        "steane",
        "--iterations",
        "10",
        "--seed",
        "7",
    ]);

    assert!(!missing_json.status.success());
    assert_eq!(missing_json.stdout, b"");
    let stderr = String::from_utf8(missing_json.stderr).unwrap();
    assert!(
        stderr.contains("JSON output is required for code css-distance random-window-upper-bound"),
        "stderr was: {stderr}"
    );
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p qec-code css_distance_random_window_upper_bound_cli_contract -q --offline`

Expected: FAIL before implementation because clap rejects `random-window-upper-bound` as an unrecognized subcommand.

- [ ] **Step 3: Add the CLI command**

In `qec-code/src/cli.rs`, replace the distance-bound import:

```rust
use crate::distance_bound::{
    random_window_css_upper_bound, randomized_css_upper_bound, RandomWindowUpperBoundOptions,
    RandomizedUpperBoundOptions,
};
```

Change `CssDistanceCommands` to include the new variant:

```rust
#[derive(Debug, Subcommand)]
pub enum CssDistanceCommands {
    Exact(ExactCssDistanceCli),
    RandomizedUpperBound(RandomizedUpperBoundCli),
    RandomWindowUpperBound(RandomWindowUpperBoundCli),
}
```

Add the new CLI args type after `RandomizedUpperBoundCli`:

```rust
#[derive(Debug, Args)]
pub struct RandomWindowUpperBoundCli {
    #[arg(long)]
    code_id: Option<String>,
    #[arg(long)]
    hx: Option<PathBuf>,
    #[arg(long)]
    hz: Option<PathBuf>,
    #[arg(long)]
    quantum_tanner_spec: Option<PathBuf>,
    #[arg(long)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    restarts: usize,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    target_weight: Option<usize>,
    #[arg(long)]
    json: bool,
}
```

Update `run_css_distance`:

```rust
fn run_css_distance(command: CssDistanceCommands) -> Result<String, QecError> {
    match command {
        CssDistanceCommands::Exact(options) => run_css_exact_distance(options),
        CssDistanceCommands::RandomizedUpperBound(options) => {
            run_css_randomized_upper_bound(options)
        }
        CssDistanceCommands::RandomWindowUpperBound(options) => {
            run_css_random_window_upper_bound(options)
        }
    }
}
```

Add the runner and input helper after `css_code_from_randomized_upper_bound_cli`:

```rust
fn run_css_random_window_upper_bound(cli: RandomWindowUpperBoundCli) -> Result<String, QecError> {
    const COMMAND: &str = "code css-distance random-window-upper-bound";

    if !cli.json {
        return Err(QecError::JsonOutputRequired { command: COMMAND });
    }

    let css = css_code_from_random_window_upper_bound_cli(&cli)?;
    let options = RandomWindowUpperBoundOptions {
        iterations: cli.iterations,
        restarts: cli.restarts,
        seed: cli.seed,
        target_weight: cli.target_weight,
    };
    let result = random_window_css_upper_bound(&css, options)?;

    serde_json::to_string(&result).map_err(|err| QecError::InvalidCssDistanceInput(err.to_string()))
}

fn css_code_from_random_window_upper_bound_cli(
    cli: &RandomWindowUpperBoundCli,
) -> Result<CssCode, QecError> {
    match css_distance_input_selection(&cli.code_id, &cli.hx, &cli.hz, &cli.quantum_tanner_spec)? {
        CssDistanceInputSelection::CodeId(code_id) => css_code_from_built_in(code_id),
        CssDistanceInputSelection::Files { hx, hz } => css_code_from_files(hx, hz),
        CssDistanceInputSelection::QuantumTannerSpec(spec) => {
            css_code_from_quantum_tanner_spec(spec)
        }
    }
}
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `cargo test -p qec-code css_distance_random_window_upper_bound_cli_contract -q --offline`

Expected: PASS.

- [ ] **Step 5: Run the issue verification command**

Run:

```bash
cargo run -p qec-code --offline -- code css-distance random-window-upper-bound --code-id surface_rotated:d=5 --iterations 5000 --restarts 8 --seed 7 --target-weight 5 --json
```

Expected: exit 0, stderr empty, stdout valid JSON with `method = "random-window-upper-bound"`, `bound_type = "upper"`, `upper_bound = 5`, and `witness.weight = 5`.

- [ ] **Step 6: Run the negative control**

Run:

```bash
cargo run -p qec-code --offline -- code css-distance random-window-upper-bound --code-id steane --iterations 10 --seed 7
```

Expected: nonzero exit, no stdout, and stderr contains `JSON output is required for code css-distance random-window-upper-bound`.

- [ ] **Step 7: Run formatting and full verification**

Run:

```bash
cargo fmt --check
cargo test --offline
```

Expected: both commands PASS. Existing unrelated warnings in `rmatching/tests/coverage.rs` may appear during the full test run.

- [ ] **Step 8: Commit**

Run:

```bash
git add qec-code/src/cli.rs qec-code/tests/cli.rs
git commit -m "feat: expose random-window upper-bound cli"
```
