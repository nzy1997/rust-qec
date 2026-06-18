# Issue 91 Rbposd LSD Runner Params Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add typed LSD-specific `rbposd` runner parameter parsing in `rsinter` while keeping actual LSD DEM decoding deferred to issue #92.

**Architecture:** Extend the benchmark registry so `lsd_method` and `lsd_order` are recognized as `rbposd` decoder params. Refactor `RbposdRunnerParams` into an explicit OSD/LSD decoder-family shape, preserving OSD behavior and normalized params. Valid LSD params pass preflight, but an attempted LSD benchmark run returns a clear #92 boundary error before artifacts are written.

**Tech Stack:** Rust 2024 workspace; `rsinter` crate; TOML benchmark specs; JSONL benchmark artifacts; `cargo test`; `cargo fmt`.

## Global Constraints

- Do not add an LSD DEM adapter in #91.
- Do not run LSD-backed benchmarks end to end.
- Do not record LSD params in successful result rows yet; #93 owns result-row recording once #92 can execute the path.
- Do not update smoke or full benchmark specs.
- Do not add BP method or schedule expansion; #94 through #97 own that work.
- Do not add new public `rbposd` LSD methods or algorithm behavior.
- Preserve existing OSD specs and normalized OSD result params.
- Do not silently fall back to the OSD DEM adapter for LSD configs.
- Unknown, ill-typed, unsupported, and mixed-family params must fail before artifacts are written.

---

## File Structure

- `rsinter/src/bench/registry.rs`
  - Add `lsd_method` and `lsd_order` to the `rbposd` decoder-param allowlist.
- `rsinter/src/bench/runners/rbposd.rs`
  - Replace the single `DecoderConfig` parser output with a private `RbposdDecoderFamily` enum.
  - Parse shared BP params once.
  - Parse default OSD params when LSD keys are absent.
  - Parse `LsdConfig` when LSD keys are present.
  - Reject mixed OSD/LSD keys.
  - Keep OSD execution unchanged.
  - Return a clear #92 error for LSD execution.
- `rsinter/tests/bench_registry.rs`
  - Add a registry expansion test proving LSD params are carried as decoder params without multiplying benchmark points.
- `rsinter/tests/bench_runner_wrappers.rs`
  - Add direct preflight and parser-boundary tests for supported LSD params and unsupported LSD values.
- `rsinter/tests/bench_run.rs`
  - Add benchmark-level negative controls that prove unknown LSD-like keys, mixed OSD/LSD keys, and attempted LSD execution leave no artifacts.

---

### Task 1: Carry LSD Params Through The Benchmark Registry

**Files:**
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/tests/bench_registry.rs`

**Interfaces:**
- Consumes: `expand_runner_points_for_runner(runner_name: &str, params: &BTreeMap<String, toml::Value>) -> Result<Vec<BenchCasePoint>, String>`.
- Produces: `BenchCasePoint.decoder_params` entries for `lsd_method` and `lsd_order` when `runner_name == "rbposd"`.

- [ ] **Step 1: Write the failing registry test**

Add this test in `rsinter/tests/bench_registry.rs` immediately after `expand_runner_points_for_runner_carries_decoder_params_without_multiplying_points`:

```rust
#[test]
fn expand_runner_points_accepts_rbposd_lsd_params() {
    let mut params = valid_runner_params();
    params.insert(
        "lsd_method".into(),
        toml::Value::String("localized_statistics".into()),
    );
    params.insert("lsd_order".into(), toml::Value::Integer(1));

    let points = expand_runner_points_for_runner("rbposd", &params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]
            .decoder_params
            .get("lsd_method")
            .and_then(toml::Value::as_str),
        Some("localized_statistics")
    );
    assert_eq!(
        points[0]
            .decoder_params
            .get("lsd_order")
            .and_then(toml::Value::as_integer),
        Some(1)
    );
    assert_eq!(points[0].input_type, "surface_rotated_memory_x");
    assert_eq!(points[0].distance, Some(3));
    assert_eq!(points[0].rounds, 1);
    assert_eq!(points[0].p, 0.002);
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```bash
cargo test -p rsinter expand_runner_points_accepts_rbposd_lsd_params
```

Expected: FAIL with this error from the test body:

```text
called `Result::unwrap()` on an `Err` value: "unknown rbposd runner param: lsd_method"
```

- [ ] **Step 3: Add the LSD keys to the `rbposd` decoder-param allowlist**

In `rsinter/src/bench/registry.rs`, replace the `rbposd` branch inside `is_decoder_param_key` with:

```rust
        "rbposd" => matches!(
            key,
            "bp_algorithm"
                | "bp_iters"
                | "max_bp_iterations"
                | "early_stop"
                | "osd_method"
                | "osd_order"
                | "lsd_method"
                | "lsd_order"
        ),
```

- [ ] **Step 4: Run the focused test and the registry suite**

Run:

```bash
cargo test -p rsinter expand_runner_points_accepts_rbposd_lsd_params
cargo test -p rsinter --test bench_registry
```

Expected: both commands pass.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add rsinter/src/bench/registry.rs rsinter/tests/bench_registry.rs
git commit -m "feat: accept rbposd lsd runner params"
```

---

### Task 2: Parse Typed OSD And LSD Rbposd Runner Params

**Files:**
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

**Interfaces:**
- Consumes: `rbposd::DecoderConfig`, `rbposd::LsdConfig`, `rbposd::LsdMethod`, `optional_bool`, `optional_string`, and `optional_usize`.
- Produces: private `RbposdDecoderFamily` enum and private `RbposdRunnerParams { bp_config, decoder, normalized }`.
- Produces: `RbposdRunner::preflight_point(&BenchCasePoint) -> Result<(), String>` accepting valid LSD params and rejecting unsupported/mixed values.

- [ ] **Step 1: Add helper imports for direct runner tests**

In `rsinter/tests/bench_runner_wrappers.rs`, replace the top imports with:

```rust
use std::collections::BTreeMap;

use rsinter::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use rsinter::bench::runners::rbposd::RbposdRunner;
use rsinter::bench::runners::rilpqec::RilpqecRunner;
```

- [ ] **Step 2: Add a shared point builder for wrapper tests**

Add this helper near the top of `rsinter/tests/bench_runner_wrappers.rs`, before the first test:

```rust
fn rbposd_point_with_decoder_params(
    decoder_params: BTreeMap<String, toml::Value>,
) -> BenchCasePoint {
    BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
        seed: 12_345,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots: 0,
        max_errors: 2,
        max_wall_seconds: None,
        batch_size: 4,
        decoder_params,
    }
}
```

- [ ] **Step 3: Update the existing rbposd wrapper test to use the helper**

In `rbposd_runner_handles_zero_shot_benchmark_points`, replace the manual `BenchCasePoint` initializer with:

```rust
    let point = rbposd_point_with_decoder_params(BTreeMap::new());
```

Leave the rest of the test unchanged.

- [ ] **Step 4: Write failing direct preflight and parser-boundary tests**

Add these tests in `rsinter/tests/bench_runner_wrappers.rs` after `rbposd_runner_handles_zero_shot_benchmark_points`:

```rust
#[test]
fn rbposd_runner_preflight_accepts_lsd_params() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        (
            "lsd_method".into(),
            toml::Value::String("localized_statistics".into()),
        ),
        ("lsd_order".into(), toml::Value::Integer(1)),
    ]));

    runner.preflight_point(&point).unwrap();
}

#[test]
fn rbposd_runner_preflight_defaults_lsd_method_when_order_is_set() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(0),
    )]));

    runner.preflight_point(&point).unwrap();
}

#[test]
fn rbposd_runner_preflight_defaults_lsd_order_when_method_is_set() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_method".into(),
        toml::Value::String("localized_statistics".into()),
    )]));

    runner.preflight_point(&point).unwrap();
}

#[test]
fn rbposd_runner_preflight_rejects_unsupported_lsd_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_method".into(),
        toml::Value::String("unknown_method".into()),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(
        err,
        "rbposd lsd_method must be \"localized_statistics\", got \"unknown_method\""
    );
}

#[test]
fn rbposd_runner_preflight_rejects_unsupported_lsd_order() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(2),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "rbposd lsd_order must be <= 1, got 2");
}

#[test]
fn rbposd_runner_preflight_rejects_negative_lsd_order() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Integer(-1),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "lsd_order must be non-negative");
}

#[test]
fn rbposd_runner_preflight_rejects_non_integer_lsd_order() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "lsd_order".into(),
        toml::Value::Float(1.0),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "lsd_order must be an integer");
}

#[test]
fn rbposd_runner_preflight_rejects_mixed_osd_and_lsd_params() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        ("osd_order".into(), toml::Value::Integer(10)),
        ("lsd_order".into(), toml::Value::Integer(1)),
    ]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(err, "rbposd params must not mix OSD and LSD decoder params");
}
```

- [ ] **Step 5: Run the direct preflight tests and confirm intended failures**

Run:

```bash
cargo test -p rsinter --test bench_runner_wrappers rbposd_runner_preflight
```

Expected: the new LSD tests fail because `lsd_method` and `lsd_order` are not parsed by `RbposdRunnerParams` yet. The mixed-param test may fail with `osd_order` accepted and `lsd_order` ignored depending on Task 1 state; either failure is acceptable before implementation.

- [ ] **Step 6: Import LSD config types in the runner**

In `rsinter/src/bench/runners/rbposd.rs`, replace:

```rust
use rbposd::DecoderConfig;
```

with:

```rust
use rbposd::{DecoderConfig, LsdConfig, LsdMethod};
```

- [ ] **Step 7: Replace the runner param structs with typed family structs**

In `rsinter/src/bench/runners/rbposd.rs`, replace the current `RbposdRunnerParams` struct with:

```rust
struct RbposdRunnerParams {
    bp_config: DecoderConfig,
    decoder: RbposdDecoderFamily,
    normalized: ParamMap,
}

enum RbposdDecoderFamily {
    Osd {
        osd_method: String,
        osd_order: usize,
    },
    Lsd {
        lsd_method: String,
        lsd_order: usize,
        lsd_config: LsdConfig,
    },
}
```

- [ ] **Step 8: Replace `RbposdRunnerParams::parse` with typed parsing**

In `rsinter/src/bench/runners/rbposd.rs`, replace the entire `fn parse(...)` body with:

```rust
    fn parse(params: &BTreeMap<String, Value>) -> Result<Self, String> {
        let mut bp_config = DecoderConfig::default();
        let bp_algorithm =
            optional_string(params, "bp_algorithm")?.unwrap_or_else(|| "min_sum".to_string());
        if bp_algorithm != "min_sum" {
            return Err(format!(
                "rbposd bp_algorithm must be \"min_sum\", got \"{bp_algorithm}\""
            ));
        }

        let bp_iters = optional_usize(params, "bp_iters")?;
        let max_bp_iterations = optional_usize(params, "max_bp_iterations")?;
        let bp_iters = match (bp_iters, max_bp_iterations) {
            (Some(_), Some(_)) => {
                return Err(
                    "rbposd params must not set both bp_iters and max_bp_iterations".into(),
                );
            }
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => bp_config.max_bp_iterations,
        };
        bp_config.max_bp_iterations = bp_iters;
        bp_config.early_stop = optional_bool(params, "early_stop")?.unwrap_or(bp_config.early_stop);

        let has_lsd_params = params.contains_key("lsd_method") || params.contains_key("lsd_order");
        let has_osd_params = params.contains_key("osd_method") || params.contains_key("osd_order");
        if has_lsd_params && has_osd_params {
            return Err("rbposd params must not mix OSD and LSD decoder params".into());
        }

        if has_lsd_params {
            let lsd_method = optional_string(params, "lsd_method")?
                .unwrap_or_else(|| "localized_statistics".to_string());
            if lsd_method != "localized_statistics" {
                return Err(format!(
                    "rbposd lsd_method must be \"localized_statistics\", got \"{lsd_method}\""
                ));
            }
            let lsd_order =
                optional_usize(params, "lsd_order")?.unwrap_or(LsdConfig::default().lsd_order);
            if lsd_order > 1 {
                return Err(format!("rbposd lsd_order must be <= 1, got {lsd_order}"));
            }
            let lsd_config = LsdConfig {
                method: LsdMethod::LocalizedStatistics,
                lsd_order,
            };
            return Ok(Self {
                bp_config,
                decoder: RbposdDecoderFamily::Lsd {
                    lsd_method: lsd_method.clone(),
                    lsd_order,
                    lsd_config,
                },
                normalized: ParamMap::from_pairs([
                    ("bp_algorithm", serde_json::json!(bp_algorithm)),
                    ("bp_iters", serde_json::json!(bp_config.max_bp_iterations)),
                    ("early_stop", serde_json::json!(bp_config.early_stop)),
                    ("lsd_method", serde_json::json!(lsd_method)),
                    ("lsd_order", serde_json::json!(lsd_order)),
                ]),
            });
        }

        let osd_method = optional_string(params, "osd_method")?
            .unwrap_or_else(|| "combination_sweep".to_string());
        if osd_method != "combination_sweep" {
            return Err(format!(
                "rbposd osd_method must be \"combination_sweep\", got \"{osd_method}\""
            ));
        }
        bp_config.osd_order = optional_usize(params, "osd_order")?.unwrap_or(bp_config.osd_order);

        Ok(Self {
            bp_config,
            decoder: RbposdDecoderFamily::Osd {
                osd_method: osd_method.clone(),
                osd_order: bp_config.osd_order,
            },
            normalized: ParamMap::from_pairs([
                ("bp_algorithm", serde_json::json!(bp_algorithm)),
                ("bp_iters", serde_json::json!(bp_config.max_bp_iterations)),
                ("early_stop", serde_json::json!(bp_config.early_stop)),
                ("osd_method", serde_json::json!(osd_method)),
                ("osd_order", serde_json::json!(bp_config.osd_order)),
            ]),
        })
    }
```

- [ ] **Step 9: Update OSD execution to use `bp_config`**

In `RbposdRunner::run_point`, replace:

```rust
        let decoder = RbposdDemDecoder::new(params.config);
```

with:

```rust
        let decoder = RbposdDemDecoder::new(params.bp_config);
```

The LSD execution branch is added in Task 3. This intermediate state leaves `params.decoder` unread, so Rust reports an unused-field warning during Task 2; Task 3 consumes `params.decoder` in `run_point` before final verification.

- [ ] **Step 10: Run the direct preflight tests and OSD wrapper test**

Run:

```bash
cargo test -p rsinter --test bench_runner_wrappers rbposd_runner_preflight
cargo test -p rsinter --test bench_runner_wrappers rbposd_runner_handles_zero_shot_benchmark_points
```

Expected: all commands pass.

- [ ] **Step 11: Run formatting**

Run:

```bash
cargo fmt --check --package rsinter
```

Expected: PASS. If formatting fails, run `cargo fmt --package rsinter`, then rerun the check.

- [ ] **Step 12: Commit Task 2**

Run:

```bash
git add rsinter/src/bench/runners/rbposd.rs rsinter/tests/bench_runner_wrappers.rs
git commit -m "feat: parse typed rbposd lsd params"
```

---

### Task 3: Enforce LSD Execution Boundary And Artifact-Free Failures

**Files:**
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `RbposdDecoderFamily` from Task 2.
- Produces: `RbposdRunner::run_point` returning `Err("rbposd LSD DEM decoding is not implemented yet; see issue #92")` for LSD configs.
- Produces: benchmark-level negative controls proving failed LSD-related runs do not create runner artifact directories.

- [ ] **Step 1: Write benchmark-level helper for issue 91 specs**

Add this helper near the other benchmark-test helpers in `rsinter/tests/bench_run.rs`, before the new issue 91 tests:

```rust
fn issue91_surface_spec(extra_params: &str) -> String {
    format!(
        r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_lsd"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
{extra_params}

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{{runner}}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#
    )
}
```

- [ ] **Step 2: Write the unknown LSD-like key negative-control test**

Add this test in `rsinter/tests/bench_run.rs` near `rbposd_benchmark_rejects_unknown_decoder_param_without_results`:

```rust
#[test]
fn rbposd_runner_rejects_unknown_lsd_param_without_artifacts() {
    let spec_text = issue91_surface_spec("bogus_lsd = 1");
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus_lsd");
    assert!(!dir.path().join("rbposd_lsd").exists());
}
```

- [ ] **Step 3: Write the mixed-family negative-control test**

Add this test after the unknown LSD-like key test:

```rust
#[test]
fn rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts() {
    let spec_text = issue91_surface_spec(
        r#"
osd_order = 10
lsd_order = 1
"#,
    );
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "rbposd params must not mix OSD and LSD decoder params");
    assert!(!dir.path().join("rbposd_lsd").exists());
}
```

- [ ] **Step 4: Write the LSD execution-boundary artifact test**

Add this test after the mixed-family negative-control test:

```rust
#[test]
fn rbposd_lsd_run_fails_without_silent_osd_fallback_or_artifacts() {
    let spec_text = issue91_surface_spec(
        r#"
lsd_method = "localized_statistics"
lsd_order = 1
"#,
    );
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(
        err,
        "rbposd LSD DEM decoding is not implemented yet; see issue #92"
    );
    assert!(!dir.path().join("rbposd_lsd").exists());
}
```

- [ ] **Step 5: Run the new benchmark tests and confirm the execution-boundary test fails**

Run:

```bash
cargo test -p rsinter rbposd_runner_rejects_unknown_lsd_param_without_artifacts
cargo test -p rsinter rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts
cargo test -p rsinter rbposd_lsd_run_fails_without_silent_osd_fallback_or_artifacts
```

Expected:

- The unknown-key test passes after Task 1.
- The mixed-family test passes after Task 2.
- The LSD execution-boundary test fails because `run_point` still constructs `RbposdDemDecoder` for every parsed param family.

- [ ] **Step 6: Add explicit OSD/LSD execution branching**

In `rsinter/src/bench/runners/rbposd.rs`, replace `RbposdRunner::run_point` with:

```rust
    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let params = RbposdRunnerParams::parse(&point.decoder_params)?;
        match &params.decoder {
            RbposdDecoderFamily::Osd { .. } => {
                let decoder = RbposdDemDecoder::new(params.bp_config);
                run_decoder_point_with_dem_mode(
                    self.name(),
                    &decoder,
                    point,
                    ctx,
                    &params.normalized,
                    DemBuildMode::Raw,
                )
            }
            RbposdDecoderFamily::Lsd { .. } => {
                Err("rbposd LSD DEM decoding is not implemented yet; see issue #92".into())
            }
        }
    }
```

- [ ] **Step 7: Run the new benchmark tests**

Run:

```bash
cargo test -p rsinter rbposd_runner_rejects_unknown_lsd_param_without_artifacts
cargo test -p rsinter rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts
cargo test -p rsinter rbposd_lsd_run_fails_without_silent_osd_fallback_or_artifacts
```

Expected: all three commands pass.

- [ ] **Step 8: Run all issue-named verification commands**

Run:

```bash
cargo test -p rsinter expand_runner_points_accepts_rbposd_lsd_params
cargo test -p rsinter rbposd_runner_preflight_accepts_lsd_params
cargo test -p rsinter rbposd_runner_rejects_unknown_lsd_param_without_artifacts
cargo test -p rsinter rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts
```

Expected: all commands pass.

- [ ] **Step 9: Run the relevant rsinter suites**

Run:

```bash
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_runner_wrappers
cargo test -p rsinter --test bench_run
```

Expected: all commands pass.

- [ ] **Step 10: Run formatting and diff checks**

Run:

```bash
cargo fmt --check --package rsinter
git diff --check
```

Expected: both commands pass. If formatting fails, run `cargo fmt --package rsinter`, then rerun the checks.

- [ ] **Step 11: Commit Task 3**

Run:

```bash
git add rsinter/src/bench/runners/rbposd.rs rsinter/tests/bench_run.rs
git commit -m "test: enforce rbposd lsd runner boundary"
```

---

## Final Verification

After all tasks are complete, run:

```bash
cargo test -p rsinter expand_runner_points_accepts_rbposd_lsd_params
cargo test -p rsinter rbposd_runner_preflight_accepts_lsd_params
cargo test -p rsinter rbposd_runner_rejects_unknown_lsd_param_without_artifacts
cargo test -p rsinter rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_runner_wrappers
cargo test -p rsinter --test bench_run
cargo fmt --check --package rsinter
git diff --check
```

All commands must pass before marking issue #91 complete.
