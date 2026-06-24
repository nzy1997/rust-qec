# Issue 146 APM Decoder Hierarchy Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a durable project roadmap for future APM-CSS decoder hierarchy reproduction without implementing relay-BP or MIP fallback.

**Architecture:** Keep production behavior unchanged. Add one human-facing roadmap document under `docs/` and link it from the P=96 CSS fixture README so future split issues have stable stage names, evidence requirements, acceptance cases, and negative controls.

**Tech Stack:** Markdown docs in the existing Cargo workspace; no new Rust API, dependencies, tests, fixtures, or benchmark schema.

## Global Constraints

- Do not implement relay-BP, MIP fallback, a new decoder runtime API, or a new benchmark output schema.
- Preserve `rsinter/tests/apm_p96_rbposd_smoke.rs` as the current P=96 native BP/BP-OSD baseline and first-stage reference.
- The first child issue verification command must be exactly `cargo test -p rsinter apm_decoder_hierarchy_classifies_seeded_cases -q`.
- The first child issue must include a BP-success case reported as `stage=bp`.
- The first child issue must include a fallback case reported as `stage=fallback` when BP is deliberately disabled or made insufficient.
- The first child issue must include an invalid inconsistent case that reports a structured failure instead of a false success.
- The first child issue negative control must disable fallback on the fallback-required case and leave a nonzero residual or explicit failure.
- Link the roadmap from `rsinter/tests/fixtures/css/README.md`.
- Run `cargo test`.

---

## File Structure

- Create: `docs/apm_decoder_hierarchy.md` for the future relay-BP/MIP decoder hierarchy roadmap and child-issue split criteria.
- Modify: `rsinter/tests/fixtures/css/README.md` to link the roadmap from the P=96 fixture provenance note.
- Create: `docs/superpowers/plans/2026-06-24-issue-146-apm-decoder-hierarchy-tracking.md` with this execution plan.

### Task 1: Document APM Decoder Hierarchy Roadmap

**Files:**
- Create: `docs/apm_decoder_hierarchy.md`
- Modify: `rsinter/tests/fixtures/css/README.md`
- Create: `docs/superpowers/plans/2026-06-24-issue-146-apm-decoder-hierarchy-tracking.md`

**Interfaces:**
- Consumes: P=96 fixture pair `rsinter/tests/fixtures/css/apm_p96_hx.json` and `rsinter/tests/fixtures/css/apm_p96_hz.json`.
- Consumes: current baseline smoke `rsinter/tests/apm_p96_rbposd_smoke.rs`.
- Produces: roadmap document that future issue authors can use to split stage-classification, relay-BP, and MIP fallback work.
- Produces: a link from `rsinter/tests/fixtures/css/README.md` to the roadmap.

- [ ] **Step 1: Create the roadmap document**

Create `docs/apm_decoder_hierarchy.md` with exactly this content:

```markdown
# APM Decoder Hierarchy Roadmap

Issue #146 tracks future reproduction of the APM-CSS decoder hierarchy after
the P=96 native BP baseline has landed. The current checked baseline is
`rsinter/tests/apm_p96_rbposd_smoke.rs`, which loads the P=96 fixture pair,
generates deterministic seeded syndromes, and proves the native `rbposd`
BP/BP-OSD path can return residual-zero corrections.

This roadmap is intentionally not a relay-BP or MIP implementation. It records
the future stage contract so the next issue can add the smallest useful harness
without changing benchmark output or decoder APIs prematurely.

## Current Boundary

The first stage is the existing native BP/BP-OSD path through `rbposd`.
The P=96 fixtures come from:

- `rsinter/tests/fixtures/css/apm_p96_hx.json`
- `rsinter/tests/fixtures/css/apm_p96_hz.json`

The future hierarchy should start from those fixed matrices and seeded syndrome
cases. Any local `drafts/` decoder references named in the tracking issue are
reference material only and should not become production dependencies.

## Future Stage Vocabulary

Future staged results should use these stage names:

- `bp`: the existing native `rbposd` first-stage decoder solves the case with
  residual zero.
- `relay_bp`: a future relay-BP stage solves a case that plain BP did not solve.
- `fallback`: a final fallback stage solves a case after earlier stages fail.
  The first child issue may use a narrow fixture fallback to prove
  classification without claiming a real MIP implementation.
- `failed`: all enabled stages failed, or the case was invalid.

The first child issue should use `stage=bp` and `stage=fallback` in its
assertions so the output contract is visible before the real relay-BP and MIP
implementations land.

## Evidence Contract

Each staged decode attempt should preserve enough evidence to diagnose why a
stage succeeded or failed:

- case id or seed/support label
- enabled stages
- selected final stage or failure status
- syndrome weight
- residual weight after each attempted stage
- correction width when a correction is produced
- structured failure reason for invalid inputs or unsolved cases

Residual evidence is part of the contract. A later MIP fallback must not erase
the BP or relay-BP residual that caused escalation.

## Recommended Split

1. Stage-classification harness.

   Add a small `rsinter` harness that classifies fixed P=96 seeded cases by
   stage and preserves residual/failure evidence. The first version should
   prove the stage contract only; it should not implement relay-BP or claim a
   production MIP fallback.

2. Relay-BP stage.

   Add the paper-specific relay-BP behavior once the stage harness makes the
   escalation boundary testable. Reuse `rbposd` BP-family components where
   possible before adding new solver abstractions.

3. MIP fallback.

   Add MIP fallback only after the project has a clear CSS-matrix to MIP
   lowering. Prefer reusing `rilpqec` and `qec-ilp-core` backend plumbing, but
   avoid forcing DEM-oriented APIs into the CSS harness if the problem
   semantics do not match.

## First Child Issue Acceptance

The first implementation issue should provide this focused command:

```sh
cargo test -p rsinter apm_decoder_hierarchy_classifies_seeded_cases -q
```

The test should produce PASS/FAIL evidence for at least three fixed cases:

- a BP case solved by the existing native baseline, reported as `stage=bp`
- a fallback case where BP is deliberately disabled or made insufficient and an
  enabled fallback reports `stage=fallback`
- an invalid inconsistent case that reports a structured failure instead of a
  false success

Required negative control:

- a fallback-disabled run on the fallback-required case leaves a nonzero
  residual or explicit failure status

## Non-Goals For This Tracking Issue

- Do not implement relay-BP.
- Do not implement MIP fallback.
- Do not add a new public decoder runtime API.
- Do not add or change benchmark output schema.
- Do not add stochastic logical error rate reproduction.
```

- [ ] **Step 2: Link the roadmap from the CSS fixture README**

Replace `rsinter/tests/fixtures/css/README.md` with exactly this content:

```markdown
# CSS Fixtures

APM P=96 fixtures are generated from the qec-code built-in CSS export:

```sh
cargo run -p qec-code -- code css apm_kasai:p=96 hx > rsinter/tests/fixtures/css/apm_p96_hx.json
cargo run -p qec-code -- code css apm_kasai:p=96 hz > rsinter/tests/fixtures/css/apm_p96_hz.json
```

The native BP/BP-OSD baseline for these fixtures is checked by
`rsinter/tests/apm_p96_rbposd_smoke.rs`. Future relay-BP and MIP fallback
reproduction is tracked in
[`docs/apm_decoder_hierarchy.md`](../../../docs/apm_decoder_hierarchy.md).
```

- [ ] **Step 3: Verify the docs contain the required future acceptance gates**

Run:

```sh
rg -n "apm_decoder_hierarchy_classifies_seeded_cases|stage=bp|stage=fallback|fallback-disabled|relay-BP|MIP" docs/apm_decoder_hierarchy.md rsinter/tests/fixtures/css/README.md
```

Expected: output includes matches in `docs/apm_decoder_hierarchy.md` for the
future command, both stage labels, relay-BP, MIP, and fallback-disabled language.
It also includes the roadmap link text in `rsinter/tests/fixtures/css/README.md`.

- [ ] **Step 4: Run full verification**

Run:

```sh
cargo test
```

Expected: the workspace test suite exits 0. Existing warnings from unrelated
tests are acceptable only if the command succeeds.

- [ ] **Step 5: Commit**

Stage the docs and commit:

```sh
git add docs/apm_decoder_hierarchy.md \
  rsinter/tests/fixtures/css/README.md \
  docs/superpowers/plans/2026-06-24-issue-146-apm-decoder-hierarchy-tracking.md
git commit -m "docs: track apm decoder hierarchy roadmap"
```
