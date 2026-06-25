# BB Circuit rbposd vs ldpc/bposd Smoke Comparison

Run the smoke comparison from the repository root with:

```bash
make bb-circuit-bposd-compare-smoke
```

The target runs:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke --output-dir benchmarks/bb_circuit_bposd_compare/results/smoke
python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv
```

If you need the upstream Python decoder stack locally, install:

```bash
python3 -m pip install 'ldpc>=2.4.1' bposd numpy
```

Artifacts are written under `benchmarks/bb_circuit_bposd_compare/results/smoke/`:

- `results.csv`: paired Rust `rbposd` and Python `ldpc_bposd` comparison rows
- `summary.md`: smoke-tier markdown summary of the recorded timings and outcomes

Missing Python dependencies are an expected local failure mode. In that case `run_compare` still writes
`results.csv` and `summary.md`, marks the Python `ldpc_bposd` rows as `status=skipped`, and exits nonzero with
an explicit dependency error. The follow-up verifier then rejects that CSV because the smoke contract requires
completed paired Rust/Python rows.
