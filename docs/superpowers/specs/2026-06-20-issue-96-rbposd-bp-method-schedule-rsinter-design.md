# Issue 96 rbposd BP Method and Schedule rsinter Design

## Context

`rbposd` now exposes explicit BP method and schedule choices through
`DecoderConfig`. `rsinter` can run `rbposd` benchmarks, but its runner parser
only accepts the legacy `bp_algorithm = "min_sum"` label plus iteration, OSD,
and LSD parameters. Benchmark specs cannot exercise `product_sum` or `serial`
until those keys are accepted, validated, normalized, and passed to the typed
`rbposd` config.

The existing `[runner.params]` namespace already has a generic `schedule` key
for CSS circuit generation. Existing CSS specs use `schedule = "greedy"` and
`schedule = "sequential"`, so the BP schedule parsing must not steal those
values from circuit construction.

## Approach

Use the existing `rbposd` runner parameter pipeline instead of adding a new
benchmark schema layer.

1. Extend `rsinter/src/bench/registry.rs` so `bp_method` is recognized as an
   `rbposd` decoder parameter. Treat `schedule` as a generic CSS parameter
   for `schedule = "greedy"` and `schedule = "sequential"`; otherwise route it
   to the `rbposd` decoder parser so unsupported BP schedules fail during
   preflight instead of being ignored.
2. Extend `rsinter/src/bench/runners/rbposd.rs` to parse:
   - `bp_method = "minimum_sum"` or `"product_sum"`
   - legacy `bp_algorithm = "min_sum"` as a backward-compatible alias for
     `bp_method = "minimum_sum"`
   - `schedule = "parallel"` or `"serial"`
3. Map parsed values into `rbposd::DecoderConfig` using `BpVariant` and
   `Schedule`.
4. Normalize result-row params with the upstream-facing names `bp_method` and
   `bp_schedule`, while preserving the legacy `bp_algorithm` field for existing
   consumers and tests. For points that already carry a circuit `schedule`,
   omit decoder `schedule` from the normalized map so merge order preserves the
   circuit schedule in result rows.

## Rejected Options

- Replacing `bp_algorithm` with `bp_method` outright would simplify output but
  break existing specs and row consumers that assert `bp_algorithm`.
- Treating every `schedule` key as a decoder parameter for `rbposd` would
  break existing CSS benchmarks that need `schedule = "greedy"` or
  `schedule = "sequential"` for circuit generation.
- Adding only a `bp_schedule` input key would avoid ambiguity, but it would not
  satisfy the issue's requested `schedule` surface for non-CSS rows.

## Error Handling

Parsing fails during benchmark preflight, before result artifacts are written,
when:

- `bp_method` has an unsupported value
- `schedule` is routed to the `rbposd` decoder and has an unsupported BP value
- both `bp_method` and `bp_algorithm` are set
- legacy `bp_algorithm` is set to anything except `"min_sum"`

Type errors reuse the existing helper messages such as
`bp_method must be a string` and `schedule must be a string`.

## Testing

Add focused coverage in `rsinter`:

- registry expansion carries `bp_method` and BP `schedule` to
  `BenchCasePoint.decoder_params`
- `RbposdRunner::preflight_point` accepts `bp_method = "product_sum"` with
  `schedule = "serial"`
- an `rbposd` benchmark row records normalized `bp_method` and `bp_schedule`
- CSS `rbposd` rows keep the circuit schedule in `params.schedule`
- unsupported BP method or schedule fails before stale result artifacts are
  written
- existing CSS `schedule = "greedy"` and `schedule = "sequential"` remain
  generic for CSS circuit generation

Run the issue's commands plus `cargo test -p rsinter` and the required
workspace `cargo test`.
