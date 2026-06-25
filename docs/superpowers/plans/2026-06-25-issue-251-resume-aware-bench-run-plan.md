# Issue 251 Resume-Aware Bench Run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--resume` support to `rsinter bench run` so existing artifact-directory JSONL rows can skip completed benchmark points and preserve interrupted work.

**Architecture:** Add a small options struct around `run_rust_benchmark`, load existing per-runner rows before cleanup when resume is enabled, compute planned row identities from runner output previews, skip complete identities, and write merged existing plus fresh rows through the current `test-run.tmp` staging path. Keep the public runner wrapper backward compatible and document the CLI flag.

**Tech Stack:** Rust 2024, clap derive, existing `rsinter` benchmark runner modules, existing JSONL helpers, existing identity-aware `merge_result_rows`, existing integration tests.

## Global Constraints

- Preserve the current `rsinter bench run --spec ... --language ... --out <artifact-dir>` model.
- Add `--resume` only to `bench run`; do not add a separate `--output` file mode.
- Existing rows are loaded from `<out>/<runner>/test-run/results.jsonl`.
- Existing JSONL must be read before any `test-run` artifact is deleted in resume mode.
- A completed existing row is `status == "ok"` and is matched to planned rows by `BenchmarkResultRow::identity()`.
- Incomplete existing rows are rerun and merged deterministically with fresh rows using `merge_result_rows`.
- Final output must not duplicate same-identity completed rows after one or more resume runs.
- Corrupted existing JSONL must fail clearly and leave the original `results.jsonl` untouched.
- Continue using `test-run.tmp` plus rename for final writes.
- Required focused verification command: `cargo test -p rsinter --test bench_run rust_benchmark_run_resumes_partial_results`.
- Required broad verification command: `cargo test`.

---

## File Structure

- Modify `rsinter/src/bench/run.rs`: add `BenchRunOptions`, add resume loading/planning/writing helpers, keep `run_rust_benchmark` as a compatibility wrapper.
- Modify `rsinter/src/bin/rsinter.rs`: add the `--resume` flag and call the options-based runner.
- Modify `rsinter/tests/bench_run.rs`: add a focused regression test and helper assertions for identity deduplication and corrupt JSONL preservation.
- Modify `benchmarks/surface_decoder_compare/README.md`: document the `--resume` workflow.

---

### Task 1: Resume-Aware Runner Behavior

**Files:**
- Modify: `rsinter/src/bench/run.rs`
- Test: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: `BenchmarkResultRow::identity()`, `read_results_jsonl`, `write_results_jsonl`, `merge_result_rows`, `RustBenchRunner::run_point`.
- Produces: `BenchRunOptions { pub resume: bool }` and `run_rust_benchmark_with_options(..., options: BenchRunOptions) -> Result<PathBuf, String>`.

- [ ] **Step 1: Write the failing regression test**

Add this test near the other `bench_run` artifact tests:

```rust
#[test]
fn rust_benchmark_run_resumes_partial_results() {
    let spec_text = r#"
name = "resume_predict_zero"
version = 1
mode = "independent"

[[runner]]
name = "predict-zero-resume"
language = "rust"
impl_key = "predict-zero"

[runner.params]
input_type = "css"
code_id = "steane"
hx = "tests/fixtures/css/steane_hx.json"
hz = "tests/fixtures/css/steane_hz.json"
basis = "x"
rounds = [1]
p = [0.0, 0.1]
max_shots = 4
max_errors = 4
batch_size = 2
control_label = "unchanged"

[plot]
title = "Resume"

[plot.x]
field = "params.p"
scale = "linear"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{{runner}}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
"#;
    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
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
    let results_path = artifact_root
        .join("predict-zero-resume")
        .join("test-run")
        .join("results.jsonl");
    let initial_rows = read_results_jsonl(&fs::read(&results_path).unwrap()[..]).unwrap();
    assert_eq!(initial_rows.len(), 2);

    let kept = initial_rows[0].clone();
    write_rows_to_path(std::slice::from_ref(&kept), &results_path);

    let artifact_root = run_rust_benchmark_with_options(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        BenchRunOptions { resume: true },
    )
    .unwrap();
    let resumed_rows = read_rows(&artifact_root, "predict-zero-resume");
    assert_eq!(resumed_rows.len(), 2);
    assert_eq!(
        identity_count(&resumed_rows, &kept.identity().unwrap()),
        1,
        "completed row identity was duplicated"
    );

    run_rust_benchmark_with_options(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        BenchRunOptions { resume: true },
    )
    .unwrap();
    let rerun_rows = read_rows(dir.path(), "predict-zero-resume");
    assert_eq!(rerun_rows.len(), 2);
    assert_same_identities(&resumed_rows, &rerun_rows);

    let changed_text = spec_text.replace(
        "control_label = \"unchanged\"",
        "control_label = \"changed\"",
    );
    let changed_spec: BenchmarkSpec = toml::from_str(&changed_text).unwrap();
    run_rust_benchmark_with_options(
        &changed_spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        BenchRunOptions { resume: true },
    )
    .unwrap();
    let changed_rows = read_rows(dir.path(), "predict-zero-resume");
    assert_eq!(changed_rows.len(), 4);

    fs::write(&results_path, b"{not valid jsonl\n").unwrap();
    let err = run_rust_benchmark_with_options(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        BenchRunOptions { resume: true },
    )
    .expect_err("corrupt existing JSONL must fail");
    assert!(err.contains("failed to read resume results"), "{err}");
    assert_eq!(fs::read(&results_path).unwrap(), b"{not valid jsonl\n");
}
```

Add these helpers near the existing test helpers:

```rust
use rsinter::bench::run::{run_rust_benchmark, run_rust_benchmark_with_options, BenchRunOptions};

fn read_rows(artifact_root: &Path, runner_name: &str) -> Vec<BenchmarkResultRow> {
    let data = fs::read(
        artifact_root
            .join(runner_name)
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    read_results_jsonl(&data[..]).unwrap()
}

fn write_rows_to_path(rows: &[BenchmarkResultRow], path: &Path) {
    let mut data = Vec::new();
    rsinter::bench::result::write_results_jsonl(rows, &mut data).unwrap();
    fs::write(path, data).unwrap();
}

fn identity_count(rows: &[BenchmarkResultRow], identity: &str) -> usize {
    rows.iter()
        .filter(|row| row.identity().unwrap() == identity)
        .count()
}

fn assert_same_identities(left: &[BenchmarkResultRow], right: &[BenchmarkResultRow]) {
    let mut left_ids: Vec<_> = left.iter().map(|row| row.identity().unwrap()).collect();
    let mut right_ids: Vec<_> = right.iter().map(|row| row.identity().unwrap()).collect();
    left_ids.sort();
    right_ids.sort();
    assert_eq!(left_ids, right_ids);
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_resumes_partial_results
```

Expected: FAIL before production changes because `BenchRunOptions` and
`run_rust_benchmark_with_options` do not exist.

- [ ] **Step 3: Add the options wrapper and resume entry point**

In `rsinter/src/bench/run.rs`, update imports:

```rust
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::bench::merge::merge_result_rows;
use crate::bench::result::{BenchmarkResultRow, RunManifest, read_results_jsonl, write_results_jsonl};
```

Add:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct BenchRunOptions {
    pub resume: bool,
}
```

Change `run_rust_benchmark` to call:

```rust
run_rust_benchmark_with_options(
    spec,
    language,
    out_root,
    registry,
    spec_dir,
    BenchRunOptions::default(),
)
```

Move the existing body into `run_rust_benchmark_with_options` with the same
arguments plus `options: BenchRunOptions`.

- [ ] **Step 4: Load resume rows before cleanup**

Inside the options-based runner, after `plan_rust_runs`, build:

```rust
let resume_rows = if options.resume {
    load_resume_rows(&planned_runs, out_root)?
} else {
    BTreeMap::new()
};
```

Implement:

```rust
fn load_resume_rows(
    planned_runs: &[PlannedRustRun<'_>],
    out_root: &Path,
) -> Result<BTreeMap<String, Vec<BenchmarkResultRow>>, String> {
    let mut rows_by_runner = BTreeMap::new();
    for planned in planned_runs {
        let path = out_root
            .join(&planned.runner.name)
            .join("test-run")
            .join("results.jsonl");
        if !path.exists() {
            continue;
        }
        let data = fs::read(&path)
            .map_err(|error| format!("failed to read resume results {}: {error}", path.display()))?;
        let rows = read_results_jsonl(&data[..])
            .map_err(|error| format!("failed to read resume results {}: {error}", path.display()))?;
        rows_by_runner.insert(planned.runner.name.clone(), rows);
    }
    Ok(rows_by_runner)
}
```

- [ ] **Step 5: Skip complete identities and merge partial rows**

For each planned runner:

```rust
let existing_rows = resume_rows
    .get(&runner.name)
    .cloned()
    .unwrap_or_default();
let completed = completed_identities(&existing_rows)?;
let mut fresh_rows = Vec::new();
for point in &points {
    let preview = runner_impl.run_point(point, &ctx)?;
    let identity = preview.identity()?;
    if !completed.contains(&identity) {
        fresh_rows.push(preview);
    }
}
let rows = if options.resume {
    merge_result_rows(vec![existing_rows, fresh_rows])?
} else {
    fresh_rows
};
```

Implement:

```rust
fn completed_identities(rows: &[BenchmarkResultRow]) -> Result<BTreeSet<String>, String> {
    let mut completed = BTreeSet::new();
    for row in rows {
        if row.status == "ok" {
            completed.insert(row.identity()?);
        }
    }
    Ok(completed)
}
```

Use the preview row as the final row when it is not skipped so each point is
executed at most once.

- [ ] **Step 6: Preserve original artifacts on resume read or merge failure**

Keep `clear_rust_run_artifacts` only for non-resume mode. In resume mode,
remove only stale `test-run.tmp` after resume rows have been loaded. Write the
manifest and merged rows to staging. After staging succeeds, remove the old
`test-run` and rename staging into place.

```rust
if !options.resume {
    clear_rust_run_artifacts(spec, language, out_root)?;
}
```

Within each runner:

```rust
if staging_dir.exists() {
    fs::remove_dir_all(&staging_dir).map_err(|e| e.to_string())?;
}
fs::create_dir_all(&staging_dir).map_err(|e| e.to_string())?;
...
if artifact_dir.exists() {
    fs::remove_dir_all(&artifact_dir).map_err(|e| e.to_string())?;
}
fs::rename(&staging_dir, &artifact_dir).map_err(|e| e.to_string())?;
```

- [ ] **Step 7: Run focused runner tests**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_resumes_partial_results
cargo test -p rsinter --test bench_run
```

Expected: both PASS.

- [ ] **Step 8: Commit Task 1**

```bash
git add rsinter/src/bench/run.rs rsinter/tests/bench_run.rs
git commit -m "feat: resume rsinter bench runs"
```

---

### Task 2: CLI Flag and Documentation

**Files:**
- Modify: `rsinter/src/bin/rsinter.rs`
- Modify: `benchmarks/surface_decoder_compare/README.md`

**Interfaces:**
- Consumes: `BenchRunOptions` and `run_rust_benchmark_with_options`.
- Produces: `rsinter bench run --resume` CLI behavior and user-facing documentation.

- [ ] **Step 1: Add the CLI flag test coverage through the integration path**

Extend `rust_benchmark_run_resumes_partial_results` or add a small CLI smoke
assertion only if the direct API test does not cover flag wiring. The direct
API test is the behavioral gate; clap help text will be covered by build.

- [ ] **Step 2: Wire clap to options**

In `rsinter/src/bin/rsinter.rs`, update imports:

```rust
use rsinter::bench::run::{BenchRunOptions, run_rust_benchmark_with_options};
```

Add `resume: bool` to `BenchCommands::Run`:

```rust
#[arg(long, help = "Resume from existing per-runner test-run/results.jsonl rows under --out")]
resume: bool,
```

Pass the option:

```rust
run_rust_benchmark_with_options(
    &bench_spec,
    &language,
    PathBuf::from(out).as_path(),
    &registry,
    &spec_dir,
    BenchRunOptions { resume },
)?;
```

- [ ] **Step 3: Document resume behavior**

In `benchmarks/surface_decoder_compare/README.md`, add:

```markdown
To resume an interrupted Rust runner artifact directory, rerun the same command
with `--resume`. Existing completed row identities in
`<out>/<runner>/test-run/results.jsonl` are preserved and skipped; missing or
incomplete identities are rerun and merged through the normal staged
`test-run.tmp` write.
```

- [ ] **Step 4: Run CLI-focused checks**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_resumes_partial_results
cargo test -p rsinter --test bench_cli
```

Expected: both PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add rsinter/src/bin/rsinter.rs benchmarks/surface_decoder_compare/README.md
git commit -m "docs: document bench run resume flag"
```
