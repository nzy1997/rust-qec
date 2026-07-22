# Issue 518 Rsinter Feature Gates Design

Scope: GitHub issue #518, `rsinter` optional runner and plotting dependencies.

## Context

`rsinter/Cargo.toml` currently leaves `default = []` but still declares
`rmatching`, `rbposd`, `rilpqec`, and `plotters` as normal dependencies. A
`cargo build -p rsinter --no-default-features` therefore still compiles the
native ILP/HiGHS/bindgen path and the plotting/font stack. The issue requires a
minimal CSS BP+OSD build that keeps only the `rbposd` runner stack, while the
ordinary default build remains full-featured.

Issue #516 is closed and merged into `master`, and this worker branch starts at
the merge commit that tracks `Cargo.lock`, so locked verification is available.
There is no `AGENTS.md` or sibling repository instruction file in this checkout.

## Approaches Considered

### Recommended: Optional deps with stable disabled-capability shims

Make the runner and plotting crates optional in `rsinter/Cargo.toml`, define
`rbposd-runner`, `rmatching-runner`, `ilp-runner`, `plotting`, and `full`, and
set `default = ["full"]`. Keep CLI command variants and known runner identifiers
compiled in all feature combinations. Use `#[cfg(feature = "...")]` to compile
real adapters and runner modules only when their feature is enabled; otherwise
register a small disabled runner that returns `requires Cargo feature
'<feature>'` before any benchmark artifacts are written. Plot subcommands return
`requires Cargo feature 'plotting'` before reading inputs or creating outputs.

This approach directly matches the issue contract, keeps help and runner names
stable, and gives precise missing-feature errors.

### Alternative: Omit unavailable runners from the registry

This would minimize code but turn disabled capabilities into `unknown rust
runner` errors. That fails the issue requirement because a known runner would
look like a typo or could be confused with another implementation.

### Alternative: Split minimal CLI into a separate binary

A second binary could isolate dependencies, but it would duplicate CLI surface
and documentation. The issue explicitly asks for the existing `rsinter` binary
to support the feature contract.

## Design

`rsinter/Cargo.toml` defines:

- `rbposd-runner = ["dep:rbposd"]`
- `rmatching-runner = ["dep:rmatching"]`
- `ilp-runner = ["dep:rilpqec", "dep:qec-ilp-core"]`
- `plotting = ["dep:plotters"]`
- `full = ["rbposd-runner", "rmatching-runner", "ilp-runner", "plotting"]`
- `default = ["full"]`
- `gurobi = ["ilp-runner", "rilpqec/gurobi"]`

`qec-ilp-core` is included through `ilp-runner` so Cargo feature metadata names
the workspace ILP core dependency explicitly. Runtime code does not need to
import it directly.

`rsinter/src/lib.rs` gates the adapter modules. `rsinter/src/decode.rs` only
re-exports feature-enabled adapters. Tests that import those public adapters are
gated with the same feature. Core sampler, stats, benchmark spec, merge, CSV,
and predict-zero code stay feature-independent.

`rsinter/src/bench/runners/mod.rs` exposes all runner module names, but
feature-specific modules choose between the real implementation and a disabled
stub. The disabled runner implements `RustBenchRunner`, keeps the canonical
`name()`, and returns the missing feature from `preflight_point`,
`plan_point_identity`, and `run_point`. The benchmark run path already plans all
points before writing result artifacts, so a disabled runner fails without a
completed `results.jsonl`.

`rsinter/src/bin/rsinter.rs` keeps all CLI subcommands. Plotting imports and
calls are gated behind helper functions. When `plotting` is disabled, `bench
plot`, `bench plot-surface-compare-csv`, and `bench plot-bb-compare-csv` return
`requires Cargo feature 'plotting'` before opening the spec, reading inputs, or
creating parent directories. `bench run` and `bench merge` remain available
without plotting.

The existing BB circuit BP+OSD memory command imports `rbposd::OsdVariant`, so
it is part of the `rbposd-runner` capability. In a build without
`rbposd-runner`, the command name stays in help and immediately returns
`requires Cargo feature 'rbposd-runner'`.

Two Steane benchmark fixtures are added:

- `rsinter/tests/fixtures/bench/minimal_steane_css_rbposd.toml`
- `rsinter/tests/fixtures/bench/minimal_steane_css_rilpqec.toml`

Both use the committed Steane CSS matrices under `rsinter/tests/fixtures/css/`.
The `rbposd` fixture is the minimal positive smoke path. The `rilpqec` fixture
is the disabled-runner negative control.

## Tests And CI

Add Rust tests that prove:

- the default registry still exposes `rmatching`, `rbposd`, `rilpqec`, and
  `predict-zero`;
- in a minimal `rbposd-runner` build, the `rilpqec` runner reports `requires
  Cargo feature 'ilp-runner'` before writing results;
- in a minimal `rbposd-runner` build, plotting commands report `requires Cargo
  feature 'plotting'` before reading missing inputs or writing outputs; and
- the committed Steane `rbposd` fixture runs and emits exactly one successful
  CSS row.

Gate existing runner and plotting tests with Cargo feature requirements so
minimal feature builds compile. CI keeps the locked full workspace test and adds
a minimal `rsinter` build/smoke job matching the issue's dependency graph check.

## Documentation

Document the `rsinter` feature matrix and two common build commands in the
surface-decoder benchmark documentation:

```bash
cargo build --locked -p rsinter
cargo build --locked -p rsinter --no-default-features --features rbposd-runner
```

The docs must state that default builds enable `full`, and that minimal
`rbposd-runner` builds keep CSS benchmark running while excluding
`rmatching`, `rilpqec`, `qec-ilp-core`, HiGHS, and `plotters` from the normal
and build dependency graph.

## Validation

Required verification is exactly the issue's three groups:

- minimal positive path with `cargo build --locked -p rsinter
  --no-default-features --features rbposd-runner`, one Steane `rbposd` row, and
  a forbidden-dependency `cargo tree` check;
- disabled ILP and plotting negative controls with exact missing-feature
  messages and no completed artifacts; and
- full/default compatibility with locked `rsinter` build, ILP test, and plot
  CLI test.

The broader repository gate is `cargo test` as requested by Agent Desk.

## Automatic Approval

Because this is a non-interactive Agent Desk run, the issue body and Standing
Answer Policy approve this design. The selected approach is the recommended
conservative option because it preserves current CLI and runner identity
surface while removing optional dependency stacks from minimal builds.
