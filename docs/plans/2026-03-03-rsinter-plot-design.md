# rsinter Plot Design

**Date:** 2026-03-03
**Feature:** Logical error rate vs physical error rate visualization
**Crate:** `rsinter`

## Goal

Add a `plot_error_rate` function to `rsinter` that renders simulation results as a publication-quality plot: physical error rate on the x-axis, logical error rate on the y-axis, with error bars and per-group curves.

## API

```rust
pub fn plot_error_rate(
    stats: &[TaskStats],
    x_func: impl Fn(&TaskStats) -> f64,
    group_func: impl Fn(&TaskStats) -> String,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>>
```

- `x_func` — extracts the physical error rate from metadata, e.g. `|s| s.metadata["p"].as_f64().unwrap()`
- `group_func` — extracts the group label, e.g. `|s| s.metadata["d"].to_string()`
- `output` — file path; extension determines format: `.svg` → SVGBackend, `.png` → BitMapBackend

## Architecture

- New module: `rsinter/src/plot.rs`, exposed from `rsinter/src/lib.rs`
- Add `plotters` to `rsinter/Cargo.toml`
- Uses existing `fit_binomial()` from `rsinter/src/stats.rs` for confidence intervals

## Data Flow

1. Group `TaskStats` by `group_func` result
2. For each group, compute `(x, Fit)` pairs:
   - `x = x_func(stat)`
   - `Fit { low, best, high } = fit_binomial(stat.shots - stat.discards, stat.errors, 9.0)`
3. Sort each group's points by x
4. Render with plotters:
   - Line through `best` values
   - Vertical error bars from `low` to `high`

## Plot Styling

- **Canvas**: 800×600 pixels
- **X-axis**: linear scale, label `"Physical Error Rate"`
- **Y-axis**: log scale, label `"Logical Error Rate"`
- **Error bars**: vertical, same color as curve
- **Colors**: `Palette99`, cycled per group
- **Legend**: top-left, one entry per group
- No grid lines, no title

## Output Formats

| Extension | Backend |
|-----------|---------|
| `.svg` | `SVGBackend` (default) |
| `.png` | `BitMapBackend` |
