# Issue 130 BB144 QEC-Code Regression Design

Date: 2026-06-23
Status: Approved for implementation under the Agent Desk standing policy
Scope: Final `qec-code` regression coverage and scope notes for the bivariate-bicycle CSS family MVP

## Summary

Issue #130 closes the bivariate-bicycle CSS family MVP with end-to-end CLI
coverage for the BB144 parameterized spec:

```text
bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0
```

The test should prove that `qec-code code css <spec> hx` emits compact
`sparse_rows` JSON with `num_cols == 144` and `72` rows. It should also prove
that malformed shift syntax exits non-zero without JSON stdout.

This remains `qec-code` construction and export support only. Circuit-level
BB144 work and benchmark/reproduction work remain downstream in #110 and #124.

## Current State

The dependency issues have landed the required layers:

- #126 / PR #131 added `BivariateBicycleParams` and
  `bivariate_bicycle_css_checks(...)`.
- #127 / PR #147 routed the fixed `bb72` alias through the generic constructor.
- #128 / PR #144 added `bb:lx=...,ly=...,a=...,b=...` parser support.
- #129 / PR #149 wired parsed bivariate-bicycle specs through existing CSS
  sparse-row export and catalog output.

`qec-code/tests/cli.rs` already has fixture comparisons for the fixed `bb72`
alias and parameterized BB72, plus a negative CLI control for `lx=0`. The missing
coverage is a BB144-sized parameterized CLI smoke that parses stdout as JSON and
checks the sparse-row shape requested by issue #130.

## Chosen Approach

Add a focused integration test in `qec-code/tests/cli.rs`:

- define a `BB144_PARAMETERIZED_SPEC` constant,
- run `qec-code code css <BB144 spec> hx`,
- parse stdout with `serde_json`,
- assert `format == "sparse_rows"`,
- assert `num_cols == 144`,
- assert `rows.len() == 72`,
- assert all rows have weight `6`.

Add a second CLI negative-control test for:

```text
bb:lx=12,ly=6,a=3:0|,b=0:3
```

The command must fail, print empty stdout, and report an invalid `a` parameter
on stderr. This proves the smoke test is using the parser/export path rather
than accepting malformed BB specs.

## Alternatives Considered

1. Add a committed BB144 fixture. Rejected because issue #130 asks for a smoke
   shape check, not a new pinned matrix artifact.
2. Add another constructor-level BB144 test. Rejected because #126 already covers
   constructor shape and orthogonality; #130 specifically asks for CLI sparse-row
   export.
3. Modify `rsinter` or circuit benchmark tests. Rejected because #130 explicitly
   keeps #110 and #124 as downstream circuit/benchmark scopes.

## Testing

The focused regression should be exercised by:

```bash
cargo test -p qec-code --test cli bb144
```

Final verification should run:

```bash
cargo test -p qec-code
cargo run -p qec-code -- code css "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0" hx
cargo run -p qec-code -- code css "bb:lx=12,ly=6,a=3:0|,b=0:3" hx
cargo test
```

The positive CLI run should emit compact `sparse_rows` JSON with
`num_cols == 144` and `72` rows. The negative CLI run should exit non-zero and
emit no JSON stdout.
