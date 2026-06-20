# Issue 96 rbposd BP Method Schedule rsinter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `rsinter` so `rbposd` benchmark specs accept BP method and schedule parameters, pass them into typed `rbposd::DecoderConfig`, and record normalized row params.

**Architecture:** Keep all parser behavior in the existing `rsinter` benchmark registry and `rbposd` runner wrapper. Add narrow disambiguation for the already-used generic `schedule` key so CSS `schedule = "greedy"` and `schedule = "sequential"` remain circuit-generation parameters while other `rbposd` schedule values route to the decoder. Keep legacy `bp_algorithm = "min_sum"` accepted and recorded, and normalize decoder schedule as `bp_schedule` so CSS rows can preserve their circuit schedule field.

**Tech Stack:** Rust 2024, Cargo workspace, `rsinter` integration tests, `rbposd::DecoderConfig`, `toml::Value`, `serde_json`.

## Global Constraints

- Preserve existing `bp_iters`, `max_bp_iterations`, `early_stop`, OSD, and LSD parameter behavior.
- Preserve legacy `bp_algorithm = "min_sum"` compatibility.
- Record normalized upstream-facing row params named `bp_method` and `bp_schedule`.
- Do not break existing CSS benchmark specs that use `schedule = "greedy"` or `schedule = "sequential"`.
- Fail unsupported BP method or schedule values during preflight before result artifacts are written.
- Do not change plot semantics, smoke/full benchmark specs, or Python differential harnesses.

---

## File Structure

- Modify `rsinter/tests/bench_registry.rs`: add registry tests proving `bp_method` and BP `schedule` are decoder params while CSS `schedule = "greedy"` stays generic.
- Modify `rsinter/tests/bench_runner_wrappers.rs`: add focused `RbposdRunner::preflight_point` tests for accepted and rejected BP method/schedule values.
- Modify `rsinter/tests/bench_run.rs`: add end-to-end benchmark result and negative-control artifact tests.
- Modify `rsinter/src/bench/registry.rs`: recognize `bp_method` and route `schedule` by runner/value.
- Modify `rsinter/src/bench/runners/rbposd.rs`: parse `bp_method`, legacy `bp_algorithm`, BP `schedule`, map them to `DecoderConfig`, and normalize row params while omitting decoder `schedule` when a point already has a circuit schedule.

## Task 1: Add Failing rsinter Coverage

**Files:**
- Modify: `rsinter/tests/bench_registry.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `expand_runner_points_for_runner`, `RbposdRunner::preflight_point`, `run_rust_benchmark`.
- Produces: tests named `expand_runner_points_accepts_rbposd_bp_method_and_schedule_params`, `expand_runner_points_keeps_css_greedy_schedule_generic_for_rbposd`, `rbposd_runner_accepts_bp_method_and_schedule_params`, `rbposd_runner_rejects_unknown_bp_method`, `rbposd_runner_rejects_unknown_bp_schedule`, `rbposd_benchmark_records_bp_method_and_schedule`, and `rbposd_runner_rejects_unknown_bp_method_without_results`.

- [ ] **Step 1: Add registry tests**

In `rsinter/tests/bench_registry.rs`, after `expand_runner_points_accepts_rbposd_lsd_params`, add:

```rust
#[test]
fn expand_runner_points_accepts_rbposd_bp_method_and_schedule_params() {
    let mut params = valid_runner_params();
    params.insert(
        "bp_method".into(),
        toml::Value::String("product_sum".into()),
    );
    params.insert("schedule".into(), toml::Value::String("serial".into()));

    let points = expand_runner_points_for_runner("rbposd", &params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]
            .decoder_params
            .get("bp_method")
            .and_then(toml::Value::as_str),
        Some("product_sum")
    );
    assert_eq!(
        points[0]
            .decoder_params
            .get("schedule")
            .and_then(toml::Value::as_str),
        Some("serial")
    );
    assert_eq!(points[0].schedule, None);
}

#[test]
fn expand_runner_points_keeps_css_greedy_schedule_generic_for_rbposd() {
    let mut params = valid_css_runner_params();
    params.insert("schedule".into(), toml::Value::String("greedy".into()));
    params.insert(
        "bp_method".into(),
        toml::Value::String("minimum_sum".into()),
    );

    let points = expand_runner_points_for_runner("rbposd", &params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].schedule.as_deref(), Some("greedy"));
    assert!(!points[0].decoder_params.contains_key("schedule"));
    assert_eq!(
        points[0]
            .decoder_params
            .get("bp_method")
            .and_then(toml::Value::as_str),
        Some("minimum_sum")
    );
}
```

- [ ] **Step 2: Add runner preflight tests**

In `rsinter/tests/bench_runner_wrappers.rs`, after `rbposd_runner_handles_zero_shot_benchmark_points`, add:

```rust
#[test]
fn rbposd_runner_accepts_bp_method_and_schedule_params() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        (
            "bp_method".into(),
            toml::Value::String("product_sum".into()),
        ),
        ("schedule".into(), toml::Value::String("serial".into())),
    ]));

    runner.preflight_point(&point).unwrap();
}

#[test]
fn rbposd_runner_rejects_unknown_bp_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "bp_method".into(),
        toml::Value::String("sum_product".into()),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(
        err,
        "rbposd bp_method must be \"minimum_sum\" or \"product_sum\", got \"sum_product\""
    );
}

#[test]
fn rbposd_runner_rejects_unknown_bp_schedule() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([(
        "schedule".into(),
        toml::Value::String("flooding".into()),
    )]));

    let err = runner.preflight_point(&point).unwrap_err();

    assert_eq!(
        err,
        "rbposd schedule must be \"parallel\" or \"serial\", got \"flooding\""
    );
}
```

- [ ] **Step 3: Add benchmark row and artifact rejection tests**

In `rsinter/tests/bench_run.rs`, after `rbposd_benchmark_records_normalized_decoder_params`, add:

```rust
#[test]
fn rbposd_benchmark_records_bp_method_and_schedule() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_product_sum_serial"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bp_method = "product_sum"
schedule = "serial"
bp_iters = 3
osd_order = 0

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rbposd_product_sum_serial")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);
    assert_eq!(rows[0].params["bp_method"], serde_json::json!("product_sum"));
    assert_eq!(rows[0].params["schedule"], serde_json::json!("serial"));
    assert_eq!(rows[0].params["bp_algorithm"], serde_json::json!("min_sum"));
    assert_eq!(rows[0].params["bp_iters"], serde_json::json!(3));
    assert_eq!(rows[0].params["osd_method"], serde_json::json!("combination_sweep"));
    assert_eq!(rows[0].params["osd_order"], serde_json::json!(0));
}

#[test]
fn rbposd_runner_rejects_unknown_bp_method_without_results() {
    let spec_text = issue91_surface_spec(r#"bp_method = "sum_product""#);
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
        "rbposd bp_method must be \"minimum_sum\" or \"product_sum\", got \"sum_product\""
    );
    assert!(!dir.path().join("rbposd_lsd").exists());
}
```

- [ ] **Step 4: Verify focused tests fail for missing behavior**

Run:

```bash
cargo test -p rsinter expand_runner_points_accepts_rbposd_bp_method_and_schedule_params
cargo test -p rsinter rbposd_runner_accepts_bp_method_and_schedule_params
cargo test -p rsinter rbposd_benchmark_records_bp_method_and_schedule
cargo test -p rsinter rbposd_runner_rejects_unknown_bp_method_without_results
```

Expected: at least the new BP method/schedule tests fail because the parser does not yet recognize or normalize those keys.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add rsinter/tests/bench_registry.rs rsinter/tests/bench_runner_wrappers.rs rsinter/tests/bench_run.rs
git commit -m "test: cover rsinter rbposd bp params"
```

## Task 2: Implement rbposd BP Parameter Parsing and Normalization

**Files:**
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: tests from Task 1.
- Produces: `RbposdRunnerParams::parse` mapping BP method/schedule strings into `rbposd::DecoderConfig`, plus normalized result params `bp_method` and `schedule`.

- [ ] **Step 1: Route rbposd BP schedule values as decoder params**

In `rsinter/src/bench/registry.rs`, replace the `split_runner_params` loop with this value-aware routing:

```rust
for (key, value) in params {
    if is_decoder_param_entry(runner_name, key, value) {
        decoder.insert(key.clone(), value.clone());
    } else if is_generic_param_key(key) {
        generic.insert(key.clone(), value.clone());
    } else {
        return Err(format!("unknown {runner_name} runner param: {key}"));
    }
}
```

Then replace `is_decoder_param_key` with:

```rust
fn is_decoder_param_entry(runner_name: &str, key: &str, value: &Value) -> bool {
    match runner_name {
        "rbposd" => {
            matches!(
                key,
                "bp_algorithm"
                    | "bp_method"
                    | "bp_iters"
                    | "max_bp_iterations"
                    | "early_stop"
                    | "osd_method"
                    | "osd_order"
                    | "lsd_method"
                    | "lsd_order"
            ) || is_rbposd_bp_schedule_entry(key, value)
        }
        "rilpqec" => matches!(
            key,
            "backend" | "time_limit_s" | "mip_gap" | "threads" | "verbose"
        ),
        "rmatching" | "generic" => false,
        _ => false,
    }
}

fn is_rbposd_bp_schedule_entry(key: &str, value: &Value) -> bool {
    key == "schedule" && !matches!(value.as_str(), Some("greedy"))
}
```

This preserves the existing CSS `schedule = "greedy"` value and routes
`"parallel"`, `"serial"`, unsupported strings, and non-string schedule values
to the typed rbposd parser.

- [ ] **Step 2: Parse BP method and schedule in the rbposd runner**

In `rsinter/src/bench/runners/rbposd.rs`, update imports:

```rust
use rbposd::{BpVariant, DecoderConfig, LsdConfig, LsdMethod, Schedule};
```

Replace the initial BP parser block in `RbposdRunnerParams::parse` with:

```rust
let mut bp_config = DecoderConfig::default();
let legacy_bp_algorithm = optional_string(params, "bp_algorithm")?;
let explicit_bp_method = optional_string(params, "bp_method")?;
if legacy_bp_algorithm.is_some() && explicit_bp_method.is_some() {
    return Err("rbposd params must not set both bp_algorithm and bp_method".into());
}

let bp_method = match (explicit_bp_method, legacy_bp_algorithm.as_deref()) {
    (Some(value), None) => value,
    (None, Some("min_sum")) | (None, None) => "minimum_sum".to_string(),
    (None, Some(value)) => {
        return Err(format!(
            "rbposd bp_algorithm must be \"min_sum\", got \"{value}\""
        ));
    }
    (Some(_), Some(_)) => unreachable!("checked above"),
};
bp_config.bp_variant = parse_bp_method(&bp_method)?;

let bp_schedule = optional_string(params, "schedule")?.unwrap_or_else(|| "parallel".to_string());
bp_config.schedule = parse_bp_schedule(&bp_schedule)?;
```

Add these helper functions above `impl RbposdRunnerParams`:

```rust
fn parse_bp_method(value: &str) -> Result<BpVariant, String> {
    match value {
        "minimum_sum" => Ok(BpVariant::MinimumSum),
        "product_sum" => Ok(BpVariant::ProductSum),
        other => Err(format!(
            "rbposd bp_method must be \"minimum_sum\" or \"product_sum\", got \"{other}\""
        )),
    }
}

fn parse_bp_schedule(value: &str) -> Result<Schedule, String> {
    match value {
        "parallel" => Ok(Schedule::Parallel),
        "serial" => Ok(Schedule::Serial),
        other => Err(format!(
            "rbposd schedule must be \"parallel\" or \"serial\", got \"{other}\""
        )),
    }
}
```

- [ ] **Step 3: Normalize BP method and schedule fields**

In both `normalized: ParamMap::from_pairs([...])` blocks in `rsinter/src/bench/runners/rbposd.rs`, include these fields immediately after `bp_algorithm`:

```rust
("bp_method", serde_json::json!(bp_method)),
("schedule", serde_json::json!(bp_schedule)),
```

Keep the legacy field:

```rust
("bp_algorithm", serde_json::json!("min_sum")),
```

so existing assertions continue to pass even when the selected BP method is `product_sum`.

- [ ] **Step 4: Update legacy benchmark assertions for default normalized fields**

In `rsinter/tests/bench_run.rs`, add these assertions anywhere a test already asserts `row.params["bp_algorithm"] == "min_sum"` for rbposd default rows:

```rust
assert_eq!(row.params["bp_method"], serde_json::json!("minimum_sum"));
assert_eq!(row.params["schedule"], serde_json::json!("parallel"));
```

For tests that use `rows[0]`, use:

```rust
assert_eq!(rows[0].params["bp_method"], serde_json::json!("minimum_sum"));
assert_eq!(rows[0].params["schedule"], serde_json::json!("parallel"));
```

- [ ] **Step 5: Run issue verification and rsinter suite**

Run:

```bash
cargo test -p rsinter rbposd_runner_accepts_bp_method_and_schedule_params
cargo test -p rsinter rbposd_benchmark_records_bp_method_and_schedule
cargo test -p rsinter rbposd_runner_rejects_unknown_bp_method_without_results
cargo test -p rsinter
```

Expected: all commands pass.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add rsinter/src/bench/registry.rs rsinter/src/bench/runners/rbposd.rs rsinter/tests/bench_registry.rs rsinter/tests/bench_runner_wrappers.rs rsinter/tests/bench_run.rs
git commit -m "feat: parse rbposd bp method schedule params"
```

## Task 3: Final Verification

**Files:**
- No source edits unless verification exposes a defect.

**Interfaces:**
- Consumes: committed Tasks 1 and 2.
- Produces: final evidence for PR description.

- [ ] **Step 1: Run required full workspace verification**

Run:

```bash
cargo test
```

Expected: all workspace tests pass.

- [ ] **Step 2: Check final diff hygiene**

Run:

```bash
git status --short
git diff --check
```

Expected: `git status --short` shows no uncommitted source changes and `git diff --check` exits successfully.
