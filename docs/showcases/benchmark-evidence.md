# Benchmark And Reproduction Evidence

This showcase maps the benchmark and reproduction evidence already committed
to the repository. It is a guide to evidence surfaces, not a new benchmark run
or an algorithmic comparison claim.

## What This Shows

The repository has two benchmark evidence tracks that answer different
questions:

- Surface-decoder comparison evidence in
  [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)
  and the checked-in full-tier artifacts under
  [`benchmarks/surface_decoder_compare/results/full/`](benchmarks/surface_decoder_compare/results/full/).
- BB72/BB144 circuit-level BP-OSD comparison evidence in
  [`benchmarks/bb_circuit_bposd_compare/README.md`](benchmarks/bb_circuit_bposd_compare/README.md)
  and the checked-in full-tier artifacts under
  [`benchmarks/bb_circuit_bposd_compare/results/full/`](benchmarks/bb_circuit_bposd_compare/results/full/).

The surface-decoder comparison material demonstrates benchmark harness wiring,
tracked result artifacts, and smoke versus full campaign entry points. The
BB72/BB144 material demonstrates the paired batched Rust/Python comparison
workflow, checked-in result tables, a source-backed reference-gap report, and a
Rust-rendered full plot.

## Run It

Smoke commands are intended for local implementation checks:

```sh
make surface-decoder-compare-smoke
make bench-surface-smoke
make bb-circuit-bposd-compare-plot-smoke
```

Full campaigns are longer-running evidence-generation commands:

```sh
make surface-decoder-compare-full
make bench-surface-full
make bb-circuit-bposd-compare-full
```

## Expected Result

`make surface-decoder-compare-smoke` writes local smoke artifacts under
`benchmarks/surface_decoder_compare/results/smoke/`; that directory is ignored
and is for iteration only.

`make surface-decoder-compare-full` writes `results.csv` and a Rust-rendered
`surface_decoder_compare.png` under
[`benchmarks/surface_decoder_compare/results/full/`](benchmarks/surface_decoder_compare/results/full/).
The committed full-tier evidence currently includes
[`results.csv`](benchmarks/surface_decoder_compare/results/full/results.csv) and
[`surface_decoder_compare.png`](benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png).
The PNG is generated from the comparison CSV by `rsinter bench
plot-surface-compare-csv`.

`make bench-surface-smoke` and `make bench-surface-full` route through the
`rsinter` benchmark framework and write artifacts under
`benchmarks/out/surface_decoder/`.

`make bb-circuit-bposd-compare-plot-smoke` writes a tiny paired BB72/BB144
plot-smoke artifact set under
`benchmarks/bb_circuit_bposd_compare/results/plot-smoke/`; that directory is
ignored and is for local plotting checks.

`make bb-circuit-bposd-compare-full` writes `results.csv`, `summary.md`, and a
Rust-rendered `bb_circuit_bposd_compare.png` under
[`benchmarks/bb_circuit_bposd_compare/results/full/`](benchmarks/bb_circuit_bposd_compare/results/full/).
The committed full-tier evidence currently includes
[`results.csv`](benchmarks/bb_circuit_bposd_compare/results/full/results.csv),
[`summary.md`](benchmarks/bb_circuit_bposd_compare/results/full/summary.md),
[`bb_circuit_bposd_compare.png`](benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png),
and
[`reference_gap_report.md`](benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md).

## Visual Evidence

The checked-in surface-decoder full-tier artifact is generated from
`benchmarks/surface_decoder_compare/results/full/results.csv` by `rsinter bench
plot-surface-compare-csv`.

![Surface-code decoder comparison plot](../../benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png)

The checked-in BB72/BB144 circuit BP-OSD full-tier artifact is generated from
`benchmarks/bb_circuit_bposd_compare/results/full/results.csv` by the
BB-circuit comparison workflow using `benchmarks/bb_circuit_bposd_compare/plot.toml`.

![BB72/BB144 circuit BP-OSD comparison plot](../../benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png)

## Code

Primary evidence docs and commands:

- [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)
- [`benchmarks/bb_circuit_bposd_compare/README.md`](benchmarks/bb_circuit_bposd_compare/README.md)
- [`Makefile`](Makefile)

Tracked surface-decoder comparison artifacts:

- [`benchmarks/surface_decoder_compare/results/full/results.csv`](benchmarks/surface_decoder_compare/results/full/results.csv)
- [`benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`](benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png)

`rsinter` benchmark specs:

- [`benchmarks/surface_decoder/spec.toml`](benchmarks/surface_decoder/spec.toml)
- [`benchmarks/surface_decoder/full.toml`](benchmarks/surface_decoder/full.toml)
- [`benchmarks/bb_circuit_bposd_compare/plot.toml`](benchmarks/bb_circuit_bposd_compare/plot.toml)

Tracked BB72/BB144 circuit comparison artifacts:

- [`benchmarks/bb_circuit_bposd_compare/results/full/results.csv`](benchmarks/bb_circuit_bposd_compare/results/full/results.csv)
- [`benchmarks/bb_circuit_bposd_compare/results/full/summary.md`](benchmarks/bb_circuit_bposd_compare/results/full/summary.md)
- [`benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png`](benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png)
- [`benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md`](benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/benchmark-evidence.md
```

Run the surface-decoder comparison docs contract:

```sh
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
```

That contract checks the required evidence links and rejects misspelled
BB-circuit make targets in the showcase text.

Run the BB circuit evidence validators:

```sh
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
```

Run the `rsinter` benchmark spec and registry tests:

```sh
cargo test -p rsinter --test bench_specs --test bench_registry -q
```

These tests keep checked-in surface-decoder runner aliases and registry
expansion behavior current, and keep the BB circuit plot spec pinned to the
paper-style logical-rate-per-syndrome-cycle view.

## Limits

The checked-in surface-decoder full-tier artifacts are evidence for the
committed run, not a promise about current local machine speed or a general
decoder ordering.

The surface-decoder smoke commands are implementation checks. They are not a
replacement for the full comparison campaign and should not be cited as
statistical evidence.

The BB72/BB144 full rows are batched, error-budget-stopped paired comparison
rows. They are not a fixed-shot reproduction of the pinned Bravyi reference
curve, and the reference-gap report records that interpretation boundary.

This page does not implement new benchmark functionality, regenerate results,
or resolve open algorithmic questions about decoder behavior.
