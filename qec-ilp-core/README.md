# qec-ilp-core

Binary ILP model types, validation, and solver adapters shared by RustQEC code
distance checks and exact decoders. Most users should start with `qec-code` or
`rustqec-cli`; this package is useful when integrating directly with their ILP
models.

The first crates.io release is being prepared and is **not published yet**.
Use a reviewed RustQEC checkout with a path dependency until publication. The
planned registry dependency is `qec-ilp-core = "0.1.0"`.

This package compiles the HiGHS native solver by default, even though its Cargo
default feature list is empty. C/C++ build tools, CMake, and Clang/libclang are
required. The `gurobi` feature additionally requires a working Gurobi installation
and its runtime/license configuration. It is not enabled by the normal release.

Rust 1.88 is the minimum compiler; Ubuntu 24.04 x86_64 and macOS 15 Apple silicon
are the tested native environments. The package's public API is pre-1.0.

See the [source API](https://github.com/nzy1997/rust-qec/tree/master/qec-ilp-core/src),
[support contract](https://nzy1997.github.io/rust-qec/support/), and
[package guide](https://github.com/nzy1997/rust-qec/blob/master/docs/crates-io.md).
Licensed under Apache-2.0; see LICENSE in this package.
