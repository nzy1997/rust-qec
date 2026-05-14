# rstim

[![CI](https://github.com/nzy1997/rstim/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rstim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rstim/branch/master/graph/badge.svg)](https://codecov.io/gh/nzy1997/rstim)

A Rust implementation of Stim-like stabilizer circuit simulation.

## Current Features
- Clifford/stabilizer simulator with basic gates and measurements
- Atom-loss-aware simulation with `LOSS`, loss-visible measurement gates, and loss-caused measurement outcomes
- Detector/observable semantics with `rec[]`
- `REPEAT` blocks and case-insensitive parsing
- Coordinate annotations: `QUBIT_COORDS`, `SHIFT_COORDS`, `TICK`
- Pauli noise channels: `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1/2`
- QP101 export for both raw circuits and single-shot sample overlays

## CLI Workflow

`rstim` already includes a CLI for inspecting, sampling, analyzing, generating,
and exporting circuits.

## Atom Loss Workflow

Atom loss is a first-class workflow in `rstim`, not an afterthought bolted onto
generic Pauli noise. The simulator can model explicit `LOSS` events, carry that
state through later gates, and distinguish between an ordinary `1` measurement
and a `1` that happened because the atom was lost.

For example, this circuit injects a Pauli error on `q0`, loses atoms on `q1`
and `q2`, then records both a normal measurement and a loss-visible
measurement:

```stim
DEPOLARIZE1(1) 0
LOSS(1) 1
LOSS(1) 2
M 1
MRL 2
DETECTOR rec[-3]
```

You can export one seeded sample shot as QP101 JSON with inline annotations:

```sh
cargo run -p rstim --bin rstim -- export_json --sample_shot --seed 7 < qp101-viz/examples/atom-loss-sample.stim
```

That shot marks the fired depolarizing branch, the two loss events, the `1[L]`
measurement caused by loss, the `MRL` pair `L=1 | M=1[L]`, and the flipped
detector. The matching `qp101-viz` demo lives at
[`qp101-viz/examples/atom-loss-sample.typ`](qp101-viz/examples/atom-loss-sample.typ)
with its exported sample result in
[`qp101-viz/examples/atom-loss-sample.qp101.json`](qp101-viz/examples/atom-loss-sample.qp101.json).

For a larger example, see
[`qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ`](qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ),
which shows both the source circuit and one seeded sample shot for a rotated
surface-code memory-X experiment with `d=3`, `r=3`, and atom-loss noise
inserted onto every data qubit at the start of each round. This example uses
the default measurement path, so loss-caused outcomes appear as `1[L]` on the
ordinary measurement gates instead of a separate loss-flag/value pair.

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
