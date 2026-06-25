# Issue 246 Logical Rate Unit Transforms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add plot-level logical-error-rate units so benchmark plots can display per-shot, per-round, per-observable, or per-round-per-observable rates.

**Architecture:** Add a validated `LogicalRateUnit` enum to `PlotSpec`, default it to `per_shot`, and keep plot rendering routed through the existing `ErrorRatePoint` interval path from #244. Resolve required metadata per row during logical-error-rate panel preparation, transform each fit endpoint with `shot_error_rate_to_piece_error_rate` for per-piece units, and preserve absent zero-error best points.

**Tech Stack:** Rust 2024, `serde`/`toml`, `plotters`, existing `rsinter` integration tests.

## Global Constraints

- Supported `[plot].logical_rate_unit` values are exactly `per_shot`, `per_round`, `per_observable`, and `per_round_per_observable`.
- `per_shot` is the default when `[plot].logical_rate_unit` is absent.
- `per_round` uses positive numeric `params.rounds`.
- `per_observable` uses positive numeric `case_summary.logical_observable_count`, falling back to positive numeric `case_summary.num_obs`.
- `per_round_per_observable` requires both the round count and the observable count and uses their product as the piece count.
- Missing or invalid metadata for a requested transform must fail clearly with the unit, missing field, and row context.
- Transform `low`, `best`, and `high` logical-rate fit values before plotting; if `best` is absent for a zero-error row, keep it absent.
- Use `shot_error_rate_to_piece_error_rate` for per-piece transforms, not bare division.
- Do not change benchmark runner output, decoder logic, or non-logical-error-rate panel behavior.
- Required focused verification command: `cargo test -p rsinter --test bench_plot logical_rate_unit_transforms_best_and_interval_bounds`.
- Broader requested verification command: `cargo test`.

---

## File Structure

- Modify `rsinter/src/bench/spec.rs`: add `LogicalRateUnit`, default it to `PerShot`, and store it on `PlotSpec`.
- Modify `rsinter/src/bench/plot.rs`: import the enum and the shot-to-piece helper, add metadata resolution and fit transformation helpers, and use them in `prepare_error_rate_panel`.
- Modify `rsinter/tests/bench_spec.rs`: test default parsing, non-default parsing, and invalid enum rejection; update `PlotSpec` struct literals.
- Modify `rsinter/tests/bench_plot.rs`: add focused logical-rate transform coverage and update row helpers so tests can opt into rounds and observable counts.
- Modify `rsinter/tests/quantum_tanner_css_fixture.rs`: update the `PlotSpec` struct literal with the default logical-rate unit.

---

### Task 1: Spec Enum, Plot Transform, And Regression Tests

**Files:**
- Modify: `rsinter/src/bench/spec.rs`
- Modify: `rsinter/src/bench/plot.rs`
- Modify: `rsinter/tests/bench_spec.rs`
- Modify: `rsinter/tests/bench_plot.rs`
- Modify: `rsinter/tests/quantum_tanner_css_fixture.rs`

**Interfaces:**
- Consumes: existing `render_benchmark_plot(spec: &BenchmarkSpec, rows: &[BenchmarkResultRow], out: &Path) -> Result<(), String>`.
- Produces: `rsinter::bench::spec::LogicalRateUnit` with serde snake-case parsing and `Default::default() == LogicalRateUnit::PerShot`.
- Produces: unchanged `render_benchmark_plot` public signature.

- [ ] **Step 1: Write the failing spec tests**

In `rsinter/tests/bench_spec.rs`, extend the top-level import:

```rust
use rsinter::bench::spec::{
    AxisSpec, BenchmarkMode, BenchmarkSpec, LogicalRateUnit, PanelSpec, PlotSpec, SeriesSpec,
};
```

Add these tests after `benchmark_spec_loads_from_toml_fixture`:

```rust
#[test]
fn benchmark_spec_defaults_logical_rate_unit_to_per_shot() {
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
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert_eq!(spec.plot.logical_rate_unit, LogicalRateUnit::PerShot);
}

#[test]
fn benchmark_spec_parses_non_default_logical_rate_unit() {
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
logical_rate_unit = "per_round_per_observable"

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
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert_eq!(
        spec.plot.logical_rate_unit,
        LogicalRateUnit::PerRoundPerObservable
    );
}

#[test]
fn benchmark_spec_rejects_invalid_logical_rate_unit() {
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
logical_rate_unit = "per_cycle"

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
"#;

    let err = toml::from_str::<BenchmarkSpec>(text).unwrap_err();
    assert!(err.to_string().contains("per_cycle"));
}
```

Update all manual `PlotSpec { ... }` literals to include:

```rust
logical_rate_unit: LogicalRateUnit::PerShot,
```

- [ ] **Step 2: Write the failing plot transform test**

In `rsinter/tests/bench_plot.rs`, extend the imports:

```rust
use rsinter::bench::plot::{logical_rate_fit_for_plot, render_benchmark_plot};
use rsinter::bench::spec::{BenchmarkSpec, LogicalRateUnit};
use rsinter::stats::{fit_binomial, shot_error_rate_to_piece_error_rate};
```

Add this helper near the existing helpers:

```rust
fn ok_row_with_metadata(
    runner: &str,
    distance: u64,
    rounds: Option<u64>,
    p: f64,
    logical_error_rate: f64,
    logical_errors: f64,
    shots_used: f64,
    decode_us_per_shot: f64,
    logical_observable_count: Option<u64>,
    num_obs: Option<u64>,
) -> BenchmarkResultRow {
    let mut row = ok_row(
        runner,
        distance,
        p,
        logical_error_rate,
        logical_errors,
        shots_used,
        decode_us_per_shot,
    );
    if let Some(rounds) = rounds {
        row.params
            .insert("rounds".to_string(), serde_json::json!(rounds));
    }
    if let Some(count) = logical_observable_count {
        row.case_summary.insert(
            "logical_observable_count".to_string(),
            serde_json::json!(count),
        );
    }
    if let Some(count) = num_obs {
        row.case_summary
            .insert("num_obs".to_string(), serde_json::json!(count));
    }
    row
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12,
        "actual {actual} did not match expected {expected}"
    );
}
```

Add this test after `zero_error_logical_rate_uses_interval_without_fake_best_point`:

```rust
#[test]
fn logical_rate_unit_transforms_best_and_interval_bounds() {
    let row = ok_row_with_metadata(
        "rmatching",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let fit = fit_binomial(1000, 10, 9.0);
    let expected_low = fit.low.unwrap().max(1e-10);
    let expected_best = fit.best.unwrap().max(1e-10);
    let expected_high = fit.high.unwrap().max(1e-10);

    let per_shot = logical_rate_fit_for_plot(&row, LogicalRateUnit::PerShot).unwrap();
    assert_close(per_shot.low, expected_low);
    assert_close(per_shot.best.unwrap(), 0.01);
    assert_close(per_shot.best.unwrap(), expected_best);
    assert_close(per_shot.high, expected_high);

    let per_round = logical_rate_fit_for_plot(&row, LogicalRateUnit::PerRound).unwrap();
    assert_close(
        per_round.low,
        shot_error_rate_to_piece_error_rate(expected_low, 10.0).max(1e-10),
    );
    assert_close(
        per_round.best.unwrap(),
        shot_error_rate_to_piece_error_rate(0.01, 10.0).max(1e-10),
    );
    assert_close(
        per_round.high,
        shot_error_rate_to_piece_error_rate(expected_high, 10.0).max(1e-10),
    );

    let per_observable =
        logical_rate_fit_for_plot(&row, LogicalRateUnit::PerObservable).unwrap();
    assert_close(
        per_observable.best.unwrap(),
        shot_error_rate_to_piece_error_rate(0.01, 2.0).max(1e-10),
    );

    let fallback_obs_row = ok_row_with_metadata(
        "rmatching",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        None,
        Some(2),
    );
    let fallback_observable =
        logical_rate_fit_for_plot(&fallback_obs_row, LogicalRateUnit::PerObservable).unwrap();
    assert_close(fallback_observable.best.unwrap(), per_observable.best.unwrap());

    let per_round_per_observable =
        logical_rate_fit_for_plot(&row, LogicalRateUnit::PerRoundPerObservable).unwrap();
    assert_close(
        per_round_per_observable.best.unwrap(),
        shot_error_rate_to_piece_error_rate(0.01, 20.0).max(1e-10),
    );

    let zero_error_row = ok_row_with_metadata(
        "zero",
        3,
        Some(10),
        0.002,
        0.0,
        0.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let zero_fit = logical_rate_fit_for_plot(&zero_error_row, LogicalRateUnit::PerRound).unwrap();
    assert!(
        zero_fit.best.is_none(),
        "zero-error best estimate must stay absent after transform"
    );

    let per_round_spec = spec_with_logical_rate_unit("per_round");
    let dir = tempfile::tempdir().unwrap();
    let missing_rounds_row = ok_row_with_metadata(
        "missing_rounds",
        3,
        None,
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let missing_rounds_err = render_benchmark_plot(
        &per_round_spec,
        &[missing_rounds_row],
        &dir.path().join("missing-rounds.svg"),
    )
    .unwrap_err();
    assert!(missing_rounds_err.contains("logical_rate_unit = \"per_round\""));
    assert!(missing_rounds_err.contains("params.rounds"));

    let per_observable_spec = spec_with_logical_rate_unit("per_observable");
    let missing_observable_row = ok_row_with_metadata(
        "missing_observable",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        None,
        None,
    );
    let missing_observable_err = render_benchmark_plot(
        &per_observable_spec,
        &[missing_observable_row],
        &dir.path().join("missing-observable.svg"),
    )
    .unwrap_err();
    assert!(missing_observable_err.contains("logical_rate_unit = \"per_observable\""));
    assert!(missing_observable_err.contains("case_summary.logical_observable_count"));
    assert!(missing_observable_err.contains("case_summary.num_obs"));
}
```

Add this helper near `spec_with_panels`:

```rust
fn spec_with_logical_rate_unit(unit: &str) -> BenchmarkSpec {
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
p = [0.002]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"
logical_rate_unit = "{unit}"

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
    ))
    .unwrap()
}
```

- [ ] **Step 3: Run tests to verify RED**

Run:

```bash
cargo test -p rsinter --test bench_spec benchmark_spec_defaults_logical_rate_unit_to_per_shot
cargo test -p rsinter --test bench_plot logical_rate_unit_transforms_best_and_interval_bounds
```

Expected: FAIL to compile because `LogicalRateUnit` and `logical_rate_fit_for_plot` do not exist yet, proving the new tests are exercising missing behavior.

- [ ] **Step 4: Add the validated enum to `PlotSpec`**

In `rsinter/src/bench/spec.rs`, change the plot spec definitions to:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlotSpec {
    pub title: String,
    #[serde(default)]
    pub logical_rate_unit: LogicalRateUnit,
    pub x: AxisSpec,
    pub series: SeriesSpec,
    #[serde(default, rename = "panel")]
    pub panels: Vec<PanelSpec>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicalRateUnit {
    PerShot,
    PerRound,
    PerObservable,
    PerRoundPerObservable,
}

impl Default for LogicalRateUnit {
    fn default() -> Self {
        Self::PerShot
    }
}

impl LogicalRateUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PerShot => "per_shot",
            Self::PerRound => "per_round",
            Self::PerObservable => "per_observable",
            Self::PerRoundPerObservable => "per_round_per_observable",
        }
    }
}
```

- [ ] **Step 5: Add logical-rate fit transformation helpers**

In `rsinter/src/bench/plot.rs`, replace the imports:

```rust
use crate::bench::spec::{BenchmarkSpec, PanelSpec};
use crate::stats::fit_binomial;
```

with:

```rust
use crate::bench::spec::{BenchmarkSpec, LogicalRateUnit, PanelSpec};
use crate::stats::{fit_binomial, shot_error_rate_to_piece_error_rate};
```

Add this struct after `ErrorRatePoint`:

```rust
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRateFitForPlot {
    pub low: f64,
    pub best: Option<f64>,
    pub high: f64,
}
```

Add these helpers before `prepare_error_rate_panel`:

```rust
#[doc(hidden)]
pub fn logical_rate_fit_for_plot(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
) -> Result<LogicalRateFitForPlot, String> {
    let shots = required_count_metric(row, "shots_used")?;
    if shots == 0 {
        return Err(format!(
            "shots_used must be positive for {}",
            row_context(row)
        ));
    }
    let errors = required_count_metric(row, "logical_errors")?;
    if errors > shots {
        return Err(format!(
            "logical_errors must be <= shots_used for {}",
            row_context(row)
        ));
    }

    let pieces = logical_rate_pieces(row, unit)?;
    let fit = fit_binomial(shots, errors, MAX_LIKELIHOOD_FACTOR);
    let low = transform_logical_rate(fit.low.unwrap_or(0.0), pieces).max(MIN_LOG_Y);
    let best = if errors == 0 {
        None
    } else {
        Some(transform_logical_rate(fit.best.unwrap_or(0.0), pieces).max(MIN_LOG_Y))
    };
    let high = transform_logical_rate(fit.high.unwrap_or(0.0), pieces).max(MIN_LOG_Y);

    Ok(LogicalRateFitForPlot { low, best, high })
}

fn logical_rate_pieces(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
) -> Result<Option<f64>, String> {
    match unit {
        LogicalRateUnit::PerShot => Ok(None),
        LogicalRateUnit::PerRound => Ok(Some(required_positive_metadata(
            row,
            unit,
            "params.rounds",
        )?)),
        LogicalRateUnit::PerObservable => Ok(Some(required_observable_count(row, unit)?)),
        LogicalRateUnit::PerRoundPerObservable => {
            let rounds = required_positive_metadata(row, unit, "params.rounds")?;
            let observables = required_observable_count(row, unit)?;
            Ok(Some(rounds * observables))
        }
    }
}

fn required_observable_count(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
) -> Result<f64, String> {
    resolve_numeric_field(row, "case_summary.logical_observable_count")
        .or_else(|| resolve_numeric_field(row, "case_summary.num_obs"))
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            format!(
                "logical_rate_unit = \"{}\" requires positive numeric case_summary.logical_observable_count or case_summary.num_obs for {}",
                unit.as_str(),
                row_context(row)
            )
        })
}

fn required_positive_metadata(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
    field: &str,
) -> Result<f64, String> {
    resolve_numeric_field(row, field)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            format!(
                "logical_rate_unit = \"{}\" requires positive numeric {field} for {}",
                unit.as_str(),
                row_context(row)
            )
        })
}

fn transform_logical_rate(shot_rate: f64, pieces: Option<f64>) -> f64 {
    match pieces {
        Some(pieces) => shot_error_rate_to_piece_error_rate(shot_rate, pieces),
        None => shot_rate,
    }
}
```

- [ ] **Step 6: Route error-rate panel preparation through the helper**

In `prepare_error_rate_panel`, replace the manual `shots`, `errors`, and `fit` block with:

```rust
let fit = logical_rate_fit_for_plot(row, spec.plot.logical_rate_unit)?;
let label = render_series_label(row, spec);

groups.entry(label).or_default().push(ErrorRatePoint {
    x,
    low: fit.low,
    best: fit.best,
    high: fit.high,
});
x_values.push(x);
y_values.extend([fit.low, fit.high]);
if let Some(best) = fit.best {
    y_values.push(best);
}
```

- [ ] **Step 7: Run focused tests to verify GREEN**

Run:

```bash
cargo test -p rsinter --test bench_spec logical_rate_unit
cargo test -p rsinter --test bench_plot logical_rate_unit_transforms_best_and_interval_bounds
```

Expected: PASS.

- [ ] **Step 8: Run the bench plot integration tests**

Run:

```bash
cargo test -p rsinter --test bench_plot
```

Expected: PASS.

- [ ] **Step 9: Format and inspect the diff**

Run:

```bash
cargo fmt
git diff -- rsinter/src/bench/spec.rs rsinter/src/bench/plot.rs rsinter/tests/bench_spec.rs rsinter/tests/bench_plot.rs rsinter/tests/quantum_tanner_css_fixture.rs
```

Expected: formatting completes, and the diff is limited to the enum, transform helper, plot integration, and focused tests.

- [ ] **Step 10: Commit the implementation**

Run:

```bash
git add rsinter/src/bench/spec.rs rsinter/src/bench/plot.rs rsinter/tests/bench_spec.rs rsinter/tests/bench_plot.rs rsinter/tests/quantum_tanner_css_fixture.rs
git commit -m "feat: add logical rate unit transforms"
```

Expected: one implementation commit.
