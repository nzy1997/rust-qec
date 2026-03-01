# Getting Started Doc + Missing Features Design

**Date:** 2026-03-01

## Problem

rstim lacks a getting_started doc equivalent to Stim's `getting_started.ipynb`. Two features used in that tutorial are also missing:

1. `decompose_errors` option for `circuit_to_dem()` — needed by MWPM decoders
2. Per-channel noise parameters in codegen — Stim supports 4 independent noise channels, rstim uses a single `noise: f64`

## Feature 1: `decompose_errors`

### What it does

MWPM decoders require "graphlike" errors (at most 2 detectors per error component). The current `ErrorAnalyzer::circuit_to_dem()` can produce errors with 3+ detectors (e.g., from `DEPOLARIZE2`). `decompose_errors=true` decomposes these into graphlike components.

### Approach

Add `decompose_errors: bool` parameter to `circuit_to_dem()`. When true, post-process the DEM:

1. Identify all non-graphlike errors (3+ detectors in a single component)
2. Build a map of known graphlike errors: `{detector_set} -> error`
3. For each non-graphlike error, find a combination of graphlike errors whose detector sets XOR to match
4. Replace the non-graphlike error with decomposed components separated by `^`
5. Return error if decomposition fails

### API change

```rust
// Before
pub fn circuit_to_dem(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String>

// After
pub fn circuit_to_dem(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String>
pub fn circuit_to_dem_decomposed(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String>
```

### DEM changes needed

`DemTarget` needs a `Separator` variant (already exists). Errors with `^` separators represent independent components that happen to share the same probability.

## Feature 2: Per-Channel Noise in Codegen

### Current state

```rust
pub fn rotated_memory_z(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr>
```

Single `noise` value applied uniformly to all channels.

### Stim's 4 noise channels

| Parameter | Noise instruction | Applied where |
|---|---|---|
| `before_round_data_depolarization` | `DEPOLARIZE1` | Data qubits at round start |
| `after_clifford_depolarization` | `DEPOLARIZE1`/`DEPOLARIZE2` | After 1-qubit/2-qubit gates |
| `before_measure_flip_probability` | `X_ERROR` | Before measurement |
| `after_reset_flip_probability` | `X_ERROR` | After reset |

### Approach

```rust
pub struct NoiseParams {
    pub before_round_data_depolarization: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
}

impl NoiseParams {
    pub fn uniform(noise: f64) -> Self { /* all fields = noise */ }
}
```

Update codegen functions to accept `NoiseParams`. Keep the existing single-`f64` API as a convenience wrapper that calls `NoiseParams::uniform()`.

### Affected codegen functions

- `repetition_code_memory(distance, rounds, noise)` -> add `_with_params` variant
- `rotated_memory_x/z(distance, rounds, noise)` -> add `_with_params` variant
- `unrotated_memory_x/z(distance, rounds, noise)` -> add `_with_params` variant
- `color_code::memory_xyz(distance, rounds, noise)` -> add `_with_params` variant

## Feature 3: Getting Started Doc

### Format

`doc/getting_started.md` with embedded Rust code blocks. Each section is a complete runnable example.

### Sections (mirroring Stim's tutorial)

1. **Parse/build a circuit** — `parse_lines()` and `StimInstr::new()`
2. **Sample measurements** — `sample_batch()` with `BatchOutput.measurements`
3. **Add detectors, sample them** — detector annotations, `BatchOutput.detections`
4. **Generate QEC circuits** — `repetition_code_memory()`, `rotated_memory_z()` with `NoiseParams`
5. **Extract DEM** — `circuit_to_dem_decomposed()`
6. **Decode with rmatching** — `rmatching::Matching` from DEM, `decode_batch()`
7. **Estimate threshold** — Monte Carlo loop over distances/noise rates
8. **Use rsinter** — `rsinter::Task`, `rsinter::collect()`, `shot_error_rate_to_piece_error_rate()`

### Skipped (per user request)

- All visualization (diagrams, SVG, 3D)
