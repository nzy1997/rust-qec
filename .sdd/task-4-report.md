# Task 4 Report

## Scope

Implemented Task 4 within the requested ownership boundary:

- `Makefile`
- `benchmarks/bb_circuit_bposd_compare/README.md`
- `benchmarks/bb_circuit_bposd_compare/tests/test_verify_smoke.py`

No unrelated files were reverted. Generated result artifacts under
`benchmarks/bb_circuit_bposd_compare/results/` were left uncommitted.

## Changes

1. Added `bb-circuit-bposd-compare-smoke` to `.PHONY` and `help`.
2. Added the smoke target to run:
   - `python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier smoke --output-dir benchmarks/bb_circuit_bposd_compare/results/smoke`
   - `python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv`
3. Added `benchmarks/bb_circuit_bposd_compare/README.md` documenting:
   - the make target
   - Python dependency install hint
   - expected output paths
   - missing-dependency behavior
4. Added a focused verifier unit test covering a missing required smoke case.

## Verification

### Python unit tests

Command:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary benchmarks.bb_circuit_bposd_compare.tests.test_run_compare
```

Result: PASS (`Ran 13 tests`, `OK`)

### Focused Rust tests

Commands:

```bash
cargo test -p rsinter build_code_supports_bb72_smoke_shape -q
cargo test -p rsinter comparison_case_export_contains_models_samples_and_profile -q
cargo test -p rsinter rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle -q
```

Result: PASS for all three commands.

### Smoke command

Command:

```bash
make bb-circuit-bposd-compare-smoke
```

Result in this environment: FAIL as expected because Python `ldpc` is unavailable.

Observed generated artifacts:

- `benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv`
- `benchmarks/bb_circuit_bposd_compare/results/smoke/summary.md`

Observed `results.csv` rows:

- Rust `rbposd` rows completed with `status=ok` for `bb72` and `bb90`
- Python `ldpc_bposd` rows were written with `status=skipped`
- Error text included: `python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'`

Observed `summary.md`:

- Contains the smoke summary table for the completed Rust rows

### Direct verifier

Command:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv
```

Result: FAIL as expected in the missing-dependency environment.

Observed verifier stderr:

```text
no paired Rust/Python diagnostic case is present
```

### Negative control: missing upstream Python rows

Commands:

```bash
python3 - <<'PY'
import csv
from pathlib import Path
src = Path("benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv")
dst = Path("/tmp/bb-compare-missing-ldpc.csv")
rows = list(csv.DictReader(src.open()))
with dst.open("w", newline="") as f:
    writer = csv.DictWriter(f, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows([r for r in rows if r["decoder_impl"] != "ldpc_bposd"])
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke /tmp/bb-compare-missing-ldpc.csv
```

Result: FAIL as expected.

Observed verifier stderr included:

- `upstream ldpc/bposd comparison row is missing`
- `no paired Rust/Python diagnostic case is present`

### Negative control: unpaired case IDs

Commands:

```bash
python3 - <<'PY'
import csv
from pathlib import Path
src = Path("benchmarks/bb_circuit_bposd_compare/results/smoke/results.csv")
dst = Path("/tmp/bb-compare-unpaired-cases.csv")
rows = list(csv.DictReader(src.open()))
for row in rows:
    if row["decoder_impl"] == "ldpc_bposd":
        row["case_id"] = row["case_id"] + "-python-only"
with dst.open("w", newline="") as f:
    writer = csv.DictWriter(f, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows(rows)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke /tmp/bb-compare-unpaired-cases.csv
```

Result: FAIL as expected.

Observed verifier stderr included:

- `no paired Rust/Python diagnostic case is present`

### Broad Rust verification

Command:

```bash
cargo test
```

Result: PASS.

## Notes / Concerns

1. The local environment does not provide the upstream Python `ldpc` dependency, so the smoke target exits nonzero after producing skipped Python rows. This matches the issue's documented allowed failure mode.
2. The current `run_compare` default output directory is `benchmarks/bb_circuit_bposd_compare/results/`, while Task 4 requires smoke artifacts under `.../results/smoke/`. To satisfy the Task 4 contract without widening ownership into Task 3 code, the new make target passes `--output-dir benchmarks/bb_circuit_bposd_compare/results/smoke` explicitly.
3. Running `python3 -m benchmarks.bb_circuit_bposd_compare.verify_smoke ...` emits a `runpy` warning because the package `__init__.py` imports `verify_smoke` before module execution. The verifier behavior and exit codes were still correct; this report records the warning rather than changing out-of-scope package wiring.

## Self-review

- Spec coverage: met for Make target, README, smoke artifact paths, missing-dependency behavior, verifier behavior, negative controls, and broad Rust verification.
- Scope control: edits stayed within the requested task boundary.
- Remaining risk: the only non-green command is the smoke verifier path under missing Python deps, which is expected and documented above.

---

## Review Fix Addendum

### What changed

1. Updated `benchmarks/bb_circuit_bposd_compare/run_compare.py` so the CLI prints skipped `ldpc_bposd` dependency errors to stderr before exiting nonzero. The emitted text reuses the existing row message format, including `python dependency unavailable for ldpc_bposd replay: ...`.
2. Added CLI-facing tests in `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py` covering:
   - default behavior: missing Python dependencies return nonzero and print the dependency message to stderr
   - allowed behavior: `--allow-missing-python` returns zero and does not print the fatal dependency message
3. Updated `benchmarks/bb_circuit_bposd_compare/README.md` to document the `--allow-missing-python` escape hatch and clarify that `verify_smoke` still rejects outputs with skipped Python rows.

### Test command/output

Commands run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke benchmarks.bb_circuit_bposd_compare.tests.test_summary
make bb-circuit-bposd-compare-smoke
```

Observed results:

- `python3 -m unittest ...`: PASS (`Ran 15 tests`, `OK`)
- `make bb-circuit-bposd-compare-smoke`: FAIL as expected in this environment because `ldpc` is not installed, and stderr now includes:

```text
python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'
```

### Files changed

- `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- `benchmarks/bb_circuit_bposd_compare/README.md`
- `.sdd/task-4-report.md`

### Self-review

- The fix stays inside the allowed Task 4 ownership boundary and does not touch Rust sources.
- The new tests exercise the user-visible stderr behavior directly instead of only inferring it from CSV contents.
- The CLI now surfaces the missing dependency reason in the same command output path that `make bb-circuit-bposd-compare-smoke` exposes, which closes the reported usability gap.
