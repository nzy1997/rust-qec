# Issue 244 Zero-Error Plot Interval Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: `rsinter bench plot` logical-error-rate panel data and rendering

## Summary

`rsinter bench plot` currently turns zero observed logical failures into a
finite plotted best value by clamping the fitted best estimate to `MIN_LOG_Y`.
That makes a zero-failure benchmark row look like a measured point on the
logical-error-rate curve.

The change will keep the uncertainty interval for zero-failure rows, but omit
their finite best marker and omit them from the best-value line series. Rows
with at least one logical failure will continue to draw both the best marker
and the uncertainty interval.

## Goals

- Preserve interval rendering for rows where `metrics.shots_used > 0` and
  `metrics.logical_errors == 0`.
- Avoid drawing a fake best marker at `MIN_LOG_Y` or any other log floor for
  zero-error rows.
- Keep nonzero-error rows unchanged: finite best point plus uncertainty
  interval.
- Keep log-axis safety clamping limited to interval endpoints and axis range
  inputs.
- Add a regression test that fails if zero-error best values are clamped back
  to the log floor, and fails if nonzero best markers are removed.

## Non-Goals

- Do not change benchmark sampling or decoder correctness logic.
- Do not recompute committed benchmark result artifacts.
- Do not change the public `render_benchmark_plot` API.
- Do not redesign all plotting data structures beyond the logical-error-rate
  panel internals needed for this behavior.

## Current State

`prepare_error_rate_panel` computes a binomial fit for each logical-error-rate
row. It clamps `fit.low`, `fit.best`, and `fit.high` to `MIN_LOG_Y`, then stores
all three values in `ErrorRateGroups`.

`draw_error_rate_series` maps every stored tuple into `best_points`, draws a
line through those best points, and draws a circular marker for each one. Since
`fit.best` is clamped even when zero failures were observed, zero-error rows
become visible points at the log floor.

## Design

### Data Shape

Replace the tuple alias for error-rate points with a small internal struct:

```rust
struct ErrorRatePoint {
    x: f64,
    low: f64,
    best: Option<f64>,
    high: f64,
}
```

`best` is `None` only when `logical_errors == 0`. Interval endpoints remain
plain `f64` values and continue to be clamped to `MIN_LOG_Y` so log-scale
rendering and axis bounds stay valid.

For rows where `logical_errors > 0`, `best` remains `Some(...)` and is clamped
as before for log-axis safety.

### Rendering

`draw_error_rate_series` will build the best-value line and marker input by
filtering to points where `best.is_some()`. Zero-error rows therefore still
participate in the confidence band or single-point error bar, but not in the
best-point line or marker series.

The confidence-band helper will read `x`, `low`, and `high` from every
`ErrorRatePoint`, so interval rendering remains complete even when every row in
a group has no best point.

If a group has no best points, the renderer will skip line and marker drawing
for that group while still drawing intervals. This avoids inventing a legend
entry from a non-existent best curve.

### Testing

Add the focused integration test requested by the issue:

```bash
cargo test -p rsinter --test bench_plot zero_error_logical_rate_uses_interval_without_fake_best_point
```

The test will render a logical-error-rate SVG with one zero-error row and one
nonzero-error row in separate series. It will assert that:

- The SVG contains a nonzero best marker.
- The zero-error series color is present in interval rendering.
- The zero-error series does not have a marker at the plot point that the old
  `MIN_LOG_Y` clamping behavior produced.

## Alternatives Considered

### 1. Filter zero-error rows only during rendering

This would keep the tuple type unchanged and drop zero-error best values when
building `best_points`.

Rejected because the tuple cannot represent "interval exists but best is
absent"; the fake best value is still present in prepared data and can leak
into axis bounds or future rendering paths.

### 2. Make `best` optional in the internal error-rate point

This is the chosen approach. It directly models the statistical distinction
between an uncertainty interval and a best estimate, and keeps the change local
to plot preparation and rendering.

### 3. Add a public plot-data preparation API for tests

Rejected for this issue. A public testing seam would be heavier than the
behavior change and would expand API surface without a caller need.

## Verification

Run the focused regression test:

```bash
cargo test -p rsinter --test bench_plot zero_error_logical_rate_uses_interval_without_fake_best_point
```

Then run the broader requested verification:

```bash
cargo test
```
