# qec-ilp-core

Binary ILP model types, validation, and solver adapters shared by RustQEC code
distance checks and exact decoders. Most users should start with `qec-code` or
`rustqec-cli`; this package is useful when integrating directly with their ILP
models.

The first crates.io release is being prepared and is **not published yet**.
Use a reviewed RustQEC checkout with a path dependency until publication. The
planned registry dependency is `qec-ilp-core = "0.1.0"`.

The default feature set contains only backend-neutral model types and validation;
it does not compile or link a native solver. Enable a backend explicitly:

```toml
qec-ilp-core = { version = "0.1.0", features = ["highs"] }
```

- `highs` enables the open-source HiGHS adapter and its native `highs` and
  `highs-sys` dependencies. C/C++ build tools, CMake, and Clang/libclang are
  required.
- `gurobi` enables only the Gurobi adapter and requires a working Gurobi
  installation plus its runtime/license configuration.

Code that previously relied on HiGHS being linked implicitly must add the
`highs` feature. Requesting `Auto`, `Highs`, or `Gurobi` without a usable enabled
backend returns `BinaryIlpError::BackendUnavailable`.

Rust 1.88 is the minimum compiler; Ubuntu 24.04 x86_64 and macOS 15 Apple silicon
are the tested native environments. The package's public API is pre-1.0.

See the [source API](https://github.com/nzy1997/rust-qec/tree/master/qec-ilp-core/src),
[support contract](https://nzy1997.github.io/rust-qec/support/), and
[package guide](https://github.com/nzy1997/rust-qec/blob/master/docs/crates-io.md).
Licensed under Apache-2.0; see LICENSE in this package.
