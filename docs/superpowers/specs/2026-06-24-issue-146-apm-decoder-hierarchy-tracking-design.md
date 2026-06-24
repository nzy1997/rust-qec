# Issue 146 APM Decoder Hierarchy Tracking Design

Issue: #146 Track relay-BP and MIP decoder reproduction after the BP baseline lands

## Context

Issue #142 is closed and PR #161 has merged the P=96 native BP/BP-OSD smoke.
That gives the decoder hierarchy work a stable first stage:
`rsinter/tests/apm_p96_rbposd_smoke.rs` loads the P=96 APM-CSS fixture pair,
generates fixed seeded syndromes, and proves the existing `rbposd` path returns
residual-zero corrections.

The future paper reproduction needs more than that baseline. The target decoder
flow escalates from BP to relay-BP and then to a MIP-style fallback for harder
cases. This workspace already has useful pieces: `rbposd` for BP/BP-OSD/LSD,
`rilpqec` for DEM-oriented ILP decoding, `qec-ilp-core` for shared binary ILP
backend plumbing, and `rsinter/src/rbposd_adapter.rs` for existing `rsinter`
decoder integration. The issue's local `drafts/` reference paths are not
present in this worktree, so this tracking pass treats them as future reference
material rather than production dependencies.

Sibling tracking issue #145 was resolved as a roadmap document plus a link from
the relevant implementation contract, without adding the future implementation.
Issue #146 has the same shape: it asks for a split-ready tracking artifact and
explicitly says the future relay-BP/MIP work should be split after the BP
baseline is stable.

## Approaches Considered

1. Implement the staged decoder harness now.

   This would produce the future `apm_decoder_hierarchy_classifies_seeded_cases`
   test immediately, but it would also require choosing placeholder relay-BP or
   fallback semantics before the project has decided the CSS-to-relay-BP and
   CSS-to-MIP boundaries. It risks turning a tracking issue into a premature
   public API.

2. Add stage reporting to the benchmark runner schema now.

   This would make stage telemetry visible in benchmark output, but current
   benchmark rows model decoder success/failure and logical error counts rather
   than per-case residual evidence. Adding output fields before a concrete
   hierarchy harness exists would widen compatibility surface without tested
   semantics.

3. Add a durable decoder hierarchy roadmap and future acceptance contract.

   This is the selected approach. It records the stage vocabulary, evidence
   contract, first child issue command, and negative controls while keeping the
   current production code unchanged. It matches the sibling tracking issue
   pattern and keeps relay-BP/MIP implementation out of this PR.

## Chosen Design

Add `docs/apm_decoder_hierarchy.md` as the project roadmap for future APM-CSS
decoder hierarchy reproduction. Link that roadmap from
`rsinter/tests/fixtures/css/README.md`, where the P=96 fixture provenance and
the current BP baseline are already closest to the future seeded-case inputs.

The roadmap should define the future stage vocabulary:

- `bp`: the existing native `rbposd` first-stage decoder solves the case with
  residual zero.
- `relay_bp`: a future relay-BP stage solves a case that plain BP did not solve.
- `fallback`: a future final fallback solves a case after earlier stages fail.
  The first child issue may use a deliberately narrow fixture fallback to prove
  classification without claiming a real MIP implementation.
- `failed`: all enabled stages failed, or the case was invalid.

The roadmap should define the evidence each staged result must preserve:

- case id or seed/support label
- enabled stages
- stage selected for the final status
- syndrome weight
- residual weight after each attempted stage
- correction width when a correction is produced
- structured failure reason for invalid inputs or unsolved cases

The first child issue should add the concrete harness and test named
`apm_decoder_hierarchy_classifies_seeded_cases`, with this command:

```sh
cargo test -p rsinter apm_decoder_hierarchy_classifies_seeded_cases -q
```

That test should cover:

- a fixed P=96 APM-CSS syndrome solved by BP and reported as `stage=bp`
- a fixed case where BP is deliberately disabled or made insufficient and the
  enabled fallback reports `stage=fallback`
- an invalid inconsistent case that returns a structured failure instead of a
  false success
- a negative control where disabling fallback for the fallback-required case
  leaves a nonzero residual or explicit failure status

The roadmap should recommend reusing `rbposd` for the BP baseline and future
relay/BP-family experiments. It should recommend using `rilpqec` and
`qec-ilp-core` only after the project has a clear CSS-matrix to MIP problem
lowering, rather than forcing DEM-oriented ILP APIs into the first harness.

## Tracking-Issue Acceptance Criteria

- Add a durable project document describing the APM decoder hierarchy roadmap.
- Preserve the current P=96 native BP baseline as the first stage and production
  starting point.
- Include the first child issue's exact verification command:
  `cargo test -p rsinter apm_decoder_hierarchy_classifies_seeded_cases -q`.
- Include the three fixed future cases and the fallback-disabled negative
  control from the issue.
- Link the roadmap from the checked-in P=96 CSS fixture README.
- Do not implement relay-BP, MIP fallback, a new decoder runtime API, or a new
  benchmark output schema in this issue.

## Verification

The documentation contract should be checked with a focused text search:

```sh
rg -n "apm_decoder_hierarchy_classifies_seeded_cases|stage=bp|stage=fallback|fallback-disabled|relay-BP|MIP" docs/apm_decoder_hierarchy.md rsinter/tests/fixtures/css/README.md
```

The repository-level verification remains:

```sh
cargo test
```
