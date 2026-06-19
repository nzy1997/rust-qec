# Issue 93 Rbposd LSD Benchmark Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic `rsinter` tests proving LSD-backed `rbposd` benchmark runs record normalized LSD params, write normal result artifacts, reject invalid LSD params before artifacts, and change logical error rate when `lsd_order` changes.

**Architecture:** Keep the existing `rbposd` runner and LSD DEM adapter path unless tests expose missing behavior. Add issue-named integration tests in `rsinter/tests/bench_run.rs` around the normal benchmark workflow, and add an exact small-DEM LSD behavior test in `rsinter/tests/decode_rbposd.rs`.

**Tech Stack:** Rust 2024 workspace; `rsinter` crate; `rbposd::{LsdConfig, LsdMethod}` through existing adapter APIs; `cargo test`; GitHub PR workflow.

## Global Constraints

- Do not update benchmark spec fixtures or plot rendering.
- Do not expand supported LSD methods or orders.
- Do not change `BenchmarkResultRow` shape.
- Do not add broad borrowed differential fixture coverage.
- Do not alter `rbposd` core decoding behavior.
- Keep `BenchmarkResultRow.params` flat.
- Record effective normalized runner values, not raw TOML text.
- Use `--offline` only if Cargo tries to access the network in this Agent Desk workspace.

---

## File Structure

- `rsinter/tests/bench_run.rs`
  - Add small helpers for the issue #91 LSD surface benchmark spec.
  - Add the three issue-named benchmark workflow tests.
- `rsinter/tests/decode_rbposd.rs`
  - Add the issue-named exact LSD logical-error-rate test and helper.
- `rsinter/src/bench/runners/rbposd.rs`
  - No planned changes. Modify only if the new normalized-param tests fail.

---

### Task 1: Add LSD Benchmark Workflow Tests

**Files:**
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `issue91_surface_spec(extra_params: &str) -> String`
- Consumes: `run_rust_benchmark(&BenchmarkSpec, &str, &Path, &RunnerRegistry, &Path) -> Result<PathBuf, String>`
- Consumes: `read_results_jsonl(input) -> Result<Vec<BenchmarkResultRow>, String>`
- Produces: `rbposd_lsd_benchmark_records_normalized_decoder_params`
- Produces: `rbposd_lsd_benchmark_run_writes_results_jsonl`
- Produces: `rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results`

- [ ] **Step 1: Write the missing issue-named benchmark tests**

In `rsinter/tests/bench_run.rs`, replace:

```rust
use rsinter::bench::result::read_results_jsonl;
```

with:

```rust
use rsinter::bench::result::{BenchmarkResultRow, read_results_jsonl};
```

Immediately after `issue91_surface_spec`, add:

```rust
fn run_issue91_surface_benchmark(extra_params: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let spec_text = issue91_surface_spec(extra_params);
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

    (dir, artifact_root)
}

fn read_issue91_lsd_results(artifact_root: &Path) -> Vec<BenchmarkResultRow> {
    let data = fs::read(
        artifact_root
            .join("rbposd_lsd")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    read_results_jsonl(&data[..]).unwrap()
}
```

Immediately before the existing `rbposd_lsd_run_uses_lsd_dem_adapter_and_writes_artifacts` test, add:

```rust
#[test]
fn rbposd_lsd_benchmark_records_normalized_decoder_params() {
    let (_dir, artifact_root) = run_issue91_surface_benchmark("lsd_order = 1");
    let rows = read_issue91_lsd_results(&artifact_root);

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.status, "ok");
    assert_eq!(row.error, None);
    assert_eq!(row.params["input_type"], serde_json::json!("surface"));
    assert_eq!(row.params["distance"], serde_json::json!(3));
    assert_eq!(row.params["rounds"], serde_json::json!(3));
    assert_eq!(row.params["p"], serde_json::json!(0.002));
    assert_eq!(row.params["bp_algorithm"], serde_json::json!("min_sum"));
    assert_eq!(row.params["bp_iters"], serde_json::json!(30));
    assert_eq!(row.params["early_stop"], serde_json::json!(true));
    assert_eq!(
        row.params["lsd_method"],
        serde_json::json!("localized_statistics")
    );
    assert_eq!(row.params["lsd_order"], serde_json::json!(1));
    assert_eq!(row.params["decoder_impl"], serde_json::json!("rbposd"));
    assert_eq!(row.params["seed"], serde_json::json!(12_345));
}

#[test]
fn rbposd_lsd_benchmark_run_writes_results_jsonl() {
    let (_dir, artifact_root) = run_issue91_surface_benchmark(
        r#"
lsd_method = "localized_statistics"
lsd_order = 1
"#,
    );

    let artifact_dir = artifact_root.join("rbposd_lsd").join("test-run");
    assert!(artifact_dir.join("run_manifest.json").exists());
    assert!(artifact_dir.join("results.jsonl").exists());

    let rows = read_issue91_lsd_results(&artifact_root);
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

#[test]
fn rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results() {
    let spec_text = issue91_surface_spec("bogus_lsd = 1");
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
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

    assert_eq!(err, "unknown rbposd runner param: bogus_lsd");
    assert!(!dir.path().join("rbposd_lsd").exists());
}
```

- [ ] **Step 2: Run the issue-named benchmark tests**

Run:

```bash
cargo test -p rsinter rbposd_lsd_benchmark_records_normalized_decoder_params
cargo test -p rsinter rbposd_lsd_benchmark_run_writes_results_jsonl
cargo test -p rsinter rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results
```

Expected: all three commands pass. If one fails because a normalized LSD param is missing from `BenchmarkResultRow.params`, update `rsinter/src/bench/runners/rbposd.rs` so the LSD branch's `normalized: ParamMap::from_pairs([...])` includes `("lsd_method", serde_json::json!(lsd_method))` and `("lsd_order", serde_json::json!(lsd_order))`, then rerun the failed command.

- [ ] **Step 3: Commit Task 1**

Run:

```bash
git add rsinter/tests/bench_run.rs rsinter/src/bench/runners/rbposd.rs
git commit -m "test: cover rbposd lsd benchmark result rows"
```

Expected: a commit is created. If `rsinter/src/bench/runners/rbposd.rs` was not modified, `git add` leaves it unchanged and the commit contains only `rsinter/tests/bench_run.rs`.

---

### Task 2: Add Exact LSD Order Logical Error Rate Test

**Files:**
- Modify: `rsinter/tests/decode_rbposd.rs`

**Interfaces:**
- Consumes: `RbposdLsdDemDecoder::new(LsdConfig) -> RbposdLsdDemDecoder`
- Consumes: `DetectorErrorModel::parse(&str) -> Result<DetectorErrorModel, _>`
- Produces: `rbposd_lsd_order_changes_logical_error_rate`
- Produces: private helper `exact_three_error_lsd_logical_error_rate(dem: &DetectorErrorModel, lsd_order: usize) -> f64`

- [ ] **Step 1: Write the exact deterministic LSD order test**

In `rsinter/tests/decode_rbposd.rs`, immediately after `rbposd_osd_order_changes_ler`, add:

```rust
#[test]
fn rbposd_lsd_order_changes_logical_error_rate() {
    let dem = DetectorErrorModel::parse(concat!(
        "error(0.3775406687981454) D0\n",
        "error(0.3775406687981454) D1\n",
        "error(0.3775406687981454) D1 L0\n",
    ))
    .unwrap();

    let order0_ler = exact_three_error_lsd_logical_error_rate(&dem, 0);
    let order1_ler = exact_three_error_lsd_logical_error_rate(&dem, 1);

    assert_ne!(
        order1_ler, order0_ler,
        "expected lsd_order to change LER: order0={order0_ler}, order1={order1_ler}"
    );
    assert!(
        order1_ler < order0_ler,
        "expected lsd_order=1 to improve LER: order0={order0_ler}, order1={order1_ler}"
    );
}
```

Immediately before `exact_three_error_logical_error_rate`, add:

```rust
fn exact_three_error_lsd_logical_error_rate(dem: &DetectorErrorModel, lsd_order: usize) -> f64 {
    let lsd_config = LsdConfig {
        lsd_order,
        ..LsdConfig::default()
    };
    let decoder = RbposdLsdDemDecoder::new(lsd_config);
    let compiled = decoder.compile_for_dem(dem).unwrap();
    let probabilities = [
        0.377_540_668_798_145_4,
        0.377_540_668_798_145_4,
        0.377_540_668_798_145_4,
    ];
    let mut ler = 0.0;
    for e0 in [false, true] {
        for e1 in [false, true] {
            for e2 in [false, true] {
                let event = [e0, e1, e2];
                let probability = event
                    .iter()
                    .zip(probabilities.iter())
                    .map(|(&fired, &p)| if fired { p } else { 1.0 - p })
                    .product::<f64>();
                let det0 = e0;
                let det1 = e1 ^ e2;
                let observed = e2;
                let det_byte = u8::from(det0) | (u8::from(det1) << 1);
                let predicted = compiled
                    .decode_shots_bit_packed(&[det_byte], 1, 2, 1)
                    .unwrap()[0]
                    & 1
                    != 0;
                if predicted != observed {
                    ler += probability;
                }
            }
        }
    }
    ler
}
```

- [ ] **Step 2: Run the issue-named LSD order test**

Run:

```bash
cargo test -p rsinter rbposd_lsd_order_changes_logical_error_rate
```

Expected: the command passes. If `order1_ler` is not lower than `order0_ler`, inspect the printed assertion values and swap the observable marker from the `D1 L0` column to the `D1` column so the known order-1 correction is the lower-error observable prediction, then rerun this command.

- [ ] **Step 3: Commit Task 2**

Run:

```bash
git add rsinter/tests/decode_rbposd.rs
git commit -m "test: cover deterministic rbposd lsd order behavior"
```

Expected: a commit is created.

---

### Task 3: Final Verification And PR Preparation

**Files:**
- No planned source edits. Modify only if verification exposes a defect in files touched by Tasks 1 or 2.

**Interfaces:**
- Produces: verified branch ready for code review and pull request creation.

- [ ] **Step 1: Run the issue verification commands**

Run:

```bash
cargo test -p rsinter rbposd_lsd_benchmark_records_normalized_decoder_params
cargo test -p rsinter rbposd_lsd_benchmark_run_writes_results_jsonl
cargo test -p rsinter rbposd_lsd_order_changes_logical_error_rate
cargo test -p rsinter rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results
```

Expected: all four commands pass.

- [ ] **Step 2: Run package and workspace verification**

Run:

```bash
cargo test -p rsinter
cargo test
git diff --check
```

Expected: all commands pass. If Cargo attempts network access, rerun the Cargo commands with `--offline` and record that automatic choice in the decision log.

- [ ] **Step 3: Run final branch review**

Use `superpowers:requesting-code-review` on the diff from `origin/master` to `HEAD`. Fix Critical or Important findings, rerun the relevant tests, and commit fixes before finishing.

- [ ] **Step 4: Finish through PR workflow**

Use `superpowers:finishing-a-development-branch`. Under the Standing Answer Policy, choose "Push and create a Pull Request" when asked how to finish the branch. Create a PR targeting `master` and do not merge it.

---

## Self-Review Notes

- Spec coverage: Task 1 covers normalized params, `results.jsonl`, and the negative control. Task 2 covers deterministic LSD-order behavior. Task 3 covers requested verification, whole-package/workspace verification, review, and PR creation.
- Placeholder scan: no unfinished markers remain.
- Type consistency: helper and test names match the files and imports they use.
