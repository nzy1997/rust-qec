# Issue 95 rbposd ProductSum and Serial BP Design

## Context

Issue #94 made `BpVariant::ProductSum` and `Schedule::Serial` part of the
public `rbposd::DecoderConfig`, but the compiled BP core still routes every
selector combination through the minimum-sum parallel update loop. Issue #95 is
the first behavior implementation behind that public surface: a selected
non-default method and schedule must change BP updates, snapshots, residuals, or
reliability values, and the same execution path must be shared by OSD-backed and
LSD-backed decoders.

## Decision

Keep one compiled BP implementation file and add explicit local dispatch inside
`rbposd/src/bp.rs`. The dispatch will select a check-to-variable update rule
from `BpVariant` and a scheduling loop from `Schedule`:

- `MinimumSum + Parallel` remains the current default behavior.
- `ProductSum + Parallel` uses the exact product-sum check update while keeping
  the existing parallel variable update phase.
- `MinimumSum + Serial` and `ProductSum + Serial` update each check and then
  immediately refresh the affected variable-to-check messages and hard decision
  state, making serial scheduling observably different when the graph and priors
  are sensitive to in-iteration updates.

The compatibility wrapper names for minimum-sum stay available for old unit
tests, but OSD and LSD decoders continue to call `BpCore::run_bp_in_place`, so
both families consume the same selector-aware core.

## Product-Sum Update

For each check-to-variable edge, compute the outgoing message using the standard
binary BP tanh rule:

`2 * atanh(syndrome_sign * product(tanh(v_to_c / 2) over other incident edges))`

Inputs are clamped away from `atanh(±1)` to keep values finite when priors or
degree-one checks are effectively certain. Degree-one checks keep the existing
`CERTAINTY_LLR` behavior because excluding the target edge leaves a parity check
that directly determines the bit.

## Serial Schedule

The serial loop updates one check at a time. After each check update, it
recomputes the posterior LLR, hard decision, reliability, and outgoing
variable-to-check messages for only the bits incident to that check. At the end
of each sweep it recomputes the residual and applies the same convergence and
best-snapshot rules as the parallel loop. This preserves the current snapshot
contract while giving non-parallel schedules a traceable execution path.

## Tests and Fixtures

Add failing tests before implementation:

- `product_sum_serial_changes_bp_snapshot_on_borrowed_case` compares the
  existing `bp_repetition_single_flip.json` default case with a new
  `bp_product_sum_serial_sensitive.json` fixture and asserts that
  `ProductSum + Serial` changes a documented BP observable on the sensitive
  case while preserving a valid decode.
- `minimum_sum_parallel_regression_suite_still_passes` runs the existing
  checked-in default parity fixtures as a negative control.
- A focused LSD config test constructs `BpLsdDecoder::with_bp_config` with
  `ProductSum + Serial` and confirms it follows the selected BP path.
- Unit tests in `bp.rs` cover product-sum check messages and serial scheduling
  behavior at the compiled core boundary.

The parity fixture schema must accept `product_sum` and `serial`, and
`ParityCase` should pass the built `DecoderConfig` to `BpLsdDecoder` through
`with_bp_config` so the fixture/dev harness exercises selected BP settings for
both decoder families.

## Scope

In scope:

- `rbposd/src/bp.rs`
- `rbposd/dev/parity_schema.rs`
- `rbposd/tests/bp.rs`
- `rbposd/tests/lsd_bp_config.rs`
- `rbposd/tests/reference.rs`
- `rbposd/tests/fixtures/parity/*.json`
- `rbposd/doc/ldpc_mvp_reference.md`

Out of scope:

- `rsinter` parameter parsing
- benchmark-spec edits
- additional decoder families beyond OSD and LSD
- replacing the OSD or LSD residual solvers

## Review Notes

The design keeps dispatch explicit and local, preserves the current default
path, and makes the non-default execution path reusable from both public decoder
families without forking a second BP implementation file.
