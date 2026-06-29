# Issue 323 QEC-Code Random-Window Summary Design

## Context

Issue #323 adds the reporting layer for the local random-window benchmark
runner from issue #322. The runner already emits JSONL rows for cases described
by the issue #321 TOML manifests. The summarizer must turn those raw rows into
a compact, stable case-level summary that can later be compared with imported
paper data.

GitHub issue API reads were blocked by the local sandbox proxy during this run.
The design uses the provided issue body, the merged #321/#322 commits, and the
local manifest and runner artifacts.

## Approaches Considered

1. Add a standard-library Python summarizer beside the existing manifest
   validator and runner.
   - Pros: matches the requested module interface, reuses existing manifest
     loading, needs no new dependencies, and keeps the benchmark folder
     self-contained.
   - Cons: Markdown table rendering is implemented locally.

2. Add summary support to `run_local.py`.
   - Pros: fewer benchmark entry points.
   - Cons: mixes raw data collection with reporting and makes it harder to
     summarize multiple previously captured JSONL files.

3. Add a Rust summarizer.
   - Pros: could share future typed benchmark structures if the raw data moves
     into Rust.
   - Cons: the requested interface is a Python module and the existing #321/#322
     benchmark utilities are Python standard-library scripts.

Chosen approach: a standalone Python standard-library module
`benchmarks/qec_code_random_window/summarize.py`.

## Design

The summarizer CLI is:

```bash
python3 -m benchmarks.qec_code_random_window.summarize \
  --cases benchmarks/qec_code_random_window/cases.smoke.toml \
  --runs /tmp/qec-rw-smoke.jsonl \
  --out-dir /tmp/qec-rw-summary
```

`--runs` accepts one or more JSONL paths. The summarizer loads and validates
the manifest with the existing #321 `load_manifest` and `validate_manifest`
functions. It then reads every JSONL line, validates the row shape required for
summarization, groups rows by `case_id`, and emits exactly one summary row for
each manifest case in manifest order.

Attempted seed rows are counted as all JSONL rows for a manifest case.
Successful seed rows are rows with `status = "ok"`. Best upper bound is the
minimum `upper_bound` across successful rows only. Elapsed-time statistics are
computed over successful rows only using Python `statistics.median` for the
median. Cases with no successful rows keep numeric result fields blank and are
marked `no_success`.

If a manifest case has `target_upper_bound`, a successful row hits the target
when `upper_bound <= target_upper_bound`. The summary records both
`target_hit_count` and `target_hit_rate`; the rate is blank when there are zero
successful rows or no manifest target. The summary preserves
`baseline_key` and `baseline_required` directly from the manifest for #325.

## Output Contract

The summarizer creates the output directory and writes:

- `summary.csv`
- `summary.md`

CSV rows include:

- manifest identity: `case_id`, `code_id`, `distance_side`
- baseline metadata: `baseline_key`, `baseline_required`
- manifest settings: `manifest_seed`, `manifest_iterations`,
  `manifest_restarts`, `manifest_target_weight`, `target_upper_bound`
- run settings observed from JSONL rows: `run_seed_values`,
  `run_iterations_values`, `run_restarts_values`, `run_target_weight_values`,
  and `run_status_values`
- metrics: `attempted_seed_rows`, `successful_seed_rows`, `best_upper_bound`,
  `median_elapsed_s`, `min_elapsed_s`, `max_elapsed_s`, `target_hit_count`,
  `target_hit_rate`, and `summary_status`

Markdown includes a short provenance section with the manifest path, run file
paths, manifest suite/version, and the exact summarizer argv. Its table has one
row per manifest case and prints `NO SUCCESSFUL ROWS` in the status column for
cases without successful rows.

## Validation and Error Handling

The summarizer exits nonzero with file and line context when:

- a JSONL line is malformed JSON or not an object
- a row has no string `case_id`
- a row references a `case_id` not present in the manifest
- a row with `status = "ok"` is missing a positive integer `upper_bound`
- a row with `status = "ok"` is missing numeric `elapsed_s`
- an output file cannot be written

Non-`ok` rows do not need `upper_bound` and do not contribute to elapsed-time
statistics, but they still count as attempted seed rows and contribute to the
observed status list.

## Testing

Add `benchmarks/qec_code_random_window/tests/test_summarize.py` and checked
fixtures under `benchmarks/qec_code_random_window/tests/fixtures/`.

Required coverage:

- a fixture JSONL with known rows produces exact expected `summary.csv` values,
  including best upper bound and median elapsed seconds
- `summary.md` contains one table row per manifest case and clearly marks cases
  with zero successful rows
- a malformed JSONL fixture missing `upper_bound` for a successful row exits
  nonzero with a useful error
- `baseline_required` and `baseline_key` from the manifest are preserved in the
  summary
- `python3 -m benchmarks.qec_code_random_window.summarize --help` exits 0

Required verification:

- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_run_local benchmarks.qec_code_random_window.tests.test_summarize -q`
- `python3 -m benchmarks.qec_code_random_window.summarize --help`
- `cargo test`
