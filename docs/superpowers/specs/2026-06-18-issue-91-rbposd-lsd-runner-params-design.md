# Issue 91 Rbposd LSD Runner Params Design

Date: 2026-06-18
Status: Proposed
Scope: GitHub issue #91, `rsinter` `rbposd` runner parameter parsing and typed LSD parameter normalization

## Summary

Issue #91 adds the `rsinter` parser bridge needed before benchmark specs can
name the new `rbposd` LSD decoder family.

`rbposd` already exposes `BpLsdDecoder`, `LsdConfig`, and
`LsdMethod::LocalizedStatistics`. It supports `lsd_order = 0` and
`lsd_order = 1`, while `lsd_order > 1` remains unsupported. `rsinter` does not
yet know how to accept or normalize LSD-specific runner params, and its current
`rbposd` runner always produces a `DecoderConfig` for the OSD-backed DEM
adapter.

This issue should keep the boundary narrow: accept, validate, and normalize
`lsd_method` and `lsd_order` into a typed runner config, preserve existing OSD
source compatibility, and fail clearly if someone tries to execute the LSD DEM
path before #92 adds the adapter.

## Goals

- Accept `lsd_method` and `lsd_order` as known `rbposd` decoder params in
  `rsinter` runner specs.
- Preserve the current OSD default when LSD params are omitted.
- Normalize OSD and LSD params into an explicit typed `RbposdRunnerParams`
  shape instead of a flat `DecoderConfig` only.
- Reject unknown, ill-typed, unsupported, or contradictory decoder params during
  preflight, before artifacts are written.
- Keep result-row normalization ready for #93 without making #91 write LSD
  result rows.
- Leave actual LSD-backed DEM decoding to #92.

## Non-Goals

- Do not add an LSD DEM adapter in #91.
- Do not run LSD-backed benchmarks end to end.
- Do not record LSD params in successful result rows yet; #93 owns result-row
  recording once #92 can execute the path.
- Do not update smoke or full benchmark specs.
- Do not add BP method or schedule expansion; #94 through #97 own that work.
- Do not add new public `rbposd` LSD methods or algorithm behavior.

## Current Context

#88 and PR #106 added the public `BpLsdDecoder`, `LsdConfig`, and
`LsdMethod` surface in `rbposd`.

#89 and PR #108 added deterministic `lsd_order = 1` behavior, kept
`lsd_order = 0`, and made `lsd_order > 1` return
`DecodeError::UnsupportedLsdOrder`. The PR review also fixed LSD candidate
cost ordering so lexicographic tie-breaking applies only to exactly equal
weighted costs. `rsinter` should pass typed config through later, not reinterpret
these algorithm rules.

#90 and PR #109 added LSD fixture metadata and an opt-in parity harness path
for the current LSD fixture set. That remains `rbposd`-side validation and does
not wire LSD into `rsinter` benchmark execution.

#103 and PR #107 recently extended the `rsinter` `rbposd` runner with
paper-facing OSD labels:

- `bp_algorithm = "min_sum"`
- `bp_iters` / `max_bp_iterations`
- `early_stop`
- `osd_method = "combination_sweep"`
- `osd_order`

Issue #91 should follow this parser and provenance style instead of introducing
a separate stringly typed path.

## Alternatives Considered

### 1. Typed Parser Bridge Only

Accept LSD params, validate them, normalize them into an explicit decoder-family
enum, and return a clear execution error if an LSD run reaches `run_point`
before #92 exists.

Benefits:

- Matches the issue split: #91 parses, #92 decodes, #93 records successful LSD
  result rows.
- Prevents silent fallback to the OSD path.
- Gives #92 a typed handoff point.
- Keeps tests fast and focused.

Cost:

- A spec with LSD params can pass preflight but cannot yet produce a successful
  benchmark artifact.

This is the chosen approach.

### 2. Allow Zero-Shot LSD Benchmark Artifacts

Accept LSD params and allow `max_shots = 0` runs to write result rows before the
LSD DEM adapter exists.

Benefits:

- Makes smoke fixtures appear runnable earlier.

Costs:

- Creates benchmark artifacts for a decoder path that cannot actually decode
  DEMs yet.
- Risks confusing #93's normalized-result-row contract.
- Blurs the boundary between parser support and execution support.

This is rejected.

### 3. Fold The LSD DEM Adapter Into #91

Implement typed params and the #92 adapter in one larger issue.

Benefits:

- Produces end-to-end LSD benchmark execution sooner.

Costs:

- Violates the current milestone split.
- Mixes parser errors, DEM-lowering behavior, and decode behavior in one diff.
- Increases review and test scope.

This is rejected.

## Architecture

### Runner Registry

Extend `rsinter/src/bench/registry.rs` so the `rbposd` decoder-param key list
also accepts:

- `lsd_method`
- `lsd_order`

These keys should be carried in `BenchCasePoint.decoder_params`, not multiplied
into independent benchmark points. Unknown keys should keep the existing error
shape:

```text
unknown rbposd runner param: <key>
```

This ensures typos fail in `plan_rust_runs` during preflight, before
`run_rust_benchmark` writes staging or final artifact directories.

### Typed Runner Params

Refactor `rsinter/src/bench/runners/rbposd.rs` so
`RbposdRunnerParams::parse` produces a typed decoder family instead of only a
`DecoderConfig`.

Keep the shape private to the runner:

```rust
struct RbposdRunnerParams {
    bp_config: DecoderConfig,
    decoder: RbposdDecoderFamily,
    normalized: ParamMap,
}

enum RbposdDecoderFamily {
    Osd {
        osd_method: String,
        osd_order: usize,
    },
    Lsd {
        lsd_method: String,
        lsd_order: usize,
        lsd_config: rbposd::LsdConfig,
    },
}
```

This boundary is explicit: OSD and LSD are separate families, and later adapter
code can switch on the family without inspecting raw TOML values.

`bp_config` should keep the shared BP settings used by both families:

- `max_bp_iterations`, from `bp_iters` or `max_bp_iterations`
- `early_stop`
- current default BP variant and schedule

This mirrors `BpLsdDecoder` today, which still uses the default BP path.

### Default OSD Compatibility

If neither `lsd_method` nor `lsd_order` is present, parsing should preserve the
current OSD behavior:

- `bp_algorithm` defaults to `"min_sum"`
- `bp_iters` defaults to `DecoderConfig::default().max_bp_iterations`
- `early_stop` defaults to `DecoderConfig::default().early_stop`
- `osd_method` defaults to `"combination_sweep"`
- `osd_order` defaults to `DecoderConfig::default().osd_order`

Existing benchmark specs that omit LSD params must keep running through
`RbposdDemDecoder::new(bp_config)`.

### LSD Parsing Semantics

Setting either `lsd_method` or `lsd_order` selects the LSD family.

Accepted LSD values:

- `lsd_method = "localized_statistics"`
- `lsd_order = 0`
- `lsd_order = 1`

If `lsd_method` is omitted but `lsd_order` is present, use
`"localized_statistics"` as the normalized method. If `lsd_order` is omitted
but `lsd_method` is present, use `LsdConfig::default().lsd_order`, currently
`0`.

Unsupported or ill-typed values should fail during preflight:

```text
rbposd lsd_method must be "localized_statistics", got "<value>"
rbposd lsd_order must be <= 1, got <value>
lsd_order must be an integer
lsd_order must be non-negative
```

The `localized_statistics` spelling is intentionally flat and issue-facing.
It maps to `rbposd::LsdMethod::LocalizedStatistics`.

### Contradictory Params

Do not allow OSD-only and LSD-only params in the same `rbposd` runner config.

OSD-only keys:

- `osd_method`
- `osd_order`

LSD-only keys:

- `lsd_method`
- `lsd_order`

If both families are present, fail during preflight:

```text
rbposd params must not mix OSD and LSD decoder params
```

This avoids ambiguous specs such as:

```toml
osd_order = 10
lsd_order = 1
```

### Execution Boundary

`RbposdRunner::run_point` should continue to execute OSD configs exactly as it
does now:

```rust
let decoder = RbposdDemDecoder::new(params.bp_config);
```

For LSD configs, #91 should return a clear error before any artifact is written:

```text
rbposd LSD DEM decoding is not implemented yet; see issue #92
```

This is not a preflight rejection, because the params themselves are valid. It
is an execution-boundary error that prevents silent OSD fallback until #92 adds
an LSD-backed adapter.

The current `run_rust_benchmark` implementation builds rows before creating the
staging artifact directory. Therefore a `run_point` error still leaves no
partial `results.jsonl` artifact.

## Normalized Params

For OSD runs, keep the existing normalized params:

```json
{
  "bp_algorithm": "min_sum",
  "bp_iters": 30,
  "early_stop": true,
  "osd_method": "combination_sweep",
  "osd_order": 0
}
```

For LSD parsing, build a normalized map in memory so #92 and #93 can reuse it:

```json
{
  "bp_algorithm": "min_sum",
  "bp_iters": 30,
  "early_stop": true,
  "lsd_method": "localized_statistics",
  "lsd_order": 1
}
```

#91 should not write a successful LSD row with these fields, because successful
LSD benchmark execution is out of scope.

## Error Handling

Errors should stay deterministic and reviewer-readable:

- Unknown keys fail in the registry splitter with
  `unknown rbposd runner param: <key>`.
- Ill-typed TOML values use existing helper wording where possible, such as
  `<key> must be a string`, `<key> must be an integer`, or
  `<key> must be non-negative`.
- Unsupported OSD labels keep existing messages.
- Unsupported LSD labels should mirror OSD label messages.
- Mixed OSD/LSD params should name the contradiction directly.

All these failures should happen before benchmark artifacts are written.

## Testing

### Registry Test

Add `expand_runner_points_accepts_rbposd_lsd_params` in
`rsinter/tests/bench_registry.rs`.

The test should:

- start from `valid_runner_params()`
- add `lsd_method = "localized_statistics"` and `lsd_order = 1`
- call `expand_runner_points_for_runner("rbposd", &params)`
- assert there is still exactly one point
- assert both LSD keys are present in `points[0].decoder_params`
- assert the generic point fields still match the default surface-code point

### Preflight Acceptance Test

Add `rbposd_runner_preflight_accepts_lsd_params`.

The test should use `RbposdRunner` directly with a constructed
`BenchCasePoint`. It should assert that preflight accepts:

```toml
lsd_method = "localized_statistics"
lsd_order = 1
```

This proves the LSD param set is valid even though execution waits for #92.

### Unknown Key Negative Control

Add `rbposd_runner_rejects_unknown_lsd_param_without_artifacts`.

Use a benchmark spec with an unknown LSD-looking key, for example:

```toml
bogus_lsd = 1
```

Assert:

- the error is `unknown rbposd runner param: bogus_lsd`
- the runner artifact directory does not exist

This is the issue's negative-control contract.

### Mixed Family Negative Control

Add `rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts`.

Use a benchmark spec that contains both:

```toml
osd_order = 10
lsd_order = 1
```

Assert:

- the error is `rbposd params must not mix OSD and LSD decoder params`
- the runner artifact directory does not exist

### Unsupported LSD Value Coverage

Add focused parser or benchmark tests for:

- `lsd_method = "unknown_method"`
- `lsd_order = 2`
- negative or non-integer `lsd_order`

These can be grouped with existing `rbposd_benchmark_rejects_...` tests in
`rsinter/tests/bench_run.rs` if that keeps the suite readable.

## Verification

Issue #91 names these commands:

```bash
cargo test -p rsinter expand_runner_points_accepts_rbposd_lsd_params
cargo test -p rsinter rbposd_runner_preflight_accepts_lsd_params
cargo test -p rsinter rbposd_runner_rejects_unknown_lsd_param_without_artifacts
```

Additional recommended checks:

```bash
cargo test -p rsinter rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_run
cargo fmt --check --package rsinter
git diff --check
```

## Acceptance Criteria

- `rbposd` runner specs accept typed LSD params in `[runner.params]`.
- LSD params are carried as decoder params without multiplying benchmark
  points.
- `RbposdRunner::preflight_point` accepts the supported LSD param set.
- Unsupported, unknown, ill-typed, and mixed-family params fail before artifacts
  are written.
- Existing OSD specs remain source-compatible and keep their normalized result
  params unchanged.
- An LSD run does not silently execute the OSD DEM adapter.
- #92 has an explicit typed family branch to connect to the LSD DEM adapter.
