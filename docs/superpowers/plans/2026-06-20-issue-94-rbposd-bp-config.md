# Issue 94 rbposd BP Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `rbposd::DecoderConfig` so public BP method and schedule selections include `ProductSum` and `Serial` while defaults remain `MinimumSum` and `Parallel`.

**Architecture:** Keep the existing `DecoderConfig` fields and public enum types. Add the new variants in `rbposd/src/config.rs`, validate them through crate-level integration tests, and route decoder execution through a selector-aware BP entrypoint that currently maps all supported selections to the existing MVP kernel.

**Tech Stack:** Rust 2024, Cargo workspace, `rbposd` crate integration tests.

## Global Constraints

- Preserve `DecoderConfig::default()` as `minimum_sum + parallel`.
- Use explicit Rust enum variants, not strings.
- Do not implement mathematically distinct ProductSum updates.
- Do not implement a true serial message-update schedule.
- Do not change `rsinter` runner parameter parsing.
- Do not update the Python differential harness.

---

## File Structure

- Modify `rbposd/src/config.rs`: add the public `BpVariant::ProductSum` and `Schedule::Serial` variants.
- Modify `rbposd/src/bp.rs`: add selector-aware compiled BP dispatch while preserving the existing minimum-sum kernel.
- Modify `rbposd/src/decoder_core.rs`: expose the selector-aware dispatch through `BpCore`.
- Modify `rbposd/src/decoder.rs`: call the selector-aware `BpCore` method.
- Modify `rbposd/src/lsd_decoder.rs`: call the selector-aware `BpCore` method for LSD-backed decoding.
- Modify `rbposd/tests/smoke.rs`: add public contract tests required by issue #94.
- Modify `rbposd/doc/ldpc_mvp_reference.md`: record the expanded public config contract.

## Task 1: Public Config Variants and Contract Tests

**Files:**
- Modify: `rbposd/src/config.rs`
- Modify: `rbposd/tests/smoke.rs`

**Interfaces:**
- Consumes: `DecoderConfig`, `BpVariant`, and `Schedule` as currently exported by `rbposd`.
- Produces: `BpVariant::ProductSum` and `Schedule::Serial` as public enum variants usable in `DecoderConfig`.

- [ ] **Step 1: Write the failing public contract tests**

Add these tests below `decoder_config_default_contract` in `rbposd/tests/smoke.rs`:

```rust
#[test]
fn decoder_config_defaults_do_not_silently_change() {
    let cfg = DecoderConfig::default();

    assert_eq!(cfg.bp_variant, BpVariant::MinimumSum);
    assert_eq!(cfg.schedule, Schedule::Parallel);
}

#[test]
fn decoder_config_exposes_bp_method_and_schedule_variants() {
    let methods = [BpVariant::MinimumSum, BpVariant::ProductSum];
    let schedules = [Schedule::Parallel, Schedule::Serial];

    assert_eq!(methods[0], BpVariant::MinimumSum);
    assert_eq!(methods[1], BpVariant::ProductSum);
    assert_eq!(schedules[0], Schedule::Parallel);
    assert_eq!(schedules[1], Schedule::Serial);

    let cfg = DecoderConfig {
        bp_variant: BpVariant::ProductSum,
        schedule: Schedule::Serial,
        ..DecoderConfig::default()
    };

    assert_eq!(cfg.bp_variant, BpVariant::ProductSum);
    assert_eq!(cfg.schedule, Schedule::Serial);
}
```

- [ ] **Step 2: Run the new variant exposure test and verify it fails**

Run:

```bash
cargo test -p rbposd decoder_config_exposes_bp_method_and_schedule_variants
```

Expected: FAIL with compiler errors that `BpVariant::ProductSum` and `Schedule::Serial` are missing.

- [ ] **Step 3: Add the public enum variants**

Change `rbposd/src/config.rs` to:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpVariant {
    MinimumSum,
    ProductSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Parallel,
    Serial,
}
```

Leave `DecoderConfig::default()` unchanged:

```rust
bp_variant: BpVariant::MinimumSum,
schedule: Schedule::Parallel,
```

- [ ] **Step 4: Run the targeted public contract tests**

Run:

```bash
cargo test -p rbposd decoder_config_exposes_bp_method_and_schedule_variants
cargo test -p rbposd decoder_config_defaults_do_not_silently_change
```

Expected: both commands PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add rbposd/src/config.rs rbposd/tests/smoke.rs
git commit -m "feat: expose rbposd bp config variants"
```

## Task 2: Selector-Aware BP Dispatch

**Files:**
- Modify: `rbposd/src/bp.rs`
- Modify: `rbposd/src/decoder_core.rs`
- Modify: `rbposd/src/decoder.rs`
- Modify: `rbposd/src/lsd_decoder.rs`

**Interfaces:**
- Consumes: `DecoderConfig { bp_variant, schedule, .. }`.
- Produces: `run_bp_compiled_in_place(graph, syndrome, prior_llrs, config, workspace) -> BpRunInfo` and `BpCore::run_bp_in_place(...) -> BpRunInfo`.

- [ ] **Step 1: Add selector-aware compiled dispatch in `bp.rs`**

Change the config import to:

```rust
use crate::config::{BpVariant, DecoderConfig, Schedule};
```

Add this function above `run_minimum_sum_compiled_in_place`:

```rust
pub(crate) fn run_bp_compiled_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    match (config.bp_variant, config.schedule) {
        (BpVariant::MinimumSum, Schedule::Parallel)
        | (BpVariant::MinimumSum, Schedule::Serial)
        | (BpVariant::ProductSum, Schedule::Parallel)
        | (BpVariant::ProductSum, Schedule::Serial) => {
            run_minimum_sum_compiled_in_place(graph, syndrome, prior_llrs, config, workspace)
        }
    }
}
```

- [ ] **Step 2: Route `BpCore` through the selector-aware function**

In `rbposd/src/decoder_core.rs`, import `run_bp_compiled_in_place` instead of `run_minimum_sum_compiled_in_place` and replace `run_minimum_sum_in_place` with:

```rust
pub(crate) fn run_bp_in_place(
    &self,
    syndrome: &Syndrome,
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    run_bp_compiled_in_place(
        &self.graph,
        syndrome,
        &self.prior_llrs,
        config,
        workspace,
    )
}
```

- [ ] **Step 3: Update OSD and LSD decoders to call `run_bp_in_place`**

In `rbposd/src/decoder.rs`, change:

```rust
.run_minimum_sum_in_place(syndrome, &self.config, &mut bp_workspace);
```

to:

```rust
.run_bp_in_place(syndrome, &self.config, &mut bp_workspace);
```

In `rbposd/src/lsd_decoder.rs`, change:

```rust
.run_minimum_sum_in_place(syndrome, &self.bp_config, &mut bp_workspace);
```

to:

```rust
.run_bp_in_place(syndrome, &self.bp_config, &mut bp_workspace);
```

- [ ] **Step 4: Run BP and LSD focused tests**

Run:

```bash
cargo test -p rbposd minimum_sum_decodes_a_single_flip_without_osd
cargo test -p rbposd lsd_order_one_improves_over_baseline_fixture
```

Expected: both commands PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add rbposd/src/bp.rs rbposd/src/decoder_core.rs rbposd/src/decoder.rs rbposd/src/lsd_decoder.rs
git commit -m "feat: route rbposd bp through config selectors"
```

## Task 3: Contract Documentation and Full Verification

**Files:**
- Modify: `rbposd/doc/ldpc_mvp_reference.md`

**Interfaces:**
- Consumes: expanded `BpVariant` and `Schedule` public surface.
- Produces: documented default contract and deferred algorithm notes.

- [ ] **Step 1: Update the reference contract doc**

In `rbposd/doc/ldpc_mvp_reference.md`, update the included public surface from:

```markdown
- `DecoderConfig` and its default contract:
  `max_bp_iterations=30`, `early_stop=true`, `bp_variant=MinimumSum`,
  `schedule=Parallel`, `osd_variant=Osd0`
```

to:

```markdown
- `DecoderConfig` and its default contract:
  `max_bp_iterations=30`, `early_stop=true`, `bp_variant=MinimumSum`,
  `schedule=Parallel`, `osd_variant=Osd0`
- `BpVariant` with `MinimumSum` and `ProductSum`
- `Schedule` with `Parallel` and `Serial`
```

Add this note under `Excluded`:

```markdown
- mathematically distinct `ProductSum` updates and true serial message
  scheduling internals; the issue #94 public selectors are compatibility
  surface until those algorithms are implemented
```

- [ ] **Step 2: Run issue-required verification**

Run:

```bash
cargo test -p rbposd decoder_config_exposes_bp_method_and_schedule_variants
cargo test -p rbposd decoder_config_defaults_do_not_silently_change
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
git commit -m "docs: record rbposd bp config variants"
```

## Self-Review

- Spec coverage: Task 1 exposes variants and pins defaults; Task 2 removes decoder call-site hard-coding; Task 3 documents the public contract and verification.
- Completion scan: all sections are concrete and ready to execute.
- Type consistency: public selectors are consistently `BpVariant::ProductSum` and `Schedule::Serial`; dispatch returns existing `BpRunInfo`.
