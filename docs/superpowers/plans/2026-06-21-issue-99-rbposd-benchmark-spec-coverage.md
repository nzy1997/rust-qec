# Issue 99 rbposd Benchmark Spec Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add checked-in surface-decoder benchmark entries for LSD-backed and non-default BP-option `rbposd` runs, with tests that prove they parse through the existing registry.

**Architecture:** Keep the existing default `rbposd` benchmark row unchanged. Add two alias runner specs that reuse `impl_key = "rbposd"` and carry only the decoder params needed for LSD order-1 and product-sum serial BP coverage. Validate those checked-in TOML files through `BenchmarkSpec`, `expand_runner_points_for_runner`, and `RbposdRunner::preflight_point`.

**Tech Stack:** Rust 2024 Cargo workspace, `rsinter` integration tests, TOML benchmark specs, existing `rsinter::bench` registry APIs.

## Global Constraints

- Only update `benchmarks/surface_decoder/spec.toml`, `benchmarks/surface_decoder/full.toml`, and focused `rsinter` tests.
- First checked-in runner names must include `rbposd_lsd_order1` and `rbposd_product_sum_serial`.
- `rbposd_lsd_order1` uses `lsd_method = "localized_statistics"` and `lsd_order = 1`.
- `rbposd_product_sum_serial` uses `bp_method = "product_sum"` and `schedule = "serial"`.
- Do not regenerate benchmark artifacts.
- Do not change non-`rbposd` decoder specs or plot design.

---

### Task 1: Add Checked-In Spec Coverage Tests

**Files:**
- Create: `rsinter/tests/bench_specs.rs`

**Interfaces:**
- Consumes: `BenchmarkSpec::validate`, `build_default_rust_runner_registry`, `expand_runner_points_for_runner`, and `RustBenchRunner::preflight_point`.
- Produces: integration tests named `rbposd_benchmark_specs_cover_lsd_and_bp_option_runners` and `rbposd_benchmark_specs_reject_unknown_decoder_modes`.

- [ ] **Step 1: Write the failing tests**

Create `rsinter/tests/bench_specs.rs` with:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use rsinter::bench::registry::{
    BenchCasePoint, RustBenchRunner, build_default_rust_runner_registry,
    expand_runner_points_for_runner,
};
use rsinter::bench::spec::{BenchmarkSpec, RunnerSpec};
use toml::Value;

const LSD_RUNNER: &str = "rbposd_lsd_order1";
const BP_RUNNER: &str = "rbposd_product_sum_serial";

#[test]
fn rbposd_benchmark_specs_cover_lsd_and_bp_option_runners() {
    for spec_path in surface_decoder_spec_paths() {
        let spec = load_spec(&spec_path);
        spec.validate()
            .unwrap_or_else(|err| panic!("{} failed validation: {err}", spec_path.display()));

        let lsd_runner = runner_named(&spec, LSD_RUNNER);
        assert_rbposd_alias(lsd_runner);
        assert_eq!(
            lsd_runner
                .params
                .get("lsd_method")
                .and_then(Value::as_str),
            Some("localized_statistics")
        );
        assert_eq!(
            lsd_runner
                .params
                .get("lsd_order")
                .and_then(Value::as_integer),
            Some(1)
        );
        preflight_all_points(lsd_runner);

        let bp_runner = runner_named(&spec, BP_RUNNER);
        assert_rbposd_alias(bp_runner);
        assert_eq!(
            bp_runner.params.get("bp_method").and_then(Value::as_str),
            Some("product_sum")
        );
        assert_eq!(
            bp_runner.params.get("schedule").and_then(Value::as_str),
            Some("serial")
        );
        preflight_all_points(bp_runner);
    }
}

#[test]
fn rbposd_benchmark_specs_reject_unknown_decoder_modes() {
    let spec = load_spec(&workspace_root().join("benchmarks/surface_decoder/spec.toml"));

    let mut lsd_params = runner_named(&spec, LSD_RUNNER).params.clone();
    lsd_params.insert("lsd_method".into(), Value::String("bogus_lsd".into()));
    let err = preflight_err(&one_rbposd_point(&lsd_params));
    assert!(
        err.contains("rbposd lsd_method must be \"localized_statistics\""),
        "unexpected LSD validation error: {err}"
    );

    let mut bp_params = runner_named(&spec, BP_RUNNER).params.clone();
    bp_params.insert("bp_method".into(), Value::String("bogus_bp".into()));
    let err = preflight_err(&one_rbposd_point(&bp_params));
    assert!(
        err.contains("rbposd bp_method must be \"minimum_sum\" or \"product_sum\""),
        "unexpected BP validation error: {err}"
    );
}

fn surface_decoder_spec_paths() -> [PathBuf; 2] {
    [
        workspace_root().join("benchmarks/surface_decoder/spec.toml"),
        workspace_root().join("benchmarks/surface_decoder/full.toml"),
    ]
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rsinter manifest should live under the workspace root")
        .to_path_buf()
}

fn load_spec(path: &Path) -> BenchmarkSpec {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    toml::from_str(&text).unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn runner_named<'a>(spec: &'a BenchmarkSpec, name: &str) -> &'a RunnerSpec {
    spec.runners
        .iter()
        .find(|runner| runner.name == name)
        .unwrap_or_else(|| panic!("missing checked-in runner {name}"))
}

fn assert_rbposd_alias(runner: &RunnerSpec) {
    assert_eq!(runner.language, "rust");
    assert_eq!(runner.impl_key, "rbposd");
}

fn preflight_all_points(runner: &RunnerSpec) {
    let registry = build_default_rust_runner_registry();
    let runner_impl = registry
        .get(&runner.impl_key)
        .unwrap_or_else(|| panic!("missing rust runner impl {}", runner.impl_key));
    let points = expand_runner_points_for_runner(&runner.impl_key, &runner.params)
        .unwrap_or_else(|err| panic!("{} failed point expansion: {err}", runner.name));
    assert!(
        !points.is_empty(),
        "{} should expand to at least one benchmark point",
        runner.name
    );
    for point in &points {
        runner_impl
            .preflight_point(point)
            .unwrap_or_else(|err| panic!("{} failed preflight: {err}", runner.name));
    }
}

fn one_rbposd_point(params: &std::collections::BTreeMap<String, Value>) -> BenchCasePoint {
    expand_runner_points_for_runner("rbposd", params)
        .expect("mutated rbposd params should still expand before preflight")
        .into_iter()
        .next()
        .expect("mutated rbposd params should expand to one or more points")
}

fn preflight_err(point: &BenchCasePoint) -> String {
    let registry = build_default_rust_runner_registry();
    registry
        .get("rbposd")
        .expect("rbposd runner should be registered")
        .preflight_point(point)
        .expect_err("mutated rbposd params should fail preflight")
}
```

- [ ] **Step 2: Run the first verification command and observe the red test**

Run:

```bash
cargo test -p rsinter rbposd_benchmark_specs_cover_lsd_and_bp_option_runners
```

Expected: FAIL with `missing checked-in runner rbposd_lsd_order1`.

- [ ] **Step 3: Run the negative-control test and observe its dependency on checked-in names**

Run:

```bash
cargo test -p rsinter rbposd_benchmark_specs_reject_unknown_decoder_modes
```

Expected: FAIL with `missing checked-in runner rbposd_lsd_order1`.

### Task 2: Add rbposd Alias Runners To Checked-In Specs

**Files:**
- Modify: `benchmarks/surface_decoder/spec.toml`
- Modify: `benchmarks/surface_decoder/full.toml`
- Test: `rsinter/tests/bench_specs.rs`

**Interfaces:**
- Consumes: existing `rbposd` `impl_key` registry entry and runner param validation.
- Produces: stable checked-in benchmark names `rbposd_lsd_order1` and `rbposd_product_sum_serial`.

- [ ] **Step 1: Add smoke-tier runner aliases**

In `benchmarks/surface_decoder/spec.toml`, insert this block immediately after the existing `rbposd` runner block:

```toml
[[runner]]
name = "rbposd_lsd_order1"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002, 0.005, 0.010]
max_shots = 2000
max_errors = 20
batch_size = 256
lsd_method = "localized_statistics"
lsd_order = 1

[[runner]]
name = "rbposd_product_sum_serial"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002, 0.005, 0.010]
max_shots = 2000
max_errors = 20
batch_size = 256
bp_method = "product_sum"
schedule = "serial"
```

- [ ] **Step 2: Add full-tier runner aliases**

In `benchmarks/surface_decoder/full.toml`, insert this block immediately after the existing `rbposd` runner block:

```toml
[[runner]]
name = "rbposd_lsd_order1"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3, 5]
rounds = [3, 5]
p = [0.002, 0.005, 0.010]
max_shots = 10000
max_errors = 200
batch_size = 256
lsd_method = "localized_statistics"
lsd_order = 1

[[runner]]
name = "rbposd_product_sum_serial"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3, 5]
rounds = [3, 5]
p = [0.002, 0.005, 0.010]
max_shots = 10000
max_errors = 200
batch_size = 256
bp_method = "product_sum"
schedule = "serial"
```

- [ ] **Step 3: Run focused coverage tests**

Run:

```bash
cargo test -p rsinter rbposd_benchmark_specs_cover_lsd_and_bp_option_runners
cargo test -p rsinter rbposd_benchmark_specs_reject_unknown_decoder_modes
```

Expected: both commands PASS.

- [ ] **Step 4: Commit the passing implementation**

Run:

```bash
git add benchmarks/surface_decoder/spec.toml benchmarks/surface_decoder/full.toml rsinter/tests/bench_specs.rs
git commit -m "test: cover rbposd benchmark spec variants"
```

### Task 3: Final Verification

**Files:**
- No additional file edits.

**Interfaces:**
- Consumes: committed Task 1 and Task 2 changes.
- Produces: verification evidence for PR description.

- [ ] **Step 1: Run required issue verification**

Run:

```bash
cargo test -p rsinter rbposd_benchmark_specs_cover_lsd_and_bp_option_runners
cargo test -p rsinter rbposd_benchmark_specs_reject_unknown_decoder_modes
```

Expected: both commands PASS.

- [ ] **Step 2: Run required Agent Desk verification**

Run:

```bash
cargo test
```

Expected: PASS for the whole workspace.

- [ ] **Step 3: Inspect final diff**

Run:

```bash
git status --short
git log --oneline --decorate -3
```

Expected: clean worktree after commits, with the design commit and implementation commit on the worker branch.
