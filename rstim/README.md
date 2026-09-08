# rstim

`rstim` is a Rust stabilizer-circuit simulator and detector-error-model (DEM)
toolkit. It parses Stim-style circuits, generates common QEC memory circuits,
samples measurements and detection events, and extracts DEMs for downstream
decoders.

Add the simulator library from crates.io:

```toml
[dependencies]
rand = "0.8"
rstim = "0.3.0"
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

No optional feature is required for the simulator library API. Reusable
file-independent command operations live in `rstim::operations`.

Optional features are intentionally additive:

- `cli` builds the compatibility `rstim` executable and argument parser.
- `codegen-css` enables CSS circuit generation backed by `qec-code`.
- `shot-viewer` embeds the browser viewer and exposes its loopback server API.
- `benchmark-tools` builds benchmark worker binaries and only adds their
  argument parser dependency; it does not enable the compatibility CLI, CSS
  generation, or the viewer.
- `benchmark-telemetry` enables internal benchmark instrumentation.

For example, install the complete compatibility executable with
`cargo install rstim --version 0.3.0 --features cli,codegen-css,shot-viewer`.
A smaller CLI can be installed with only `--features cli`; unavailable optional
subcommands are omitted or report the feature needed to enable them.
Detector error models must be decomposed into graphlike components before
passing them to a matching decoder that only accepts one- and two-detector
errors.

See the [getting-started guide](doc/getting_started.md), the repository's
[complete external-consumer example](https://github.com/nzy1997/rust-qec/blob/master/examples/rust-consumer/src/main.rs),
and the [rmatching decoder](https://github.com/nzy1997/rust-qec/tree/master/rmatching).
The full API reference is available on [docs.rs](https://docs.rs/rstim).

Licensed under [Apache-2.0](LICENSE).
