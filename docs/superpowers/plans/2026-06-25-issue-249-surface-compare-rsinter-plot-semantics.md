# Issue 249 Surface Compare Rsinter Plot Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the surface-decoder comparison Python plotter use the same logical-error-rate plotting semantics as `rsinter bench plot`.

**Architecture:** A shared CSV fixture will exercise the same zero-error and nonzero-error rows from Rust and Python tests. `rsinter` remains the preferred plotting implementation; the Python script becomes a compatibility path with a local mirror of the existing `rsinter::stats::fit_binomial` per-shot logic, interval bounds for all rows, and absent best points for zero-error rows. Documentation will direct future comparison figures to `rsinter bench plot`.

**Tech Stack:** Rust 2024, `rsinter` integration tests, Python 3 unittest, matplotlib, CSV fixtures, Cargo offline-capable test commands.

## Global Constraints

- Preserve `rsinter bench plot` as the preferred plotting path.
- Keep the Python comparison plot as a compatibility path only where still needed.
- Do not plot zero-error rows as fake best points in the Python path.
- Use a configurable interval likelihood factor with default `9.0`.
- Keep Python-only visual styling changes minimal and compatibility-focused.
- Do not regenerate full benchmark artifacts.
- Do not change decoder implementations.
- Add the Rust regression test named `surface_compare_fixture_matches_rsinter_plot_semantics`.

---

## File Structure

- Create `benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv`: shared fixture consumed by both Rust and Python tests.
- Modify `rsinter/tests/bench_plot.rs`: add a test that reads the fixture, renders with default and wide interval factors, and checks marker/interval semantics.
- Modify `benchmarks/surface_decoder_compare/tests/test_plot_compare.py`: update Python tests to read the fixture and assert the same logical-rate preparation rules.
- Modify `benchmarks/surface_decoder_compare/plot_compare.py`: add local `rsinter`-matching fit preparation and render interval primitives without zero-error best points.
- Modify `benchmarks/surface_decoder_compare/README.md`: document `rsinter bench plot` as preferred for future comparison figures.

### Task 1: Shared Fixture And Failing Tests

**Files:**
- Create: `benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv`
- Modify: `rsinter/tests/bench_plot.rs`
- Modify: `benchmarks/surface_decoder_compare/tests/test_plot_compare.py`

**Interfaces:**
- Consumes: existing comparison CSV columns, `render_benchmark_plot`, `BenchmarkResultRow`, Python `render_axes`.
- Produces: a shared fixture file and tests that fail until Python logical-rate prep stops plotting zero-error best points and accepts interval factors.

- [ ] **Step 1: Add the shared fixture**

Create `benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv` with:

```csv
tier,decoder,backend,distance,rounds,p,seed,num_dets,num_obs,shots_budget,errors_budget,shots_used,logical_errors,logical_error_rate,compile_us,total_decode_us,decode_us_per_shot,status,error
smoke,rmatching,rust,3,3,0.002,12345,24,1,2000,20,2000,0,0.0,10.0,100.0,0.05,ok,
smoke,rmatching,rust,3,3,0.004,12345,24,1,2000,20,2000,2,0.001,10.0,120.0,0.06,ok,
```

- [ ] **Step 2: Add Rust fixture loader and regression test**

Add these imports to `rsinter/tests/bench_plot.rs`:

```rust
use std::path::{Path, PathBuf};
```

Add this test before helper functions:

```rust
#[test]
fn surface_compare_fixture_matches_rsinter_plot_semantics() {
    let rows = surface_compare_fixture_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].metrics["logical_errors"], 0.0);
    assert_eq!(rows[1].metrics["logical_errors"], 2.0);

    let default_spec = surface_compare_fixture_spec("");
    let default_svg = render_plot_svg(&default_spec, &rows, "surface-compare-default.svg");
    assert_eq!(
        default_svg.matches("<circle").count(),
        1,
        "shared fixture should draw only the nonzero best marker; svg was:\n{default_svg}"
    );

    let default_interval_height = target_interval_pixel_height(&default_svg);
    let wide_spec = surface_compare_fixture_spec("confidence_interval_likelihood_factor = 25.0");
    let wide_svg = render_plot_svg(&wide_spec, &rows, "surface-compare-wide.svg");
    let wide_interval_height = target_interval_pixel_height(&wide_svg);
    assert!(
        wide_interval_height > default_interval_height,
        "wider factor should produce a taller interval; default={default_interval_height}, wide={wide_interval_height}\n\
         default svg:\n{default_svg}\nwide svg:\n{wide_svg}"
    );
}
```

Add these helpers near the existing helper functions:

```rust
fn surface_compare_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv")
}

fn surface_compare_fixture_rows() -> Vec<BenchmarkResultRow> {
    let mut reader = csv::Reader::from_path(surface_compare_fixture_path()).unwrap();
    let headers = reader.headers().unwrap().clone();
    reader
        .records()
        .map(|record| {
            let record = record.unwrap();
            let field = |name: &str| -> &str {
                let index = headers.iter().position(|header| header == name).unwrap();
                record.get(index).unwrap()
            };
            let logical_errors: f64 = field("logical_errors").parse().unwrap();
            BenchmarkResultRow {
                benchmark: "surface_decoder".into(),
                runner: field("decoder").into(),
                language: "rust".into(),
                status: field("status").into(),
                failure_kind: if logical_errors > 0.0 {
                    FailureKind::LogicalFailure
                } else {
                    FailureKind::Ok
                },
                params: ParamMap::from_pairs([
                    ("distance", serde_json::json!(field("distance").parse::<u64>().unwrap())),
                    ("rounds", serde_json::json!(field("rounds").parse::<u64>().unwrap())),
                    ("p", serde_json::json!(field("p").parse::<f64>().unwrap())),
                ]),
                case_summary: CaseSummary::from_pairs([
                    ("num_dets", serde_json::json!(field("num_dets").parse::<u64>().unwrap())),
                    ("num_obs", serde_json::json!(field("num_obs").parse::<u64>().unwrap())),
                ]),
                metrics: MetricMap::from_pairs([
                    ("logical_error_rate", field("logical_error_rate").parse().unwrap()),
                    ("decode_us_per_shot", field("decode_us_per_shot").parse().unwrap()),
                    ("shots_used", field("shots_used").parse().unwrap()),
                    ("logical_errors", logical_errors),
                ]),
                artifacts: std::collections::BTreeMap::new(),
                error: None,
            }
        })
        .collect()
}

fn surface_compare_fixture_spec(plot_extra: &str) -> BenchmarkSpec {
    toml::from_str(&format!(
        r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002, 0.004]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"
{plot_extra}

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{{runner}} d={{params.distance}}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#
    ))
    .unwrap()
}
```

- [ ] **Step 3: Add Python fixture helpers and assertions**

Update the Python test imports:

```python
import math
```

Import `_logical_error_rate_fit_for_plot` from `plot_compare.py`.

Add:

```python
FIXTURE_PATH = (
    Path(__file__).parent / "fixtures" / "rsinter_plot_semantics.csv"
)


def _fixture_rows() -> list[dict[str, str]]:
    with FIXTURE_PATH.open(newline="") as handle:
        return list(csv.DictReader(handle))
```

Add this test:

```python
def test_surface_compare_fixture_matches_rsinter_plot_semantics(self) -> None:
    rows = _fixture_rows()
    zero_fit = _logical_error_rate_fit_for_plot(rows[0])
    nonzero_fit = _logical_error_rate_fit_for_plot(rows[1])

    self.assertIsNone(zero_fit.best)
    self.assertGreater(zero_fit.high, 0.0)
    self.assertEqual(nonzero_fit.best, 0.001)

    wide_fit = _logical_error_rate_fit_for_plot(
        rows[1],
        confidence_interval_likelihood_factor=25.0,
    )
    self.assertGreater(
        wide_fit.high - wide_fit.low,
        nonzero_fit.high - nonzero_fit.low,
    )

    import matplotlib.pyplot as plt

    fig, (ax_left, ax_right) = plt.subplots(1, 2)
    try:
        render_axes(ax_left, ax_right, rows)
        logical_line = ax_left.get_lines()[0]
        ydata = list(logical_line.get_ydata())
        self.assertTrue(math.isnan(ydata[0]))
        self.assertEqual(ydata[1], 0.001)
    finally:
        plt.close(fig)
```

- [ ] **Step 4: Run tests to verify RED**

Run:

```bash
cargo test -p rsinter --test bench_plot surface_compare_fixture_matches_rsinter_plot_semantics --offline
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_plot_compare -v
```

Expected before implementation: Rust may pass because `rsinter` already has the dependency issue semantics; Python fails to import or find `_logical_error_rate_fit_for_plot` until Task 2.

### Task 2: Python Compatibility Plot Semantics

**Files:**
- Modify: `benchmarks/surface_decoder_compare/plot_compare.py`
- Test: `benchmarks/surface_decoder_compare/tests/test_plot_compare.py`

**Interfaces:**
- Consumes: comparison CSV rows with `shots_used`, `logical_errors`, `p`, and `decode_us_per_shot`.
- Produces: `_logical_error_rate_fit_for_plot(row, confidence_interval_likelihood_factor=9.0) -> LogicalRateFitForPlot`.

- [ ] **Step 1: Add local fit structures and helpers**

In `benchmarks/surface_decoder_compare/plot_compare.py`, remove:

```python
from sinter import fit_binomial
```

Add:

```python
import math
from dataclasses import dataclass
```

Then add:

```python
DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR = 9.0
MIN_LOG_Y = 1e-10


@dataclass(frozen=True)
class BinomialFit:
    low: float
    best: float
    high: float


@dataclass(frozen=True)
class LogicalRateFitForPlot:
    low: float
    best: float | None
    high: float


def _log_binomial(p: float, n: int, hits: int) -> float:
    p = min(max(p, 0.0), 1.0)
    misses = n - hits
    if hits > 0 and p == 0.0:
        return float("-inf")
    if misses > 0 and p == 1.0:
        return float("-inf")
    result = 0.0
    if p > 0.0:
        result += math.log(p) * hits
    if p < 1.0:
        result += math.log(1.0 - p) * misses
    return result + math.lgamma(n + 1.0) - math.lgamma(misses + 1.0) - math.lgamma(hits + 1.0)


def _binary_search(func, min_x: int, max_x: int, target: float) -> int:
    lo = min_x
    hi = max_x
    while hi > lo + 1:
        mid = lo + (hi - lo) // 2
        value = func(mid)
        if value < target:
            lo = mid
        elif value > target:
            hi = mid
        else:
            return mid
    hi_delta = 0.0 if func(hi) == target else abs(func(hi) - target)
    lo_delta = 0.0 if func(lo) == target else abs(func(lo) - target)
    return hi if hi_delta < lo_delta else lo


def _fit_binomial(num_shots: int, num_hits: int, max_likelihood_factor: float) -> BinomialFit:
    if num_shots == 0:
        return BinomialFit(low=0.0, best=0.5, high=1.0)
    best_p = num_hits / num_shots
    log_ml = _log_binomial(best_p, num_shots, num_hits)
    target = log_ml - math.log(max_likelihood_factor)
    accuracy = 100
    denominator = accuracy * num_shots
    low = _binary_search(
        lambda expected_errors: _log_binomial(expected_errors / denominator, num_shots, num_hits),
        0,
        num_hits * accuracy,
        target,
    )
    high = _binary_search(
        lambda expected_errors: -_log_binomial(expected_errors / denominator, num_shots, num_hits),
        num_hits * accuracy,
        num_shots * accuracy,
        -target,
    )
    return BinomialFit(
        low=low / denominator,
        best=best_p,
        high=high / denominator,
    )
```

- [ ] **Step 2: Replace display-rate preparation**

Replace `_logical_error_display_rate` with:

```python
def _logical_error_rate_fit_for_plot(
    row: dict[str, str],
    confidence_interval_likelihood_factor: float = DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR,
) -> LogicalRateFitForPlot:
    shots_used = int(row["shots_used"])
    if shots_used <= 0:
        raise ValueError("shots_used must be positive")
    logical_errors = int(row["logical_errors"])
    if logical_errors < 0:
        raise ValueError("logical_errors must be non-negative")
    if logical_errors > shots_used:
        raise ValueError("logical_errors must be <= shots_used")

    fit = _fit_binomial(
        num_shots=shots_used,
        num_hits=logical_errors,
        max_likelihood_factor=confidence_interval_likelihood_factor,
    )
    return LogicalRateFitForPlot(
        low=max(fit.low, MIN_LOG_Y),
        best=None if logical_errors == 0 else max(fit.best, MIN_LOG_Y),
        high=max(fit.high, MIN_LOG_Y),
    )
```

- [ ] **Step 3: Render intervals and absent best points**

Change the logical-rate portion of `render_axes` to:

```python
fits = [_logical_error_rate_fit_for_plot(row) for row in items]
y_left = [fit.best if fit.best is not None else math.nan for fit in fits]
y_right = [float(row["decode_us_per_shot"]) for row in items]
...
ax_left.vlines(
    x,
    [fit.low for fit in fits],
    [fit.high for fit in fits],
    color=color,
    linestyle=line_style,
    linewidth=1.0,
)
ax_left.plot(
    x,
    y_left,
    color=color,
    linestyle=line_style,
    marker="o",
    label=label,
)
```

Keep the decode-time `ax_right.plot` call unchanged.

- [ ] **Step 4: Run Python tests to verify GREEN**

Run:

```bash
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_plot_compare -v
```

Expected: all Python plot tests pass without requiring `sinter`.

### Task 3: README Preferred Plot Command

**Files:**
- Modify: `benchmarks/surface_decoder_compare/README.md`

**Interfaces:**
- Consumes: existing README run sections.
- Produces: documentation that names `rsinter bench plot` as preferred and frames Python plotting as compatibility-only.

- [ ] **Step 1: Update README**

Add this paragraph after the `rsinter` framework flow:

````markdown
For future comparison figures, prefer the `rsinter bench plot` path. The
framework commands above render through the same plotter, and the direct command
shape is:

```bash
cargo run -p rsinter --bin rsinter -- bench plot --spec <benchmark.toml> --input <results.jsonl> --out <figure.svg>
```

The legacy `plot_compare.py` script is kept as a compatibility path for older
CSV comparison outputs; new benchmark figures should use `rsinter bench plot` so
zero-error intervals, interval factors, and series grouping stay aligned with
the main benchmark plotter.
````

- [ ] **Step 2: Run README assertion**

Run:

```bash
rg -n "rsinter bench plot|compatibility path" benchmarks/surface_decoder_compare/README.md
```

Expected: output contains both phrases.

### Task 4: Verification And Commit

**Files:**
- Verify all modified implementation, test, and docs files.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: committed implementation ready for PR.

- [ ] **Step 1: Run focused Rust verification**

Run:

```bash
cargo test -p rsinter --test bench_plot surface_compare_fixture_matches_rsinter_plot_semantics --offline
```

Expected: one Rust test passes.

- [ ] **Step 2: Run issue Python verification command**

If `.venv-surface-decoder/bin/python` is missing, create the venv without network and use the system-site packages available in the sandbox:

```bash
python3 -m venv --system-site-packages .venv-surface-decoder
```

Then run:

```bash
.venv-surface-decoder/bin/python -m unittest benchmarks.surface_decoder_compare.tests.test_plot_compare -v
```

Expected: all `test_plot_compare` tests pass.

- [ ] **Step 3: Run broader required Rust gate**

Run:

```bash
cargo test --offline
```

Expected: workspace Rust tests pass. If the exact required `cargo test` without `--offline` is run, it may fail before compilation in the Agent Desk sandbox because network access is restricted; record that separately and keep the offline result as the code verification.

- [ ] **Step 4: Commit implementation**

Run:

```bash
git add benchmarks/surface_decoder_compare/plot_compare.py \
  benchmarks/surface_decoder_compare/README.md \
  benchmarks/surface_decoder_compare/tests/test_plot_compare.py \
  benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv \
  rsinter/tests/bench_plot.rs \
  docs/superpowers/plans/2026-06-25-issue-249-surface-compare-rsinter-plot-semantics.md
git commit -m "fix: align surface compare plot semantics"
```
