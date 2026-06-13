# Rsinter Decoder Params Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement GitHub issue #47 by letting `rsinter` benchmark runner params configure decoders, recording those params in result rows, and adding real `rbposd.osd_order` behavior.

**Architecture:** Keep `BenchCasePoint` responsible for generic benchmark points and attach a cloned decoder-param map to each point. Each runner parses its own typed params and passes normalized JSON params into `run_decoder_point`, while `rbposd` grows a real OSD-k search path behind `DecoderConfig::osd_order`.

**Tech Stack:** Rust 2024 workspace; `toml::Value` for input params; `serde_json::Value` result params; `cargo test`; crates `rsinter`, `rbposd`, and `rilpqec`.

---

## File Structure

- Modify `rsinter/src/bench/registry.rs`: split generic benchmark params from runner-specific params, add `decoder_params` to `BenchCasePoint`, and expose `expand_runner_points_for_runner`.
- Modify `rsinter/src/bench/run.rs`: call runner-aware point expansion using the resolved runner implementation name.
- Create `rsinter/src/bench/runners/params.rs`: shared typed parsing helpers for runner parameter values.
- Modify `rsinter/src/bench/runners/mod.rs`: import `params`, merge normalized decoder params into result rows, and update `run_decoder_point` signature.
- Modify `rsinter/src/bench/runners/rbposd.rs`: parse/apply `bp_iters`, `max_bp_iterations`, `early_stop`, and `osd_order`.
- Modify `rsinter/src/bench/runners/rilpqec.rs`: parse/apply `backend`, `time_limit_s`, `mip_gap`, `threads`, and `verbose`.
- Modify `rsinter/src/bench/runners/rmatching.rs`: pass an empty decoder-param map.
- Modify `rbposd/src/config.rs`: add `DecoderConfig::osd_order`.
- Modify `rbposd/src/gf2.rs`: expose a detailed solve path with pivot/free columns and forced free-column assignment.
- Modify `rbposd/src/osd.rs`: implement OSD-k candidate search.
- Modify `rbposd/src/decoder.rs`: pass `osd_order` into OSD.
- Modify tests in `rsinter/tests/bench_registry.rs`, `rsinter/tests/bench_run.rs`, `rsinter/tests/bench_runner_wrappers.rs`, `rsinter/tests/decode_rbposd.rs`, `rbposd/tests/smoke.rs`, and `rbposd/tests/osd.rs`.

---

### Task 1: Split Generic And Decoder Params In `rsinter`

**Files:**
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/src/bench/run.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/tests/bench_registry.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

- [ ] **Step 1: Write failing registry tests**

Add these tests to `rsinter/tests/bench_registry.rs` after `expand_runner_points_defaults_to_legacy_surface_input`:

```rust
#[test]
fn expand_runner_points_for_runner_carries_decoder_params_without_multiplying_points() {
    let mut params = valid_runner_params();
    params.insert("bp_iters".into(), toml::Value::Integer(50));
    params.insert("osd_order".into(), toml::Value::Integer(10));

    let points = expand_runner_points_for_runner("rbposd", &params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]
            .decoder_params
            .get("bp_iters")
            .and_then(toml::Value::as_integer),
        Some(50)
    );
    assert_eq!(
        points[0]
            .decoder_params
            .get("osd_order")
            .and_then(toml::Value::as_integer),
        Some(10)
    );
    assert_eq!(points[0].distance, Some(3));
}

#[test]
fn expand_runner_points_for_runner_rejects_unknown_decoder_param() {
    let mut params = valid_runner_params();
    params.insert("bogus".into(), toml::Value::Integer(1));

    let err = expand_runner_points_for_runner("rbposd", &params).unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus");
}

#[test]
fn expand_runner_points_for_runner_rejects_decoder_params_for_rmatching() {
    let mut params = valid_runner_params();
    params.insert("osd_order".into(), toml::Value::Integer(10));

    let err = expand_runner_points_for_runner("rmatching", &params).unwrap_err();

    assert_eq!(err, "unknown rmatching runner param: osd_order");
}
```

Update the import at the top of the same file:

```rust
use rsinter::bench::registry::{
    build_default_rust_runner_registry, default_rust_runner_names, expand_runner_points,
    expand_runner_points_for_runner,
};
```

Update every manual `BenchCasePoint` literal in `rsinter/tests/bench_runner_wrappers.rs` to include:

```rust
decoder_params: std::collections::BTreeMap::new(),
```

Update every manual `BenchCasePoint` literal in `rsinter/src/bench/runners/mod.rs` tests to include:

```rust
decoder_params: BTreeMap::new(),
```

- [ ] **Step 2: Run the failing registry tests**

Run:

```bash
cargo test -p rsinter bench_registry
```

Expected: FAIL because `expand_runner_points_for_runner` and `BenchCasePoint::decoder_params` do not exist.

- [ ] **Step 3: Add param splitting and point-carried decoder params**

In `rsinter/src/bench/registry.rs`, add the field to `BenchCasePoint`:

```rust
pub struct BenchCasePoint {
    pub input_type: String,
    pub code_id: Option<String>,
    pub distance: Option<usize>,
    pub rounds: usize,
    pub p: f64,
    pub basis: Option<String>,
    pub schedule: Option<String>,
    pub hx_path: Option<String>,
    pub hz_path: Option<String>,
    pub observables_path: Option<String>,
    pub max_shots: u64,
    pub max_errors: u64,
    pub batch_size: usize,
    pub decoder_params: BTreeMap<String, Value>,
}
```

Add these helpers near `expand_runner_points`:

```rust
struct SplitRunnerParams {
    generic: BTreeMap<String, Value>,
    decoder: BTreeMap<String, Value>,
}

pub fn expand_runner_points(
    params: &BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String> {
    expand_runner_points_for_runner("generic", params)
}

pub fn expand_runner_points_for_runner(
    runner_name: &str,
    params: &BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String> {
    let split = split_runner_params(runner_name, params)?;
    expand_generic_runner_points(&split.generic, split.decoder)
}

fn expand_generic_runner_points(
    params: &BTreeMap<String, Value>,
    decoder_params: BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String> {
    let input_type =
        optional_string(params, "input_type")?.unwrap_or_else(|| "surface_rotated_memory_x".into());
    let rounds = require_array(params, "rounds")?;
    let ps = require_array(params, "p")?;
    let max_shots = require_u64(params, "max_shots")?;
    let max_errors = require_u64(params, "max_errors")?;
    let batch_size = require_usize(params, "batch_size")?;
    if rounds.is_empty() {
        return Err("rounds must not be empty".into());
    }
    if ps.is_empty() {
        return Err("p must not be empty".into());
    }
    if batch_size == 0 {
        return Err("batch_size must be positive".into());
    }

    match input_type.as_str() {
        "surface_rotated_memory_x" => expand_surface_points(
            params,
            rounds,
            ps,
            max_shots,
            max_errors,
            batch_size,
            decoder_params,
        ),
        "css" => expand_css_points(
            params,
            rounds,
            ps,
            max_shots,
            max_errors,
            batch_size,
            decoder_params,
        ),
        other => Err(format!("unknown input_type: {other}")),
    }
}
```

Replace the old `expand_runner_points` body with the new functions above.

Add these helper functions below `expand_css_points`:

```rust
fn split_runner_params(
    runner_name: &str,
    params: &BTreeMap<String, Value>,
) -> Result<SplitRunnerParams, String> {
    let mut generic = BTreeMap::new();
    let mut decoder = BTreeMap::new();
    for (key, value) in params {
        if is_generic_param_key(key) {
            generic.insert(key.clone(), value.clone());
        } else if is_decoder_param_key(runner_name, key) {
            decoder.insert(key.clone(), value.clone());
        } else {
            return Err(format!("unknown {runner_name} runner param: {key}"));
        }
    }
    Ok(SplitRunnerParams { generic, decoder })
}

fn is_generic_param_key(key: &str) -> bool {
    matches!(
        key,
        "input_type"
            | "distance"
            | "rounds"
            | "p"
            | "max_shots"
            | "max_errors"
            | "batch_size"
            | "basis"
            | "schedule"
            | "hx"
            | "hz"
            | "observables"
            | "code_id"
    )
}

fn is_decoder_param_key(runner_name: &str, key: &str) -> bool {
    match runner_name {
        "rbposd" => matches!(key, "bp_iters" | "max_bp_iterations" | "early_stop" | "osd_order"),
        "rilpqec" => matches!(
            key,
            "backend" | "time_limit_s" | "mip_gap" | "threads" | "verbose"
        ),
        "rmatching" | "generic" => false,
        _ => false,
    }
}
```

Update `expand_surface_points` and `expand_css_points` signatures to accept `decoder_params: BTreeMap<String, Value>`. When pushing a point, set:

```rust
decoder_params: decoder_params.clone(),
```

In `rsinter/src/bench/run.rs`, update the import and point expansion call:

```rust
use crate::bench::registry::{
    expand_runner_points_for_runner, BenchRunContext, RustRunnerRegistry,
};
```

```rust
let points = expand_runner_points_for_runner(runner_impl.name(), &runner.params)?;
```

- [ ] **Step 4: Run the registry tests**

Run:

```bash
cargo test -p rsinter bench_registry
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add rsinter/src/bench/registry.rs rsinter/src/bench/run.rs rsinter/src/bench/runners/mod.rs rsinter/tests/bench_registry.rs rsinter/tests/bench_runner_wrappers.rs
git commit -m "feat: split rsinter decoder params"
```

---

### Task 2: Record Normalized Decoder Params And Parse `rbposd`

**Files:**
- Create: `rsinter/src/bench/runners/params.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/src/bench/runners/rmatching.rs`
- Modify: `rsinter/src/bench/runners/rilpqec.rs`
- Modify: `rbposd/src/config.rs`
- Modify: `rbposd/tests/smoke.rs`
- Modify: `rsinter/tests/bench_run.rs`

- [ ] **Step 1: Write failing tests for result recording and rbposd config defaults**

In `rbposd/tests/smoke.rs`, update `decoder_config_defaults_match_reference_expectations` to include:

```rust
assert_eq!(cfg.osd_order, 0);
```

In `rsinter/tests/bench_run.rs`, add this test after `rust_benchmark_results_use_runner_name_not_impl_key`:

```rust
#[test]
fn rbposd_benchmark_records_normalized_decoder_params() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_tuned"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bp_iters = 50
early_stop = false
osd_order = 10

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
            .join("rbposd_tuned")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["bp_iters"], serde_json::json!(50));
    assert_eq!(rows[0].params["early_stop"], serde_json::json!(false));
    assert_eq!(rows[0].params["osd_order"], serde_json::json!(10));
}

#[test]
fn rbposd_benchmark_rejects_both_bp_iteration_aliases() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bp_iters = 50
max_bp_iterations = 60

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
        "rbposd params must not set both bp_iters and max_bp_iterations"
    );
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p rbposd decoder_config_defaults_match_reference_expectations
cargo test -p rsinter rbposd_benchmark_records_normalized_decoder_params
cargo test -p rsinter rbposd_benchmark_rejects_both_bp_iteration_aliases
```

Expected: FAIL because `DecoderConfig::osd_order`, param parsing, and result merging do not exist.

- [ ] **Step 3: Add `osd_order` to `rbposd::DecoderConfig`**

In `rbposd/src/config.rs`, update the struct and default:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderConfig {
    pub max_bp_iterations: usize,
    pub early_stop: bool,
    pub bp_variant: BpVariant,
    pub schedule: Schedule,
    pub osd_variant: OsdVariant,
    pub osd_order: usize,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            max_bp_iterations: 30,
            early_stop: true,
            bp_variant: BpVariant::MinimumSum,
            schedule: Schedule::Parallel,
            osd_variant: OsdVariant::Osd0,
            osd_order: 0,
        }
    }
}
```

- [ ] **Step 4: Add shared runner-param parsing helpers**

Create `rsinter/src/bench/runners/params.rs`:

```rust
use std::collections::BTreeMap;

use toml::Value;

pub(crate) fn optional_bool(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a boolean")),
    }
}

pub(crate) fn optional_f64(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<f64>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => {
            if let Some(value) = value.as_float() {
                Ok(Some(value))
            } else if let Some(value) = value.as_integer() {
                Ok(Some(value as f64))
            } else {
                Err(format!("{key} must be numeric"))
            }
        }
    }
}

pub(crate) fn optional_string(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(str::to_string)
            .map(Some)
            .ok_or_else(|| format!("{key} must be a string")),
    }
}

pub(crate) fn optional_usize(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<usize>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => {
            let integer = value
                .as_integer()
                .ok_or_else(|| format!("{key} must be an integer"))?;
            usize::try_from(integer)
                .map(Some)
                .map_err(|_| format!("{key} must be non-negative"))
        }
    }
}

pub(crate) fn optional_positive_u32(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<u32>, String> {
    match optional_usize(params, key)? {
        None => Ok(None),
        Some(0) => Err(format!("{key} must be positive")),
        Some(value) => u32::try_from(value)
            .map(Some)
            .map_err(|_| format!("{key} exceeds supported u32 range")),
    }
}
```

In `rsinter/src/bench/runners/mod.rs`, add:

```rust
pub(crate) mod params;
```

- [ ] **Step 5: Merge normalized decoder params into result rows**

In `rsinter/src/bench/runners/mod.rs`, change `run_decoder_point` to accept normalized decoder params:

```rust
pub(crate) fn run_decoder_point(
    runner_name: &'static str,
    decoder: &dyn Decoder,
    point: &BenchCasePoint,
    ctx: &BenchRunContext,
    decoder_params: &crate::bench::result::ParamMap,
) -> Result<BenchmarkResultRow, String> {
```

Before constructing `BenchmarkResultRow`, add:

```rust
    let mut result_params = built.params;
    for (key, value) in decoder_params {
        result_params.insert(key.clone(), value.clone());
    }
```

Set the row field to:

```rust
        params: result_params,
```

Update `RmatchingRunner::run_point` in `rsinter/src/bench/runners/rmatching.rs`:

```rust
        let decoder_params = crate::bench::result::ParamMap::new();
        run_decoder_point(self.name(), &decoder, point, ctx, &decoder_params)
```

Update `RilpqecRunner::run_point` temporarily in `rsinter/src/bench/runners/rilpqec.rs`:

```rust
        let decoder = IlpDemDecoder::default();
        let decoder_params = crate::bench::result::ParamMap::new();
        run_decoder_point(self.name(), &decoder, point, ctx, &decoder_params)
```

- [ ] **Step 6: Parse and apply `rbposd` runner params**

Replace `rsinter/src/bench/runners/rbposd.rs` with:

```rust
use std::collections::BTreeMap;

use rbposd::DecoderConfig;
use toml::Value;

use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, PairMapExt, ParamMap};
use crate::bench::runners::params::{optional_bool, optional_usize};
use crate::bench::runners::run_decoder_point;
use crate::decode::RbposdDemDecoder;

pub struct RbposdRunner;

struct RbposdRunnerParams {
    config: DecoderConfig,
    normalized: ParamMap,
}

impl RbposdRunnerParams {
    fn parse(params: &BTreeMap<String, Value>) -> Result<Self, String> {
        let mut config = DecoderConfig::default();
        let bp_iters = optional_usize(params, "bp_iters")?;
        let max_bp_iterations = optional_usize(params, "max_bp_iterations")?;
        let bp_iters = match (bp_iters, max_bp_iterations) {
            (Some(_), Some(_)) => {
                return Err(
                    "rbposd params must not set both bp_iters and max_bp_iterations".into(),
                );
            }
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => config.max_bp_iterations,
        };
        config.max_bp_iterations = bp_iters;
        config.early_stop = optional_bool(params, "early_stop")?.unwrap_or(config.early_stop);
        config.osd_order = optional_usize(params, "osd_order")?.unwrap_or(config.osd_order);

        Ok(Self {
            config,
            normalized: ParamMap::from_pairs([
                ("bp_iters", serde_json::json!(config.max_bp_iterations)),
                ("early_stop", serde_json::json!(config.early_stop)),
                ("osd_order", serde_json::json!(config.osd_order)),
            ]),
        })
    }
}

impl RustBenchRunner for RbposdRunner {
    fn name(&self) -> &'static str {
        "rbposd"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let params = RbposdRunnerParams::parse(&point.decoder_params)?;
        let decoder = RbposdDemDecoder::new(params.config);
        run_decoder_point(self.name(), &decoder, point, ctx, &params.normalized)
    }
}
```

- [ ] **Step 7: Run Task 2 tests**

Run:

```bash
cargo test -p rbposd decoder_config_defaults_match_reference_expectations
cargo test -p rsinter rbposd_benchmark_records_normalized_decoder_params
cargo test -p rsinter rbposd_benchmark_rejects_both_bp_iteration_aliases
```

Expected: PASS.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add rbposd/src/config.rs rbposd/tests/smoke.rs rsinter/src/bench/runners rsinter/tests/bench_run.rs
git commit -m "feat: record rbposd benchmark params"
```

---

### Task 3: Parse And Apply `rilpqec` Runner Params

**Files:**
- Modify: `rsinter/src/bench/runners/rilpqec.rs`
- Modify: `rsinter/tests/bench_run.rs`

- [ ] **Step 1: Write failing `rilpqec` runner-param tests**

Add these tests to `rsinter/tests/bench_run.rs` after the `rbposd` param tests:

```rust
#[test]
fn rilpqec_benchmark_records_normalized_decoder_params() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rilpqec_tuned"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
backend = "highs"
time_limit_s = 5.0
mip_gap = 0.01
threads = 1
verbose = true

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
            .join("rilpqec_tuned")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["backend"], serde_json::json!("highs"));
    assert_eq!(rows[0].params["time_limit_s"], serde_json::json!(5.0));
    assert_eq!(rows[0].params["mip_gap"], serde_json::json!(0.01));
    assert_eq!(rows[0].params["threads"], serde_json::json!(1));
    assert_eq!(rows[0].params["verbose"], serde_json::json!(true));
}

#[test]
fn rilpqec_benchmark_rejects_invalid_mip_gap() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rilpqec_bad"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
mip_gap = 1.0

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "mip_gap must be in [0, 1)");
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p rsinter rilpqec_benchmark_records_normalized_decoder_params
cargo test -p rsinter rilpqec_benchmark_rejects_invalid_mip_gap
```

Expected: FAIL because `rilpqec` params are not parsed or recorded.

- [ ] **Step 3: Implement `rilpqec` typed param parsing**

Replace `rsinter/src/bench/runners/rilpqec.rs` with:

```rust
use std::collections::BTreeMap;

use rilpqec::{BackendKind, IlpDecoderConfig};
use toml::Value;

use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, ParamMap};
use crate::bench::runners::params::{
    optional_bool, optional_f64, optional_positive_u32, optional_string,
};
use crate::bench::runners::run_decoder_point;
use crate::decode::IlpDemDecoder;

pub struct RilpqecRunner;

struct RilpqecRunnerParams {
    config: IlpDecoderConfig,
    normalized: ParamMap,
}

impl RilpqecRunnerParams {
    fn parse(params: &BTreeMap<String, Value>) -> Result<Self, String> {
        let mut config = IlpDecoderConfig::default();
        let backend_name = optional_string(params, "backend")?.unwrap_or_else(|| "auto".into());
        config.backend.kind = match backend_name.as_str() {
            "auto" => BackendKind::Auto,
            "highs" => BackendKind::Highs,
            "gurobi" => BackendKind::Gurobi,
            other => return Err(format!("unknown rilpqec backend: {other}")),
        };

        config.backend.time_limit_seconds = optional_f64(params, "time_limit_s")?;
        if let Some(limit) = config.backend.time_limit_seconds {
            if !limit.is_finite() || limit <= 0.0 {
                return Err("time_limit_s must be positive".into());
            }
        }

        config.backend.mip_gap = optional_f64(params, "mip_gap")?;
        if let Some(gap) = config.backend.mip_gap {
            if !gap.is_finite() || !(0.0..1.0).contains(&gap) {
                return Err("mip_gap must be in [0, 1)".into());
            }
        }

        config.backend.threads = optional_positive_u32(params, "threads")?;
        config.backend.verbose = optional_bool(params, "verbose")?.unwrap_or(false);

        let mut normalized = ParamMap::new();
        normalized.insert("backend".into(), serde_json::json!(backend_name));
        if let Some(limit) = config.backend.time_limit_seconds {
            normalized.insert("time_limit_s".into(), serde_json::json!(limit));
        }
        if let Some(gap) = config.backend.mip_gap {
            normalized.insert("mip_gap".into(), serde_json::json!(gap));
        }
        if let Some(threads) = config.backend.threads {
            normalized.insert("threads".into(), serde_json::json!(threads));
        }
        normalized.insert("verbose".into(), serde_json::json!(config.backend.verbose));

        Ok(Self { config, normalized })
    }
}

impl RustBenchRunner for RilpqecRunner {
    fn name(&self) -> &'static str {
        "rilpqec"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let params = RilpqecRunnerParams::parse(&point.decoder_params)?;
        let decoder = IlpDemDecoder::new(params.config);
        run_decoder_point(self.name(), &decoder, point, ctx, &params.normalized)
    }
}
```

- [ ] **Step 4: Run the `rilpqec` tests**

Run:

```bash
cargo test -p rsinter rilpqec_benchmark_records_normalized_decoder_params
cargo test -p rsinter rilpqec_benchmark_rejects_invalid_mip_gap
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add rsinter/src/bench/runners/rilpqec.rs rsinter/tests/bench_run.rs
git commit -m "feat: record rilpqec benchmark params"
```

---

### Task 4: Add Detailed GF(2) Solves For OSD-k

**Files:**
- Modify: `rbposd/src/gf2.rs`

- [ ] **Step 1: Write failing GF(2) tests**

In `rbposd/src/gf2.rs`, add this test inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn prepared_system_can_force_free_columns() {
    let pcm =
        ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
    let syndrome = Syndrome::from(vec![true, true]);
    let mut prepared = PreparedLinearSystem::from_pcm(&pcm);

    let osd0 = prepared
        .solve_with_column_order_detailed(&syndrome, &[0, 1, 2], &[])
        .unwrap();
    let forced = prepared
        .solve_with_column_order_detailed(&syndrome, &[0, 1, 2], &[2])
        .unwrap();

    assert_eq!(osd0.correction, Correction::from(vec![true, true, false]));
    assert_eq!(osd0.pivot_columns, vec![0, 1]);
    assert_eq!(osd0.free_columns, vec![2]);
    assert_eq!(forced.correction, Correction::from(vec![false, false, true]));
    assert_eq!(pcm.multiply(&forced.correction), syndrome);
}
```

- [ ] **Step 2: Run the failing GF(2) test**

Run:

```bash
cargo test -p rbposd prepared_system_can_force_free_columns
```

Expected: FAIL because `solve_with_column_order_detailed` does not exist.

- [ ] **Step 3: Implement detailed solve results**

In `rbposd/src/gf2.rs`, add this struct above `PreparedLinearSystem`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DetailedSolution {
    pub(crate) correction: Correction,
    pub(crate) pivot_columns: Vec<usize>,
    pub(crate) free_columns: Vec<usize>,
}
```

Replace `solve_with_column_order` with:

```rust
    pub(crate) fn solve_with_column_order(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
    ) -> Result<Correction, DecodeError> {
        self.solve_with_column_order_detailed(syndrome, column_order, &[])
            .map(|solution| solution.correction)
    }
```

Add this method to `impl PreparedLinearSystem`:

```rust
    pub(crate) fn solve_with_column_order_detailed(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
        forced_true_columns: &[usize],
    ) -> Result<DetailedSolution, DecodeError> {
        self.scratch_rows.clone_from(&self.base_rows);
        self.scratch_rhs.copy_from_slice(syndrome.as_slice());
        self.pivot_columns.clear();
        let mut row = 0usize;

        for (pivot_position, &column) in column_order.iter().enumerate() {
            if row == self.scratch_rows.len() {
                break;
            }
            let pivot = (row..self.scratch_rows.len())
                .find(|&candidate| self.scratch_rows[candidate][column]);
            if let Some(pivot_row) = pivot {
                self.scratch_rows.swap(row, pivot_row);
                self.scratch_rhs.swap(row, pivot_row);
                for other in 0..self.scratch_rows.len() {
                    if other != row && self.scratch_rows[other][column] {
                        for &physical in column_order.iter().skip(pivot_position) {
                            self.scratch_rows[other][physical] ^=
                                self.scratch_rows[row][physical];
                        }
                        self.scratch_rhs[other] ^= self.scratch_rhs[row];
                    }
                }
                self.pivot_columns.push(column);
                row += 1;
            }
        }

        if self.scratch_rhs.iter().skip(row).any(|&bit| bit) {
            return Err(DecodeError::SingularSystem);
        }

        let mut is_pivot = vec![false; self.num_bits];
        for &column in &self.pivot_columns {
            is_pivot[column] = true;
        }
        let free_columns = column_order
            .iter()
            .copied()
            .filter(|&column| !is_pivot[column])
            .collect::<Vec<_>>();

        let mut solution = vec![false; self.num_bits];
        for &column in forced_true_columns {
            if column >= self.num_bits {
                return Err(DecodeError::InvalidColumnIndex {
                    column,
                    num_bits: self.num_bits,
                });
            }
            if is_pivot[column] {
                return Err(DecodeError::SingularSystem);
            }
            solution[column] = true;
        }

        for (pivot_row, &column) in self.pivot_columns.iter().enumerate().rev() {
            let mut value = self.scratch_rhs[pivot_row];
            for (physical, &coefficient) in self.scratch_rows[pivot_row].iter().enumerate() {
                if physical != column && coefficient && solution[physical] {
                    value ^= true;
                }
            }
            solution[column] = value;
        }

        Ok(DetailedSolution {
            correction: Correction::from(solution),
            pivot_columns: self.pivot_columns.clone(),
            free_columns,
        })
    }
```

- [ ] **Step 4: Run the GF(2) tests**

Run:

```bash
cargo test -p rbposd gf2
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add rbposd/src/gf2.rs
git commit -m "feat: expose rbposd osd solution space"
```

---

### Task 5: Implement `rbposd` OSD-k Behavior

**Files:**
- Modify: `rbposd/src/osd.rs`
- Modify: `rbposd/src/decoder.rs`
- Modify: `rbposd/tests/osd.rs`

- [ ] **Step 1: Write failing OSD-k behavior tests**

Add this test to `rbposd/tests/osd.rs`:

```rust
#[test]
fn osd_order_one_can_improve_over_osd0() {
    let pcm =
        ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
    let syndrome = Syndrome::from(vec![true, true]);
    let channel = ChannelModel::BitFlipProbabilities(vec![
        0.268_941_421_369_995_1,
        0.268_941_421_369_995_1,
        0.182_425_523_806_356_35,
    ]);

    let mut osd0_config = DecoderConfig::default();
    osd0_config.max_bp_iterations = 0;
    osd0_config.osd_order = 0;
    let osd0 = BpOsdDecoder::new(pcm.clone(), channel.clone(), osd0_config)
        .unwrap()
        .decode(&syndrome)
        .unwrap();

    let mut osd1_config = DecoderConfig::default();
    osd1_config.max_bp_iterations = 0;
    osd1_config.osd_order = 1;
    let osd1 = BpOsdDecoder::new(pcm.clone(), channel, osd1_config)
        .unwrap()
        .decode(&syndrome)
        .unwrap();

    assert_eq!(osd0.correction, Correction::from(vec![true, true, false]));
    assert_eq!(osd1.correction, Correction::from(vec![false, false, true]));
    assert_eq!(pcm.multiply(&osd1.correction), syndrome);
}
```

- [ ] **Step 2: Run the failing OSD-k test**

Run:

```bash
cargo test -p rbposd osd_order_one_can_improve_over_osd0
```

Expected: FAIL because `osd_order` is not used.

- [ ] **Step 3: Implement OSD-k candidate search**

In `rbposd/src/osd.rs`, update imports:

```rust
use crate::gf2::{DetailedSolution, PreparedLinearSystem};
```

Add this constant near the top:

```rust
const OSD_FREE_COLUMN_FRONTIER: usize = 16;
```

Replace `decode_osd0_with_workspace` with this wrapper plus the new function:

```rust
pub(crate) fn decode_osd0_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
) -> Result<Correction, DecodeError> {
    decode_osd_with_workspace(
        pcm,
        syndrome,
        base_correction_bits,
        reliability,
        workspace,
        0,
    )
}

pub(crate) fn decode_osd_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
    osd_order: usize,
) -> Result<Correction, DecodeError> {
    debug_assert_eq!(workspace.num_checks, pcm.num_checks());
    debug_assert_eq!(workspace.num_bits, pcm.num_bits());
    debug_assert_eq!(base_correction_bits.len(), pcm.num_bits());
    debug_assert_eq!(reliability.len(), pcm.num_bits());
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    workspace.sort_unreliable_columns(reliability);
    let base = workspace
        .prepared
        .solve_with_column_order_detailed(&target_syndrome, &workspace.column_order, &[])
        .map_err(|_| DecodeError::NoOsdSolution)?;
    let best = if osd_order == 0 {
        base
    } else {
        best_osd_candidate(&target_syndrome, reliability, workspace, base, osd_order)?
    };
    Ok(xor_correction_bits(base_correction_bits, &best.correction))
}
```

Add these helpers below `decode_osd_with_workspace`:

```rust
fn best_osd_candidate(
    target_syndrome: &Syndrome,
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
    base: DetailedSolution,
    osd_order: usize,
) -> Result<DetailedSolution, DecodeError> {
    let frontier_len = base.free_columns.len().min(OSD_FREE_COLUMN_FRONTIER);
    let frontier = base.free_columns[..frontier_len].to_vec();
    let max_order = osd_order.min(frontier.len());
    let mut best = base;
    let mut forced = Vec::new();
    for order in 1..=max_order {
        visit_combinations(&frontier, order, 0, &mut forced, &mut |columns| {
            if let Ok(candidate) = workspace.prepared.solve_with_column_order_detailed(
                target_syndrome,
                &workspace.column_order,
                columns,
            ) {
                if is_better_solution(&candidate, &best, reliability) {
                    best = candidate;
                }
            }
        });
    }
    Ok(best)
}

fn visit_combinations(
    columns: &[usize],
    target_len: usize,
    start: usize,
    forced: &mut Vec<usize>,
    visit: &mut impl FnMut(&[usize]),
) {
    if forced.len() == target_len {
        visit(forced);
        return;
    }
    let remaining = target_len - forced.len();
    for index in start..=columns.len() - remaining {
        forced.push(columns[index]);
        visit_combinations(columns, target_len, index + 1, forced, visit);
        forced.pop();
    }
}

fn is_better_solution(
    candidate: &DetailedSolution,
    best: &DetailedSolution,
    reliability: &[f64],
) -> bool {
    let candidate_cost = residual_cost(candidate.correction.as_slice(), reliability);
    let best_cost = residual_cost(best.correction.as_slice(), reliability);
    if candidate_cost < best_cost - f64::EPSILON {
        return true;
    }
    if (candidate_cost - best_cost).abs() <= f64::EPSILON {
        return candidate.correction.as_slice() < best.correction.as_slice();
    }
    false
}

fn residual_cost(bits: &[bool], reliability: &[f64]) -> f64 {
    bits.iter()
        .zip(reliability.iter())
        .filter_map(|(&bit, &cost)| bit.then_some(cost))
        .sum()
}
```

In `rbposd/src/decoder.rs`, update the OSD import:

```rust
use crate::osd::{decode_osd_with_workspace, OsdWorkspace};
```

Update the OSD call:

```rust
            let correction = {
                let mut osd_workspace = self.osd_workspace.lock().unwrap();
                decode_osd_with_workspace(
                    &self.pcm,
                    syndrome,
                    &bp_workspace.hard_decision_bits,
                    &bp_workspace.reliability,
                    &mut osd_workspace,
                    self.config.osd_order,
                )?
            };
```

- [ ] **Step 4: Run the OSD tests**

Run:

```bash
cargo test -p rbposd osd
```

Expected: PASS.

- [ ] **Step 5: Commit Task 5**

Run:

```bash
git add rbposd/src/osd.rs rbposd/src/decoder.rs rbposd/tests/osd.rs
git commit -m "feat: add rbposd osd order search"
```

---

### Task 6: Add The Issue #47 Teeth Test And Error Controls

**Files:**
- Modify: `rsinter/tests/decode_rbposd.rs`
- Modify: `rsinter/tests/bench_run.rs`

- [ ] **Step 1: Add exact LER teeth test for `rbposd_osd_order_changes_ler`**

Add this test and helpers to `rsinter/tests/decode_rbposd.rs`:

```rust
#[test]
fn rbposd_osd_order_changes_ler() {
    let dem = DetectorErrorModel::parse(concat!(
        "error(0.2689414213699951) D0\n",
        "error(0.2689414213699951) D1\n",
        "error(0.18242552380635635) D0 D1 L0\n",
    ))
    .unwrap();

    let order0_ler = exact_three_error_logical_error_rate(&dem, 0);
    let order10_ler = exact_three_error_logical_error_rate(&dem, 10);

    assert!(
        order10_ler < order0_ler,
        "expected osd_order=10 to improve LER: order0={order0_ler}, order10={order10_ler}"
    );
}

fn exact_three_error_logical_error_rate(dem: &DetectorErrorModel, osd_order: usize) -> f64 {
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;
    config.osd_order = osd_order;
    let decoder = RbposdDemDecoder::new(config);
    let compiled = decoder.compile_for_dem(dem);
    let probabilities = [
        0.268_941_421_369_995_1,
        0.268_941_421_369_995_1,
        0.182_425_523_806_356_35,
    ];
    let mut ler = 0.0;
    for e0 in [false, true] {
        for e1 in [false, true] {
            for e2 in [false, true] {
                let event = [e0, e1, e2];
                let probability = event
                    .iter()
                    .zip(probabilities.iter())
                    .map(|(&fired, &p)| if fired { p } else { 1.0 - p })
                    .product::<f64>();
                let det0 = e0 ^ e2;
                let det1 = e1 ^ e2;
                let observed = e2;
                let det_byte = u8::from(det0) | (u8::from(det1) << 1);
                let predicted = compiled.decode_shots_bit_packed(&[det_byte], 1, 2, 1)[0] & 1 != 0;
                if predicted != observed {
                    ler += probability;
                }
            }
        }
    }
    ler
}
```

- [ ] **Step 2: Add unknown-key benchmark control**

Add this test to `rsinter/tests/bench_run.rs`:

```rust
#[test]
fn rbposd_benchmark_rejects_unknown_decoder_param_without_results() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bogus = 1

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus");
    assert!(!dir.path().join("rbposd_bad").join("test-run").exists());
}
```

- [ ] **Step 3: Run the teeth and control tests**

Run:

```bash
cargo test -p rsinter rbposd_osd_order_changes_ler
cargo test -p rsinter rbposd_benchmark_rejects_unknown_decoder_param_without_results
```

Expected: PASS.

- [ ] **Step 4: Commit Task 6**

Run:

```bash
git add rsinter/tests/decode_rbposd.rs rsinter/tests/bench_run.rs
git commit -m "test: cover decoder params teeth"
```

---

### Task 7: Final Verification

**Files:**
- Read: `docs/superpowers/specs/2026-06-14-rsinter-decoder-params-design.md`
- Read: `docs/superpowers/plans/2026-06-14-rsinter-decoder-params.md`

- [ ] **Step 1: Run focused package tests**

Run:

```bash
cargo test -p rbposd
cargo test -p rsinter bench_registry
cargo test -p rsinter bench_run
cargo test -p rsinter decode_rbposd
```

Expected: all commands PASS.

- [ ] **Step 2: Run the requested issue test by name**

Run:

```bash
cargo test -p rsinter rbposd_osd_order_changes_ler
```

Expected: PASS.

- [ ] **Step 3: Run workspace verification**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 4: Inspect final diff**

Run:

```bash
git status --short
git diff --check
```

Expected: `git diff --check` prints nothing. `git status --short` shows only the intended implementation files if changes are not yet committed.
