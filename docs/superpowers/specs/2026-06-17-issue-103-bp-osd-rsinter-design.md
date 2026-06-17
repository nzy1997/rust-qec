# Issue #103 BP+OSD Rsinter Path Design

## Context

Issue #103 asks for a paper-faithful `rsinter` path for bivariate-bicycle CSS
memory benchmarks. The concrete target is the BB `[[72,12,6]]` instance under
CSS input, explicit logical observables, a greedy syndrome-extraction schedule,
and BP+OSD decoder settings matching the BB paper's numerical setup:
MIN-SUM belief propagation, 10000 BP iterations, and combination-sweep OSD.

The required prerequisites are already merged into this repository:

- #46 / PR #51 added general CSS codegen and `rsinter input_type = "css"`.
- #47 / PR #52 added tunable `rsinter` decoder parameters and preflight
  validation for unknown runner parameters.
- #101 / PR #104 added an exact CSS distance backend and verifies `bb72` has
  exact distance 6.
- #102 / PR #105 added explicit CSS logical observables and BB72 observable
  fixtures.

The related AutoQEC path is issue #18, which needs the first BB qLDPC campaign,
and issue #16 / PR #31, which introduced concrete decoder registry entries:
`rbposd-osd0-v1`, `rbposd-osd10-v1`, and `predict-zero-v1`. PR #31's committed
artifacts show that OSD0 and predict-zero can run on the CSS BB path, while
OSD10 was deferred as too heavy for the snapshot. This design therefore keeps
fast CI checks small and puts paper-reference reproduction behind a manual or
ignored long-run fixture.

## Goal

Make `rsinter` able to run and record a paper-faithful BP+OSD path for fixed BB
CSS memory benchmarks, with enough provenance for AutoQEC to distinguish a
human-readable decoder ID from the backend implementation and exact effective
decoder settings.

The implementation should close issue #103 by providing:

- a real `predict-zero` runner key for the existing always-zero decoder
- validated `rbposd` paper-parameter labels for BP and OSD choices
- deterministic seed input and output provenance
- explicit result-row provenance for the backend decoder implementation
- a BB72 CSS BP+OSD fixture with explicit observables
- fast tests for the contract, plus a documented manual reference fixture for
  the expensive paper-reference point

## Non-Goals

This design does not make CI reproduce the full BB paper Table 1 or Figure 2A
data on every run. The accepted scope is fast deterministic contract coverage
in CI and a manual or ignored long-run fixture for paper-reference validation.

This design does not add a general bivariate-bicycle parameter generator to
`rsinter`. It uses committed BB72 CSS matrices and observable fixtures.

This design does not change the core `rbposd` algorithm beyond exposing and
validating names for settings that are already represented by the current Rust
implementation.

## Current Gaps

`rsinter` already records the generic CSS fields needed by AutoQEC:
`input_type`, `code_id`, `basis`, `schedule`, `rounds`, `p`, matrix paths,
observable provenance, shot budgets, logical error counts, and LER. `rbposd`
already records normalized `bp_iters`, `early_stop`, and `osd_order`.

The missing pieces are:

1. `predict-zero` is not exposed as a benchmark runner, even though
   `VacuousDecoder` already exists.
2. `rbposd` does not accept or record the paper-facing labels
   `bp_algorithm = "min_sum"` and `osd_method = "combination_sweep"`.
3. The benchmark seed is fixed internally to `12345`, but not accepted as a
   runner parameter or recorded as effective provenance.
4. Result rows expose the runner alias in `row.runner`, but do not record a
   stable `decoder_impl` field for the backend implementation key.
5. Existing BB72 benchmark smoke coverage uses `rmatching`; issue #103 needs a
   BP+OSD BB72 fixture and a predict-zero negative control.

## Design

### Runner Registry

Add a `predict-zero` rust benchmark runner to the default registry. It should
wrap `VacuousDecoder`, accept no decoder-specific parameters, and use the same
shared `run_decoder_point` path as the other DEM decoders. This keeps logical
error accounting identical to the real decoders: a logical failure is any
sample whose actual observable bit-packed bytes differ from the all-zero
prediction.

The default registry should expose:

- `rmatching`
- `rbposd`
- `rilpqec`
- `predict-zero`

Unknown decoder-specific params for `predict-zero` must fail during preflight,
before artifacts are written.

### Rbposd Paper Parameters

Extend `RbposdRunnerParams::parse` to accept these optional scalar fields:

- `bp_algorithm`
- `osd_method`

The only supported `bp_algorithm` value is `"min_sum"`. It maps to the current
`DecoderConfig` default `BpVariant::MinimumSum`. Unsupported values fail during
preflight with a message that names the invalid value and the supported value.

The only supported `osd_method` value is `"combination_sweep"`. It names the
current positive-order OSD search behavior in `rbposd::osd`, and is also valid
when `osd_order = 0` because it records the method family selected by the
runner configuration. Unsupported values fail during preflight.

Normalized result params for `rbposd` should include:

```json
{
  "bp_algorithm": "min_sum",
  "bp_iters": 10000,
  "early_stop": true,
  "osd_method": "combination_sweep",
  "osd_order": 10
}
```

Defaults are recorded even when the input omits the labels, so result rows show
the effective paper-facing configuration. Existing `bp_iters` and
`max_bp_iterations` alias behavior remains unchanged: setting both is invalid,
and result rows use the canonical `bp_iters` key.

### Generic Seed Parameter

Add `seed` as a generic benchmark runner parameter. It must be a non-negative
integer accepted by every runner, not a decoder-specific field. When omitted,
the current default seed remains `12345`.

`BenchCasePoint` should carry `seed: u64`, and `run_decoder_point` should seed
its sampler from `point.seed` instead of the run-context seed alone. The
context-level default stays useful for legacy specs and future top-level run
configuration, but the point carries the effective value.

Result rows should include:

```json
{
  "seed": 12345
}
```

This makes the issue #103 sample TOML a real reproducibility contract instead
of relying on an implicit harness default.

### Decoder Implementation Provenance

Result rows should record the backend implementation key separately from the
user-facing runner alias. The shared runner path can add:

```json
{
  "decoder_impl": "rbposd"
}
```

For example, an AutoQEC runner named `rbposd-osd10-v1` with
`impl_key = "rbposd"` should produce:

- `row.runner = "rbposd-osd10-v1"`
- `row.params.decoder_impl = "rbposd"`

The same contract applies to `predict-zero`, `rmatching`, and `rilpqec`.

### BB72 BP+OSD Fixture

Add a committed benchmark fixture for the BB72 CSS path. The fixture should use
the existing BB72 matrices and explicit X logical observables:

```toml
name = "bb72_css_bposd"
version = 1
mode = "independent"

[[runner]]
name = "rbposd-osd10-v1"
language = "rust"
impl_key = "rbposd"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.003]
seed = 12345
max_shots = 64
max_errors = 32
batch_size = 64
bp_algorithm = "min_sum"
bp_iters = 50
early_stop = true
osd_method = "combination_sweep"
osd_order = 10
```

The fast fixture intentionally uses a small shot budget and `bp_iters = 50`.
Its job is to prove the contract and output schema, not to reproduce the paper
statistics. The `code_id` uses AutoQEC's instance name while the matrix payloads
remain the existing `bb72` fixtures, which are already tied to the paper's
`l = 6, m = 6, A = x^3 + y + y^2, B = y^3 + x + x^2` construction.

Add a second predict-zero fixture or a second runner in the same fixture:

```toml
[[runner]]
name = "predict-zero-v1"
language = "rust"
impl_key = "predict-zero"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.003]
seed = 12345
max_shots = 64
max_errors = 32
batch_size = 64
```

The negative-control assertion should use a wide deterministic window, such as
`0.35 <= LER <= 0.65`, only if the chosen fixed-seed small budget lands there
reliably. If the deterministic sample is outside that window, adjust the seed or
budget and document the chosen value in the test.

### Manual Paper-Reference Fixture

Add an ignored test or documented fixture for the heavier paper-reference
configuration:

- `input_type = "css"`
- `code_id = "bivariate-bicycle-code-m6-n6"`
- explicit BB72 X logical observables
- `basis = "x"`
- `schedule = "greedy"`
- `rounds = [3]` for the issue #103 memory-benchmark point
- `seed = 12345`
- `bp_algorithm = "min_sum"`
- `bp_iters = 10000`
- `osd_method = "combination_sweep"`
- `osd_order = 10`

The documentation should state that the paper Table 1 reports BB72 logical
error-rate reference values for the circuit-based protocol, and that Table 6
reports the fitting parameters used for extrapolation:

```text
p_L(p) = p^(d_circ / 2) * exp(c0 + c1*p + c2*p^2)
BB72: d_circ = 6, c0 = 11.09, c1 = 365.6, c2 = -16088
```

This gives useful BB72 reference points such as approximately `0.00458` at
`p = 0.003` and approximately `0.507` at `p = 0.01`. The manual fixture may
target either or both points, but it must record which point was checked and
the binomial confidence interval used. Because this repository's fast CI point
uses a small finite shot budget and may not run the identical long protocol
budget, the manual fixture is the correct place to check binomial confidence
intervals against the reference point.

## Error Handling

All unsupported or misspelled parameters must fail before any results artifact
is written. This preserves the guarantee added by PR #52.

Validation should reject:

- unknown generic keys
- unknown decoder-specific keys
- `rbposd` configs that set both `bp_iters` and `max_bp_iterations`
- non-string `bp_algorithm` or `osd_method`
- unsupported `bp_algorithm` values
- unsupported `osd_method` values
- negative or non-integer `seed`

Errors should mention the runner implementation and key where possible. For
example: `rbposd bp_algorithm must be "min_sum", got "sum_product"`.

## Testing

Fast tests:

1. Registry tests confirm `predict-zero` is present in the default rust runner
   registry and names list.
2. `predict-zero` benchmark smoke runs on BB72 CSS explicit observables and
   records `decoder_impl = "predict-zero"`, `seed = 12345`, and the expected CSS
   provenance fields.
3. `rbposd` benchmark smoke records `decoder_impl = "rbposd"`,
   `bp_algorithm = "min_sum"`, `bp_iters`, `early_stop`,
   `osd_method = "combination_sweep"`, and `osd_order`.
4. `rbposd` rejects unsupported `bp_algorithm` and `osd_method` values before
   writing results.
5. The generic parameter expander accepts scalar `seed`, defaults it to
   `12345`, and rejects malformed values.
6. Existing unknown-parameter tests continue to pass for decoder-specific and
   generic misspellings.

Manual or ignored tests:

1. Run the BB72 BP+OSD fixture with `bp_iters = 10000` and `osd_order = 10`.
2. Accumulate enough shots or logical errors to compare against the documented
   paper-reference point using a binomial confidence interval.
3. Keep the command and expected interpretation in docs so AutoQEC can reuse
   the same configuration in issue #18.

Suggested verification during implementation:

```sh
cargo test -p rsinter --test bench_registry
cargo test -p rsinter --test bench_run
cargo test -p qec-code --test code bb72
```

If the exact-distance feature is available in the local environment, also run
the BB72 exact-distance smoke from issue #101.

## Completion Criteria

Issue #103 is complete when:

- `rsinter` accepts the issue sample shape for CSS BB input using `rbposd`
  BP+OSD parameters.
- Result rows include CSS input provenance, explicit logical-observable
  provenance, `decoder_impl`, effective seed, effective BP+OSD params,
  shots/errors/LER, and existing run metadata.
- Unsupported and misspelled generic or decoder parameters fail before result
  artifacts are written.
- A fast committed BB72 CSS BP+OSD smoke test runs in CI.
- A fast predict-zero negative control remains available.
- A manual or ignored paper-reference fixture documents the heavy
  `bp_iters = 10000`, `osd_order = 10`, MIN-SUM, combination-sweep path and how
  to compare it against the BB72 reference point.

## Risks

The main risk is overclaiming paper reproduction from a small CI fixture. The
design avoids this by separating contract tests from manual reference
validation.

The second risk is parameter-name drift between AutoQEC and `rsinter`. Recording
both `row.runner` and `params.decoder_impl`, plus normalized BP+OSD params,
keeps the contract explicit.

The third risk is making `seed` appear decoder-specific. Treating it as a
generic `BenchCasePoint` parameter keeps reproducibility independent of decoder
implementation.
