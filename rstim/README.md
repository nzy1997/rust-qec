# rstim

`rstim` is a Rust stabilizer-circuit simulator and detector-error-model (DEM)
toolkit. It parses Stim-style circuits, generates common QEC memory circuits,
samples measurements and detection events, and extracts DEMs for downstream
decoders.

Add the simulator library from crates.io:

```toml
[dependencies]
rand = "0.8"
rstim = { version = "0.3.0", default-features = false }
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

No optional feature is required for the simulator library API. Library-only
consumers can disable the default CLI features with `default-features = false`.
Reusable file-independent command operations live in `rstim::operations`.

Optional features are intentionally additive:

- `unified-cli` adds structured circuit, dataset, decoding, and capability commands.
- `cli` builds the simulator commands and `rstim` executable.
- `ilp` adds exact envelope MLE through HiGHS.
- `codegen-css` enables CSS circuit generation backed by `qec-code`.
- `shot-viewer` embeds the browser viewer and exposes its loopback server API.
- `plotting` adds the self-contained `rstim surface-code-ler` experiment and SVG plot.
- `benchmark-tools` builds benchmark workers and examples.
- `benchmark-telemetry` enables internal benchmark instrumentation.

The development checkout combines all public commands in one executable.
Install it with `cargo install --locked --path rstim --force`. This combined release
has not yet been published to crates.io. A smaller simulator-only CLI can be
built with `--no-default-features --features cli`.
After installation, run `rstim surface-code-ler --distances 3,5,7 --rounds 9,15,21
--physical-error-rates 0.008,0.009,0.01,0.011,0.012 --shots 2000 --out-dir .`
from any working directory to produce a CSV and SVG plot.
Detector error models must be decomposed into graphlike components before
passing them to a matching decoder that only accepts one- and two-detector
errors.

See the [getting-started guide](doc/getting_started.md), the
[experimental near-Clifford backend contract](doc/near-clifford.md),
the repository's
[complete external-consumer example](https://github.com/nzy1997/rust-qec/blob/master/examples/rust-consumer/src/main.rs),
and the [rmatching decoder](https://github.com/nzy1997/rust-qec/tree/master/rmatching).
The full API reference is available on [docs.rs](https://docs.rs/rstim).

Licensed under [Apache-2.0](LICENSE).
