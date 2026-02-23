# RStim Roadmap: Toward Full Stim Compatibility

**Goal:** Bring rstim to feature parity with [Stim](https://github.com/quantumlib/Stim), the reference C++ stabilizer circuit simulator.

**Current state:** rstim implements a functional Clifford/stabilizer simulator with basic gates (H, S, X, Y, Z, CX, CZ), measurements (M, MX, MY), Pauli noise (X_ERROR, Z_ERROR, DEPOLARIZE1/2), annotations (DETECTOR, OBSERVABLE_INCLUDE), REPEAT blocks, and coordinate tracking.

---

## Feature Comparison

| Area | rstim today | Stim |
|------|-------------|------|
| Tableau simulator | Basic (2n×n) | Inverted tableau, SIMD-optimized |
| Gates | 7 | 82 |
| Measurements | M, MX, MY | M, MX, MY, MR, MRX, MRY, MPP, MXX, MYY, MZZ, MPAD |
| Resets | — | R, RX, RY |
| Noise channels | 4 | 12+ |
| Frame simulator | — | Full Pauli frame (batch SIMD) |
| Detector error model | — | Full DEM format, conversion, sampling |
| Error analysis | — | Circuit→DEM, error matching |
| Output formats | — | 6 formats (01, b8, ptb64, hits, r8, dets) |
| CLI | version only | sample, detect, analyze_errors, gen, diagram, repl |
| Circuit transforms | — | flattened, inverse, without_noise, without_tags |

---

## Phase 1 — Complete Clifford Gate Set + Resets

> **Detailed plan:** `docs/plans/2026-02-22-phase1-clifford-gates.md`

Add the remaining Clifford gates so rstim can execute most real `.stim` circuit files.

**Deliverables:**
- Single-qubit gates: `I`, `S_DAG`, `SQRT_X`, `SQRT_X_DAG`, `SQRT_Y`, `SQRT_Y_DAG`
- Hadamard variants: `H_XY`, `H_YZ`
- Two-qubit gates: `CY`, `SWAP`, `ISWAP`, `ISWAP_DAG`
- Controlled variants: `XCX`, `XCY`, `XCZ`, `YCX`, `YCY`, `YCZ`
- Swap-controlled gates: `CXSWAP`, `CZSWAP`, `SWAPCX`
- Resets: `R`, `RX`, `RY`
- Measure+reset: `MR`, `MRX`, `MRY`
- Noise: `Y_ERROR`

---

## Phase 2 — MPP and Pauli Product Targets

`MPP` (multi-Pauli-product measurement) is heavily used in real QEC circuits (surface code, color code). This phase adds Pauli-string targets to the IR, parser, and simulator.

**Deliverables:**
- Pauli target types in IR: `PauliX(u32)`, `PauliY(u32)`, `PauliZ(u32)`
- Combiner target (`*`) in parser for Pauli product specs
- `MPP` implementation in tableau simulator
- `SPP`, `SPP_DAG` (Pauli product phase gates)
- Pair measurements: `MXX`, `MYY`, `MZZ`
- `MPAD` (measurement record padding)

---

## Phase 3 — Remaining Noise Channels

Complete the noise model to match Stim's full set.

**Deliverables:**
- `Y_ERROR` (if not done in Phase 1), `I_ERROR`, `II_ERROR`
- `CORRELATED_ERROR` / `ELSE_CORRELATED_ERROR` (multi-qubit correlated Pauli errors with Pauli targets)
- `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2` (general single/two-qubit Pauli channels)
- `HERALDED_ERASE`, `HERALDED_PAULI_CHANNEL_1`

---

## Phase 4 — Frame Simulator (Performance)

The Pauli frame simulator is Stim's performance secret: it processes many shots in parallel using bit-packed SIMD operations, avoiding the O(n²) per-gate cost of the tableau simulator.

**Deliverables:**
- `FrameSimulator` with batch X/Z frame tracking (bit-packed `u64` or SIMD)
- Batch `sample(circuit, n_shots)` API
- Detection event and observable flip output from frame sim
- Compiled sampler: pre-analyze the circuit once, then sample cheaply

---

## Phase 5 — Detector Error Model (DEM)

DEM is the bridge between circuit-level noise and decoders. This phase adds the DEM IR, circuit→DEM conversion, and DEM file I/O.

**Deliverables:**
- DEM IR: `DemInstruction` (error, detector, logical_observable), `DetectorErrorModel`
- `ErrorAnalyzer`: backward propagation of errors through the circuit to produce a DEM
- DEM file format: parser and writer (`.dem` files)
- DEM sampler: sample detection events directly from a DEM
- Circuit→DEM conversion API

---

## Phase 6 — CLI and Output Formats

Add a usable command-line interface for sampling, error analysis, and format conversion.

**Deliverables:**
- `rstim sample`: sample measurement results from circuits
- `rstim detect`: convert measurements to detection events
- `rstim analyze_errors`: convert circuits to detector error models
- Output formats: `01` (dense text), `b8` (binary), `dets` (sparse detector), `hits` (sparse indices), `r8` (run-length)
- `rstim convert`: convert between output formats
- Structured CLI with `clap`

---

## Phase 7 — Polish and Extended Features

Round out the feature set with circuit transforms, code generation, and remaining exotic gates.

**Deliverables:**
- Circuit transforms: `flattened()`, `inverse()`, `without_noise()`, `without_tags()`
- `sweep[]` targets (classical control input for batch experiments)
- Instruction tags (arbitrary string metadata on instructions)
- `stim gen` equivalent: generate common QEC circuits (repetition code, surface code)
- Remaining exotic gates: period-3 (C_XYZ family), additional Hadamard variants (H_NXY, H_NXZ, H_NYZ)
- Circuit statistics API (measurement count, detector count, etc.)
- Remove unused `yao-rs` dependency from `Cargo.toml`

---

## Dependency Graph

```
Phase 1 (gates/resets)
  └─► Phase 2 (MPP/Pauli targets)
        └─► Phase 3 (noise channels)
              └─► Phase 4 (frame simulator)
                    └─► Phase 5 (DEM)
                          └─► Phase 6 (CLI)
                                └─► Phase 7 (polish)
```

Phases 1–3 are independent of each other at the code level but are ordered by priority. Phases 4+ build on earlier phases.
