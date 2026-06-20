# Issue 99 rbposd Benchmark Spec Coverage Design

## Objective

Add checked-in `rsinter` surface-decoder benchmark spec entries that make the
expanded `rbposd` decoder surface discoverable from shared benchmark entrypoints.
The first coverage must be narrow and stable:

- `rbposd_lsd_order1` exercises the LSD-backed runner path with
  `lsd_method = "localized_statistics"` and `lsd_order = 1`.
- `rbposd_product_sum_serial` exercises a non-default BP mode with
  `bp_method = "product_sum"` and `schedule = "serial"`.

The existing `rbposd` runner remains unchanged so historical default OSD rows
retain the same name and params.

## Context

`benchmarks/surface_decoder/spec.toml` and
`benchmarks/surface_decoder/full.toml` currently include a single Rust `rbposd`
runner using `impl_key = "rbposd"`. The runner parser already accepts LSD params,
`bp_method`, and BP schedule params, and `run_rust_benchmark` preflights each
expanded point through the selected registry runner before creating artifacts.

`schedule` has dual meaning in the benchmark surface:

- generic CSS schedule values are `greedy` and `sequential`
- `rbposd` BP schedule values are `parallel` and `serial`

For the surface-decoder specs in this issue, `schedule = "serial"` is a decoder
param and will be validated by `RbposdRunner::preflight_point`.

## Design

Update both surface-decoder benchmark specs by adding two Rust runner aliases
after the existing default `rbposd` runner:

- `rbposd_lsd_order1`, `language = "rust"`, `impl_key = "rbposd"`
- `rbposd_product_sum_serial`, `language = "rust"`, `impl_key = "rbposd"`

Each alias copies the same narrow sweep dimensions and shot limits used by the
existing `rbposd` runner in that file. This makes result rows distinguishable by
runner name while keeping the default runner untouched. The full-tier spec only
adds these two aliases, avoiding a larger runner matrix.

No generated benchmark artifacts are regenerated in this issue.

## Validation

Add focused `rsinter` tests for the checked-in specs:

- `rbposd_benchmark_specs_cover_lsd_and_bp_option_runners` parses both TOML
  files, validates the generic spec shape, locates the two required runner
  names, expands each runner through `expand_runner_points_for_runner`, and
  preflights every expanded point through the default Rust registry.
- The same test asserts representative params for the checked-in names:
  `lsd_method`, `lsd_order`, `bp_method`, and `schedule`.
- `rbposd_benchmark_specs_reject_unknown_decoder_modes` builds small mutated
  copies of valid runner params and checks that bogus `lsd_method` and
  `bp_method` values fail `RbposdRunner::preflight_point`.

These tests prove the checked-in aliases parse through the existing benchmark
registry and that invalid decoder modes do not silently fall back to defaults.

## Out Of Scope

- Plot redesign.
- Full benchmark artifact regeneration.
- Non-`rbposd` decoder spec changes.
- Changes to the `rbposd` public API or runner parser semantics.

## Verification Commands

```bash
cargo test -p rsinter rbposd_benchmark_specs_cover_lsd_and_bp_option_runners
cargo test -p rsinter rbposd_benchmark_specs_reject_unknown_decoder_modes
cargo test
```
