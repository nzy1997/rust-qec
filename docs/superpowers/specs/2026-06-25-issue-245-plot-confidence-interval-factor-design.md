# Issue 245 Plot Confidence Interval Factor Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: `rsinter bench plot` benchmark TOML parsing, validation, and logical-error-rate plotting

## Summary

`rsinter bench plot` currently computes logical-error-rate uncertainty intervals
with a hardcoded likelihood factor of `9.0`. The benchmark TOML should expose
that plotting policy so quick exploratory plots and paper-quality plots can use
different interval widths without changing code.

The change will add an optional `[plot]` field:

```toml
[plot]
confidence_interval_likelihood_factor = 9.0
```

When absent, the default remains `9.0`.

## Goals

- Parse `plot.confidence_interval_likelihood_factor` from benchmark TOML.
- Preserve the current default likelihood factor of `9.0` when the field is
  absent.
- Reject non-finite values and values below `1.0` during benchmark spec
  validation with a clear error message.
- Use the configured factor in the binomial fit for logical-error-rate plot
  intervals.
- Add a regression test showing that `25.0` produces a wider rendered interval
  than the default.
- Document the field in a checked-in surface-decoder benchmark example.

## Non-Goals

- Do not change benchmark sampling or collection stopping criteria.
- Do not add a broader sinter-compatible plotting API.
- Do not change logical-error-rate best-point behavior introduced by issue
  #244.
- Do not change numeric panels or non-logical-error-rate plotting.

## Current State

`PlotSpec` contains the plot title, x-axis spec, series spec, and panels. It has
no field for interval policy.

`prepare_error_rate_panel` calls:

```rust
fit_binomial(shots, errors, MAX_LIKELIHOOD_FACTOR)
```

where `MAX_LIKELIHOOD_FACTOR` is a private constant in
`rsinter/src/bench/plot.rs` set to `9.0`.

`BenchmarkSpec::validate` currently checks runner and panel presence, but not
plot policy values. The `bench run` CLI validates specs before use. The `bench
plot` CLI parses the spec and then renders directly.

## Design

### Spec Field

Add this field to `PlotSpec`:

```rust
#[serde(default = "default_confidence_interval_likelihood_factor")]
pub confidence_interval_likelihood_factor: f64,
```

Define the default as a public constant in `spec.rs` so tests and plotting code
can share one source of truth:

```rust
pub const DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR: f64 = 9.0;
```

This keeps old TOML files valid and preserves existing behavior.

### Validation

Add plot-policy validation to `BenchmarkSpec::validate`:

```rust
if !factor.is_finite() || factor < 1.0 {
    return Err(
        "plot confidence_interval_likelihood_factor must be finite and >= 1.0"
            .into(),
    );
}
```

The lower bound keeps the likelihood threshold meaningful. A factor of `1.0`
is allowed and represents the maximum-likelihood boundary.

The `bench plot` command should call `bench_spec.validate()?` after parsing so
invalid plot policy is rejected on the user-facing path, matching `bench run`.

### Plot Threading

Remove the local `MAX_LIKELIHOOD_FACTOR` constant from `bench/plot.rs` and pass
the configured value into `fit_binomial`:

```rust
let fit = fit_binomial(
    shots,
    errors,
    spec.plot.confidence_interval_likelihood_factor,
);
```

No other rendering behavior changes. Zero-error rows still use the issue #244
interval-only representation: interval endpoints are present and best is absent.

### Documentation

Add the field under `[plot]` in the checked-in surface-decoder benchmark spec
examples with a short TOML comment. Use `9.0` there so examples document the
setting while preserving the current behavior.

### Testing

Add the focused integration test requested by the issue:

```bash
cargo test -p rsinter --test bench_plot plot_confidence_interval_factor_is_read_from_toml
```

The test will:

- Parse a spec without the field and assert the default is `9.0`.
- Render a default plot and a `25.0` plot from otherwise identical rows, then
  assert the target logical-error-rate interval is wider for `25.0`.
- Parse invalid `0.0`, `nan`, and negative values and assert
  `BenchmarkSpec::validate` rejects them with a message mentioning
  `confidence_interval_likelihood_factor`, `finite`, and `>= 1.0`.

## Alternatives Considered

### 1. Keep the hardcoded plot constant

Rejected because the issue specifically asks for benchmark TOML configuration
and because the current behavior forces source edits for exploratory interval
policy changes.

### 2. Add the field to each logical-error-rate panel

Rejected for the first interface. The requested TOML places the value under
`[plot]`, and the policy is global to logical-error-rate interval computation
for the whole plot.

### 3. Add a larger plotting-policy table

Rejected as premature. A single optional field solves the requested use case
without committing to a broader sinter-compatible API.

### 4. Store the default only in plotting code

Rejected because TOML deserialization, tests, and plotting should agree on the
same default through `PlotSpec`.

## Verification

Run the focused regression test:

```bash
cargo test -p rsinter --test bench_plot plot_confidence_interval_factor_is_read_from_toml
```

Then run the broader requested verification:

```bash
cargo test
```
