# Packaged-crate consumer check

This independent Rust workspace demonstrates the `rstim` 0.3.0 and `rmatching`
0.3.0 registry APIs. Once these versions are available on crates.io, run:

```sh
cargo run --manifest-path examples/rust-consumer/Cargo.toml
```

For an unpublished release candidate, use the package check below instead.

Maintainers can build the same program entirely from real `.crate` archives:

```sh
python3 tools/check_crate_consumer.py --repo-root . --target-dir target
```

The checker runs `cargo package`, unpacks the resulting archives outside the
source workspace, patches only the temporary manifests to those unpacked
packages, and then builds this consumer. Its successful program output is:

```text
validated Bell parity and 128/128 observable predictions
```
