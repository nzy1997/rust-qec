# Issue #276 LDPC-Compatible OSD-CS Contract Design

Date: 2026-06-26
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #276, document the `ldpc`-compatible OSD-CS contract for
`rbposd`

## Context

Issue #209 tracks a BB circuit BP-OSD runtime and semantic gap between Rust
`rbposd` and upstream Python `ldpc`/`bposd`. The current Rust OSD candidate
planner keeps a legacy internal search over a capped 16-column frontier and
enumerates all combinations up to `osd_order`. For `osd_order = 7` that legacy
shape reports `sum(C(16, r), r=1..7) = 26332` candidates, which is the count
recorded by the BB90 hard-syndrome fixture.

The upstream-compatible `ldpc` OSD-CS shape needed by follow-up issues is
narrower: all single non-pivot columns, plus pair candidates drawn only from
the first `osd_order` non-pivot columns. This issue is a contract and guardrail
change only. It must not route decoders through a new selector, change scoring,
or claim a performance improvement.

## Goals

- Add a focused repository-owned contract document at
  `rbposd/doc/osd_cs_contract.md`.
- State the upstream candidate-plan shape verbatim:
  `singles over all non-pivot columns + pairs among the first osd_order non-pivot columns`.
- State that candidate ordering/selection and candidate scoring/objective
  weights are separate concerns.
- Add a small Rust contract test that includes the checked-in contract document
  so deleting or renaming it fails compilation.
- Add a count helper for the documented plan shape:
  `free_column_count + C(osd_order, 2)` when the free-column count is at least
  `osd_order`.
- Add a negative control proving the existing exhaustive/frontier count is not
  accepted as `ldpc`-compatible.

## Non-Goals

- Do not change `BpOsdDecoder` decode behavior.
- Do not add the public `ldpc_cs` selector; that belongs to #277.
- Do not change candidate scoring; channel-prior objective scoring belongs to
  #278.
- Do not update campaign runners or benchmark flows.
- Do not edit the BB90 hard-syndrome sample itself.

## Approaches Considered

### 1. New contract document plus crate-private planner helper

Create `rbposd/doc/osd_cs_contract.md`, add a crate-private
`LdpcOsdCsCandidatePlan` and planner/count helper in `rbposd/src/osd.rs`, and
test it from the existing `osd` unit-test module. This is the chosen approach:
it creates a reusable internal contract point for #277 without changing public
API or decoder behavior.

### 2. Integration-test-only helper

Keep production code untouched and compute the count only inside
`rbposd/tests/osd.rs`. This would satisfy the immediate count check, but it
would not give the next selector issue a named internal contract to reuse.

### 3. Extend current `OsdCandidateSearchPlan`

Reuse the existing diagnostic struct for both legacy and `ldpc` modes. This is
too easy to misread because the current fields name the legacy frontier and
order traversal. A separate plan type keeps the old and new contracts visibly
different.

## Design

`rbposd/doc/osd_cs_contract.md` documents the candidate-planning contract, the
legacy/internal frontier contrast, and the scoring boundary. It names the BB90
hard-syndrome fixture as evidence for the legacy count and as a future
comparison target, but it does not change the fixture.

`rbposd/src/osd.rs` gains:

- `LDPC_OSD_CS_CONTRACT_DOC_PATH`, a repository-relative path printed by the
  contract test.
- `LdpcOsdCsCandidatePlan`, with fields for `free_column_count`,
  `pair_candidate_frontier_size`, `osd_order`, and `planned_candidate_count`.
- `ldpc_osd_cs_candidate_plan_for_free_columns`, which counts all singles over
  `free_column_count` and all pairs among
  `min(free_column_count, osd_order)` selected non-pivot columns.

The helper is `pub(crate)` and does not affect existing `BpOsdDecoder` routing.
The existing legacy `candidate_search_plan` remains unchanged and continues to
serve current diagnostics until #277 introduces a selectable planner.

## Testing

The main test,
`ldpc_osd_cs_contract_matches_reference_candidate_plan`, includes the contract
document with `include_str!`, prints `rbposd/doc/osd_cs_contract.md`, checks the
required contract phrases, builds a small known free-column fixture with
`free_column_count = 20` and `osd_order = 7`, and asserts
`20 + C(7, 2) = 41`.

The negative-control test,
`ldpc_osd_cs_contract_rejects_exhaustive_frontier_plan`, computes the existing
legacy/frontier count for a 16-column frontier and order 7. It asserts that
`26332` is not accepted as the `ldpc`-compatible candidate count and uses a
failure message that explains the exhaustive/frontier search remains a separate
legacy/internal mode.

Required verification:

```sh
cargo test -p rbposd ldpc_osd_cs_contract_matches_reference_candidate_plan -- --nocapture
cargo test -p rbposd ldpc_osd_cs_contract_rejects_exhaustive_frontier_plan -q
cargo test
```

## Spec Self-Review

Placeholder scan: passed. Scope check: this is one contract document and one
internal count helper plus tests; behavior changes remain out of scope.
Ambiguity check: the candidate-plan formula and the selection/scoring boundary
are explicit above.
