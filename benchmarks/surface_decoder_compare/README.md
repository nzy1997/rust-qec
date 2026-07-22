# Surface Decoder Compare

This benchmark compares:

- `rbposd`
- `rilpqec`
- `rmatching`
- `pymatching`
- `ilpqec`
- `ldpc`

on shared Stim-generated rotated surface-code memory-X workloads.

## Install

Create a virtual environment and install the benchmark dependencies:

```bash
python3 -m venv .venv-surface-decoder
.venv-surface-decoder/bin/python -m pip install -r benchmarks/surface_decoder_compare/requirements.txt
.venv-surface-decoder/bin/python -m pip install gurobipy
```

## rsinter Cargo Features

`rsinter` defaults to the `full` feature set, so ordinary builds keep every
benchmark runner and plotting command available.

| Feature | Enables |
| --- | --- |
| `default` | The `full` feature set for ordinary `rsinter` builds |
| `rbposd-runner` | The `rbposd` adapter and benchmark runner required by the minimal CSS BP+OSD path |
| `rmatching-runner` | The `rmatching` adapter and benchmark runner |
| `ilp-runner` | The `rilpqec` adapter, `qec-ilp-core`, and HiGHS-backed ILP runner path |
| `plotting` | The `plotters` dependency and `rsinter` plotting implementations |
| `full` | `rbposd-runner`, `rmatching-runner`, `ilp-runner`, and `plotting` |

Full/default build:

```bash
cargo build --locked -p rsinter
```

Minimal CSS `rbposd` build:

```bash
cargo build --locked -p rsinter --no-default-features --features rbposd-runner
```

The minimal `rbposd-runner` build keeps the CSS benchmark run path available
while excluding `rmatching`, `rilpqec`, `qec-ilp-core`, HiGHS, and `plotters`
from the normal and build dependency graph.

## Run

Comparison benchmark:

```bash
make surface-decoder-compare-smoke
make surface-decoder-compare-full
```

Both commands write `results.csv` and a Rust-rendered
`surface_decoder_compare.png` under
`benchmarks/surface_decoder_compare/results/<tier>/`. The comparison runner
still owns the legacy CSV table; the figure is rendered from that table by
`rsinter bench plot-surface-compare-csv`.

Only the `full` tier artifacts are tracked in git. The `smoke` tier is for
local iteration and is ignored.

`rsinter` framework flow:

```bash
make bench-surface-smoke
make bench-surface-full
```

These commands route through the `rsinter` benchmark framework. Rust runners are
executed by the `rsinter` CLI, Python runners are executed by the Python
benchmark entrypoint, and the final merged plot is rendered by `rsinter`.
Artifacts are written under `benchmarks/out/surface_decoder/`.

For future comparison figures, prefer the `rsinter bench plot` path. The
framework commands above render through the same plotter, and the direct command
shape is:

```bash
cargo run -p rsinter --bin rsinter -- bench plot --spec <benchmark.toml> --input <results.jsonl> --out <figure.svg>
```

For existing surface-comparison CSV outputs, use the Rust CSV compatibility
plot command:

```bash
cargo run -p rsinter --bin rsinter -- bench plot-surface-compare-csv --spec <benchmark.toml> --input <results.csv> --out <figure.png>
```

The legacy `plot_compare.py` script is kept as a manual compatibility path for
older CSV comparison outputs. The Makefile targets no longer use it; benchmark
figures should use `rsinter` so zero-error intervals, interval factors, logical
rate units, and series grouping stay aligned with the main benchmark plotter.

To resume an interrupted Rust runner artifact directory, rerun the same command
with `--resume`. Existing completed row identities in
`<out>/<runner>/test-run/results.jsonl` are preserved and skipped; missing or
incomplete identities are rerun and merged through the normal staged
`test-run.tmp` write.

## Notes

- Shared workload: `stim gen surface_code:rotated_memory_x`
- Smoke sweep: `distance in {3}` and `p in {0.002, 0.005, 0.010}`
- Full sweep: `distance in {3, 5}` and `p in {0.002, 0.005, 0.010}`
- Full tier budgets: `max_shots=10000`, `max_errors=200`
- Plotting uses one color per `(decoder family, distance)` pair and uses
  solid/dashed lines to distinguish paired implementations within a family
- Shared public shot pool for all decoders per case
- Time metric: decode time only
- ILP decoders prefer `gurobi` when available and record the actual backend used;
  Python `ilpqec` additionally needs `gurobipy` installed in the benchmark venv

## See Also

- [Benchmark Evidence showcase](docs/showcases/benchmark-evidence.md)
- [Showcase index](docs/showcases/README.md)
