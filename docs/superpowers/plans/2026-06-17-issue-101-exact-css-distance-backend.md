# Issue 101 Exact CSS Distance Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a stable `qec-code code css-distance exact` backend that AutoQEC can call as `rstim-ilp-exact`.

**Architecture:** Keep exact-distance solving behind the existing `qec-code::distance::compute_distance` API. Add a small exact-result serialization module, then wire a new `Exact` CLI subcommand beside the existing `randomized-upper-bound` command. Reuse the existing built-in CSS registry, `SparseRowsMatrix` parser, and `CssCode::from_hx_hz` validation.

**Tech Stack:** Rust 2024, `clap` derive CLI, `serde`/`serde_json`, existing `qec-code` CSS and distance modules, optional `distance-ilp-highs` feature for ILP acceptance tests.

## Global Constraints

- The command shape is `qec-code code css-distance exact --code-id <spec> --json`.
- The command shape is `qec-code code css-distance exact --hx <hx.json> --hz <hz.json> --json`.
- `--json` is required for the exact command.
- Input mode is exactly one of `--code-id` or `--hx/--hz`.
- Matrix file input supports only the existing `sparse_rows` JSON wrapper.
- Do not add direct support for AutoQEC's `dense_binary_matrix` artifact format inside `qec-code`.
- Successful exact output must include `status: "completed"`, `method: "rstim-ilp-exact"`, and `bound_type: "exact"`.
- Failure paths must write no completed JSON payload.
- The existing `randomized-upper-bound` command must keep emitting `bound_type: "upper"`.
- Do not change the existing `code steane distance` human-readable command.

---

## File Structure

- Create `qec-code/src/distance_exact.rs`: exact CSS distance result/options/provenance JSON types.
- Modify `qec-code/src/lib.rs`: export `distance_exact`.
- Modify `qec-code/src/cli.rs`: add `CssDistanceCommands::Exact`, parse exact CLI options, build `CssCode`, run `compute_distance`, and serialize `ExactCssDistanceResult`.
- Create `qec-code/tests/distance_exact.rs`: result-contract serialization tests.
- Modify `qec-code/tests/cli.rs`: exact CLI success/error tests and feature-gated known-distance acceptance tests.

## Task 1: Exact Result Contract Types

**Files:**
- Create: `qec-code/src/distance_exact.rs`
- Modify: `qec-code/src/lib.rs`
- Create: `qec-code/tests/distance_exact.rs`

**Interfaces:**
- Consumes: `qec_code::distance::DistanceResult`, `qec_code::distance::LogicalClass`, `qec_code::distance_bound::DistanceBoundWitness`, `qec_code::Pauli`.
- Produces:
  - `qec_code::distance_exact::ExactCssDistanceInput`
  - `qec_code::distance_exact::ExactCssDistanceOptions`
  - `qec_code::distance_exact::ExactCssDistanceProvenance`
  - `qec_code::distance_exact::ExactCssDistanceResult`
  - `ExactCssDistanceResult::completed(distance: DistanceResult, options: ExactCssDistanceOptions) -> ExactCssDistanceResult`

- [ ] **Step 1: Write the failing result-contract tests**

Create `qec-code/tests/distance_exact.rs` with:

```rust
use qec_code::distance::{DistanceResult, LogicalClass};
use qec_code::distance_exact::{
    ExactCssDistanceInput, ExactCssDistanceOptions, ExactCssDistanceProvenance,
    ExactCssDistanceResult,
};
use qec_code::Pauli;

fn sample_distance_result() -> DistanceResult {
    let witness = Pauli::from_xz_bits(vec![1, 0, 1], vec![0, 0, 0]).unwrap();
    DistanceResult {
        distance: 2,
        witness,
        logical_class: LogicalClass::XLike,
    }
}

#[test]
fn exact_css_distance_result_serializes_completed_contract() {
    let result = ExactCssDistanceResult::completed(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "surface_rotated:d=3".to_owned(),
            },
        },
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 2);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["logical_class"], "x_like");
    assert_eq!(json["witness"]["x"], serde_json::json!([1, 0, 1]));
    assert_eq!(json["witness"]["z"], serde_json::json!([0, 0, 0]));
    assert_eq!(json["witness"]["weight"], 2);
    assert_eq!(json["options"]["input"], "code_id");
    assert_eq!(json["options"]["code_id"], "surface_rotated:d=3");
    assert_eq!(json["provenance"]["tool"], "qec-code");
    assert_eq!(json["provenance"]["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["provenance"]["method_revision"], 1);
}

#[test]
fn exact_css_distance_file_options_serialize_input_paths() {
    let result = ExactCssDistanceResult::completed(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::Files {
                hx: "input/hx.json".to_owned(),
                hz: "input/hz.json".to_owned(),
            },
        },
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["options"]["input"], "files");
    assert_eq!(json["options"]["hx"], "input/hx.json");
    assert_eq!(json["options"]["hz"], "input/hz.json");
}

#[test]
fn exact_css_distance_provenance_uses_current_package_version() {
    let provenance = ExactCssDistanceProvenance::current();

    assert_eq!(provenance.tool, "qec-code");
    assert_eq!(provenance.tool_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(provenance.method_revision, 1);
}
```

- [ ] **Step 2: Run the result-contract tests to verify they fail**

Run:

```bash
cargo test -p qec-code --test distance_exact exact_css_distance_ -q
```

Expected: FAIL with an unresolved import for `qec_code::distance_exact`.

- [ ] **Step 3: Implement the exact result module**

Create `qec-code/src/distance_exact.rs` with:

```rust
use crate::distance::{DistanceResult, LogicalClass};
use crate::distance_bound::DistanceBoundWitness;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExactCssDistanceMethod {
    RstimIlpExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactDistanceBoundType {
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactCssDistanceStatus {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum ExactCssDistanceInput {
    CodeId { code_id: String },
    Files { hx: String, hz: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceOptions {
    #[serde(flatten)]
    pub input: ExactCssDistanceInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceProvenance {
    pub tool: String,
    pub tool_version: String,
    pub method_revision: u32,
}

impl ExactCssDistanceProvenance {
    pub fn current() -> Self {
        Self {
            tool: "qec-code".to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            method_revision: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceResult {
    pub status: ExactCssDistanceStatus,
    pub distance: usize,
    pub method: ExactCssDistanceMethod,
    pub bound_type: ExactDistanceBoundType,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: ExactCssDistanceOptions,
    pub provenance: ExactCssDistanceProvenance,
}

impl ExactCssDistanceResult {
    pub fn completed(
        distance: DistanceResult,
        options: ExactCssDistanceOptions,
    ) -> Self {
        Self {
            status: ExactCssDistanceStatus::Completed,
            distance: distance.distance,
            method: ExactCssDistanceMethod::RstimIlpExact,
            bound_type: ExactDistanceBoundType::Exact,
            logical_class: distance.logical_class,
            witness: DistanceBoundWitness::from_pauli(&distance.witness),
            options,
            provenance: ExactCssDistanceProvenance::current(),
        }
    }
}
```

- [ ] **Step 4: Export the module**

Add this line to `qec-code/src/lib.rs` after `pub mod distance_bound;`:

```rust
pub mod distance_exact;
```

- [ ] **Step 5: Run the result-contract tests to verify they pass**

Run:

```bash
cargo test -p qec-code --test distance_exact exact_css_distance_ -q
```

Expected: PASS. The output should include `3 passed`.

- [ ] **Step 6: Commit the result contract**

Run:

```bash
git add qec-code/src/distance_exact.rs qec-code/src/lib.rs qec-code/tests/distance_exact.rs
git commit -m "feat: add exact css distance result contract"
```

Expected: commit succeeds with only those three paths staged.

## Task 2: Exact CSS Distance CLI

**Files:**
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes:
  - `ExactCssDistanceInput`
  - `ExactCssDistanceOptions`
  - `ExactCssDistanceResult::completed(distance, options)`
  - `css_code_from_built_in(code_id: &str) -> Result<CssCode, QecError>`
  - `css_code_from_files(hx_path: &PathBuf, hz_path: &PathBuf) -> Result<CssCode, QecError>`
- Produces:
  - CLI command `qec-code code css-distance exact --code-id <spec> --json`
  - CLI command `qec-code code css-distance exact --hx <path> --hz <path> --json`
  - function `run_css_exact_distance(cli: ExactCssDistanceCli) -> Result<String, QecError>`
  - function `css_code_and_exact_options_from_cli(cli: &ExactCssDistanceCli) -> Result<(CssCode, ExactCssDistanceOptions), QecError>`

- [ ] **Step 1: Add failing exact CLI tests**

Append these tests to `qec-code/tests/cli.rs`:

```rust
#[test]
fn code_css_distance_exact_code_id_returns_exact_json() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 3);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 3);
    assert_eq!(json["options"]["input"], "code_id");
    assert_eq!(json["options"]["code_id"], "steane");
    assert_eq!(json["provenance"]["tool"], "qec-code");
}

#[test]
fn code_css_distance_exact_hx_hz_files_return_exact_json() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(&hx)
        .arg("--hz")
        .arg(&hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 3);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 3);
    assert_eq!(json["options"]["input"], "files");
    assert_eq!(json["options"]["hx"], hx.display().to_string());
    assert_eq!(json["options"]["hz"], hz.display().to_string());
}

#[test]
fn code_css_distance_exact_requires_json_flag() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("JSON output is required for code css-distance exact"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_code_id_and_file_input_together() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let hz = workspace_root().join("rsinter/tests/fixtures/css/steane_hz.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--code-id", "steane", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("use either --code-id or --hx/--hz, not both"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_missing_matrix_pair() {
    let hx = workspace_root().join("rsinter/tests/fixtures/css/steane_hx.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(hx)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--hx and --hz must be provided together"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_missing_input_source() {
    let output = run_qec_code(&["code", "css-distance", "exact", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("provide --code-id or both --hx and --hz"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_mismatched_file_widths() {
    let dir = tempdir().unwrap();
    let hx = write_matrix_file(
        dir.path(),
        "hx.json",
        r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,1]]}"#,
    );
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":4,"rows":[[2,3]]}"#,
    );
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("hx width 3 does not match hz width 4"),
        "stderr was: {stderr}"
    );
}

#[test]
fn code_css_distance_exact_rejects_non_commuting_css_before_solving() {
    let dir = tempdir().unwrap();
    let hx = write_matrix_file(
        dir.path(),
        "hx.json",
        r#"{"format":"sparse_rows","num_cols":1,"rows":[[0]]}"#,
    );
    let hz = write_matrix_file(
        dir.path(),
        "hz.json",
        r#"{"format":"sparse_rows","num_cols":1,"rows":[[0]]}"#,
    );
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--hx"])
        .arg(hx)
        .arg("--hz")
        .arg(hz)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("CSS X/Z checks are not orthogonal"),
        "stderr was: {stderr}"
    );
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn code_css_distance_exact_surface_rotated_known_distances_with_ilp() {
    for (code_id, expected_distance) in [
        ("surface_rotated:d=3", 3),
        ("surface_rotated:d=5", 5),
        ("surface_rotated:d=7", 7),
    ] {
        let output = run_qec_code(&[
            "code",
            "css-distance",
            "exact",
            "--code-id",
            code_id,
            "--json",
        ]);

        assert!(output.status.success(), "case {code_id} failed");
        assert_eq!(output.stderr, b"", "case {code_id} printed stderr");

        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

        assert_eq!(json["distance"], expected_distance);
        assert_eq!(json["bound_type"], "exact");
        assert_eq!(json["method"], "rstim-ilp-exact");
        assert_eq!(json["witness"]["weight"], expected_distance);
    }
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn code_css_distance_exact_bb72_known_distance_with_ilp() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "bb72",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["distance"], 6);
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["witness"]["weight"], 6);
}
```

- [ ] **Step 2: Run the exact CLI tests to verify they fail**

Run:

```bash
cargo test -p qec-code --test cli code_css_distance_exact_ -q
```

Expected: FAIL. The first failing cases should report that `exact` is not a recognized `css-distance` subcommand.

- [ ] **Step 3: Import exact result types in the CLI**

In `qec-code/src/cli.rs`, add this import beside the existing distance imports:

```rust
use crate::distance_exact::{
    ExactCssDistanceInput, ExactCssDistanceOptions, ExactCssDistanceResult,
};
```

Keep this existing import because `code steane distance` still uses it:

```rust
use crate::distance::compute_distance;
```

- [ ] **Step 4: Add the exact CLI args and subcommand**

Replace the current `CssDistanceCommands` enum and nearby CLI structs with this shape, preserving the existing `RandomizedUpperBoundCli` fields:

```rust
#[derive(Debug, Subcommand)]
pub enum CssDistanceCommands {
    Exact(ExactCssDistanceCli),
    RandomizedUpperBound(RandomizedUpperBoundCli),
}

#[derive(Debug, Args)]
pub struct ExactCssDistanceCli {
    #[arg(long)]
    code_id: Option<String>,
    #[arg(long)]
    hx: Option<PathBuf>,
    #[arg(long)]
    hz: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
pub struct RandomizedUpperBoundCli {
    #[arg(long)]
    code_id: Option<String>,
    #[arg(long)]
    hx: Option<PathBuf>,
    #[arg(long)]
    hz: Option<PathBuf>,
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

- [ ] **Step 5: Dispatch the exact subcommand**

Replace `run_css_distance` in `qec-code/src/cli.rs` with:

```rust
fn run_css_distance(command: CssDistanceCommands) -> Result<String, QecError> {
    match command {
        CssDistanceCommands::Exact(options) => run_css_exact_distance(options),
        CssDistanceCommands::RandomizedUpperBound(options) => {
            run_css_randomized_upper_bound(options)
        }
    }
}
```

- [ ] **Step 6: Implement exact CLI execution**

Add these functions above `run_css_randomized_upper_bound`:

```rust
fn run_css_exact_distance(cli: ExactCssDistanceCli) -> Result<String, QecError> {
    const COMMAND: &str = "code css-distance exact";

    if !cli.json {
        return Err(QecError::JsonOutputRequired { command: COMMAND });
    }

    let (css, options) = css_code_and_exact_options_from_cli(&cli)?;
    let distance = compute_distance(css.code())?;
    let result = ExactCssDistanceResult::completed(distance, options);

    serde_json::to_string(&result).map_err(|err| QecError::InvalidCssDistanceInput(err.to_string()))
}

fn css_code_and_exact_options_from_cli(
    cli: &ExactCssDistanceCli,
) -> Result<(CssCode, ExactCssDistanceOptions), QecError> {
    match (&cli.code_id, &cli.hx, &cli.hz) {
        (Some(code_id), None, None) => Ok((
            css_code_from_built_in(code_id)?,
            ExactCssDistanceOptions {
                input: ExactCssDistanceInput::CodeId {
                    code_id: code_id.clone(),
                },
            },
        )),
        (None, Some(hx), Some(hz)) => Ok((
            css_code_from_files(hx, hz)?,
            ExactCssDistanceOptions {
                input: ExactCssDistanceInput::Files {
                    hx: hx.display().to_string(),
                    hz: hz.display().to_string(),
                },
            },
        )),
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "use either --code-id or --hx/--hz, not both".to_owned(),
        )),
        (None, Some(_), None) | (None, None, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "--hx and --hz must be provided together".to_owned(),
        )),
        (None, None, None) => Err(QecError::InvalidCssDistanceInput(
            "provide --code-id or both --hx and --hz".to_owned(),
        )),
    }
}
```

- [ ] **Step 7: Run the default exact CLI tests**

Run:

```bash
cargo test -p qec-code --test cli code_css_distance_exact_ -q
```

Expected: PASS for non-feature-gated exact CLI tests. Feature-gated ILP tests are not compiled in this command.

- [ ] **Step 8: Run the feature-gated exact CLI tests**

Run:

```bash
cargo test -p qec-code --features distance-ilp-highs --test cli code_css_distance_exact_ -q
```

Expected: PASS. This includes `surface_rotated:d=3/5/7` and `bb72`.

- [ ] **Step 9: Verify randomized output stayed upper-bound-only**

Run:

```bash
cargo test -p qec-code --test cli css_distance_randomized_upper_bound_code_id_outputs_json -q
```

Expected: PASS, with the existing test still asserting `bound_type == "upper"`.

- [ ] **Step 10: Commit the exact CLI**

Run:

```bash
git add qec-code/src/cli.rs qec-code/tests/cli.rs
git commit -m "feat: expose exact css distance cli"
```

Expected: commit succeeds with only those two paths staged.

## Task 3: Final Verification

**Files:**
- Test only: `qec-code/src/distance_exact.rs`
- Test only: `qec-code/src/cli.rs`
- Test only: `qec-code/tests/distance_exact.rs`
- Test only: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: all interfaces from Tasks 1 and 2.
- Produces: verified branch state ready for review or PR creation.

- [ ] **Step 1: Run the exact result test file**

Run:

```bash
cargo test -p qec-code --test distance_exact -q
```

Expected: PASS. The output should include `3 passed`.

- [ ] **Step 2: Run the focused exact CLI tests without ILP**

Run:

```bash
cargo test -p qec-code --test cli code_css_distance_exact_ -q
```

Expected: PASS for all non-feature-gated exact CLI tests.

- [ ] **Step 3: Run the focused exact CLI tests with ILP**

Run:

```bash
cargo test -p qec-code --features distance-ilp-highs --test cli code_css_distance_exact_ -q
```

Expected: PASS, including `bb72` distance `6`.

- [ ] **Step 4: Run existing exact-distance ILP regression tests**

Run:

```bash
cargo test -p qec-code --features distance-ilp-highs --test logical_distance -q
```

Expected: PASS. This confirms `compute_distance` still works through the ILP path.

- [ ] **Step 5: Run the package test suite**

Run:

```bash
cargo test -p qec-code -q
```

Expected: PASS.

- [ ] **Step 6: Run package formatting check**

Run:

```bash
cargo fmt --check --package qec-code
```

Expected: PASS with no diff.

- [ ] **Step 7: Inspect git status**

Run:

```bash
git status --short
```

Expected: no unstaged source or test changes. Documentation changes from the plan/spec may remain committed before implementation starts.
