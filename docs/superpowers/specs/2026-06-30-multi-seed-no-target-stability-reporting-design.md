# Multi-Seed No-Target Stability Reporting Design

## Context

Issue #339 extends the qec-code random-window benchmark reporting path for
release/no-target smoke runs. Issue #335 already added a release no-target
BB smoke manifest and runner path. Issues #337 and #338 added a no-target
ladder suite and search-stat summary aggregation.

The existing runner already accepts `--seeds` and emits one JSONL row per
case/seed pair. The summarizer already aggregates per-case rows into CSV and
Markdown, including attempted rows, successful rows, best upper bound,
elapsed-time min/median/max, target-hit counts/rates, and observed seed
values. The missing pieces are explicit validation that a multi-seed no-target
summary is complete and homogeneous, clearer Markdown wording for the
multi-seed fields, and a small repeatable Make target for the issue's
three-seed smoke path.

This run is non-interactive. The design is approved under the standing answer
policy because the issue body gives exact input/output expectations,
verification commands, and out-of-scope constraints.

## Explored Approaches

### Approach A: summary-only validation and clarity

Extend `summarize.py` to validate the observed run rows against manifest
settings and observed build profiles. Keep the JSONL shape unchanged and
continue relying on `run_local.py --seeds` for per-seed execution.

Pros: smallest behavioral surface, no random-window search changes, preserves
per-seed rows, and aligns with the issue recommendation to reuse existing seed
support.

Cons: the summarizer must infer the expected seed set from observed rows plus
manifest settings, because the manifest stores only the default seed.

### Approach B: add a manifest-level seed list

Teach case manifests to carry `seeds = [7, 11, 17]`, have the runner execute
that list by default, and have the summarizer validate against that manifest
field.

Pros: explicit persistent seed sets.

Cons: larger manifest schema change, more validator/test churn, and not needed
for the interface requested by the issue, which passes seeds on the runner
command line.

### Approach C: aggregate in the runner

Have `run_local.py` write both per-seed JSONL rows and aggregate summary rows.

Pros: the runner knows the requested seed list directly.

Cons: duplicates summarizer responsibilities, risks weakening exact per-seed
JSONL preservation, and mixes execution with reporting.

## Selected Design

Use Approach A, with a small Make target from the issue recommendation.

The runner remains per-seed only. The multi-seed no-target workflow is:

1. Build `target/release/qec-code`.
2. Run `benchmarks.qec_code_random_window.run_local` on
   `cases.no-target-smoke.toml` with `--build-profile release --seeds 7 11 17`.
3. Run `benchmarks.qec_code_random_window.summarize` on the resulting JSONL.

The Make target `qec-code-random-window-bench-no-target-multiseed-smoke` will
wrap that sequence and write to
`benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/`.

## Reporting Contract

For each manifest case, the summarizer must write one summary row with:

- `attempted_seed_rows`: number of JSONL rows observed for the case.
- `successful_seed_rows`: number of rows with `status = "ok"`.
- `run_seed_values`: sorted semicolon-separated observed seeds.
- `best_upper_bound`: best successful upper bound, blank if no successful rows.
- `target_hit_count`: number of successful rows with
  `upper_bound <= target_upper_bound`, when `target_upper_bound` is present.
- `target_hit_rate`: fixed six-decimal ratio over successful rows, blank when
  no successful rows exist.
- `median_elapsed_s`, `min_elapsed_s`, and `max_elapsed_s`: computed from
  successful rows only.
- `run_target_weight_values`: blank when all observed rows have
  `target_weight = null`, which makes no-target runs visibly unset in CSV.
- `run_build_profile_values`: sorted semicolon-separated observed build
  profiles, so release/debug mixing is visible and validated.

The Markdown case table must make the same multi-seed nature clear without
requiring readers to open the CSV. It should include attempted/successful seed
counts, observed seeds, target-hit count/rate, elapsed distribution, observed
target-weight values, and observed build profiles.

## Validation Contract

The summarizer must reject invalid run sets before writing summary files.

For every case with at least one observed row:

- Every row must match the manifest `iterations` and `restarts`.
- Every row must match the manifest `target_weight`; for no-target manifests,
  every row must have `target_weight = null`.
- Every row with a `target_upper_bound` field must match the manifest
  `target_upper_bound`.
- Every row with a `build_profile` field must agree with all other rows for
  the same case. A mixed `release`/`debug` set is rejected with an error naming
  the case and `build_profile`.
- The case must have exactly one attempted row per observed seed, so duplicate
  seed rows are rejected.

For no-target cases, if at least one case observes a multi-seed set, every
no-target case in the same summary must observe the same seed set. This rejects
the issue's negative-control case where one required seed is missing for a
case. Existing single-seed summaries remain valid because no multi-seed set is
present.

## Tests

Add `benchmarks.qec_code_random_window.tests.test_multiseed_summary` with:

- a positive fixture that summarizes two no-target cases with seeds
  `7, 11, 17`, mixed successful/error statuses, target-hit counts/rates, and
  elapsed distributions;
- assertions that `summary.csv` and `summary.md` expose the required
  multi-seed fields;
- a negative-control test named
  `test_rejects_missing_seed_or_mixed_build_profile` that constructs one run
  set missing seed `17` for a case and another run set mixing
  `build_profile = "debug"` into a release/no-target summary. Each rejection
  must name the affected case and field.

Existing summarizer tests continue to pass. Search-stat aggregation from
issue #338 remains unchanged.

## Out of Scope

This design does not change random-window sampling semantics, does not pass
`--target-weight` for no-target smoke runs, does not add statistical
significance claims or hard performance thresholds, and does not require any
external reference implementation.
