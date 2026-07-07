# Issue 387 Sample Correctness Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Rust integration tests that validate compiled and interpreted `rstim` sample paths against the shared #385 fixture catalog.

**Architecture:** Keep the contract in `rstim/tests/sample_correctness_contract.rs`. The test loads the smoke TOML manifest with typed structs, validates metadata with `rstim::stats::summarize`, routes each executable case through `choose_sampler_path`, and compares detector/observable streams from `SamplingBackend::Interpreted` and `SamplingBackend::Compiled` over deterministic seeds.

**Tech Stack:** Rust 2024 integration tests, `rstim` public modules, `rand::rngs::StdRng`, `serde::Deserialize`, `toml` dev-dependency, checked `.stim` fixture files from `benchmarks/rstim_vs_stim_simulator/`.

## Global Constraints

- Test file name: `rstim/tests/sample_correctness_contract.rs`.
- Reuse `benchmarks/rstim_vs_stim_simulator/cases.smoke.toml`; do not duplicate circuit definitions.
- Keep the check internal to `rstim`; do not invoke Stim.
- Validate metadata before sampling and fail with `metadata mismatch`.
- Reject injected detector or observable stream disagreement with `statistical mismatch`.
- If a catalog case falls back from compiled sampling, require and report the explicit routing reason.
- Do not optimize compiled sampling.

---

## File Structure

- Modify `rstim/Cargo.toml`: add `toml = "0.8"` under `[dev-dependencies]`.
- Create `rstim/tests/sample_correctness_contract.rs`: manifest loading, metadata checks, routing checks, sample comparison helpers, main catalog test, and two negative controls.

### Task 1: Failing Integration Test Shell

**Files:**
- Modify: `rstim/Cargo.toml`
- Create: `rstim/tests/sample_correctness_contract.rs`

**Interfaces:**
- Consumes: `benchmarks/rstim_vs_stim_simulator/cases.smoke.toml` and fixture paths under the same directory.
- Produces: test named `compiled_and_interpreted_sample_paths_agree_on_catalog`.

- [ ] **Step 1: Add the dev dependency**

Edit `rstim/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
toml = "0.8"
```

- [ ] **Step 2: Add a minimal failing test file**

Create `rstim/tests/sample_correctness_contract.rs`:

```rust
#[test]
fn compiled_and_interpreted_sample_paths_agree_on_catalog() {
    panic!("contract not implemented");
}
```

- [ ] **Step 3: Verify RED**

Run:

```sh
cargo test -p rstim --test sample_correctness_contract -- --exact compiled_and_interpreted_sample_paths_agree_on_catalog
```

Expected: FAIL containing `contract not implemented`.

### Task 2: Metadata Contract And Negative Control

**Files:**
- Modify: `rstim/tests/sample_correctness_contract.rs`

**Interfaces:**
- Produces: `load_manifest(path: &Path) -> Result<Manifest, String>`.
- Produces: `validate_case_metadata(case: &CatalogCase) -> Result<ParsedCase, String>`.

- [ ] **Step 1: Write metadata tests and helpers**

Replace the test shell with typed manifest structs, a loader, and metadata
validation using `rstim::stats::summarize`. Add:

```rust
#[test]
fn metadata_contract_rejects_mismatched_detector_counts() {
    let mut manifest = load_smoke_manifest().expect("smoke manifest");
    manifest.cases[0].expected_detectors += 1;
    let err = validate_case_metadata(&manifest.cases[0]).unwrap_err();
    assert!(err.contains("metadata mismatch"));
}
```

- [ ] **Step 2: Verify RED/GREEN**

Run:

```sh
cargo test -p rstim --test sample_correctness_contract metadata_contract_rejects_mismatched_detector_counts
```

Expected after implementation: PASS.

### Task 3: Routing And Statistical Agreement Contract

**Files:**
- Modify: `rstim/tests/sample_correctness_contract.rs`

**Interfaces:**
- Produces: `compare_sample_paths(case: &ParsedCase) -> Result<CaseOutcome, String>`.
- Produces: `assert_streams_agree(case_id: &str, seed: u64, interpreted: &BatchOutput, compiled: &BatchOutput) -> Result<(), String>`.

- [ ] **Step 1: Add comparison helpers**

Implement helpers that:

```rust
let mut interpreted_rng = StdRng::seed_from_u64(seed);
let interpreted = sample_batch_with_options(
    &parsed.instrs,
    parsed.case.shots,
    &mut interpreted_rng,
    SampleOptions { backend: SamplingBackend::Interpreted, ..SampleOptions::default() },
)?;

let mut compiled_rng = StdRng::seed_from_u64(seed);
let compiled = sample_batch_with_options(
    &parsed.instrs,
    parsed.case.shots,
    &mut compiled_rng,
    SampleOptions { backend: SamplingBackend::Compiled, ..SampleOptions::default() },
)?;
```

Then compare detector and observable `BitTable` dimensions and bits. Return an
error containing `statistical mismatch` for any disagreement.

- [ ] **Step 2: Add the injected mismatch negative control**

Add:

```rust
#[test]
fn statistical_contract_rejects_injected_detector_or_observable_mismatch() {
    let parsed = first_compiled_capable_case().expect("compiled capable case");
    let mut outputs = sample_pair(&parsed, DETERMINISTIC_SEEDS[0]).expect("sample pair");
    flip_first_comparison_bit(&mut outputs.compiled).expect("comparison bit");
    let err = assert_streams_agree(
        &parsed.case.case_id,
        DETERMINISTIC_SEEDS[0],
        &outputs.interpreted,
        &outputs.compiled,
    )
    .unwrap_err();
    assert!(err.contains("statistical mismatch"));
}
```

- [ ] **Step 3: Implement the catalog test**

The main test must load the smoke manifest, metadata-check all cases,
sample non-`documentation-only` cases, record fallback reasons, require at
least one compiled-capable case, and print:

```rust
println!(
    "checked {compiled_checked} compiled-capable catalog cases; recorded {fallback_recorded} fallback cases"
);
```

- [ ] **Step 4: Verify focused test**

Run:

```sh
cargo test -p rstim --test sample_correctness_contract -- --exact compiled_and_interpreted_sample_paths_agree_on_catalog
```

Expected: PASS and output includes `checked`.

### Task 4: Full Verification And Commit

**Files:**
- Modify: any files touched by the previous tasks.

**Interfaces:**
- Produces: committed implementation ready for PR.

- [ ] **Step 1: Run all contract tests**

Run:

```sh
cargo test -p rstim --test sample_correctness_contract
```

Expected: PASS.

- [ ] **Step 2: Run repository-required verification**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 3: Check formatting and diff hygiene**

Run:

```sh
cargo fmt --check
git diff --check
git status --short
```

Expected: formatting and diff checks pass; only intended files are modified.

- [ ] **Step 4: Commit**

Run:

```sh
git add rstim/Cargo.toml rstim/tests/sample_correctness_contract.rs docs/superpowers/specs/2026-07-08-issue-387-sample-correctness-contract-design.md docs/superpowers/plans/2026-07-08-issue-387-sample-correctness-contract.md
git commit -m "test: add sample correctness contract"
```

Expected: commit succeeds.

## Self-Review

- The plan covers metadata mismatch, statistical mismatch, compiled routing,
  catalog reuse, focused verification, and full `cargo test`.
- No placeholder task text remains.
- File paths and test names match the issue request.
