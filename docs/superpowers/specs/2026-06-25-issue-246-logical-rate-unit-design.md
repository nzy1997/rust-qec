# Issue 246 Logical Rate Unit Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: `rsinter bench plot` logical-error-rate panel preparation

## Summary

`rsinter bench plot` currently renders `metrics.logical_error_rate` as a
per-shot logical failure rate. The plotter will gain a small validated
`[plot]` setting that can display the same fitted logical-error-rate points per
shot, per round, per observable, or per round per observable.

The benchmark runner and result row format stay unchanged. The plotter will
derive the required normalization metadata from existing row fields:
`params.rounds` for rounds and `case_summary.logical_observable_count`, falling
back to `case_summary.num_obs`, for observables.

## Goals

- Add `logical_rate_unit = "per_shot"` to `[plot]`, defaulting to `per_shot`
  when the key is absent.
- Support `per_shot`, `per_round`, `per_observable`, and
  `per_round_per_observable`.
- Reject unknown `logical_rate_unit` values during TOML parsing.
- Fail clearly when a requested transform needs metadata missing from a row.
- Apply the selected transform to `low`, `best`, and `high` logical-rate fit
  values before rendering.
- Preserve the #244 behavior where zero-logical-error rows keep interval bounds
  but have no finite `best` marker.
- Use the existing `shot_error_rate_to_piece_error_rate` helper for per-piece
  conversions instead of bare division.

## Non-Goals

- Do not add user-provided callbacks or expression evaluation.
- Do not change how benchmark rows record `metrics.logical_error_rate`.
- Do not change benchmark sampling or decoder correctness logic.
- Do not alter non-logical-error-rate panels.

## Current State

`prepare_error_rate_panel` computes a binomial fit from `shots_used` and
`logical_errors`, then stores an internal `ErrorRatePoint` containing `low`,
`best: Option<f64>`, and `high`. The optional `best` field comes from #244 and
allows zero-error rows to render intervals without fake log-floor markers.

`PlotSpec` has no logical-rate display unit. TOML parsing accepts only the
current plot fields: title, x axis, series, and panels.

## Design

### TOML Interface

Add a validated enum to the plot spec:

```toml
[plot]
title = "Surface Decoder"
logical_rate_unit = "per_round"
```

The Rust shape is:

```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicalRateUnit {
    PerShot,
    PerRound,
    PerObservable,
    PerRoundPerObservable,
}
```

`PlotSpec` stores `logical_rate_unit: LogicalRateUnit` with
`#[serde(default)]`, and `LogicalRateUnit::default()` returns `PerShot`.

### Metadata Resolution

For each ok row plotted in a logical-error-rate panel:

- `per_shot` needs no extra metadata.
- `per_round` requires a positive numeric `params.rounds`.
- `per_observable` requires a positive numeric
  `case_summary.logical_observable_count`, falling back to
  `case_summary.num_obs`.
- `per_round_per_observable` requires both metadata sources and multiplies them
  into one positive piece count.

Errors include the requested unit, missing field, and row context so a user can
fix the benchmark rows or choose a compatible unit.

### Transform Semantics

The transform runs after the binomial fit produces per-shot interval values and
before values enter `ErrorRatePoint`:

```rust
let transformed = logical_rate_unit.transform_rate(per_shot_rate, row)?;
```

For per-piece units, `transform_rate` uses:

```rust
shot_error_rate_to_piece_error_rate(per_shot_rate, pieces)
```

where `pieces` is rounds, observables, or `rounds * observables`.

Log rendering still receives positive values. For zero-error lower bounds,
`0.0` transforms to `0.0` and is then clamped to `MIN_LOG_Y`, matching the
existing log-axis safety behavior. A missing zero-error `best` remains missing
instead of being transformed into a synthetic point.

### Testing

Add a focused integration test:

```bash
cargo test -p rsinter --test bench_plot logical_rate_unit_transforms_best_and_interval_bounds
```

The test will render SVGs and assert that a row with `logical_errors = 10`,
`shots_used = 1000`, `rounds = 10`, and two observables plots:

- `per_shot` near `0.01`;
- `per_round` near `0.001`;
- `per_observable` near `0.005`;
- `per_round_per_observable` near `0.0005`.

It will also assert that missing `params.rounds` fails for `per_round`, and
that missing both observable-count fields fails for `per_observable`.

Spec parsing tests will verify the default `per_shot` behavior, parsing of
non-default enum values, and rejection of invalid enum strings.

## Alternatives Considered

### 1. Keep `logical_rate_unit` as a string

This would minimize type changes but defer validation until plotting. Rejected
because the issue asks for a validated enum, and invalid plot specs should fail
at TOML parse time.

### 2. Add per-panel logical-rate units

This would allow multiple logical-rate panels in different units in one figure.
Rejected as out of scope because the requested interface is plot-level
`[plot].logical_rate_unit`, and the current benchmark plot use cases have one
logical-error-rate unit per output figure.

### 3. Use a plot-level enum and row-local metadata resolution

This is the chosen approach. It keeps the feature small, keeps benchmark rows
unchanged, and reuses the existing #244 `ErrorRatePoint` path so best values
and interval bounds are transformed consistently.

## Verification

Run the focused regression test:

```bash
cargo test -p rsinter --test bench_plot logical_rate_unit_transforms_best_and_interval_bounds
```

Then run the broader requested verification:

```bash
cargo test
```
