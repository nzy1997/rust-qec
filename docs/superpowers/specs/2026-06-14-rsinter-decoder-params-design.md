# Rsinter Decoder Params Design

Date: 2026-06-14
Status: Draft accepted in-session, written for review
Scope: GitHub issue #47, decoder-specific benchmark parameters in `rsinter`

## Summary

Expose decoder-specific benchmark parameters in `rsinter` runner specs, apply
those parameters to the selected decoder, and record the normalized parameters
in each benchmark result row. The design keeps existing generic sweep keys
source-compatible while adding typed per-runner config parsing for `rbposd`,
`rilpqec`, and `rmatching`.

For strict issue #47 coverage, this also extends `rbposd` beyond pure OSD-0 by
adding an `osd_order` setting. `osd_order = 0` preserves current behavior, while
positive values enable a real higher-order OSD search over unreliable non-pivot
columns. This makes `rbposd` runs with `osd_order = 0` and `osd_order = 10`
meaningfully different and gives AutoQEC a real sweep knob.

## Current State

`rsinter/src/bench/registry.rs` expands `RunnerSpec.params` into
`BenchCasePoint` values using only generic benchmark keys such as `distance`,
`rounds`, `p`, `max_shots`, `max_errors`, and `batch_size`. Decoder runners
construct fixed decoders:

- `rbposd` always uses `rbposd::DecoderConfig::default()`.
- `rilpqec` always uses `IlpDemDecoder::default()`.
- `rmatching` has no tunable runner params.

`BenchmarkResultRow.params` is populated from the generated circuit source, so
the output records case parameters but not decoder settings.

The `rbposd` crate currently exposes only `OsdVariant::Osd0`; there is no
existing `osd_order` behavior to plumb through.

## Goals

This feature should:

1. Let `[runner.params]` contain decoder-specific keys in addition to generic
   sweep keys.
2. Parse decoder-specific keys through typed per-runner config structs.
3. Reject unknown decoder parameter keys with a clear validation error.
4. Apply `rbposd` settings: `bp_iters`, `max_bp_iterations`, `early_stop`, and
   `osd_order`.
5. Apply `rilpqec` settings: `backend`, `time_limit_s`, `mip_gap`, `threads`,
   and `verbose`.
6. Preserve source compatibility for existing benchmark specs.
7. Record normalized decoder parameters actually used in
   `BenchmarkResultRow.params`.
8. Add a teeth test where `rbposd.osd_order = 0` and `rbposd.osd_order = 10`
   produce different logical error rates and record their orders.

## Non-Goals

This feature should not:

1. Add decorative `rbposd` config keys for enum variants that have only one
   implemented value.
2. Add new `rmatching` behavior beyond validating that it has no tunable params.
3. Change plot semantics or require benchmark TOML files to change.
4. Change benchmark result schemas beyond adding more keys to the existing
   `params` map.
5. Optimize the higher-order OSD implementation for large-order exhaustive
   search. The implementation should be correct and bounded for benchmark
   tuning, then optimized later if needed.

## Architecture

The implementation keeps the existing benchmark layering but adds one typed
configuration boundary per runner.

`expand_runner_points` remains responsible for generic benchmark point
expansion. It should know the set of generic point keys, ignore decoder-specific
keys for point expansion, and reject keys that are neither generic nor accepted
by the target runner. If passing the runner identity into `expand_runner_points`
keeps this validation simpler, `run_rust_benchmark` can call a new
runner-aware expansion function while preserving the old helper for tests.

Each runner module owns a small config parser:

- `RbposdRunnerParams`
- `RilpqecRunnerParams`
- `RmatchingRunnerParams`

Those structs parse `toml::Value` inputs into concrete decoder configs and also
produce a normalized `serde_json` parameter map for result recording.

`run_decoder_point` should accept an additional normalized decoder-param map and
merge it into `BenchmarkResultRow.params` after `build_circuit_for_point`
returns the case params. This keeps circuit-source construction independent of
decoder configuration.

`rbposd::DecoderConfig` should gain:

```rust
pub osd_order: usize
```

The default is `0`, preserving existing behavior. `osd_order > 0` enables an
OSD-k improvement step after the existing OSD-0 residual solve.

## Input Interface

Existing specs continue to work:

```toml
[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 2000
max_errors = 20
batch_size = 256
```

Decoder-specific keys live in the same `params` table:

```toml
[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 2000
max_errors = 20
batch_size = 256
bp_iters = 50
early_stop = true
osd_order = 10
```

`bp_iters` is the issue-facing input name. `max_bp_iterations` may be accepted
as a compatibility alias for the Rust config field, but a spec must not provide
both names at once.

For `rilpqec`:

```toml
[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 200
max_errors = 20
batch_size = 16
backend = "highs"
time_limit_s = 5.0
mip_gap = 0.01
threads = 1
verbose = false
```

## Output Interface

`BenchmarkResultRow.params` should include the generic case params and the
normalized decoder params actually used. For an `rbposd` row:

```json
{
  "distance": 3,
  "rounds": 3,
  "p": 0.002,
  "max_shots": 2000,
  "max_errors": 20,
  "batch_size": 256,
  "bp_iters": 50,
  "early_stop": true,
  "osd_order": 10
}
```

Flat keys are intentional. Existing plot grouping and label templates already
understand fields such as `params.distance`, so AutoQEC and `rsinter` users can
also group by `params.osd_order` without new path syntax.

## `rbposd` OSD Order Semantics

`osd_order = 0` keeps the current algorithm:

1. Run BP.
2. If BP leaves a residual syndrome, solve the residual linear system using
   columns ordered by unreliability.
3. XOR that residual with the BP hard decision.

For `osd_order > 0`, OSD should search for lower-weight corrections by varying
the least reliable non-pivot/free columns after preparing the linear system.
The search should:

1. Use the same reliability ordering as OSD-0.
2. Identify free columns from the prepared solve basis.
3. Enumerate combinations of up to `osd_order` free columns, bounded to the
   unreliable frontier needed for practical benchmark sizes.
4. For each combination, solve the dependent pivot bits and build a correction
   satisfying the target syndrome.
5. Score candidates by channel log-likelihood cost derived from bit
   probabilities, preferring lower cost and then deterministic tie-breaks.
6. Return the best valid correction XORed with the BP hard decision.

This is real decoder behavior, not a `rsinter`-level simulation. Small focused
unit tests in `rbposd` should prove that positive order can choose a better
correction than OSD-0 on a tiny parity-check matrix.

## Validation And Errors

Validation should happen before any benchmark artifact directory is committed.

Unknown keys should name both the runner and key:

```text
unknown rbposd runner param: bogus
```

Type and range validation:

- `bp_iters`, `max_bp_iterations`, and `osd_order` must be non-negative
  integers.
- `bp_iters` and `max_bp_iterations` are aliases; using both in one runner is
  an error.
- `early_stop` and `verbose` must be booleans.
- `backend` must be `auto`, `highs`, or `gurobi`.
- `time_limit_s` must be numeric and positive if present.
- `mip_gap` must be numeric and in `[0, 1)` if present.
- `threads` must be a positive integer if present.
- `rmatching` rejects all decoder-specific keys.

Default values should be recorded in result rows only when they affect the
decoder config. For `rbposd`, record normalized `bp_iters`, `early_stop`, and
`osd_order` even when omitted so result rows are self-describing.

## Testing

Add registry/config parsing coverage:

- Existing generic params still expand unchanged.
- Decoder-specific keys do not create extra benchmark points.
- Valid `rbposd` and `rilpqec` keys parse to typed configs.
- Unknown keys fail with clear runner-specific messages.

Add runner/output coverage:

- An `rbposd` benchmark with `osd_order = 10` writes a result row where
  `params["osd_order"] == 10`.
- A bogus decoder key fails before stale artifact results remain.
- `rilpqec` params are recorded and applied to `IlpDecoderConfig`.

Add the issue teeth test:

```text
cargo test -p rsinter rbposd_osd_order_changes_ler
```

The test should run two otherwise identical `rbposd` benchmarks on the same
case and seed, one with `osd_order = 0` and one with `osd_order = 10`. It should
assert that the logical error rates differ and that each row records the
expected `osd_order`. The chosen case and shot count should be deterministic in
the Rust test suite.

Add focused `rbposd` unit tests:

- `DecoderConfig::default()` preserves `osd_order = 0`.
- A tiny OSD-k example returns a lower-cost correction than OSD-0.
- `osd_order = 0` keeps current OSD-0 outputs.

## Implementation Notes

The plan should keep changes scoped:

1. Add typed runner-param parsing and normalized result-param plumbing in
   `rsinter`.
2. Add `osd_order` config and OSD-k behavior in `rbposd`.
3. Wire `rbposd` and `rilpqec` configs into their runners.
4. Add targeted tests before broad benchmark tests.
5. Run focused package tests, then the requested teeth test.
