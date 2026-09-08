# Crates.io publication and user installation

The first crates.io release is **being prepared, not published**. The current
v0.2.1 native archives remain available through the website installer. This
preparation does not create, move, or publish a release tag.

## One task, one installation entry

The introductory workflow uses only `rustqec`: capability discovery, circuit
stats, detector sampling, and DEM generation. From the development checkout:

```sh
cargo install --locked --path rustqec-cli
```

The default build includes envelope matching but not the native HiGHS solver.
To add exact envelope MLE, install the native solver build prerequisites in the
root README and use `--features ilp`. Official native archives deliberately build
with ILP enabled. The binary's `capabilities --format json` output reflects its
compiled features.

For existing Stim-style workflows, install the separate executable:

```sh
cargo install --locked --path rstim --bin rstim
```

Installing a library dependency never implicitly installs that library's bins.
Internal `rstim` worker bins require `benchmark-tools`; `rmatching` benchmark
bins require `bench`. They are not part of a default installation.

After registry publication, the corresponding commands will be
`cargo install --locked rustqec-cli` and `cargo install --locked rstim --bin rstim`.
Until then, these registry commands cannot be used.

## Package boundaries

The checked publication allowlist is `tools/crates_io_packages.json`. The first
batch uses this dependency order:

1. `qec-ilp-core` — solver/model infrastructure, currently 0.1.0.
2. `qec-code` — code construction, currently 0.1.0.
3. `rstim` — simulator and its compatibility CLI, currently 0.2.1.
4. `rmatching` — standalone MWPM library, currently 0.2.1.
5. `rustqec-cli` — primary `rustqec` application, currently 0.1.0.

`qec-code`'s ILP dependency and `rmatching`'s benchmark dependency are optional
for compilation but still form registry publication dependencies. First-batch
packages have versioned path dependencies: local development uses the path,
published packages resolve the specified registry version.

`rbposd`, `rsinter`, `rilpqec`, and `renvelope` are deferred; their manifests use
`publish = false` until an explicitly reviewed later batch. The WebAssembly UI
adapter and benchmark bridge also remain unpublished. A workspace is not one
registry package; consumers select the library they need or install the CLI.

For a complete Rust library example, see
[`examples/rust-consumer`](../examples/rust-consumer/README.md). It is an independent
Cargo workspace with registry dependency declarations. The package check copies
it outside this repository and uses unpacked local artifacts before the first
registry versions exist; that is not proof of a registry upload.

## Development feature migration

`rsinter` now defaults to its harness without optional runners or plotting.
Choose `rbposd-runner`, `rmatching-runner`, `ilp-runner`, or `plotting`; use `full`
for the previous complete research environment. The full CI and benchmark bridge
explicitly enable the features they test. Run all non-proprietary features with:

```sh
cargo test --locked --workspace --features rustqec-cli/ilp,rsinter/full,rstim/benchmark-tools
```

Run standalone worker tests with `cargo test -p rstim --features benchmark-tools`.
Existing immutable native releases are unchanged. Future release numbering must
account for these default-feature changes; do not republish changed code under
the existing v0.2.1 tag. CLI product versions and independently versioned library
packages should be listed explicitly in the next release manifest.

## Checks before upload

Run the checked package/consumer workflow in CI and inspect its artifacts:

```sh
python3 tools/check_crates_io.py
python3 tools/check_crate_consumer.py
```

The first command checks metadata, the publication graph, installation targets,
and package file lists. The consumer check validates actual unpacked package
sources and an isolated CLI installation, followed by an opt-in ILP build from the same
unpacked sources. The consumer checker therefore needs the native solver build
prerequisites; default CLI installation itself does not. Neither command uploads a package.
Retain source tests and compile-time fixtures/assets; exclude repository plans,
research outputs, caches, and unrelated development tools from package files.

Before the actual release, choose the final versions and build from the reviewed
release commit. Authenticate interactively with `cargo login`, then perform a
full `cargo publish --dry-run` without `--no-verify`. Publish in the allowlist's
order and confirm each upstream package is available before proceeding. After
upload, verify `cargo install`, the external Rust consumer, crates.io metadata,
and docs.rs from a clean environment using registry dependencies only.

Cargo's [publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
and [dependency guide](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#multiple-locations)
describe the registry requirements. First publication establishes the crate;
subsequent releases can use Trusted Publishing. Credentials never belong in
manifests, reports, or Git history.
