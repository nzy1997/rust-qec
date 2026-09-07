# rilpqec

Experimental ILP decoding for `rstim` detector error models. This source-workspace
component is deferred from the first crates.io publication batch. Its public
model types are coupled to `rstim` and `qec-ilp-core` versions.

Use `cargo test --locked -p rilpqec` from a reviewed RustQEC checkout. The default
backend builds HiGHS and needs C/C++ tools, CMake, and Clang/libclang. The optional
`gurobi` feature requires a configured Gurobi installation and license.

Start with the [source API](https://github.com/nzy1997/rust-qec/tree/master/rilpqec/src)
and [support boundaries](https://nzy1997.github.io/rust-qec/support/).
Rust 1.88 minimum; Apache-2.0, with LICENSE included in this directory.
