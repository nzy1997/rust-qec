# Issue 143 APM Kasai P=192 CSS Spec Design

Issue: #143 Add P=192 APM generator acceptance and registry coverage

## Context

`qec-code` already contains the Table A1 manifest entry for
`apm_kasai:p=192`, a native APM sparse-row builder, shared APM structural
verification helpers, and the built-in CSS registry shape used by
`apm_kasai:p=96`. The current registry deliberately supports only P=96 and
the CLI test asserts that P=192 remains unsupported.

The arXiv record for 2604.16209 reports the target `[[2304,1156,<=14]]`
instance, matching the issue body and the committed Table A1 manifest. No
project `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md` file is present in this
checkout. Issue #143 has no comments, and no matching PR was found.

## Approaches Considered

1. Generate and commit full P=192 sparse-row fixtures.

   This would make exact output comparisons possible, but it adds large static
   fixtures for no issue requirement. The native generator already produces
   deterministic sparse rows, and the milestone asks for structural acceptance
   rather than a fixture copier.

2. Register P=192 with only a shallow shape smoke test.

   This is small, but it would not prove the generator is preserving CSS
   orthogonality, regular row/column weights, or the paper's rank-derived
   `k=1156`.

3. Add structural acceptance through the built-in CSS entrypoint.

   This is the selected approach. It uses the P=192 Table A1 constants in the
   native generator, exposes `apm_kasai:p=192` in the same registry/CLI path as
   P=96, and proves the generated matrices with the shared verifier without
   committing full fixtures or adding decoder benchmarks.

## Chosen Design

Extend the APM Kasai built-in CSS registry from one supported P value to two
supported values: `96` and `192`. The catalog should list both concrete specs:

- `apm_kasai:p=96`
- `apm_kasai:p=192`

The builder should dispatch on `p` and construct an `ApmCssManifestEntry` from
the pinned Table A1 constants. Keep the existing P=96 constants unchanged and
add P=192 constants:

- `P = 192`
- `J = 3`
- `L = 12`
- `f = [(71,127), (97,80), (67,117), (163,165), (25,60), (187,33)]`
- `g = [(163,165), (55,183), (167,79), (139,41), (109,78), (31,27)]`

Avoid introducing manifest JSON parsing into the runtime registry. The
predecessor P=96 path uses pinned constants, and keeping P=192 in the same
shape avoids test-fixture coupling in production code.

## Error Handling

Unsupported APM Kasai P values remain explicit
`UnsupportedBuiltInCssIntegerParameter` errors. After this change the error
for `apm_kasai:p=128` must name the requested value and list both supported
values, `96` and `192`. The old note that P=192 is tracked by #143 should be
removed because P=192 is now supported.

## Testing

Add a structural acceptance test named `apm_p192_builds_paper_stats` in
`qec-code/tests/code.rs`. It should call `built_in_css_checks("apm_kasai:p=192")`
and assert:

- `qec-code` catalog contains `apm_kasai:p=192`
- `code_id == "apm_kasai:p=192"`
- `num_cols == 2304`
- `hx.len() == 576`
- `hz.len() == 576`
- the shared APM verifier reports CSS orthogonality
- all Hx/Hz row weights are 12
- all Hx/Hz column weights are 3
- `k = 2304 - rank_x - rank_z = 1156`
- a P=192 in-memory coefficient mutation using a corresponding P=96
  coefficient fails verifier expectations
- `built_in_css_checks("apm_kasai:p=128")` reports supported values `96, 192`

Update `qec-code/tests/cli.rs` so `qec-code code css list` includes P=192 and
`qec-code code css apm_kasai:p=192 hx|hz` emits sparse-row JSON with
`num_cols = 2304`. Keep the P=128 negative CLI check and update the supported
values assertion to include `96, 192`.

Focused verification:

```sh
cargo test -p qec-code apm_p192_builds_paper_stats -q
```

Full verification:

```sh
cargo test
```

## Out of Scope

- No stochastic decoding or circuit-level simulation for P=192.
- No committed full P=192 sparse-row fixtures unless a later issue asks for
  exact fixture comparison.
- No public production verifier API changes.
