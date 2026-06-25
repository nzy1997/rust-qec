# Plot Series Group By Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `plot.series.group_by` control benchmark plot series identity independently from rendered legend labels.

**Architecture:** `rsinter/src/bench/spec.rs` will default `SeriesSpec.group_by` so specs without the key still parse. `rsinter/src/bench/plot.rs` will introduce an internal `SeriesKey` built from configured row fields, store display labels separately, and use keys for grouping and style lookup. Tests will inspect generated SVG output to verify split and merge behavior through observable plotted series output.

**Tech Stack:** Rust 2024, serde TOML parsing, plotters SVG backend, rsinter integration tests.

## Global Constraints

- Preserve current behavior when `plot.series.group_by` is absent by falling back to label-based grouping.
- Keep legend labels derived from `plot.series.label_template`.
- Do not change benchmark row serialization.
- Do not add a styling DSL.
- Add a regression test named `plot_series_group_by_is_independent_from_label`.

---

## File Structure

- Modify `rsinter/src/bench/spec.rs`: add serde defaulting for `SeriesSpec.group_by`.
- Modify `rsinter/src/bench/plot.rs`: add `SeriesKey`, `SeriesData<T>`, field resolution for group keys, and key-based grouping/style lookup.
- Modify `rsinter/tests/bench_plot.rs`: add the regression test and a small SVG helper for marker colors.
- Modify `rsinter/tests/bench_spec.rs`: add parsing coverage for omitted `plot.series.group_by`.

### Task 1: Regression Tests And Parsing Default

**Files:**
- Modify: `rsinter/src/bench/spec.rs`
- Modify: `rsinter/tests/bench_spec.rs`
- Modify: `rsinter/tests/bench_plot.rs`

**Interfaces:**
- Consumes: existing `BenchmarkSpec`, `SeriesSpec`, `render_benchmark_plot`, `spec_with_panels`, and `ok_row`.
- Produces: `SeriesSpec.group_by` parses as an empty vector when omitted; failing plot regression proves labels no longer define identity.

- [ ] **Step 1: Add parsing default**

Change `SeriesSpec` in `rsinter/src/bench/spec.rs` to:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SeriesSpec {
    #[serde(default)]
    pub group_by: Vec<String>,
    pub label_template: String,
}
```

- [ ] **Step 2: Add spec parsing coverage**

Add this test to `rsinter/tests/bench_spec.rs`:

```rust
#[test]
fn benchmark_spec_allows_omitted_plot_series_group_by() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert!(spec.plot.series.group_by.is_empty());
}
```

- [ ] **Step 3: Add SVG marker color helper**

Add this helper near the bottom of `rsinter/tests/bench_plot.rs`, before `spec_with_panels`:

```rust
fn svg_circle_fill_colors(svg: &str) -> std::collections::BTreeSet<String> {
    svg.lines()
        .filter(|line| line.contains("<circle"))
        .filter_map(|line| {
            line.split("fill=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .map(str::to_string)
        })
        .collect()
}
```

- [ ] **Step 4: Add the failing group-by regression**

Add this test to `rsinter/tests/bench_plot.rs` before helper functions:

```rust
#[test]
fn plot_series_group_by_is_independent_from_label() {
    let split_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "shared label"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );
    let split_rows = vec![
        ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0),
        ok_row("predict_zero", 3, 0.002, 0.001, 2.0, 2000.0, 14.0),
    ];

    let dir = tempfile::tempdir().unwrap();
    let split_out = dir.path().join("split-same-label.svg");
    render_benchmark_plot(&split_spec, &split_rows, &split_out).unwrap();
    let split_svg = std::fs::read_to_string(split_out).unwrap();
    let split_colors = svg_circle_fill_colors(&split_svg);
    assert_eq!(
        split_colors.len(),
        2,
        "different runner group keys should remain distinct series even with the same label; svg was:\n{split_svg}"
    );

    let merge_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner} p={params.p}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );
    let merge_rows = vec![
        ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0),
        ok_row("rmatching", 3, 0.005, 0.001, 2.0, 2000.0, 14.0),
    ];

    let merge_out = dir.path().join("merge-different-labels.svg");
    render_benchmark_plot(&merge_spec, &merge_rows, &merge_out).unwrap();
    let merge_svg = std::fs::read_to_string(merge_out).unwrap();
    assert!(
        merge_svg.contains("rmatching p=0.002"),
        "merged series should keep the first rendered label; svg was:\n{merge_svg}"
    );
    assert!(
        !merge_svg.contains("rmatching p=0.005"),
        "a changed label must not split rows with the same configured group key; svg was:\n{merge_svg}"
    );
}
```

- [ ] **Step 5: Run tests to verify the regression fails before implementation**

Run:

```bash
cargo test -p rsinter --test bench_spec benchmark_spec_allows_omitted_plot_series_group_by
cargo test -p rsinter --test bench_plot plot_series_group_by_is_independent_from_label
```

Expected before implementation: the spec parsing test passes after the defaulting change; the plot regression fails because current plotting groups by rendered label.

### Task 2: Key-Based Plot Grouping

**Files:**
- Modify: `rsinter/src/bench/plot.rs`
- Test: `rsinter/tests/bench_plot.rs`

**Interfaces:**
- Consumes: `SeriesSpec.group_by`, `BenchmarkResultRow`, `render_series_label`, `value_to_string`, `metric_to_string`, and `row_context`.
- Produces: `series_key(row: &BenchmarkResultRow, spec: &BenchmarkSpec) -> Result<SeriesKey, String>` and `BTreeMap<SeriesKey, SeriesData<T>>` groups.

- [ ] **Step 1: Add series key data structures**

At the top of `rsinter/src/bench/plot.rs`, replace the current group type aliases with:

```rust
type SeriesKey = Vec<String>;
type NumericGroups = BTreeMap<SeriesKey, SeriesData<(f64, f64)>>;
type ErrorRateGroups = BTreeMap<SeriesKey, SeriesData<ErrorRatePoint>>;

struct SeriesData<T> {
    label: String,
    points: Vec<T>,
}
```

- [ ] **Step 2: Group error-rate panel rows by internal key**

In `prepare_error_rate_panel`, replace the rendered-label grouping block with:

```rust
let key = series_key(row, spec)?;
let label = render_series_label(row, spec);

groups
    .entry(key)
    .or_insert_with(|| SeriesData {
        label,
        points: Vec::new(),
    })
    .points
    .push(ErrorRatePoint { x, low, best, high });
```

Change sorting to:

```rust
for series in groups.values_mut() {
    series
        .points
        .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
}
```

- [ ] **Step 3: Group numeric panel rows by internal key**

In `prepare_numeric_panel`, replace the rendered-label grouping block with:

```rust
let key = series_key(row, spec)?;
let label = render_series_label(row, spec);
groups
    .entry(key)
    .or_insert_with(|| SeriesData {
        label,
        points: Vec::new(),
    })
    .points
    .push((x, y));
```

Change sorting to:

```rust
for series in groups.values_mut() {
    series
        .points
        .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
}
```

- [ ] **Step 4: Draw groups using key for styles and label for legend**

In `draw_error_rate_series`, iterate with `key` and `series`, resolve style by key, and use `series.points` and `series.label`:

```rust
for (index, (key, series)) in groups.iter().enumerate() {
    let style = series_styles
        .get(key)
        .copied()
        .unwrap_or_else(|| default_series_style(index));
    let points = &series.points;
    let label = &series.label;
```

Apply the same pattern in `draw_numeric_series`.

- [ ] **Step 5: Build styles by internal key**

Change `build_series_styles` to return `Result<BTreeMap<SeriesKey, SeriesStyle>, String>`, compute `let key = series_key(row, spec)?;`, and store styles under the key:

```rust
let mut styles = BTreeMap::new();
for (index, row) in rows.iter().enumerate() {
    let key = series_key(row, spec)?;
    styles.entry(key).or_insert_with(|| {
        let color_index = runner_index.get(&row.runner).copied().unwrap_or(index);
        let pattern_index = row
            .params
            .get("distance")
            .map(value_to_string)
            .and_then(|value| distance_index.get(&value).copied())
            .unwrap_or(0);
        SeriesStyle {
            color: Palette99::pick(color_index).mix(0.9),
            pattern: line_pattern_for_index(pattern_index),
        }
    });
}
Ok(styles)
```

Update `render_benchmark_plot` to call:

```rust
let series_styles = build_series_styles(spec, &ok_rows)?;
```

- [ ] **Step 6: Add group field resolution**

Add these helpers near `render_series_label`:

```rust
fn series_key(row: &BenchmarkResultRow, spec: &BenchmarkSpec) -> Result<SeriesKey, String> {
    if spec.plot.series.group_by.is_empty() {
        return Ok(vec![format!("label={}", render_series_label(row, spec))]);
    }

    spec.plot
        .series
        .group_by
        .iter()
        .map(|field| {
            resolve_series_group_field(row, field)
                .map(|value| format!("{field}={value}"))
                .ok_or_else(|| {
                    format!(
                        "missing required series group field {field} for {}",
                        row_context(row)
                    )
                })
        })
        .collect()
}

fn resolve_series_group_field(row: &BenchmarkResultRow, field: &str) -> Option<String> {
    if field == "runner" {
        return Some(row.runner.clone());
    }
    if field == "language" {
        return Some(row.language.clone());
    }

    let (scope, key) = field.split_once('.')?;
    match scope {
        "params" => row.params.get(key).map(value_to_string),
        "metrics" => row.metrics.get(key).copied().map(metric_to_string),
        "case_summary" => row.case_summary.get(key).map(value_to_string),
        _ => None,
    }
}
```

- [ ] **Step 7: Run focused verification**

Run:

```bash
cargo test -p rsinter --test bench_plot plot_series_group_by_is_independent_from_label
```

Expected: PASS.

- [ ] **Step 8: Run affected parser coverage**

Run:

```bash
cargo test -p rsinter --test bench_spec benchmark_spec_allows_omitted_plot_series_group_by
```

Expected: PASS.

- [ ] **Step 9: Commit implementation**

```bash
git add rsinter/src/bench/spec.rs rsinter/src/bench/plot.rs rsinter/tests/bench_spec.rs rsinter/tests/bench_plot.rs
git commit -m "fix: group benchmark plot series by configured fields"
```
