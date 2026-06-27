# Task 1 Report: Python Bravyi LER Verifier

## Scope Completed

Implemented `benchmarks/bb_circuit_bposd_compare/verify_bravyi_ler.py` and
`benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py`
to enforce Bravyi-style trial-level logical error rate normalization for
batched BB compare CSV rows.

The verifier:

- accepts `ok` and `partial` batched rows,
- checks that `logical_error_rate == logical_errors / shots_used`,
- rejects per-cycle-normalized rows with an explicit error,
- exposes `VerifiedRow.bravyi_tuple`,
- loads CSV rows from disk,
- and provides a CLI at `python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler <csv_path>`.

## TDD Evidence

### RED

Command:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
```

Observed failure before implementation:

```text
ImportError: cannot import name 'verify_bravyi_ler' from 'benchmarks.bb_circuit_bposd_compare'
```

That failure was expected because the verifier module had not been added yet.

### GREEN

Command:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
```

Result: `5 passed`.

Command:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Result: exit code `0` and PASS rows printed, including:

```text
PASS bb144-p0030-c12-t1000000-seed12345 rbposd 40000 200 0.0050000000000000001 bravyi_tuple=(0.003, 12, 40000, 200)
```

## Notes

- The checked-in full results CSV already contains trial-level normalization
  for the BB144 row used in the test.
- Pytest emitted a cache write warning because the workspace cannot write to
  the repo root `.pytest_cache`; this did not affect test results.

## Review Fix

### RED

After tightening the tests to the review findings, the focused suite failed in
the expected places:

```text
TypeError: 'VerificationResult' object is not iterable
```

This showed the verifier was still returning the old wrapper instead of the
required `list[VerifiedRow | VerificationError]`.

The CLI column-shape test also failed on the first header token:

```text
AssertionError: assert ['status', ...] == ['case_id', ...]
```

That confirmed the table still exposed a `status` header, which the review asked
to remove.

### GREEN

The implementation now returns a list of `VerifiedRow` and `VerificationError`
instances, so callers can partition with `isinstance(...)`.
The CLI header is now exactly:

```text
case_id decoder_impl shots_used logical_errors logical_error_rate bravyi_tuple
```

Verification commands:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Both passed on the final run.

## Second Review Fix

### RED

Added a regression test for malformed accepted rows:

```python
verify_bravyi_ler.verify_rows([make_row(shots_used="bad")])
```

The focused pytest run failed in the broken parse path, first with the undefined
`errors` name and then with a message that did not identify the numeric field.
That confirmed the parser was still not returning a `VerificationError` for bad
numeric input.

### GREEN

`_parse_row` now returns `VerificationError` instances for parse and validation
failures, and the error text names the specific field when numeric parsing
fails.

Verification commands:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Both passed on the final run, and the new regression test now covers the
malformed accepted-row case.
