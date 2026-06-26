# Task 2 Report: Python Hard-Replay Runner And Expanded CSV Rows

## Status

DONE_WITH_CONCERNS

## Summary

Implemented the Python-side BB90 hard-replay runner and expanded CSV rows for the replay diagnostic path.

Changed files:

- `benchmarks/bb_circuit_bposd_compare/__init__.py`
- `benchmarks/bb_circuit_bposd_compare/cases.py`
- `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`

## Implemented Requirements

- Added `HARD_REPLAY_CASES` with case id `bb90-p006-c10-seed12345-order7-hard-syndrome`.
- Pinned the hard replay to basis `Z`, `bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, and `osd_order=7`.
- Expanded `CSV_HEADER` with replay metadata columns:
  - `basis`
  - `syndrome_weight`
  - `syndrome_support`
  - `logical_prediction`
  - `expected_logical`
- Expanded `CSV_HEADER` with per-trial profile counters:
  - `bp_seconds`
  - `osd_seconds`
  - `decode_call_count`
  - `bp_iteration_count`
  - `osd_use_count`
  - `osd_candidate_count`
  - `gf2_solve_count`
  - `gf2_full_elimination_count`
- Added optional Rust binary selection with CLI `--rust-binary`.
- Threaded Rust exporter command selection through `run_suite`.
- Added `--tier hard-replay`.
- Added fixture loading and validation against `rsinter/tests/fixtures/bb_circuit_bposd/bb90_hard_syndrome.json`.
- Added Rust hard-replay row construction using `z_logical_prediction` and `z_profile`.
- Added Python hard-replay row construction using `ldpc.BpOsdDecoder` on the replay syndrome.
- Added skipped Python dependency rows for hard replay.
- Hard replay returns nonzero when Python dependencies are missing unless `allow_missing_python=True`.

## TDD Evidence

RED command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare -v
```

RED result:

- Failed before implementation with `ImportError: cannot import name 'HARD_REPLAY_CASES'`.

GREEN command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare -v
```

GREEN result:

- `Ran 9 tests in 7.371s`
- `OK`

Additional check:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_cases.py -v
```

Result:

- `8 passed`
- Pytest emitted a cache write warning because the repository root cache path is outside this sandbox's writable roots.

## Notes

The task brief's fake hard replay model used `augmented_columns` that could not produce `FAKE_HARD_LOGICAL` from `FakeHardDecoder.decode()`. I adjusted only the fake model columns in `test_run_compare.py` so the test matches its stated intent: the fake decoder returns a correction that maps through the model to the same logical prediction as the fake Rust export.

## Concern

`python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke -v` currently fails because `test_verify_smoke.py` constructs row dictionaries without the newly added CSV columns, while `verify_smoke.verify_rows()` requires every `CSV_HEADER` column to be present. I did not edit `test_verify_smoke.py` or `verify_smoke.py` because the task brief scoped Task 2 ownership to `__init__.py`, `cases.py`, `run_compare.py`, and `tests/test_run_compare.py`.

## Follow-up Fix

Updated `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py::make_row()` to include the new replay metadata and profile counter CSV fields with blank defaults for smoke rows. This keeps `verify_smoke.verify_rows()` strict about every shared CSV column while aligning the synthetic smoke rows with the expanded `CSV_HEADER`.

## Follow-up Test Evidence

Command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke -v
```

Result:

- `Ran 7 tests in 0.000s`
- `OK`

Command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare -v
```

Result:

- `Ran 9 tests in 8.473s`
- `OK`

## Follow-up Concern Resolution

The earlier `test_verify_smoke` concern is resolved.
