# Showcase Index

This directory is the stable front door for runnable rstim workspace
showcases. Each showcase page should explain what the example demonstrates,
how to run it, where the result appears, which code owns it, how to verify it,
and what its limits are.

Individual showcase pages will be added by follow-up issues. Use
[`_template.md`](_template.md) for new pages.

## Categories

### Simulator And CLI Workflows

Showcases in this category should demonstrate `rstim` circuit simulation,
sampling, detector event extraction, detector error model analysis, and CLI
input/output behavior.

Primary code and docs:

- [`rstim/`](rstim/)
- [`rstim/doc/getting_started.md`](rstim/doc/getting_started.md)
- [`rstim/doc/cli.md`](rstim/doc/cli.md)
- [`README.md`](README.md)

### Visualization And QP101 Artifacts

Showcases in this category should demonstrate built-in SVG rendering, seeded
sample-shot overlays, detector-error-model highlights, QP101 JSON export, and
the optional Typst renderer fixtures.

Primary code and docs:

- [`rstim/`](rstim/)
- [`qp101-viz/`](qp101-viz/)
- [`qp101-viz/README.md`](qp101-viz/README.md)
- [`qp101-viz/examples/`](qp101-viz/examples/)

### Decoder And Benchmark Workflows

Showcases in this category should demonstrate decoder integrations, benchmark
harnesses, smoke runs, and reproducible comparison outputs.

Primary code and docs:

- [`rsinter/`](rsinter/)
- [`rmatching/`](rmatching/)
- [`rbposd/`](rbposd/)
- [`rilpqec/`](rilpqec/)
- [`benchmarks/surface_decoder_compare/`](benchmarks/surface_decoder_compare/)
- [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)

### Code Construction Workflows

Showcases in this category should demonstrate CSS code construction, built-in
code families, sparse support exports, distance checks, and code-oriented CLI
flows.

Primary code and docs:

- [`qec-code/`](qec-code/)
- [`qec-ilp-core/`](qec-ilp-core/)
- [`docs/apm_decoder_hierarchy.md`](docs/apm_decoder_hierarchy.md)
- [`docs/bb144_circuit_bposd_reproduction.md`](docs/bb144_circuit_bposd_reproduction.md)

## Page Contract

Every individual showcase page must include these sections:

- `What This Shows`
- `Run It`
- `Expected Result`
- `Code`
- `Verification`
- `Limits`

Run the checker before opening a showcase documentation pull request:

```sh
python3 tools/check_showcase_docs.py docs/showcases
```
