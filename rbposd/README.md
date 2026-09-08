# rbposd

`rbposd` provides belief-propagation decoders with ordered-statistics (OSD) or
localized-statistics (LSD) post-processing for binary parity-check matrices.
It exposes sparse parity-check matrices, channel models, decoder configuration,
syndrome and correction types, and reusable BP+OSD/BP+LSD decoders.

Add the crate to an application with:

```toml
[dependencies]
rbposd = "0.3.0"
```

```rust
use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let checks = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1]])?;
    let decoder = BpOsdDecoder::new(
        checks.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )?;
    let syndrome = Syndrome::from(vec![true]);
    let result = decoder.decode(&syndrome)?;
    assert_eq!(checks.multiply(&result.correction), syndrome);
    Ok(())
}
```

The crate has no Cargo features. Callers supply a parity-check matrix and
channel model directly; `rbposd` does not parse detector error models.
Converting a circuit or DEM into those inputs belongs in an integration layer
such as [`rsinter`](https://crates.io/crates/rsinter).

Channel probabilities must be finite and strictly between 0 and 1. A
per-bit probability vector must have one entry for every matrix bit.

## Supported decoder modes

- BP supports minimum-sum and product-sum updates with parallel or serial
  scheduling.
- LSD supports orders 0 and 1. Larger values return
  `DecodeError::UnsupportedLsdOrder`.
- `OsdVariant::Osd0` performs the order-zero solve. Setting `osd_order > 0`
  with this variant selects the legacy combination sweep, which searches
  combinations among at most the 16 least-reliable free columns.
- `OsdVariant::LdpcCombinationSweep` tests every single free column and every
  pair among the first `min(free_column_count, osd_order)` free columns.

`ParityCheckMatrix::row_neighbors`, `column_neighbors`, and `multiply` are
low-overhead operations that expect valid dimensions. An out-of-range neighbor
index or a correction shorter than `num_bits()` will panic. Decoder entrypoints
validate syndrome and channel dimensions and return `DecodeError`.

See the [complete BP+OSD example](examples/basic_decode.rs), the BP+LSD example
in the [API documentation](https://docs.rs/rbposd), and the
[`rsinter` integration crate](https://crates.io/crates/rsinter). The full API
reference can also be built locally with `cargo doc -p rbposd --open`.

Licensed under [Apache-2.0](LICENSE).
