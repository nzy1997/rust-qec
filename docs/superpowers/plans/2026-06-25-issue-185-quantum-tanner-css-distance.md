# Quantum Tanner CSS Distance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `qec-code` CSS distance commands consume a quantum Tanner spec directly.

**Architecture:** Keep the change inside the existing `qec-code` CLI and exact-result metadata. Add one input-selection helper that routes built-in ids, sparse-row files, and quantum Tanner specs into ordinary `CssCode` construction before calling the existing exact or randomized distance machinery.

**Tech Stack:** Rust 2024, clap, serde, existing `qec-code` CSS, quantum Tanner, exact distance, and randomized upper-bound modules.

## Global Constraints

- Required exact command: `cargo run -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json --json`.
- The exact command must return JSON containing `"status":"completed"` and `"distance":4` for `toric_d4.json`.
- Negative exact command: `cargo run -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json --json`.
- The negative command must exit non-zero and report spec/constructor validation failure before producing a distance result.
- Reuse the existing CSS distance machinery; do not add a separate quantum Tanner distance implementation.
- Do not add new distance algorithms, `rsinter` benchmark flows, decoder integrations, importer tooling, or group-search functionality.

---

## File Structure

- Modify `qec-code/src/cli.rs`: add `--quantum-tanner-spec` to distance CLI structs, route it through a shared input-selection helper, and construct `CssCode` from quantum Tanner checks.
- Modify `qec-code/src/distance_exact.rs`: add exact JSON input metadata for the quantum Tanner spec path.
- Modify `qec-code/tests/cli.rs`: add exact and randomized CLI coverage and update input-validation assertions for the new source list.

---

### Task 1: Quantum Tanner Spec Input For CSS Distance

**Files:**
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/src/distance_exact.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: `quantum_tanner_css_checks(&QuantumTannerSpec) -> Result<QuantumTannerCssChecks>`.
- Produces: `--quantum-tanner-spec <path>` accepted by `code css-distance exact` and `code css-distance randomized-upper-bound`.
- Produces: exact JSON options serialized as `{"input":"quantum_tanner_spec","quantum_tanner_spec":"<path>"}`.

- [ ] **Step 1: Write failing exact CLI tests**

Add this helper near the existing CSS distance tests in `qec-code/tests/cli.rs`:

```rust
fn quantum_tanner_fixture_path(name: &str) -> PathBuf {
    workspace_root().join("qec-code/tests/fixtures/quantum_tanner").join(name)
}
```

Add this passing-case test:

```rust
#[test]
fn code_css_distance_exact_quantum_tanner_spec_returns_exact_json() {
    let spec = quantum_tanner_fixture_path("toric_d4.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--quantum-tanner-spec"])
        .arg(&spec)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["distance"], 4);
    assert_eq!(json["method"], "rstim-ilp-exact");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["witness"]["weight"], 4);
    assert_eq!(json["options"]["input"], "quantum_tanner_spec");
    assert_eq!(json["options"]["quantum_tanner_spec"], spec.display().to_string());
}
```

Add this negative-control test:

```rust
#[test]
fn code_css_distance_exact_quantum_tanner_invalid_spec_fails_before_distance_result() {
    let spec = quantum_tanner_fixture_path("invalid_non_symmetric_a.json");
    let output = Command::new(qec_code_bin())
        .args(["code", "css-distance", "exact", "--quantum-tanner-spec"])
        .arg(spec)
        .arg("--json")
        .output()
        .expect("qec-code binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid quantum Tanner generator set A"),
        "stderr was: {stderr}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stderr).is_err(),
        "stderr should not be a distance result: {stderr}"
    );
}
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test -p qec-code --test cli code_css_distance_exact_quantum_tanner -- --nocapture
```

Expected: clap rejects `--quantum-tanner-spec` because the flag does not exist yet.

- [ ] **Step 3: Add exact result input metadata**

In `qec-code/src/distance_exact.rs`, add this enum variant:

```rust
QuantumTannerSpec { quantum_tanner_spec: String },
```

- [ ] **Step 4: Add CLI fields**

In `qec-code/src/cli.rs`, add this field to both `ExactCssDistanceCli` and `RandomizedUpperBoundCli`:

```rust
#[arg(long)]
quantum_tanner_spec: Option<PathBuf>,
```

- [ ] **Step 5: Add shared source selection and quantum Tanner CSS construction**

Add this enum and helper in `qec-code/src/cli.rs` near the distance helpers:

```rust
enum CssDistanceInputSelection<'a> {
    CodeId(&'a str),
    Files { hx: &'a PathBuf, hz: &'a PathBuf },
    QuantumTannerSpec(&'a PathBuf),
}

fn css_distance_input_selection<'a>(
    code_id: &'a Option<String>,
    hx: &'a Option<PathBuf>,
    hz: &'a Option<PathBuf>,
    quantum_tanner_spec: &'a Option<PathBuf>,
) -> Result<CssDistanceInputSelection<'a>, QecError> {
    let source_count = usize::from(code_id.is_some())
        + usize::from(hx.is_some() || hz.is_some())
        + usize::from(quantum_tanner_spec.is_some());

    if source_count == 0 {
        return Err(QecError::InvalidCssDistanceInput(
            "provide --code-id, --quantum-tanner-spec, or both --hx and --hz".to_owned(),
        ));
    }
    if source_count > 1 {
        return Err(QecError::InvalidCssDistanceInput(
            "use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz".to_owned(),
        ));
    }

    match (
        code_id.as_deref(),
        hx.as_ref(),
        hz.as_ref(),
        quantum_tanner_spec.as_ref(),
    ) {
        (Some(code_id), None, None, None) => Ok(CssDistanceInputSelection::CodeId(code_id)),
        (None, Some(hx), Some(hz), None) => Ok(CssDistanceInputSelection::Files { hx, hz }),
        (None, None, None, Some(spec)) => Ok(CssDistanceInputSelection::QuantumTannerSpec(spec)),
        (None, Some(_), None, None) | (None, None, Some(_), None) => {
            Err(QecError::InvalidCssDistanceInput(
                "--hx and --hz must be provided together".to_owned(),
            ))
        }
        _ => Err(QecError::InvalidCssDistanceInput(
            "use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz".to_owned(),
        )),
    }
}
```

Add this constructor:

```rust
fn css_code_from_quantum_tanner_spec(path: &PathBuf) -> Result<CssCode, QecError> {
    let spec = read_quantum_tanner_spec(path)?;
    let checks = quantum_tanner_css_checks(&spec)?;
    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx)?.to_dense_rows();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz)?.to_dense_rows();
    CssCode::from_hx_hz(hx, hz)
}
```

- [ ] **Step 6: Route exact and randomized commands through the helper**

Change `css_code_and_exact_options_from_cli` to match on `css_distance_input_selection(...)`.
For `QuantumTannerSpec(spec)`, call `css_code_from_quantum_tanner_spec(spec)` and return:

```rust
ExactCssDistanceOptions {
    input: ExactCssDistanceInput::QuantumTannerSpec {
        quantum_tanner_spec: spec.display().to_string(),
    },
}
```

Change `css_code_from_randomized_upper_bound_cli` to use the same helper and call
`css_code_from_quantum_tanner_spec(spec)` for the new source.

- [ ] **Step 7: Update validation assertions and add randomized coverage**

Update existing input-error tests to expect:

```rust
"provide --code-id, --quantum-tanner-spec, or both --hx and --hz"
"use only one input source: --code-id, --quantum-tanner-spec, or --hx/--hz"
```

Add a randomized smoke test:

```rust
#[test]
fn css_distance_randomized_upper_bound_quantum_tanner_spec_outputs_json() {
    let spec = quantum_tanner_fixture_path("toric_d4.json");
    let output = Command::new(qec_code_bin())
        .args([
            "code",
            "css-distance",
            "randomized-upper-bound",
            "--quantum-tanner-spec",
        ])
        .arg(spec)
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

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "randomized-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    assert!(json["upper_bound"].as_u64().unwrap() <= 4);
}
```

- [ ] **Step 8: Verify GREEN**

Run:

```bash
cargo test -p qec-code --test cli code_css_distance_exact_quantum_tanner -- --nocapture
cargo test -p qec-code --test cli css_distance_randomized_upper_bound_quantum_tanner_spec_outputs_json -- --nocapture
```

Expected: the exact tests and randomized smoke test pass.

- [ ] **Step 9: Run issue commands**

Run:

```bash
cargo run -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json --json
cargo run -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json --json
```

Expected: first command exits 0 with completed distance `4`; second command exits non-zero with the invalid generator-set message and no distance JSON.
