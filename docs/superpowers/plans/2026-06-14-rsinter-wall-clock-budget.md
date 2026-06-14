# rsinter Wall-Clock Budget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `max_wall_seconds` as a wall-clock stopping rule for `rsinter` collection and Rust benchmark runner loops.

**Architecture:** Extend the existing option structs and bench point model with `Option<f64>` wall-clock budgets. Keep shots and errors as required bench limits, and treat wall-clock time as a third optional stop condition checked before each batch and updated after each whole batch. No decoder trait changes are required.

**Tech Stack:** Rust 2024, Cargo workspace, `rsinter` crate integration tests, existing `rstim` sampler and `rsinter::decode` traits.

---

## File Structure

- Modify `rsinter/src/task.rs`: add `max_wall_seconds` to per-task `CollectionOptions`.
- Modify `rsinter/src/collect.rs`: add global `CollectOptions::max_wall_seconds`, validate budgets, time full batches, and stop by shots/errors/time.
- Modify `rsinter/tests/collect.rs`: add slow decoder tests and update existing struct literals with `max_wall_seconds: None`.
- Modify `rsinter/tests/decode_rbposd.rs`, `rsinter/tests/decode_ilp.rs`, and `rsinter/tests/integration.rs`: update direct `CollectionOptions` and `CollectOptions` literals.
- Modify `rsinter/src/bench/registry.rs`: add optional `max_wall_seconds` to generic runner params and `BenchCasePoint`.
- Modify `rsinter/src/bench/circuit_source.rs`: include `max_wall_seconds` in result params when configured.
- Modify `rsinter/tests/bench_registry.rs`: add parser and validation tests.
- Modify `rsinter/src/bench/runners/mod.rs`: stop runner loop by wall-clock time and emit `wall_seconds` metric.
- Modify `rsinter/tests/bench_runner_wrappers.rs`: update direct `BenchCasePoint` literals.

---

### Task 1: Add Failing Collect Tests

**Files:**
- Modify: `rsinter/tests/collect.rs`

- [ ] **Step 1: Add slow decoder test helpers**

In `rsinter/tests/collect.rs`, replace the decode import and add the slow decoder helpers near the existing imports:

```rust
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::{CompiledDecoder, Decoder, VacuousDecoder};
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

struct SlowDecoder {
    sleep: Duration,
}

struct SlowCompiledDecoder {
    sleep: Duration,
}

impl Decoder for SlowDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        Box::new(SlowCompiledDecoder { sleep: self.sleep })
    }
}

impl CompiledDecoder for SlowCompiledDecoder {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        thread::sleep(self.sleep);
        let obs_bytes = num_obs.div_ceil(8);
        vec![0u8; num_shots * obs_bytes]
    }
}
```

- [ ] **Step 2: Update the existing task literal**

In `make_task`, add the new field so the old tests state that they do not use a wall-clock budget:

```rust
collection_options: CollectionOptions {
    max_shots: Some(1000),
    max_errors: None,
    max_wall_seconds: None,
},
```

- [ ] **Step 3: Add a slow-decoder factory**

Add this helper below `make_decoders`:

```rust
fn make_slow_decoders(
    sleep: Duration,
) -> HashMap<String, Box<dyn rsinter::decode::Decoder>> {
    let mut decoders: HashMap<String, Box<dyn rsinter::decode::Decoder>> = HashMap::new();
    decoders.insert("vacuous".into(), Box::new(SlowDecoder { sleep }));
    decoders
}
```

- [ ] **Step 4: Update existing `CollectOptions` literals in this file**

Add `max_wall_seconds: None` to each existing `CollectOptions` literal:

```rust
let options = CollectOptions {
    num_workers: 1,
    max_shots: Some(1000),
    max_errors: None,
    max_wall_seconds: None,
    max_batch_size: Some(256),
    start_batch_size: 64,
    save_resume_filepath: None,
    print_progress: false,
};
```

Use the same placement for the other `CollectOptions` values in `collect_respects_max_errors` and `collect_csv_resume`.

- [ ] **Step 5: Add the wall-clock collect test**

Append this test:

```rust
#[test]
fn collect_respects_wall_clock() {
    let mut task = make_task();
    task.collection_options = CollectionOptions {
        max_shots: None,
        max_errors: None,
        max_wall_seconds: None,
    };

    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: Some(0.09),
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let results = collect(
        vec![task],
        make_slow_decoders(Duration::from_millis(35)),
        &options,
    )
    .unwrap();

    let stats = &results[0];
    assert!(stats.seconds >= 0.09, "seconds={}", stats.seconds);
    assert!(stats.seconds < 0.5, "seconds={}", stats.seconds);
    assert!(stats.shots > 0, "shots={}", stats.shots);
    assert!(stats.shots < 20, "shots={}", stats.shots);
}
```

- [ ] **Step 6: Add the negative-control collect test**

Append this test:

```rust
#[test]
fn collect_rejects_non_positive_wall_clock() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: Some(20),
        max_errors: None,
        max_wall_seconds: Some(0.0),
        max_batch_size: Some(1),
        start_batch_size: 1,
        save_resume_filepath: None,
        print_progress: false,
    };

    let err = collect(vec![make_task()], make_decoders(), &options).unwrap_err();

    assert!(err.contains("max_wall_seconds must be positive"), "{err}");
}
```

- [ ] **Step 7: Run the new collect tests to verify they fail**

Run:

```bash
cargo test -p rsinter collect_respects_wall_clock
```

Expected: FAIL to compile because `CollectionOptions` and `CollectOptions` do not yet have `max_wall_seconds`.

Run:

```bash
cargo test -p rsinter collect_rejects_non_positive_wall_clock
```

Expected: FAIL to compile for the same missing-field reason.

---

### Task 2: Implement Collection Wall-Clock Budgets

**Files:**
- Modify: `rsinter/src/task.rs`
- Modify: `rsinter/src/collect.rs`
- Modify: `rsinter/tests/decode_rbposd.rs`
- Modify: `rsinter/tests/decode_ilp.rs`
- Modify: `rsinter/tests/integration.rs`
- Test: `rsinter/tests/collect.rs`

- [ ] **Step 1: Add the per-task option field**

In `rsinter/src/task.rs`, change `CollectionOptions` to:

```rust
#[derive(Clone, Debug, Default)]
pub struct CollectionOptions {
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
    pub max_wall_seconds: Option<f64>,
}
```

- [ ] **Step 2: Add the global collect option field**

In `rsinter/src/collect.rs`, change `CollectOptions` to:

```rust
pub struct CollectOptions {
    pub num_workers: usize,
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
    pub max_wall_seconds: Option<f64>,
    pub max_batch_size: Option<usize>,
    pub start_batch_size: usize,
    pub save_resume_filepath: Option<PathBuf>,
    pub print_progress: bool,
}
```

- [ ] **Step 3: Add validation and stop helpers in `collect.rs`**

Add these helper functions above `pub fn collect`:

```rust
fn validate_max_wall_seconds(max_wall_seconds: Option<f64>) -> Result<(), String> {
    if let Some(seconds) = max_wall_seconds {
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err("max_wall_seconds must be positive".into());
        }
    }
    Ok(())
}

fn under_wall_budget(total_seconds: f64, max_wall_seconds: Option<f64>) -> bool {
    match max_wall_seconds {
        Some(max_seconds) => total_seconds < max_seconds,
        None => true,
    }
}

fn should_continue_collecting(
    total_shots: u64,
    total_errors: u64,
    total_seconds: f64,
    max_shots: u64,
    max_errors: u64,
    max_wall_seconds: Option<f64>,
) -> bool {
    total_shots < max_shots
        && total_errors < max_errors
        && under_wall_budget(total_seconds, max_wall_seconds)
}
```

- [ ] **Step 4: Validate budgets before starting collection**

At the top of `collect`, before loading existing data, add:

```rust
validate_max_wall_seconds(options.max_wall_seconds)?;
for task in &tasks {
    validate_max_wall_seconds(task.collection_options.max_wall_seconds)?;
}
```

- [ ] **Step 5: Resolve the effective per-task wall-clock budget**

Inside the per-task map closure, after `max_errors`, add:

```rust
let max_wall_seconds = task
    .collection_options
    .max_wall_seconds
    .or(options.max_wall_seconds);
```

- [ ] **Step 6: Time whole collect batches and stop by time**

Replace the current loop header and sampler-only timing block in `collect.rs` with this structure:

```rust
while should_continue_collecting(
    total_shots,
    total_errors,
    total_seconds,
    max_shots,
    max_errors,
    max_wall_seconds,
) {
    let remaining = (max_shots - total_shots) as usize;
    let n = batch_size.min(remaining);
    if n == 0 {
        break;
    }

    let batch_started = Instant::now();
    let batch = sample_batch(&task.circuit, n, &mut rng).unwrap();

    let mut det_buf = Vec::new();
    write_shots_b8(&batch.detections, &mut det_buf).unwrap();
    let mut obs_buf = Vec::new();
    write_shots_b8(&batch.observable_flips, &mut obs_buf).unwrap();

    let predictions = compiled.decode_shots_bit_packed(&det_buf, n, num_dets, num_obs);

    let mut batch_errors = 0u64;
    for shot in 0..n {
        let offset = shot * obs_bytes_per_shot;
        let mut mismatch = false;
        for byte in 0..obs_bytes_per_shot {
            if predictions[offset + byte] != obs_buf[offset + byte] {
                mismatch = true;
                break;
            }
        }
        if mismatch {
            batch_errors += 1;
        }
    }

    total_shots += n as u64;
    total_errors += batch_errors;
    total_seconds += batch_started.elapsed().as_secs_f64();

    if let Some(max) = options.max_batch_size {
        batch_size = (batch_size * 2).min(max);
    } else {
        batch_size *= 2;
    }
}
```

- [ ] **Step 7: Update direct collect option literals outside `collect.rs`**

Add `max_wall_seconds: None` to each direct `CollectionOptions` and `CollectOptions` literal in these files:

```text
rsinter/tests/decode_rbposd.rs
rsinter/tests/decode_ilp.rs
rsinter/tests/integration.rs
```

For `CollectionOptions`, use:

```rust
CollectionOptions {
    max_shots: Some(32),
    max_errors: Some(32),
    max_wall_seconds: None,
}
```

For `CollectOptions`, use:

```rust
CollectOptions {
    num_workers: 1,
    max_shots: None,
    max_errors: None,
    max_wall_seconds: None,
    max_batch_size: Some(32),
    start_batch_size: 8,
    save_resume_filepath: None,
    print_progress: false,
}
```

Keep each file's existing `max_shots`, `max_errors`, `max_batch_size`, and `start_batch_size` values; add only the new field.

- [ ] **Step 8: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: command exits successfully and formats the touched Rust files.

- [ ] **Step 9: Run collect tests**

Run:

```bash
cargo test -p rsinter collect_respects_wall_clock
```

Expected: PASS.

Run:

```bash
cargo test -p rsinter collect_rejects_non_positive_wall_clock
```

Expected: PASS.

Run:

```bash
cargo test -p rsinter --test collect
```

Expected: PASS for all collect integration tests.

- [ ] **Step 10: Commit collection changes**

Run:

```bash
git add rsinter/src/task.rs rsinter/src/collect.rs rsinter/tests/collect.rs rsinter/tests/decode_rbposd.rs rsinter/tests/decode_ilp.rs rsinter/tests/integration.rs
git commit -m "feat: add rsinter collect wall-clock budget"
```

Expected: commit succeeds with only the collection-related files staged.

---

### Task 3: Add Failing Bench Registry Tests

**Files:**
- Modify: `rsinter/tests/bench_registry.rs`

- [ ] **Step 1: Add parsing test for optional wall-clock budget**

Add this test after `expand_runner_points_defaults_to_legacy_surface_input`:

```rust
#[test]
fn expand_runner_points_accepts_optional_max_wall_seconds() {
    let mut params = valid_runner_params();
    params.insert("max_wall_seconds".into(), toml::Value::Float(2.5));

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].max_wall_seconds, Some(2.5));
}
```

- [ ] **Step 2: Add validation test for bad wall-clock budgets**

Add this test near the other validation tests:

```rust
#[test]
fn expand_runner_points_rejects_non_positive_max_wall_seconds() {
    let mut params = valid_runner_params();
    params.insert("max_wall_seconds".into(), toml::Value::Float(0.0));
    assert_eq!(expand_points_err(&params), "max_wall_seconds must be positive");

    let mut params = valid_runner_params();
    params.insert("max_wall_seconds".into(), toml::Value::Float(-1.0));
    assert_eq!(expand_points_err(&params), "max_wall_seconds must be positive");
}
```

- [ ] **Step 3: Add compatibility test that `max_shots` remains required**

Add this test:

```rust
#[test]
fn expand_runner_points_still_requires_max_shots_with_wall_clock_budget() {
    let mut params = valid_runner_params();
    params.remove("max_shots");
    params.insert("max_wall_seconds".into(), toml::Value::Float(2.5));

    assert_eq!(expand_points_err(&params), "missing runner param: max_shots");
}
```

- [ ] **Step 4: Run registry tests to verify they fail**

Run:

```bash
cargo test -p rsinter --test bench_registry max_wall_seconds
```

Expected: FAIL to compile because `BenchCasePoint` does not yet have `max_wall_seconds` and the parser does not accept the key.

---

### Task 4: Implement Bench Registry Wall-Clock Parsing

**Files:**
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/src/bench/circuit_source.rs`
- Test: `rsinter/tests/bench_registry.rs`

- [ ] **Step 1: Add the bench point field**

In `rsinter/src/bench/registry.rs`, add the field after `max_errors`:

```rust
pub max_shots: u64,
pub max_errors: u64,
pub max_wall_seconds: Option<f64>,
pub batch_size: usize,
```

- [ ] **Step 2: Parse and validate the optional runner param**

In `expand_generic_runner_points`, add parsing after `max_errors`:

```rust
let max_wall_seconds = optional_f64(params, "max_wall_seconds")?;
validate_max_wall_seconds(max_wall_seconds)?;
```

Pass `max_wall_seconds` into both `expand_surface_points` and `expand_css_points`.

- [ ] **Step 3: Update point expansion function signatures**

Change the signatures to include the new argument:

```rust
fn expand_surface_points(
    params: &BTreeMap<String, Value>,
    rounds: &[Value],
    ps: &[Value],
    max_shots: u64,
    max_errors: u64,
    max_wall_seconds: Option<f64>,
    batch_size: usize,
    decoder_params: BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String>
```

```rust
fn expand_css_points(
    params: &BTreeMap<String, Value>,
    rounds: &[Value],
    ps: &[Value],
    max_shots: u64,
    max_errors: u64,
    max_wall_seconds: Option<f64>,
    batch_size: usize,
    decoder_params: BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String>
```

- [ ] **Step 4: Store the field in every expanded point**

In both `BenchCasePoint` literals in `registry.rs`, add:

```rust
max_wall_seconds,
```

Place it between `max_errors` and `batch_size`.

- [ ] **Step 5: Accept `max_wall_seconds` as a generic runner param**

In `is_generic_param_key`, add the key:

```rust
| "max_wall_seconds"
```

- [ ] **Step 6: Add optional-f64 and validation helpers**

In `registry.rs`, add these helpers near `require_u64` and `value_as_f64`:

```rust
fn optional_f64(params: &BTreeMap<String, Value>, key: &str) -> Result<Option<f64>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value_as_f64(value, key).map(Some),
    }
}

fn validate_max_wall_seconds(max_wall_seconds: Option<f64>) -> Result<(), String> {
    if let Some(seconds) = max_wall_seconds {
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err("max_wall_seconds must be positive".into());
        }
    }
    Ok(())
}
```

- [ ] **Step 7: Include `max_wall_seconds` in bench result params**

In `rsinter/src/bench/circuit_source.rs`, add this helper near `build_surface`:

```rust
fn insert_max_wall_seconds(params: &mut ParamMap, max_wall_seconds: Option<f64>) {
    if let Some(seconds) = max_wall_seconds {
        params.insert("max_wall_seconds".into(), serde_json::json!(seconds));
    }
}
```

In `build_surface`, change the `params` construction to mutable and call the helper:

```rust
let mut params = ParamMap::from_pairs([
    ("input_type", serde_json::json!("surface_rotated_memory_x")),
    ("distance", serde_json::json!(distance)),
    ("rounds", serde_json::json!(point.rounds)),
    ("p", serde_json::json!(point.p)),
    ("max_shots", serde_json::json!(point.max_shots)),
    ("max_errors", serde_json::json!(point.max_errors)),
    ("batch_size", serde_json::json!(point.batch_size)),
]);
insert_max_wall_seconds(&mut params, point.max_wall_seconds);
```

Then return `params` in `BuiltCircuit`.

In `build_css`, after the existing `observables` insert, call:

```rust
insert_max_wall_seconds(&mut params, point.max_wall_seconds);
```

- [ ] **Step 8: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: command exits successfully.

- [ ] **Step 9: Run bench registry tests**

Run:

```bash
cargo test -p rsinter --test bench_registry
```

Expected: PASS.

- [ ] **Step 10: Commit bench registry changes**

Run:

```bash
git add rsinter/src/bench/registry.rs rsinter/src/bench/circuit_source.rs rsinter/tests/bench_registry.rs
git commit -m "feat: parse bench wall-clock budget"
```

Expected: commit succeeds with the registry, circuit source, and registry tests staged.

---

### Task 5: Add Failing Bench Runner Tests

**Files:**
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

- [ ] **Step 1: Update existing bench point literals in runner tests**

In the two existing `BenchCasePoint` literals inside `rsinter/src/bench/runners/mod.rs`, add:

```rust
max_wall_seconds: None,
```

Place it between `max_errors` and `batch_size`.

- [ ] **Step 2: Add slow decoder helpers to the runner test module**

Inside `#[cfg(test)] mod tests` in `rsinter/src/bench/runners/mod.rs`, add these imports and helpers after the existing empty-prediction types:

```rust
use std::thread;
use std::time::Duration;

struct SlowPredictionDecoder {
    sleep: Duration,
}

struct SlowPredictionCompiled {
    sleep: Duration,
}

impl Decoder for SlowPredictionDecoder {
    fn compile_for_dem(&self, _dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        Box::new(SlowPredictionCompiled { sleep: self.sleep })
    }
}

impl CompiledDecoder for SlowPredictionCompiled {
    fn decode_shots_bit_packed(
        &self,
        _dets: &[u8],
        num_shots: usize,
        _num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        thread::sleep(self.sleep);
        let obs_bytes = num_obs.div_ceil(8);
        vec![0u8; num_shots * obs_bytes]
    }
}
```

- [ ] **Step 3: Add wall-clock runner-loop test**

Add this test in the same test module:

```rust
#[test]
fn run_decoder_point_respects_wall_clock_budget() {
    let point = BenchCasePoint {
        input_type: "surface_rotated_memory_x".into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.0,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots: 20,
        max_errors: 20,
        max_wall_seconds: Some(0.09),
        batch_size: 1,
        decoder_params: BTreeMap::new(),
    };
    let ctx = BenchRunContext {
        benchmark_name: "surface_decoder".into(),
        runner_name: "fake".into(),
        language: "rust".into(),
        seed: 12_345,
        spec_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };

    let decoder = SlowPredictionDecoder {
        sleep: Duration::from_millis(35),
    };
    let decoder_params = crate::bench::result::ParamMap::new();
    let row = run_decoder_point("fake", &decoder, &point, &ctx, &decoder_params).unwrap();

    assert!(row.metrics["shots_used"] > 0.0);
    assert!(row.metrics["shots_used"] < 20.0);
    assert!(row.metrics["wall_seconds"] >= 0.09);
    assert!(row.metrics["wall_seconds"] < 0.5);
}
```

- [ ] **Step 4: Update wrapper test point literals**

In `rsinter/tests/bench_runner_wrappers.rs`, add `max_wall_seconds: None` to both `BenchCasePoint` literals:

```rust
max_shots: 0,
max_errors: 2,
max_wall_seconds: None,
batch_size: 4,
```

- [ ] **Step 5: Run runner tests to verify they fail**

Run:

```bash
cargo test -p rsinter run_decoder_point_respects_wall_clock_budget
```

Expected: FAIL because `run_decoder_point` does not yet emit `wall_seconds` or stop by `max_wall_seconds`.

---

### Task 6: Implement Bench Runner Wall-Clock Budgets

**Files:**
- Modify: `rsinter/src/bench/runners/mod.rs`
- Test: `rsinter/src/bench/runners/mod.rs`

- [ ] **Step 1: Add a wall-budget helper**

In `rsinter/src/bench/runners/mod.rs`, add this helper near the top-level `run_decoder_point` function:

```rust
fn under_wall_budget(total_seconds: f64, max_wall_seconds: Option<f64>) -> bool {
    match max_wall_seconds {
        Some(max_seconds) => total_seconds < max_seconds,
        None => true,
    }
}
```

- [ ] **Step 2: Track wall-clock seconds in the runner loop**

In `run_decoder_point`, after `let mut total_decode_us = 0.0;`, add:

```rust
let mut wall_seconds = 0.0;
```

- [ ] **Step 3: Stop the runner loop by time**

Replace the loop condition with:

```rust
while shots_used < max_shots
    && logical_errors < max_errors
    && under_wall_budget(wall_seconds, point.max_wall_seconds)
{
```

- [ ] **Step 4: Time whole runner batches**

At the start of the loop body, before sampling, add:

```rust
let batch_started = Instant::now();
```

After the per-shot counting loop, add:

```rust
wall_seconds += batch_started.elapsed().as_secs_f64();
```

- [ ] **Step 5: Emit `wall_seconds` metric**

In the metrics map, add:

```rust
("wall_seconds", wall_seconds),
```

Place it near `total_decode_us` so timing metrics stay together:

```rust
("compile_us", compile_us),
("total_decode_us", total_decode_us),
("wall_seconds", wall_seconds),
```

- [ ] **Step 6: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: command exits successfully.

- [ ] **Step 7: Run focused runner tests**

Run:

```bash
cargo test -p rsinter run_decoder_point_respects_wall_clock_budget
```

Expected: PASS.

Run:

```bash
cargo test -p rsinter --test bench_runner_wrappers
```

Expected: PASS.

- [ ] **Step 8: Commit runner changes**

Run:

```bash
git add rsinter/src/bench/runners/mod.rs rsinter/tests/bench_runner_wrappers.rs
git commit -m "feat: stop bench runner by wall-clock budget"
```

Expected: commit succeeds with runner-loop files staged.

---

### Task 7: Compile Sweep And Remaining Literal Fixes

**Files:**
- Modify only files that fail to compile because they directly construct `CollectionOptions`, `CollectOptions`, or `BenchCasePoint`.

- [ ] **Step 1: Run a compile sweep**

Run:

```bash
cargo test -p rsinter --no-run
```

Expected: either PASS or compile errors that point to remaining struct literals missing `max_wall_seconds`.

- [ ] **Step 2: Fix any remaining `CollectionOptions` literals**

For each compiler error on `CollectionOptions`, add:

```rust
max_wall_seconds: None,
```

Example final shape:

```rust
CollectionOptions {
    max_shots: Some(32),
    max_errors: Some(32),
    max_wall_seconds: None,
}
```

- [ ] **Step 3: Fix any remaining `CollectOptions` literals**

For each compiler error on `CollectOptions`, add:

```rust
max_wall_seconds: None,
```

Example final shape:

```rust
CollectOptions {
    num_workers: 1,
    max_shots: None,
    max_errors: None,
    max_wall_seconds: None,
    max_batch_size: Some(32),
    start_batch_size: 8,
    save_resume_filepath: None,
    print_progress: false,
}
```

- [ ] **Step 4: Fix any remaining `BenchCasePoint` literals**

For each compiler error on `BenchCasePoint`, add:

```rust
max_wall_seconds: None,
```

Example final shape:

```rust
BenchCasePoint {
    input_type: "surface_rotated_memory_x".into(),
    code_id: None,
    distance: Some(3),
    rounds: 3,
    p: 0.002,
    basis: None,
    schedule: None,
    hx_path: None,
    hz_path: None,
    observables_path: None,
    max_shots: 4,
    max_errors: 2,
    max_wall_seconds: None,
    batch_size: 2,
    decoder_params: BTreeMap::new(),
}
```

- [ ] **Step 5: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: command exits successfully.

- [ ] **Step 6: Re-run compile sweep**

Run:

```bash
cargo test -p rsinter --no-run
```

Expected: PASS.

- [ ] **Step 7: Commit mechanical compile fixes if any were needed**

If Step 2, Step 3, or Step 4 changed files, run:

```bash
git add rsinter
git commit -m "chore: update rsinter wall-clock option literals"
```

Expected: commit succeeds only when there were remaining literal fixes. If no files changed, skip this commit.

---

### Task 8: Final Verification

**Files:**
- No planned source changes.

- [ ] **Step 1: Run focused issue checks**

Run:

```bash
cargo test -p rsinter collect_respects_wall_clock
```

Expected: PASS.

Run:

```bash
cargo test -p rsinter collect_rejects_non_positive_wall_clock
```

Expected: PASS.

Run:

```bash
cargo test -p rsinter --test bench_registry
```

Expected: PASS.

Run:

```bash
cargo test -p rsinter run_decoder_point_respects_wall_clock_budget
```

Expected: PASS.

- [ ] **Step 2: Run crate-level verification**

Run:

```bash
cargo test -p rsinter
```

Expected: PASS.

- [ ] **Step 3: Inspect git status**

Run:

```bash
git status --short
```

Expected: no unstaged or staged source changes after the final implementation commit.

- [ ] **Step 4: Prepare issue summary**

Use this summary in the final response or PR body:

```text
Implemented issue 48 by adding max_wall_seconds to rsinter collection options and Rust bench runner params. Collection now validates positive finite wall-clock budgets, counts full batch wall-clock time in TaskStats.seconds, and stops when shots, errors, or wall time reaches the configured limit. Rust bench points parse optional max_wall_seconds, preserve required max_shots/max_errors, stop runner loops by wall time, and emit wall_seconds metrics.

Verification:
- cargo test -p rsinter collect_respects_wall_clock
- cargo test -p rsinter collect_rejects_non_positive_wall_clock
- cargo test -p rsinter --test bench_registry
- cargo test -p rsinter run_decoder_point_respects_wall_clock_budget
- cargo test -p rsinter
```
