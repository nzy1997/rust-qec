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
- BB144 circuit-level BP-OSD reproduction evidence in
  [`docs/bb144_circuit_bposd_reproduction.md`](docs/bb144_circuit_bposd_reproduction.md).

The surface-decoder comparison material demonstrates benchmark harness wiring,
tracked result artifacts, and smoke versus full campaign entry points. The
BB144 note records implementation smoke evidence for the `bb-circuit-bposd-memory`
path and separates that smoke result from the manual upstream-budget
reproduction command.

## Run It

Smoke commands are intended for local implementation checks:

```sh
make surface-decoder-compare-smoke
make bench-surface-smoke
cargo run -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 1 \
  --seed 12345 \
  --max-bp-iterations 10 \
  --osd-order 0
```

Full or manual campaigns are longer-running evidence-generation commands:

```sh
make surface-decoder-compare-full
make bench-surface-full
cargo run --release -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 50000 \
  --max-bp-iterations 10000 \
  --osd-order 7
```

## Expected Result

`make surface-decoder-compare-smoke` writes local smoke artifacts under
`benchmarks/surface_decoder_compare/results/smoke/`; that directory is ignored
and is for iteration only.

`make surface-decoder-compare-full` writes `results.csv` and
`surface_decoder_compare.png` under
[`benchmarks/surface_decoder_compare/results/full/`](benchmarks/surface_decoder_compare/results/full/).
The committed full-tier evidence currently includes
[`results.csv`](benchmarks/surface_decoder_compare/results/full/results.csv) and
[`surface_decoder_compare.png`](benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png).

`make bench-surface-smoke` and `make bench-surface-full` route through the
`rsinter` benchmark framework and write artifacts under
`benchmarks/out/surface_decoder/`.

The short BB144 command prints a four-column result line such as
`0.003	12	1	<num_failed_trials>`. That command is implementation smoke
evidence, not statistical reproduction. The upstream-budget BB144 command has
the same output shape with `50000` trials and is the command documented for a
statistical comparison attempt.

## Code

Primary evidence docs and commands:

- [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)
- [`docs/bb144_circuit_bposd_reproduction.md`](docs/bb144_circuit_bposd_reproduction.md)
- [`Makefile`](Makefile)

Tracked surface-decoder comparison artifacts:

- [`benchmarks/surface_decoder_compare/results/full/results.csv`](benchmarks/surface_decoder_compare/results/full/results.csv)
- [`benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`](benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png)

`rsinter` benchmark specs:

- [`benchmarks/surface_decoder/spec.toml`](benchmarks/surface_decoder/spec.toml)
- [`benchmarks/surface_decoder/full.toml`](benchmarks/surface_decoder/full.toml)

BB144 reference material:

- [`docs/figures/bb144_reference/small_ldpc.png`](docs/figures/bb144_reference/small_ldpc.png)
- [`docs/figures/bb144_reference/ldpc_vs_surface.png`](docs/figures/bb144_reference/ldpc_vs_surface.png)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/benchmark-evidence.md
```

Run the surface-decoder comparison docs contract:

```sh
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
```

That contract checks the required evidence links and owns the negative control
for BB-circuit command-key typos by rejecting unknown BB command keys.

Run the `rsinter` benchmark spec and registry tests:

```sh
cargo test -p rsinter --test bench_specs --test bench_registry -q
```

These tests keep checked-in surface-decoder runner aliases and registry
expansion behavior current.

## Limits

The checked-in surface-decoder full-tier artifacts are evidence for the
committed run, not a promise about current local machine speed or a general
decoder ordering.

The surface-decoder smoke commands are implementation checks. They are not a
replacement for the full comparison campaign and should not be cited as
statistical evidence.

The BB144 lower-budget and one-trial commands are implementation smoke
evidence, not statistical reproduction. The BB144 note documents the
50,000-trial manual command for statistical comparison, but this showcase does
not add or claim a completed new 50,000-trial result.

This page does not implement new benchmark functionality, regenerate results,
or resolve open algorithmic questions about decoder behavior.
