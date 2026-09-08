# Crates.io publication and user installation

The **v0.3.0** release publishes the first eight workspace crates to crates.io.
See the [release and migration notes](releases/v0.3.0.md). The website installer
uses the v0.3.0 native archives; the historical v0.2.1 release and archives remain
available from GitHub.

## One task, one installation entry

The introductory workflow uses only `rustqec`: capability discovery, circuit
stats, detector sampling, and DEM generation. Install the published command:

```sh
cargo install --locked rustqec-cli --version 0.3.0
```

The default build includes envelope matching but not the native HiGHS solver.
To add exact envelope MLE, install the native solver build prerequisites in the
root README and use `--features ilp`. Official native archives deliberately build
with ILP enabled. The binary's `capabilities --format json` output reflects its
compiled features. From a development checkout, replace the package and version
with `--path rustqec-cli`.

For existing Stim-style workflows, install the separate executable:

```sh
cargo install --locked rstim --version 0.3.0 --bin rstim --features cli,codegen-css,shot-viewer
```

Installing a library dependency never implicitly installs that library's bins.
Internal `rstim` worker bins require `benchmark-tools`. Matching benchmark bins
live in the private `rmatching-bench-tools` workspace package; they are not part
of the published `rmatching` package. Compatibility CLI bins in `rstim` and
`qec-code` require `cli`.

From a development checkout, replace the package and version with `--path rstim`.

## Package boundaries

The checked publication allowlist is `tools/crates_io_packages.json`. The first
batch uses this dependency order:

1. `qec-ilp-core` — solver/model infrastructure, 0.3.0.
2. `qec-code` — code construction, 0.3.0.
3. `rstim` — simulator and its compatibility CLI, 0.3.0.
4. `rmatching` — standalone MWPM library, 0.3.0.
5. `rbposd` — standalone BP+OSD and BP+LSD decoders, 0.3.0.
6. `rilpqec` — DEM ILP decoder with HiGHS, 0.3.0.
7. `rsinter` — sampling harness and opt-in decoder/replay CLI, 0.3.0.
8. `rustqec-cli` — primary `rustqec` application, 0.3.0.

`qec-code`'s ILP dependency and `rstim`'s CSS generation dependency are optional
for compilation but still form registry publication dependencies. First-batch
packages have versioned path dependencies: local development uses the path,
published packages resolve the specified registry version.

`renvelope` remains deferred with `publish = false`: it is a separate research
API, and none of the published packages requires it as a runtime dependency. The WebAssembly UI
adapter, benchmark bridge, and matching benchmark tools also remain unpublished. A workspace is not one
registry package; consumers select the library they need or install the CLI.

For a complete Rust library example, see
[`examples/rust-consumer`](../examples/rust-consumer/README.md). It is an independent
Cargo workspace with registry dependency declarations. The package check copies
it outside this repository and uses unpacked local artifacts before the first
registry versions exist; that is not proof of a registry upload.

## Library feature boundaries

The simulator, code/model libraries, and standalone decoders keep their narrow
defaults. `rsinter` installs its harness CLI but requires explicit decoder
features; `rilpqec` always builds HiGHS. Choose optional capabilities explicitly:

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
| `rsinter` | `rbposd-runner`, `rmatching-runner` | BP and matching decoder runners/replay |
| `rsinter` | `ilp-runner` | ILP runner/replay through `rilpqec` and HiGHS |
| `rsinter` | `plotting` | Plot generation |
| `rilpqec` | `gurobi` | Adds Gurobi support; HiGHS remains included |

`rustqec-cli` selects CSS generation explicitly and calls `rstim::operations`;
it does not require `rstim::cli` or the viewer. Its `ilp` feature selects HiGHS.
The native archives select `cli,codegen-css,shot-viewer` for `rstim`, retaining
the existing complete installation experience. Library users can call shared
operations without command-line argument parsing.

Viewer files remain in the `.crate` archive so the optional feature works after
installation; disabling the feature prevents compiling the viewer module, not
downloading those package files. The browser WASM adapter uses the minimal library.
Missing solver backends return explicit errors; importing ILP model types alone
does not compile a solver. Actual Gurobi runtime and license validation were not
performed for v0.3.0.

## Development feature migration

`rsinter` now defaults to its harness without optional runners or plotting.
Choose `rbposd-runner`, `rmatching-runner`, `ilp-runner`, or `plotting`; use `full`
for the previous complete research environment. The full CI and benchmark bridge
explicitly enable the features they test. Run the standard workspace suite with CLI, CSS, viewer, and HiGHS enabled with:

```sh
make test
```

Run standalone worker tests with `cargo test -p rstim --features benchmark-tools`.
## Version policy

All eight first-batch packages start at 0.3.0. Under
`tools/release_version_policy.json`, subsequent patches can be published
independently within the same major/minor series: fixing only `rilpqec` may
produce `rilpqec 0.3.1` while the other packages remain at 0.3.0. Compatible Cargo
requirements such as `version = "0.3.0"` accept those patches. A coordinated
0.4.0 release moves the public series together; unchanged crates do not need
republishing for a 0.3.x patch. The repository tag records a tested snapshot,
with no package ahead of the tag and at least one package matching its version.
Each repository release uses a new tag; changed packages can adopt that tag's
patch number while unchanged packages stay put. Individual packages may
therefore skip patch numbers.
Historical v0.2.1 tags keep their original four-package synchronization rule.

## Decoder and sampling entry points

`rbposd` is a solver-free matrix decoder. `rilpqec` accepts `rstim` DEMs and
builds a native ILP solver. `rsinter` joins the simulator to optional decoders,
so publishing its optional dependencies first is necessary even for a slim
installation. Together these packages cover the simulation → decode → collect
workflow; first publication is not an expansion of numerical acceptance claims.

The recommended sampling/replay installation is:

```sh
cargo install --locked rsinter --version 0.3.0 --features rbposd-runner,rmatching-runner
```

From this checkout use `--path rsinter` instead of `rsinter --version 0.3.0`.
Add `plotting` for plots, or `ilp-runner` with native build prerequisites for
ILP decoding. The default CLI explains missing features. See the self-contained
[replay quickstart](../rsinter/README.md) and [ILP example](../rilpqec/README.md).

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
and a separately installed full compatibility CLI serving its packaged viewer.
It runs the shipped tests/examples for all three added packages, checks the slim
`rsinter` failure message, and verifies actual BP, matching, and ILP replay
predictions plus malformed-input rejection from an installed full CLI. The consumer checker therefore needs the native solver build
prerequisites; default CLI installation itself does not. Neither command uploads a package.
Retain source tests and compile-time fixtures/assets; exclude repository plans,
research outputs, caches, and unrelated development tools from package files.

Build from the reviewed release commit after its required CI checks pass. With
Cargo 1.93.1 (the tooling used to verify this candidate), check all eight packages
together, including compilation of their packaged sources:

```sh
cargo publish --dry-run --locked \
  -p qec-ilp-core -p qec-code -p rstim -p rmatching \
  -p rbposd -p rilpqec -p rsinter -p rustqec-cli
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
