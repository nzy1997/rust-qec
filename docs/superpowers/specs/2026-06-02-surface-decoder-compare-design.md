# Surface Decoder Compare Design

Date: 2026-06-02
Status: Proposed
Scope: Workspace benchmark harness, cross-decoder comparison workflow, and plot generation for rotated surface-code memory-X decoding

## Summary

This design adds a reproducible repository benchmark that compares six decoder
packages on the same rotated surface-code memory-X workloads and renders the
results in one figure with two side-by-side panels.

The compared decoders are:

- `rbposd`
- `rilpqec`
- `rmatching`
- `pymatching`
- `ilpqec`
- `ldpc`

The benchmark workload is fixed to:

- `stim gen surface_code:rotated_memory_x`
- `rounds = distance`
- uniform single-parameter noise `p`
- `distance in {3, 5, 7}`
- `p in {0.001, 0.002, 0.003, 0.005, 0.007, 0.010, 0.015}`

Each `(distance, p)` case uses one shared Stim-generated circuit, one shared
detector error model, and one shared deterministic shot stream. All six
decoders consume that same case data. The figure contains:

- left panel:
  `logical_error_rate vs p`
- right panel:
  `decode_time_per_shot vs p`

The benchmark is repository-owned and reproducible. It ships as scripts,
driver glue, schema, tests, documentation, and plot generation logic inside
the repo instead of as a one-off local experiment.

## Goals

- Compare the six target decoders on identical surface-code memory-X cases.
- Keep the comparison fair by sharing circuit generation, DEM generation, and
  sampled shot order across all decoders.
- Separate decoder build time from pure decode time.
- Produce repository-native benchmark artifacts:
  result tables, optional diagnostic JSON, and one comparison figure.
- Support two benchmark tiers:
  a fast `smoke` tier for pipeline validation and a larger `full` tier for the
  checked-in experiment workflow.
- Preserve enough structure that future decoder additions reuse the same
  harness instead of forking a second benchmark path.

## Non-Goals

- Do not compare more surface-code tasks in the first version.
- Do not benchmark `rotated_memory_z`, unrotated codes, repetition codes, or
  color codes in this project.
- Do not include circuit sampling time, DEM construction time, bit-packing
  conversion time, or subprocess startup time in the primary decode-time
  metric.
- Do not turn this first delivery into a general benchmark platform for every
  repository crate.
- Do not rewrite existing `rmatching` private benchmark coverage to fit this
  workflow.
- Do not promise that all full-run raw result files will be checked into git if
  their size becomes inconvenient.

## Current State

The repository already contains most of the ingredients needed for this
project, but they are spread across crate-local benchmark code and decoder
adapters instead of one shared comparison harness.

- [`rmatching/benchmarks/run_surface_dem_benchmark.py`](/Users/nzy/rcode/rstim/rmatching/benchmarks/run_surface_dem_benchmark.py:1)
  already compares `rmatching` against `pymatching` on fixed surface-code DEM
  cases.
- [`rmatching/benchmarks/surface_dem_cases.py`](/Users/nzy/rcode/rstim/rmatching/benchmarks/surface_dem_cases.py:1)
  already demonstrates the basic pattern of:
  Stim circuit -> DEM -> sampled syndromes/observables.
- [`rsinter/src/decode.rs`](/Users/nzy/rcode/rstim/rsinter/src/decode.rs:1)
  already defines the reusable decoder boundary for Rust-side DEM decoders:
  compile once from a `DetectorErrorModel`, then decode bit-packed detector
  shots.
- [`rmatching/src/decoder.rs`](/Users/nzy/rcode/rstim/rmatching/src/decoder.rs:1),
  [`rsinter/src/rbposd_adapter.rs`](/Users/nzy/rcode/rstim/rsinter/src/rbposd_adapter.rs:1),
  and [`rsinter/src/ilpqec_adapter.rs`](/Users/nzy/rcode/rstim/rsinter/src/ilpqec_adapter.rs:1)
  already show that the repository-owned Rust decoders are fed by DEM plus
  syndrome data, not by raw `.stim` circuits.
- [`rstim/src/codegen/noise_params.rs`](/Users/nzy/rcode/rstim/rstim/src/codegen/noise_params.rs:1)
  and [`rstim/src/codegen/surface_code.rs`](/Users/nzy/rcode/rstim/rstim/src/codegen/surface_code.rs:1)
  already support four-channel noise internally, while the current simple CLI
  generation path still exposes a single uniform noise knob.
- [`rilpqec/src/config.rs`](/Users/nzy/rcode/rstim/rilpqec/src/config.rs:1)
  and [`rbposd/src/config.rs`](/Users/nzy/rcode/rstim/rbposd/src/config.rs:1)
  already define solver and decoder configuration surfaces that the new
  benchmark can drive explicitly.

What is still missing is the repository-level experiment harness that:

- owns the six-decoder comparison contract
- owns the case sweep
- owns the result schema
- owns the pure-decode timing contract
- owns the plot that combines accuracy and speed in one figure

## Decision Summary

The chosen direction is:

- create a new repository-level benchmark directory dedicated to this
  experiment instead of copying `rmatching/benchmarks`
- reuse ideas and small helpers from the existing `rmatching` benchmark flow
  without making this comparison live under the `rmatching` crate directory
- use Stim as the shared circuit generator, DEM generator, and shot sampler
- feed all decoders with the same DEM and the same deterministic detector-shot
  stream
- run a Python orchestrator as the experiment entrypoint because half of the
  compared decoders are Python packages
- add a small Rust bridge for the repository-owned Rust decoders so Python can
  ask them to compile and decode without measuring cross-language overhead as
  decode time
- measure pure decode time only
- publish one figure with two side-by-side panels

## Alternatives Considered

### 1. Expand `rmatching/benchmarks` in place

This option would turn the existing `rmatching` benchmark directory into the
home of the six-decoder comparison.

Benefits:

- shortest path from the current two-decoder benchmark
- easiest way to reuse the current surface DEM scripts verbatim

Costs:

- semantically wrong ownership:
  the experiment is not `rmatching`-private
- mixes crate-local regression benchmarks with workspace-level comparison logic
- makes future additions such as `rbposd` or `rilpqec` look like attachments to
  `rmatching` instead of first-class peers

### 2. Create a new repo-level comparison harness with targeted reuse

This option adds a new benchmark directory at the repository level while
borrowing only the useful case-building and timing patterns from the current
`rmatching` scripts.

Benefits:

- clean ownership boundary for a workspace-wide experiment
- avoids copying an entire directory and letting two similar benchmark trees
  drift
- keeps existing `rmatching`-private regression workflows intact
- lets the comparison own a single result schema and plot contract

Costs:

- requires a bit more up-front structure than a copy-and-edit approach
- needs explicit bridge code for Rust decoders

This is the recommended option.

### 3. Build the whole harness around `rsinter` with Python sidecars

This option would make a Rust-first runner the primary orchestrator and bolt
the Python decoders onto it through extra adapters or files.

Benefits:

- elegant shape for the Rust decoders
- natural fit for `rmatching`, `rbposd`, and `rilpqec`

Costs:

- fights the fact that three of the six target decoders are already Python
  packages
- makes plotting, dependency management, and experiment iteration less direct
- introduces more bridge complexity than needed for this benchmark

This is not the recommended first implementation path.

## Success Criteria

The project is successful only if all of the following are true:

- the repository contains one documented command path for `smoke` and one for
  `full`
- the benchmark runs all six decoders on the shared case sweep
- the harness guarantees that every decoder for a given case sees the same DEM
  and the same shot order
- the primary accuracy metric is `logical_error_rate`
- the primary time metric is pure decoder time per consumed shot
- the output figure contains exactly two side-by-side panels:
  `logical_error_rate vs p` and `decode_time_per_shot vs p`
- the harness records actual backend choice for ILP decoders
- the repository includes tests that cover case generation, at least one driver
  smoke path, and end-to-end artifact generation
- the benchmark logic is stored in a new repo-level experiment directory, not
  by copying `rmatching/benchmarks` wholesale

## Recommended Architecture

The benchmark should be split into six focused components under one
repository-level directory.

### Component 1: Case Definitions

`cases.py` owns the benchmark contract for workload generation.

Responsibilities:

- define the fixed decoder sweep:
  `distance`, `rounds`, and `p`
- construct the Stim circuit for each case
- derive the matching DEM
- sample a deterministic public shot pool for each case
- persist or expose compact metadata needed by all drivers

The first-version workload contract is fixed to:

- task:
  `surface_code:rotated_memory_x`
- rounds rule:
  `rounds = distance`
- distances:
  `3, 5, 7`
- noise values:
  `0.001, 0.002, 0.003, 0.005, 0.007, 0.010, 0.015`

### Component 2: Shared Shot Bundle

The case layer should materialize a compact bundle representing one benchmark
case.

Suggested bundle contents:

- circuit text or generation metadata
- DEM text
- detector shots in bit-packed `b8` form
- observable truth values in bit-packed `b8` form
- metadata:
  `distance`, `rounds`, `p`, `num_dets`, `num_obs`, `max_shots`, `seed`

The important rule is not the exact file extension. The important rule is that
all decoders consume the same public shot pool instead of resampling privately.

The first implementation should generate one public pool per case sized to the
tier's `max_shots`. Each decoder then consumes a prefix of that pool until the
decoder-specific stopping rule is satisfied.

### Component 3: Decoder Drivers

The experiment directory should own one driver per compared decoder.

Suggested ownership split:

- `drivers/pymatching_driver.py`
- `drivers/ilpqec_driver.py`
- `drivers/ldpc_driver.py`
- `drivers/rust_bridge.py` plus Rust-side bridge code for:
  `rmatching`, `rbposd`, `rilpqec`

Each driver should expose one stable contract:

- `compile(case_bundle) -> compiled_decoder_handle`
- `decode_batch(compiled_decoder_handle, shots_batch) -> predictions + decode_time`

This contract makes two important decisions explicit:

- Rust decoders are still benchmarked through DEM plus syndrome inputs, not by
  passing `.stim` directly into the decoder
- compile time is collected separately from decode time

### Component 4: Runner

`run_compare.py` owns experiment execution.

Responsibilities:

- choose `smoke` or `full`
- iterate over all cases and all decoders
- compile each decoder once per case
- feed the shared shot pool in fixed batches
- stop a decoder's run when:
  `shots_used == max_shots` or `logical_errors == max_errors`
- write normalized results and optional diagnostics

The runner should not perform plotting itself.

### Component 5: Plotting

`plot_compare.py` owns figure generation from the normalized result table.

Responsibilities:

- read result CSV
- validate required columns
- filter to successful rows
- render one figure with two side-by-side axes
- save a deterministic output file path

### Component 6: Documentation And Fixtures

The experiment directory should include:

- `README.md`
- small smoke fixtures or smoke example output
- schema notes for result columns
- commands for rerunning the benchmark locally

## Benchmark Contract

This section fixes the benchmark semantics so later implementation choices do
not silently change what the figure means.

### Decoder Set

The compared decoders are exactly:

- `rbposd`
- `rilpqec`
- `rmatching`
- `pymatching`
- `ilpqec`
- `ldpc`

The first version should not include optional extra baselines or a "null"
decoder.

### Workload Contract

Every benchmark case is:

- code family:
  `surface_code`
- task:
  `rotated_memory_x`
- rounds rule:
  `rounds = distance`
- noise rule:
  one uniform scalar `p`

The first version intentionally does not expose the full four-channel
`NoiseParams` surface in the experiment contract.

### Tier Contract

Two tiers are required.

`smoke`:

- `max_shots = 2_000`
- `max_errors = 20`

`full`:

- `max_shots = 100_000`
- `max_errors = 1_000`

Both tiers use the same case grid and the same decoder set. Only the stopping
budget changes.

### Backend Policy

`rilpqec` and `ilpqec` must prefer `Gurobi` when the machine supports it.

The harness must also record the actual backend used in results, for example:

- `gurobi`
- `highs`
- `auto:gurobi`
- `auto:highs`

This prevents plots or CSV rows from hiding whether a fallback occurred.

The first version does not require the benchmark to fail purely because
`Gurobi` is absent, but it must never hide that fact.

If a decoder package does not expose explicit backend selection, the harness
must either detect and record the backend it actually used or fail fast during
startup. It must not silently label an opaque default path as a unified
`Gurobi` run.

## Data Flow

The benchmark data flow is:

1. Generate Stim circuit for one `(distance, p)` case.
2. Derive a decomposed DEM for that circuit.
3. Sample one deterministic public pool of detector shots and observable truths
   with size `tier.max_shots`.
4. Compile each decoder from that shared DEM.
5. Feed the public shot pool to each decoder in fixed batches.
6. After each batch:
   compare predictions against observable truth, update cumulative logical
   error count, and update cumulative pure decode time.
7. Stop that decoder's run when it reaches `max_shots` or `max_errors`.
8. Emit one normalized result row for that `(tier, decoder, distance, p)` run.

This design deliberately allows different decoders to stop after different
prefix lengths of the same public shot pool. That preserves the requested
dual stopping rule while keeping the underlying random sample order shared.

## Timing Contract

The primary time metric is pure decoder time.

That means the main figure must exclude:

- Stim circuit generation time
- DEM generation time
- Python/Rust bridge setup time
- data marshaling time outside the decoder's native decode call
- result CSV write time
- plot generation time

What the main time metric must include:

- the decoder's native decode call or batch decode calls applied to the shot
  batches actually consumed by that decoder

The normalized primary time column should be:

- `decode_us_per_shot = total_decode_us / shots_used`

The result table should still preserve additional secondary timing fields:

- `compile_us`
- `total_decode_us`

Those fields are report context, not the primary plotted time metric.

## Result Schema

The normalized result table should contain at least these columns:

- `tier`
- `decoder`
- `backend`
- `distance`
- `rounds`
- `p`
- `seed`
- `num_dets`
- `num_obs`
- `shots_budget`
- `errors_budget`
- `shots_used`
- `logical_errors`
- `logical_error_rate`
- `compile_us`
- `total_decode_us`
- `decode_us_per_shot`
- `status`
- `error`

Recommended optional columns:

- `batch_size`
- `dem_num_errors`
- `circuit_path`
- `dem_path`
- `notes`

One row should represent one completed or failed `(tier, decoder, distance, p)`
run.

## Plot Contract

The output figure should be one image with two side-by-side panels.

Left panel:

- x-axis:
  `p`
- y-axis:
  `logical_error_rate`
- y-scale:
  logarithmic

Right panel:

- x-axis:
  `p`
- y-axis:
  `decode_us_per_shot`
- y-scale:
  logarithmic

Style contract:

- color encodes decoder identity
- line style encodes distance
- both panels share the same color and line-style mapping

This keeps the figure readable even though it contains multiple distances and
multiple decoders in one image.

## Failure Handling

The harness should surface failures as data, not as silent omissions.

Required failure classes:

- dependency or capability error:
  package missing, solver missing, bridge binary unavailable
- compile error:
  decoder could not build from the DEM
- decode error:
  decoder failed while consuming the public shot stream
- schema or artifact error:
  output missing required fields or files

Failed rows should still be written with:

- `status != ok`
- a non-empty `error` message

The plot step should skip failed rows, but the raw result table must preserve
them.

## Testing

The first version should add three layers of test coverage.

### 1. Case Tests

Check:

- case count
- `distance` set
- `p` set
- `rounds = distance`
- deterministic shot pools for fixed seeds

### 2. Driver Smoke Tests

Check at least one small deterministic DEM path for each driver family:

- one Rust-bridge decoder path
- one Python MWPM path
- one Python ILP path
- one Python `ldpc` path

The purpose is interface confidence, not large benchmark coverage.

### 3. End-To-End Pipeline Test

Run the `smoke` tier and assert:

- result CSV exists
- required columns exist
- at least one successful row exists
- plot file exists

The first version does not need a `full` tier CI run.

## Repository Layout

The recommended directory shape is:

```text
benchmarks/
  surface_decoder_compare/
    README.md
    cases.py
    run_compare.py
    plot_compare.py
    drivers/
      __init__.py
      rust_bridge.py
      pymatching_driver.py
      ilpqec_driver.py
      ldpc_driver.py
    rust_bridge/
      ...
    results/
      smoke/
      full/
    tests/
      ...
```

This layout makes the experiment clearly repo-level while keeping existing
crate-local benchmark directories intact.

## Artifact Policy

The repository should always check in:

- the benchmark code
- the tests
- the README
- the plot script
- at least one smoke example artifact
- the final representative figure produced by the documented `full` workflow

The repository should not require large full raw tables to be checked in if
their size becomes inconvenient. The harness must support writing them, but git
check-in of those large files is optional.

## Implementation Notes

Two implementation constraints are important enough to lock now.

First, the experiment entrypoint should be Python-owned. That keeps the three
Python decoders natural and keeps plotting in the same toolchain.

Second, the Rust decoder comparison path should use a purpose-built bridge
instead of timing `cargo run` process startup as decode work. The bridge can be
a small benchmark helper binary or equivalent thin executable interface, but it
must report compile and decode timing from inside the Rust process so the main
time metric stays honest.
