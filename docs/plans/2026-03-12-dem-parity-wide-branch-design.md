# DEM Parity Wide Branch Design

**Date:** 2026-03-12

## Goal

Extend the current `fix/analyze-errors-measurement-noise` branch into a wider
DEM-parity branch that closes two remaining high-value gaps against Stim:

1. `decompose_errors` semantic parity
2. `fold_loops` structural parity as an explicit opt-in feature

The target is **semantic parity**, not byte-for-byte output identity. For
representative circuits, `rstim` should agree with Stim on:

- number of detectors
- number of observables
- error target sets
- probabilities
- graphlike status after decomposition

For loop folding, the goal is that folded output is semantically equivalent to
the corresponding flat output and broadly matches Stim's `repeat` /
`shift_detectors` structure on representative repeated circuits.

## Branch Scope

This branch intentionally goes beyond the narrow measurement-noise fix and
becomes a dedicated DEM-parity branch. The branch will cover:

- library and CLI support for `decompose_errors`
- semantic cross-checks against Stim for decomposed DEMs
- library and CLI support for explicit loop folding
- semantic equivalence checks between folded and flat DEMs
- representative codegen and hand-written regression circuits

This branch will **not** attempt:

- exact text-format parity in all cases
- full reimplementation of Stim internals
- unrelated simulator, parser, or sampling changes

## Guiding Principles

### 1. Preserve Current Defaults

Current default behavior should remain stable unless a behavior is already
wrong relative to Stim. In particular:

- CLI `analyze_errors` should continue to default to flat output
- new folding behavior should be opt-in
- plain non-decomposed analysis should remain unchanged when the new flags are
  not used

This mirrors the user's preference to avoid surprising default changes while
still adding parity features.

### 2. Separate Semantic and Structural Concerns

`decompose_errors` and `fold_loops` solve different problems and should not be
implemented as one blended analysis path.

- `decompose_errors` changes the *shape of error terms* in order to make
  non-graphlike mechanisms graphlike
- `fold_loops` changes the *representation of the DEM* by compressing repeated
  structure into `repeat` blocks

The implementation should keep these concerns layered so that semantic bugs can
be debugged independently from structural compression bugs.

### 3. Use Stim as the Semantic Oracle

For accepted inputs, the branch should compare against Stim on concrete
behavior, not on visual similarity. The primary checks are:

- detector and observable counts
- target-set multisets
- probabilities per target set
- graphlike property after decomposition

For folded output, flat-vs-folded equivalence inside `rstim` is also a required
check because loop folding is a structural compression pass.

## Architecture

The recommended architecture is a single base DEM analysis path with two
explicit opt-in post-processing transforms:

1. plain analysis
2. optional decomposition transform
3. optional loop-folding transform

This yields four supported modes:

- plain
- decompose
- fold
- decompose + fold

The architecture should avoid duplicating analysis logic across CLI and library
entrypoints. The CLI and library should share the same option surface and the
same internal pipeline.

### Plain Mode

This remains the current source of truth for default analysis. It produces the
flat detector error model used today.

### Decompose Mode

This mode starts from the plain DEM and applies graphlike decomposition. The
goal is not to copy Stim's exact decomposition wording or exact component order
in every case. The goal is to match Stim semantically:

- graphlike output where Stim succeeds
- correct target components
- preserved detector / observable counts
- matching probabilities

### Fold Mode

This mode starts from the semantically correct DEM of the chosen mode
(`plain` or `decompose`) and attempts to compress repeated patterns into
`repeat` blocks and `shift_detectors`.

This should be treated as a structural compression pass, not a second semantic
analyzer. If a circuit does not fit the supported folding pattern, the code
should conservatively fall back to flat output instead of forcing a bad fold.

## Decompose Errors Strategy

`decompose_errors` should be implemented and validated first. It carries higher
semantic risk because it changes error terms and graphlike status.

### Priority Inputs

There are two required input classes.

#### 1. Hand-Written Mechanism Circuits

These are small circuits chosen to isolate behavior:

- single `DEPOLARIZE1` or `DEPOLARIZE2` circuits producing non-graphlike errors
- circuits with observables included in decomposed error terms
- cases with multiple plausible decomposition paths
- combinations of decomposition with existing analysis options

These tests are meant to answer narrow questions quickly and make debugging
root causes feasible.

#### 2. Codegen Circuits

These provide realistic workload coverage:

- repetition code
- rotated surface code
- color code

These tests ensure the implementation is not overfit to toy cases and behaves
correctly on real circuits produced by `rstim` itself.

### Acceptance Criteria for Decomposition

For representative circuits where Stim succeeds:

- `rstim` should produce graphlike components
- detector / observable counts should match Stim
- the error target multiset should match Stim semantically
- probabilities should match Stim within floating-point tolerance
- plain mode output should remain unchanged when decomposition is not enabled

## Fold Loops Strategy

`fold_loops` should be implemented after decomposition is stable.

### Default Behavior

The user's chosen compatibility target is:

- CLI remains flat by default
- folded output is available only via explicit opt-in
- library behavior should also expose folding explicitly instead of silently
  changing existing defaults in this branch

This avoids mixing API-default parity work with structural compression work.

### Supported Folding Shape

The first supported folding pass should focus on common steady-state repeated
circuits:

- explicit `REPEAT N { ... }`
- stable detector pattern per iteration
- fixed detector index shift between iterations
- stable error pattern modulo detector offset
- stable or boundary-only observable contribution

If these conditions are not met, the implementation should keep the output
flat. Correctness is more important than forcing compression.

### Acceptance Criteria for Folding

For representative circuits:

- folded and flat output must be semantically equivalent
- detector / observable counts must be unchanged
- `repeat` / `shift_detectors` structure should be broadly comparable to Stim
  on standard repeated circuits
- default CLI output must remain flat

## Testing Strategy

The branch should use one shared test matrix across both features.

### Test Inputs

- hand-written non-graphlike circuits
- hand-written repeat / shift-detectors circuits
- hand-written observable edge cases
- repetition code circuits
- rotated surface code circuits
- color code circuits

### Test Modes

Each representative circuit should be checked in up to four modes:

- plain
- decompose
- fold
- decompose + fold

### Test Assertions

Priority assertions:

- detector count
- observable count
- error target multiset
- per-target probability map
- graphlike property after decomposition
- folded-vs-flat semantic equivalence

Secondary assertions for folded output:

- presence of `repeat`
- reasonable `shift_detectors` placement
- broad structural similarity to Stim on supported repeated circuits

## Execution Order

The branch should proceed in four batches.

### Batch 1: Decomposition Cross-Checks

Add Stim-vs-rstim regression tests for hand-written and codegen circuits in
plain and decomposed modes. Use these tests to reveal the current semantic
gaps before changing implementation.

### Batch 2: Decomposition Semantics

Tighten `decompose_errors` behavior until the decomposition cross-checks pass.
Keep plain mode unchanged.

### Batch 3: Explicit Loop Folding

Add opt-in folding support in the library and CLI. Validate folded-vs-flat
equivalence first, then compare folded structure against Stim where practical.

### Batch 4: Combined Regression Matrix

Run the unified mode matrix on the representative circuit set:

- plain
- decompose
- fold
- decompose + fold

This final batch ensures the two features compose correctly and that the branch
has a clear semantic completion bar.

## Completion Criteria

This branch is complete when all of the following are true:

- `decompose_errors` is available in library and CLI paths used for DEM
  extraction
- representative decomposed outputs match Stim semantically
- loop folding is available as an explicit opt-in path
- folded outputs remain semantically equivalent to the corresponding flat DEMs
- CLI defaults remain unchanged
- regression coverage exists for hand-written and code-generated circuits across
  the supported mode matrix
