# Issue 92 Rbposd LSD DEM Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an LSD-backed `rbposd` DEM decoder adapter in `rsinter` and route typed LSD runner params through it.

**Architecture:** Refactor `rsinter/src/rbposd_adapter.rs` so the current DEM lowering and compiled observable mapping are shared by OSD and LSD backends. Keep `RbposdDemDecoder` as the OSD public adapter, add `RbposdLsdDemDecoder` as the LSD public adapter, and update the `rbposd` benchmark runner LSD branch to call that adapter instead of returning the #91 boundary error.

**Tech Stack:** Rust 2024 workspace; `rsinter` crate; `rbposd::{BpOsdDecoder, BpLsdDecoder, DecoderConfig, LsdConfig}`; `cargo test`; `cargo fmt`; GitHub PR workflow.

## Global Constraints

- Do not normalize LSD result rows beyond the params already available in #91.
- Do not update smoke or full benchmark specs.
- Do not expand BP method, BP schedule, or LSD method support.
- Do not alter `rbposd` core LSD algorithm behavior.
- Do not change the `CompiledDecoder` trait or bit-packing contract.
- Reuse the current DEM-to-matrix lowering logic in `rsinter/src/rbposd_adapter.rs`.
- Preserve forced syndrome bits, baseline observables, observable-only terms, exact-probability filtering, and bit-packed observable output behavior.
- Keep OSD and LSD selection explicit in the typed `RbposdRunnerParams` family.
- Return clear adapter compile errors for malformed DEM-lowered inputs.
- Use offline Cargo commands in this Agent Desk workspace because the crates.io index is unreachable.

---

## File Structure

- `rsinter/src/rbposd_adapter.rs`
  - Add `RbposdLsdDemDecoder`.
  - Replace the OSD-only compiled backend with a private OSD/LSD backend enum.
  - Keep DEM lowering, probability filtering, forced syndrome bits, baseline observables, and observable mapping in one shared path.
- `rsinter/src/decode.rs`
  - Export `RbposdLsdDemDecoder`.
- `rsinter/src/bench/runners/rbposd.rs`
  - Route `RbposdDecoderFamily::Lsd` through `RbposdLsdDemDecoder`.
  - Preserve the existing OSD branch.
- `rsinter/tests/decode_rbposd.rs`
  - Add issue-named positive and negative LSD adapter tests.
- `rsinter/tests/bench_run.rs`
  - Replace the #91 LSD execution-boundary test with a successful LSD benchmark wiring test.

---

### Task 1: Add The LSD DEM Adapter Backend

**Files:**
- Modify: `rsinter/src/rbposd_adapter.rs`
- Modify: `rsinter/src/decode.rs`
- Modify: `rsinter/tests/decode_rbposd.rs`

**Interfaces:**
- Consumes: `rbposd::BpLsdDecoder`, `rbposd::LsdConfig`, existing `dem_to_matrix_problem`, `CompiledDecoder`, and `Decoder`.
- Produces: `pub struct RbposdLsdDemDecoder` with `new(config: LsdConfig) -> Self`.
- Produces: `rsinter::decode::RbposdLsdDemDecoder`.
- Produces: shared compiled backend enum that returns `Correction` from either `BpOsdDecoder` or `BpLsdDecoder`.

- [ ] **Step 1: Write the failing LSD adapter tests**

In `rsinter/tests/decode_rbposd.rs`, replace the imports:

```rust
use rbposd::DecoderConfig;
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::{Decoder, RbposdDemDecoder};
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::DetectorErrorModel;
```

with:

```rust
use rbposd::{DecoderConfig, LsdConfig};
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::{Decoder, RbposdDemDecoder, RbposdLsdDemDecoder};
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::{DemTarget, DetectorErrorModel};
```

Add these tests immediately after `rbposd_dem_decoder_predicts_a_single_observable_flip`:

```rust
#[test]
fn lsd_dem_decoder_predicts_a_known_single_observable_flip() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RbposdLsdDemDecoder::new(LsdConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(
        f64::NAN,
        vec![DemTarget::Detector(0), DemTarget::Observable(0)],
    );
    let decoder = RbposdLsdDemDecoder::new(LsdConfig::default());

    let err = match decoder.compile_for_dem(&dem) {
        Ok(_) => panic!("expected invalid LSD DEM compile to fail"),
        Err(err) => err,
    };

    assert!(
        err.contains("failed to compile rbposd decoder"),
        "expected rbposd compile error, got {err:?}"
    );
}
```

- [ ] **Step 2: Run the issue-named tests and confirm RED**

Run:

```bash
cargo test -p rsinter lsd_dem_decoder_predicts_a_known_single_observable_flip --offline
cargo test -p rsinter lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem --offline
```

Expected: both commands fail to compile because `rsinter::decode::RbposdLsdDemDecoder` is not exported yet.

- [ ] **Step 3: Extend the adapter imports and public structs**

In `rsinter/src/rbposd_adapter.rs`, replace the `rbposd` import with:

```rust
use rbposd::{
    BpLsdDecoder, BpOsdDecoder, ChannelModel, Correction, DecodeError, DecoderConfig, LsdConfig,
    ParityCheckMatrix, Syndrome,
};
```

After `RbposdDemDecoder`, add:

```rust
pub struct RbposdLsdDemDecoder {
    config: LsdConfig,
}

impl RbposdLsdDemDecoder {
    pub fn new(config: LsdConfig) -> Self {
        Self { config }
    }
}
```

- [ ] **Step 4: Add private backend enums**

In `rsinter/src/rbposd_adapter.rs`, replace:

```rust
struct CompiledRbposdDemDecoder {
    decoder: Option<BpOsdDecoder>,
    num_dets: usize,
    num_obs: usize,
    observable_columns: Vec<Vec<usize>>,
    forced_syndrome: Vec<bool>,
    baseline_observables: Vec<bool>,
}
```

with:

```rust
enum RbposdDemBackendConfig {
    Osd(DecoderConfig),
    Lsd(LsdConfig),
}

enum RbposdDemBackend {
    Osd(BpOsdDecoder),
    Lsd(BpLsdDecoder),
}

impl RbposdDemBackendConfig {
    fn compile(
        &self,
        pcm: ParityCheckMatrix,
        probabilities: Vec<f64>,
    ) -> Result<RbposdDemBackend, String> {
        let channel = ChannelModel::BitFlipProbabilities(probabilities);
        match self {
            Self::Osd(config) => BpOsdDecoder::new(pcm, channel, config.clone())
                .map(RbposdDemBackend::Osd),
            Self::Lsd(config) => {
                BpLsdDecoder::new(pcm, channel, *config).map(RbposdDemBackend::Lsd)
            }
        }
        .map_err(|error| format!("failed to compile rbposd decoder: {error}"))
    }
}

impl RbposdDemBackend {
    fn decode(&self, syndrome: &Syndrome) -> Result<Correction, DecodeError> {
        match self {
            Self::Osd(decoder) => decoder.decode(syndrome),
            Self::Lsd(decoder) => decoder.decode(syndrome),
        }
        .map(|result| result.correction)
    }
}

struct CompiledRbposdDemDecoder {
    decoder: Option<RbposdDemBackend>,
    num_dets: usize,
    num_obs: usize,
    observable_columns: Vec<Vec<usize>>,
    forced_syndrome: Vec<bool>,
    baseline_observables: Vec<bool>,
}
```

- [ ] **Step 5: Replace OSD-only compile logic with the shared compile function**

In `rsinter/src/rbposd_adapter.rs`, replace the body of `impl Decoder for RbposdDemDecoder` with:

```rust
impl Decoder for RbposdDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        compile_rbposd_dem_with_backend(
            dem,
            RbposdDemBackendConfig::Osd(self.config.clone()),
        )
    }
}

impl Decoder for RbposdLsdDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        compile_rbposd_dem_with_backend(dem, RbposdDemBackendConfig::Lsd(self.config))
    }
}
```

Then add this helper before `impl CompiledDecoder for CompiledRbposdDemDecoder`:

```rust
fn compile_rbposd_dem_with_backend(
    dem: &DetectorErrorModel,
    backend_config: RbposdDemBackendConfig,
) -> Result<Box<dyn CompiledDecoder>, String> {
    let (detector_columns, probabilities, observable_columns, num_dets, num_obs) =
        dem_to_matrix_problem(dem);

    let mut filtered_detector_columns = Vec::new();
    let mut filtered_observable_columns = Vec::new();
    let mut filtered_probabilities = Vec::new();
    let mut forced_syndrome = vec![false; num_dets];
    let mut baseline_observables = vec![false; num_obs];

    for ((detectors, observables), probability) in detector_columns
        .into_iter()
        .zip(observable_columns.into_iter())
        .zip(probabilities.into_iter())
    {
        if probability <= 0.0 {
            continue;
        }

        if probability >= 1.0 {
            xor_indices(&mut forced_syndrome, &detectors);
            xor_indices(&mut baseline_observables, &observables);
            continue;
        }

        if detectors.is_empty() {
            if probability > 0.5 {
                xor_indices(&mut baseline_observables, &observables);
            }
            continue;
        }

        filtered_detector_columns.push(detectors);
        filtered_observable_columns.push(observables);
        filtered_probabilities.push(probability);
    }

    let decoder = if filtered_detector_columns.is_empty() {
        None
    } else {
        let pcm = ParityCheckMatrix::from_sparse_columns(
            num_dets,
            filtered_detector_columns.len(),
            filtered_detector_columns,
        )
        .map_err(|error| format!("invalid rbposd parity matrix: {error}"))?;

        Some(backend_config.compile(pcm, filtered_probabilities)?)
    };

    Ok(Box::new(CompiledRbposdDemDecoder {
        decoder,
        num_dets,
        num_obs,
        observable_columns: filtered_observable_columns,
        forced_syndrome,
        baseline_observables,
    }))
}
```

- [ ] **Step 6: Update compiled decode to call the backend enum**

In `decode_shots_bit_packed`, replace:

```rust
                let result = decoder
                    .decode(&Syndrome::from(syndrome_bits))
                    .map_err(|error| format!("rbposd decode failed: {error}"))?;
                let decoded_observables = correction_to_observables(
                    &result.correction,
                    &self.observable_columns,
                    self.num_obs,
                );
```

with:

```rust
                let correction = decoder
                    .decode(&Syndrome::from(syndrome_bits))
                    .map_err(|error| format!("rbposd decode failed: {error}"))?;
                let decoded_observables =
                    correction_to_observables(&correction, &self.observable_columns, self.num_obs);
```

- [ ] **Step 7: Export the LSD adapter**

In `rsinter/src/decode.rs`, replace:

```rust
pub use crate::rbposd_adapter::RbposdDemDecoder;
```

with:

```rust
pub use crate::rbposd_adapter::{RbposdDemDecoder, RbposdLsdDemDecoder};
```

- [ ] **Step 8: Run focused tests and confirm GREEN**

Run:

```bash
cargo test -p rsinter lsd_dem_decoder_predicts_a_known_single_observable_flip --offline
cargo test -p rsinter lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem --offline
cargo test -p rsinter rbposd_dem_decoder_predicts_a_single_observable_flip --offline
```

Expected: all three commands pass.

- [ ] **Step 9: Commit Task 1**

Run:

```bash
git add rsinter/src/rbposd_adapter.rs rsinter/src/decode.rs rsinter/tests/decode_rbposd.rs
git commit -m "feat: add rbposd lsd dem adapter"
```

---

### Task 2: Route Typed LSD Runner Params Through The Adapter

**Files:**
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `RbposdDecoderFamily::Lsd { lsd_config, .. }` from #91.
- Consumes: `RbposdLsdDemDecoder::new(LsdConfig)`.
- Produces: successful `run_rust_benchmark` execution for valid `rbposd` LSD params.

- [ ] **Step 1: Replace the #91 execution-boundary test with a failing success test**

In `rsinter/tests/bench_run.rs`, replace the full
`rbposd_lsd_run_fails_without_silent_osd_fallback_or_artifacts` test with:

```rust
#[test]
fn rbposd_lsd_run_uses_lsd_dem_adapter_and_writes_artifacts() {
    let spec_text = issue91_surface_spec(
        r#"
lsd_method = "localized_statistics"
lsd_order = 1
"#,
    );
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
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

    let artifact_dir = artifact_root.join("rbposd_lsd").join("test-run");
    assert!(artifact_dir.join("run_manifest.json").exists());
    assert!(artifact_dir.join("results.jsonl").exists());

    let data = fs::read(artifact_dir.join("results.jsonl")).unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].runner, "rbposd_lsd");
    assert_eq!(rows[0].language, "rust");
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);
    assert_eq!(
        rows[0].params["lsd_method"],
        serde_json::json!("localized_statistics")
    );
    assert_eq!(rows[0].params["lsd_order"], serde_json::json!(1));
    assert_eq!(rows[0].params["decoder_impl"], serde_json::json!("rbposd"));
}
```

- [ ] **Step 2: Run the benchmark wiring test and confirm RED**

Run:

```bash
cargo test -p rsinter rbposd_lsd_run_uses_lsd_dem_adapter_and_writes_artifacts --offline
```

Expected: the test fails because `RbposdRunner::run_point` still returns `rbposd LSD DEM decoding is not implemented yet; see issue #92`.

- [ ] **Step 3: Import the LSD adapter in the runner**

In `rsinter/src/bench/runners/rbposd.rs`, replace:

```rust
use crate::decode::RbposdDemDecoder;
```

with:

```rust
use crate::decode::{RbposdDemDecoder, RbposdLsdDemDecoder};
```

- [ ] **Step 4: Replace the LSD execution-boundary branch**

In `RbposdRunner::run_point`, replace:

```rust
            RbposdDecoderFamily::Lsd { .. } => {
                Err("rbposd LSD DEM decoding is not implemented yet; see issue #92".into())
            }
```

with:

```rust
            RbposdDecoderFamily::Lsd { lsd_config, .. } => {
                let decoder = RbposdLsdDemDecoder::new(*lsd_config);
                run_decoder_point_with_dem_mode(
                    self.name(),
                    &decoder,
                    point,
                    ctx,
                    &params.normalized,
                    DemBuildMode::Raw,
                )
            }
```

- [ ] **Step 5: Run focused runner and adapter tests**

Run:

```bash
cargo test -p rsinter rbposd_lsd_run_uses_lsd_dem_adapter_and_writes_artifacts --offline
cargo test -p rsinter rbposd_runner_preflight_accepts_lsd_params --offline
cargo test -p rsinter lsd_dem_decoder_predicts_a_known_single_observable_flip --offline
```

Expected: all three commands pass.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add rsinter/src/bench/runners/rbposd.rs rsinter/tests/bench_run.rs
git commit -m "feat: route rbposd lsd runner through dem adapter"
```

---

### Task 3: Final Verification And Cleanup

**Files:**
- No planned source edits. Only commit fixes if verification exposes an issue.

**Interfaces:**
- Produces verified branch ready for PR creation.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check --package rsinter
```

Expected: PASS. If this command reports formatting drift only in files touched by this plan, run `cargo fmt --package rsinter`, rerun the check, and commit the formatting change with `git commit -m "style: format issue 92 rsinter changes"`.

- [ ] **Step 2: Run issue-named verification**

Run:

```bash
cargo test -p rsinter lsd_dem_decoder_predicts_a_known_single_observable_flip --offline
cargo test -p rsinter lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem --offline
```

Expected: both commands pass.

- [ ] **Step 3: Run package and workspace verification**

Run:

```bash
cargo test -p rsinter --offline
cargo test --offline
git diff --check
```

Expected: all commands pass.

- [ ] **Step 4: Final code review**

Create a review package from the branch merge-base to `HEAD` and dispatch a final reviewer using `superpowers:requesting-code-review`. Fix any Critical or Important findings, rerun the covering tests, and commit fixes before finishing.

- [ ] **Step 5: Finish through PR workflow**

Use `superpowers:finishing-a-development-branch`. Choose "Push and create a Pull Request" under the Standing Answer Policy, push the worker branch, and create a draft PR targeting `master`.

---

## Self-Review Notes

- Spec coverage: Task 1 implements the exported LSD adapter and shared DEM lowering; Task 2 wires typed LSD runner params; Task 3 runs issue-named, package, workspace, formatting, diff, and review checks.
- Placeholder scan: no unresolved markers or incomplete implementation steps are intentionally present.
- Type consistency: `RbposdLsdDemDecoder`, `LsdConfig`, and `RbposdDecoderFamily::Lsd { lsd_config, .. }` are named consistently across tasks.
