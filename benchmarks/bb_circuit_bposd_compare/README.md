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

If you want the compare step to complete successfully while still recording skipped Python rows, run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke --output-dir benchmarks/bb_circuit_bposd_compare/results/smoke --allow-missing-python
```

`--allow-missing-python` is only an escape hatch for local environments without the upstream Python decoder stack.
The emitted CSV still contains skipped `ldpc_bposd` rows, so `verify_smoke` continues to reject that output.

## Small-LDPC Catalog Dry Run

The complete #209 `small_ldpc.png` target catalog is checked in as
`SMALL_LDPC_CASES`. It contains 31 manifest cases and does not run the
50,000-trial campaign by default.

Write the dry-run manifest with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier small_ldpc_catalog --output-dir /tmp/rstim-small-ldpc-catalog
```

The command validates the catalog and writes
`/tmp/rstim-small-ldpc-catalog/manifest.csv`.

The dry run is pinned to the upstream decoder settings recorded in
`SMALL_LDPC_CASES`: `num_trials=50000`, `seed=12345`, `bp_method=ms`,
`max_iter=10000`, `osd_method=osd_cs`, `osd_order=7`, and `scaling=0`.
This branch documents `osd_cs` in the manifest because that is the
upstream `ldpc` package spelling equivalent to the older `ldpc_cs`
label still accepted by the validator.

| code_id | cycles | p points | catalog status |
| --- | ---: | ---: | --- |
| `bb72` | 6 | 7 | supported |
| `bb90` | 10 | 7 | supported |
| `bb108` | 10 | 7 | unsupported Rust constructor |
| `bb144` | 12 | 6 | supported |
| `bb288` | 18 | 4 | unsupported Rust constructor |
