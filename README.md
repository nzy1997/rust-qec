# rstim Workspace

[![CI](https://github.com/nzy1997/rstim/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rstim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rstim/branch/master/graph/badge.svg)](https://codecov.io/gh/nzy1997/rstim)

This repository is a Rust quantum error correction workspace centered on
Stim-like circuit simulation, detector error models, decoding, and benchmarking.

The main entrypoints are:

- `rstim`: stabilizer circuit simulation, circuit generation, DEM extraction,
  and QP101 export
- `rsinter`: benchmark and sampling harness for decoder experiments
- `rmatching`: Rust MWPM decoder for DEM-based workflows

## Workspace Map

| Path | Role |
| --- | --- |
| `rstim/` | Stim-like simulator crate and `rstim` CLI |
| `rsinter/` | Parallel collection and benchmark harness, plus `rsinter` CLI |
| `rmatching/` | Sparse Blossom MWPM decoder |
| `rbposd/` | BP+OSD decoder components |
| `rilpqec/` | ILP-based decoding path |
| `qec-code/` | CSS/code construction helpers and `qec-code` CLI |
| `benchmarks/surface_decoder_compare/` | Cross-decoder comparison harness |
| `qp101-viz/` | Optional legacy/prototype Typst renderer for QP101 circuit JSON |

## Quick Start

Build the workspace:

```sh
cargo build --workspace
```

Run a minimal `rstim` CLI check:

```sh
printf 'H 0\nM 0\nDETECTOR rec[-1]\n' | cargo run -p rstim --bin rstim -- stats
```

Run the Rust test suite:

```sh
cargo test --workspace
```

If you only want the simulator, start with
[`rstim/doc/getting_started.md`](rstim/doc/getting_started.md) and
[`rstim/doc/cli.md`](rstim/doc/cli.md).

## Common Workflows

### Inspect And Sample A Circuit

Use `rstim stats` before heavier workflows:

```sh
printf 'H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n' | \
  cargo run -p rstim --bin rstim -- stats
```

Then move on to:

- `sample` for measurements
- `detect` for detection events and observable flips
- `analyze_errors` for DEM extraction
- `render_svg` for built-in static SVG circuit diagrams
- `export_json` for QP101 structured-data export

The full command reference is in
[`rstim/doc/cli.md`](rstim/doc/cli.md).

### Generate Standard QEC Circuits

`rstim gen` can generate common benchmark circuits:

```sh
cargo run -p rstim --bin rstim -- gen \
  --code repetition_code \
  --task memory \
  --distance 5 \
  --rounds 5
```

For code-construction-oriented workflows, see `qec-code`:

```sh
cargo run -p qec-code -- --help
```

### Run Decoder Benchmarks

The workspace includes two benchmark layers:

- `rsinter` benchmark flow under `benchmarks/surface_decoder/`
- comparison harness under `benchmarks/surface_decoder_compare/`

Smoke benchmark through `rsinter`:

```sh
make bench-surface-smoke
```

Cross-decoder comparison harness:

```sh
make surface-decoder-compare-smoke
```

Benchmark setup details are in
[`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md).

## Static SVG Diagrams And Atom Loss Overlays

For static circuit visualization, use the built-in SVG renderer first:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

Omit `--out` to write the SVG document to stdout, which is useful for pipes and
quick checks:

```sh
printf 'H 0\nCX 0 1\nTICK\nM 0\n' | rstim render_svg > circuit.svg
```

Atom loss is a first-class workflow in `rstim`. The simulator can model
explicit `LOSS` events, propagate loss through later operations, and annotate
loss-caused measurement outcomes in seeded sample-shot SVGs.

Example circuit:

```stim
DEPOLARIZE1(1) 0
LOSS(1) 1
LOSS(1) 2
M 1
MRL 2
DETECTOR rec[-3]
```

Render one seeded sample shot with atom-loss and detector-flip overlays:

```sh
rstim render_svg --sample_shot --seed 7 \
  --in qp101-viz/examples/atom-loss-sample.stim \
  --out atom-loss-sample.svg
```

Render a selected detector-error-model error as source and symptom highlights:

```sh
printf 'X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n' > /tmp/rstim-dem-highlight.stim
rstim render_svg --highlight_dem_error 0 \
  --in /tmp/rstim-dem-highlight.stim \
  --out dem-highlight.svg
```

Use `export_json` when you need QP101 structured data for downstream tooling,
fixture generation, or the optional legacy/prototype Typst workflow:

```sh
cargo run -p rstim --bin rstim -- export_json --sample_shot --seed 7 \
  < qp101-viz/examples/atom-loss-sample.stim
```

Regenerate the larger mixed-noise showcase:

```sh
cargo run -p rstim --example mixed_noise_showcase
```

Related files:

- [`qp101-viz/examples/atom-loss-sample.typ`](qp101-viz/examples/atom-loss-sample.typ)
- [`qp101-viz/examples/atom-loss-sample.qp101.json`](qp101-viz/examples/atom-loss-sample.qp101.json)
- [`qp101-viz/README.md`](qp101-viz/README.md)

## Stim Parity And Performance Evidence

This repository keeps parity and benchmark evidence for generated circuits and
detector error models against Stim-oriented workflows.

To rerun the current parity showcase:

```sh
cargo build -p rstim --bin rstim
cargo run -p rstim --example stim_parity_showcase
```

Supporting notes live in:

- [`rstim/doc/performance_parity.md`](rstim/doc/performance_parity.md)
- [`docs/superpowers/specs/2026-05-25-performance-parity-foundation-design.md`](docs/superpowers/specs/2026-05-25-performance-parity-foundation-design.md)

## Further Reading

- [`rstim/doc/getting_started.md`](rstim/doc/getting_started.md)
- [`rstim/doc/cli.md`](rstim/doc/cli.md)
- [`rmatching/README.md`](rmatching/README.md)
- [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)
- [`qp101-viz/README.md`](qp101-viz/README.md)

## Maintainer Release Flow

Cut a new tagged release with:

```sh
make release V=0.1.1
```

That target:

- bumps selected workspace crate versions
- runs `cargo check --workspace`
- creates a release commit
- creates an annotated tag `vX.Y.Z`
- pushes the default branch and tags to `origin`

Pushing a tag that matches `v*.*.*` triggers
[`.github/workflows/release.yml`](.github/workflows/release.yml), which creates
a GitHub Release with generated notes.

The default branch is currently `master`. If that changes later, override it:

```sh
make release V=0.1.1 DEFAULT_BRANCH=main
```
