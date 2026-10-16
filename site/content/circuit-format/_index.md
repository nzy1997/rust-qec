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

RustQEC reads Stim-style syntax. Accepted instructions vary by execution, analysis, export and decoder; see the [support contract](../support/). For shared instructions, consult [Stim's gate reference](https://github.com/quantumlib/Stim/wiki/Stim-v1.13-Gate-Reference).

## Atom-loss extensions

The simulator uses a gate-cancellation model: `LOSS(p) q...` independently removes each still-present target with probability p, and loss persists until reset. Ideal single-qubit gates do nothing to absent atoms; a two-qubit interaction is skipped if either partner is absent. `DEPOLARIZE2` also skips an incomplete pair.

`M` and `ML` both measure in the Z basis when the atom is present. Their stored records differ: `M` writes one value bit, while `ML` writes a loss flag followed by a value bit for each target. For plain, uninverted readout in this simulator:

| Instruction | Atom present | Atom absent |
| --- | --- | --- |
| `M` | `0` or `1`: measurement outcome | `1`: placeholder, with no loss flag |
| `ML` | `00` or `01`: flag 0, then measurement outcome | `11`: flag 1, then placeholder |

A `1` from `M` alone cannot distinguish loss from an ordinary measurement outcome. With `ML`, flag 1 identifies loss and tells the decoder to ignore the accompanying value as measurement evidence; it does not reveal exactly when the atom was lost. Result inversion changes the value bit, not the loss flag.

**Experimental readout.** An example of the binary information represented by `M` is state-selective push-out followed by fluorescence imaging: atoms in |1⟩ are expelled, so an atom lost earlier produces the same absence signal as |1⟩. See [Levine et al., *Phys. Rev. Lett.* 123, 170503 (2019)](https://arxiv.org/abs/1908.06101v2), especially the Supplemental Material's “State readout through atom loss” section.

`ML` represents readout that distinguishes |0⟩, |1⟩ and atom loss. [Bluvstein et al., arXiv:2506.20661 (2025)](https://arxiv.org/html/2506.20661v1) demonstrate **spin-to-position conversion**: a state-selective one-dimensional optical lattice pins one spin state while optical tweezers move the other to a separate position. Imaging then identifies the state from the atom's position, or detects loss if neither position contains an atom (Fig. 1b, Extended Data Fig. 2 and Methods).

Another implementation uses low-loss fluorescence state detection followed by a state-insensitive presence check; see [Chow et al., *PRX Quantum* 5, 040343 (2024)](https://journals.aps.org/prxquantum/abstract/10.1103/PRXQuantum.5.040343), Sec. I A and Appendix A7.

| Instruction | Stored record | Reset |
| --- | --- | --- |
| `ML` / `MZL` | loss flag, then Z-value bit for each target | No |
| `MRL` / `MRZL` | loss flag, then Z-value bit for each target | Yes, after readout |
| `MXL`, `MYL` | loss flag, then X/Y-value bit | No |
| `MRXL`, `MRYL` | loss flag, then X/Y-value bit | Yes |

Reset restores the atom after recording the flag. The simulator supports X/Y loss readouts; the native v1 loss decoder accepts Z loss readouts only. Its result must be independent of the lost-value placeholder.

Generated Mid-SWAP circuits keep physical wire identities fixed and exchange data/check roles. `MIDSWAP_SHUTTLE` comments record the role changes; the generator does not insert a literal `SWAP` instruction.

## Blinded logical input marker

`TICK[rstim:logical_flip_point]` marks the insertion point for a private logical Pauli in a blinded dataset. It must occur exactly once at top level, after ideal preparation and before positive-probability noise. A comment with similar text is not the marker. The [training tutorial](../sampling-data/#marker-contract) explains how the labels and masks relate.

## Native loss decoder acceptance

The v1 contract below specifies accepted circuits, resource limits, bundle validation and failure codes. Release support is documented in the [support table](../support/#atom-loss-support-boundary).
