+++
title = "Circuit and loss instructions"
template = "circuit-format.html"
[extra]
root = ".."
nav = "circuit-format"
+++
## Circuit syntax

A `.stim` file contains one instruction per line: `NAME(arguments) targets`. Arguments and targets depend on the instruction. `#` introduces a comment; `REPEAT N { ... }` repeats a block. Qubit targets are zero-based integers. Use `rstim circuit stats --in circuit.stim` to parse a file and inspect its measurement, detector and observable counts.

| Instruction | Role | Example |
| --- | --- | --- |
| `R`, `RX`, `RY` | Prepare a qubit in a Pauli basis; restores a lost atom | `R 0 1` |
| `H`, `S`, `CX`, `CZ` | Clifford evolution on available qubits | `CX 0 1` |
| `X_ERROR(p)`, `DEPOLARIZE1(p)`, `DEPOLARIZE2(p)` | Stochastic Pauli noise | `X_ERROR(0.01) 0` |
| `M`, `MX`, `MY`, `MR` | Measure, optionally reset | `M 0 1` |
| `DETECTOR` | Declare a measurement parity relative to the noiseless reference | `DETECTOR rec[-1]` |
| `OBSERVABLE_INCLUDE(k)` | Add measurement parity to logical observable k | `OBSERVABLE_INCLUDE(0) rec[-1]` |
| `TICK`, coordinates | Timing and drawing annotations | `TICK` |

A record target `rec[-k]` refers to the kth most recent stored bit at that point in execution. It is not a qubit index. Each detector and observable computes a parity; these declarations do not perform another measurement. The [Get started circuit](../get-started/#first-circuit) explains the example step by step.

RustQEC reads Stim-style syntax, but support differs by execution, analysis and export mode. The [support contract](../support/) scopes compatibility; accepted simulator instructions are not automatically accepted by every decoder. [Stim's upstream gate reference](https://github.com/quantumlib/Stim/wiki/Stim-v1.13-Gate-Reference) is background for overlapping instructions, not a promise of complete compatibility.

## Atom-loss extensions

`LOSS(p) q...` independently removes each still-present target with probability p. Loss persists until reset. Ideal single-qubit gates do nothing to absent atoms; a two-qubit interaction is skipped if either partner is absent. In the current executor, `DEPOLARIZE2` also skips an incomplete pair. These execution rules define this simulator's model and do not claim to describe all physical loss mechanisms.

| Instruction | Stored record | Reset |
| --- | --- | --- |
| `ML` / `MZL` | loss flag, then Z-value bit for each target | No |
| `MRL` / `MRZL` | loss flag, then Z-value bit for each target | Yes, after readout |
| `MXL`, `MYL` | loss flag, then X/Y-value bit | No |
| `MRXL`, `MRYL` | loss flag, then X/Y-value bit | Yes |

Flag 1 means the atom is absent at readout. An uninverted lost measurement stores value 1 as a placeholder, not a physical outcome. Reset restores the atom after recording the flag. The simulator's X/Y loss readouts do not extend the native loss decoder: v1 accepts Z loss readouts only. A decoder's result must not depend on the chosen placeholder.

## Blinded logical input marker

`TICK[rstim:logical_flip_point]` marks the insertion point for a private logical Pauli in a blinded dataset. It must occur exactly once at top level, after ideal preparation and before positive-probability noise. A comment with similar text is not the marker. The [training tutorial](../sampling-data/#marker-contract) explains how the labels and masks relate.

## Native loss decoder acceptance

The following is rendered from the canonical v1 contract. It defines parser and compiler acceptance, resource limits, bundle validation and failure codes. Acceptance of an external producer is determined by circuit conformance; release support promises additionally have a finite tested scope. See the [support boundary](../support/#atom-loss-support-boundary).
