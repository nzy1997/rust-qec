# Issue 518 Rsinter Feature Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `rsinter` build and run the CSS `rbposd` benchmark path without pulling disabled `rmatching`, ILP/HiGHS, or plotting dependencies, while preserving full default CLI behavior.

**Architecture:** Cargo features own dependency selection. Feature-gated adapter and runner modules provide real implementations when enabled and small disabled shims when unavailable. CLI subcommands remain visible in every build and fail before input/output side effects when their required feature is disabled.

**Tech Stack:** Rust 2024, Cargo feature resolver 3, `clap`, `serde`, `toml`, `plotters`, workspace crates `rbposd`, `rmatching`, `rilpqec`, `qec-ilp-core`, GitHub Actions.

## Global Constraints

- `rbposd-runner`: enables the `rbposd` adapter and benchmark runner required by the minimal CSS BP+OSD path.
- `rmatching-runner`: enables the `rmatching` adapter and benchmark runner.
- `ilp-runner`: enables `rilpqec`, `qec-ilp-core`/HiGHS, and the ILP adapter and runner.
- `plotting`: enables `plotters` and plotting implementations.
- `full`: enables all four capabilities above.
- `default = ["full"]`: preserves ordinary `cargo build -p rsinter` and existing benchmark commands.
- Keep CLI subcommand names and known runner identifiers compiled in all feature combinations.
- Disabled commands or runners must return `requires Cargo feature '<feature>'` rather than disappearing or becoming unknown.
- Minimal `rbposd-runner` normal/build dependency graph must not contain `rmatching`, `rilpqec`, `qec-ilp-core`, `highs`, `highs-sys`, or `plotters`.
- Add `rsinter/tests/fixtures/bench/minimal_steane_css_rbposd.toml`.
- Add `rsinter/tests/fixtures/bench/minimal_steane_css_rilpqec.toml`.
- Document minimal and full build commands in benchmark or `rsinter` docs.

---

## File Structure

- Modify `rsinter/Cargo.toml`: feature contract and optional dependencies.
- Modify `rsinter/src/lib.rs`: cfg-gate feature-specific adapter modules.
- Modify `rsinter/src/decode.rs`: cfg-gate public adapter exports.
- Modify `rsinter/src/bench/runners/mod.rs`: keep module names stable while allowing cfg-backed real/stub modules.
- Modify `rsinter/src/bench/runners/rmatching.rs`, `rsinter/src/bench/runners/rbposd.rs`, and `rsinter/src/bench/runners/rilpqec.rs`: real implementations behind feature gates, disabled shims otherwise.
- Modify `rsinter/src/bench/registry.rs`: register stable runner names regardless of enabled features.
- Modify `rsinter/src/bin/rsinter.rs`: cfg-gate plotting and BB circuit BP+OSD command implementations while keeping CLI variants.
- Modify `rsinter/src/plot.rs` and `rsinter/src/bench/plot.rs`: either compile only with plotting or expose missing-feature stubs.
- Create `rsinter/tests/fixtures/bench/minimal_steane_css_rbposd.toml` and `rsinter/tests/fixtures/bench/minimal_steane_css_rilpqec.toml`.
- Modify feature-specific tests in `rsinter/tests/*.rs` and add minimal negative/positive CLI tests.
- Modify `.github/workflows/ci.yml`: add locked minimal `rsinter` build/smoke dependency-graph coverage.
- Modify `benchmarks/surface_decoder_compare/README.md`: feature matrix and commands.

## Task 1: Cargo Features And Adapter Gates

**Files:**
- Modify: `rsinter/Cargo.toml`
- Modify: `rsinter/src/lib.rs`
- Modify: `rsinter/src/decode.rs`
- Modify tests importing adapter types: `rsinter/tests/decode_rbposd.rs`, `rsinter/tests/decode_rmatching.rs`, `rsinter/tests/decode_ilp.rs`, `rsinter/tests/css_surface_special.rs`

**Interfaces:**
- Consumes: existing `Decoder` and `CompiledDecoder` traits.
- Produces: Cargo features named `rbposd-runner`, `rmatching-runner`, `ilp-runner`, `plotting`, `full`; public adapter exports only under matching features.

- [ ] **Step 1: Write failing compile checks**

Run:

```bash
cargo check --locked -p rsinter --no-default-features --features rbposd-runner
cargo check --locked -p rsinter --no-default-features
```

Expected before implementation: both checks fail because adapter modules import optional crates that are still unconditional or because tests/public exports are not gated.

- [ ] **Step 2: Update Cargo feature contract**

Change `rsinter/Cargo.toml` so the feature and dependency sections are:

```toml
[features]
default = ["full"]
rbposd-runner = ["dep:rbposd"]
rmatching-runner = ["dep:rmatching"]
ilp-runner = ["dep:rilpqec", "dep:qec-ilp-core"]
plotting = ["dep:plotters"]
full = ["rbposd-runner", "rmatching-runner", "ilp-runner", "plotting"]
gurobi = ["ilp-runner", "rilpqec/gurobi"]

[dependencies]
rstim = { path = "../rstim" }
rmatching = { path = "../rmatching", features = ["bench"], optional = true }
rbposd = { path = "../rbposd", optional = true }
rilpqec = { path = "../rilpqec", optional = true }
qec-ilp-core = { path = "../qec-ilp-core", optional = true }
rand = "0.8"
rayon = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
csv = "1"
sha2 = "0.10"
toml = "0.8"
plotters = { version = "0.3", default-features = false, features = ["svg_backend", "bitmap_backend", "bitmap_encoder", "line_series", "full_palette", "ttf"], optional = true }
```

- [ ] **Step 3: Gate adapter modules and exports**

In `rsinter/src/lib.rs`, use:

```rust
#[cfg(feature = "ilp-runner")]
mod ilpqec_adapter;
#[cfg(feature = "rbposd-runner")]
mod rbposd_adapter;
#[cfg(feature = "rmatching-runner")]
mod rmatching_adapter;
```

In `rsinter/src/decode.rs`, wrap public exports with the same features:

```rust
#[cfg(feature = "ilp-runner")]
pub use crate::ilpqec_adapter::IlpDemDecoder;
#[cfg(feature = "rbposd-runner")]
pub use crate::rbposd_adapter::{RbposdDemDecoder, RbposdLsdDemDecoder};
#[cfg(feature = "rmatching-runner")]
pub use crate::rmatching_adapter::RmatchingDemDecoder;
```

- [ ] **Step 4: Gate adapter-specific tests**

Add crate-level cfgs:

```rust
#![cfg(feature = "rbposd-runner")]
```

to `rsinter/tests/decode_rbposd.rs`; add:

```rust
#![cfg(feature = "rmatching-runner")]
```

to `rsinter/tests/decode_rmatching.rs` and `rsinter/tests/css_surface_special.rs`; add:

```rust
#![cfg(feature = "ilp-runner")]
```

to `rsinter/tests/decode_ilp.rs`.

- [ ] **Step 5: Verify green**

Run:

```bash
cargo check --locked -p rsinter --no-default-features --features rbposd-runner
cargo check --locked -p rsinter --no-default-features
cargo test --locked -p rsinter --features full --test decode_ilp ilp_dem_decoder_predicts_a_single_observable_flip
```

Expected: all commands exit 0 after later tasks finish any plotting or runner compile fallout.

## Task 2: Stable Runner Registry With Disabled Shims

**Files:**
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/src/bench/runners/rmatching.rs`
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/src/bench/runners/rilpqec.rs`
- Modify: `rsinter/src/bench/registry.rs`
- Modify: `rsinter/tests/bench_registry.rs`
- Modify: `rsinter/tests/bench_run.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

**Interfaces:**
- Consumes: `RustBenchRunner` trait.
- Produces: real or disabled runner structs named `RmatchingRunner`, `RbposdRunner`, and `RilpqecRunner`, each with stable `name()`.

- [ ] **Step 1: Write failing disabled-runner tests**

Add tests that run under `#[cfg(all(feature = "rbposd-runner", not(feature = "ilp-runner")))]`:

```rust
#[test]
fn disabled_rilpqec_runner_reports_required_feature_before_artifacts() {
    let spec: BenchmarkSpec = toml::from_str(include_str!(
        "fixtures/bench/minimal_steane_css_rilpqec.toml"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench").as_path(),
    )
    .expect_err("disabled ILP runner must fail");

    assert!(err.contains("requires Cargo feature 'ilp-runner'"), "{err}");
    assert!(!dir.path().join("rilpqec-steane").join("test-run").join("results.jsonl").exists());
}
```

Also add a registry test asserting that the minimal registry still contains
`rmatching`, `rbposd`, `rilpqec`, and `predict-zero`.

Expected before implementation: minimal feature builds fail to compile or report unknown runner.

- [ ] **Step 2: Gate real runner imports and code**

Wrap each real runner file body:

```rust
#[cfg(feature = "rbposd-runner")]
mod enabled {
    // existing rbposd runner implementation
}

#[cfg(feature = "rbposd-runner")]
pub use enabled::RbposdRunner;
```

Use equivalent `rmatching-runner` and `ilp-runner` gates for the other files.

- [ ] **Step 3: Add disabled runner implementations**

In each runner file, add a disabled module for the inverse cfg:

```rust
#[cfg(not(feature = "ilp-runner"))]
pub struct RilpqecRunner;

#[cfg(not(feature = "ilp-runner"))]
impl RustBenchRunner for RilpqecRunner {
    fn name(&self) -> &'static str {
        "rilpqec"
    }

    fn preflight_point(&self, _point: &BenchCasePoint) -> Result<(), String> {
        Err("runner 'rilpqec' requires Cargo feature 'ilp-runner'".into())
    }

    fn plan_point_identity(
        &self,
        _point: &BenchCasePoint,
        _ctx: &BenchRunContext,
    ) -> Result<String, String> {
        Err("runner 'rilpqec' requires Cargo feature 'ilp-runner'".into())
    }

    fn run_point(
        &self,
        _point: &BenchCasePoint,
        _ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        Err("runner 'rilpqec' requires Cargo feature 'ilp-runner'".into())
    }
}
```

Use `rbposd-runner` and `rmatching-runner` messages for the other disabled
runners.

- [ ] **Step 4: Keep registry names stable**

Do not remove registry insertions in `build_default_rust_runner_registry()`.
The same type names should resolve to real or disabled implementations based on
features.

- [ ] **Step 5: Gate wrapper tests that call real runners**

Put feature cfgs around test functions in `rsinter/tests/bench_run.rs` and
`rsinter/tests/bench_runner_wrappers.rs` that execute `rbposd`, `rmatching`, or
`rilpqec`. Keep tests using `predict-zero` and disabled-runner checks available
in minimal builds.

- [ ] **Step 6: Verify green**

Run:

```bash
cargo test --locked -p rsinter --no-default-features --features rbposd-runner --test bench_registry
cargo test --locked -p rsinter --no-default-features --features rbposd-runner --test bench_run disabled_rilpqec_runner_reports_required_feature_before_artifacts
```

Expected: both commands exit 0.

## Task 3: CLI Plotting And BB Circuit Gates

**Files:**
- Modify: `rsinter/src/bin/rsinter.rs`
- Modify: `rsinter/src/plot.rs`
- Modify: `rsinter/src/bench/plot.rs`
- Modify: `rsinter/src/bench/mod.rs`
- Modify: `rsinter/tests/bench_cli.rs`
- Modify: `rsinter/tests/bench_plot.rs`
- Modify: `rsinter/tests/plot.rs`
- Modify: `rsinter/tests/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Consumes: existing CLI enums.
- Produces: feature-independent CLI help; plotting and BB circuit commands with explicit missing-feature errors when disabled.

- [ ] **Step 1: Write failing CLI negative tests**

In `rsinter/tests/bench_cli.rs`, add tests gated with
`#[cfg(all(feature = "rbposd-runner", not(feature = "plotting")))]`:

```rust
#[test]
fn rsinter_bench_plot_requires_plotting_feature_before_reading_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bench",
            "plot",
            "--spec",
            "tests/fixtures/bench/minimal_steane_css_rbposd.toml",
            "--input",
            dir.path().join("missing-results.jsonl").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("requires Cargo feature 'plotting'"), "{stderr}");
    assert!(!out.exists());
}
```

Expected before implementation: the test fails by trying to read inputs or by
compilation with missing `plotters`.

- [ ] **Step 2: Gate plotting modules**

In `rsinter/src/bench/mod.rs`, keep `pub mod plot;` so the path exists. In
`rsinter/src/bench/plot.rs` and `rsinter/src/plot.rs`, put the current
plotters-backed implementation under `#[cfg(feature = "plotting")]` and expose
same-signature stubs under `#[cfg(not(feature = "plotting"))]` returning:

```rust
Err("requires Cargo feature 'plotting'".into())
```

For public `rsinter::plot` functions that return `Box<dyn std::error::Error>`,
return:

```rust
Err("requires Cargo feature 'plotting'".into())
```

- [ ] **Step 3: Preflight plotting subcommands in the CLI**

Add:

```rust
#[cfg(not(feature = "plotting"))]
fn require_plotting_feature() -> Result<(), String> {
    Err("requires Cargo feature 'plotting'".into())
}

#[cfg(feature = "plotting")]
fn require_plotting_feature() -> Result<(), String> {
    Ok(())
}
```

Call it as the first statement in `BenchCommands::Plot`,
`BenchCommands::PlotSurfaceCompareCsv`, and `BenchCommands::PlotBbCompareCsv`.

- [ ] **Step 4: Gate BB circuit BP+OSD command internals**

Move the body of `Commands::BbCircuitBposdMemory` into helper functions:

```rust
#[cfg(feature = "rbposd-runner")]
fn run_bb_circuit_bposd_memory(args: BbCircuitBposdMemoryArgs) -> Result<(), String> {
    // existing implementation
}

#[cfg(not(feature = "rbposd-runner"))]
fn run_bb_circuit_bposd_memory(_args: BbCircuitBposdMemoryArgs) -> Result<(), String> {
    Err("requires Cargo feature 'rbposd-runner'".into())
}
```

Only import `rbposd::OsdVariant` and `rsinter::bb_circuit_memory::*` under
`#[cfg(feature = "rbposd-runner")]`.

- [ ] **Step 5: Gate plotting and BB tests**

Add `#![cfg(feature = "plotting")]` to `rsinter/tests/bench_plot.rs` and
`rsinter/tests/plot.rs`. Add `#![cfg(feature = "rbposd-runner")]` to BB
circuit memory fixture tests that require the real BP+OSD stack.

- [ ] **Step 6: Verify green**

Run:

```bash
cargo test --locked -p rsinter --no-default-features --features rbposd-runner --test bench_cli rsinter_bench_plot_requires_plotting_feature_before_reading_inputs
cargo test --locked -p rsinter --features full --test bench_cli rsinter_bench_plot_writes_svg_from_jsonl_input
```

Expected: both commands exit 0.

## Task 4: Steane Fixtures And Minimal Smoke Path

**Files:**
- Create: `rsinter/tests/fixtures/bench/minimal_steane_css_rbposd.toml`
- Create: `rsinter/tests/fixtures/bench/minimal_steane_css_rilpqec.toml`
- Modify: `rsinter/tests/bench_cli.rs`
- Modify: `rsinter/tests/bench_spec.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: Steane matrices in `rsinter/tests/fixtures/css/`.
- Produces: committed known-input smoke specs required by issue #518.

- [ ] **Step 1: Add fixture load tests first**

Add a `bench_spec` test that parses both new fixtures with `include_str!` and
calls `validate()`. Expected before fixture creation: compile or parse failure.

- [ ] **Step 2: Create the `rbposd` fixture**

Create `rsinter/tests/fixtures/bench/minimal_steane_css_rbposd.toml`:

```toml
name = "minimal_steane_css_rbposd"
version = 1
mode = "independent"

[[runner]]
name = "rbposd-steane"
language = "rust"
impl_key = "rbposd"

[runner.params]
input_type = "css"
code_id = "steane"
hx = "../css/steane_hx.json"
hz = "../css/steane_hz.json"
observables = "../css/steane_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [1]
p = [0.0]
seed = 12345
max_shots = 8
max_errors = 4
batch_size = 4
bp_algorithm = "min_sum"
bp_iters = 50
early_stop = true
osd_method = "combination_sweep"
osd_order = 2

[plot]
title = "Minimal Steane CSS RBPOSD"

[plot.x]
field = "params.p"
scale = "linear"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
```

- [ ] **Step 3: Create the `rilpqec` fixture**

Create `rsinter/tests/fixtures/bench/minimal_steane_css_rilpqec.toml` with the
same generic CSS fields but:

```toml
name = "minimal_steane_css_rilpqec"

[[runner]]
name = "rilpqec-steane"
language = "rust"
impl_key = "rilpqec"

[runner.params]
backend = "auto"
```

and otherwise match the same CSS input, seed, budgets, and plot section.

- [ ] **Step 4: Add minimal positive CLI smoke**

Add a `bench_cli` test gated with `#[cfg(feature = "rbposd-runner")]` that runs
`bench run --spec tests/fixtures/bench/minimal_steane_css_rbposd.toml
--language rust --out <tempdir>`, reads the single `results.jsonl` row, and
asserts `input_type == "css"`, `code_id == "steane"`,
`decoder_impl == "rbposd"`, and `status == "ok"`.

- [ ] **Step 5: Verify green**

Run:

```bash
cargo test --locked -p rsinter --no-default-features --features rbposd-runner --test bench_spec steane_minimal_feature_gate_fixtures_are_valid
cargo test --locked -p rsinter --no-default-features --features rbposd-runner --test bench_cli rsinter_bench_run_minimal_steane_css_rbposd_fixture_writes_one_ok_row
```

Expected: both commands exit 0.

## Task 5: CI, Docs, And End-To-End Verification

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `benchmarks/surface_decoder_compare/README.md`
- Modify: all files touched by earlier tasks as needed for final polish

**Interfaces:**
- Consumes: fixture paths and feature contract.
- Produces: CI coverage and user documentation.

- [ ] **Step 1: Add CI minimal job**

Add a CI step after toolchain/cache setup that runs:

```bash
cargo build --locked -p rsinter --no-default-features --features rbposd-runner
minimal_out="$(mktemp -d)"
target/debug/rsinter bench run \
  --spec rsinter/tests/fixtures/bench/minimal_steane_css_rbposd.toml \
  --language rust \
  --out "$minimal_out"
cargo tree --locked -p rsinter \
  --no-default-features --features rbposd-runner \
  --edges normal,build > /tmp/rsinter-minimal-tree.txt
if rg -n '(rmatching|rilpqec|qec-ilp-core|highs|highs-sys|plotters)( |$)' /tmp/rsinter-minimal-tree.txt; then
  echo "ERROR: minimal dependency graph contains a disabled capability" >&2
  exit 1
fi
rm -r "$minimal_out"
rm /tmp/rsinter-minimal-tree.txt
```

- [ ] **Step 2: Document feature matrix**

Add a `rsinter Cargo Features` section to
`benchmarks/surface_decoder_compare/README.md` with the six feature entries
from Global Constraints and both commands:

```bash
cargo build --locked -p rsinter
cargo build --locked -p rsinter --no-default-features --features rbposd-runner
```

- [ ] **Step 3: Run formatting**

Run:

```bash
cargo fmt
```

Expected: exits 0.

- [ ] **Step 4: Run issue verification**

Run the exact minimal positive path, disabled negative controls, and
full/default compatibility commands from issue #518.

- [ ] **Step 5: Run Agent Desk required gate**

Run:

```bash
cargo test
```

Expected: exits 0.

- [ ] **Step 6: Commit implementation**

Run:

```bash
git status -sb
git add rsinter/Cargo.toml rsinter/src rsinter/tests .github/workflows/ci.yml benchmarks/surface_decoder_compare/README.md docs/superpowers/plans/2026-07-22-issue-518-rsinter-feature-gates.md
git commit -m "feat: gate rsinter optional runner dependencies"
```

Expected: implementation commit is created after verification passes.

---

## Plan Self-Review

- Spec coverage: tasks cover Cargo features, stable CLI/runner names, missing-feature errors, Steane fixtures, CI, docs, and minimal/full verification.
- Placeholder scan: no TBD/TODO/placeholder steps remain.
- Type consistency: runner type names and feature names match the design and issue contract.

## Execution Choice

Because this is a non-interactive Agent Desk run, choose option 1,
Subagent-Driven (recommended), and continue without waiting for user input.
