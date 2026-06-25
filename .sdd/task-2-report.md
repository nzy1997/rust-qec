# Task 2 Report

## Scope

Implemented the Task 2 Python package skeleton under `benchmarks/bb_circuit_bposd_compare/`:

- `__init__.py`
- `cases.py`
- `summary.py`
- `verify_smoke.py`
- `tests/__init__.py`
- `tests/test_verify_smoke.py`
- `tests/test_summary.py`

No Rust files or Task 3 runner files were touched.

## TDD Record

1. Added `test_verify_smoke.py` and `test_summary.py` first.
2. Ran:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary
```

3. Observed the expected red-phase import failures because `verify_smoke.py` and `summary.py` did not exist yet.
4. Implemented the minimal package, schema constants, verifier, and summary writer.
5. Re-ran the same unittest command, fixed the pairing rule to require the pinned smoke cases to be paired, and re-ran until green.

## Verification

Passing command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary
```

Observed result:

```text
.....
----------------------------------------------------------------------
Ran 5 tests in 0.001s

OK
```

## Self-Review

- `cases.py` matches the brief’s pinned `CSV_HEADER`, `CompareCase`, and `SMOKE_CASES`.
- `verify_smoke.py` checks for required columns, both decoder implementations, required timing fields on `status=ok` rows, required BB72/BB90 case presence, and required paired diagnostic coverage.
- `summary.py` writes a markdown table for `status=ok` rows with the specified columns.
- Changes are limited to the Python package and tests owned by Task 2.

## Concerns

None.

## Fix Report: review findings follow-up

### What changed

- Tightened `verify_rows()` so completed `decoder_impl=ldpc_bposd` rows must keep the pinned upstream settings: `bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, `osd_order=7`, and `seed=12345`.
- Added a regression test that proves the verifier rejects a completed upstream Python row when one pinned setting is wrong.
- Added a direct empty-CSV diagnostic: `CSV has no data rows`.

### Test command/output

Command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary
```

Red phase before the verifier fix:

```text
....F.
======================================================================
FAIL: test_verify_rows_rejects_mismatched_upstream_pinned_settings
...
AssertionError: 'completed upstream ldpc/bposd row has mismatched pinned setting' not found in ''

----------------------------------------------------------------------
Ran 6 tests in 0.001s

FAILED (failures=1)
```

Green phase after the verifier fix:

```text
......
----------------------------------------------------------------------
Ran 6 tests in 0.001s

OK
```

### Files changed

- `benchmarks/bb_circuit_bposd_compare/verify_smoke.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py`
- `.sdd/task-2-report.md`

### Self-review

- The verifier change is scoped to Task 2 Python code only.
- The new check applies only to completed upstream Python rows, matching the review finding.
- The regression test exercises the actual failure mode instead of a synthetic helper path.
