# Issue 95 rbposd ProductSum Serial BP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement selector-aware `ProductSum` BP updates and `Serial` BP scheduling in `rbposd`, shared by OSD-backed and LSD-backed decoders.

**Architecture:** Keep dispatch in `rbposd/src/bp.rs`, where `DecoderConfig` selects the check update rule and schedule loop. Preserve the existing minimum-sum parallel loop as the default path, add product-sum check messages and serial sweep behavior beside it, and extend parity fixtures/tests to document an observable non-default difference.

**Tech Stack:** Rust 2024, Cargo workspace, `rbposd` crate unit and integration tests, checked-in JSON parity fixtures.

## Global Constraints

- Preserve `DecoderConfig::default()` as `minimum_sum + parallel`.
- Preserve existing tests for `minimum_sum + parallel`.
- Keep dispatch explicit and local in `rbposd/src/bp.rs`.
- Make the selected BP execution path reusable by both OSD and LSD decoder families.
- Do not change `rsinter` parameter parsing.
- Do not edit benchmark specs.
- Do not add decoder families beyond OSD and LSD.

---

## File Structure

- Modify `rbposd/src/bp.rs`: add product-sum check update functions, serial schedule loop, and explicit `(BpVariant, Schedule)` dispatch.
- Modify `rbposd/dev/parity_schema.rs`: parse `product_sum` and `serial`, build matching config values, and pass selected BP config into LSD fixture decoding.
- Modify `rbposd/tests/bp.rs`: add borrowed-case snapshot tests and the default-path negative-control test required by issue #95.
- Modify `rbposd/tests/lsd_bp_config.rs`: prove LSD-backed decoding consumes the selected BP execution path.
- Modify `rbposd/tests/reference.rs`: include the new non-default parity fixture in the seed fixture list.
- Create `rbposd/tests/fixtures/parity/bp_product_sum_serial_sensitive.json`: document a deterministic non-default-sensitive borrowed case.
- Modify `rbposd/doc/ldpc_mvp_reference.md`: record the product-sum/serial behavior contract and fixture.

## Task 1: Failing OSD/LSD Contract Tests and Fixture Schema

**Files:**
- Modify: `rbposd/dev/parity_schema.rs`
- Modify: `rbposd/tests/bp.rs`
- Modify: `rbposd/tests/lsd_bp_config.rs`
- Modify: `rbposd/tests/reference.rs`
- Create: `rbposd/tests/fixtures/parity/bp_product_sum_serial_sensitive.json`

**Interfaces:**
- Consumes: `ParityCase`, `BpOsdDecoder`, `BpLsdDecoder::with_bp_config`, `BpVariant::ProductSum`, `Schedule::Serial`.
- Produces: failing tests named `product_sum_serial_changes_bp_snapshot_on_borrowed_case` and `minimum_sum_parallel_regression_suite_still_passes`.

- [ ] **Step 1: Extend parity schema variants for test inputs**

In `rbposd/dev/parity_schema.rs`, change the fixture-only enums and config builder to:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BpVariantSpec {
    MinimumSum,
    ProductSum,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleSpec {
    Parallel,
    Serial,
}
```

and:

```rust
bp_variant: match self.bp_variant {
    BpVariantSpec::MinimumSum => BpVariant::MinimumSum,
    BpVariantSpec::ProductSum => BpVariant::ProductSum,
},
schedule: match self.schedule {
    ScheduleSpec::Parallel => Schedule::Parallel,
    ScheduleSpec::Serial => Schedule::Serial,
},
```

- [ ] **Step 2: Route LSD fixture decoding through the selected BP config**

In `rbposd/dev/parity_schema.rs`, replace `build_lsd_decoder` with:

```rust
fn build_lsd_decoder(&self) -> Result<BpLsdDecoder, DecodeError> {
    let lsd_config = self
        .lsd_config
        .map(LsdConfigSpec::build)
        .unwrap_or_default();
    BpLsdDecoder::with_bp_config(
        self.matrix.build()?,
        self.channel.build(),
        lsd_config,
        self.config.build(),
    )
}
```

- [ ] **Step 3: Add the non-default-sensitive parity fixture**

Create `rbposd/tests/fixtures/parity/bp_product_sum_serial_sensitive.json`
from a deterministic 3-check / 4-bit chain case that exposes a public decode
result difference between default `MinimumSum + Parallel` and non-default
`ProductSum + Serial`:

```json
{
  "name": "bp_product_sum_serial_sensitive",
  "matrix": {
    "num_checks": 3,
    "num_bits": 4,
    "rows": [[0, 1], [1, 2], [2, 3]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false, true],
  "config": {
    "max_bp_iterations": 30,
    "early_stop": true,
    "bp_variant": "product_sum",
    "schedule": "serial",
    "osd_variant": "osd0"
  },
  "expected": {
    "status": "success",
    "correction": [false, true, true, false],
    "diagnostics": {
      "converged": true,
      "bp_iterations": 3,
      "used_osd": false,
      "residual_syndrome_weight": 0
    }
  },
  "tags": ["static-baseline", "bp-only", "product-sum", "serial"]
}
```

- [ ] **Step 4: Add the required OSD snapshot and regression tests**

Append to `rbposd/tests/bp.rs`:

```rust
#[path = "../dev/parity_runner.rs"]
mod parity_runner;
#[path = "../dev/parity_schema.rs"]
mod parity_schema;

use std::path::PathBuf;

use rbposd::{BpVariant, Schedule};

fn parity_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}

fn load_parity_case(name: &str) -> parity_schema::ParityCase {
    parity_schema::load_case(&parity_fixture_dir().join(name))
}

#[test]
fn product_sum_serial_changes_bp_snapshot_on_borrowed_case() {
    let default_case = load_parity_case("bp_repetition_single_flip.json");
    let sensitive_case = load_parity_case("bp_product_sum_serial_sensitive.json");

    assert_eq!(default_case.config.bp_variant, parity_schema::BpVariantSpec::MinimumSum);
    assert_eq!(default_case.config.schedule, parity_schema::ScheduleSpec::Parallel);
    assert_eq!(sensitive_case.config.bp_variant, parity_schema::BpVariantSpec::ProductSum);
    assert_eq!(sensitive_case.config.schedule, parity_schema::ScheduleSpec::Serial);

    let default_report = parity_runner::run_case(&default_case);
    let sensitive_report = parity_runner::run_case(&sensitive_case);

    assert_eq!(default_report.matches_expected, Some(true));
    assert_eq!(sensitive_report.matches_expected, Some(true));

    let mut comparison_case = sensitive_case.clone();
    comparison_case.config.bp_variant = parity_schema::BpVariantSpec::MinimumSum;
    comparison_case.config.schedule = parity_schema::ScheduleSpec::Parallel;
    let default_mode_report = parity_runner::run_case(&comparison_case);

    assert_ne!(
        sensitive_report.actual,
        default_mode_report.actual,
        "product_sum + serial must differ from minimum_sum + parallel on the sensitive fixture"
    );
}

#[test]
fn minimum_sum_parallel_regression_suite_still_passes() {
    for fixture_name in [
        "bp_repetition_single_flip.json",
        "osd_equal_reliability_tiebreak.json",
        "osd_small_sparse_code.json",
    ] {
        let case = load_parity_case(fixture_name);
        assert_eq!(case.config.bp_variant, parity_schema::BpVariantSpec::MinimumSum);
        assert_eq!(case.config.schedule, parity_schema::ScheduleSpec::Parallel);
        let report = parity_runner::run_case(&case);
        assert_eq!(
            report.matches_expected,
            Some(true),
            "default regression fixture {fixture_name} changed: expected {:?}, actual {:?}",
            report.expected,
            report.actual
        );
    }
}
```

Add `PartialEq, Eq` derives to `BpVariantSpec` and `ScheduleSpec` so the assertions compile.

- [ ] **Step 5: Add an LSD selected-path test**

Append to `rbposd/tests/lsd_bp_config.rs`:

```rust
use rbposd::{BpVariant, Schedule};

#[test]
fn bplsddecoder_with_bp_config_uses_product_sum_serial_execution() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        3,
        4,
        vec![vec![0, 1], vec![1, 2], vec![2, 3]],
    )
    .unwrap();
    let decoder = BpLsdDecoder::with_bp_config(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.2, 0.35, 0.2, 0.2]),
        LsdConfig::default(),
        DecoderConfig {
            max_bp_iterations: 3,
            early_stop: false,
            bp_variant: BpVariant::ProductSum,
            schedule: Schedule::Serial,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false, true]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert_eq!(result.bp_iterations, 3);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}
```

- [ ] **Step 6: Include the new fixture in the seed fixture contract**

In `rbposd/tests/reference.rs`, include `"bp_product_sum_serial_sensitive.json"` in the sorted expected file list before `"bp_repetition_single_flip.json"`.

- [ ] **Step 7: Run tests and verify the expected red state**

Run:

```bash
cargo test -p rbposd product_sum_serial_changes_bp_snapshot_on_borrowed_case
cargo test -p rbposd bplsddecoder_with_bp_config_uses_product_sum_serial_execution
```

Expected: FAIL because `product_sum + serial` still routes to the current minimum-sum parallel kernel and does not match the new non-default fixture.

- [ ] **Step 8: Commit Task 1**

```bash
git add rbposd/dev/parity_schema.rs rbposd/tests/bp.rs rbposd/tests/lsd_bp_config.rs rbposd/tests/reference.rs rbposd/tests/fixtures/parity/bp_product_sum_serial_sensitive.json
git commit -m "test: cover rbposd product sum serial bp path"
```

## Task 2: Product-Sum Check Updates and Serial BP Schedule

**Files:**
- Modify: `rbposd/src/bp.rs`

**Interfaces:**
- Consumes: `CompiledGraph`, `Syndrome`, `BpWorkspace`, `DecoderConfig`.
- Produces: `run_bp_compiled_in_place` dispatch where `(ProductSum, Serial)` runs product-sum check updates with serial scheduling and `(MinimumSum, Parallel)` preserves the existing loop.

- [ ] **Step 1: Add helper enums and numeric constants**

In `rbposd/src/bp.rs`, below `CERTAINTY_LLR`, add:

```rust
const TANH_EPSILON: f64 = 1.0e-12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckUpdateRule {
    MinimumSum,
    ProductSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BpSchedule {
    Parallel,
    Serial,
}
```

- [ ] **Step 2: Split minimum-sum check update into a per-check helper**

Extract the body of the existing per-check loop into:

```rust
fn update_minimum_sum_check_to_variable_messages_for_check(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    check: usize,
) {
    let start = graph.check_edge_offsets[check];
    let end = graph.check_edge_offsets[check + 1];
    let syndrome_sign = if syndrome.as_slice()[check] { -1.0 } else { 1.0 };

    if end - start == 1 {
        workspace.c_to_v[start] = syndrome_sign * CERTAINTY_LLR;
        return;
    }

    let mut total_sign = syndrome_sign;
    let mut min_abs = f64::INFINITY;
    let mut second_min_abs = f64::INFINITY;
    let mut min_count = 0usize;

    for edge in start..end {
        let msg = workspace.v_to_c[edge];
        if msg < 0.0 {
            total_sign = -total_sign;
        }
        let abs = msg.abs();
        if abs < min_abs {
            second_min_abs = min_abs;
            min_abs = abs;
            min_count = 1;
        } else if abs == min_abs {
            min_count += 1;
        } else if abs < second_min_abs {
            second_min_abs = abs;
        }
    }

    for edge in start..end {
        let msg = workspace.v_to_c[edge];
        let sign = if msg < 0.0 { -total_sign } else { total_sign };
        let abs = msg.abs();
        let excluded_min_abs = if abs == min_abs && min_count == 1 {
            second_min_abs
        } else {
            min_abs
        };
        workspace.c_to_v[edge] = sign * excluded_min_abs;
    }
}
```

Then make `update_check_to_variable_messages` call that helper for each check to preserve the current public unit test.

- [ ] **Step 3: Add product-sum check update helpers**

Add:

```rust
fn clamp_tanh_product(value: f64) -> f64 {
    value.clamp(-1.0 + TANH_EPSILON, 1.0 - TANH_EPSILON)
}

fn product_sum_message_from_extrinsic(extrinsic_product: f64) -> f64 {
    2.0 * clamp_tanh_product(extrinsic_product).atanh()
}

fn update_product_sum_check_to_variable_messages_for_check(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    check: usize,
) {
    let start = graph.check_edge_offsets[check];
    let end = graph.check_edge_offsets[check + 1];
    let syndrome_sign = if syndrome.as_slice()[check] { -1.0 } else { 1.0 };

    if end - start == 1 {
        workspace.c_to_v[start] = syndrome_sign * CERTAINTY_LLR;
        return;
    }

    for target_edge in start..end {
        let mut product = syndrome_sign;
        for edge in start..end {
            if edge != target_edge {
                product *= (workspace.v_to_c[edge] / 2.0).tanh();
            }
        }
        workspace.c_to_v[target_edge] = product_sum_message_from_extrinsic(product);
    }
}
```

- [ ] **Step 4: Add generic update dispatch helpers**

Add:

```rust
fn update_check_to_variable_messages_for_check(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    check: usize,
    rule: CheckUpdateRule,
) {
    match rule {
        CheckUpdateRule::MinimumSum => {
            update_minimum_sum_check_to_variable_messages_for_check(
                graph, syndrome, workspace, check,
            );
        }
        CheckUpdateRule::ProductSum => {
            update_product_sum_check_to_variable_messages_for_check(
                graph, syndrome, workspace, check,
            );
        }
    }
}

fn update_check_to_variable_messages_with_rule(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
) {
    for check in 0..graph.num_checks {
        update_check_to_variable_messages_for_check(graph, syndrome, workspace, check, rule);
    }
}
```

- [ ] **Step 5: Split variable update logic for full and touched-bit updates**

Extract the existing posterior and variable-to-check phases into:

```rust
fn refresh_bit_posterior_from_messages(
    graph: &CompiledGraph,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
    bit: usize,
) {
    let start = graph.bit_edge_offsets[bit];
    let end = graph.bit_edge_offsets[bit + 1];
    let mut incoming_sum = 0.0;
    for slot in start..end {
        let edge = graph.bit_edges[slot];
        incoming_sum += workspace.c_to_v[edge];
    }
    workspace.incoming_llr_sum[bit] = incoming_sum;
    workspace.posterior_llr[bit] = prior_llrs[bit] + incoming_sum;
    workspace.hard_decision_bits[bit] = workspace.posterior_llr[bit] < 0.0;
    workspace.reliability[bit] = workspace.posterior_llr[bit].abs();
}

fn refresh_all_bit_posteriors(
    graph: &CompiledGraph,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
) {
    for bit in 0..graph.num_bits {
        refresh_bit_posterior_from_messages(graph, prior_llrs, workspace, bit);
    }
}

fn refresh_variable_to_check_messages_for_bit(
    graph: &CompiledGraph,
    workspace: &mut BpWorkspace,
    bit: usize,
) {
    let start = graph.bit_edge_offsets[bit];
    let end = graph.bit_edge_offsets[bit + 1];
    for slot in start..end {
        let edge = graph.bit_edges[slot];
        workspace.v_to_c[edge] = workspace.posterior_llr[bit] - workspace.c_to_v[edge];
    }
}

fn refresh_all_variable_to_check_messages(graph: &CompiledGraph, workspace: &mut BpWorkspace) {
    for bit in 0..graph.num_bits {
        refresh_variable_to_check_messages_for_bit(graph, workspace, bit);
    }
}
```

- [ ] **Step 6: Add selected BP setup and shared best-snapshot helpers**

Add:

```rust
fn initialize_variable_to_check_messages(
    graph: &CompiledGraph,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
) {
    for edge in 0..graph.edge_bits.len() {
        workspace.v_to_c[edge] = prior_llrs[graph.edge_bits[edge]];
    }
}

fn zero_iteration_snapshot(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    for bit in 0..graph.num_bits {
        workspace.hard_decision_bits[bit] = prior_llrs[bit] < 0.0;
        workspace.reliability[bit] = prior_llrs[bit].abs();
    }
    let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
    BpRunInfo {
        iterations: 0,
        converged: residual_weight == 0,
        residual_weight,
    }
}

fn remember_converged_snapshot(workspace: &mut BpWorkspace) {
    workspace
        .best_hard_decision_bits
        .copy_from_slice(&workspace.hard_decision_bits);
    workspace
        .best_reliability
        .copy_from_slice(&workspace.reliability);
}

fn restore_converged_snapshot(workspace: &mut BpWorkspace) {
    workspace
        .hard_decision_bits
        .copy_from_slice(&workspace.best_hard_decision_bits);
    workspace
        .reliability
        .copy_from_slice(&workspace.best_reliability);
    workspace.residual_weight = 0;
}
```

- [ ] **Step 7: Add parallel and serial run loops**

Rename the current loop body into:

```rust
fn run_bp_parallel_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
) -> BpRunInfo {
    let mut best_info = None;

    for iteration in 1..=config.max_bp_iterations {
        update_check_to_variable_messages_with_rule(graph, syndrome, workspace, rule);
        refresh_all_bit_posteriors(graph, prior_llrs, workspace);

        let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
        if residual_weight == 0 {
            let info = BpRunInfo {
                iterations: iteration,
                converged: true,
                residual_weight: 0,
            };
            if config.early_stop {
                return info;
            }
            remember_converged_snapshot(workspace);
            best_info = Some(info);
        }

        refresh_all_variable_to_check_messages(graph, workspace);
    }

    if let Some(info) = best_info {
        restore_converged_snapshot(workspace);
        return info;
    }

    BpRunInfo {
        iterations: config.max_bp_iterations,
        converged: workspace.residual_weight == 0,
        residual_weight: workspace.residual_weight,
    }
}
```

Add:

```rust
fn run_bp_serial_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
) -> BpRunInfo {
    let mut best_info = None;

    for iteration in 1..=config.max_bp_iterations {
        for check in 0..graph.num_checks {
            update_check_to_variable_messages_for_check(
                graph, syndrome, workspace, check, rule,
            );
            let start = graph.check_edge_offsets[check];
            let end = graph.check_edge_offsets[check + 1];
            for edge in start..end {
                let bit = graph.edge_bits[edge];
                refresh_bit_posterior_from_messages(graph, prior_llrs, workspace, bit);
                refresh_variable_to_check_messages_for_bit(graph, workspace, bit);
            }
        }

        let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
        if residual_weight == 0 {
            let info = BpRunInfo {
                iterations: iteration,
                converged: true,
                residual_weight: 0,
            };
            if config.early_stop {
                return info;
            }
            remember_converged_snapshot(workspace);
            best_info = Some(info);
        }
    }

    if let Some(info) = best_info {
        restore_converged_snapshot(workspace);
        return info;
    }

    BpRunInfo {
        iterations: config.max_bp_iterations,
        converged: workspace.residual_weight == 0,
        residual_weight: workspace.residual_weight,
    }
}
```

- [ ] **Step 8: Route `run_bp_compiled_in_place` through rule and schedule selectors**

Replace `run_bp_compiled_in_place` with:

```rust
pub(crate) fn run_bp_compiled_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    let rule = match config.bp_variant {
        BpVariant::MinimumSum => CheckUpdateRule::MinimumSum,
        BpVariant::ProductSum => CheckUpdateRule::ProductSum,
    };
    let schedule = match config.schedule {
        Schedule::Parallel => BpSchedule::Parallel,
        Schedule::Serial => BpSchedule::Serial,
    };
    run_bp_selected_in_place(graph, syndrome, prior_llrs, config, workspace, rule, schedule)
}
```

Add:

```rust
fn run_bp_selected_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
    schedule: BpSchedule,
) -> BpRunInfo {
    workspace.reset(graph, prior_llrs);
    if config.max_bp_iterations == 0 {
        return zero_iteration_snapshot(graph, syndrome, prior_llrs, workspace);
    }
    initialize_variable_to_check_messages(graph, prior_llrs, workspace);
    match schedule {
        BpSchedule::Parallel => {
            run_bp_parallel_in_place(graph, syndrome, prior_llrs, config, workspace, rule)
        }
        BpSchedule::Serial => {
            run_bp_serial_in_place(graph, syndrome, prior_llrs, config, workspace, rule)
        }
    }
}
```

Then make `run_minimum_sum_compiled_in_place` call `run_bp_selected_in_place` with `CheckUpdateRule::MinimumSum` and `BpSchedule::Parallel` so existing tests keep the default kernel.

- [ ] **Step 9: Add focused BP unit tests**

In the `#[cfg(test)]` module in `rbposd/src/bp.rs`, add tests that verify product-sum messages stay finite and differ from minimum-sum on a degree-three check, and serial scheduling changes a sensitive workspace snapshot:

```rust
#[test]
fn product_sum_check_update_differs_from_minimum_sum_for_degree_three_check() {
    let pcm =
        ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
    let graph = CompiledGraph::from_pcm(&pcm);
    let syndrome = Syndrome::from(vec![false]);
    let mut minimum_workspace = BpWorkspace::new(&graph);
    let mut product_workspace = BpWorkspace::new(&graph);
    minimum_workspace.v_to_c = vec![0.8, -1.2, 1.6];
    product_workspace.v_to_c = minimum_workspace.v_to_c.clone();

    update_check_to_variable_messages_with_rule(
        &graph,
        &syndrome,
        &mut minimum_workspace,
        CheckUpdateRule::MinimumSum,
    );
    update_check_to_variable_messages_with_rule(
        &graph,
        &syndrome,
        &mut product_workspace,
        CheckUpdateRule::ProductSum,
    );

    assert_ne!(minimum_workspace.c_to_v, product_workspace.c_to_v);
    assert!(product_workspace.c_to_v.iter().all(|value| value.is_finite()));
}

#[test]
fn serial_schedule_updates_messages_differently_from_parallel_schedule() {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        3,
        4,
        vec![vec![0, 1], vec![1, 2], vec![2, 3]],
    )
    .unwrap();
    let graph = CompiledGraph::from_pcm(&pcm);
    let syndrome = Syndrome::from(vec![true, false, true]);
    let prior_llrs = vec![
        ((1.0_f64 - 0.2) / 0.2).ln(),
        ((1.0_f64 - 0.35) / 0.35).ln(),
        ((1.0_f64 - 0.2) / 0.2).ln(),
        ((1.0_f64 - 0.2) / 0.2).ln(),
    ];
    let config = DecoderConfig {
        max_bp_iterations: 3,
        early_stop: false,
        bp_variant: BpVariant::ProductSum,
        schedule: Schedule::Parallel,
        ..DecoderConfig::default()
    };
    let mut parallel_workspace = BpWorkspace::new(&graph);
    let mut serial_workspace = BpWorkspace::new(&graph);
    run_bp_selected_in_place(
        &graph,
        &syndrome,
        &prior_llrs,
        &config,
        &mut parallel_workspace,
        CheckUpdateRule::ProductSum,
        BpSchedule::Parallel,
    );
    run_bp_selected_in_place(
        &graph,
        &syndrome,
        &prior_llrs,
        &config,
        &mut serial_workspace,
        CheckUpdateRule::ProductSum,
        BpSchedule::Serial,
    );

    assert_ne!(parallel_workspace.reliability, serial_workspace.reliability);
    assert_eq!(
        serial_workspace.hard_decision_bits,
        vec![true, false, false, true]
    );
}
```

- [ ] **Step 10: Run focused tests and verify green**

Run:

```bash
cargo test -p rbposd product_sum_check_update_differs_from_minimum_sum_for_degree_three_check
cargo test -p rbposd serial_schedule_updates_messages_differently_from_parallel_schedule
cargo test -p rbposd product_sum_serial_changes_bp_snapshot_on_borrowed_case
cargo test -p rbposd bplsddecoder_with_bp_config_uses_product_sum_serial_execution
```

Expected: all commands PASS.

- [ ] **Step 11: Commit Task 2**

```bash
git add rbposd/src/bp.rs rbposd/tests/fixtures/parity/bp_product_sum_serial_sensitive.json
git commit -m "feat: implement rbposd product sum serial bp"
```

## Task 3: Documentation and Full Verification

**Files:**
- Modify: `rbposd/doc/ldpc_mvp_reference.md`

**Interfaces:**
- Consumes: implemented `ProductSum + Serial` behavior and new parity fixture.
- Produces: documented behavior note and complete verification evidence.

- [ ] **Step 1: Update reference documentation**

In `rbposd/doc/ldpc_mvp_reference.md`, add `bp_product_sum_serial_sensitive.json` to the parity fixture list and replace the excluded note that product-sum/serial internals are deferred with a note that issue #95 implements the first compiled `ProductSum` update and `Serial` schedule path.

- [ ] **Step 2: Run issue-required verification**

Run:

```bash
cargo test -p rbposd product_sum_serial_changes_bp_snapshot_on_borrowed_case
cargo test -p rbposd minimum_sum_parallel_regression_suite_still_passes
```

Expected: both commands PASS.

- [ ] **Step 3: Run crate and workspace verification**

Run:

```bash
cargo test -p rbposd
cargo test
git diff --check
```

Expected: all commands PASS.

- [ ] **Step 4: Commit Task 3**

```bash
git add rbposd/doc/ldpc_mvp_reference.md
git commit -m "docs: document rbposd product sum serial bp path"
```

## Self-Review

- Spec coverage: Task 1 adds required failing contract tests and fixtures; Task 2 implements product-sum updates and serial scheduling in the compiled core; Task 3 documents the behavior and runs required verification.
- Placeholder scan: all tasks use concrete paths, functions, commands, and expected outcomes.
- Type consistency: selector names are consistently `BpVariant::ProductSum`, `Schedule::Serial`, fixture strings `product_sum`, and `serial`.
