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
