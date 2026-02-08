# Stim vs yao-rs: Missing Operations and Semantics (Current Snapshot)

This document summarizes **Stim operations not currently represented in `yao-rs`** (based on `Stim/src/stim/gates/gates.h` and `yao-rs/src/gate.rs`).
The list is grouped by category to help plan extensions or adapters.

## A. Non-unitary / Control-flow / Annotations (No direct yao-rs equivalent)
- Annotations: `DETECTOR`, `OBSERVABLE_INCLUDE`, `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`
- Control flow: `REPEAT`
- Record / padding: `MPAD` (padding), `rec[]` references (semantic dependency on measurement history)

## B. Measurement / Reset (Not present as first-class semantics in yao-rs)
- Single-qubit measurement: `M`, `MX`, `MY`
- Reset: `R`, `RX`, `RY`
- Measure + reset: `MR`, `MRX`, `MRY`
- Two-qubit joint measurements: `MXX`, `MYY`, `MZZ`
- Pauli string measurement / phase: `MPP`, `SPP`, `SPP_DAG`

## C. Noise Channels (No noise model in yao-rs today)
- `DEPOLARIZE1`, `DEPOLARIZE2`
- `X_ERROR`, `Y_ERROR`, `Z_ERROR`, `I_ERROR`, `II_ERROR`
- `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`
- `E` (alias `CORRELATED_ERROR`), `ELSE_CORRELATED_ERROR`
- `HERALDED_ERASE`, `HERALDED_PAULI_CHANNEL_1`

## D. Clifford / Named Gates Missing as Direct Variants
These could be expressed via decompositions or custom matrices, but are not named gates in `yao-rs`.

- Hadamard-like: `H_XY`, `H_YZ`, `H_NXY`, `H_NXZ`, `H_NYZ`
- Period-3: `C_XYZ`, `C_ZYX`, `C_NXYZ`, `C_XNYZ`, `C_XYNZ`, `C_NZYX`, `C_ZNYX`, `C_ZYNX`
- Period-4: `SQRT_X_DAG`, `SQRT_Y_DAG`, `S_DAG` (yao-rs has `S`/`T`)
- Two-qubit parity phases: `II`, `SQRT_XX`, `SQRT_XX_DAG`, `SQRT_YY`, `SQRT_YY_DAG`, `SQRT_ZZ`, `SQRT_ZZ_DAG`
- Swap-family: `CXSWAP`, `SWAPCX`, `CZSWAP`, `ISWAP_DAG`

## E. Controlled Gates with Non-Z Control Semantics
yao-rs supports controlled gates, but Stim includes variants that are not equivalent to standard Z-control without basis changes.

- `XCX`, `XCY`, `XCZ`, `YCX`, `YCY`, `YCZ`

## F. Circuit-Level Features Beyond Gates (Not in yao-rs today)
These are **circuit semantics and target types** supported by Stim that do not exist as first-class concepts in yao-rs.

- **Rich target types** in a single instruction:
  - Pauli targets like `X5`, `Y2`, `Z7`
  - Inverted targets `!5`, `!X3`, `!Z9`
  - Measurement record targets `rec[-k]`
  - Sweep bits `sweep[k]` (classical control input)
  - Combiner `*` for Pauli product specs (e.g., `MPP` targets)
- **Instruction tags** (arbitrary string metadata on each instruction)
- **Circuit stats** (counts of measurements, detectors, observables, ticks, lookback, etc.)
- **Circuit transforms**: `flattened()` (expand `REPEAT`/`SHIFT_COORDS`), `inverse()`, `without_noise()`, `without_tags()`
- **Instruction fusion** (combining compatible adjacent ops for performance)
- **Program-level APIs**: slicing, concatenation, repetition (`circuit * k`), equality/approx-equality

## Notes
- Some Stim operations are **aliases** of others (e.g., `CNOT` maps to `CX`, `MZ` maps to `M`).
- Several missing gates can be represented by **decomposition into existing gates** or by `Custom` matrices in yao-rs, but lack named variants and/or semantic support (especially for measurement, detector logic, and noise).
- Stim supports **per-qubit coordinate annotations** (`QUBIT_COORDS`, `SHIFT_COORDS`, plus `TICK`), enabling timeline visualization. yao-rs has only lightweight `Annotation` for circuit diagrams and does **not** track coordinates or time ticks as first-class data.
