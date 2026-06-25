# Task 3 Report

## Scope

Implemented the Python comparison runner for the BB circuit BPOSD smoke benchmark in:

- `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py`

## What Changed

- Added `run_suite(...)` to orchestrate:
  - Rust JSON export via `cargo run -q -p rsinter --bin rsinter -- bb-circuit-bposd-memory ... --json-compare-case`
  - Rust row conversion into benchmark CSV rows
  - Python upstream replay via in-function `numpy` and `ldpc.BpOsdDecoder` imports
  - CSV writing to `results.csv`
  - markdown summary writing to `summary.md`
- Added CLI entrypoint:
  - `python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke`
  - supports `--output-dir` and `--allow-missing-python`
- Added TDD coverage for missing Python dependencies:
  - nonzero exit when Python replay is skipped by default
  - zero exit with `--allow-missing-python` semantics
  - skipped rows still fail `verify_rows(...)`
- Added smoke-verifier coverage proving skipped `ldpc_bposd` rows do not satisfy the paired-row requirement.

## TDD Record

1. Wrote `test_run_compare.py` first.
2. Ran the required red command:

   ```bash
   python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
   ```

   Observed expected failure: `ModuleNotFoundError` because `benchmarks.bb_circuit_bposd_compare.run_compare` did not exist.

3. Implemented `run_compare.py`.
4. Re-ran the required command and it passed.

## Verification

Required command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed: `Ran 2 tests ... OK`

Adjacent touched tests:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed: `Ran 8 tests ... OK`

## Self-Review

- Scope stayed within the task-owned benchmark runner and related tests.
- Missing Python dependency handling is explicit and leaves CSV evidence behind.
- The smoke verifier still rejects skipped Python rows, matching the brief.
- No unrelated files were reverted or modified.

## Concerns

- The unit tests exercise the missing-dependency path and row-writing contract, but they do not execute a real `ldpc` replay or live `cargo` export. That behavior remains integration-level coverage.

## Fix Report: Review Finding - Python Row Pinned Settings

### What Changed

- Updated `benchmarks/bb_circuit_bposd_compare/run_compare.py` so `ldpc_bposd` rows no longer inherit replay metadata from `CompareCase`.
- Added explicit pinned upstream settings for Python replay rows:
  - `bp_method=ms`
  - `max_iter=10000`
  - `osd_method=osd_cs`
  - `osd_order=7`
  - `seed=12345`
- Reused those pinned values both when constructing `BpOsdDecoder` instances and when writing successful or skipped Python CSV rows.
- Added a regression test in `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py` that fakes `numpy` and `ldpc` imports, drives a successful `_python_row(...)` call with a deliberately mismatched case, and verifies both:
  - the CSV row records the pinned upstream settings
  - the fake decoders were constructed with the same pinned settings

### Test Command / Output

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed:

```text
...
----------------------------------------------------------------------
Ran 3 tests in 0.002s

OK
```

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed:

```text
.........
----------------------------------------------------------------------
Ran 9 tests in 0.002s

OK
```

### Files Changed

- `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- `.sdd/task-3-report.md`

### Self-Review

- The fix stays inside the Task 3 runner and its tests.
- The regression is targeted at the exact mismatch from review: a successful Python row can no longer mirror altered case metadata.
- The fake-module test avoids a real `ldpc` dependency while still checking the import-time path and constructor arguments.

## Fix Report: Review Finding - Missing Dependency Import Failures

### What Changed

- Updated `benchmarks/bb_circuit_bposd_compare/run_compare.py` so `run_suite(...)` handles import-time dependency failures from `_python_row(...)` for both `ModuleNotFoundError` and dependency-shaped `ImportError`.
- Added explicit dependency classification via `_is_missing_python_dependency(...)`:
  - skips rows only for import failures that clearly point at `ldpc`, `bposd`, `numpy`, or `BpOsdDecoder`
  - re-raises unrelated `ImportError` cases so runtime failures still surface as errors instead of being silently downgraded
- Improved skipped-row evidence text to say `python dependency unavailable for ldpc_bposd replay: ...`, which makes the CSV failure mode clearer.
- Added regression coverage in `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py` for a broken `ldpc` import path:
  - `ImportError("cannot import name 'BpOsdDecoder' from 'ldpc'")` now produces skipped `ldpc_bposd` rows
  - the suite still returns nonzero without `allow_missing_python`
  - `verify_rows(...)` still rejects the CSV as not green

### Test Command / Output

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed:

```text
....
----------------------------------------------------------------------
Ran 4 tests in 0.002s

OK
```

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed:

```text
..........
----------------------------------------------------------------------
Ran 10 tests in 0.002s

OK
```

### Files Changed

- `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- `.sdd/task-3-report.md`

### Self-Review

- The change stays inside the Task 3 runner, its unit tests, and the task report.
- Missing or broken `ldpc`/`bposd` import-time failures now leave CSV evidence instead of aborting the suite mid-stream.
- The classification remains narrow enough that non-dependency `ImportError` failures continue to raise instead of being mislabeled as skips.

## Fix Report: Review Finding - Unrelated ImportError Boundary

### What Changed

- Added a regression test in `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py` covering the opposite boundary of `_is_missing_python_dependency(...)`.
- The new test forces `_python_row(...)` to raise `ImportError("cannot import name 'frobnicate' from 'internal_helpers'")` and asserts that:
  - `run_suite(...)` re-raises that `ImportError`
  - no `results.csv` file is written, proving the suite aborts instead of converting the failure into skipped `ldpc_bposd` rows
- Left `benchmarks/bb_circuit_bposd_compare/run_compare.py` unchanged because it already behaved correctly.

### Test Command / Output

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed:

```text
.....
----------------------------------------------------------------------
Ran 5 tests in 0.002s

OK
```

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Observed:

```text
...........
----------------------------------------------------------------------
Ran 11 tests in 0.002s

OK
```

### Files Changed

- `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- `.sdd/task-3-report.md`

### Self-Review

- The regression is narrowly scoped to the review finding and does not broaden ownership into Rust files or Task 4 docs.
- It proves the intended control-flow boundary directly at `run_suite(...)`, which is where a mistaken dependency classification would silently turn a real failure into skipped rows.
- The implementation stayed unchanged, which is preferable here because the existing behavior already matched the review requirement.
