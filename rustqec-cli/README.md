# rustqec-cli

The `rustqec` command-line application for inspecting and sampling quantum
error-correction circuits, producing datasets, and loss-aware decoding.

Install the versioned command-line package from crates.io:

```sh
cargo install --locked rustqec-cli --version 0.1.0
```

This package installs only the `rustqec` executable; dependency crates do not
install their CLIs.

## First circuit

```sh
rustqec capabilities --format json
printf 'R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n' > circuit.stim
rustqec circuit stats --format json --in circuit.stim
rustqec circuit detect --in circuit.stim --shots 1 --out-format dets --append-observables --out events.dets
rustqec circuit dem --in circuit.stim --out model.dem
cat events.dets model.dem
```

The final file contents are `shot D0 L0` and `error(1) D0 L0`. No `rstim`
executable or source checkout is needed to run this workflow after installation.

## Features and support

- Default: circuit inspection/generation/sampling, dataset tools, and
  `envelope-matching`. This dependency selection does not include HiGHS.
- `ilp`: adds exact `envelope-mle`, compiling the HiGHS solver. Install native
  C/C++ build tools, CMake, and Clang/libclang before using this feature.
- Official native archives include both decoders. Run `capabilities --format json`
  on the actual binary to discover its supported choices and error contracts.

Rust 1.88 is the minimum supported compiler. Tested native environments are
Ubuntu 24.04 x86_64 and macOS 15 Apple silicon. Decoder acceptance is limited by
the [support contract](https://nzy1997.github.io/rust-qec/support/); envelope
decoding is beta, not a universal decoder for arbitrary loss circuits.

For the separate Stim-style command, use the `rstim` package and select its
`rstim` binary. See the [getting-started guide](https://nzy1997.github.io/rust-qec/get-started/)
and [publishing guide](https://github.com/nzy1997/rust-qec/blob/master/docs/crates-io.md).

Licensed under Apache-2.0; see LICENSE in this package.
