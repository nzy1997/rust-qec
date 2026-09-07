# rstim

`rstim` is a Rust stabilizer-circuit simulator and detector-error-model (DEM)
toolkit. It parses Stim-style circuits, generates common QEC memory circuits,
samples measurements and detection events, and extracts DEMs for downstream
decoders.

The crate is being prepared for its first crates.io release; it is not yet
available from the registry. In this repository it is consumed through the
workspace path. After release, an application can use:

```toml
[dependencies]
rand = "0.8"
rstim = "0.2.1"
```

`rstim`'s sampling APIs accept RNGs from `rand` 0.8, so downstream crates that
seed their own RNG should depend on that major version explicitly.

```rust
use rand::{rngs::StdRng, SeedableRng};
use rstim::{parser::parse_lines, sampler::sample_batch};

let circuit = parse_lines("H 0\nCNOT 0 1\nM 0 1")?;
let mut rng = StdRng::seed_from_u64(42);
let samples = sample_batch(&circuit, 8, &mut rng)?;
for shot in 0..8 {
    assert_eq!(samples.measurements.get(0, shot), samples.measurements.get(1, shot));
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

There are currently no optional features required for the public simulator
API. The `benchmark-telemetry` and `benchmark-tools` features are for internal
benchmark instrumentation and worker binaries.
Detector error models must be decomposed into graphlike components before
passing them to a matching decoder that only accepts one- and two-detector
errors.

See the [getting-started guide](doc/getting_started.md), the repository's
[complete external-consumer example](../examples/rust-consumer/src/main.rs),
and the [rmatching decoder](../rmatching/README.md). The full API reference can
be built locally with `cargo doc -p rstim --open`.

Licensed under [Apache-2.0](../LICENSE).
