# Issue 277 ldpc-compatible OSD-CS selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit `ldpc`-compatible OSD-CS selector to `rbposd` while preserving the legacy Rust planner as the default.

**Architecture:** Extend the public OSD config enum with legacy and `ldpc` planner variants, route `decode`, profiling, and diagnostics through a selected planner, and parse benchmark `osd_method` names through the same `rbposd` selector contract. Diagnostics expose a stable planner name and candidate-plan counts so `ldpc_osd_cs` and the legacy frontier path are distinguishable.

**Tech Stack:** Rust 2024 workspace; `rbposd` crate; `rsinter` benchmark runner; existing GF(2)/OSD helpers; `cargo test`.

## Global Constraints

- Keep `DecoderConfig::default()` as `OsdVariant::Osd0` with `osd_order = 0`.
- Preserve existing behavior for callers that only set `osd_order`: use the legacy Rust frontier planner.
- The explicit `ldpc` planner candidate set is singles over all non-pivot columns plus pairs among the first `osd_order` non-pivot columns.
- Candidate scoring changes are out of scope; use the existing scoring function.
- Unsupported method names must be rejected and must name the unsupported string.
- No benchmark row or artifact directory may be emitted for unsupported `osd_method` values.

---

## File Structure

- Modify `rbposd/src/config.rs`: add public OSD selector variants and method-name parsing.
- Modify `rbposd/src/error.rs`: add `UnsupportedOsdMethod`.
- Modify `rbposd/src/osd.rs`: add planner-specific candidate planning, decode, and profiling helpers.
- Modify `rbposd/src/decoder.rs`: pass the effective planner and expose `osd_planner` in diagnostics.
- Modify `rbposd/tests/osd.rs`: add the required `ldpc_osd_cs` positive control.
- Modify `rbposd/tests/smoke.rs` and `rbposd/tests/parity_dev.rs`: cover the new public enum/error contract.
- Modify `rbposd/dev/parity_schema.rs`: keep stable error-code mapping exhaustive.
- Modify `rsinter/src/bench/runners/rbposd.rs`: parse runner `osd_method` names through `OsdVariant`.
- Modify `rsinter/tests/bench_run.rs` and `rsinter/tests/bench_runner_wrappers.rs`: add/adjust runner validation coverage.

### Task 1: Write Failing Selector And Diagnostic Tests

**Files:**
- Modify: `rbposd/tests/osd.rs`
- Modify: `rbposd/tests/smoke.rs`
- Modify: `rsinter/tests/bench_run.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

**Interfaces:**
- Consumes: existing `BpOsdDecoder::diagnose_osd_path`, `DecoderConfig`, `OsdVariant`.
- Produces: failing tests that require `OsdVariant::LdpcCombinationSweep`, `OsdVariant::from_method_name`, and `OsdPathDiagnostic::osd_planner`.

- [ ] **Step 1: Add the rbposd positive-control integration test**

Add this import update at the top of `rbposd/tests/osd.rs`:

```rust
use rbposd::{
    BpOsdDecoder, ChannelModel, Correction, DecodeError, DecoderConfig, OsdVariant,
    ParityCheckMatrix, Syndrome,
};
```

Add this test after `diagnose_osd_path_reports_candidate_search_planning`:

```rust
#[test]
fn ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        2,
        10,
        vec![vec![0, 2, 3, 4, 5, 6, 7, 8, 9], vec![1, 2, 3, 4, 5, 6, 7, 8, 9]],
    )
    .unwrap();
    let channel = ChannelModel::BitFlipProbabilities(vec![
        0.2, 0.2, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08,
    ]);
    let syndrome = Syndrome::from(vec![true, true]);

    let ldpc = BpOsdDecoder::new(
        pcm.clone(),
        channel.clone(),
        DecoderConfig {
            max_bp_iterations: 0,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let ldpc_diagnostic = ldpc.diagnose_osd_path(&syndrome).unwrap();

    assert_eq!(ldpc_diagnostic.osd_planner, "ldpc_osd_cs");
    assert!(ldpc_diagnostic.free_column_count >= 7);
    assert_eq!(ldpc_diagnostic.candidate_search_frontier_size, 7);
    assert_eq!(ldpc_diagnostic.max_candidate_order, 2);
    assert_eq!(
        ldpc_diagnostic.planned_candidate_count,
        ldpc_diagnostic.free_column_count as u128 + 21
    );

    let legacy = BpOsdDecoder::new(
        pcm,
        channel,
        DecoderConfig {
            max_bp_iterations: 0,
            osd_order: 7,
            ..DecoderConfig::default()
        },
    )
    .unwrap();
    let legacy_diagnostic = legacy.diagnose_osd_path(&syndrome).unwrap();

    assert_eq!(legacy_diagnostic.osd_planner, "legacy_combination_sweep");
    assert_ne!(legacy_diagnostic.osd_planner, ldpc_diagnostic.osd_planner);
    assert_ne!(
        legacy_diagnostic.planned_candidate_count,
        ldpc_diagnostic.planned_candidate_count
    );
}
```

- [ ] **Step 2: Add the rbposd unsupported-method negative control**

Add this test to `rbposd/tests/smoke.rs`:

```rust
#[test]
fn unsupported_osd_method_is_rejected_without_fallback() {
    let error = OsdVariant::from_method_name("osd_cs_typo").unwrap_err();

    assert_eq!(
        error,
        DecodeError::UnsupportedOsdMethod {
            method: "osd_cs_typo".to_string()
        }
    );
    assert!(error.to_string().contains("osd_cs_typo"));
}
```

- [ ] **Step 3: Add rsinter runner validation coverage**

In `rsinter/tests/bench_runner_wrappers.rs`, add:

```rust
#[test]
fn rbposd_runner_preflight_accepts_ldpc_osd_cs_method() {
    let runner = RbposdRunner;
    let point = rbposd_point_with_decoder_params(BTreeMap::from([
        ("osd_method".into(), toml::Value::String("ldpc_osd_cs".into())),
        ("osd_order".into(), toml::Value::Integer(7)),
    ]));

    runner.preflight_point(&point).unwrap();
}
```

Rename `rsinter/tests/bench_run.rs` test `rbposd_benchmark_rejects_unsupported_osd_method` to `unsupported_osd_method_is_rejected_without_fallback` and change the fixture method string to `osd_cs_typo`. Expected error:

```rust
assert_eq!(
    err,
    "unsupported OSD method \"osd_cs_typo\"; supported methods are combination_sweep, legacy_combination_sweep, ldpc_osd_cs, osd_cs"
);
assert!(!dir.path().join("rbposd_bad").exists());
```

- [ ] **Step 4: Verify tests fail for the missing implementation**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs -- --nocapture
cargo test -p rbposd unsupported_osd_method_is_rejected_without_fallback -q
cargo test -p rsinter unsupported_osd_method_is_rejected_without_fallback -q
```

Expected: FAIL with missing `LdpcCombinationSweep`, `from_method_name`, `UnsupportedOsdMethod`, and `osd_planner` symbols.

### Task 2: Implement rbposd Selector Routing

**Files:**
- Modify: `rbposd/src/config.rs`
- Modify: `rbposd/src/error.rs`
- Modify: `rbposd/src/osd.rs`
- Modify: `rbposd/src/decoder.rs`
- Modify: `rbposd/dev/parity_schema.rs`
- Modify: `rbposd/tests/smoke.rs`
- Modify: `rbposd/tests/parity_dev.rs`

**Interfaces:**
- Consumes: failing tests from Task 1.
- Produces: `OsdVariant::{LegacyCombinationSweep,LdpcCombinationSweep}`, `OsdVariant::from_method_name`, `OsdPathDiagnostic::osd_planner`, and planner-specific OSD traversal.

- [ ] **Step 1: Add public selector/error contract**

In `rbposd/src/config.rs`, extend `OsdVariant` and add:

```rust
impl OsdVariant {
    pub fn from_method_name(method: &str) -> Result<Self, crate::error::DecodeError> {
        match method {
            "combination_sweep" | "legacy_combination_sweep" => Ok(Self::LegacyCombinationSweep),
            "ldpc_osd_cs" | "osd_cs" => Ok(Self::LdpcCombinationSweep),
            other => Err(crate::error::DecodeError::UnsupportedOsdMethod {
                method: other.to_string(),
            }),
        }
    }

    pub fn planner_name(self) -> &'static str {
        match self {
            Self::Osd0 => "osd0",
            Self::LegacyCombinationSweep => "legacy_combination_sweep",
            Self::LdpcCombinationSweep => "ldpc_osd_cs",
        }
    }
}
```

In `rbposd/src/error.rs`, add:

```rust
UnsupportedOsdMethod {
    method: String,
},
```

and display it as:

```rust
Self::UnsupportedOsdMethod { method } => write!(
    f,
    "unsupported OSD method \"{method}\"; supported methods are combination_sweep, legacy_combination_sweep, ldpc_osd_cs, osd_cs"
),
```

Update `rbposd/tests/smoke.rs`, `rbposd/tests/parity_dev.rs`, and `rbposd/dev/parity_schema.rs` with the new enum/error cases.

- [ ] **Step 2: Route OSD helpers by planner**

In `rbposd/src/osd.rs`, import `OsdVariant`, pass a `planner: OsdVariant` into decode/diagnose/profile functions, and add helper:

```rust
pub(crate) fn effective_osd_variant(config: crate::config::DecoderConfig) -> OsdVariant {
    match config.osd_variant {
        OsdVariant::Osd0 if config.osd_order > 0 => OsdVariant::LegacyCombinationSweep,
        other => other,
    }
}
```

Split current `best_osd_candidate`, `candidate_search_plan`, and bounded
profile traversal into legacy helpers. Add `ldpc` helpers that visit every
free column as a single forced-column candidate, then visit pair combinations
from `base.free_columns[..min(len, osd_order)]`.

- [ ] **Step 3: Update diagnostics**

In `rbposd/src/decoder.rs`, add `pub osd_planner: &'static str` to
`OsdPathDiagnostic`. For zero-syndrome and BP-converged paths, set it to the
effective planner's `planner_name()`. For OSD-used paths, pass the effective
planner into `diagnose_osd_candidate_search_with_workspace` and report the same
planner name.

- [ ] **Step 4: Verify rbposd tests pass**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs -- --nocapture
cargo test -p rbposd unsupported_osd_method_is_rejected_without_fallback -q
cargo test -p rbposd
```

Expected: all pass.

### Task 3: Wire rsinter Benchmark Parser

**Files:**
- Modify: `rsinter/src/bench/runners/rbposd.rs`
- Modify: `rsinter/tests/bench_run.rs`
- Modify: `rsinter/tests/bench_runner_wrappers.rs`

**Interfaces:**
- Consumes: `OsdVariant::from_method_name`.
- Produces: runner validation and execution that selects the `ldpc` planner for `osd_cs` or `ldpc_osd_cs`.

- [ ] **Step 1: Parse OSD method through rbposd**

In `rsinter/src/bench/runners/rbposd.rs`, import `OsdVariant` and replace the
manual `osd_method != "combination_sweep"` check with:

```rust
let osd_method = optional_string(params, "osd_method")?
    .unwrap_or_else(|| "combination_sweep".to_string());
bp_config.osd_variant = OsdVariant::from_method_name(&osd_method).map_err(|error| error.to_string())?;
bp_config.osd_order = optional_usize(params, "osd_order")?.unwrap_or(bp_config.osd_order);
```

Keep normalized `osd_method` as the user/default string so benchmark rows remain explicit about the requested method.

- [ ] **Step 2: Verify rsinter focused tests pass**

Run:

```bash
cargo test -p rsinter unsupported_osd_method_is_rejected_without_fallback -q
cargo test -p rsinter rbposd_runner_preflight_accepts_ldpc_osd_cs_method -q
cargo test -p rsinter rbposd_benchmark_records_normalized_decoder_params -q
```

Expected: all pass.

### Task 4: Final Verification

**Files:**
- All touched files.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: formatted, verified PR-ready branch.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --check
```

If it fails, run `cargo fmt`, then rerun `cargo fmt --check`.

- [ ] **Step 2: Run issue verification**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs -- --nocapture
cargo test -p rbposd unsupported_osd_method_is_rejected_without_fallback -q
cargo test -p rsinter unsupported_osd_method_is_rejected_without_fallback -q
```

Expected: all pass, with the positive-control output showing the `ldpc` planner.

- [ ] **Step 3: Run full workspace verification**

Run:

```bash
cargo test
```

Expected: all pass.

## Self-Review

- Spec coverage: the plan covers explicit selector API, default legacy behavior, `ldpc` candidate planning, diagnostics, runner validation, and negative controls.
- Placeholder scan: no placeholder work remains.
- Type consistency: public names are `OsdVariant::LdpcCombinationSweep`, `OsdVariant::LegacyCombinationSweep`, `OsdVariant::from_method_name`, `DecodeError::UnsupportedOsdMethod`, and `OsdPathDiagnostic::osd_planner`.

## Execution Choice

Standing answer policy selects **Subagent-Driven (recommended)** because it is the recommended option in the writing-plans handoff. This run uses the plan task order and records any deviation caused by Agent Desk sandbox limits in the final decision log.
