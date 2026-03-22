# rstim

[![CI](https://github.com/nzy1997/rstim/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rstim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rstim/branch/master/graph/badge.svg)](https://codecov.io/gh/nzy1997/rstim)

A Rust implementation of Stim-like stabilizer circuit simulation.

## Current Features
- Clifford/stabilizer simulator with basic gates and measurements
- Detector/observable semantics with `rec[]`
- `REPEAT` blocks and case-insensitive parsing
- Coordinate annotations: `QUBIT_COORDS`, `SHIFT_COORDS`, `TICK`
- Pauli noise channels: `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1/2`

## CLI Workflow

`rstim` already includes a CLI for inspecting, sampling, analyzing, generating,
and exporting circuits.

Use `rstim stats` to inspect a circuit before running heavier workflows:

```sh
printf 'H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n' | rstim stats
```

Example output:

```text
instruction_count: 5
repeat_blocks: 1
max_repeat_depth: 1
num_qubits: 1
num_measurements: 2
num_detectors: 2
num_observables: 0
num_ticks: 2
num_sweep_bits: 0
```

For machine-readable output:

```sh
printf 'M 0\nDETECTOR rec[-1]\n' | rstim stats --json
```

For the full CLI reference, including `sample`, `detect`, `analyze_errors`,
`convert`, `m2d`, `gen`, `sample_dem`, `explain_errors`, and `export_json`, see
[`rstim/doc/cli.md`](rstim/doc/cli.md) and
[`rstim/doc/getting_started.md`](rstim/doc/getting_started.md).

## Stim Parity Showcase

On six representative `repetition_code` and rotated `surface_code` cases (`d=5`
and `d=13`), `rstim` matches `stim` on:

- noiseless generated-circuit structure (`Gen = normalized`)
- noisy `analyze_errors` detector error model semantics (`DEM = match`)

For `Gen`, `normalized` means the generated circuits are structurally equivalent
after stripping Stim's comment preamble and comparing normalized instruction
summaries instead of raw text. For `DEM`, both tools analyze the same noisy
circuit produced by:

```sh
stim gen ... --after_clifford_depolarization 0.001
```

The timing numbers below are median wall-clock times over 5 local runs on one
development machine, so they should be treated as illustrative instead of a
portable benchmark.

| Case | Gen | DEM | Max Rel Error | Stim Gen ms | rstim Gen ms | Stim DEM ms | rstim DEM ms | Gen Ratio | DEM Ratio |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| repetition_code/memory d=5 r=5 | normalized | match | 2.032e-16 | 18.794 | 1.852 | 19.482 | 2.449 | 0.10x | 0.13x |
| repetition_code/memory d=13 r=13 | normalized | match | 2.032e-16 | 19.161 | 2.085 | 20.546 | 7.318 | 0.11x | 0.36x |
| surface_code/rotated_memory_x d=5 r=5 | normalized | match | 4.068e-16 | 19.085 | 2.026 | 21.966 | 12.535 | 0.11x | 0.57x |
| surface_code/rotated_memory_x d=13 r=13 | normalized | match | 4.068e-16 | 20.292 | 7.518 | 100.010 | 271.136 | 0.37x | 2.71x |
| surface_code/rotated_memory_z d=5 r=5 | normalized | match | 4.076e-16 | 18.894 | 1.956 | 21.725 | 12.455 | 0.10x | 0.57x |
| surface_code/rotated_memory_z d=13 r=13 | normalized | match | 4.076e-16 | 20.485 | 7.518 | 99.428 | 268.938 | 0.37x | 2.70x |

To reproduce the table locally:

```sh
cargo build -p rstim --bin rstim
cargo run -p rstim --example stim_parity_showcase
```
