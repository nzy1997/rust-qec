# Packaged-crate consumer check

This is an independent Rust workspace with the registry dependencies that a
consumer will use after the first releases. Those releases are not on
crates.io yet, so running this manifest directly is intentionally deferred
until publication.

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
