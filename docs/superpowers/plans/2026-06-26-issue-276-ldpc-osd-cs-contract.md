# Issue #276 LDPC-Compatible OSD-CS Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Check in a written `ldpc`-compatible OSD-CS contract for `rbposd` and a Rust guardrail test for its candidate-plan count.

**Architecture:** Keep current decoder behavior unchanged. Add a focused contract document, a crate-private count helper for the future `ldpc_cs` selector, and unit tests that include the document and reject the existing legacy exhaustive/frontier count as not `ldpc`-compatible.

**Tech Stack:** Rust 2024, existing `rbposd/src/osd.rs` unit tests, Markdown repository documentation.

## Global Constraints

- Contract document path: `rbposd/doc/osd_cs_contract.md`.
- Required upstream-shape text: `singles over all non-pivot columns + pairs among the first osd_order non-pivot columns`.
- The contract text must explicitly separate candidate ordering/selection from candidate scoring/objective weights.
- `ldpc`-compatible candidate count for `free_column_count = 20` and `osd_order = 7` is `20 + C(7, 2) = 41`.
- The legacy exhaustive/frontier count for frontier size 16 and order 7 is `sum(C(16, r), r=1..7) = 26332`, and it must not be accepted as `ldpc`-compatible.
- Do not change decoder behavior, campaign runners, public API, or performance claims.
- Required verification commands:
  - `cargo test -p rbposd ldpc_osd_cs_contract_matches_reference_candidate_plan -- --nocapture`
  - `cargo test -p rbposd ldpc_osd_cs_contract_rejects_exhaustive_frontier_plan -q`
  - `cargo test`

---

## File Structure

- Create: `rbposd/doc/osd_cs_contract.md`
  - Defines the upstream-compatible candidate-planning shape, scoring boundary, and legacy frontier contrast.
- Modify: `rbposd/src/osd.rs`
  - Adds a crate-private candidate-plan helper and unit tests that include the document.

## Task 1: Contract Document And Candidate-Plan Guardrail

**Files:**
- Create: `rbposd/doc/osd_cs_contract.md`
- Modify: `rbposd/src/osd.rs`

**Interfaces:**
- Produces: `ldpc_osd_cs_candidate_plan_for_free_columns(free_column_count: usize, osd_order: usize) -> LdpcOsdCsCandidatePlan`.
- Produces: `LdpcOsdCsCandidatePlan { free_column_count, pair_candidate_frontier_size, osd_order, planned_candidate_count }`.
- Consumes: existing `binomial(n, k)` helper for count calculation and legacy negative control.

- [ ] **Step 1: Add the RED contract tests**

In `rbposd/src/osd.rs`, update the test module import:

```rust
use super::{
    OsdWorkspace, binomial, decode_osd0_with_workspace,
    ldpc_osd_cs_candidate_plan_for_free_columns,
};
```

Then append these tests inside the existing `#[cfg(test)] mod tests` block:

```rust
    const LDPC_OSD_CS_CONTRACT_PATH: &str = "rbposd/doc/osd_cs_contract.md";
    const LDPC_OSD_CS_CONTRACT: &str = include_str!("../doc/osd_cs_contract.md");
    const REQUIRED_UPSTREAM_SHAPE: &str =
        "singles over all non-pivot columns + pairs among the first osd_order non-pivot columns";
    const REQUIRED_SCORING_BOUNDARY: &str =
        "Candidate ordering/selection is separate from candidate scoring/objective weights";

    #[test]
    fn ldpc_osd_cs_contract_matches_reference_candidate_plan() {
        println!("contract document: {LDPC_OSD_CS_CONTRACT_PATH}");
        assert_contract_text_is_complete();

        let free_column_count = 20;
        let osd_order = 7;
        let plan = ldpc_osd_cs_candidate_plan_for_free_columns(free_column_count, osd_order);

        assert_eq!(plan.free_column_count, free_column_count);
        assert_eq!(plan.pair_candidate_frontier_size, osd_order);
        assert_eq!(plan.osd_order, osd_order);
        assert_eq!(
            plan.planned_candidate_count,
            free_column_count as u128 + binomial(osd_order, 2)
        );
    }

    #[test]
    fn ldpc_osd_cs_contract_rejects_exhaustive_frontier_plan() {
        assert_contract_text_is_complete();

        let plan = ldpc_osd_cs_candidate_plan_for_free_columns(20, 7);
        let legacy_exhaustive_frontier_count: u128 =
            (1..=7).map(|order| binomial(16, order)).sum();

        assert_eq!(legacy_exhaustive_frontier_count, 26_332);
        assert_ne!(
            plan.planned_candidate_count,
            legacy_exhaustive_frontier_count,
            "exhaustive/frontier search remains a separate legacy/internal mode, \
             not the upstream ldpc osd_cs contract"
        );
    }

    fn assert_contract_text_is_complete() {
        assert!(
            LDPC_OSD_CS_CONTRACT.contains(REQUIRED_UPSTREAM_SHAPE),
            "contract document {LDPC_OSD_CS_CONTRACT_PATH} must state `{REQUIRED_UPSTREAM_SHAPE}`"
        );
        assert!(
            LDPC_OSD_CS_CONTRACT.contains(REQUIRED_SCORING_BOUNDARY),
            "contract document {LDPC_OSD_CS_CONTRACT_PATH} must separate ordering/selection \
             from scoring/objective weights"
        );
    }
```

- [ ] **Step 2: Run RED**

Run:

```sh
cargo test -p rbposd ldpc_osd_cs_contract_matches_reference_candidate_plan -- --nocapture
```

Expected: fails because `rbposd/doc/osd_cs_contract.md` and
`ldpc_osd_cs_candidate_plan_for_free_columns` do not exist yet.

- [ ] **Step 3: Add the contract document**

Create `rbposd/doc/osd_cs_contract.md` with this content:

```markdown
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
```

- [ ] **Step 4: Add the candidate-plan helper**

In `rbposd/src/osd.rs`, add this struct and helper after
`OsdCandidateSearchPlan`:

```rust
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LdpcOsdCsCandidatePlan {
    pub(crate) free_column_count: usize,
    pub(crate) pair_candidate_frontier_size: usize,
    pub(crate) osd_order: usize,
    pub(crate) planned_candidate_count: u128,
}

#[allow(dead_code)]
pub(crate) fn ldpc_osd_cs_candidate_plan_for_free_columns(
    free_column_count: usize,
    osd_order: usize,
) -> LdpcOsdCsCandidatePlan {
    let pair_candidate_frontier_size = free_column_count.min(osd_order);
    let planned_candidate_count =
        free_column_count as u128 + binomial(pair_candidate_frontier_size, 2);

    LdpcOsdCsCandidatePlan {
        free_column_count,
        pair_candidate_frontier_size,
        osd_order,
        planned_candidate_count,
    }
}
```

- [ ] **Step 5: Run GREEN focused checks**

Run:

```sh
cargo test -p rbposd ldpc_osd_cs_contract_matches_reference_candidate_plan -- --nocapture
cargo test -p rbposd ldpc_osd_cs_contract_rejects_exhaustive_frontier_plan -q
```

Expected: both pass. The first command prints
`contract document: rbposd/doc/osd_cs_contract.md`.

- [ ] **Step 6: Format and run package tests**

Run:

```sh
cargo fmt
cargo test -p rbposd
```

Expected: both exit 0.

- [ ] **Step 7: Commit**

Run:

```sh
git add rbposd/doc/osd_cs_contract.md rbposd/src/osd.rs
git commit -m "docs: document ldpc osd cs contract"
```

## Plan Self-Review

Spec coverage: the contract document, exact required phrases, candidate-plan
count, negative control, and no-behavior-change boundary are all implemented by
Task 1. Placeholder scan: no unresolved placeholder markers or vague
implementation steps. Type consistency: `LdpcOsdCsCandidatePlan` and
`ldpc_osd_cs_candidate_plan_for_free_columns` are named consistently.
