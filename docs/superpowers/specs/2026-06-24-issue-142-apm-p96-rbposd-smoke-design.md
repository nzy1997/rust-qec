# Issue 142 APM P=96 rbposd Smoke Design

Issue: #142 Run a native BP baseline smoke test on the P=96 APM-CSS code

## Context

#141 is merged into `master` and provides committed `rsinter` fixtures:

- `rsinter/tests/fixtures/css/apm_p96_hx.json`
- `rsinter/tests/fixtures/css/apm_p96_hz.json`

`rsinter` already depends on `rbposd` and already has `qec-code` as a
dev-dependency for parsing the sparse-row fixture format. `rbposd` exposes the
direct native path needed for this smoke through:

- `ParityCheckMatrix::from_sparse_rows`
- `BpOsdDecoder::new`
- `BpOsdDecoder::decode`

The requested verification command is a focused `rsinter` test, so the smoke
should live under `rsinter/tests` rather than adding benchmark runner behavior.

## Approaches Considered

1. Add a focused `rsinter` integration test that parses the P=96 fixture pair
   and calls `rbposd::BpOsdDecoder` directly.

   This is the selected approach. It proves the committed P=96 matrix shape can
   compile into the native decoder and keeps the test independent of benchmark
   sampling, logical observable selection, and CLI artifact writing.

2. Add a tiny benchmark fixture using the existing CSS benchmark runner path.

   This would exercise more of `rsinter`, but it would require observable
   fixtures or benchmark output plumbing that the issue explicitly does not ask
   for. It is broader than a decoder smoke.

3. Add a helper API in `rsinter/src/rbposd_adapter.rs` for CSS matrices.

   This could be reused later, but the current issue only needs one deterministic
   native decode smoke. Adding public or semi-public adapter surface now would
   widen API compatibility risk without improving the verification.

## Chosen Design

Add `rsinter/tests/apm_p96_rbposd_smoke.rs` with one test named
`apm_p96_rbposd_smoke_decodes_seeded_syndromes`.

The test will:

- load both P=96 APM-CSS fixtures from #141 with `include_str!`
- parse both fixtures with `qec_code::css::sparse_rows_matrix_from_json_str`
- build one native `rbposd::ParityCheckMatrix` from the Hx rows, representing
  the Z-error syndrome side
- construct one `rbposd::BpOsdDecoder` with explicit configuration:
  `max_bp_iterations = 96`, `early_stop = true`,
  `bp_variant = BpVariant::MinimumSum`, `schedule = Schedule::Parallel`,
  `osd_variant = OsdVariant::Osd0`, and `osd_order = 0`
- use `ChannelModel::Bsc { error_rate: 0.02 }`
- derive three sparse nonzero error patterns from fixed seed
  `0xA9_6B_50_D5_EE_D5_14_2A`
- pin the expected supports from that seed as `[223]`, `[780, 1033]`, and
  `[346, 632, 921]`
- assert the generated supports are exactly the expected supports, so the seed
  cannot silently drift
- compute each syndrome as `Hx * error`
- assert each syndrome is nonzero
- decode each syndrome and assert the returned correction has residual zero:
  `Hx * correction == syndrome`
- run an all-zero correction negative control against the same nonzero
  syndromes and assert at least one residual remains nonzero

## Error Handling

Fixture parse and decoder construction errors should fail the test with clear
context. Syndrome generation should assert nonzero syndromes before decoding so
the negative control cannot pass vacuously.

## Test Contract

Focused verification:

```sh
cargo test -p rsinter apm_p96_rbposd_smoke_decodes_seeded_syndromes -q
```

Full verification:

```sh
cargo test
```

## Out Of Scope

- No stochastic logical error rate reproduction.
- No relay-BP implementation.
- No MIP fallback implementation.
- No new logical observable fixture selection.
- No benchmark artifact or CLI fixture changes.
