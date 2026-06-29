# Issue 326 QEC-Code Random-Window Benchmark Entrypoints Design

Issue #326 completes the random-window benchmark evidence pipeline by adding
repository-level Make targets and a showcase page for the pieces delivered by
#321 through #325.

## Context

The merged dependency PRs provide:

- `benchmarks/qec_code_random_window/cases.smoke.toml` and
  `cases.full.toml` with explicit `baseline_required` fields.
- `validate_cases.py` for manifest validation.
- `run_local.py` for local `qec-code code css-distance
  random-window-upper-bound` JSONL runs.
- `summarize.py` for `summary.csv` and `summary.md`.
- `import_paper_baselines.py` for canonical codeDistancePYPI baseline CSVs.
- `compare_paper.py` for `comparison.csv` and `comparison.md`.

There is no `AGENTS.md` in this checkout. Existing benchmark entry points live
in the root `Makefile`, and generated benchmark output under `benchmarks/out/`
is already ignored by git.

## Chosen Approach

Add root Make targets that wire the existing Python modules together without
adding another orchestration script. This matches the existing benchmark target
style, keeps each command auditable, and leaves the individual modules as the
tested units.

The smoke target will:

1. Validate `cases.smoke.toml`.
2. Build the local `qec-code` binary.
3. Run the local random-window runner into
   `benchmarks/out/qec_code_random_window/smoke/local-runs.jsonl`.
4. Summarize the run into
   `benchmarks/out/qec_code_random_window/smoke/summary/`.
5. Write a header-only canonical paper-baseline CSV under the smoke output.
6. Run non-strict comparison into
   `benchmarks/out/qec_code_random_window/smoke/comparison/`.

The header-only smoke baseline CSV is intentional: smoke runs must not require
external codeDistancePYPI spreadsheets, and non-strict comparison should render
unmatched cases with `NA` paper fields rather than invented provenance.

The full target will use the same pipeline with `cases.full.toml`, import
baselines through `CODEDISTANCE_PAPER_RESULTS_DIR`, and run comparison with
`--strict-baselines`. The full target is allowed to fail early with the
importer's existing message when external paper results are not supplied.

## Documentation

Add `docs/showcases/qec-code-random-window-benchmark.md` with the required
showcase sections. The page must include the exact command:

```sh
make qec-code-random-window-bench-smoke
```

It will explain local inputs, generated outputs, the source of paper baselines,
interpretation limits, the meaning of `NA` baseline rows, and that local runs
execute only `random-window-upper-bound`, not external paper algorithms.

Update `docs/showcases/README.md` so users can discover the new page from the
decoder and benchmark showcase category.

## Tests

Add a focused Python unittest that validates the public contract:

- the root `Makefile` exposes smoke and full targets;
- the smoke target validates, runs, summarizes, and compares without
  `--strict-baselines`;
- the smoke path creates or uses a header-only baseline CSV contract;
- the full target invokes the importer through `CODEDISTANCE_PAPER_RESULTS_DIR`
  and uses `--strict-baselines`;
- the showcase page contains the exact smoke command and the
  `random-window-upper-bound` local-run limitation.

This test is documentation/contract coverage. End-to-end verification still
runs the actual smoke target.

## Risks And Limits

Full benchmark verification depends on external upstream spreadsheets and is
not part of normal smoke verification. Smoke timing is an implementation check,
not paper-quality evidence. The smoke comparison can show `NA` fields even for
manifest cases whose full-run baseline is required, because the no-external-data
path deliberately avoids strict baseline enforcement.
