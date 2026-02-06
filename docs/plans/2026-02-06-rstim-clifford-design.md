# RStim Clifford Simulator Design

Date: 2026-02-06

## Goal
Implement a Stim‑like Clifford/stabilizer simulator in `rstim` (no `yao-rs` state evolution), supporting key Stim semantics: Clifford gates, measurements, noise channels, `DETECTOR/OBSERVABLE`, `REPEAT`, `TICK`, and coordinate annotations.

## Scope
- Full `.stim` parsing (case‑insensitive names, tags, arguments, targets)
- Clifford gate simulation using a stabilizer tableau
- Measurement (`M`, `MX`, `MY`, `MPP`) and `rec[]`
- Control flow: `REPEAT` (nesting allowed)
- Annotations: `DETECTOR`, `OBSERVABLE_INCLUDE`, `QUBIT_COORDS`, `SHIFT_COORDS`, `TICK`
- Noise: Pauli channels (`X_ERROR`, `Z_ERROR`, `DEPOLARIZE1/2`, `CORRELATED_ERROR` subset)

## Architecture
### Layers
1. **Parser** → produces `ir::StimProgram`
2. **IR** → instruction stream + nested `Repeat` blocks
3. **Simulator** → `sim::StabilizerState` (tableau) + `sim::Runner`
4. **Report** → results: measurements, detectors, observables

### Core Types
- `StimProgram` → `Vec<StimInstr>` (includes `Repeat { count, body }`)
- `StimInstr` → name, args, targets, tag
- `Recorder` → measurement record for `rec[-k]`
- `CoordState` → `TICK` and `SHIFT_COORDS` tracking
- `StabilizerState` → Clifford tableau operations

### Execution Flow
- Parse `.stim` → IR
- Runner executes instructions in order
  - Clifford gates → update tableau
  - Measurement → sample result, collapse, write to recorder
  - Noise → sample Pauli and apply
  - DETECTOR/OBSERVABLE → XOR `rec[]` references into outputs
  - REPEAT → recursive execution
  - TICK/SHIFT_COORDS → update coord metadata

## Error Handling
- Parse errors: syntax, args/targets, illegal `REPEAT 0`, invalid `rec[]` syntax
- Runtime errors: `rec[]` out of range, unsupported instruction, invalid target types
- All errors should include line number + snippet

## Testing
- Parsing: case‑insensitive names, tags, args, `REPEAT` nesting
- Stabilizer semantics: Bell state, deterministic correlations
- Measurement: distribution + collapse correctness
- Noise: frequency approx. in multi‑shot tests
- Annotations: XOR semantics for `DETECTOR` and indexed observables
- Control flow: `REPEAT` correctness with `rec[]`

## Milestones
1. **M1**: Stabilizer tableau + basic gates + `M` + recorder
2. **M2**: `DETECTOR/OBSERVABLE` + `REPEAT` + `TICK/SHIFT_COORDS`
3. **M3**: Noise channels (Pauli subset)
4. **M4**: `MX/MY/MPP` + expanded gate set
