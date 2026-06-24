# Issue 141 APM P=96 rsinter Fixture Design

Issue: #141 Add P=96 APM-CSS rsinter fixtures and logical-count smoke coverage

## Context

`qec-code` now exposes the P=96 APM Kasai CSS instance as
`apm_kasai:p=96`, and `master` contains the merged #140 export path. The
source-of-truth commands are:

```sh
cargo run -p qec-code -- code css apm_kasai:p=96 hx
cargo run -p qec-code -- code css apm_kasai:p=96 hz
```

`rsinter/tests/fixtures/css` already keeps downstream CSS fixtures for
`steane` and `bb72`. This issue adds the same committed fixture surface for
the P=96 APM code, without adding stochastic benchmark coverage or logical
observable fixtures.

## Approaches Considered

1. Shell out to `cargo run -p qec-code` from the `rsinter` test.

   This matches the command text literally, but it nests Cargo inside Cargo's
   test runner. That is slower, brittle under lock contention, and not the
   pattern used by existing tests.

2. Copy qec-code's existing test fixtures into rsinter and compare the two
   committed files.

   This would catch accidental edits to the rsinter copy, but it would not
   prove the copy still matches the current built-in `qec-code` export after
   registry or builder changes.

3. Add `qec-code` as an `rsinter` dev-dependency and call the public
   `qec_code::cli::run` CSS export path from the test.

   This is the selected approach. It exercises the same code path used by
   `qec-code code css apm_kasai:p=96 hx|hz` while staying in-process and
   deterministic. The test compares exact JSON bytes, then parses the same
   fixture pair to verify the structural CSS facts.

## Chosen Design

Add these committed downstream fixtures:

- `rsinter/tests/fixtures/css/apm_p96_hx.json`
- `rsinter/tests/fixtures/css/apm_p96_hz.json`

Generate them from the current `qec-code` export. Add
`rsinter/tests/fixtures/css/README.md` with exact regeneration commands for
the APM fixtures.

Add a focused `rsinter` integration test named
`apm_p96_css_fixture_has_580_logicals`. The test will:

- load the committed `rsinter` `Hx` and `Hz` fixtures with `include_str!`
- call `qec_code::cli::run` with `CodeCommands::Css(CssArgs::export(...))`
  for `apm_kasai:p=96 hx|hz`
- assert exact byte-for-byte JSON equality between committed fixtures and the
  current qec-code export strings
- parse the fixture JSON through `qec_code::css::sparse_rows_matrix_from_json_str`
- assert both matrices use `num_cols = 1152`
- compute `rank_x` and `rank_z` with `qec_code::binary::try_binary_rank`
- assert `k = 1152 - rank_x - rank_z = 580`
- assert all X/Z row overlaps have even parity
- mutate one in-memory `Hz` support and assert the corrupted pair fails the
  exact-export comparison or CSS orthogonality

## Test Contract

Focused verification:

```sh
cargo test -p rsinter apm_p96_css_fixture_has_580_logicals -q
```

Full verification:

```sh
cargo test
```

## Out Of Scope

- No stochastic BP or BP+OSD benchmarks.
- No explicit APM logical observable fixture.
- No production `rsinter` dependency on `qec-code`.
- No P=192 fixture coverage.
