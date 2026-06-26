# rbposd LDPC-Compatible OSD-CS Contract

Date: 2026-06-26

This document defines what `ldpc`-compatible OSD-CS means for future `rbposd`
selector work. It is a compatibility contract, not a decoder-behavior change.
The current default Rust OSD path remains the legacy/internal frontier search
until a later issue adds an explicit selector.

## Candidate Planning

For a reduced OSD system, the `ldpc`-compatible OSD-CS candidate set is:

```text
singles over all non-pivot columns + pairs among the first osd_order non-pivot columns
```

The single-column sweep covers every non-pivot, also called free, column in the
reduced system. The pair sweep is intentionally narrower: it considers only
two-column combinations drawn from the first `osd_order` non-pivot columns in
the selected column order.

For `free_column_count = F` and `pair_frontier = min(F, osd_order)`, the planned
candidate count is:

```text
F + C(pair_frontier, 2)
```

For the order-7 BB90 hard-syndrome setting, this means the compatible count is
`free_column_count + C(7, 2)`, not every subset up to order 7 over a fixed
frontier.

## Ordering Versus Scoring

Candidate ordering/selection is separate from candidate scoring/objective weights.

Column ordering may follow BP soft information or another documented
reliability order. That ordering decides which non-pivot columns are "first"
for the `osd_order` pair frontier. It does not by itself define how candidate
corrections are scored.

Candidate scoring/objective weights must be documented explicitly for each
selector. A future `ldpc`-compatible selector must state whether it scores
candidates with channel-prior weights, BP posterior reliability, Hamming
weight, or another objective. A candidate planner can match upstream `osd_cs`
enumeration while still being incompatible if it scores the planned candidates
with the wrong objective.

## Legacy Frontier Contrast

The existing Rust OSD path keeps an internal 16-column frontier and enumerates
all combinations of size 1 through `osd_order` inside that frontier. For
`osd_order = 7`, the legacy/frontier count is:

```text
sum(C(16, r), r=1..7) = 26332
```

That exhaustive/frontier search remains a separate legacy/internal mode. It is
not the upstream `ldpc` `osd_cs` contract, even when both paths use the same BP
column ordering or the same `osd_order` value.

## BB90 Fixture Note

`rsinter/tests/fixtures/bb_circuit_bposd/bb90_hard_syndrome.json` records the
legacy order-7 diagnostic count `26332` for the current Rust frontier search.
Future work that introduces an explicit `ldpc`-compatible selector should keep
that legacy count distinguishable from the compatible plan count
`free_column_count + C(7, 2)`.
