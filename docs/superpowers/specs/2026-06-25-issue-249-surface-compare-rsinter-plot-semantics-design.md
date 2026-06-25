# Issue 249 Surface Compare Rsinter Plot Semantics Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: Surface-decoder comparison plotting compatibility path, shared fixture coverage, and documentation

## Summary

The surface-decoder comparison Python plotter still prepares logical-error-rate
points independently from `rsinter bench plot`. The Rust plotter already carries
the semantics introduced by issues #244, #245, and #247: zero-logical-error rows
produce uncertainty intervals without best markers, interval width is controlled
by `plot.confidence_interval_likelihood_factor`, and series identity comes from
explicit grouping fields.

This change keeps `rsinter bench plot` as the preferred plotting path and narrows
the Python plotter to a compatibility path whose logical-rate preparation follows
the same statistical rules.

## Goals

- Add a small shared surface-compare fixture with one zero-error row and one
  nonzero-error row.
- Exercise that fixture from both `rsinter/tests/bench_plot.rs` and
  `benchmarks/surface_decoder_compare/tests/test_plot_compare.py`.
- Update `benchmarks/surface_decoder_compare/plot_compare.py` so zero-error rows
  carry interval bounds but no plotted best point.
- Thread a configurable interval likelihood factor through the Python logical
  rate preparation path, defaulting to the `rsinter` value of `9.0`.
- Keep Python visual changes minimal: retain the existing comparison figure
  styling while adding interval primitives and omitting zero-error best markers.
- Document `rsinter bench plot` as the preferred command for future comparison
  figures in `benchmarks/surface_decoder_compare/README.md`.

## Non-Goals

- Do not delete the Python plotter.
- Do not regenerate committed full benchmark artifacts.
- Do not redesign the comparison figure.
- Do not change decoder implementations, benchmark sampling, or result schemas.
- Do not introduce a new Python dependency just for compatibility plotting.

## Current State

`rsinter/src/bench/plot.rs` prepares logical-error-rate points through
`logical_rate_fit_for_plot_with_factor`. It computes binomial intervals from
`shots_used` and `logical_errors`, clamps interval endpoints for log-axis safety,
and sets `best = None` when `logical_errors == 0`. Plot rendering draws interval
primitives for those rows but skips marker and line points for the missing best
value.

`benchmarks/surface_decoder_compare/plot_compare.py` currently imports
`sinter.fit_binomial` and converts zero-error rows into a positive display rate
using the upper interval bound. The plotting loop then includes that surrogate in
the marker and line series. That makes a zero-error row look like a measured
best point.

The comparison README already mentions the `rsinter` framework flow, but it does
not explicitly name `rsinter bench plot` as the preferred plotting command for
future figures.

## Design

### Shared Fixture

Add `benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv`
with two `ok` rows in the existing comparison CSV schema:

- `rmatching`, `distance = 3`, `p = 0.002`, `shots_used = 2000`,
  `logical_errors = 0`, `logical_error_rate = 0.0`.
- `rmatching`, `distance = 3`, `p = 0.004`, `shots_used = 2000`,
  `logical_errors = 2`, `logical_error_rate = 0.001`.

Both Rust and Python tests will read this file directly. Keeping it in the
benchmark test fixture directory avoids duplicating row literals across
languages.

### Python Logical-Rate Preparation

Replace the Python plotter's direct `sinter.fit_binomial` dependency with a
small local implementation of the same algorithm used by `rsinter::stats`:

- `log_binomial(p, n, hits)` using `math.lgamma`.
- Integer binary search over expected-error counts with accuracy `100`.
- `fit_binomial(num_shots, num_hits, max_likelihood_factor)` returning
  `low`, `best`, and `high`.

Then add `_logical_error_rate_fit_for_plot(row, factor)` that mirrors the Rust
plotter for the per-shot comparison CSV path:

- Require positive `shots_used`.
- Require `0 <= logical_errors <= shots_used`.
- Compute interval bounds from counts and the factor.
- Clamp `low` and `high` to `1e-10` for log-axis safety.
- Return `best = None` for zero-error rows.
- Return `best = errors / shots_used` for nonzero rows.

This path intentionally uses counts instead of trusting the stored
`logical_error_rate` field, matching `rsinter bench plot`.

### Python Rendering

Keep the existing grouping, colors, line styles, axes, labels, and legend.
Within each series:

- Draw vertical interval lines on the logical-error-rate axis for all rows.
- Plot best markers and line segments only for rows whose fit has `best`.
- Use `NaN` in the best series for zero-error rows so matplotlib neither draws a
  marker nor connects through the missing point.

Decode-time plotting remains unchanged.

### Rust Test Coverage

Add `surface_compare_fixture_matches_rsinter_plot_semantics` to
`rsinter/tests/bench_plot.rs`. The test will read the shared CSV fixture, render
the logical-error-rate panel, and assert:

- The zero-error row does not create a best marker.
- The nonzero row does create a best marker.
- A wider `confidence_interval_likelihood_factor` produces a taller interval
  for the fixture than the default factor.

### Python Test Coverage

Update `benchmarks/surface_decoder_compare/tests/test_plot_compare.py` to read
the same fixture and assert:

- `_logical_error_rate_fit_for_plot` returns `best is None` for the zero-error
  row and a finite best value for the nonzero row.
- Increasing the factor widens the nonzero interval.
- `render_axes` leaves a `NaN` in the logical-error-rate best series for the
  zero-error row and retains the nonzero best marker.

### Documentation

Update `benchmarks/surface_decoder_compare/README.md` so future comparison
figures are directed to the `rsinter` flow and its `rsinter bench plot` command.
The older Python plot remains documented as a compatibility path for the legacy
CSV comparison output.

## Alternatives Considered

### 1. Shell out from Python to `rsinter bench plot`

Rejected for this compatibility path. It would require translating legacy CSV
rows to benchmark JSONL and locating a benchmark TOML spec from the Python
script. That would make the old script larger and more fragile while the README
can instead direct new work to the native `rsinter` command.

### 2. Keep using `sinter.fit_binomial`

Rejected because the issue asks the comparison plot to inherit `rsinter`
semantics. The current Python path already demonstrates the risk: it computes an
upper bound but then plots it as a best point. A small local mirror of the
existing Rust fit logic keeps tests deterministic and avoids requiring `sinter`
for plotting unit tests.

### 3. Delete the Python plotter

Rejected as out of scope. Existing comparison CSV artifacts and tests still use
the Python path, so this PR keeps it compatible while naming `rsinter bench plot`
as preferred.

## Verification

Run the focused regression commands from the issue:

```bash
cargo test -p rsinter --test bench_plot surface_compare_fixture_matches_rsinter_plot_semantics
.venv-surface-decoder/bin/python -m unittest benchmarks.surface_decoder_compare.tests.test_plot_compare -v
```

Then run the broader required gate:

```bash
cargo test
```
