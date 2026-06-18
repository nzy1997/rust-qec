# Issue 90 BpLsd Shared BP And Fixture Coverage Design

Date: 2026-06-18
Status: Proposed
Scope: GitHub issue #90, `rbposd` shared BP reuse checks, LSD fixture manifest validation, and Python `ldpc` differential coverage for the current LSD fixture set

## Summary

Issue #90 should make the new `BpLsdDecoder` path trustworthy without widening
into the later fixture-catalog work owned by #98.

The work builds on #88 and #89. `BpLsdDecoder` already exists, reuses the
internal `BpCore`, supports `lsd_order = 0` and `lsd_order = 1`, and has three
checked-in LSD JSON fixtures. #90 should lock those pieces together with a
small repo-owned validation layer:

- assert that a single `BpLsdDecoder` instance can decode multiple syndromes
  while reusing the shared BP workspace machinery safely
- add a small manifest for the checked-in LSD fixtures
- validate that manifest from Rust tests
- extend the existing Python parity harness so the current LSD fixtures can be
  compared against upstream `ldpc`

This issue should not fold the OSD/BP parity fixtures into a unified catalog.
That broader catalog and BP-option expansion remains #98.

## Goals

- Keep `BpOsdDecoder` and `BpLsdDecoder` on the shared BP core path.
- Add a regression test proving one `BpLsdDecoder` instance can decode multiple
  syndromes correctly.
- Add checked-in LSD fixture metadata with explicit provenance, verifier, pass
  condition, and consuming issue ids.
- Validate that each checked-in LSD fixture has exactly one valid manifest
  entry.
- Reject malformed manifest metadata with a clear Rust test failure.
- Extend the Python parity harness to run the current LSD fixture set against
  upstream `ldpc`.
- Keep unsupported LSD or upstream mapping combinations explicit.

## Non-Goals

- Do not add `rsinter` runner support, DEM adapters, benchmark result rows, or
  benchmark spec changes.
- Do not add new LSD algorithms, new public LSD method variants, or support for
  `lsd_order > 1`.
- Do not expand BP methods, schedules, `bits_per_step`, or broader BP-option
  support.
- Do not migrate the existing `rbposd/tests/fixtures/parity/` OSD/BP fixtures
  into the new manifest in #90.
- Do not make performance claims from the differential harness.
- Do not change the public shape of `DecodeResult`.

## Current Context

#88 added the public `BpLsdDecoder`, `LsdConfig`, and `LsdMethod` API and
extracted common BP setup into `rbposd/src/decoder_core.rs`.

#89 added `rbposd/src/lsd.rs`, `DecodeError::NoLsdSolution`, deterministic
`lsd_order = 1`, and these LSD fixtures:

- `rbposd/tests/fixtures/lsd/lsd_small_sparse_code.json`
- `rbposd/tests/fixtures/lsd/lsd_order_one_improves_over_baseline.json`
- `rbposd/tests/fixtures/lsd/lsd_unsatisfiable_case.json`

The existing Python parity harness currently targets the OSD/BP parity schema
under `rbposd/tests/fixtures/parity/`. It maps only the supported
`minimum_sum + parallel + OSD_0` shape into upstream `ldpc.BpOsdDecoder`.

#90 should bridge that gap for the current LSD fixture set while keeping the
larger shared catalog design for #98.

## Alternatives Considered

### 1. LSD-Only Manifest And Harness Adapter

Add `rbposd/tests/fixtures/lsd/manifest.json`, validate it in Rust, and teach
the Python harness how to load and compare only the current LSD fixture set.

Benefits:

- Fits #90's narrow milestone.
- Provides real repo-owned differential coverage against upstream `ldpc`.
- Avoids prematurely absorbing #98's broader BP-option fixture catalog.
- Keeps current OSD/BP parity behavior unchanged.

Cost:

- #98 will still need a follow-up pass to unify LSD and parity fixtures into a
  broader catalog.

This is the chosen approach.

### 2. Unified Manifest For LSD And Existing OSD/BP Parity Fixtures

Create one manifest that covers both `fixtures/lsd/` and `fixtures/parity/`.

Benefits:

- Closer to the eventual #98 shape.
- Gives one metadata source for all current fixtures.

Costs:

- Pulls #98's catalog work into #90.
- Increases the chance of changing mature OSD/BP parity behavior while trying
  to stabilize LSD.

This is rejected for #90.

### 3. Manifest Validation Only, No Python Harness Work

Add the LSD manifest and Rust validation tests but leave `parity_harness.py`
unchanged.

Benefits:

- Smallest implementation.
- Gives immediate fixture provenance coverage.

Costs:

- Does not satisfy #90's request for repo-owned differential tests against
  upstream `ldpc`.
- Leaves LSD trust dependent only on Rust-side tests.

This is rejected.

## Fixture Manifest Design

Add `rbposd/tests/fixtures/lsd/manifest.json`.

The manifest should cover only LSD fixtures in #90. A simple object shape is
enough:

```json
{
  "fixtures": [
    {
      "id": "lsd_small_sparse_code",
      "path": "lsd_small_sparse_code.json",
      "provenance": "Borrowed small-matrix LSD alignment case introduced for issues #89 and #90.",
      "verifier": "cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly",
      "pass_condition": "BpLsdDecoder with lsd_order=1 returns a residual-zero correction.",
      "consumes": ["#89", "#90"]
    }
  ]
}
```

Required fields:

- `id`
- `path`
- `provenance`
- `verifier`
- `pass_condition`
- `consumes`

Validation rules:

- `fixtures` must not be empty.
- Every checked-in `rbposd/tests/fixtures/lsd/*.json` file except
  `manifest.json` must have exactly one manifest entry.
- Each manifest `path` must exist and point to a fixture whose JSON `id` matches
  the manifest `id`.
- `provenance`, `verifier`, and `pass_condition` must be non-empty strings.
- `consumes` must be non-empty and include `#90`.
- Duplicate ids or duplicate paths are invalid.
- Unknown extra fixture entries are invalid.

The initial valid set should include:

- `lsd_small_sparse_code`
- `lsd_order_one_improves_over_baseline`
- `lsd_unsatisfiable_case`

Malformed-manifest coverage should be generated in tests rather than checked in
as a bad fixture file.

## Rust Architecture

### Shared BP Reuse Test

Add a focused integration test:

```text
bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes
```

The test should construct one `BpLsdDecoder`, decode at least two syndromes
through the same instance, and assert that each successful result satisfies
`pcm.multiply(&result.correction) == syndrome`.

This test is the #90 guard that the shared BP core and `BpWorkspace` reuse path
works for LSD as well as OSD. The existing
`minimum_sum_decoder_reuses_one_instance_for_multiple_syndromes` test should
remain unchanged and passing.

### Fixture Manifest Types

Add test-local manifest structs near `rbposd/tests/lsd.rs` or in a small
test-only helper:

- `LsdFixtureManifest`
- `LsdFixtureManifestEntry`

Keep the types test-local unless implementation pressure proves they are useful
for production code. The manifest is a validation artifact, not a public API.

### Manifest Validation Tests

Add:

```text
bplsd_fixture_manifest_cases_decode_cleanly
bplsd_fixture_manifest_rejects_invalid_case_metadata
```

`bplsd_fixture_manifest_cases_decode_cleanly` should:

1. Load the manifest.
2. Validate manifest coverage and required metadata.
3. Load each referenced fixture.
4. For success cases, construct `BpLsdDecoder`, decode the syndrome, and assert
   residual-zero parity.
5. For error cases, assert the expected error code.

`bplsd_fixture_manifest_rejects_invalid_case_metadata` should create malformed
manifest values in memory and assert that validation fails clearly for:

- missing provenance
- missing verifier
- missing pass condition
- missing `#90` consumer
- stale path or missing fixture entry

## Python Harness Design

Extend `rbposd/scripts/parity_harness.py` without changing the current OSD/BP
default behavior.

### Discovery

Keep `--fixtures-dir` defaulting to `rbposd/tests/fixtures/parity` for existing
runs. Add a separate LSD discovery path, for example:

```text
--lsd-fixtures-dir rbposd/tests/fixtures/lsd
--include-lsd
```

Tests may exercise the LSD path directly without requiring the default command
to include LSD immediately.

### LSD Case Loading

Load `manifest.json`, then load each fixture listed by the manifest. The Python
loader should validate enough metadata to reject unsupported or stale entries
instead of silently skipping them.

### Rust-Side Execution

Prefer extending the existing Rust parity report path so OSD and LSD produce
the same JSON shape:

```json
{
  "actual": {
    "status": "success",
    "correction": [false, true],
    "diagnostics": {
      "converged": false,
      "bp_iterations": 30,
      "used_osd": false,
      "residual_syndrome_weight": 0
    }
  }
}
```

A minimal way to do that is to extend the dev parity schema with a
decoder-family field while preserving backwards compatibility:

```json
{
  "decoder": "bplsd"
}
```

If omitted, the existing behavior remains `bposd`. LSD cases use
`decoder = "bplsd"` and include `lsd_config`.

This keeps one CLI/example report shape for the Python harness.

### Upstream `ldpc` Mapping

For `decoder = "bplsd"`, the Python side should instantiate upstream
`ldpc.BpLsdDecoder` with:

- dense matrix from the fixture
- BSC or bit-flip-probability channel
- `max_iter` matching the repo's default BP iteration contract unless the
  fixture explicitly carries an override
- `bp_method = "minimum_sum"`
- `schedule = "parallel"`
- `lsd_method` corresponding to localized statistics
- `lsd_order` from the fixture
- `input_vector_type = "syndrome"`

Only the currently supported combinations should map. Unsupported methods,
schedules, or orders should return a structured Python-side error entry.

Because upstream `ldpc` versions may use slightly different keyword names for
LSD parameters, implementation should keep that mapping in one helper and cover
it with mocked unit tests.

### Comparison Semantics

Reuse the existing classification model:

- `exact_match`
- `diagnostics_mismatch`
- `status_mismatch`
- `error_mismatch`
- `correction_mismatch`
- `payload_mismatch`

Diagnostics drift remains non-fatal. Status, error, correction, and payload
mismatches remain real mismatches.

For LSD success cases, both Rust and upstream Python outputs must produce a
correction whose residual syndrome weight is zero. Exact correction agreement is
preferred where upstream behavior is deterministic; if a fixture proves that
multiple residual-zero corrections are valid and upstream selects a different
one, that drift must be documented explicitly before being treated as
non-fatal. Do not silently downgrade correction mismatches in #90.

## Error Handling

Rust validation should produce clear failures for:

- missing manifest
- invalid JSON
- duplicate manifest id
- duplicate manifest path
- manifest path that does not exist
- fixture id mismatch between manifest and fixture JSON
- fixture file missing from manifest
- manifest entry with no checked-in fixture
- empty provenance, verifier, pass condition, or consumers
- manifest entry that does not consume `#90`

Python harness should produce structured error payloads for:

- unsupported LSD mapping
- unsupported channel type
- upstream `ldpc` constructor or decode exceptions
- missing manifest or fixture file

No path should silently coerce LSD cases into OSD decoding.

## Testing And Verification

Required commands:

```bash
cargo test -p rbposd bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes
cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly
cargo test -p rbposd bplsd_fixture_manifest_rejects_invalid_case_metadata
python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd
```

Existing regression guard:

```bash
cargo test -p rbposd minimum_sum_decoder_reuses_one_instance_for_multiple_syndromes
```

Expected results:

- all commands pass
- one `BpLsdDecoder` instance decodes multiple syndromes correctly
- every checked-in LSD fixture is covered by the manifest
- malformed manifest metadata is rejected clearly
- current LSD fixtures can be compared through the Python harness
- unsupported LSD mappings are rejected explicitly

## Documentation

Update `rbposd/doc/ldpc_mvp_reference.md` to describe:

- the new LSD fixture manifest
- the manifest metadata contract
- the LSD differential harness path
- the boundary between #90 and #98

The wording should make clear that #90 covers only the current LSD fixture set
and that #98 owns the broader shared LSD and BP-option catalog.

## Acceptance Criteria

- #90's required test command names exist and pass.
- `rbposd/tests/fixtures/lsd/manifest.json` exists and covers the current LSD
  fixture files.
- Manifest validation rejects malformed metadata.
- `BpLsdDecoder` repeated-decode reuse is tested.
- The Python harness has unit coverage for LSD fixture loading, LSD upstream
  mapping, and unsupported LSD mapping.
- Existing OSD/BP parity harness behavior remains unchanged by default.
- `rbposd/doc/ldpc_mvp_reference.md` documents the new manifest and #98
  boundary.
