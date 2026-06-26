# Issue 277 ldpc-compatible OSD-CS selector design

Issue: #277 Add an explicit ldpc-compatible OSD-CS selector to rbposd

Date: 2026-06-26

## Context

Issue #276 is already merged through PR #288 and documents the required
`ldpc`-compatible OSD-CS contract. The key candidate-planning rule is singles
over all non-pivot columns plus pairs among the first `osd_order` non-pivot
columns. The current Rust OSD path remains a separate legacy/internal frontier
search that enumerates all combinations up to `osd_order` inside a 16-column
frontier.

Issue #277 adds the explicit selector and diagnostics needed by #209 so
benchmark rows cannot claim upstream `osd_cs` compatibility while executing the
legacy Rust frontier planner.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming review
gates use the standing answer policy:

- No visual companion is needed because the work is API and candidate-planner
  behavior, not visual design.
- The design is approved from the issue contract and merged #276 document.
- The safest compatibility choice is to keep the existing default behavior as
  the legacy Rust planner and make `ldpc` compatibility opt-in.

## Approaches Considered

1. Add an explicit planner enum to `DecoderConfig`, keep default legacy behavior,
   and parse benchmark `osd_method` values into that enum.
   This is recommended because it preserves existing callers while making
   compatibility claims explicit and observable.
2. Change `osd_order > 0` to use the `ldpc` planner by default.
   This would be simpler for future benchmarks but changes existing behavior and
   conflicts with the #276 contract caution.
3. Leave `DecoderConfig` unchanged and only change `rsinter` benchmark parsing.
   This would not let library callers or diagnostics distinguish the path.

## Design

`rbposd::OsdVariant` will distinguish the OSD candidate planner:

- `Osd0`: existing default order-0 spelling.
- `LegacyCombinationSweep`: the current Rust frontier/exhaustive planner.
- `LdpcCombinationSweep`: the upstream-compatible `osd_cs` planner from #276.

`DecoderConfig::default()` remains `Osd0` with `osd_order = 0`. Existing code
that only sets `osd_order` keeps legacy behavior unless it explicitly sets
`osd_variant = OsdVariant::LdpcCombinationSweep`.

`OsdVariant::from_method_name` will parse benchmark/user-facing method names
and return `DecodeError::UnsupportedOsdMethod` for unknown strings. This gives
`rbposd` itself a negative-control surface instead of leaving all validation in
`rsinter`.

`rbposd/src/osd.rs` will route decode, profiling, and diagnostic planning
through the selected planner. The legacy planner will keep using the current
16-column frontier and combination enumeration. The `ldpc` planner will try all
single forced free columns and all pair combinations among the first
`osd_order` free columns, using the existing candidate scoring function. Scoring
parity is out of scope for this issue and remains owned by #278.

`OsdPathDiagnostic` will report a stable planner name such as
`legacy_combination_sweep` or `ldpc_osd_cs`, along with the existing count
fields. For `ldpc_osd_cs`, `planned_candidate_count` is
`free_column_count + C(min(free_column_count, osd_order), 2)`. For the legacy
planner, existing frontier fields and counts remain distinguishable.

`rsinter/src/bench/runners/rbposd.rs` will accept explicit OSD method names:

- `combination_sweep` and `legacy_combination_sweep` select the legacy Rust
  planner.
- `ldpc_osd_cs` and `osd_cs` select the `ldpc`-compatible planner.

Unsupported method names continue to fail preflight before any benchmark output
is emitted, and the error message names the bad method.

## Testing

Add a focused `rbposd/tests/osd.rs` test for the required positive control:

- construct a decoder with `OsdVariant::LdpcCombinationSweep` and
  `osd_order = 7`;
- call `diagnose_osd_path`;
- assert planner name `ldpc_osd_cs`;
- assert planned count `free_column_count + C(7, 2)` on a fixture with enough
  free columns;
- assert a legacy config reports a distinct planner and legacy count.

Add a negative-control test named
`unsupported_osd_method_is_rejected_without_fallback` in `rbposd` for
`OsdVariant::from_method_name("osd_cs_typo")`, plus matching `rsinter`
benchmark coverage so the same bad runner parameter fails validation without
writing an artifact directory or result row.

Regression checks:

```bash
cargo test -p rbposd ldpc_osd_cs_candidate_plan_counts_singles_and_order_pairs -- --nocapture
cargo test -p rbposd unsupported_osd_method_is_rejected_without_fallback -q
cargo test -p rsinter unsupported_osd_method_is_rejected_without_fallback -q
cargo test
```
