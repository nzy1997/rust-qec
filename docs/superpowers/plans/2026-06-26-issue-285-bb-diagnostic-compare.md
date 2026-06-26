# Issue 285 BB Diagnostic Compare Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a cheap high-p `diagnostic` tier that writes paired Rust/Python BB90 and BB144 compare rows and verifies exact coverage, pairing, counters, and skipped-Python behavior.

**Architecture:** Keep `cases.py` as the source of case truth, reuse `run_compare.run_suite()` for full Rust/Python paired rows, populate aggregate Rust counter columns from `rust_result.profile`, and add a focused `verify_diagnostic.py` for the new CSV contract. The diagnostic tier is separate from `smoke`, `small_ldpc_catalog`, and `hard-replay`.

**Tech Stack:** Python benchmark harness under `benchmarks/bb_circuit_bposd_compare`, existing Rust `rsinter bb-circuit-bposd-memory --json-compare-case` release binary, pytest/unittest-style Python tests, Cargo workspace tests.

## Global Constraints

- Diagnostic case list must include BB90 at `p = 0.006`, `num_cycles = 10`, `num_trials = 1`, `seed = 12345`.
- Diagnostic case list must include BB144 at `p = 0.006`, `num_cycles = 12`, `num_trials = 1`, `seed = 12345`.
- Diagnostic trial budget is exactly `1` unless both the case catalog and verifier assertions are updated.
- Python path uses upstream `ldpc.BpOsdDecoder` with `bp_method = "ms"`, `max_iter = 10000`, `osd_method = "osd_cs"`, and `osd_order = 7`.
- Missing Python dependencies produce skipped rows and verifier failure unless an explicit allow-missing mode is used.
- Full 50,000-trial campaign execution, publication plots, and decoder semantic changes are out of scope.
- Do not broaden the low-p `smoke` tier.

---

## File Structure

- Modify `benchmarks/bb_circuit_bposd_compare/cases.py` to add diagnostic constants, `DIAGNOSTIC_CASES`, and `validate_diagnostic_cases()`.
- Modify `benchmarks/bb_circuit_bposd_compare/tests/test_cases.py` to cover the diagnostic catalog and negative controls.
- Create `benchmarks/bb_circuit_bposd_compare/verify_diagnostic.py` for CSV verification.
- Create `benchmarks/bb_circuit_bposd_compare/tests/test_verify_diagnostic.py` for verifier positive and negative controls.
- Modify `benchmarks/bb_circuit_bposd_compare/run_compare.py` to add the diagnostic CLI tier and aggregate Rust counters in standard Rust rows.
- Modify `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py` to cover diagnostic runner behavior.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md` to document diagnostic commands, cases, and skipped-Python semantics.

---

### Task 1: Diagnostic Case Catalog

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/cases.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_cases.py`

**Interfaces:**
- Produces: `DIAGNOSTIC_TRIALS: int`, `DIAGNOSTIC_CASES: tuple[CompareCase, ...]`, `validate_diagnostic_cases(cases: Sequence[CompareCase] = DIAGNOSTIC_CASES) -> list[str]`
- Consumes: existing `CompareCase`, `format_case_id()`, `_target_key()`, `_target_label()`

- [ ] **Step 1: Write failing diagnostic catalog tests**

Add these imports to `test_cases.py`:

```python
from benchmarks.bb_circuit_bposd_compare.cases import (
    DIAGNOSTIC_CASES,
    DIAGNOSTIC_TRIALS,
    SMALL_LDPC_CASES,
    format_case_id,
    validate_diagnostic_cases,
    validate_small_ldpc_catalog,
)
```

Add these tests:

```python
def _diagnostic_case(code_id: str):
    return next(case for case in DIAGNOSTIC_CASES if case.code_id == code_id)


def test_diagnostic_catalog_has_exact_high_p_points() -> None:
    assert DIAGNOSTIC_TRIALS == 1
    assert validate_diagnostic_cases(DIAGNOSTIC_CASES) == []
    assert len(DIAGNOSTIC_CASES) == 2

    bb90 = _diagnostic_case("bb90")
    assert bb90.p == 0.006
    assert bb90.num_cycles == 10
    assert bb90.num_trials == 1
    assert bb90.seed == 12345

    bb144 = _diagnostic_case("bb144")
    assert bb144.p == 0.006
    assert bb144.num_cycles == 12
    assert bb144.num_trials == 1
    assert bb144.seed == 12345


def test_diagnostic_catalog_case_ids_and_decoder_settings_are_pinned() -> None:
    for case in DIAGNOSTIC_CASES:
        assert case.case_id == format_case_id(case)
        assert case.bp_method == "ms"
        assert case.max_iter == 10000
        assert case.osd_method == "osd_cs"
        assert case.osd_order == 7


def test_diagnostic_catalog_negative_control_rejects_missing_bb144() -> None:
    copied = tuple(case for case in DIAGNOSTIC_CASES if case.code_id != "bb144")
    errors = "\n".join(validate_diagnostic_cases(copied))
    assert "diagnostic catalog must contain exactly 2 cases" in errors
    assert "missing diagnostic target: bb144 p=0.006 cycles=12" in errors


def test_diagnostic_catalog_negative_control_rejects_wrong_bb144_config() -> None:
    copied = list(DIAGNOSTIC_CASES)
    index = next(i for i, case in enumerate(copied) if case.code_id == "bb144")
    copied[index] = replace(copied[index], p=0.005, num_cycles=10)

    errors = "\n".join(validate_diagnostic_cases(tuple(copied)))
    assert "missing diagnostic target: bb144 p=0.006 cycles=12" in errors
    assert "unexpected diagnostic target: bb144 p=0.005 cycles=10" in errors
```

- [ ] **Step 2: Run catalog tests and confirm RED**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_cases.py -k diagnostic -q
```

Expected: fails with `ImportError` or `NameError` for missing diagnostic symbols.

- [ ] **Step 3: Implement diagnostic catalog**

In `cases.py`, add diagnostic constants near the existing small-LDPC constants:

```python
DIAGNOSTIC_TRIALS = 1
DIAGNOSTIC_SEED = 12345
DIAGNOSTIC_BP_METHOD = "ms"
DIAGNOSTIC_MAX_ITER = 10000
DIAGNOSTIC_OSD_METHOD = "osd_cs"
DIAGNOSTIC_OSD_ORDER = 7
DIAGNOSTIC_TARGETS = {
    "bb90": (10, 0.006),
    "bb144": (12, 0.006),
}
```

Add helper and tuple after `SMOKE_CASES`:

```python
def _diagnostic_case(code_id: str, cycles: int, p: float) -> CompareCase:
    case = CompareCase(
        case_id="",
        code_id=code_id,
        p=p,
        num_cycles=cycles,
        num_trials=DIAGNOSTIC_TRIALS,
        seed=DIAGNOSTIC_SEED,
        bp_method=DIAGNOSTIC_BP_METHOD,
        max_iter=DIAGNOSTIC_MAX_ITER,
        osd_method=DIAGNOSTIC_OSD_METHOD,
        osd_order=DIAGNOSTIC_OSD_ORDER,
    )
    return CompareCase(
        case_id=format_case_id(case),
        code_id=code_id,
        p=p,
        num_cycles=cycles,
        num_trials=DIAGNOSTIC_TRIALS,
        seed=DIAGNOSTIC_SEED,
        bp_method=DIAGNOSTIC_BP_METHOD,
        max_iter=DIAGNOSTIC_MAX_ITER,
        osd_method=DIAGNOSTIC_OSD_METHOD,
        osd_order=DIAGNOSTIC_OSD_ORDER,
    )


DIAGNOSTIC_CASES = tuple(
    _diagnostic_case(code_id, cycles, p)
    for code_id, (cycles, p) in DIAGNOSTIC_TARGETS.items()
)
```

Add validator after `validate_small_ldpc_catalog()`:

```python
def validate_diagnostic_cases(
    cases: Sequence[CompareCase] = DIAGNOSTIC_CASES,
) -> list[str]:
    errors: list[str] = []
    expected_keys = {
        (code_id, _exact_decimal(p), cycles)
        for code_id, (cycles, p) in DIAGNOSTIC_TARGETS.items()
    }
    actual_keys = {_target_key(case) for case in cases}

    if len(cases) != 2:
        errors.append(f"diagnostic catalog must contain exactly 2 cases, got {len(cases)}")
    for key in sorted(expected_keys - actual_keys):
        errors.append(f"missing diagnostic target: {_target_label(key)}")
    for key in sorted(actual_keys - expected_keys):
        errors.append(f"unexpected diagnostic target: {_target_label(key)}")

    seen_case_ids: set[str] = set()
    for case in cases:
        if case.case_id in seen_case_ids:
            errors.append(f"duplicate diagnostic case_id: {case.case_id}")
        seen_case_ids.add(case.case_id)

        expected_case_id = format_case_id(case)
        if case.case_id != expected_case_id:
            errors.append(
                "diagnostic case_id mismatch for "
                f"{_target_label(_target_key(case))}: expected {expected_case_id}, "
                f"got {case.case_id}"
            )
        if case.num_trials != DIAGNOSTIC_TRIALS:
            errors.append(
                "diagnostic trial budget mismatch for "
                f"{_target_label(_target_key(case))}: expected {DIAGNOSTIC_TRIALS}, "
                f"got {case.num_trials}"
            )
        if case.seed != DIAGNOSTIC_SEED:
            errors.append(
                "diagnostic seed mismatch for "
                f"{_target_label(_target_key(case))}: expected {DIAGNOSTIC_SEED}, "
                f"got {case.seed}"
            )
        if case.bp_method != DIAGNOSTIC_BP_METHOD:
            errors.append(
                "diagnostic BP method mismatch for "
                f"{_target_label(_target_key(case))}: expected {DIAGNOSTIC_BP_METHOD}, "
                f"got {case.bp_method}"
            )
        if case.max_iter != DIAGNOSTIC_MAX_ITER:
            errors.append(
                "diagnostic max_iter mismatch for "
                f"{_target_label(_target_key(case))}: expected {DIAGNOSTIC_MAX_ITER}, "
                f"got {case.max_iter}"
            )
        if case.osd_method != DIAGNOSTIC_OSD_METHOD:
            errors.append(
                "diagnostic OSD method mismatch for "
                f"{_target_label(_target_key(case))}: expected {DIAGNOSTIC_OSD_METHOD}, "
                f"got {case.osd_method}"
            )
        if case.osd_order != DIAGNOSTIC_OSD_ORDER:
            errors.append(
                "diagnostic OSD order mismatch for "
                f"{_target_label(_target_key(case))}: expected {DIAGNOSTIC_OSD_ORDER}, "
                f"got {case.osd_order}"
            )

    return errors
```

- [ ] **Step 4: Run catalog tests and confirm GREEN**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_cases.py -k diagnostic -q
```

Expected: diagnostic catalog tests pass.

- [ ] **Step 5: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/cases.py benchmarks/bb_circuit_bposd_compare/tests/test_cases.py
git commit -m "Add BB diagnostic case catalog"
```

---

### Task 2: Diagnostic CSV Verifier

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/verify_diagnostic.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_verify_diagnostic.py`

**Interfaces:**
- Consumes: `CSV_HEADER`, `DIAGNOSTIC_CASES`
- Produces: `verify_rows(rows: list[dict[str, str]], allow_missing_python: bool = False) -> list[str]`

- [ ] **Step 1: Write failing verifier tests**

Create `test_verify_diagnostic.py` with rows for all CSV columns, a valid BB90/BB144 pair, and these assertions:

```python
import unittest

from benchmarks.bb_circuit_bposd_compare.cases import DIAGNOSTIC_CASES
from benchmarks.bb_circuit_bposd_compare.verify_diagnostic import verify_rows


def _case(code_id: str):
    return next(case for case in DIAGNOSTIC_CASES if case.code_id == code_id)


def make_row(case, decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": case.case_id,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": case.code_id,
        "p": str(case.p),
        "num_cycles": str(case.num_cycles),
        "num_trials": str(case.num_trials),
        "seed": str(case.seed),
        "bp_method": case.bp_method,
        "max_iter": str(case.max_iter),
        "osd_method": case.osd_method,
        "osd_order": str(case.osd_order),
        "basis": "",
        "syndrome_weight": "",
        "syndrome_support": "",
        "logical_prediction": "",
        "expected_logical": "",
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "2" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "20000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "16" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "1" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


def valid_rows() -> list[dict[str, str]]:
    return [
        make_row(_case("bb90"), "rbposd"),
        make_row(_case("bb90"), "ldpc_bposd"),
        make_row(_case("bb144"), "rbposd"),
        make_row(_case("bb144"), "ldpc_bposd"),
    ]


class VerifyDiagnosticTest(unittest.TestCase):
    def test_verify_rows_accepts_paired_diagnostic_cases(self) -> None:
        self.assertEqual(verify_rows(valid_rows()), [])

    def test_verify_rows_rejects_mismatched_case_id_pair(self) -> None:
        rows = valid_rows()
        rows[1]["case_id"] = "wrong-case-id"
        errors = "\n".join(verify_rows(rows))
        self.assertIn("expected exactly one Python ldpc_bposd diagnostic row", errors)

    def test_verify_rows_rejects_mismatched_pair_config(self) -> None:
        rows = valid_rows()
        rows[1]["num_cycles"] = "11"
        self.assertIn("Rust/Python diagnostic rows differ on num_cycles", "\n".join(verify_rows(rows)))

    def test_verify_rows_rejects_missing_bb144(self) -> None:
        rows = [row for row in valid_rows() if row["code_id"] != "bb144"]
        self.assertIn("required diagnostic case is missing: bb144", "\n".join(verify_rows(rows)))

    def test_verify_rows_rejects_wrong_bb144_point(self) -> None:
        rows = valid_rows()
        for row in rows:
            if row["code_id"] == "bb144":
                row["p"] = "0.005"
        errors = "\n".join(verify_rows(rows))
        self.assertIn("diagnostic row has mismatched p for bb144", errors)

    def test_verify_rows_rejects_missing_rust_counters(self) -> None:
        rows = valid_rows()
        rows[0]["gf2_solve_count"] = ""
        self.assertIn(
            "Rust rbposd diagnostic row is missing OSD/GF(2) counter fields",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_skipped_python_without_allow_missing(self) -> None:
        rows = valid_rows()
        rows[1].update(
            status="skipped",
            setup_seconds="",
            decode_seconds="",
            run_seconds="",
            logical_error_rate="",
            error="python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
        )
        self.assertIn("Python ldpc_bposd diagnostic row is skipped", "\n".join(verify_rows(rows)))

    def test_verify_rows_allows_skipped_python_with_allow_missing(self) -> None:
        rows = valid_rows()
        for row in rows:
            if row["decoder_impl"] == "ldpc_bposd":
                row.update(
                    status="skipped",
                    setup_seconds="",
                    decode_seconds="",
                    run_seconds="",
                    logical_error_rate="",
                    error="python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
                )
        self.assertEqual(verify_rows(rows, allow_missing_python=True), [])
```

- [ ] **Step 2: Run verifier tests and confirm RED**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_verify_diagnostic.py -q
```

Expected: fails with `ModuleNotFoundError` for `verify_diagnostic`.

- [ ] **Step 3: Implement verifier**

Implement `verify_diagnostic.py` by following `verify_replay.py` style. Required constants:

```python
REQUIRED_OK_FIELDS = (
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
)
PAIR_FIELDS = (
    "case_id",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
)
RUST_COUNTER_FIELDS = (
    "bp_seconds",
    "osd_seconds",
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)
RUST_INTEGER_COUNTER_FIELDS = (
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)
```

`verify_rows()` must:

```python
def verify_rows(
    rows: list[dict[str, str]],
    allow_missing_python: bool = False,
) -> list[str]:
    errors: list[str] = []
    if not rows:
        return ["CSV has no data rows"]

    missing_columns = [
        column for column in CSV_HEADER if not all(column in row for row in rows)
    ]
    if missing_columns:
        errors.append("row is missing required CSV column(s): " + ", ".join(missing_columns))

    for case in DIAGNOSTIC_CASES:
        expected = {
            "code_id": case.code_id,
            "p": str(case.p),
            "num_cycles": str(case.num_cycles),
            "num_trials": str(case.num_trials),
            "seed": str(case.seed),
            "bp_method": case.bp_method,
            "max_iter": str(case.max_iter),
            "osd_method": case.osd_method,
            "osd_order": str(case.osd_order),
        }
        case_rows = [row for row in rows if row.get("case_id") == case.case_id]
        if not case_rows:
            errors.append(f"required diagnostic case is missing: {case.code_id}")
            continue
        for row in case_rows:
            for field, expected_value in expected.items():
                if row.get(field) != expected_value:
                    errors.append(
                        f"diagnostic row has mismatched {field} for {case.code_id}: "
                        f"expected {expected_value}, got {row.get(field, '')}"
                    )
        rust_rows = [row for row in case_rows if row.get("decoder_impl") == "rbposd"]
        python_rows = [row for row in case_rows if row.get("decoder_impl") == "ldpc_bposd"]
        if len(rust_rows) != 1:
            errors.append(f"expected exactly one Rust rbposd diagnostic row for {case.case_id}")
        if len(python_rows) != 1:
            errors.append(f"expected exactly one Python ldpc_bposd diagnostic row for {case.case_id}")
        if len(rust_rows) == 1:
            _verify_rust_counters(rust_rows[0], errors)
        if len(rust_rows) == 1 and len(python_rows) == 1:
            _verify_pair(rust_rows[0], python_rows[0], errors)
            _verify_completed_or_skipped_python(python_rows[0], allow_missing_python, errors)
            _verify_ok_row(rust_rows[0], errors)
            if python_rows[0].get("status") == "ok":
                _verify_ok_row(python_rows[0], errors)

    return errors
```

Add helper functions mirroring `verify_replay.py`: `_verify_pair()`, `_verify_ok_row()`, `_verify_completed_or_skipped_python()`, `_verify_rust_counters()`, `_require_nonnegative_number()`, `_require_integer()`, `_as_int()`, `_load_rows()`, and `main()` with `--allow-missing-python`.

- [ ] **Step 4: Run verifier tests and confirm GREEN**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_verify_diagnostic.py -q
```

Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/verify_diagnostic.py benchmarks/bb_circuit_bposd_compare/tests/test_verify_diagnostic.py
git commit -m "Add BB diagnostic CSV verifier"
```

---

### Task 3: Diagnostic Runner and Documentation

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: `DIAGNOSTIC_CASES`, `validate_diagnostic_cases()`
- Produces: `run_diagnostic_suite(output_dir: Path, allow_missing_python: bool = False, rust_binary: Path | None = None, rust_exporter: Callable[..., dict[str, Any]] | None = None) -> int`
- Extends CLI `--tier` choices with `"diagnostic"`

- [ ] **Step 1: Write failing runner tests**

In `test_run_compare.py`, import `DIAGNOSTIC_CASES`, `run_diagnostic_suite`, and `verify_diagnostic_rows`:

```python
from benchmarks.bb_circuit_bposd_compare.cases import (
    DIAGNOSTIC_CASES,
    HARD_REPLAY_CASES,
    SMOKE_CASES,
)
from benchmarks.bb_circuit_bposd_compare.run_compare import (
    _python_row,
    main,
    run_diagnostic_suite,
    run_hard_replay_suite,
    run_suite,
)
from benchmarks.bb_circuit_bposd_compare.verify_diagnostic import (
    verify_rows as verify_diagnostic_rows,
)
```

Add a fake diagnostic export that includes aggregate Rust counters:

```python
def fake_diagnostic_export(case):
    export = fake_export(case)
    export["rust_result"]["profile"].update(
        {
            "bp_seconds": 0.12,
            "osd_seconds": 0.10,
            "decode_call_count": 2,
            "bp_iteration_count": 20000,
            "osd_use_count": 1,
            "osd_candidate_count": 16,
            "gf2_solve_count": 1,
            "gf2_full_elimination_count": 1,
        }
    )
    return export
```

Add a one-bit fake decoder for the normal full-trial Python path:

```python
class FakeDiagnosticDecoder:
    def __init__(self, matrix, **kwargs):
        self.kwargs = kwargs

    def decode(self, syndrome):
        return FakeHardVector([False])
```

Add tests:

```python
def test_diagnostic_suite_writes_paired_high_p_rows(self) -> None:
    fake_ldpc = ModuleType("ldpc")
    fake_ldpc.BpOsdDecoder = FakeDiagnosticDecoder

    with tempfile.TemporaryDirectory() as tmpdir:
        with mock.patch.dict("sys.modules", {"numpy": FakeHardNumpy(), "ldpc": fake_ldpc}):
            status = run_diagnostic_suite(
                Path(tmpdir),
                rust_exporter=fake_diagnostic_export,
            )
        with (Path(tmpdir) / "results.csv").open(newline="") as handle:
            rows = list(csv.DictReader(handle))

    self.assertEqual(status, 0)
    self.assertEqual(len(rows), 4)
    self.assertEqual([case.case_id for case in DIAGNOSTIC_CASES], [rows[0]["case_id"], rows[2]["case_id"]])
    self.assertEqual(verify_diagnostic_rows(rows), [])
    rust_rows = [row for row in rows if row["decoder_impl"] == "rbposd"]
    self.assertTrue(all(row["gf2_solve_count"] == "1" for row in rust_rows))


def test_diagnostic_suite_records_skipped_python_dependency_row(self) -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        with mock.patch(
            "benchmarks.bb_circuit_bposd_compare.run_compare._python_row",
            side_effect=ModuleNotFoundError("No module named 'ldpc'"),
        ):
            status = run_diagnostic_suite(
                Path(tmpdir),
                rust_exporter=fake_diagnostic_export,
            )
        with (Path(tmpdir) / "results.csv").open(newline="") as handle:
            rows = list(csv.DictReader(handle))

    self.assertNotEqual(status, 0)
    python_rows = [row for row in rows if row["decoder_impl"] == "ldpc_bposd"]
    self.assertEqual(len(python_rows), len(DIAGNOSTIC_CASES))
    self.assertTrue(all(row["status"] == "skipped" for row in python_rows))
    self.assertIn("Python ldpc_bposd diagnostic row is skipped", "\n".join(verify_diagnostic_rows(rows)))


def test_main_accepts_diagnostic_tier(self) -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        with mock.patch(
            "benchmarks.bb_circuit_bposd_compare.run_compare.run_diagnostic_suite",
            return_value=0,
        ) as run_diagnostic:
            status = main(["--tier", "diagnostic", "--output-dir", tmpdir])

    self.assertEqual(status, 0)
    run_diagnostic.assert_called_once()
```

- [ ] **Step 2: Run runner tests and confirm RED**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py -k diagnostic -q
```

Expected: fails with missing `run_diagnostic_suite` or missing `diagnostic` CLI tier.

- [ ] **Step 3: Implement runner**

Update imports in `run_compare.py`:

```python
from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    DIAGNOSTIC_CASES,
    HARD_REPLAY_CASES,
    SMALL_LDPC_CASES,
    CompareCase,
    SMOKE_CASES,
    small_ldpc_manifest_rows,
    validate_diagnostic_cases,
    validate_small_ldpc_catalog,
)
```

Add constants:

```python
RUST_PROFILE_COUNTER_FIELDS = (
    "bp_seconds",
    "osd_seconds",
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)
```

In `_rust_row()`, after timing/logical fields are set, copy any aggregate profile counters that exist:

```python
    for field in RUST_PROFILE_COUNTER_FIELDS:
        if field in profile:
            row[field] = _format_value(profile[field])
```

Add:

```python
def run_diagnostic_suite(
    output_dir: Path,
    allow_missing_python: bool = False,
    rust_binary: Path | None = None,
    rust_exporter: Callable[..., dict[str, Any]] | None = None,
) -> int:
    errors = validate_diagnostic_cases(DIAGNOSTIC_CASES)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    return run_suite(
        output_dir=output_dir,
        allow_missing_python=allow_missing_python,
        cases=DIAGNOSTIC_CASES,
        rust_binary=rust_binary,
        rust_exporter=rust_exporter,
    )
```

Extend the parser:

```python
choices=("smoke", "small_ldpc_catalog", "hard-replay", "diagnostic")
```

Add dispatch before the smoke fallback:

```python
    if args.tier == "diagnostic":
        status = run_diagnostic_suite(
            output_dir=args.output_dir,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status != 0 and not args.allow_missing_python:
            for message in _missing_python_dependency_messages(
                _read_rows(args.output_dir / "results.csv")
            ):
                print(message, file=sys.stderr)
        return status
```

Guard the `_read_rows()` call so catalog validation failure before CSV creation does not raise `FileNotFoundError`:

```python
def _print_missing_python_dependency_messages(output_dir: Path) -> None:
    results_path = output_dir / "results.csv"
    if not results_path.exists():
        return
    for message in _missing_python_dependency_messages(_read_rows(results_path)):
        print(message, file=sys.stderr)
```

Use that helper in `hard-replay`, `diagnostic`, and `smoke` nonzero paths.

- [ ] **Step 4: Update README**

Add a `## Diagnostic Tier` section before `## BB90 Hard-Syndrome Replay`:

```markdown
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
```

- [ ] **Step 5: Run runner tests and confirm GREEN**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py -k diagnostic -q
```

Expected: diagnostic runner tests pass.

- [ ] **Step 6: Run focused package tests**

Run:

```bash
.venv-surface-decoder/bin/python -m pytest benchmarks/bb_circuit_bposd_compare/tests -q
```

Expected: package tests pass.

- [ ] **Step 7: Commit**

```bash
git add benchmarks/bb_circuit_bposd_compare/run_compare.py benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py benchmarks/bb_circuit_bposd_compare/README.md
git commit -m "Add BB diagnostic compare runner"
```

---

### Task 4: Required Verification and PR Readiness

**Files:**
- No code files unless verification reveals a defect.

**Interfaces:**
- Consumes: complete implementation from Tasks 1-3.
- Produces: final verification evidence and PR.

- [ ] **Step 1: Run Rust unit/integration tests required by Agent Desk**

Run:

```bash
cargo test
```

Expected: exit 0.

- [ ] **Step 2: Build release rsinter**

Run:

```bash
cargo build --release -p rsinter
```

Expected: exit 0 and `target/release/rsinter` exists.

- [ ] **Step 3: Run diagnostic compare command**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier diagnostic --output-dir /tmp/rstim-bb-diagnostic --rust-binary target/release/rsinter
```

Expected: exit 0 when Python `ldpc` dependencies are available. If Python dependencies are missing, rerun with `--allow-missing-python` to inspect artifacts, then record the default nonzero outcome as an environment limitation.

- [ ] **Step 4: Run diagnostic verifier**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.verify_diagnostic /tmp/rstim-bb-diagnostic/results.csv
```

Expected: exit 0 when Python `ldpc` dependencies are available. If Step 3 only produced skipped Python rows because dependencies are missing, this verifier must fail without `--allow-missing-python` and pass only with the explicit flag.

- [ ] **Step 5: Run final diff review**

Run:

```bash
git diff --stat origin/master...HEAD
git diff --check
```

Expected: no whitespace errors; diff is scoped to docs and compare harness.

- [ ] **Step 6: Commit any verification fixes**

If a verification defect is fixed, run the focused command that exposed it, then:

```bash
git add <fixed-files>
git commit -m "Fix BB diagnostic compare verification"
```

- [ ] **Step 7: Push and create PR**

Use `superpowers:finishing-a-development-branch`, choose "Push and create a Pull Request", push the worker branch, and open a PR against `master` with summary, tests, and `Closes #285`.
