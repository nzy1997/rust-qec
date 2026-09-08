# rsinter

Experimental parallel sampling, frozen-dataset replay, and benchmark collection
for RustQEC decoder comparisons. This package is not part of the first crates.io
publication batch; use a reviewed source checkout.

The default feature selection contains the harness without optional decoder
runners or plotting. Enable the capabilities needed by the experiment:

| Feature | Capability |
| --- | --- |
| `rbposd-runner` | BP-OSD decoding |
| `rmatching-runner` | MWPM decoding |
| `ilp-runner` | ILP decoding with native HiGHS build requirements |
| `plotting` | Plots, including native font dependencies |
| `full` | All the above |
| `gurobi` | Optional configured Gurobi backend |

For the previous full development setup, use
`cargo build --locked -p rsinter --features full`. For a small BP-OSD experiment,
use `cargo build --locked -p rsinter --features rbposd-runner`.

See [replay workflows](https://github.com/nzy1997/rust-qec/blob/master/docs/rsinter-replay.md)
and the [benchmark guide](https://nzy1997.github.io/rust-qec/benchmarks/).
Rust 1.88 minimum; Apache-2.0, with LICENSE included in this directory.
