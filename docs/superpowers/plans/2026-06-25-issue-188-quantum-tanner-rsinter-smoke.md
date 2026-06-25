# Issue 188 Quantum Tanner Rsinter Smoke Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `rsinter` smoke test proving generated quantum Tanner `toric_d4` sparse-row CSS fixtures run through the existing CSS benchmark/decode path and that non-orthogonal CSS input is rejected by that same path.

**Architecture:** Commit the generated `toric_d4` `Hx` and `Hz` sparse-row fixtures beside the existing `rsinter` CSS fixtures, then add one focused integration test. The positive control runs `run_rust_benchmark` with the normal `input_type = "css"` path and the `rmatching` Rust runner; the negative control swaps in a deliberately non-orthogonal `Hz` file and expects `css_memory` orthogonality validation to reject it.

**Tech Stack:** Rust 2024, `rsinter` benchmark registry/run APIs, `rstim::codegen::css::css_memory`, `qec-code` sparse-row fixture format, `cargo test`.

## Global Constraints

- Keep the smoke test tiny and avoid threshold sweeps, large decoder campaigns, new decoder algorithms, qTanner/qLDPC importers, or new benchmark schema.
- Use generated `toric_d4` `Hx`/`Hz` sparse-row fixtures derived from `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`.
- Record provenance for `drafts/qLDPC/src/qldpc/codes/quantum_test.py`, `drafts/qLDPC/src/qldpc/codes/quantum.py`, and upstream `https://github.com/qLDPCOrg/qLDPC`.
- Route both the positive control and negative control through the existing `rsinter` CSS input path that builds circuits with `rstim::codegen::css::css_memory`.
- The required focused verification command is `cargo test -p rsinter quantum_tanner_toric_d4_css_fixture_smoke -q`.

---

## File Structure

- Modify `rsinter/tests/fixtures/css/README.md`: document the quantum Tanner `toric_d4` fixture provenance and regeneration commands.
- Create `rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hx.json`: generated sparse-row `Hx` fixture.
- Create `rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hz.json`: generated sparse-row `Hz` fixture.
- Create `rsinter/tests/quantum_tanner_css_fixture.rs`: focused positive and negative smoke test.

---

### Task 1: Quantum Tanner CSS Fixture Smoke Test

**Files:**
- Modify: `rsinter/tests/fixtures/css/README.md`
- Create: `rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hx.json`
- Create: `rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hz.json`
- Create: `rsinter/tests/quantum_tanner_css_fixture.rs`

**Interfaces:**
- Consumes: `rsinter::bench::run::run_rust_benchmark(spec, "rust", out_root, registry, spec_dir)`.
- Consumes: `rsinter::bench::registry::build_default_rust_runner_registry()`.
- Produces: test `quantum_tanner_toric_d4_css_fixture_smoke`.
- Produces: committed sparse-row CSS fixtures with `num_cols = 16` and eight rows each.

- [ ] **Step 1: Write the failing test file**

Create `rsinter/tests/quantum_tanner_css_fixture.rs` with this content:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{BenchmarkResultRow, read_results_jsonl};
use rsinter::bench::run::run_rust_benchmark;
use rsinter::bench::spec::{
    AxisSpec, BenchmarkMode, BenchmarkSpec, PanelSpec, PlotSpec, RunnerSpec, SeriesSpec,
};
use rsinter::failure::FailureKind;
use toml::Value;

const TORIC_D4_HX: &str = "tests/fixtures/css/quantum_tanner_toric_d4_hx.json";
const TORIC_D4_HZ: &str = "tests/fixtures/css/quantum_tanner_toric_d4_hz.json";

#[test]
fn quantum_tanner_toric_d4_css_fixture_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows = run_quantum_tanner_css_benchmark(TORIC_D4_HZ, dir.path(), spec_dir).unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.status, "ok");
    assert_eq!(row.failure_kind, FailureKind::Ok);
    assert_eq!(row.error, None);
    assert_eq!(row.params["input_type"], serde_json::json!("css"));
    assert_eq!(
        row.params["code_id"],
        serde_json::json!("quantum_tanner_toric_d4")
    );
    assert_eq!(row.params["hx"], serde_json::json!(TORIC_D4_HX));
    assert_eq!(row.params["hz"], serde_json::json!(TORIC_D4_HZ));
    assert_eq!(row.params["decoder_impl"], serde_json::json!("rmatching"));
    assert_eq!(row.case_summary["num_obs"], serde_json::json!(2));
    assert_eq!(
        row.case_summary["logical_observable_count"],
        serde_json::json!(2)
    );
    assert_eq!(row.metrics["shots_used"], 4.0);
    assert_eq!(row.metrics["logical_errors"], 0.0);

    let non_orthogonal_hz = write_non_orthogonal_hz_fixture(dir.path());
    let err = run_quantum_tanner_css_benchmark(
        &non_orthogonal_hz.display().to_string(),
        dir.path(),
        spec_dir,
    )
    .unwrap_err();
    assert!(
        err.contains("CSS X/Z checks are not orthogonal"),
        "expected CSS orthogonality rejection, got: {err}"
    );
}

fn run_quantum_tanner_css_benchmark(
    hz_path: &str,
    out_root: &Path,
    spec_dir: &Path,
) -> Result<Vec<BenchmarkResultRow>, String> {
    let registry = build_default_rust_runner_registry();
    let artifact_root = run_rust_benchmark(
        &quantum_tanner_css_spec(hz_path),
        "rust",
        out_root,
        &registry,
        spec_dir,
    )?;
    let data = fs::read(
        artifact_root
            .join("rmatching_quantum_tanner_toric_d4")
            .join("test-run")
            .join("results.jsonl"),
    )
    .map_err(|error| error.to_string())?;
    read_results_jsonl(&data[..])
}

fn quantum_tanner_css_spec(hz_path: &str) -> BenchmarkSpec {
    let mut params = BTreeMap::new();
    params.insert("input_type".into(), Value::String("css".into()));
    params.insert(
        "code_id".into(),
        Value::String("quantum_tanner_toric_d4".into()),
    );
    params.insert("hx".into(), Value::String(TORIC_D4_HX.into()));
    params.insert("hz".into(), Value::String(hz_path.into()));
    params.insert("basis".into(), Value::String("x".into()));
    params.insert("schedule".into(), Value::String("greedy".into()));
    params.insert("rounds".into(), Value::Array(vec![Value::Integer(1)]));
    params.insert("p".into(), Value::Array(vec![Value::Float(0.0)]));
    params.insert("max_shots".into(), Value::Integer(4));
    params.insert("max_errors".into(), Value::Integer(4));
    params.insert("batch_size".into(), Value::Integer(4));

    BenchmarkSpec {
        name: "quantum_tanner_toric_d4_css".into(),
        version: 1,
        mode: BenchmarkMode::Independent,
        runners: vec![RunnerSpec {
            name: "rmatching_quantum_tanner_toric_d4".into(),
            language: "rust".into(),
            impl_key: "rmatching".into(),
            params,
        }],
        plot: PlotSpec {
            title: "Quantum Tanner toric_d4 CSS Smoke".into(),
            x: AxisSpec {
                field: "params.p".into(),
                scale: "linear".into(),
                label: "Physical Error Rate".into(),
            },
            series: SeriesSpec {
                group_by: vec!["runner".into(), "params.code_id".into()],
                label_template: "{{runner}} {{params.code_id}}".into(),
            },
            panels: vec![PanelSpec {
                metric: "metrics.logical_error_rate".into(),
                scale: "linear".into(),
                label: "Logical Error Rate".into(),
            }],
        },
    }
}

fn write_non_orthogonal_hz_fixture(dir: &Path) -> PathBuf {
    let mut hz: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/css/quantum_tanner_toric_d4_hz.json"))
            .unwrap();
    hz["rows"][0] = serde_json::json!([1, 4, 5, 6]);
    let path = dir.join("quantum_tanner_toric_d4_non_orthogonal_hz.json");
    fs::write(&path, serde_json::to_string(&hz).unwrap()).unwrap();
    path
}
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test -p rsinter quantum_tanner_toric_d4_css_fixture_smoke -q
```

Expected: compilation fails because `include_str!("fixtures/css/quantum_tanner_toric_d4_hz.json")` and the positive benchmark fixture paths do not exist yet.

- [ ] **Step 3: Add generated `Hx` fixture**

Create `rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hx.json` with exactly:

```json
{"format":"sparse_rows","num_cols":16,"rows":[[0,1,2,3],[4,5,6,7],[0,4,8,10],[2,6,9,11],[8,9,12,13],[10,11,14,15],[1,5,12,14],[3,7,13,15]]}
```

This is generated by:

```bash
cargo run -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx
```

- [ ] **Step 4: Add generated `Hz` fixture**

Create `rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hz.json` with exactly:

```json
{"format":"sparse_rows","num_cols":16,"rows":[[0,1,4,5],[2,3,6,7],[0,2,8,9],[4,6,10,11],[8,10,12,14],[9,11,13,15],[1,3,12,13],[5,7,14,15]]}
```

This is generated by:

```bash
cargo run -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hz
```

- [ ] **Step 5: Document fixture provenance**

Append this section to `rsinter/tests/fixtures/css/README.md`:

~~~markdown

Quantum Tanner `toric_d4` fixtures are generated from
`qec-code/tests/fixtures/quantum_tanner/toric_d4.json`, which is the
qLDPC-derived known-answer fixture used by the `qec-code` quantum Tanner
constructor tests:

```sh
cargo run -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx > rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hx.json
cargo run -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hz > rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hz.json
```

Reference chain:

- `drafts/qLDPC/src/qldpc/codes/quantum_test.py` for the toric Tanner known-answer case.
- `drafts/qLDPC/src/qldpc/codes/quantum.py` for `QTCode`.
- Upstream `https://github.com/qLDPCOrg/qLDPC`.
~~~

- [ ] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p rsinter quantum_tanner_toric_d4_css_fixture_smoke -q
```

Expected: test passes, showing the generated fixtures run through the existing CSS benchmark/decode path and that the non-orthogonal control is rejected by CSS orthogonality validation.

- [ ] **Step 7: Run crate verification**

Run:

```bash
cargo test -p rsinter -q
```

Expected: all `rsinter` tests pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add docs/superpowers/plans/2026-06-25-issue-188-quantum-tanner-rsinter-smoke.md rsinter/tests/fixtures/css/README.md rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hx.json rsinter/tests/fixtures/css/quantum_tanner_toric_d4_hz.json rsinter/tests/quantum_tanner_css_fixture.rs
git commit -m "test: add quantum tanner rsinter smoke"
```
