# Issue #103 BP+OSD Rsinter Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a paper-faithful BP+OSD `rsinter` benchmark path for BB72 CSS memory benchmarks with explicit provenance, a predict-zero control, fast CI coverage, and a manual reference fixture.

**Architecture:** Keep the existing `rsinter` benchmark stack. Extend the generic benchmark point contract with an effective seed, add shared result-row provenance in `run_decoder_point`, expose `VacuousDecoder` as a small runner, and extend only the `rbposd` runner parser for paper-facing labels. Fixtures stay under `rsinter/tests/fixtures/bench` and reuse the existing BB72 CSS matrix and logical-observable files.

**Tech Stack:** Rust workspace crates `rsinter`, `rbposd`, `qec-code`; TOML benchmark specs; JSONL result rows; `cargo test`.

## Global Constraints

- CI must use fast deterministic contract tests, not full paper reproduction.
- The manual reference path must document MIN-SUM BP, 10000 BP iterations, combination-sweep OSD, and OSD order 10.
- Unknown or misspelled generic and decoder parameters must fail before results artifacts are written.
- Result rows must distinguish `row.runner` from `params.decoder_impl`.
- Effective `seed` must be accepted as a generic runner parameter and recorded in result params.
- `predict-zero` must wrap the existing `VacuousDecoder`; do not add a second always-zero implementation.
- Keep `rbposd` core algorithms unchanged.
- Do not add a general bivariate-bicycle code generator.

---

## File Structure

- `rsinter/src/bench/registry.rs`: generic benchmark-point expansion, runner-param splitting, default registry membership, and the new `BenchCasePoint::seed`.
- `rsinter/src/bench/runners/mod.rs`: shared benchmark execution; inject `decoder_impl` and `seed` into result params and seed the sampler from the point.
- `rsinter/src/bench/runners/predict_zero.rs`: new thin runner around `VacuousDecoder`.
- `rsinter/src/bench/runners/rbposd.rs`: parse and normalize `bp_algorithm` and `osd_method`.
- `rsinter/src/bench/runners/params.rs`: reuse `optional_string` for `rbposd` string labels.
- `rsinter/tests/bench_registry.rs`: registry membership, seed parsing, and runner-param splitting tests.
- `rsinter/tests/bench_run.rs`: end-to-end benchmark result-row tests for provenance, `rbposd` labels, predict-zero, and BB72 fixtures.
- `rsinter/tests/fixtures/bench/bb72_css_bposd_decoder.toml`: fast committed BB72 CSS BP+OSD and predict-zero smoke fixture.
- `rsinter/tests/fixtures/bench/bb72_css_bposd_reference.toml`: ignored/manual heavy reference fixture with the paper settings and reference-point comments.

---

### Task 1: Generic Seed And Result Provenance

**Files:**
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/tests/bench_registry.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: existing `BenchCasePoint`, `run_decoder_point`, and result `ParamMap`.
- Produces: `BenchCasePoint { seed: u64, ... }`; every benchmark result row has `params["decoder_impl"]` and `params["seed"]`.

- [ ] **Step 1: Write failing registry tests for seed parsing**

Add these tests in `rsinter/tests/bench_registry.rs` after `expand_runner_points_accepts_optional_max_wall_seconds`:

```rust
#[test]
fn expand_runner_points_accepts_and_defaults_seed() {
    let default_points = expand_runner_points(&valid_runner_params()).unwrap();
    assert_eq!(default_points.len(), 1);
    assert_eq!(default_points[0].seed, 12_345);

    let mut params = valid_runner_params();
    params.insert("seed".into(), toml::Value::Integer(99));

    let explicit_points = expand_runner_points(&params).unwrap();
    assert_eq!(explicit_points.len(), 1);
    assert_eq!(explicit_points[0].seed, 99);
}

#[test]
fn expand_runner_points_rejects_invalid_seed() {
    let mut params = valid_runner_params();
    params.insert("seed".into(), toml::Value::Float(1.0));
    assert_eq!(expand_points_err(&params), "seed must be an integer");

    let mut params = valid_runner_params();
    params.insert("seed".into(), toml::Value::Integer(-1));
    assert_eq!(expand_points_err(&params), "seed must be non-negative");
}
```

- [ ] **Step 2: Write failing result-row provenance assertions**

Update `rust_benchmark_results_use_runner_name_not_impl_key` in `rsinter/tests/bench_run.rs` by adding these assertions after `assert_eq!(rows[0].runner, "mwpm_alias");`:

```rust
    assert_eq!(rows[0].params["decoder_impl"], serde_json::json!("rmatching"));
    assert_eq!(rows[0].params["seed"], serde_json::json!(12_345));
```

- [ ] **Step 3: Run the focused tests and confirm they fail**

Run:

```sh
cargo test -p rsinter --test bench_registry expand_runner_points_accepts_and_defaults_seed expand_runner_points_rejects_invalid_seed
cargo test -p rsinter --test bench_run rust_benchmark_results_use_runner_name_not_impl_key
```

Expected:

- `bench_registry` fails because `BenchCasePoint` has no `seed` field and `seed` is an unknown runner param.
- `bench_run` fails because result params do not include `decoder_impl` or `seed`.

- [ ] **Step 4: Add the seed field and parser**

In `rsinter/src/bench/registry.rs`, add this constant near the top-level type definitions:

```rust
const DEFAULT_BENCH_SEED: u64 = 12_345;
```

Add `seed` to `BenchCasePoint`:

```rust
pub struct BenchCasePoint {
    pub input_type: String,
    pub code_id: Option<String>,
    pub distance: Option<usize>,
    pub rounds: usize,
    pub p: f64,
    pub seed: u64,
    pub basis: Option<String>,
    pub schedule: Option<String>,
    pub hx_path: Option<String>,
    pub hz_path: Option<String>,
    pub observables_path: Option<String>,
    pub max_shots: u64,
    pub max_errors: u64,
    pub max_wall_seconds: Option<f64>,
    pub batch_size: usize,
    pub decoder_params: BTreeMap<String, Value>,
}
```

In `expand_generic_runner_points`, parse the optional seed after `p`:

```rust
    let seed = optional_u64(params, "seed")?.unwrap_or(DEFAULT_BENCH_SEED);
```

Pass `seed` into both expansion helpers by adding a `seed: u64` parameter to `expand_surface_points` and `expand_css_points`, then include `seed,` in every `BenchCasePoint` initializer.

Add `seed` to `is_generic_param_key`:

```rust
            | "seed"
```

Add this helper below `require_u64`:

```rust
fn optional_u64(params: &BTreeMap<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => {
            let integer = value
                .as_integer()
                .ok_or_else(|| format!("{key} must be an integer"))?;
            u64::try_from(integer)
                .map(Some)
                .map_err(|_| format!("{key} must be non-negative"))
        }
    }
}
```

- [ ] **Step 5: Inject shared provenance and use point seed**

In `rsinter/src/bench/runners/mod.rs`, replace:

```rust
    let result_params = merge_decoder_params(built.params, decoder_params);
```

with:

```rust
    let mut result_params = merge_decoder_params(built.params, decoder_params);
    result_params.insert("decoder_impl".into(), serde_json::json!(runner_name));
    result_params.insert("seed".into(), serde_json::json!(point.seed));
```

Replace:

```rust
    let mut rng = StdRng::seed_from_u64(ctx.seed);
```

with:

```rust
    let mut rng = StdRng::seed_from_u64(point.seed);
```

Update the `surface_point` helper in the internal tests in `rsinter/src/bench/runners/mod.rs` by adding `seed: 12_345,` after `p,`.

- [ ] **Step 6: Run focused tests and then the registry/run suites**

Run:

```sh
cargo test -p rsinter --test bench_registry expand_runner_points_accepts_and_defaults_seed expand_runner_points_rejects_invalid_seed
cargo test -p rsinter --test bench_run rust_benchmark_results_use_runner_name_not_impl_key
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_run
```

Expected: all commands pass.

- [ ] **Step 7: Commit Task 1**

Run:

```sh
git add rsinter/src/bench/registry.rs rsinter/src/bench/runners/mod.rs rsinter/tests/bench_registry.rs rsinter/tests/bench_run.rs
git commit -m "feat: record rsinter benchmark seed provenance"
```

---

### Task 2: Predict-Zero Benchmark Runner

**Files:**
- Create: `rsinter/src/bench/runners/predict_zero.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/tests/bench_registry.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `VacuousDecoder`, `RustBenchRunner`, `run_decoder_point`, `BenchCasePoint`.
- Produces: default rust runner implementation key `predict-zero`.

- [ ] **Step 1: Write failing registry assertions**

In `rsinter/tests/bench_registry.rs`, update the three default-runner tests:

```rust
assert!(registry.contains_key("predict-zero"));
```

```rust
assert_eq!(registry.get("predict-zero").unwrap().name(), "predict-zero");
```

```rust
assert!(names.contains(&"predict-zero".to_string()));
```

- [ ] **Step 2: Write failing predict-zero BB72 smoke test**

Add this test in `rsinter/tests/bench_run.rs` after `rust_benchmark_run_supports_bb72_css_explicit_observables`:

```rust
#[test]
fn predict_zero_benchmark_runs_bb72_css_negative_control() {
    let spec_text = r#"
name = "bb72_predict_zero"
version = 1
mode = "independent"

[[runner]]
name = "predict-zero-v1"
language = "rust"
impl_key = "predict-zero"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.001]
seed = 12345
max_shots = 64
max_errors = 64
batch_size = 64

[plot]
title = "BB72 Predict Zero"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench");

    let artifact_root = run_rust_benchmark(&spec, "rust", dir.path(), &registry, &spec_dir)
        .unwrap();
    let data = fs::read(
        artifact_root
            .join("predict-zero-v1")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].runner, "predict-zero-v1");
    assert_eq!(rows[0].params["decoder_impl"], serde_json::json!("predict-zero"));
    assert_eq!(rows[0].params["seed"], serde_json::json!(12_345));
    assert_eq!(rows[0].params["input_type"], serde_json::json!("css"));
    assert_eq!(
        rows[0].params["code_id"],
        serde_json::json!("bivariate-bicycle-code-m6-n6")
    );
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);

    let logical_error_rate = rows[0].metrics["logical_error_rate"];
    assert!(
        (0.35..=0.65).contains(&logical_error_rate),
        "predict-zero control LER was {logical_error_rate}"
    );
}
```

- [ ] **Step 3: Run focused tests and confirm they fail**

Run:

```sh
cargo test -p rsinter --test bench_registry default_rust_runner_registry_contains_workspace_decoders default_rust_runner_registry_exposes_runner_names default_rust_runner_names_include_workspace_decoders
cargo test -p rsinter --test bench_run predict_zero_benchmark_runs_bb72_css_negative_control
```

Expected: tests fail because `predict-zero` is not registered.

- [ ] **Step 4: Add the predict-zero runner file**

Create `rsinter/src/bench/runners/predict_zero.rs`:

```rust
use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, ParamMap};
use crate::bench::runners::run_decoder_point;
use crate::decode::VacuousDecoder;

pub struct PredictZeroRunner;

impl RustBenchRunner for PredictZeroRunner {
    fn name(&self) -> &'static str {
        "predict-zero"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        run_decoder_point(self.name(), &VacuousDecoder, point, ctx, &ParamMap::new())
    }
}
```

In `rsinter/src/bench/runners/mod.rs`, add:

```rust
pub mod predict_zero;
```

- [ ] **Step 5: Register the runner**

In `rsinter/src/bench/registry.rs`, add the import:

```rust
use crate::bench::runners::predict_zero::PredictZeroRunner;
```

Replace `default_rust_runner_names` with:

```rust
pub fn default_rust_runner_names() -> Vec<String> {
    ["rmatching", "rbposd", "rilpqec", "predict-zero"]
        .into_iter()
        .map(|name| name.to_string())
        .collect()
}
```

Add this line to `build_default_rust_runner_registry` after `rilpqec`:

```rust
    registry.insert("predict-zero".into(), Box::new(PredictZeroRunner));
```

No change is needed in `is_decoder_param_key`: unknown params for `predict-zero` should continue to fail through the `_ => false` branch.

- [ ] **Step 6: Run focused tests and then the full rsinter benchmark tests**

Run:

```sh
cargo test -p rsinter --test bench_registry default_rust_runner_registry_contains_workspace_decoders default_rust_runner_registry_exposes_runner_names default_rust_runner_names_include_workspace_decoders
cargo test -p rsinter --test bench_run predict_zero_benchmark_runs_bb72_css_negative_control
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_run
```

Expected: all commands pass. The standalone predict-zero negative-control smoke uses
`p = 0.001` because the fixed-seed BB72 explicit-observable sample lands in the
requested `0.35..=0.65` control window there, while `p = 0.003` and `p = 0.01`
are too high for this small all-zero-prediction smoke. Later BB72 BP+OSD and
manual reference fixtures keep their `p = 0.003` and `p = 0.01` reference
points.

- [ ] **Step 7: Commit Task 2**

Run:

```sh
git add rsinter/src/bench/registry.rs rsinter/src/bench/runners/mod.rs rsinter/src/bench/runners/predict_zero.rs rsinter/tests/bench_registry.rs rsinter/tests/bench_run.rs
git commit -m "feat: add predict-zero rsinter runner"
```

---

### Task 3: Rbposd Paper-Facing Parameter Labels

**Files:**
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: existing `rbposd::DecoderConfig` defaults and `optional_string`.
- Produces: accepted decoder params `bp_algorithm = "min_sum"` and `osd_method = "combination_sweep"`; normalized result params always include both labels.

- [ ] **Step 1: Extend the normalized-param test**

In `rbposd_benchmark_records_normalized_decoder_params` in `rsinter/tests/bench_run.rs`, add the two input lines below `batch_size = 4`:

```toml
bp_algorithm = "min_sum"
osd_method = "combination_sweep"
```

Then add these assertions after `assert_eq!(rows[0].params["osd_order"], serde_json::json!(10));`:

```rust
    assert_eq!(
        rows[0].params["bp_algorithm"],
        serde_json::json!("min_sum")
    );
    assert_eq!(
        rows[0].params["osd_method"],
        serde_json::json!("combination_sweep")
    );
```

- [ ] **Step 2: Add failing validation tests for unsupported values**

Add these tests in `rsinter/tests/bench_run.rs` after `rbposd_benchmark_rejects_both_bp_iteration_aliases`:

```rust
#[test]
fn rbposd_benchmark_rejects_unsupported_bp_algorithm() {
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
bp_algorithm = "sum_product"

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
        "rbposd bp_algorithm must be \"min_sum\", got \"sum_product\""
    );
    assert!(!dir.path().join("rbposd_bad").exists());
}

#[test]
fn rbposd_benchmark_rejects_unsupported_osd_method() {
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
osd_method = "unknown_method"

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
        "rbposd osd_method must be \"combination_sweep\", got \"unknown_method\""
    );
    assert!(!dir.path().join("rbposd_bad").exists());
}
```

- [ ] **Step 3: Run focused tests and confirm they fail**

Run:

```sh
cargo test -p rsinter --test bench_run rbposd_benchmark_records_normalized_decoder_params rbposd_benchmark_rejects_unsupported_bp_algorithm rbposd_benchmark_rejects_unsupported_osd_method
```

Expected: tests fail because `bp_algorithm` and `osd_method` are unknown `rbposd` params.

- [ ] **Step 4: Allow the new decoder-specific keys**

In `rsinter/src/bench/registry.rs`, extend the `rbposd` branch in `is_decoder_param_key`:

```rust
        "rbposd" => matches!(
            key,
            "bp_algorithm"
                | "bp_iters"
                | "max_bp_iterations"
                | "early_stop"
                | "osd_method"
                | "osd_order"
        ),
```

- [ ] **Step 5: Parse and normalize the labels**

In `rsinter/src/bench/runners/rbposd.rs`, update the params import:

```rust
use crate::bench::runners::params::{optional_bool, optional_string, optional_usize};
```

In `RbposdRunnerParams::parse`, add this code after `let mut config = DecoderConfig::default();`:

```rust
        let bp_algorithm =
            optional_string(params, "bp_algorithm")?.unwrap_or_else(|| "min_sum".to_string());
        if bp_algorithm != "min_sum" {
            return Err(format!(
                "rbposd bp_algorithm must be \"min_sum\", got \"{bp_algorithm}\""
            ));
        }
        let osd_method = optional_string(params, "osd_method")?
            .unwrap_or_else(|| "combination_sweep".to_string());
        if osd_method != "combination_sweep" {
            return Err(format!(
                "rbposd osd_method must be \"combination_sweep\", got \"{osd_method}\""
            ));
        }
```

Replace the normalized map with:

```rust
            normalized: ParamMap::from_pairs([
                ("bp_algorithm", serde_json::json!(bp_algorithm)),
                ("bp_iters", serde_json::json!(config.max_bp_iterations)),
                ("early_stop", serde_json::json!(config.early_stop)),
                ("osd_method", serde_json::json!(osd_method)),
                ("osd_order", serde_json::json!(config.osd_order)),
            ]),
```

- [ ] **Step 6: Run focused tests and benchmark suites**

Run:

```sh
cargo test -p rsinter --test bench_run rbposd_benchmark_records_normalized_decoder_params rbposd_benchmark_rejects_unsupported_bp_algorithm rbposd_benchmark_rejects_unsupported_osd_method
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_run
```

Expected: all commands pass.

- [ ] **Step 7: Commit Task 3**

Run:

```sh
git add rsinter/src/bench/registry.rs rsinter/src/bench/runners/rbposd.rs rsinter/tests/bench_run.rs
git commit -m "feat: record rbposd bp osd labels"
```

---

### Task 4: Fast BB72 BP+OSD And Predict-Zero Fixture

**Files:**
- Create: `rsinter/tests/fixtures/bench/bb72_css_bposd_decoder.toml`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `predict-zero` runner from Task 2, `rbposd` labels from Task 3, generic seed/provenance from Task 1.
- Produces: a committed fast fixture that exercises BB72 CSS explicit observables through `rbposd` and `predict-zero`.

- [ ] **Step 1: Create the fast fixture**

Create `rsinter/tests/fixtures/bench/bb72_css_bposd_decoder.toml`:

```toml
name = "bb72_css_bposd"
version = 1
mode = "independent"

[[runner]]
name = "rbposd-osd10-v1"
language = "rust"
impl_key = "rbposd"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.003]
seed = 12345
max_shots = 64
max_errors = 32
batch_size = 64
bp_algorithm = "min_sum"
bp_iters = 50
early_stop = true
osd_method = "combination_sweep"
osd_order = 10

[[runner]]
name = "predict-zero-v1"
language = "rust"
impl_key = "predict-zero"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.003]
seed = 12345
max_shots = 64
max_errors = 64
batch_size = 64

[plot]
title = "BB72 CSS BP+OSD"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
```

- [ ] **Step 2: Write the fixture integration test**

Add this test in `rsinter/tests/bench_run.rs` after `predict_zero_benchmark_runs_bb72_css_negative_control`:

```rust
#[test]
fn rust_benchmark_run_supports_bb72_css_bposd_fixture() {
    let spec_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench/bb72_css_bposd_decoder.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();

    let rbposd_data = fs::read(
        artifact_root
            .join("rbposd-osd10-v1")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rbposd_rows = read_results_jsonl(&rbposd_data[..]).unwrap();
    assert_eq!(rbposd_rows.len(), 1);
    let rbposd_row = &rbposd_rows[0];
    assert_eq!(rbposd_row.params["input_type"], serde_json::json!("css"));
    assert_eq!(
        rbposd_row.params["code_id"],
        serde_json::json!("bivariate-bicycle-code-m6-n6")
    );
    assert_eq!(
        rbposd_row.params["logical_observable_source"],
        serde_json::json!("explicit")
    );
    assert_eq!(rbposd_row.params["decoder_impl"], serde_json::json!("rbposd"));
    assert_eq!(rbposd_row.params["seed"], serde_json::json!(12_345));
    assert_eq!(
        rbposd_row.params["bp_algorithm"],
        serde_json::json!("min_sum")
    );
    assert_eq!(rbposd_row.params["bp_iters"], serde_json::json!(50));
    assert_eq!(
        rbposd_row.params["osd_method"],
        serde_json::json!("combination_sweep")
    );
    assert_eq!(rbposd_row.params["osd_order"], serde_json::json!(10));
    assert_eq!(rbposd_row.case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(rbposd_row.status, "ok");
    assert_eq!(rbposd_row.error, None);

    let predict_zero_data = fs::read(
        artifact_root
            .join("predict-zero-v1")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let predict_zero_rows = read_results_jsonl(&predict_zero_data[..]).unwrap();
    assert_eq!(predict_zero_rows.len(), 1);
    let predict_zero_row = &predict_zero_rows[0];
    assert_eq!(
        predict_zero_row.params["decoder_impl"],
        serde_json::json!("predict-zero")
    );
    assert_eq!(predict_zero_row.params["seed"], serde_json::json!(12_345));
    assert_eq!(predict_zero_row.case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(predict_zero_row.status, "ok");
    assert_eq!(predict_zero_row.error, None);
    let logical_error_rate = predict_zero_row.metrics["logical_error_rate"];
    assert!(
        (0.35..=0.65).contains(&logical_error_rate),
        "predict-zero fixture LER was {logical_error_rate}"
    );
}
```

- [ ] **Step 3: Run the fixture test and benchmark suites**

Run:

```sh
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_bb72_css_bposd_fixture
cargo test -p rsinter --test bench_run
```

Expected: all commands pass. If the OSD10 row makes the test too slow, change only the `rbposd-osd10-v1` fixture budget to `max_shots = 0`, keep all rbposd provenance assertions, and keep the predict-zero runner at `max_shots = 64`.

- [ ] **Step 4: Commit Task 4**

Run:

```sh
git add rsinter/tests/fixtures/bench/bb72_css_bposd_decoder.toml rsinter/tests/bench_run.rs
git commit -m "test: add bb72 bposd rsinter fixture"
```

---

### Task 5: Manual Paper-Reference Fixture

**Files:**
- Create: `rsinter/tests/fixtures/bench/bb72_css_bposd_reference.toml`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: an ignored/manual heavy fixture with the paper BP+OSD settings and explicit reference-point comments.

- [ ] **Step 1: Create the manual reference fixture**

Create `rsinter/tests/fixtures/bench/bb72_css_bposd_reference.toml`:

```toml
# Manual BB72 BP+OSD reference fixture for issue #103.
# Paper settings: MIN-SUM BP, 10000 BP iterations, combination-sweep OSD,
# OSD order 10.
# Table 6 formula:
#   p_L(p) = p^(d_circ / 2) * exp(c0 + c1*p + c2*p^2)
# BB72 reference constants:
#   d_circ = 6
#   c0 = 11.09
#   c1 = 365.6
#   c2 = -16088
# Useful reference points:
#   p = 0.003 -> p_L ~= 0.00458
#   p = 0.01  -> p_L ~= 0.507
name = "bb72_css_bposd_reference"
version = 1
mode = "independent"

[[runner]]
name = "rbposd-osd10-reference"
language = "rust"
impl_key = "rbposd"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.003, 0.01]
seed = 12345
max_shots = 2000
max_errors = 200
batch_size = 64
bp_algorithm = "min_sum"
bp_iters = 10000
early_stop = true
osd_method = "combination_sweep"
osd_order = 10

[plot]
title = "BB72 CSS BP+OSD Reference"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
```

- [ ] **Step 2: Add an ignored test that exercises the fixture**

Add this test in `rsinter/tests/bench_run.rs` after `rust_benchmark_run_supports_bb72_css_bposd_fixture`:

```rust
#[test]
#[ignore = "manual BB72 BP+OSD reference run; intentionally heavier than CI"]
fn manual_bb72_css_bposd_reference_fixture_records_paper_params() {
    let spec_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench/bb72_css_bposd_reference.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rbposd-osd10-reference")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();
    assert_eq!(rows.len(), 2);

    let mut seen_p003 = false;
    let mut seen_p01 = false;
    for row in rows {
        assert_eq!(row.params["decoder_impl"], serde_json::json!("rbposd"));
        assert_eq!(row.params["seed"], serde_json::json!(12_345));
        assert_eq!(row.params["bp_algorithm"], serde_json::json!("min_sum"));
        assert_eq!(row.params["bp_iters"], serde_json::json!(10_000));
        assert_eq!(
            row.params["osd_method"],
            serde_json::json!("combination_sweep")
        );
        assert_eq!(row.params["osd_order"], serde_json::json!(10));
        assert_eq!(row.params["logical_observable_source"], serde_json::json!("explicit"));
        assert_eq!(row.case_summary["num_obs"], serde_json::json!(12));
        assert_eq!(row.status, "ok");
        assert_eq!(row.error, None);

        let p = row.params["p"].as_f64().unwrap();
        if (p - 0.003).abs() < f64::EPSILON {
            seen_p003 = true;
        }
        if (p - 0.01).abs() < f64::EPSILON {
            seen_p01 = true;
        }
    }

    assert!(seen_p003);
    assert!(seen_p01);
}
```

- [ ] **Step 3: Run ignored-list check and normal benchmark tests**

Run:

```sh
cargo test -p rsinter --test bench_run manual_bb72_css_bposd_reference_fixture_records_paper_params -- --ignored
cargo test -p rsinter --test bench_run
```

Expected:

- The ignored test may take materially longer than normal CI; run it once locally before committing if the machine budget allows.
- The normal `bench_run` suite passes without running the ignored reference test.

- [ ] **Step 4: Commit Task 5**

Run:

```sh
git add rsinter/tests/fixtures/bench/bb72_css_bposd_reference.toml rsinter/tests/bench_run.rs
git commit -m "test: document bb72 bposd reference fixture"
```

---

### Task 6: Final Verification And Issue Sweep

**Files:**
- Inspect: all files changed by Tasks 1-5.

**Interfaces:**
- Consumes: complete implementation from prior tasks.
- Produces: verified branch ready for review or PR.

- [ ] **Step 1: Run the required fast verification**

Run:

```sh
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_run
cargo test -p qec-code --test code bb72
```

Expected: all commands pass.

- [ ] **Step 2: Run exact-distance smoke if the ILP feature is available**

Run:

```sh
cargo test -p qec-code --features ilp --test cli code_css_distance_exact_bb72_known_distance_with_ilp
```

Expected: pass when local ILP dependencies are available. If the command cannot run because the feature dependencies are missing, record the exact error in the final handoff and do not claim this optional verification passed.

- [ ] **Step 3: Inspect changed files**

Run:

```sh
git status --short
git diff --stat master..HEAD
git diff --check master..HEAD
```

Expected:

- `git diff --check master..HEAD` exits successfully.
- The diff contains only the intended `rsinter` benchmark runner, fixtures, tests, and the already committed design/plan docs.

- [ ] **Step 4: Final commit if verification changed tracked files**

If Step 1 or Step 2 produced tracked fixture updates, commit them:

```sh
git add rsinter/tests/fixtures/bench rsinter/tests/bench_run.rs
git commit -m "test: refresh issue 103 fixtures"
```

Expected: either a new commit is created for tracked fixture changes, or there are no tracked changes to commit.

- [ ] **Step 5: Summarize completion**

Prepare a concise handoff that includes:

```text
Implemented issue #103 rsinter BP+OSD path.

Key changes:
- predict-zero runner exposed through the default rust benchmark registry
- generic seed input and result provenance
- decoder_impl result provenance
- rbposd bp_algorithm/osd_method labels with preflight validation
- BB72 CSS BP+OSD fast fixture and manual reference fixture

Verification:
- cargo test -p rsinter --test bench_registry
- cargo test -p rsinter --test bench_run
- cargo test -p qec-code --test code bb72
- optional ILP exact-distance command result
```
