# RustQEC

[![CI](https://github.com/nzy1997/rstim/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rstim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rstim/branch/master/graph/badge.svg)](https://codecov.io/gh/nzy1997/rstim)

RustQEC is a Rust workspace for quantum error correction. It brings together
the `rstim` Stim-like circuit simulator and CLI, code-construction tools,
decoder experiments, and reproducible benchmark evidence.

## Benchmarked Documentation Site

The [benchmarked documentation site](https://nzy1997.github.io/rstim/)
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
| `rstim/` | Simulator crate and `rstim` CLI for circuit parsing, sampling, DEM extraction, SVG rendering, and QP101 export |
| `rstim/doc/` | Simulator getting-started guide, CLI reference, QP101 notes, and parity documentation |
| `docs/showcases/` | Stable index for runnable workspace showcases |
| `rsinter/` | Parallel collection and benchmark harness for decoder experiments |
| `rmatching/` | Rust MWPM decoder for detector-error-model workflows |
| `rbposd/`, `rilpqec/` | Additional decoder components used by benchmark and comparison flows |
| `qec-code/`, `qec-ilp-core/` | Code construction helpers and ILP-backed checks |
| `benchmarks/surface_decoder_compare/` | Cross-decoder comparison harness and benchmark artifacts |
| `qp101-viz/` | Optional legacy/prototype Typst renderer and committed QP101 fixtures |

## Quick Start

Build the workspace:

```sh
cargo build --workspace
```

Inspect a small circuit with `rstim stats`:

```sh
printf 'H 0\nM 0\nDETECTOR rec[-1]\n' | cargo run -p rstim --bin rstim -- stats
```

Run the Rust test suite:

```sh
cargo test --workspace
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
- [`rmatching` decoder docs](rmatching/README.md): MWPM decoder entry point for
  detector-error-model workflows.
- [Surface decoder benchmark docs](benchmarks/surface_decoder_compare/README.md):
  benchmark setup, smoke commands, and generated artifacts.

## CLI And Visualization Notes

The CLI reads from `--in <path>` or stdin and writes to `--out <path>` or
stdout for most commands. For static circuit diagrams, prefer:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

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
