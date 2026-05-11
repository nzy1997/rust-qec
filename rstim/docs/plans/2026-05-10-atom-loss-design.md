# Atom Loss Sampling Design

**Date:** 2026-05-10

## Goal

Add first-class atom loss support to rstim's circuit execution and sampling path.

The new behavior must support:

1. A loss noise channel that marks qubits as absent with probability `p`
2. Gate execution that skips only the target groups touching lost qubits
3. Two measurement modes for lost qubits:
   - default mode: loss is read as measurement result `1`
   - loss-visible mode: measurement output explicitly indicates loss
4. Syndrome sampling from circuits containing loss via the existing `sample` and `detect` flows

## Scope

This design covers the following path only:

- parser / IR
- tableau executor
- reference sample generation
- frame simulator batch sampling
- sampler and CLI integration
- tests and user-facing documentation

This design does **not** cover:

- `error_analyzer`
- `circuit_to_dem`
- loss-aware detector error models
- codegen insertion of loss noise

## Instruction Surface

### New noise instruction

Add a new noise instruction:

```text
LOSS(p) targets...
```

`LOSS(p)` independently marks each target qubit as lost with probability `p`.

Properties:

- it does not append to the measurement record
- it does not directly modify the tableau or frame Pauli state
- it only updates per-qubit loss state

### Default measurement semantics

Existing measurement instructions keep their current names.

For single-qubit measurements and measure-reset instructions:

- `M`, `MX`, `MY`, `MZ`
- `MR`, `MRX`, `MRY`, `MRZ`

the new rule is:

- if the target qubit is not lost, behavior is unchanged
- if the target qubit is lost, the measurement result is recorded as `1`

This intentionally makes loss indistinguishable from a physical `1` outcome in the default interface.

### Loss-visible measurement semantics

Add explicit loss-visible variants for the single-qubit measurement family:

- `ML`, `MXL`, `MYL`, `MZL`
- `MRL`, `MRXL`, `MRYL`, `MRZL`

For each target qubit these instructions append **two** measurement bits:

1. `loss_flag`
2. `value_bit`

Rules:

- if the qubit is not lost: append `0` then the normal measurement value
- if the qubit is lost: append `1` then `1`

This preserves compatibility with the default "loss reads as `1`" rule while still exposing loss explicitly when requested.

### Measurement instructions not extended in v1

The following instructions remain available only in their existing form in v1:

- `MPP`
- `MXX`
- `MYY`
- `MZZ`

If any qubit participating in one measured product or pair is lost, that measured group records `1`.

v1 does not add `MPPL`, `MXXL`, `MYYL`, or `MZZL`.

## Execution Model

### Runtime state

Both the tableau executor and the frame simulator gain explicit loss state:

- executor: `Vec<bool> lost`
- frame simulator: one bitset row per qubit indicating which shots are lost

Loss state is separate from the Pauli / tableau state.

### Single-qubit gates

For single-qubit unitary gates and single-qubit noise channels that act on a target qubit:

- if `lost[q]` is false, behavior is unchanged
- if `lost[q]` is true, the operation is skipped for that qubit

This applies to Clifford gates, Pauli gates, and noise channels such as `X_ERROR`, `DEPOLARIZE1`, and `PAULI_CHANNEL_1`.

### Multi-qubit gates

Operations are evaluated per natural target group, not per instruction as a whole.

Examples:

- `CX 0 1 2 3` is treated as two pairs: `(0, 1)` and `(2, 3)`
- `SWAP 0 1 2 3` is treated as two pairs
- `PAULI_CHANNEL_2` is treated per pair
- `MPP` is treated per Pauli product

If one group contains any lost qubit, only that group is skipped or forced into loss measurement semantics. Other groups in the same instruction continue to execute.

This is the required behavior for circuits such as:

```text
CX 0 1 2 3
```

where loss on qubit `1` skips only `CX 0 1` and still applies `CX 2 3`.

### Reset and recovery

The first version uses reset-based recovery:

- `R`, `RX`, `RY`, `RZ` clear loss state before preparing the target basis state
- `MR`, `MRX`, `MRY`, `MRZ` first record the measurement result, then clear loss state and reset
- `MRL`, `MRXL`, `MRYL`, `MRZL` follow the same ordering

This means loss persists until the qubit is explicitly reset or measure-reset.

## Sampling and Syndrome Behavior

### Reference sample

Reference sample generation must understand `LOSS` and the new measurement semantics.

In noiseless reference sampling:

- `LOSS` contributes no loss events because it is a noise instruction
- loss-visible instructions still append the correct number of measurement bits
- reference bits for those instructions are all zero because there is no loss in the reference path

This keeps `sample` and `detect` aligned with existing reference-sample machinery.

### Detector sampling

No detector-specific feature is required. Detector behavior follows from the measurement record.

Examples:

- default measurements allow loss to influence syndrome bits implicitly through value `1`
- loss-visible measurements allow circuits to place detector annotations directly on `loss_flag` bits via `rec[...]`

This is enough to support syndrome sampling on circuits that intentionally expose loss information.

## Testing Plan

Tests should cover four layers.

### Parser / IR

- `LOSS(p)` parses and round-trips
- new `ML` / `MRL` family parses and round-trips
- measurement count accounting reflects the extra bits produced by loss-visible instructions

### Executor

- loss is sampled independently per target
- single-qubit gates skip lost qubits
- paired gates skip only affected pairs
- `R/RX/RY/RZ` recover lost qubits
- default measurements on lost qubits return `1`
- loss-visible measurements append `(1, 1)` on lost qubits and `(0, normal)` otherwise

### Frame simulator / sampler

- batch sampling matches executor semantics on fixed seeds where applicable
- loss state survives across instructions until reset
- detector outputs can reference loss-visible measurement bits

### CLI integration

- `sample` works on circuits containing `LOSS`
- `detect` works on circuits containing `LOSS`
- output lengths are correct when `ML`-family instructions add extra measurement bits

## Documentation Requirements

User-facing documentation must explicitly state the current behavior.

It should say all of the following clearly:

- `LOSS(p)` adds loss state but no measurement record output
- default measurements treat loss as result `1`
- `ML` / `MRL` family measurements expose loss by emitting a loss flag before the value bit
- multi-qubit instructions are handled per target group
- reset instructions recover lost qubits
- v1 supports execution and sampling only, not DEM extraction or loss-aware error analysis

## Out of Scope for v1

The first version intentionally excludes:

- `HERALDED_LOSS`
- loss-visible `MPP` / pair-measure variants
- analyzer / DEM support
- automatic insertion of loss noise in code generation
- bespoke semantics for every exotic instruction beyond the common execution and sampling path

If an uncommon instruction cannot be given a clean loss rule inside the sampling path, it is acceptable for v1 to return an explicit unsupported error instead of silently inventing behavior.
