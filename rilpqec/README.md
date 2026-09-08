# rilpqec

ILP decoding of detector error models (DEMs), using `rstim`'s DEM parser and
`qec-ilp-core`'s native solver adapters. Use it for exact error-configuration
optimization on small or moderate models where solver cost is acceptable.
It does not sum probabilities over logically equivalent error configurations.

## Install and decode

```toml
[dependencies]
rilpqec = "0.3.0"
rstim = "0.3.0"
```

HiGHS is included in every `rilpqec` build. Install C/C++ build tools, CMake,
and Clang/libclang first (Ubuntu: `build-essential cmake clang libclang-dev`;
macOS: Xcode Command Line Tools plus `brew install cmake llvm`). This is a
native solver package, including when `default-features = false` is used.

```rust
use rilpqec::{BackendKind, IlpDecoderConfig, IlpDemDecoder};
use rstim::dem::DetectorErrorModel;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\n")?;
    let mut config = IlpDecoderConfig::default();
    config.backend.kind = BackendKind::Highs;
    config.backend.threads = Some(1);
    let mut decoder = IlpDemDecoder::from_dem(&dem, config)?.into_compiled()?;
    let predictions = decoder.decode_batch_bit_packed(&[0, 1, 1], 3, 1, 1)?;
    assert_eq!(predictions, vec![0, 1, 1]);
    // Reuse the same solver model for another batch.
    assert_eq!(decoder.decode_batch_bit_packed(&[1, 0], 2, 1, 1)?, vec![1, 0]);
    Ok(())
}
```

Inputs and outputs are shot-major and LSB-first, with `ceil(width / 8)` bytes
per shot. Detector and observable widths must match the DEM; unused high bits
in the final detector byte are ignored. The method returns observable
predictions, not physical-qubit corrections. The direct `IlpDemDecoder` batch
method rebuilds its solver each call; use `into_compiled` for repeated batches.

## Errors and solver selection

Handle the returned `Result`: mismatched widths, malformed buffer lengths,
overflowing sizes, allocation failures, and solver failures return errors.
A deterministic DEM (probabilities zero or one) rejects incompatible syndromes
with `IlpDecodeError::InfeasibleSyndrome { shot }`, where `shot` is zero-based.
Infeasible nontrivial models return a backend error. DEM probabilities must be
finite and within `[0, 1]`.

The default backend is `Auto`. Without the `gurobi` feature it selects HiGHS;
with that feature it tries Gurobi and falls back to HiGHS if initialization fails.
Set `config.backend.kind` explicitly when reproducibility requires one backend.
The `gurobi` feature adds Gurobi support and requires a separate installation and
license; it **does not remove HiGHS**. `BackendConfig` also exposes
`time_limit_seconds`, `mip_gap`, `threads`, and `verbose`. A time or gap limit
can return a feasible incumbent without an optimality proof. The prediction
API does not expose solver status, so a successful limited run must not be
interpreted as proof of optimality. Runs without a usable solution return errors.

Rust 1.88 is the minimum compiler. Tested native environments are Ubuntu 24.04
x86_64 and macOS 15 Apple silicon. This pre-1.0 package has an evolving API;
publication does not extend its numerical acceptance evidence.

See the [API](https://docs.rs/rilpqec),
[package guide](https://github.com/nzy1997/rust-qec/blob/master/docs/crates-io.md),
and [support contract](https://nzy1997.github.io/rust-qec/support/).
Licensed under Apache-2.0; see [LICENSE](https://github.com/nzy1997/rust-qec/blob/master/LICENSE).
