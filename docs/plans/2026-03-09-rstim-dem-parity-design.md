# rstim DEM Parity Design

**Date:** 2026-03-09

## Problem

`rstim` and `Stim` use the same broad idea for detector error model generation: walk the circuit backward, propagate detector and observable sensitivities, and map noise into DEM targets. But `rstim` currently implements a simplified analyzer. On standard surface-code and repetition-code circuits it is already close to `Stim`, but several important behavior gaps remain:

- `rstim` does not reject gauge detectors or gauge observables under default analysis.
- `rstim` accepts some disjoint or conditional noise cases that `Stim` rejects unless approximation is explicitly enabled.
- `rstim` treats `ELSE_CORRELATED_ERROR` as an ordinary independent error instead of as part of a mutually-exclusive correlated block.
- `rstim` does not reject over-mixing depolarizing channels.
- `rstim` can panic on invalid `rec[]` references instead of returning structured errors.
- Advanced `Stim` behaviors such as `decompose_errors` and `fold_loops` are not yet matched.

The goal is not to rewrite `rstim` into a line-by-line copy of `Stim`. The goal is to close behavior gaps in phases, starting with the highest-risk mismatches: cases where the default `rstim analyze_errors` result disagrees with the default `stim analyze_errors` result.

## Scope

This work is split into three phases.

### Phase 1: Default Correctness

Phase 1 aligns default analyzer behavior for inputs that should be accepted or rejected by default. It does **not** add new CLI flags and it does **not** pursue output-structure parity such as folded loops.

In scope:

- Reject gauge detectors and observables by default.
- Reject invalid `rec[]` references with `Err` instead of panicking.
- Reject `PAULI_CHANNEL_2` and multi-branch correlated error blocks unless approximation is explicitly supported.
- Fix `E` / `ELSE_CORRELATED_ERROR` semantics so conditional branches are treated as mutually exclusive.
- Reject `DEPOLARIZE1(p > 3/4)` and `DEPOLARIZE2(p > 15/16)`.

Out of scope:

- New CLI options.
- Loop folding.
- Full Stim-style graphlike decomposition pipeline.
- Tag parity and exact DEM text-format parity.

### Phase 2: Option Parity

Phase 2 exposes optional Stim-like analysis modes on top of the stricter Phase 1 default:

- `approximate_disjoint_errors`
- `allow_gauge_detectors`
- decomposition-related options only when decomposition behavior is mature enough to support them

### Phase 3: Output Structure Parity

Phase 3 targets higher-level output behavior:

- `decompose_errors`
- `fold_loops`
- closer parity for repeat blocks and output structure

## Recommended Approach

Use a **guardrail-first** approach for Phase 1.

Do not start by rewriting the analyzer around a new tracker or flush model. `rstim` already has working reverse-propagation logic and already passes existing parity checks on some important circuits. Replacing the analyzer structure up front would increase risk, expand the task, and likely break already-correct paths.

Instead, keep the current `ErrorAnalyzer::circuit_to_dem` entry point and extend it with validation and stricter semantics. The implementation should remain centered in `rstim/src/error_analyzer.rs`, with small focused helpers for validation and conditional-noise handling. This allows Phase 1 to improve behavioral correctness while preserving the current success paths that already match `Stim`.

This design deliberately separates:

- **default correctness parity**, which is Phase 1
- **optional feature parity**, which is Phase 2
- **output-structure parity**, which is Phase 3

That separation keeps the first implementation tranche small and reviewable.

## Architecture

Phase 1 keeps the public entry point unchanged:

```rust
pub fn circuit_to_dem(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String>
```

Internally, the analyzer flow becomes three logical steps:

1. **Pre-scan**
   Collect global counts and validate structure that must be known ahead of the reverse walk.

2. **Strict reverse analysis**
   Walk the circuit backward as today, but combine sensitivity propagation with strict default-semantic checks.

3. **Finalize DEM**
   Merge accumulated error probabilities, append annotation instructions, and return the model only if all checks passed.

This preserves the existing output path for valid default-semantic inputs while inserting the missing validation gates.

## Phase 1 Components

### 1. Record Reference Validation

Today `DETECTOR rec[-1]` and `OBSERVABLE_INCLUDE rec[-1]` can panic due to unsigned underflow when no measurement exists. This must become a recoverable error.

Add a helper similar to:

```rust
fn checked_rec_index(num_measurements: usize, offset: i32) -> Result<usize, String>
```

Use it anywhere a `StimTarget::Rec(offset)` is converted to an absolute measurement index. This includes detector and observable handling during backward analysis.

Expected behavior:

- valid reference -> return absolute index
- invalid lookback -> return `Err(...)`

### 2. Gauge Detection

`Stim` rejects non-deterministic detectors and observables by default. `rstim` currently clears sensitivities at resets and measurements without checking whether those sensitivities anticommute with a collapse.

Add shared helpers that detect residual incompatible sensitivity before:

- `R`, `RX`, `RY`
- `M`, `MX`, `MY`
- `MR`, `MRX`, `MRY`
- final implicit initialization check at circuit start

The Phase 1 version does not need to reproduce Stim’s long diagnostic messages. It only needs to reliably detect the condition and return a structured error such as:

- `non-deterministic detector encountered`
- `non-deterministic observable encountered`

### 3. Disjoint Noise Rejection

`Stim` rejects certain disjoint channels by default unless approximation is enabled. Phase 1 should match this default behavior.

Reject by default:

- `PAULI_CHANNEL_2` when multiple components are present
- multi-branch `E` / `ELSE_CORRELATED_ERROR` blocks
- other cases that are currently implicitly approximated but should require an explicit approximation option

Do **not** change exact solvable channels that already match `Stim`, such as:

- exact `DEPOLARIZE1`
- exact `DEPOLARIZE2`
- exact `PAULI_CHANNEL_1` cases where an independent equivalent exists

### 4. Correlated Error Block Semantics

`rstim` currently handles `"CORRELATED_ERROR" | "E" | "ELSE_CORRELATED_ERROR"` as independent error insertions. This is wrong for `ELSE_CORRELATED_ERROR`.

Introduce a lightweight correlated-block parser during reverse traversal:

- collect contiguous `ELSE_CORRELATED_ERROR` instructions
- require a preceding `E`
- convert the block into mutually exclusive effective probabilities
- reject the block by default if approximation is required and not enabled

For a block like:

```text
E(p1) ...
ELSE_CORRELATED_ERROR(p2) ...
ELSE_CORRELATED_ERROR(p3) ...
```

the effective per-branch probabilities should be computed the same way as Stim:

- last branch uses remaining probability mass
- earlier branches scale by the remaining probability after later branches

This fixes the current semantic bug where `rstim` overcounts conditional branches as independent.

### 5. Over-Mixing Depolarize Guards

Before converting depolarizing noise to independent channels, reject:

- `DEPOLARIZE1(p > 3/4)`
- `DEPOLARIZE2(p > 15/16)`

These checks should run before probability conversion. Returning an error is correct; silently producing empty or invalid output is not.

## Data Flow

For valid default-semantic circuits, data flow remains mostly unchanged:

1. Count qubits, measurements, detectors, and observables.
2. Reverse-walk the circuit.
3. Update `x_sens`, `z_sens`, and `measurement_sens`.
4. Emit raw `(probability, targets)` entries into `errors`.
5. Merge same-target errors with odd-parity probability composition.
6. Append detector and shift-detector annotations.
7. Return the DEM.

For invalid default-semantic circuits, the flow stops early with `Err(...)` during reverse analysis or final gauge post-check.

That separation is intentional: correctness errors should fail before any partially-valid DEM is returned.

## Testing Strategy

Testing should be added in three layers.

### Analyzer Behavior Tests

Extend `rstim/tests/stim_error_analyzer.rs`:

- unignore the gauge detector and gauge observable tests once implemented
- add explicit tests for invalid `rec[]`
- add tests for `ELSE_CORRELATED_ERROR` semantics
- add tests for disjoint-channel default rejection
- add tests for over-mixing depolarize rejection

### CLI Regression Tests

Extend `rstim/tests/cli_analyze.rs`:

- invalid default-semantic inputs should fail cleanly
- no panic output paths
- `rstim analyze_errors` should reject the same classes of bad input as default `stim analyze_errors`

### Cross-Validation Preservation

Keep and rerun the existing semantic parity tests for already-good paths, especially:

- surface-code DEM cross-validation
- ported Stim analyzer behavior tests that currently pass

The point of these tests is to ensure Phase 1 guardrails do not perturb circuits that already match.

## Acceptance Criteria

Phase 1 is complete when all of the following are true:

- `rstim analyze_errors` no longer panics on bad `rec[]` references.
- Gauge detectors and observables are rejected by default.
- Multi-branch correlated error blocks no longer use independent-error semantics.
- Default analysis rejects disjoint-noise cases that require approximation.
- Over-mixing `DEPOLARIZE1/2` is rejected.
- Existing passing parity checks for supported standard circuits remain green.

## Risks

### Risk: Breaking Existing Good Paths

The biggest risk is accidentally changing sensitivity propagation while adding strict checks. Mitigation: keep validation helpers narrow and avoid refactoring propagation logic unless directly necessary.

### Risk: Message-Text Overfitting

Stim emits rich diagnostic text. Reproducing it exactly would expand scope without much product value. Mitigation: assert on error category and success/failure behavior, not exact full message parity.

### Risk: Hidden Conditional-Noise Edge Cases

`E` / `ELSE_CORRELATED_ERROR` semantics are the most subtle Phase 1 area. Mitigation: add targeted minimal tests before changing logic.

## Follow-Up

After Phase 1 is implemented and verified:

- Phase 2 should add `approximate_disjoint_errors` and `allow_gauge_detectors` support.
- Phase 3 should revisit analyzer structure for `decompose_errors` and `fold_loops`.
