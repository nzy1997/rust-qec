# Issue 245 Plot Confidence Interval Factor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow benchmark TOML `[plot]` configuration to control the likelihood factor used for logical-error-rate confidence intervals.

**Architecture:** Store the plotting policy on `PlotSpec` with a serde default of `9.0`, validate it through `BenchmarkSpec::validate`, and pass it directly to `fit_binomial` when preparing logical-error-rate panels. Keep the public rendering API unchanged and document the field in surface-decoder benchmark specs.

**Tech Stack:** Rust 2024, `serde`, `toml`, `plotters`, existing `rsinter` integration tests.

## Global Constraints

- The TOML interface is `[plot] confidence_interval_likelihood_factor = 9.0`.
- The default must preserve current behavior when the field is absent: `9.0`.
- The value must be finite and greater than or equal to `1.0`.
- Invalid values such as `0.0`, `nan`, and negative numbers fail spec validation with a clear error.
- Logical-error-rate panels must pass the configured factor into the binomial fit call.
- A fixture TOML with `confidence_interval_likelihood_factor = 25.0` must produce wider plotted intervals than a default fixture.
- Do not change benchmark collection stopping criteria.
- Do not add a full sinter-compatible plotting API.
- Required focused verification command: `cargo test -p rsinter --test bench_plot plot_confidence_interval_factor_is_read_from_toml`.
- Broader requested verification command: `cargo test`.

---

## File Structure

- Modify `rsinter/src/bench/spec.rs`: add the default constant, serde-defaulted `PlotSpec` field, default helper, and validation.
- Modify `rsinter/src/bench/plot.rs`: remove the local hardcoded likelihood-factor constant and use `spec.plot.confidence_interval_likelihood_factor`.
- Modify `rsinter/src/bin/rsinter.rs`: validate specs in `bench plot` before rendering so invalid TOML fails on the CLI path.
- Modify `rsinter/tests/bench_plot.rs`: add the requested focused regression test plus SVG interval parsing helpers.
- Modify `rsinter/tests/bench_spec.rs` and `rsinter/tests/quantum_tanner_css_fixture.rs`: update manual `PlotSpec` construction with the default constant.
- Modify `benchmarks/surface_decoder/spec.toml` and `benchmarks/surface_decoder/full.toml`: document the new `[plot]` field in checked-in benchmark examples using the default value.

---

### Task 1: TOML Confidence Interval Factor

**Files:**
- Modify: `rsinter/src/bench/spec.rs`
- Modify: `rsinter/src/bench/plot.rs`
- Modify: `rsinter/src/bin/rsinter.rs`
- Modify: `rsinter/tests/bench_plot.rs`
- Modify: `rsinter/tests/bench_spec.rs`
- Modify: `rsinter/tests/quantum_tanner_css_fixture.rs`
- Modify: `benchmarks/surface_decoder/spec.toml`
- Modify: `benchmarks/surface_decoder/full.toml`

**Interfaces:**
- Consumes: benchmark TOML `[plot] confidence_interval_likelihood_factor = <float>`.
- Produces: `pub const DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR: f64 = 9.0` and `PlotSpec::confidence_interval_likelihood_factor: f64`.
- Produces: unchanged `render_benchmark_plot(spec: &BenchmarkSpec, rows: &[BenchmarkResultRow], out: &Path) -> Result<(), String>` public API.

- [ ] **Step 1: Write the failing regression test**

In `rsinter/tests/bench_plot.rs`, replace the spec import:

```rust
use rsinter::bench::spec::BenchmarkSpec;
```

with:

```rust
use rsinter::bench::spec::{BenchmarkSpec, DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR};
```

Add this test after `zero_error_logical_rate_uses_interval_without_fake_best_point`:

```rust
#[test]
fn plot_confidence_interval_factor_is_read_from_toml() {
    let default_spec: BenchmarkSpec =
        toml::from_str(&confidence_interval_factor_spec_text("")).unwrap();
    default_spec.validate().unwrap();
    assert_eq!(
        default_spec.plot.confidence_interval_likelihood_factor,
        DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR
    );

    let wide_spec: BenchmarkSpec = toml::from_str(&confidence_interval_factor_spec_text(
        "confidence_interval_likelihood_factor = 25.0",
    ))
    .unwrap();
    wide_spec.validate().unwrap();
    assert_eq!(wide_spec.plot.confidence_interval_likelihood_factor, 25.0);

    let rows = confidence_interval_probe_rows();
    let default_svg = render_plot_svg(&default_spec, &rows, "default-factor.svg");
    let wide_svg = render_plot_svg(&wide_spec, &rows, "wide-factor.svg");
    let default_interval_height = target_interval_pixel_height(&default_svg);
    let wide_interval_height = target_interval_pixel_height(&wide_svg);
    assert!(
        wide_interval_height > default_interval_height,
        "factor 25.0 should produce a wider target interval than the default; \
         default={default_interval_height}, wide={wide_interval_height}\n\
         default svg:\n{default_svg}\nwide svg:\n{wide_svg}"
    );

    for invalid in ["0.0", "nan", "-2.0"] {
        let spec: BenchmarkSpec = toml::from_str(&confidence_interval_factor_spec_text(&format!(
            "confidence_interval_likelihood_factor = {invalid}"
        )))
        .unwrap();
        let err = spec
            .validate()
            .expect_err("invalid confidence interval factor should fail validation");
        assert!(
            err.contains("confidence_interval_likelihood_factor"),
            "error should name the invalid field, got: {err}"
        );
        assert!(
            err.contains("finite") && err.contains(">= 1.0"),
            "error should explain the finite >= 1.0 constraint, got: {err}"
        );
    }
}
```

Add these helpers near the existing `spec_with_panels` helper:

```rust
fn confidence_interval_factor_spec_text(plot_extra: &str) -> String {
    format!(
        r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "zero-anchor"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.001]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[[runner]]
name = "target"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.01]
rounds = [3]
max_shots = 1000
max_errors = 10
batch_size = 256

[[runner]]
name = "one-anchor"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.1]
rounds = [3]
max_shots = 2000
max_errors = 2000
batch_size = 256

[plot]
title = "Surface Decoder"
{plot_extra}

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#
    )
}

fn confidence_interval_probe_rows() -> Vec<BenchmarkResultRow> {
    vec![
        ok_row("zero-anchor", 3, 0.001, 0.0, 0.0, 2000.0, 12.0),
        ok_row("target", 3, 0.01, 0.01, 10.0, 1000.0, 12.0),
        ok_row("one-anchor", 3, 0.1, 1.0, 2000.0, 2000.0, 12.0),
    ]
}

fn render_plot_svg(spec: &BenchmarkSpec, rows: &[BenchmarkResultRow], file_name: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join(file_name);
    render_benchmark_plot(spec, rows, &out).unwrap();
    std::fs::read_to_string(out).unwrap()
}

#[derive(Debug)]
struct VerticalIntervalSegment {
    x: f64,
    height: f64,
}

fn target_interval_pixel_height(svg: &str) -> f64 {
    let mut segments: Vec<_> = svg
        .lines()
        .filter(|line| line.contains("<polyline"))
        .filter(|line| line.contains("fill=\"none\""))
        .filter(|line| line.contains("stroke-width=\"1\""))
        .filter(|line| line.contains("stroke=\"#") && !line.contains("stroke=\"#000000\""))
        .filter_map(parse_vertical_interval_segment)
        .collect();
    segments.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert!(
        segments.len() >= 3,
        "expected at least three single-point interval segments, got {segments:?}\nsvg:\n{svg}"
    );
    segments[segments.len() / 2].height
}

fn parse_vertical_interval_segment(line: &str) -> Option<VerticalIntervalSegment> {
    let points = svg_attribute(line, "points")?;
    let mut points = points.split_whitespace().filter_map(parse_svg_point);
    let first = points.next()?;
    let second = points.next()?;
    if points.next().is_some() {
        return None;
    }
    if (first.0 - second.0).abs() > 1e-6 {
        return None;
    }
    let height = (first.1 - second.1).abs();
    if height <= 0.0 {
        return None;
    }
    Some(VerticalIntervalSegment { x: first.0, height })
}

fn svg_attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=\"");
    let start = line.find(&prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn parse_svg_point(point: &str) -> Option<(f64, f64)> {
    let (x, y) = point.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_plot plot_confidence_interval_factor_is_read_from_toml
```

Expected: FAIL before production changes because `DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR` and `PlotSpec::confidence_interval_likelihood_factor` do not exist yet.

- [ ] **Step 3: Add the spec field, default, and validation**

In `rsinter/src/bench/spec.rs`, add this constant after the imports:

```rust
pub const DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR: f64 = 9.0;
```

In `BenchmarkSpec::validate`, after the empty-panel check, add:

```rust
        if !self.plot.confidence_interval_likelihood_factor.is_finite()
            || self.plot.confidence_interval_likelihood_factor < 1.0
        {
            return Err(
                "plot confidence_interval_likelihood_factor must be finite and >= 1.0".into(),
            );
        }
```

In `PlotSpec`, add the field after `title`:

```rust
    #[serde(default = "default_confidence_interval_likelihood_factor")]
    pub confidence_interval_likelihood_factor: f64,
```

Add this helper after `PlotSpec`:

```rust
fn default_confidence_interval_likelihood_factor() -> f64 {
    DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR
}
```

- [ ] **Step 4: Thread the factor into plotting**

In `rsinter/src/bench/plot.rs`, delete:

```rust
const MAX_LIKELIHOOD_FACTOR: f64 = 9.0;
```

Replace:

```rust
let fit = fit_binomial(shots, errors, MAX_LIKELIHOOD_FACTOR);
```

with:

```rust
let fit = fit_binomial(
    shots,
    errors,
    spec.plot.confidence_interval_likelihood_factor,
);
```

- [ ] **Step 5: Validate specs on the bench plot CLI path**

In `rsinter/src/bin/rsinter.rs`, in the `BenchCommands::Plot` branch after parsing `bench_spec`, add:

```rust
                bench_spec.validate()?;
```

The branch should mirror the existing `bench run` parse-and-validate sequence before reading result rows.

- [ ] **Step 6: Update manual `PlotSpec` construction**

In `rsinter/tests/bench_spec.rs`, replace the import:

```rust
    AxisSpec, BenchmarkMode, BenchmarkSpec, PanelSpec, PlotSpec, SeriesSpec,
```

with:

```rust
    AxisSpec, BenchmarkMode, BenchmarkSpec, PanelSpec, PlotSpec, SeriesSpec,
    DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR,
```

In the manual `PlotSpec` literal, add:

```rust
            confidence_interval_likelihood_factor: DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR,
```

after `title`.

In `rsinter/tests/quantum_tanner_css_fixture.rs`, update the spec import in the same way and add the same field to its manual `PlotSpec` literal.

- [ ] **Step 7: Document the setting in surface-decoder benchmark examples**

In both `benchmarks/surface_decoder/spec.toml` and `benchmarks/surface_decoder/full.toml`, under:

```toml
[plot]
title = "Surface Decoder"
```

add:

```toml
# Likelihood factor used for logical-error-rate confidence intervals.
confidence_interval_likelihood_factor = 9.0
```

- [ ] **Step 8: Run the focused test to verify it passes**

Run:

```bash
cargo test -p rsinter --test bench_plot plot_confidence_interval_factor_is_read_from_toml
```

Expected: PASS.

- [ ] **Step 9: Run adjacent rsinter tests**

Run:

```bash
cargo test -p rsinter --test bench_plot
cargo test -p rsinter --test bench_spec
cargo test -p rsinter --test bench_specs
```

Expected: all PASS.

- [ ] **Step 10: Format and inspect the diff**

Run:

```bash
cargo fmt
git diff -- rsinter/src/bench/spec.rs rsinter/src/bench/plot.rs rsinter/src/bin/rsinter.rs rsinter/tests/bench_plot.rs rsinter/tests/bench_spec.rs rsinter/tests/quantum_tanner_css_fixture.rs benchmarks/surface_decoder/spec.toml benchmarks/surface_decoder/full.toml
```

Expected: formatting completes, and the diff is limited to the spec field, validation, factor threading, tests, and example TOML documentation.

- [ ] **Step 11: Commit the implementation**

Run:

```bash
git add rsinter/src/bench/spec.rs rsinter/src/bench/plot.rs rsinter/src/bin/rsinter.rs rsinter/tests/bench_plot.rs rsinter/tests/bench_spec.rs rsinter/tests/quantum_tanner_css_fixture.rs benchmarks/surface_decoder/spec.toml benchmarks/surface_decoder/full.toml
git commit -m "feat: expose plot confidence interval factor"
```

Expected: one implementation commit.
