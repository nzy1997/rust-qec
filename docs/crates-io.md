# Crates.io publication and user installation

This checkout prepares the **v0.3.0** first crates.io release; uploading is a
separate step. See the [release and migration notes](releases/v0.3.0.md). The current
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
cargo install --locked --path rstim --bin rstim --features cli,codegen-css,shot-viewer
```

Installing a library dependency never implicitly installs that library's bins.
Internal `rstim` worker bins require `benchmark-tools`. Matching benchmark bins
live in the private `rmatching-bench-tools` workspace package; they are not part
of the published `rmatching` package. Compatibility CLI bins in `rstim` and
`qec-code` require `cli`.

After registry publication, the corresponding commands will be
`cargo install --locked rustqec-cli --version 0.1.0` and
`cargo install --locked rstim --version 0.3.0 --bin rstim --features cli,codegen-css,shot-viewer`.
Until then, these registry commands cannot be used.

## Package boundaries

The checked publication allowlist is `tools/crates_io_packages.json`. The first
batch uses this dependency order:

1. `qec-ilp-core` — solver/model infrastructure, 0.1.0.
2. `qec-code` — code construction, 0.1.0.
3. `rstim` — simulator and its compatibility CLI, 0.3.0.
4. `rmatching` — standalone MWPM library, 0.3.0.
5. `rustqec-cli` — primary `rustqec` application, 0.1.0.

`qec-code`'s ILP dependency and `rstim`'s CSS generation dependency are optional
for compilation but still form registry publication dependencies. First-batch
packages have versioned path dependencies: local development uses the path,
published packages resolve the specified registry version.

`rbposd`, `rsinter`, `rilpqec`, and `renvelope` are deferred; their manifests use
`publish = false` until an explicitly reviewed later batch. The WebAssembly UI
adapter, benchmark bridge, and matching benchmark tools also remain unpublished. A workspace is not one
registry package; consumers select the library they need or install the CLI.

For a complete Rust library example, see
[`examples/rust-consumer`](../examples/rust-consumer/README.md). It is an independent
Cargo workspace with registry dependency declarations. The package check copies
it outside this repository and uses unpacked local artifacts before the first
registry versions exist; that is not proof of a registry upload.

## Library feature boundaries

All four published libraries default to their library interfaces. Choose optional
capabilities explicitly:

| Package | Feature | Enables |
| --- | --- | --- |
| `rstim` | `cli` | Compatibility command-line parsing and executable |
| `rstim` | `codegen-css` | CSS circuit generation using `qec-code` |
| `rstim` | `shot-viewer` | Local viewer server and embedded web resources |
| `qec-code` | `cli` | Command-line parsing and executable |
| `qec-code` | `distance-ilp-highs` | Exact distance with the HiGHS backend |
| `qec-code` | `distance-ilp-gurobi` | Exact distance with the Gurobi backend |
| `qec-ilp-core` | `highs` | Native HiGHS backend |
| `qec-ilp-core` | `gurobi` | Separately configured Gurobi backend |

`rustqec-cli` selects CSS generation explicitly and calls `rstim::operations`;
it does not require `rstim::cli` or the viewer. Its `ilp` feature selects HiGHS.
The native archives select `cli,codegen-css,shot-viewer` for `rstim`, retaining
the existing complete installation experience. Library users can call shared
operations without command-line argument parsing.

Viewer files remain in the `.crate` archive so the optional feature works after
installation; disabling the feature prevents compiling the viewer module, not
downloading those package files. The browser WASM adapter uses the minimal library.
Missing solver backends return explicit errors; importing ILP model types alone
does not compile a solver.

## Development feature migration

`rsinter` now defaults to its harness without optional runners or plotting.
Choose `rbposd-runner`, `rmatching-runner`, `ilp-runner`, or `plotting`; use `full`
for the previous complete research environment. The full CI and benchmark bridge
explicitly enable the features they test. Run the standard workspace suite with CLI, CSS, viewer, and HiGHS enabled with:

```sh
make test
```

Run standalone worker tests with `cargo test -p rstim --features benchmark-tools`.
The repository release version is v0.3.0 because these feature defaults change
source compatibility. `rstim`, `rmatching`, `rsinter`, and `rbposd` synchronize
to 0.3.0 under `tools/release_version_policy.json`; the latter two remain
unpublished. The other first-batch packages retain their independent 0.1.0
versions. Existing v0.2.1 tags and native releases remain unchanged.

## Checks before upload

Run the checked package/consumer workflow in CI and inspect its artifacts:

```sh
python3 tools/check_crates_io.py
python3 tools/check_crate_consumer.py
```

The first command checks metadata, the publication graph, installation targets,
and package file lists. The consumer check builds the unpacked simulator/decoder example with viewer
assets temporarily absent and verifies model-only libraries without native solvers.
It then validates an isolated default CLI installation, opt-in ILP compilation,
and a separately installed full compatibility CLI serving its packaged viewer. The consumer checker therefore needs the native solver build
prerequisites; default CLI installation itself does not. Neither command uploads a package.
Retain source tests and compile-time fixtures/assets; exclude repository plans,
research outputs, caches, and unrelated development tools from package files.

Build from the reviewed release commit after its required CI checks pass. With
Cargo 1.93.1 (the tooling used to verify this candidate), check all five packages
together, including compilation of their packaged sources:

```sh
cargo publish --dry-run --locked \
  -p qec-ilp-core -p qec-code -p rstim -p rmatching -p rustqec-cli
```

Selecting the packages together lets Cargo verify their unpublished dependencies
using a temporary local registry. A dry-run of only a downstream package can fail
before its upstream crates exist on crates.io. The publishing toolchain can be
newer than the library MSRV, which remains Rust 1.88.

This command does not upload. Actual publication is a separate authorized action
using a crates.io account with a verified email and suitable credentials. If
publishing individually, follow the allowlist order and confirm each upstream
package is available before proceeding. After upload, verify `cargo install`,
the external Rust consumer, crates.io metadata, and docs.rs from a clean
environment using registry dependencies only. Switch the website's installation
entry to the new release only once its packages and native assets are available.

Cargo's [publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
and [dependency guide](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#multiple-locations)
describe the registry requirements. First publication establishes the crate;
subsequent releases can use Trusted Publishing. Credentials never belong in
manifests, reports, or Git history.
