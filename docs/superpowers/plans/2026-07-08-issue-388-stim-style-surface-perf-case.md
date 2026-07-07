# Issue 388 Stim-style Surface Perf Case Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a report-only perf registry case that samples the checked Stim-generated d=11, rounds=100, shots=1024 surface-code fixture from issue #385.

**Architecture:** Extend `PerfCircuitSource` with a checked fixture variant that carries the fixture case ID, root-relative fixture path, and explicit Stim-style noise metadata. Update the perf runner to read fixture text from the workspace root and add a contract test proving the registry case uses the checked fixture instead of a regenerated `rstim` source.

**Tech Stack:** Rust 2024, `rstim` perf registry, existing `rstim/tests/perf_harness.rs` integration tests, checked `.stim` fixture under `benchmarks/rstim_vs_stim_simulator/fixtures/`.

## Global Constraints

- The perf case label is exactly `stim-style-surface-sample-d11-r100-b1024`.
- The fixture case ID is exactly `stim_surface_d11_r100`.
- The canonical fixture path is exactly `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`.
- The case workload is `sample`.
- The case shots value is `1024`.
- The case tier is `report_only`.
- The case source must be the checked Stim-generated fixture from issue #385, not a regenerated `rstim` surface-code source.
- The explicit Stim-style noise metadata is `after_clifford_depolarization = 0.001`, `after_reset_flip_probability = 0.001`, `before_measure_flip_probability = 0.001`, and `before_round_data_depolarization = 0.0`.
- `NoiseParams::uniform(0.001)` must not satisfy the contract because it enables `before_round_data_depolarization`.
- Expected variants are `stim-cli`, `rstim-interpreted`, and `rstim-compiled`.
- The case is report-only; do not add a hard CI regression threshold based on Stim-vs-`rstim` absolute speed.
- Do not optimize sampler performance or implement surface-code generator parity.

---

## File Structure

- Modify `rstim/src/perf/cases.rs`: add `PerfNoiseMetadata`, add the checked fixture source variant, and register the new report-only case.
- Modify `rstim/src/perf/runner.rs`: load fixture source text from the repository root.
- Modify `rstim/src/perf.rs`: re-export `PerfNoiseMetadata` for integration tests and external perf contract checks.
- Modify `rstim/tests/perf_harness.rs`: add the exact contract test requested by issue #388 and update existing case/variant expectations for the new case.

### Task 1: Add the Stim-style Surface Fixture Perf Case

**Files:**
- Modify: `rstim/src/perf/cases.rs`
- Modify: `rstim/src/perf/runner.rs`
- Modify: `rstim/src/perf.rs`
- Modify: `rstim/tests/perf_harness.rs`

**Interfaces:**
- Consumes: checked fixture path `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`.
- Produces: `PerfNoiseMetadata { before_round_data_depolarization: f64, after_clifford_depolarization: f64, before_measure_flip_probability: f64, after_reset_flip_probability: f64 }`.
- Produces: `PerfCircuitSource::Fixture { case_id: &'static str, canonical_input_path: &'static str, noise: PerfNoiseMetadata }`.
- Produces: a `benchmark_cases()` entry with label `stim-style-surface-sample-d11-r100-b1024`.

- [ ] **Step 1: Write the failing contract test**

Add `NoiseParams` to the imports in `rstim/tests/perf_harness.rs`:

```rust
use rstim::codegen::{NoiseParams, repetition_code_memory, rotated_memory_x};
```

Add this test to `rstim/tests/perf_harness.rs`:

```rust
#[test]
fn benchmark_cases_include_stim_style_surface_sample_contract() {
    let case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("stim-style surface sample perf case");

    assert_eq!(case.workload, PerfWorkload::Sample);
    assert_eq!(case.shots, Some(1024));
    assert_eq!(case.tier, PerfCaseTier::ReportOnly);
    assert!(case.requires_compiled);
    assert!(!case.requires_fallback);
    assert_eq!(
        case.comparisons,
        [PerfComparisonKind::SamplerCompiledVsInterpreted].as_slice()
    );

    let (case_id, canonical_input_path, noise) = match case.source {
        PerfCircuitSource::Fixture {
            case_id,
            canonical_input_path,
            noise,
        } => (case_id, canonical_input_path, noise),
        PerfCircuitSource::Generator { .. } => {
            panic!("Stim-style surface sample must use checked Stim fixture, not regenerated rstim source")
        }
        PerfCircuitSource::Inline { .. } => {
            panic!("Stim-style surface sample must point at checked Stim fixture")
        }
    };

    assert_eq!(case_id, "stim_surface_d11_r100");
    assert_eq!(
        canonical_input_path,
        "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
    );
    assert_eq!(noise.after_clifford_depolarization, 0.001);
    assert_eq!(noise.after_reset_flip_probability, 0.001);
    assert_eq!(noise.before_measure_flip_probability, 0.001);
    assert_eq!(noise.before_round_data_depolarization, 0.0);

    let uniform = NoiseParams::uniform(0.001);
    assert_ne!(
        noise.before_round_data_depolarization,
        uniform.before_round_data_depolarization,
        "uniform noise would enable before_round_data_depolarization"
    );

    let fixture_text =
        std::fs::read_to_string(std::path::Path::new("..").join(canonical_input_path))
            .expect("checked Stim fixture");
    let instrs = parse_lines(&fixture_text).expect("fixture parses");
    assert_eq!(benchmark_case_variants(case, &instrs).unwrap(), vec![
        PerfVariant::StimCli,
        PerfVariant::RstimInterpreted,
        PerfVariant::RstimCompiled,
    ]);
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
cargo test -p rstim --test perf_harness -- --exact benchmark_cases_include_stim_style_surface_sample_contract
```

Expected: FAIL because `PerfCircuitSource::Fixture` does not exist or the new case is missing.

- [ ] **Step 3: Add fixture source metadata and the benchmark case**

In `rstim/src/perf/cases.rs`, add this struct above `PerfCircuitSource`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerfNoiseMetadata {
    pub before_round_data_depolarization: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
}
```

Extend `PerfCircuitSource`:

```rust
    Fixture {
        case_id: &'static str,
        canonical_input_path: &'static str,
        noise: PerfNoiseMetadata,
    },
```

Add constants near the comparison constants:

```rust
const STIM_SURFACE_D11_R100_FIXTURE_PATH: &str =
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim";
const STIM_STYLE_SURFACE_NOISE: PerfNoiseMetadata = PerfNoiseMetadata {
    before_round_data_depolarization: 0.0,
    after_clifford_depolarization: 0.001,
    before_measure_flip_probability: 0.001,
    after_reset_flip_probability: 0.001,
};
```

Add this `PerfBenchmarkCase` to the `benchmark_cases()` vector:

```rust
        PerfBenchmarkCase {
            label: "stim-style-surface-sample-d11-r100-b1024",
            workload: PerfWorkload::Sample,
            source: PerfCircuitSource::Fixture {
                case_id: "stim_surface_d11_r100",
                canonical_input_path: STIM_SURFACE_D11_R100_FIXTURE_PATH,
                noise: STIM_STYLE_SURFACE_NOISE,
            },
            shots: Some(1024),
            tier: PerfCaseTier::ReportOnly,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: SAMPLER_COMPARE,
        },
```

In `rstim/src/perf.rs`, add `PerfNoiseMetadata` to the `pub use cases::{ ... }` list.

- [ ] **Step 4: Update fixture loading in the runner**

In `rstim/src/perf/runner.rs`, add:

```rust
use std::path::{Path, PathBuf};
```

Add helper functions above `source_text`:

```rust
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn fixture_text(canonical_input_path: &str) -> Result<String, String> {
    let path = workspace_root().join(canonical_input_path);
    std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read perf fixture {}: {e}", path.display()))
}
```

Add the fixture match arm in `source_text`:

```rust
        super::PerfCircuitSource::Fixture {
            canonical_input_path,
            ..
        } => fixture_text(canonical_input_path),
```

- [ ] **Step 5: Update existing perf harness expectations**

In `benchmark_cases_define_gating_and_report_only_contracts`, add:

```rust
        (
            "stim-style-surface-sample-d11-r100-b1024",
            PerfCaseTier::ReportOnly,
            true,
            false,
        ),
```

In `benchmark_case_variants_and_comparisons_match_declared_contracts`, add:

```rust
        (
            "stim-style-surface-sample-d11-r100-b1024",
            vec![
                PerfVariant::StimCli,
                PerfVariant::RstimInterpreted,
                PerfVariant::RstimCompiled,
            ],
            vec![PerfComparisonKind::SamplerCompiledVsInterpreted],
        ),
```

Update that test's `match case.source` to handle fixtures:

```rust
            PerfCircuitSource::Fixture {
                canonical_input_path,
                ..
            } => parse_lines(
                &std::fs::read_to_string(std::path::Path::new("..").join(canonical_input_path))
                    .unwrap(),
            )
            .unwrap(),
```

- [ ] **Step 6: Run the focused test and verify GREEN**

Run:

```sh
cargo test -p rstim --test perf_harness -- --exact benchmark_cases_include_stim_style_surface_sample_contract
```

Expected: PASS.

- [ ] **Step 7: Run the perf harness integration tests**

Run:

```sh
cargo test -p rstim --test perf_harness
```

Expected: PASS.

- [ ] **Step 8: Run rustfmt**

Run:

```sh
cargo fmt
```

Expected: no formatting errors.

- [ ] **Step 9: Commit**

Run:

```sh
git add rstim/src/perf/cases.rs rstim/src/perf/runner.rs rstim/src/perf.rs rstim/tests/perf_harness.rs docs/superpowers/plans/2026-07-08-issue-388-stim-style-surface-perf-case.md
git commit -m "perf: add stim style surface sample case"
```

Expected: implementation commit created.

## Self-Review

- Spec coverage: the single task covers the checked fixture source, report-only case, explicit noise metadata, expected variants, and negative-control assertions.
- Placeholder scan: no placeholder markers remain.
- Type consistency: `PerfNoiseMetadata` and `PerfCircuitSource::Fixture` field names are consistent across the implementation and tests.
