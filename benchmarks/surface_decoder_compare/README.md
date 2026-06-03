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

```bash
make surface-decoder-compare-smoke
make surface-decoder-compare-full
```

Both commands write `results.csv` and `surface_decoder_compare.png` under
`benchmarks/surface_decoder_compare/results/<tier>/`.

## Notes

- Shared workload: `stim gen surface_code:rotated_memory_x`
- Smoke sweep: `distance in {3}` and `p in {0.002, 0.005, 0.010}`
- Full sweep: `distance in {3, 5}` and `p in {0.002, 0.005, 0.010}`
- Full tier budgets: `max_shots=10000`, `max_errors=200`
- Shared public shot pool for all decoders per case
- Time metric: decode time only
- ILP decoders prefer `gurobi` when available and record the actual backend used;
  Python `ilpqec` additionally needs `gurobipy` installed in the benchmark venv
