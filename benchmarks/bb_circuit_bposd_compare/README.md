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

## BB72/BB144 Batched Compare

The BB72/BB144 batched compare keeps the paired Rust/Python comparison, but it
does not write per-trial syndrome or logical data to disk. Each batch is sampled
and decoded by Rust, replayed immediately by Python `ldpc_bposd`, accumulated
into aggregate rows, and then discarded. The output is only:

- `results.csv`: aggregate Rust/Python rows with shots, logical errors, timing,
  batch counts, and stop reason
- `summary.md`: compact timing table
- `bb_circuit_bposd_compare.png`: two-panel plot of logical error rate and
  seconds per shot versus physical error rate, rendered by Rust `rsinter`

The plot smoke uses the same BB72/BB144 physical-error-rate grid as the full
suite, but uses 10 trials per point and prints per-case progress while it runs:

```bash
make bb-circuit-bposd-compare-plot-smoke
```

The full suite uses the same physical-error-rate grid with shared shot/error
budgets:

| code_id | p values | max shots | max errors |
| --- | --- | ---: | ---: |
| `bb72` | `0.003, 0.004, 0.005, 0.006` | `1000000` | `200` |
| `bb144` | `0.003, 0.004, 0.005, 0.006` | `1000000` | `200` |

Run it explicitly with:

```bash
make bb-circuit-bposd-compare-full
```

The `full` tier does not set a wall budget by default. It checks the error
budget between batches; once either decoder has accumulated 200 logical errors,
the current point is written with `status=ok` and
`stop_reason=errors_budget_reached`. If `--wall-budget-seconds` is provided
manually and expires, the current aggregate rows are written with
`status=partial` and `stop_reason=wall_budget_exhausted`.

## Diagnostic Tier

The diagnostic tier runs selected high-p BB points with one trial per case. It
is meant to exercise harder syndromes without launching the full 50,000-trial
campaign.

```bash
cargo build --release -p rsinter
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier diagnostic \
  --output-dir /tmp/rstim-bb-diagnostic \
  --rust-binary target/release/rsinter
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.verify_diagnostic \
  /tmp/rstim-bb-diagnostic/results.csv
```

| code_id | p | cycles | trials | seed |
| --- | ---: | ---: | ---: | ---: |
| `bb90` | 0.006 | 10 | 1 | 12345 |
| `bb144` | 0.006 | 12 | 1 | 12345 |

Both rows use `bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, and
`osd_order=7`. The verifier requires paired Rust/Python rows for both cases
and Rust OSD/GF(2) counters. Missing Python dependencies produce skipped rows
and verifier failure unless `verify_diagnostic --allow-missing-python` is used.

## Bravyi Effective Model Audit

The BB72 model audit builds the Rust effective decoder models without Monte
Carlo trials and verifies their source-backed contract evidence against the
pinned Bravyi fixture:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit \
  --code-id bb72 \
  --physical-error-rate 0.003 \
  --num-cycles 6 \
  --out /tmp/rstim-bb-model-audit/model_audit.json
python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit \
  /tmp/rstim-bb-model-audit/model_audit.json
```

The verifier checks BB72 shape, schedule labels and counts,
`num_cycles_plus_tail=8`, X/Z `first_logical_row`, decoder dimensions,
grouped-column hashes, and grouped probability totals. A negative control can
copy the audit JSON, change `observed.syndrome_tail.noiseless_tail_cycles` from
`2` to `1`, and rerun `verify_model_audit`; the verifier exits nonzero and
names the tail-cycle mismatch.

## Full-Campaign Readiness Gate

Before launching the full BB small-LDPC campaign, collect the prerequisite
artifacts under one results directory and run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.ready_for_full --results-dir /tmp/rstim-bb-ready
```

The gate validates these required artifacts:

- `hard-replay/results.csv`: paired BB90 hard-syndrome replay rows accepted by
  `verify_replay`.
- `hard-profile/profile.json`: release hard-profile JSON with
  `osd_planner=ldpc_osd_cs`, `candidate_limit=16`, bounded OSD candidates, one
  optimized GF(2) solve, one full elimination, and consistent basis decode
  counters.
- `setup-run/profile.json`: BB p-point profile evidence with one code,
  syndrome-cycle, effective-model, and decoder build; `sample_count` equal to
  `num_trials`; and consistent Z/X decode-call counters.
- `small-ldpc-catalog/manifest.csv`: the complete 31-row small-LDPC manifest
  accepted by `validate_small_ldpc_catalog`.
- `diagnostic/results.csv`: paired high-p BB90 and BB144 diagnostic rows
  accepted by `verify_diagnostic`.

Optional `provenance.json` may include `artifact_hash`, `command`, or
`timestamp`. Missing provenance produces `WARN`, but missing, stale, malformed,
skipped, or failing required artifacts produce `FAIL` and a nonzero exit. The
gate does not use wall-clock age thresholds and does not run the full campaign.
`PASS` and `WARN` both exit 0; only `FAIL` exits nonzero.

## Reviewer Readiness Report

After collecting a `/tmp/rstim-bb-ready` artifact tree accepted by the
readiness gate, generate the reviewer-readable Markdown report with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.write_readiness_report \
  --results-dir /tmp/rstim-bb-ready \
  --out /tmp/bb-bposd-readiness.md
python3 -m benchmarks.bb_circuit_bposd_compare.validate_readiness_report \
  --results-dir /tmp/rstim-bb-ready \
  --report /tmp/bb-bposd-readiness.md
```

The report includes semantic parity replay status, BB90 hard-profile counters,
setup/run split evidence, high-p diagnostic Rust/Python compare rows, complete
small-LDPC catalog coverage, and the final verdict from `ready_for_full`.

The validator rebuilds the same readiness model from source artifacts and
compares it to the report snapshot and visible section content. It rejects
stale reports, missing source sections, placeholder headings, and reports whose
visible final verdict does not match the #286 readiness gate.

## BB90 Hard-Syndrome Replay

After building `rsinter`, replay the checked-in BB90 hard-syndrome fixture with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-replay \
  --rust-binary target/release/rsinter

python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay \
  /tmp/rstim-bb90-hard-replay/results.csv
```

The replay writes one Rust `rbposd` row and one Python `ldpc_bposd` row for
`bb90-p006-c10-seed12345-order7-hard-syndrome`. Both rows use
`bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, and `osd_order=7`.
The verifier checks that the rows are paired on the fixture basis/syndrome and
that Rust and Python logical predictions match. Rust rows also carry the OSD
and GF(2) counters from the replay decode.

The hard replay also writes `hard_replay_trace.json`, a one-case correction
trace for the pinned Z-basis syndrome. Validate it with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay_trace \
  /tmp/rstim-bb90-hard-replay/hard_replay_trace.json
```

The CSV verifier remains the historical parity gate and exits nonzero when the
logical predictions differ. The trace verifier is the diagnostic gate for that
case: it accepts a complete paired trace and records the mismatch as
`classification=logical_prediction_mismatch`.

Missing Python decoder dependencies remain explicit: `run_compare` records a
skipped Python row and exits nonzero unless `--allow-missing-python` is passed.
`verify_replay` also rejects skipped Python rows unless its own
`--allow-missing-python` diagnostic flag is used. `verify_replay_trace` always
requires a complete paired correction trace, so it rejects the incomplete trace
written by `run_compare --allow-missing-python`.

### Counter-Bounded Release Smoke

The hard-syndrome performance smoke is intentionally counter-gated rather than
wall-clock-gated. Run the positive release profile with:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_release_profile_is_counter_bounded -- --nocapture
```

The test prints profile JSON with `decode_seconds`, `bp_seconds`,
`osd_seconds`, `osd_candidate_count`, `gf2_solve_count`, and
`gf2_full_elimination_count`. The timing fields are evidence only; the pass/fail
checks assert that the BB90 fixture uses the bounded `ldpc_osd_cs` candidate
plan, one GF(2) full elimination, one optimized GF(2) solve, and consistent
per-basis decode-call counters.

The legacy exhaustive/frontier negative control is:

```bash
cargo test --release -p rsinter bb90_hard_syndrome_legacy_profile_fails_ldpc_cs_bounds -q
```

It verifies that the same validator rejects the legacy profile and names the
violating counter.
