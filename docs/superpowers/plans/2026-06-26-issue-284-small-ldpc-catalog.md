# Issue 284 Small-LDPC Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a validated dry-run catalog for all 31 #209 small-LDPC BB compare target points without running the 50,000-trial campaign by default.

**Architecture:** Keep runnable smoke cases separate from the full small-LDPC manifest. Extend `cases.py` with catalog metadata, exact validation helpers, and manifest rows; extend `run_compare.py` with a manifest-only tier; cover the catalog with pytest negative controls and document the dry-run path.

**Tech Stack:** Python 3 dataclasses, pytest/unittest-compatible assertions, existing `benchmarks.bb_circuit_bposd_compare` package, Markdown README.

## Global Constraints

- `SMOKE_CASES` must stay small and runnable; do not expand the smoke tier to 31 cases.
- The small-LDPC catalog must contain exactly 31 cases: BB72 7, BB90 7, BB108 7, BB144 6, BB288 4.
- The #209 p sweeps are exact: BB72 `0.0002, 0.0005, 0.001, 0.002, 0.003, 0.004, 0.005`; BB90 `0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006`; BB108 `0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006`; BB144 `0.001, 0.002, 0.003, 0.004, 0.005, 0.006`; BB288 `0.0035, 0.004, 0.005, 0.006`.
- The #209 cycle counts are exact: BB72 6, BB90 10, BB108 10, BB144 12, BB288 18.
- Small-LDPC catalog cases use `num_trials = 50000`, `seed = 12345`, `bp_method = "ms"`, `max_iter = 10000`, `osd_method = "osd_cs"` as documented equivalent for `ldpc_cs`, `osd_order = 7`, and `scaling = 0`.
- Every small-LDPC `case_id` must include code id, p label, cycle count, trial budget, and seed.
- `bb108` and `bb288` must remain in the catalog with an explicit unsupported Rust constructor status; do not drop them.
- Do not implement BB108 or BB288 Rust constructors in this issue.
- Do not run the full 50,000-trial tier.
- Required verification command: `python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_cases.py -k small_ldpc_catalog`.
- Also run `cargo test` before PR creation.

---

### Task 1: Small-LDPC Catalog, Validation, Dry-Run, And Docs

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/cases.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/__init__.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_cases.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: existing `CompareCase`, `SMOKE_CASES`, `CSV_HEADER`, `run_compare.main()`.
- Produces:
  - `CATALOG_HEADER: list[str]`
  - `SMALL_LDPC_CASES: tuple[CompareCase, ...]`
  - `format_case_id(case: CompareCase) -> str`
  - `small_ldpc_manifest_rows(cases: Sequence[CompareCase] = SMALL_LDPC_CASES) -> list[dict[str, str]]`
  - `validate_small_ldpc_catalog(cases: Sequence[CompareCase] = SMALL_LDPC_CASES) -> list[str]`
  - `run_compare.py --tier small_ldpc_catalog` writes `manifest.csv` only and never calls Rust/Python decoders.

- [ ] **Step 1: Write the failing test**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_cases.py` with pytest-style tests:

```python
from dataclasses import replace

from benchmarks.bb_circuit_bposd_compare.cases import (
    SMALL_LDPC_CASES,
    format_case_id,
    validate_small_ldpc_catalog,
)


EXPECTED_SWEEPS = {
    "bb72": (6, (0.0002, 0.0005, 0.001, 0.002, 0.003, 0.004, 0.005)),
    "bb90": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb108": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb144": (12, (0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb288": (18, (0.0035, 0.004, 0.005, 0.006)),
}


def _cases_for(code_id: str):
    return [case for case in SMALL_LDPC_CASES if case.code_id == code_id]


def test_small_ldpc_catalog_has_complete_issue_209_targets() -> None:
    assert len(SMALL_LDPC_CASES) == 31
    assert validate_small_ldpc_catalog(SMALL_LDPC_CASES) == []

    for code_id, (cycles, p_values) in EXPECTED_SWEEPS.items():
        cases = _cases_for(code_id)
        assert len(cases) == len(p_values)
        assert tuple(case.p for case in cases) == p_values
        assert {case.num_cycles for case in cases} == {cycles}


def test_small_ldpc_catalog_case_ids_include_identity_fields() -> None:
    for case in SMALL_LDPC_CASES:
        assert case.case_id == format_case_id(case)
        assert case.code_id in case.case_id
        assert f"c{case.num_cycles}" in case.case_id
        assert f"t{case.num_trials}" in case.case_id
        assert f"seed{case.seed}" in case.case_id


def test_small_ldpc_catalog_decoder_settings_are_pinned() -> None:
    for case in SMALL_LDPC_CASES:
        assert case.num_trials == 50000
        assert case.seed == 12345
        assert case.bp_method == "ms"
        assert case.max_iter == 10000
        assert case.osd_method in {"ldpc_cs", "osd_cs"}
        assert case.osd_order == 7
        assert case.scaling == 0


def test_small_ldpc_catalog_marks_unsupported_rust_constructors() -> None:
    unsupported = {case.code_id for case in SMALL_LDPC_CASES if case.catalog_status != "supported"}
    assert unsupported == {"bb108", "bb288"}
    for case in SMALL_LDPC_CASES:
        if case.code_id in unsupported:
            assert case.catalog_status == "unsupported_rust_constructor"
            assert "Rust constructor" in case.catalog_note


def test_small_ldpc_catalog_negative_control_names_missing_bb108() -> None:
    copied = tuple(case for case in SMALL_LDPC_CASES if case.code_id != "bb108")
    errors = "\n".join(validate_small_ldpc_catalog(copied))
    assert "missing target: bb108 p=0.0005 cycles=10" in errors


def test_small_ldpc_catalog_negative_control_names_wrong_bb288_p_value() -> None:
    copied = list(SMALL_LDPC_CASES)
    index = next(i for i, case in enumerate(copied) if case.code_id == "bb288")
    copied[index] = replace(copied[index], p=0.0036)

    errors = "\n".join(validate_small_ldpc_catalog(tuple(copied)))
    assert "missing target: bb288 p=0.0035 cycles=18" in errors
    assert "unexpected target: bb288 p=0.0036 cycles=18" in errors
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_cases.py -k small_ldpc_catalog
```

Expected: FAIL during import because `SMALL_LDPC_CASES`, `format_case_id`, and `validate_small_ldpc_catalog` do not exist yet.

- [ ] **Step 3: Implement catalog metadata and validation**

Modify `benchmarks/bb_circuit_bposd_compare/cases.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
from typing import Sequence

CSV_HEADER = [
    "case_id",
    "runner",
    "decoder_impl",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
    "error",
]

CATALOG_HEADER = [
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
    "scaling",
    "catalog_status",
    "catalog_note",
]

SMALL_LDPC_TRIALS = 50_000
SMALL_LDPC_SEED = 12345
SMALL_LDPC_BP_METHOD = "ms"
SMALL_LDPC_MAX_ITER = 10000
SMALL_LDPC_OSD_METHOD = "osd_cs"
SMALL_LDPC_OSD_ORDER = 7
SMALL_LDPC_SCALING = 0
SUPPORTED_RUST_CONSTRUCTORS = {"bb72", "bb90", "bb144"}
UNSUPPORTED_RUST_CONSTRUCTOR_STATUS = "unsupported_rust_constructor"
UNSUPPORTED_RUST_CONSTRUCTOR_NOTE = "Rust constructor support is not available in this branch."


@dataclass(frozen=True)
class CompareCase:
    case_id: str
    code_id: str
    p: float
    num_cycles: int
    num_trials: int
    seed: int = 12345
    bp_method: str = "ms"
    max_iter: int = 10000
    osd_method: str = "osd_cs"
    osd_order: int = 7
    scaling: int = 0
    catalog_status: str = "supported"
    catalog_note: str = ""


def _decimal(value: float) -> Decimal:
    return Decimal(str(value)).quantize(Decimal("0.0001"))


def _format_p_value(value: float) -> str:
    return format(_decimal(value).normalize(), "f")


def _format_p_label(value: float) -> str:
    return "p" + format(_decimal(value), "f").removeprefix("0.").replace(".", "")


def format_case_id(case: CompareCase) -> str:
    return (
        f"{case.code_id}-{_format_p_label(case.p)}-c{case.num_cycles}"
        f"-t{case.num_trials}-seed{case.seed}"
    )


def _small_ldpc_case(code_id: str, p: float, cycles: int) -> CompareCase:
    status = "supported"
    note = ""
    if code_id not in SUPPORTED_RUST_CONSTRUCTORS:
        status = UNSUPPORTED_RUST_CONSTRUCTOR_STATUS
        note = UNSUPPORTED_RUST_CONSTRUCTOR_NOTE
    case = CompareCase(
        case_id="",
        code_id=code_id,
        p=p,
        num_cycles=cycles,
        num_trials=SMALL_LDPC_TRIALS,
        seed=SMALL_LDPC_SEED,
        bp_method=SMALL_LDPC_BP_METHOD,
        max_iter=SMALL_LDPC_MAX_ITER,
        osd_method=SMALL_LDPC_OSD_METHOD,
        osd_order=SMALL_LDPC_OSD_ORDER,
        scaling=SMALL_LDPC_SCALING,
        catalog_status=status,
        catalog_note=note,
    )
    return CompareCase(
        case_id=format_case_id(case),
        code_id=code_id,
        p=p,
        num_cycles=cycles,
        num_trials=SMALL_LDPC_TRIALS,
        seed=SMALL_LDPC_SEED,
        bp_method=SMALL_LDPC_BP_METHOD,
        max_iter=SMALL_LDPC_MAX_ITER,
        osd_method=SMALL_LDPC_OSD_METHOD,
        osd_order=SMALL_LDPC_OSD_ORDER,
        scaling=SMALL_LDPC_SCALING,
        catalog_status=status,
        catalog_note=note,
    )
```

Then add exact target constants and validation:

```python
SMALL_LDPC_TARGETS = {
    "bb72": (6, (0.0002, 0.0005, 0.001, 0.002, 0.003, 0.004, 0.005)),
    "bb90": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb108": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb144": (12, (0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb288": (18, (0.0035, 0.004, 0.005, 0.006)),
}

SMALL_LDPC_CASES = tuple(
    _small_ldpc_case(code_id, p, cycles)
    for code_id, (cycles, p_values) in SMALL_LDPC_TARGETS.items()
    for p in p_values
)

SMOKE_CASES = (
    CompareCase("bb72-p0005-c1-t1-seed12345", "bb72", 0.0005, 1, 1),
    CompareCase("bb90-p0005-c1-t1-seed12345", "bb90", 0.0005, 1, 1),
)


def _target_key(case: CompareCase) -> tuple[str, str, int]:
    return (case.code_id, _format_p_value(case.p), case.num_cycles)


def _target_label(key: tuple[str, str, int]) -> str:
    code_id, p_value, cycles = key
    return f"{code_id} p={p_value} cycles={cycles}"


def small_ldpc_manifest_rows(
    cases: Sequence[CompareCase] = SMALL_LDPC_CASES,
) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for case in cases:
        rows.append(
            {
                "case_id": case.case_id,
                "code_id": case.code_id,
                "p": _format_p_value(case.p),
                "num_cycles": str(case.num_cycles),
                "num_trials": str(case.num_trials),
                "seed": str(case.seed),
                "bp_method": case.bp_method,
                "max_iter": str(case.max_iter),
                "osd_method": case.osd_method,
                "osd_order": str(case.osd_order),
                "scaling": str(case.scaling),
                "catalog_status": case.catalog_status,
                "catalog_note": case.catalog_note,
            }
        )
    return rows


def validate_small_ldpc_catalog(
    cases: Sequence[CompareCase] = SMALL_LDPC_CASES,
) -> list[str]:
    errors: list[str] = []
    expected_keys = {
        (code_id, _format_p_value(p), cycles)
        for code_id, (cycles, p_values) in SMALL_LDPC_TARGETS.items()
        for p in p_values
    }
    actual_keys = {_target_key(case) for case in cases}

    if len(cases) != 31:
        errors.append(f"small-LDPC catalog must contain exactly 31 cases, got {len(cases)}")

    for key in sorted(expected_keys - actual_keys):
        errors.append(f"missing target: {_target_label(key)}")
    for key in sorted(actual_keys - expected_keys):
        errors.append(f"unexpected target: {_target_label(key)}")

    seen_case_ids: set[str] = set()
    for case in cases:
        if case.case_id in seen_case_ids:
            errors.append(f"duplicate case_id: {case.case_id}")
        seen_case_ids.add(case.case_id)

        expected_case_id = format_case_id(case)
        if case.case_id != expected_case_id:
            errors.append(f"case_id mismatch for {_target_label(_target_key(case))}: expected {expected_case_id}, got {case.case_id}")

        if case.num_trials != SMALL_LDPC_TRIALS:
            errors.append(f"trial budget mismatch for {_target_label(_target_key(case))}: expected {SMALL_LDPC_TRIALS}, got {case.num_trials}")
        if case.seed != SMALL_LDPC_SEED:
            errors.append(f"seed mismatch for {_target_label(_target_key(case))}: expected {SMALL_LDPC_SEED}, got {case.seed}")
        if case.bp_method != SMALL_LDPC_BP_METHOD:
            errors.append(f"BP method mismatch for {_target_label(_target_key(case))}: expected {SMALL_LDPC_BP_METHOD}, got {case.bp_method}")
        if case.max_iter != SMALL_LDPC_MAX_ITER:
            errors.append(f"max_iter mismatch for {_target_label(_target_key(case))}: expected {SMALL_LDPC_MAX_ITER}, got {case.max_iter}")
        if case.osd_method not in {"ldpc_cs", SMALL_LDPC_OSD_METHOD}:
            errors.append(f"OSD method mismatch for {_target_label(_target_key(case))}: expected ldpc_cs or {SMALL_LDPC_OSD_METHOD}, got {case.osd_method}")
        if case.osd_order != SMALL_LDPC_OSD_ORDER:
            errors.append(f"OSD order mismatch for {_target_label(_target_key(case))}: expected {SMALL_LDPC_OSD_ORDER}, got {case.osd_order}")
        if case.scaling != SMALL_LDPC_SCALING:
            errors.append(f"scaling mismatch for {_target_label(_target_key(case))}: expected {SMALL_LDPC_SCALING}, got {case.scaling}")

        if case.code_id in SUPPORTED_RUST_CONSTRUCTORS:
            if case.catalog_status != "supported":
                errors.append(f"supported target has wrong catalog status for {_target_label(_target_key(case))}: {case.catalog_status}")
        elif case.catalog_status != UNSUPPORTED_RUST_CONSTRUCTOR_STATUS:
            errors.append(f"unsupported target has wrong catalog status for {_target_label(_target_key(case))}: {case.catalog_status}")

    return errors
```

- [ ] **Step 4: Implement dry-run manifest tier**

Modify `benchmarks/bb_circuit_bposd_compare/run_compare.py` imports and helpers:

```python
from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    SMALL_LDPC_CASES,
    CompareCase,
    SMOKE_CASES,
    small_ldpc_manifest_rows,
    validate_small_ldpc_catalog,
)
```

Add:

```python
def _write_manifest(rows: list[dict[str, str]], out_path: Path) -> None:
    with out_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CATALOG_HEADER)
        writer.writeheader()
        for row in rows:
            writer.writerow({column: row.get(column, "") for column in CATALOG_HEADER})


def run_small_ldpc_catalog_dry_run(
    output_dir: Path,
    cases: Sequence[CompareCase] = SMALL_LDPC_CASES,
) -> int:
    output_dir.mkdir(parents=True, exist_ok=True)
    errors = validate_small_ldpc_catalog(cases)
    _write_manifest(small_ldpc_manifest_rows(cases), output_dir / "manifest.csv")
    return 1 if errors else 0
```

Update `main()`:

```python
    parser.add_argument("--tier", choices=("smoke", "small_ldpc_catalog"), required=True)
```

and route:

```python
    if args.tier == "small_ldpc_catalog":
        return run_small_ldpc_catalog_dry_run(args.output_dir)
```

before the existing smoke `run_suite(...)` call.

- [ ] **Step 5: Export catalog helpers from package**

Modify `benchmarks/bb_circuit_bposd_compare/__init__.py` to import and export:

```python
from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    SMALL_LDPC_CASES,
    SMOKE_CASES,
)
```

Add `"CATALOG_HEADER"` and `"SMALL_LDPC_CASES"` to `__all__`.

- [ ] **Step 6: Update README**

Add a section to `benchmarks/bb_circuit_bposd_compare/README.md`:

````markdown
## Small-LDPC Catalog Dry Run

The complete #209 `small_ldpc.png` target catalog is checked in as
`SMALL_LDPC_CASES`. It contains 31 manifest cases and does not run the
50,000-trial campaign by default.

Write the dry-run manifest with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier small_ldpc_catalog --output-dir /tmp/rstim-small-ldpc-catalog
```

The command validates the catalog and writes `/tmp/rstim-small-ldpc-catalog/manifest.csv`.

| code_id | cycles | p points | catalog status |
| --- | ---: | ---: | --- |
| `bb72` | 6 | 7 | supported |
| `bb90` | 10 | 7 | supported |
| `bb108` | 10 | 7 | unsupported Rust constructor |
| `bb144` | 12 | 6 | supported |
| `bb288` | 18 | 4 | unsupported Rust constructor |
````

- [ ] **Step 7: Run focused test to verify it passes**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_cases.py -k small_ldpc_catalog
```

Expected: PASS, 6 tests selected.

- [ ] **Step 8: Run dry-run command**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare --tier small_ldpc_catalog --output-dir /tmp/rstim-small-ldpc-catalog
```

Expected: exit 0 and `/tmp/rstim-small-ldpc-catalog/manifest.csv` contains 32 CSV lines including the header.

- [ ] **Step 9: Run Rust verification**

Run:

```bash
cargo test
```

Expected: PASS. If pre-existing warnings appear, record them without broadening the change.

- [ ] **Step 10: Commit**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/cases.py benchmarks/bb_circuit_bposd_compare/run_compare.py benchmarks/bb_circuit_bposd_compare/__init__.py benchmarks/bb_circuit_bposd_compare/tests/test_cases.py benchmarks/bb_circuit_bposd_compare/README.md
git commit -m "test: add small ldpc compare catalog"
```
