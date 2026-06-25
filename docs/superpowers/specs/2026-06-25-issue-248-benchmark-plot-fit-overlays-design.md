# Issue 248 Benchmark Plot Fit Overlays Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: `rsinter bench plot` logical-error-rate panels

## Summary

`rsinter bench plot` will support an optional `[plot.fit]` section that draws
per-series fitted trend lines on logical-error-rate plots. The initial fit kind
is `log_log`, implemented as an unweighted least-squares line for `ln(y)` versus
`ln(x)`.

The fit uses only finite positive best points. Zero-error rows from issue #244
have no best point, so they remain interval-only observations and do not become
fake floor points in the fitted trend.

## Goals

- Parse an optional `plot.fit` section:

  ```toml
  [plot.fit]
  enabled = true
  kind = "log_log"
  ```

- Default omitted `plot.fit` to disabled, preserving existing plot output.
- Support `kind = "log_log"` for logical-error-rate panels with log x and log y
  axes.
- Fit each plotted series after `plot.series.group_by` has created the internal
  series groups.
- Skip fits with fewer than two finite positive best points, or degenerate x
  values, without panicking.
- Draw each fit over the finite positive best-point x range for that series
  with a distinct dashed line and a separate legend entry.

## Non-Goals

- Do not add automatic model selection.
- Do not interpret thresholds scientifically.
- Do not change raw benchmark results or benchmark row serialization.
- Do not add a warning subsystem just for skipped fits.
- Do not add other fit kinds in this issue.

## Current State

`BenchmarkSpec` parses plot title, logical-rate unit, x axis, series, and panels.
It has no fit configuration.

`prepare_error_rate_panel` already groups rows by the internal series key added
for issue #247. Each grouped `ErrorRatePoint` stores `best: Option<f64>`, where
zero-error rows from issue #244 keep `best = None` while intervals remain
available for rendering.

`draw_error_rate_series` draws uncertainty bands or single-point intervals,
then draws measured best lines and markers from points with `best.is_some()`.

## Design

### Spec Shape

Add `PlotFitSpec` and `PlotFitKind` in `rsinter/src/bench/spec.rs`:

```rust
pub struct PlotFitSpec {
    pub enabled: bool,
    pub kind: PlotFitKind,
}

pub enum PlotFitKind {
    LogLog,
}
```

`PlotSpec` gains `#[serde(default)] pub fit: PlotFitSpec`. The default is
disabled with `kind = LogLog`, so existing TOML remains valid and
`[plot.fit] enabled = true` has the obvious initial behavior. Serde enum
validation rejects unsupported `kind` values.

### Fit Computation

Add a small plot helper that accepts `(x, best)` pairs where `best` is
`Option<f64>`. It filters to points where both `x` and `best` are finite and
positive, computes:

```text
ln(y) = intercept + slope * ln(x)
```

using unweighted least squares, and returns `None` when there are fewer than two
valid points or when the log-x variance is zero.

The helper returns the fit slope, intercept, and the two endpoint points
evaluated at the valid x range. Production rendering and the regression test use
the same helper so the test can prove that `None` best values are ignored before
the fit.

### Panel Preparation

`prepare_error_rate_panel` will compute fits after all rows are grouped and
sorted. A fit is attempted only when all of these are true:

- `spec.plot.fit.enabled` is true;
- `spec.plot.fit.kind == PlotFitKind::LogLog`;
- the prepared panel is `metrics.logical_error_rate`;
- the x axis and y axis are both `"log"`.

For every grouped series, the helper receives that series' sorted
`ErrorRatePoint` values. A successful fit is stored on the internal series data
and its endpoint y values are included in the panel y-range input so the overlay
is visible. Skipped fits remain `None`.

### Rendering

`draw_error_rate_series` will keep drawing measured best points exactly as it
does today. When a series has a fit, it then draws a separate dashed line using
the same series color with reduced opacity and a legend label of
`"<series label> fit"`.

The fit line has no markers and never replaces the measured best series. This
keeps the overlay visibly distinct without adding a styling DSL.

### Testing

Add the requested integration test:

```bash
cargo test -p rsinter --test bench_plot plot_fit_ignores_zero_error_floor_points
```

The test will verify:

- a fixture with three finite nonzero points draws a fit overlay and legend
  entry;
- adding a zero-error interval-only point produces the same slope as fitting the
  three nonzero best points alone;
- treating the zero-error row as a fake `MIN_LOG_Y` floor point would produce a
  different slope;
- a fixture with fewer than two valid best points renders successfully and does
  not draw a fit legend entry.

Add spec parsing coverage for defaults, enabled `log_log`, and invalid fit
kinds.

## Alternatives Considered

### 1. Fit directly during drawing

Rejected. Drawing should consume prepared data. Computing fits during
preparation allows y ranges to include fit endpoints and keeps grouping behavior
centralized.

### 2. Fit every numeric log-log panel

Rejected for this issue. The requested minimum is logical error rate versus
physical error rate. Fitting every numeric metric would raise semantic questions
that are out of scope.

### 3. Add a warning path for skipped fits

Rejected. `rsinter` does not have an existing plot-warning path. The issue
allows keeping skip behavior testable without adding a new warning system, so
skips are represented by absent fit overlays.

## Verification

Run the focused regression test:

```bash
cargo test -p rsinter --test bench_plot plot_fit_ignores_zero_error_floor_points
```

Then run the broader requested verification:

```bash
cargo test
```
