# Issue 145 APM Searcher Integration Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a durable project roadmap for future APM searcher integration without adding a searcher implementation.

**Architecture:** Keep production behavior unchanged. Add one human-facing roadmap document under `qec-code/doc/` and link it from the existing APM construction contract so future split issues have stable acceptance criteria.

**Tech Stack:** Markdown docs in the existing Rust workspace; no new Rust API, dependencies, or generated fixtures.

## Global Constraints

- Do not add a searcher, wrapper, stochastic benchmark, or new public runtime API.
- Preserve `apm_kasai:p=96` and `apm_kasai:p=192` as the production APM path.
- Recommend manifest import before wrapper or native searcher work.
- Include first child issue verification command: `cargo test -p qec-code apm_search_tiny_case_round_trips_to_manifest -q`.
- Include negative controls for non-unit affine slope and accidentally commuting required noncommuting pair.
- Mention that `drafts/construct_apm_css_code` is reference material, not a production dependency.
- Run `cargo test`.

---

## File Structure

- Create: `qec-code/doc/apm_searcher_integration.md` for the future searcher roadmap and child-issue split criteria.
- Modify: `qec-code/doc/apm_css.md` to link the new roadmap from the existing APM construction contract.

### Task 1: Document APM Searcher Integration Roadmap

**Files:**
- Create: `qec-code/doc/apm_searcher_integration.md`
- Modify: `qec-code/doc/apm_css.md`

**Interfaces:**
- Consumes: fixed APM built-ins `apm_kasai:p=96` and `apm_kasai:p=192`.
- Consumes: existing validation contract in `qec-code/doc/apm_css.md`.
- Produces: roadmap document that future issue authors can use to split manifest import, wrapper, and native searcher work.
- Produces: a link from `qec-code/doc/apm_css.md` to the roadmap.

- [ ] **Step 1: Create the roadmap document**

Create `qec-code/doc/apm_searcher_integration.md` with exactly this content:

```markdown
# APM Searcher Integration Roadmap

Issue #145 tracks future integration of an APM parameter searcher after the
fixed Table A1 instances have landed. The current production path is still the
native fixed-instance generator exposed as `apm_kasai:p=96` and
`apm_kasai:p=192`.

## Current Boundary

`qec-code` builds the fixed Table A1 instances from pinned affine constants and
validates their sparse-row output through the existing APM construction
contract. Search work must not replace or silently mutate those built-ins.

The reference C++ searcher named in the issue lives outside the tracked
workspace under `drafts/construct_apm_css_code/apm_g8_mod.cpp` when a local
draft checkout is available. Treat that clone as reference material. A
production command should not depend on the ignored draft directory.

## Recommended Split

Start with a manifest import path before adding a wrapper or native searcher.
That keeps the search algorithm outside the production generator while proving
that discovered candidates can enter the same validation and sparse-row build
path as the fixed Table A1 instances.

1. Manifest import tool.

   Accept a tiny APM candidate manifest, validate affine and commutation
   constraints, then feed it into the native generator. This should be the first
   concrete child issue.

2. Development-only reference wrapper.

   If the local Kasai reference clone is available, add a wrapper that runs the
   C++ tool and emits the same manifest format. Keep the wrapper out of the
   production fixed-instance path.

3. Native Rust searcher.

   Consider a native searcher only after the import format and wrapper
   provenance prove which search settings are stable enough to maintain:
   `P`, `J`, `L`, required noncommuting pairs, try limits, seeds, optional
   cycle/Psi checks, and any reusable learned state.

## Future Input And Output Contract

Input should include:

- `P`, `J`, and `L`
- affine `f` and `g` map families with explicit slope and offset values
- required noncommuting pairs
- optional required commuting pairs, cycle checks, or Psi checks
- search provenance such as seed, try limit, command line, and reference-code
  revision

Output should be a validated manifest entry compatible with the native APM
generator. The importer must reject invalid affine data before matrix
generation and should preserve enough provenance to reproduce the search or
wrapper command.

## First Child Issue Acceptance

The first implementation issue should provide this focused command:

```sh
cargo test -p qec-code apm_search_tiny_case_round_trips_to_manifest -q
```

The test should generate or import a tiny known-valid APM case, validate its
affine constraints, and feed it into the native generator.

Required negative controls:

- a search/import result with a non-unit affine slope is rejected before matrix
  generation
- a required noncommuting pair that accidentally commutes is rejected before
  matrix generation

## Non-Goals

- Do not port the full searcher as the first child issue.
- Do not make the ignored `drafts/` clone a production dependency.
- Do not add stochastic decoding or benchmark coverage as part of the import
  contract.
- Do not replace the fixed `apm_kasai:p=96` or `apm_kasai:p=192` built-ins.
```

- [ ] **Step 2: Link the roadmap from the APM construction contract**

In `qec-code/doc/apm_css.md`, insert this section after the "Fixture Scope"
list and before "Data Model":

```markdown
## Searcher Integration

Future APM searcher work is tracked separately in
[`apm_searcher_integration.md`](apm_searcher_integration.md). The searcher
roadmap preserves the fixed Table A1 built-ins as the production path and starts
future integration with manifest import validation before any wrapper or native
searcher work.
```

- [ ] **Step 3: Verify the docs contain the required future acceptance gates**

Run:

```sh
rg -n "apm_search_tiny_case_round_trips_to_manifest|non-unit affine slope|accidentally commutes|manifest import" qec-code/doc/apm_searcher_integration.md qec-code/doc/apm_css.md
```

Expected: output includes matches in `qec-code/doc/apm_searcher_integration.md`
for the command, both negative controls, and manifest import language. It also
includes the link text in `qec-code/doc/apm_css.md`.

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
git add qec-code/doc/apm_searcher_integration.md qec-code/doc/apm_css.md docs/superpowers/plans/2026-06-24-issue-145-apm-searcher-integration-tracking.md
git commit -m "docs: track apm searcher integration roadmap"
```
