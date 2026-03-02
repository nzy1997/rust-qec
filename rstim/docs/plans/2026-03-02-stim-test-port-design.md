# Stim Test Port Design

**Date:** 2026-03-02

## Goal

Port Stim's behavior tests to rstim to ensure correctness parity. Focus on simulator behavior, stabilizer algebra, circuit parsing, DEM, and codegen — skip SIMD internals, diagrams, search/decoder, CLI parsing, and I/O format details.

## Scope

~348 Stim tests from 9 test files:

| Stim test file | Tests | Category |
|---|---|---|
| `tableau_simulator.test.cc` | 81 | Gate correctness, measurement, reset, noise |
| `error_analyzer.test.cc` | 55 | DEM extraction correctness |
| `circuit.test.cc` | 55 | Parse, repr, equality, repeat blocks |
| `frame_simulator.test.cc` | 42 | Batch sampling, noise channels |
| `tableau.test.cc` | 34 | Tableau identity, gate application, composition |
| `pauli_string.test.cc` | 31 | Pauli multiplication, commutation, evolution |
| `detector_error_model.test.cc` | 29 | DEM parse, display, instructions |
| `measurements_to_detection_events.test.cc` | 15 | m2d conversion |
| `circuit_gen_params.test.cc` | 6 | Noise parameter validation |

## Approach

Direct port from C++ to Rust. Each Stim `TEST()` becomes a Rust `#[test]`. Test logic is translated to use rstim's API. Duplicates with existing rstim tests are skipped.

## File mapping

| Stim test file | rstim test file |
|---|---|
| `tableau_simulator.test.cc` | `tests/stim_tableau_simulator.rs` |
| `frame_simulator.test.cc` | `tests/stim_frame_simulator.rs` |
| `error_analyzer.test.cc` | `tests/stim_error_analyzer.rs` |
| `measurements_to_detection_events.test.cc` | `tests/stim_m2d.rs` |
| `tableau.test.cc` | `tests/stim_tableau.rs` |
| `pauli_string.test.cc` | `tests/stim_pauli_string.rs` |
| `circuit.test.cc` | `tests/stim_circuit.rs` |
| `detector_error_model.test.cc` | `tests/stim_dem.rs` |
| `circuit_gen_params.test.cc` | `tests/stim_codegen.rs` |

## Out of scope

- SIMD/memory internals (different data structures in Rust)
- Diagram/visualization tests (not implemented)
- Search/decoder tests (separate crate)
- CLI argument parsing tests
- I/O format detail tests
- Utility data structure tests
