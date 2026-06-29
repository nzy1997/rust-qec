# QEC-Code Random-Window Benchmark

Run the local `qec-code` random-window upper-bound benchmark evidence pipeline
and compare its summary rows against imported codeDistancePYPI paper baselines
when those external spreadsheets are available.

## What This Shows

This showcase documents the benchmark evidence path for
`qec-code code css-distance random-window-upper-bound`. It runs local
random-window upper-bound searches over pinned case manifests, summarizes the
best local upper bound and elapsed-time distribution, and then joins the local
summary to canonical paper-baseline rows when there is a defensible
codeDistancePYPI match.

Local runs execute only the local `random-window-upper-bound` command. They do
not run QDistEvol, QDistRndMW, m4ri, Gurobi, SAT, or any other external paper
algorithm.

## Run It

Run the smoke pipeline from the repository root:

```sh
make qec-code-random-window-bench-smoke
```

Run the full pipeline only after obtaining the upstream paper-result
spreadsheets separately:

```sh
CODEDISTANCE_PAPER_RESULTS_DIR=/path/to/codeDistancePYPI/paper-results \
  make qec-code-random-window-bench-full
```

## Expected Result

The smoke target validates
`benchmarks/qec_code_random_window/cases.smoke.toml`, builds the local
`qec-code` binary, runs local random-window cases, writes a local summary, and
writes a comparison table under
`benchmarks/out/qec_code_random_window/smoke/`.

Smoke artifacts include:

- `local-runs.jsonl`: one local runner row per smoke case and seed.
- `summary/summary.csv` and `summary/summary.md`: local best upper bound and
  elapsed-time summary rows.
- `paper-baselines.empty.csv`: a header-only canonical baseline CSV used so
  smoke runs need no external spreadsheets.
- `comparison/comparison.csv` and `comparison/comparison.md`: local rows joined
  against the header-only smoke baseline table.

Rows without a paper match show `NA` in paper method, bound, elapsed-time,
delta, ratio, and provenance fields. `NA` means no defensible paper row was
provided to that comparison run; it is not a fabricated baseline and it is not
evidence that the paper has no result.

The full target writes the same artifact shape under
`benchmarks/out/qec_code_random_window/full/`, but first imports canonical
baseline rows from `CODEDISTANCE_PAPER_RESULTS_DIR`.

## Code

Pipeline entry points and generated-output policy:

- [`Makefile`](Makefile)
- [`.gitignore`](.gitignore)

Random-window benchmark modules and manifests:

- [`benchmarks/qec_code_random_window/cases.smoke.toml`](benchmarks/qec_code_random_window/cases.smoke.toml)
- [`benchmarks/qec_code_random_window/cases.full.toml`](benchmarks/qec_code_random_window/cases.full.toml)
- [`benchmarks/qec_code_random_window/validate_cases.py`](benchmarks/qec_code_random_window/validate_cases.py)
- [`benchmarks/qec_code_random_window/run_local.py`](benchmarks/qec_code_random_window/run_local.py)
- [`benchmarks/qec_code_random_window/summarize.py`](benchmarks/qec_code_random_window/summarize.py)
- [`benchmarks/qec_code_random_window/import_paper_baselines.py`](benchmarks/qec_code_random_window/import_paper_baselines.py)
- [`benchmarks/qec_code_random_window/compare_paper.py`](benchmarks/qec_code_random_window/compare_paper.py)
- [`benchmarks/qec_code_random_window/README.md`](benchmarks/qec_code_random_window/README.md)

## Verification

Run the smoke target:

```sh
make qec-code-random-window-bench-smoke
```

Confirm the comparison Markdown contains `NA` baseline fields for smoke rows
without paper data:

```sh
grep 'NA' benchmarks/out/qec_code_random_window/smoke/comparison/comparison.md
```

Run the docs checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-random-window-benchmark.md
```

Run the Makefile and showcase contract test:

```sh
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
```

## Limits

Smoke output is an implementation and wiring check. It is not a final
paper-quality performance claim and should not be cited as statistical evidence.

The smoke target intentionally uses a header-only baseline CSV and non-strict
comparison so it can run from a clean checkout without codeDistancePYPI
spreadsheets. This is why paper baseline fields can be `NA` even for cases that
require paper rows in the full manifest.

Full comparison provenance depends on the external codeDistancePYPI paper
results directory. The spreadsheets are not committed here; obtain them from
the upstream project and point `CODEDISTANCE_PAPER_RESULTS_DIR` at the local
`paper-results` or `paper results` directory before running the full target.
