# RustQEC

[![CI](https://github.com/nzy1997/rust-qec/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rust-qec/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rust-qec/branch/master/graph/badge.svg)](https://codecov.io/gh/nzy1997/rust-qec)

RustQEC is a Rust workspace for quantum error correction. It brings together
the `rstim` Stim-like circuit simulator and CLI, code-construction tools,
decoder experiments, and reproducible benchmark evidence.

## Benchmarked Documentation Site

The [benchmarked documentation site](https://nzy1997.github.io/rust-qec/)
is the broad repository reference: workspace walkthroughs, benchmark evidence,
checked results, methodology and claims limits, plus the QP101 schema browser
and gallery that used to be the whole Pages surface.

Build and check the same Pages tree locally:

```sh
make build-site
python3 tools/check_site_build.py _site
```

## What You Can Do

With RustQEC you can:

- [Trace a circuit through stats, detector events, detector-error-model
  extraction, and DEM sampling](docs/showcases/rstim-cli-dem-pipeline.md).
- [Render circuit diagrams as SVG, including seeded atom-loss sample-shot
  overlays](docs/showcases/rstim-render-svg-atom-loss.md).
- [Construct CSS code matrices and run small exact-distance
  checks](docs/showcases/qec-code-css-construction.md).
- [Inspect benchmark and reproduction evidence, including the checked-in
  surface-code decoder comparison plot](docs/showcases/benchmark-evidence.md).
- [Browse the full showcase index](docs/showcases/README.md) for runnable
  workflow categories and verification commands.

## Workspace Map

| Path | Role |
| --- | --- |
| `rustqec-cli/` | Unified automation-ready `rustqec` CLI and capability discovery |
| `rstim/` | Simulator crate and `rstim` CLI for circuit parsing, sampling, DEM extraction, SVG rendering, and QP101 export |
| `rstim/doc/` | Simulator getting-started guide, CLI reference, QP101 notes, and parity documentation |
| `docs/showcases/` | Stable index for runnable workspace showcases |
| `rsinter/` | Parallel collection and benchmark harness for decoder experiments |
| `rmatching/` | Rust MWPM decoder for detector-error-model workflows |
| `renvelope/` | Reference decoders for explicit atom-loss Pauli envelopes (exact MLE and matching) |
| `rbposd/`, `rilpqec/` | Additional decoder components used by benchmark and comparison flows |
| `qec-code/`, `qec-ilp-core/` | Code construction helpers and ILP-backed checks |
| `benchmarks/surface_decoder_compare/` | Cross-decoder comparison harness and benchmark artifacts |
| `qp101-viz/` | Optional legacy/prototype Typst renderer and committed QP101 fixtures |

## Quick Start

RustQEC supports native source builds on these tested environments:

| Operating system | Native target | Rust toolchains |
| --- | --- | --- |
| Ubuntu 24.04 x86_64 | `x86_64-unknown-linux-gnu` | 1.88.0 (MSRV), stable |
| macOS 15 on Apple silicon | `aarch64-apple-darwin` | 1.88.0 (MSRV), stable |

Install the full-workspace native build prerequisites on Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install -y build-essential clang cmake libclang-dev pkg-config libfontconfig1-dev python3-venv
```

On macOS 15, install the Xcode Command Line Tools and the Homebrew packages:

```sh
xcode-select --install
brew install cmake fontconfig pkg-config python
```

Install Rust 1.88.0 with [rustup](https://rustup.rs/) for the minimum-version
configuration, or select the current stable toolchain:

```sh
rustup toolchain install 1.88.0 --profile minimal
rustup default 1.88.0
```

These prerequisites cover the default workspace, including the HiGHS-backed
ILP crates and `rsinter` plotting. A smaller `rsinter` build avoids both HiGHS
and plotting (as well as the other optional decoder runners):

```sh
cargo build --locked -p rsinter --no-default-features --features rbposd-runner
```

The complete test suite also invokes Stim through Python. Install it in an
isolated environment before running `cargo test --locked --workspace`:

```sh
python3 -m venv .venv
. .venv/bin/activate
python3 -m pip install stim
```

The support promise is limited to the two native targets above. Windows and
other operating-system, architecture, and toolchain combinations are not part
of the validated matrix.

```sh
git clone https://github.com/nzy1997/rust-qec.git
cd rust-qec
cargo build --locked --workspace
```

Inspect a small circuit through the unified CLI:

```sh
printf 'H 0\nM 0\nDETECTOR rec[-1]\n' | \
  cargo run -p rustqec-cli --bin rustqec -- circuit stats --format json
```

Discover the currently implemented automation contract:

```sh
cargo run -p rustqec-cli --bin rustqec -- capabilities --format json
```

Automation clients can request structured errors independently of successful
output formatting by adding `--error-format json`. Capability discovery lists
the concrete argv path, supported arguments, error codes, and exit codes.

The existing crate-specific CLIs remain available. For example, the same
circuit can be inspected with `rstim stats`:

```sh
printf 'H 0\nM 0\nDETECTOR rec[-1]\n' | cargo run -p rstim --bin rstim -- stats
```

Run the Rust test suite:

```sh
cargo test --locked --workspace
```

After a native-support workflow completes, validate its four jobs, compiler
identities, uploaded CLI evidence, and the checked-out package metadata with:

```sh
python3 tools/check_native_support_matrix.py --repo-root . --run-id RUN_ID
```

## Primary Next Steps

- [Showcase index](docs/showcases/README.md): runnable workflow categories and
  the template used for future examples, including
  [rstim CLI DEM Pipeline](docs/showcases/rstim-cli-dem-pipeline.md),
  [rstim Render SVG Atom-Loss](docs/showcases/rstim-render-svg-atom-loss.md),
  [QEC-Code CSS Construction](docs/showcases/qec-code-css-construction.md), and
  [Benchmark Evidence](docs/showcases/benchmark-evidence.md).
- [Getting started with `rstim`](rstim/doc/getting_started.md): simulator and
  Rust API orientation.
- [`rstim` CLI reference](rstim/doc/cli.md): `stats`, `sample`, `detect`,
  `analyze_errors`, `render_svg`, `export_json`, and related commands.
- [Local neural-decoder training data](rstim/doc/training-data.md): aligned
  detector/observable `b8` streams plus versioned per-shot simulator traces.
- [`rmatching` decoder docs](rmatching/README.md): MWPM decoder entry point for
  detector-error-model workflows.
- [`rsinter replay`](docs/rsinter-replay.md): decode frozen `.dem` plus b8
  detector rows into b8 predictions and a reproducibility report.
- [Surface decoder benchmark docs](benchmarks/surface_decoder_compare/README.md):
  benchmark setup, smoke commands, and generated artifacts.

## CLI And Visualization Notes

The CLI reads from `--in <path>` or stdin and writes to `--out <path>` or
stdout for most commands. For static circuit diagrams, prefer:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

For an interactive single-shot view that can resample the fixed circuit, change
the realized outcome of existing noise instructions, and export SVG/PDF, run:

```sh
rstim shot_viewer
```

The hosted [Shot Lab](https://nzy1997.github.io/rust-qec/interactive/) shows one
repository-configured circuit. Its
[local-file entry](https://nzy1997.github.io/rust-qec/interactive/local/) starts
blank and processes a selected `.stim` file entirely in WebAssembly.

Use `export_json` when you need QP101 structured data for downstream tools,
fixtures, or the optional `qp101-viz` workflow:

```sh
rstim export_json --in circuit.stim --out circuit.json
```

Benchmark smoke runs are documented in
[`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md);
the README intentionally leaves algorithm details and benchmark implementation
notes to those dedicated docs.

## License

All tracked content in this repository, including the Rust workspace crates and
`qp101-viz`, is licensed under Apache-2.0. See [LICENSE](LICENSE) for the full
license text. Ignored or untracked drafts are outside this repository license
declaration.

Portions of `rstim` compatibility tests are adapted from
[Stim](https://github.com/quantumlib/Stim), and `rmatching` is ported from
[PyMatching](https://github.com/oscarhiggott/PyMatching). Both upstream projects
are Apache-2.0, and existing source-level provenance comments are preserved.
