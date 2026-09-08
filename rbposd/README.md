# rbposd

`rbposd` provides belief-propagation decoders with ordered-statistics (OSD) or
localized-statistics (LSD) post-processing for binary parity-check matrices.
It exposes sparse parity-check matrices, channel models, decoder configuration,
syndrome and correction types, and reusable BP+OSD/BP+LSD decoders.

The crate is currently source-only: its manifest disables publishing, and it
is consumed through the workspace path in this repository. If a future release
enables crates.io publishing, an application will be able to use:

```toml
[dependencies]
rbposd = "0.3.0"
```

```rust
use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

let checks = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1]])?;
let decoder = BpOsdDecoder::new(
    checks.clone(),
    ChannelModel::Bsc { error_rate: 0.05 },
    DecoderConfig::default(),
)?;
let syndrome = Syndrome::from(vec![true]);
let result = decoder.decode(&syndrome)?;
assert_eq!(checks.multiply(&result.correction), syndrome);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate currently has no Cargo features. Callers supply a parity-check matrix
and channel model directly; `rbposd` does not parse detector error models.
Converting a circuit or DEM into those inputs belongs in an integration layer
such as the workspace's `rsinter` crate.

See the [complete BP+OSD example](examples/basic_decode.rs), the BP+LSD example
in the [crate-level API documentation](src/lib.rs), and the [workspace
integration crate](../rsinter).
The full API reference can be built locally with `cargo doc -p rbposd --open`.

Licensed under [Apache-2.0](../LICENSE).
