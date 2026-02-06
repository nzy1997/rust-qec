# rstim

[![CI](https://github.com/nzy1997/rstim/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rstim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rstim/graph/badge.svg)](https://codecov.io/gh/nzy1997/rstim)

A Rust implementation of Stim-like stabilizer circuit simulation.

## Current Features
- Clifford/stabilizer simulator with basic gates and measurements
- Detector/observable semantics with `rec[]`
- `REPEAT` blocks and case-insensitive parsing
- Coordinate annotations: `QUBIT_COORDS`, `SHIFT_COORDS`, `TICK`
- Pauli noise channels: `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1/2`
