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

## Run

Comparison benchmark:

```bash
make surface-decoder-compare-smoke
make surface-decoder-compare-full
```

Both commands write `results.csv` and `surface_decoder_compare.png` under
`benchmarks/surface_decoder_compare/results/<tier>/`.

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

The legacy `plot_compare.py` script is kept as a compatibility path for older
CSV comparison outputs; new benchmark figures should use `rsinter bench plot`
so zero-error intervals, interval factors, and series grouping stay aligned
with the main benchmark plotter.

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
