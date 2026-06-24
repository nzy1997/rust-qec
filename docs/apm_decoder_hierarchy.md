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
