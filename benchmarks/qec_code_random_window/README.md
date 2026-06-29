# QEC Random Window Benchmark Baselines

This benchmark can import external paper results from
`https://github.com/m-webster/codeDistancePYPI`.

The upstream spreadsheets are external data and are not committed in this
repository. Clone or download that repository separately, then point the
importer at its `paper results/` directory from outside this checkout.

For real `.xlsx` workbook compatibility, install `openpyxl` in the environment
that runs the importer.

Run the importer with an explicit paper-results path:

```bash
python3 -m benchmarks.qec_code_random_window.import_paper_baselines \
  --cases benchmarks/qec_code_random_window/cases.full.toml \
  --paper-results-dir /path/to/codeDistancePYPI/paper-results \
  --out /tmp/codeDistancePYPI-baselines.csv
```

You can also set `CODEDISTANCE_PAPER_RESULTS_DIR` instead of passing
`--paper-results-dir` on the command line.

The canonical CSV includes only defensible mapped rows from the upstream paper
results and omits unmapped cases instead of guessing at a match.
