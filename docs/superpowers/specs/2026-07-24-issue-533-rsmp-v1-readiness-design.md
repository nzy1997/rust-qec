# Issue 533 RSMP v1 Readiness Design

## Context

Issue #533 is the final `rsmp v1` integration issue. The dependency chain has
already supplied the catalog, transform, archive, CLI, limit/error,
publication, corruption-corpus, compatibility-fixture, and compression-evidence
checks. This issue must aggregate those checks without replacing them with
static PASS text or broad repository substring searches.

This Agent Desk run is non-interactive. Per the standing policy, the approved
issue text is treated as the approval for a conservative implementation that
reuses existing checkers and fails closed when a required input is missing or
stale.

## Approaches Considered

1. **Single readiness aggregator over existing checks (selected).** Add
   `tools/check_rsmp_v1_readiness.py` to run focused Cargo/Python checks, parse
   current JSON/TOML/docs/help inputs, write `benchmarks/out/rsmp-v1`, and own
   the final PASS line. This preserves the evidence chain and gives CI one
   deterministic entrypoint.
2. **Rust-only readiness integration.** Put the aggregator in a Rust example or
   integration test. This would reuse Rust internals directly but would be
   awkward for documentation/help normalization and artifact orchestration.
3. **Make-only orchestration.** Add a Make target that shells out to existing
   tests and prints the final line. This would be simpler but would not create
   a structured failure artifact or support negative controls through the real
   validation path.

## Selected Architecture

The readiness gate is a Python aggregator with small pure validation helpers
and one command-runner layer:

- validates the fixture catalog with `tools/check_rsmp_fixture_catalog.py`;
- runs focused locked Cargo checks for RSMP format, transform, archive,
  streaming, result-format, limits, CLI, publication, corruption, and
  compatibility behavior;
- runs the corruption-corpus example to produce a real JSON summary and derives
  named recipe, truncation, and bit-flip counts from that summary;
- calls `tools.check_rsmp_v1_compression_evidence.check_bundle()` and derives
  measured gate values from current committed evidence;
- validates `rstim/doc/rsmp-v1.md`, a new `rstim/doc/rsmp-cli.md`, and a
  normalized `pack_samples`/`unpack_samples` help model built from live
  `rstim` help output;
- writes `benchmarks/out/rsmp-v1/readiness.json` with `status`, hashes, counts,
  checked commands, and named failed checks;
- prints the required final PASS line only after every validation succeeds.

## Documentation Design

`rstim/doc/rsmp-v1.md` remains the normative format document. It will gain
designated semantic sections for transform losslessness, binary fields, bit and
varint canonicality, integrity/authentication boundaries, resource limits,
validation precedence, stable error taxonomy, compatibility policy, and
compression evidence.

`rstim/doc/rsmp-cli.md` will be the operational guide. It will document
`pack_samples`, `unpack_samples`, and `unpack_samples --verify_only` using the
final underscore option spelling, all supported result formats, output
publication semantics, and the recommended nondeveloper validation route.

The checker will inspect semantic sections by heading and compare documented
CLI option models against normalized live help. It will not rely on
repository-wide greps or raw Clap whitespace.

## Error Handling

Each readiness check records a stable check id and diagnostic. On failure the
tool exits nonzero, omits the final readiness PASS line, and writes
`readiness.json` with `status = "fail"` whenever the output directory can be
created. The required negative-control diagnostics are emitted exactly as
specified by issue #533.

## CI Design

The main CI workflow gains an always-on `rsmp-v1-readiness` job for every push
and pull request covered by the workflow. It installs the same system, Stim,
and Rust prerequisites as the test job, runs `cargo fetch --locked`, then runs
`CARGO_NET_OFFLINE=true make rsmp-v1-readiness`. The readiness step captures
its outcome, uploads `benchmarks/out/rsmp-v1/` with `if: always()`, appends a
short failure summary to `$GITHUB_STEP_SUMMARY`, and explicitly fails the job
after artifact upload if the gate failed.

## Testing

Add `tools/test_check_rsmp_v1_readiness.py` with the four required negative
controls. Each control copies the required source inputs to a temporary repo
root, mutates the copy, runs the same readiness validation path in
validation-only mode, and asserts the expected diagnostic, nonzero status,
missing PASS line, and failed artifact status.

Verification commands:

- `python3 -m unittest tools.test_check_rsmp_v1_readiness.RsmpV1ReadinessNegativeControls`
- `make rsmp-v1-readiness`
- `cargo test`
