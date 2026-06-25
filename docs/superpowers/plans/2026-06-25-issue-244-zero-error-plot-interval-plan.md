# Issue 244 Zero-Error Plot Interval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make zero-logical-error rows render uncertainty intervals without fake finite best markers or line points.

**Architecture:** Keep the public plotting API unchanged. Internally replace the logical-error-rate tuple with a small point struct whose `best` value is optional, then filter best-line and marker rendering to points that actually have a best estimate.

**Tech Stack:** Rust 2024, `plotters`, existing `rsinter` integration tests.

## Global Constraints

- Only update `rsinter/src/bench/plot.rs` and `rsinter/tests/bench_plot.rs` for behavior and test coverage.
- Rows with `metrics.shots_used > 0` and `metrics.logical_errors == 0` contribute `low` and `high` interval endpoints for `metrics.logical_error_rate`.
- Rows with `metrics.shots_used > 0` and `metrics.logical_errors == 0` do not draw a finite best marker and do not contribute a log-floor best value to the line series.
- Rows with `metrics.logical_errors > 0` continue to draw a finite best marker and uncertainty interval.
- Keep `MIN_LOG_Y` clamping for interval endpoints and log-axis bounds, not for absent zero-error best values.
- Do not change benchmark sampling, decoder correctness logic, or committed benchmark artifacts.
- Required focused verification command: `cargo test -p rsinter --test bench_plot zero_error_logical_rate_uses_interval_without_fake_best_point`.
- Broader requested verification command: `cargo test`.

---

## File Structure

- Modify `rsinter/src/bench/plot.rs`: introduce `ErrorRatePoint`, prepare optional best values, update sorting, confidence-band construction, single-point error bars, line drawing, and marker drawing.
- Modify `rsinter/tests/bench_plot.rs`: add one integration regression test using the existing `spec_with_panels` and `ok_row` helpers.

---

### Task 1: Optional Best Values for Zero-Error Plot Points

**Files:**
- Modify: `rsinter/src/bench/plot.rs`
- Test: `rsinter/tests/bench_plot.rs`

**Interfaces:**
- Consumes: existing `render_benchmark_plot(spec: &BenchmarkSpec, rows: &[BenchmarkResultRow], out: &Path) -> Result<(), String>`.
- Produces: unchanged public API; internal `ErrorRatePoint { x: f64, low: f64, best: Option<f64>, high: f64 }` used by error-rate panel preparation and rendering.

- [ ] **Step 1: Write the failing regression test**

Add this test after `render_benchmark_plot_rejects_nonfinite_and_negative_count_metrics` in `rsinter/tests/bench_plot.rs`:

```rust
#[test]
fn zero_error_logical_rate_uses_interval_without_fake_best_point() {
    let spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "log",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    );

    let zero_error_row = ok_row("zero", 3, 0.002, 0.0, 0.0, 2000.0, 12.0);
    let nonzero_error_row = ok_row("nonzero", 3, 0.004, 0.001, 2.0, 2000.0, 12.0);
    let dir = tempfile::tempdir().unwrap();

    let combined_out = dir.path().join("zero-and-nonzero.svg");
    render_benchmark_plot(
        &spec,
        &[zero_error_row.clone(), nonzero_error_row],
        &combined_out,
    )
    .unwrap();
    let combined_svg = std::fs::read_to_string(combined_out).unwrap();
    let combined_marker_count = combined_svg.matches("<circle").count();
    assert_eq!(
        combined_marker_count, 1,
        "expected only the nonzero row to draw a best marker; svg was:\n{combined_svg}"
    );

    let zero_only_out = dir.path().join("zero-only.svg");
    render_benchmark_plot(&spec, &[zero_error_row], &zero_only_out).unwrap();
    let zero_only_svg = std::fs::read_to_string(zero_only_out).unwrap();
    assert!(zero_only_svg.contains("<svg"));
    assert_eq!(
        zero_only_svg.matches("<circle").count(),
        0,
        "zero-error interval-only row must not draw a best marker; svg was:\n{zero_only_svg}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_plot zero_error_logical_rate_uses_interval_without_fake_best_point
```

Expected: FAIL before the production change because the combined SVG has two marker circles: one real nonzero-error marker and one fake zero-error marker at the log floor.

- [ ] **Step 3: Add the internal error-rate point representation**

In `rsinter/src/bench/plot.rs`, replace:

```rust
type ErrorRateGroups = BTreeMap<String, Vec<(f64, f64, f64, f64)>>;
```

with:

```rust
type ErrorRateGroups = BTreeMap<String, Vec<ErrorRatePoint>>;

#[derive(Clone, Copy)]
struct ErrorRatePoint {
    x: f64,
    low: f64,
    best: Option<f64>,
    high: f64,
}
```

- [ ] **Step 4: Prepare optional best values**

In `prepare_error_rate_panel`, replace:

```rust
let low = fit.low.unwrap_or(0.0).max(MIN_LOG_Y);
let best = fit.best.unwrap_or(0.0).max(MIN_LOG_Y);
let high = fit.high.unwrap_or(0.0).max(MIN_LOG_Y);
let label = render_series_label(row, spec);

groups.entry(label).or_default().push((x, low, best, high));
x_values.push(x);
y_values.extend([low, best, high]);
```

with:

```rust
let low = fit.low.unwrap_or(0.0).max(MIN_LOG_Y);
let best = if errors == 0 {
    None
} else {
    Some(fit.best.unwrap_or(0.0).max(MIN_LOG_Y))
};
let high = fit.high.unwrap_or(0.0).max(MIN_LOG_Y);
let label = render_series_label(row, spec);

groups.entry(label).or_default().push(ErrorRatePoint {
    x,
    low,
    best,
    high,
});
x_values.push(x);
y_values.extend([low, high]);
if let Some(best) = best {
    y_values.push(best);
}
```

- [ ] **Step 5: Update sorting for the struct**

In `prepare_error_rate_panel`, replace:

```rust
points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
```

with:

```rust
points.sort_by(|a, b| {
    a.x.partial_cmp(&b.x)
        .unwrap_or(std::cmp::Ordering::Equal)
});
```

- [ ] **Step 6: Filter best values during line and marker rendering**

In `draw_error_rate_series`, replace:

```rust
let best_points: Vec<(f64, f64)> =
    points.iter().map(|(x, _, best, _)| (*x, *best)).collect();
draw_line_series(chart, label, &best_points, style)?;

chart
    .draw_series(
        best_points
            .iter()
            .copied()
            .map(|point| Circle::new(point, 4, ShapeStyle::from(&style.color).filled())),
    )
    .map_err(|e| e.to_string())?;
```

with:

```rust
let best_points: Vec<(f64, f64)> = points
    .iter()
    .filter_map(|point| point.best.map(|best| (point.x, best)))
    .collect();
if !best_points.is_empty() {
    draw_line_series(chart, label, &best_points, style)?;

    chart
        .draw_series(
            best_points
                .iter()
                .copied()
                .map(|point| Circle::new(point, 4, ShapeStyle::from(&style.color).filled())),
        )
        .map_err(|e| e.to_string())?;
}
```

- [ ] **Step 7: Update single-point error bars**

In `draw_error_rate_series`, replace:

```rust
let (x, low, _best, high) = points[0];
let x_lo = x / CAP_FACTOR;
let x_hi = x * CAP_FACTOR;
```

with:

```rust
let point = points[0];
let x = point.x;
let low = point.low;
let high = point.high;
let x_lo = x / CAP_FACTOR;
let x_hi = x * CAP_FACTOR;
```

- [ ] **Step 8: Update confidence band construction**

Replace:

```rust
fn confidence_band_polygon(points: &[(f64, f64, f64, f64)]) -> Vec<(f64, f64)> {
    let mut polygon = Vec::with_capacity(points.len() * 2);
    polygon.extend(points.iter().map(|(x, _low, _best, high)| (*x, *high)));
    polygon.extend(points.iter().rev().map(|(x, low, _best, _high)| (*x, *low)));
    polygon
}
```

with:

```rust
fn confidence_band_polygon(points: &[ErrorRatePoint]) -> Vec<(f64, f64)> {
    let mut polygon = Vec::with_capacity(points.len() * 2);
    polygon.extend(points.iter().map(|point| (point.x, point.high)));
    polygon.extend(points.iter().rev().map(|point| (point.x, point.low)));
    polygon
}
```

- [ ] **Step 9: Run the focused test to verify it passes**

Run:

```bash
cargo test -p rsinter --test bench_plot zero_error_logical_rate_uses_interval_without_fake_best_point
```

Expected: PASS.

- [ ] **Step 10: Run the bench plot integration tests**

Run:

```bash
cargo test -p rsinter --test bench_plot
```

Expected: PASS.

- [ ] **Step 11: Format and inspect the diff**

Run:

```bash
cargo fmt
git diff -- rsinter/src/bench/plot.rs rsinter/tests/bench_plot.rs
```

Expected: formatting completes, and the diff is limited to the optional-best representation plus the focused regression test.

- [ ] **Step 12: Commit the implementation**

Run:

```bash
git add rsinter/src/bench/plot.rs rsinter/tests/bench_plot.rs
git commit -m "fix: make zero-error plot points interval-only"
```

Expected: one implementation commit.
