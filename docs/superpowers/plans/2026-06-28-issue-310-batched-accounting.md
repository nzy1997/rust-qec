# Issue 310 BB Batched Accounting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pure CSV verifier that proves BB batched Rust/Python early-stop rows are comparable and that each completed row's logical error rate uses its own `shots_used`.

**Architecture:** Add one Python verifier module under `benchmarks/bb_circuit_bposd_compare` and one focused pytest module. The verifier groups `runner=batched_compare` rows by `(case_id, code_id, p, num_cycles)`, validates exact Rust/Python pairing or explicit Python dependency skips, checks shared batch metadata, checks error-budget semantics, and prints PASS/FAIL CLI output without running decoders.

**Tech Stack:** Python standard library (`argparse`, `csv`, `math`, `dataclasses`, `subprocess`, `pathlib`, `pytest`) plus existing repository CSV header constants from `benchmarks.bb_circuit_bposd_compare.cases`.

## Global Constraints

- Do not regenerate the checked-in full BB comparison CSV or PNG.
- Keep `verify_bravyi_ler.py` focused on trial-level normalization; paired early-stop semantics live in `verify_batched_accounting.py`.
- The verifier must inspect CSV rows only and must not run Rust, Python `ldpc`, matplotlib, or plot rendering.
- Group identity is exactly `(case_id, code_id, p, num_cycles)`.
- Exactly one `rbposd` row and one `ldpc_bposd` row are required per group unless the Python row is explicitly skipped with `status=skipped` and `stop_reason=python_dependency_missing`.
- Paired completed rows must have identical `shots_used`, `batch_size`, `batches_completed`, `stop_reason`, `seed`, `bp_method`, `max_iter`, `osd_method`, and `osd_order`.
- `stop_reason=errors_budget_reached` is valid only when at least one decoder has `logical_errors >= errors_budget`.
- Every completed row must satisfy `logical_error_rate == logical_errors / shots_used` within `1e-12`.
- Partial rows must clearly distinguish `wall_budget_exhausted` and `python_dependency_missing`.
- Pair mismatch failures must say the Rust/Python pair is no longer comparable.

---

### Task 1: Paired Accounting Verifier

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/verify_batched_accounting.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py`

**Interfaces:**
- Consumes: `benchmarks.bb_circuit_bposd_compare.cases.BATCHED_CSV_HEADER`
- Produces: `load_rows(csv_path: Path) -> list[dict[str, str]]`
- Produces: `verify_rows(rows: list[dict[str, str]]) -> list[VerifiedPair | VerificationError]`
- Produces: CLI `python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting <results.csv>`

- [ ] **Step 1: Write failing tests**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py` with tests in this shape:

```python
from __future__ import annotations

import csv
import subprocess
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import verify_batched_accounting
from benchmarks.bb_circuit_bposd_compare.cases import BATCHED_CSV_HEADER

FULL_RESULTS = (
    Path(__file__).resolve().parents[1] / "results" / "full" / "results.csv"
)


def make_row(decoder_impl: str = "rbposd", **overrides: str) -> dict[str, str]:
    logical_errors = "200" if decoder_impl == "rbposd" else "120"
    shots_used = "500"
    row = {column: "" for column in BATCHED_CSV_HEADER}
    row.update(
        {
            "case_id": "bb72-p0030-c6-t1000000-seed12345",
            "runner": "batched_compare",
            "decoder_impl": decoder_impl,
            "code_id": "bb72",
            "p": "0.003",
            "num_cycles": "6",
            "shots_budget": "1000000",
            "errors_budget": "200",
            "shots_used": shots_used,
            "seed": "12345",
            "bp_method": "ms",
            "max_iter": "10000",
            "osd_method": "osd_cs",
            "osd_order": "7",
            "batch_size": "500",
            "batches_completed": "1",
            "logical_errors": logical_errors,
            "logical_error_rate": str(int(logical_errors) / int(shots_used)),
            "status": "ok",
            "stop_reason": "errors_budget_reached",
            "error": "",
        }
    )
    row.update(overrides)
    if "logical_errors" in overrides and "logical_error_rate" not in overrides:
        row["logical_error_rate"] = str(
            int(row["logical_errors"]) / int(row["shots_used"])
        )
    if "shots_used" in overrides and "logical_error_rate" not in overrides:
        row["logical_error_rate"] = str(
            int(row["logical_errors"]) / int(row["shots_used"])
        )
    return row


def make_pair(
    rust_overrides: dict[str, str] | None = None,
    python_overrides: dict[str, str] | None = None,
) -> list[dict[str, str]]:
    return [
        make_row("rbposd", **(rust_overrides or {})),
        make_row("ldpc_bposd", **(python_overrides or {})),
    ]


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=BATCHED_CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)


def partition(
    result: list[
        verify_batched_accounting.VerifiedPair
        | verify_batched_accounting.VerificationError
    ],
) -> tuple[
    list[verify_batched_accounting.VerifiedPair],
    list[verify_batched_accounting.VerificationError],
]:
    verified = [
        item
        for item in result
        if isinstance(item, verify_batched_accounting.VerifiedPair)
    ]
    errors = [
        item
        for item in result
        if isinstance(item, verify_batched_accounting.VerificationError)
    ]
    return verified, errors
```

Add these behavior tests:

```python
def test_accepts_comparable_error_budget_pair() -> None:
    verified, errors = partition(verify_batched_accounting.verify_rows(make_pair()))
    assert errors == []
    assert len(verified) == 1
    assert verified[0].case_id == "bb72-p0030-c6-t1000000-seed12345"
    assert verified[0].stop_reason == "errors_budget_reached"
    assert verified[0].shots_used == 500
    assert verified[0].batches_completed == 1
    assert verified[0].rbposd_logical_errors == 200
    assert verified[0].ldpc_bposd_logical_errors == 120


def test_checked_in_full_results_have_comparable_error_budget_pairs() -> None:
    result = verify_batched_accounting.verify_rows(
        verify_batched_accounting.load_rows(FULL_RESULTS)
    )
    verified, errors = partition(result)
    assert errors == []
    assert {pair.code_id for pair in verified} == {"bb72", "bb144"}
    assert all(pair.stop_reason == "errors_budget_reached" for pair in verified)
    assert any(pair.code_id == "bb72" for pair in verified)
    assert any(pair.code_id == "bb144" for pair in verified)
    assert all(pair.shots_used > 0 for pair in verified)
    assert all(pair.batches_completed > 0 for pair in verified)


def test_rejects_mismatched_python_shots_used_as_uncomparable() -> None:
    rows = make_pair(python_overrides={"shots_used": "501"})
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "no longer comparable" in errors[0].message
    assert "shots_used" in errors[0].message


def test_rejects_mismatched_python_batches_completed_as_uncomparable() -> None:
    rows = make_pair(python_overrides={"batches_completed": "2"})
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "no longer comparable" in errors[0].message
    assert "batches_completed" in errors[0].message


def test_errors_budget_stop_requires_one_decoder_to_reach_budget() -> None:
    rows = make_pair(
        rust_overrides={"logical_errors": "199"},
        python_overrides={"logical_errors": "198"},
    )
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "errors_budget_reached" in errors[0].message
    assert "logical_errors >= errors_budget" in errors[0].message


def test_accepts_wall_budget_partial_pair() -> None:
    rows = make_pair(
        rust_overrides={
            "status": "partial",
            "stop_reason": "wall_budget_exhausted",
            "logical_errors": "2",
        },
        python_overrides={
            "status": "partial",
            "stop_reason": "wall_budget_exhausted",
            "logical_errors": "1",
        },
    )
    verified, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors == []
    assert verified[0].status == "partial"
    assert verified[0].stop_reason == "wall_budget_exhausted"


def test_rejects_partial_row_without_explicit_partial_reason() -> None:
    rows = make_pair(
        rust_overrides={"status": "partial", "stop_reason": "completed"},
        python_overrides={"status": "partial", "stop_reason": "completed"},
    )
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "partial" in errors[0].message
    assert "wall_budget_exhausted" in errors[0].message


def test_accepts_explicit_python_dependency_missing_skip() -> None:
    rows = [
        make_row(
            "rbposd",
            status="partial",
            stop_reason="python_dependency_missing",
            logical_errors="3",
        ),
        make_row(
            "ldpc_bposd",
            status="skipped",
            stop_reason="python_dependency_missing",
            logical_errors="0",
            logical_error_rate="0.0",
            error="python dependency unavailable for ldpc_bposd replay: ldpc",
        ),
    ]
    verified, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors == []
    assert verified[0].status == "partial"
    assert verified[0].stop_reason == "python_dependency_missing"
    assert verified[0].ldpc_bposd_logical_errors is None


def test_rejects_logical_error_rate_not_computed_from_shots_used() -> None:
    rows = make_pair(python_overrides={"logical_error_rate": "0.999"})
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "logical_error_rate" in errors[0].message
    assert "logical_errors / shots_used" in errors[0].message


def test_cli_prints_pass_lines_for_full_results() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_batched_accounting",
            str(FULL_RESULTS),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    assert "PASS BB batched paired accounting" in result.stdout
    assert "PASS bb72-p0030-c6-t1000000-seed12345" in result.stdout
    assert "PASS bb144-p0030-c12-t1000000-seed12345" in result.stdout
    assert "stop_reason=errors_budget_reached" in result.stdout


def test_cli_negative_control_exits_nonzero_for_unpaired_rows(tmp_path: Path) -> None:
    csv_path = tmp_path / "bb_batched_unpaired_bad.csv"
    write_csv(csv_path, make_pair(python_overrides={"shots_used": "501"}))
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_batched_accounting",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "no longer comparable" in result.stderr
```

- [ ] **Step 2: Verify RED**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py
```

Expected: fail with an import error or missing module error for
`verify_batched_accounting`.

- [ ] **Step 3: Implement verifier**

Create `benchmarks/bb_circuit_bposd_compare/verify_batched_accounting.py` with:

```python
from __future__ import annotations

import argparse
import csv
import math
import sys
from dataclasses import dataclass
from pathlib import Path

TOLERANCE = 1e-12
COMPLETED_STATUSES = {"ok", "partial"}
PARTIAL_STOP_REASONS = {"wall_budget_exhausted", "python_dependency_missing"}
PAIR_METADATA_FIELDS = (
    "shots_used",
    "batch_size",
    "batches_completed",
    "stop_reason",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
)
REQUIRED_COLUMNS = (
    "case_id",
    "runner",
    "decoder_impl",
    "code_id",
    "p",
    "num_cycles",
    "errors_budget",
    "shots_used",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
    "batch_size",
    "batches_completed",
    "logical_errors",
    "logical_error_rate",
    "status",
    "stop_reason",
    "error",
)


@dataclass(frozen=True)
class CompletedRow:
    row_index: int
    raw: dict[str, str]
    logical_errors: int
    logical_error_rate: float
    shots_used: int
    batch_size: int
    batches_completed: int
    errors_budget: int | None


@dataclass(frozen=True)
class VerifiedPair:
    case_id: str
    code_id: str
    p: str
    num_cycles: str
    status: str
    stop_reason: str
    shots_used: int
    batch_size: int
    batches_completed: int
    rbposd_logical_errors: int
    ldpc_bposd_logical_errors: int | None


@dataclass(frozen=True)
class VerificationError:
    message: str


def load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))
```

Implement `verify_rows()` as:

```python
def verify_rows(rows: list[dict[str, str]]) -> list[VerifiedPair | VerificationError]:
    if not rows:
        return [VerificationError("CSV has no data rows")]
    missing_columns = [
        column for column in REQUIRED_COLUMNS if not all(column in row for row in rows)
    ]
    if missing_columns:
        return [
            VerificationError(
                "row is missing required CSV column(s): " + ", ".join(missing_columns)
            )
        ]

    groups: dict[tuple[str, str, str, str], list[tuple[int, dict[str, str]]]] = {}
    errors: list[VerificationError] = []
    for row_index, row in enumerate(rows, start=2):
        if row.get("runner") != "batched_compare":
            continue
        decoder_impl = row.get("decoder_impl", "")
        if decoder_impl not in {"rbposd", "ldpc_bposd"}:
            errors.append(
                VerificationError(
                    f"row {row_index} {row.get('case_id', '<missing case_id>')}: "
                    f"unsupported decoder_impl {decoder_impl!r}"
                )
            )
            continue
        key = (row["case_id"], row["code_id"], row["p"], row["num_cycles"])
        groups.setdefault(key, []).append((row_index, row))

    results: list[VerifiedPair | VerificationError] = []
    results.extend(errors)
    if not groups:
        results.append(
            VerificationError("CSV has no batched_compare rbposd/ldpc_bposd rows")
        )
        return results

    for key in sorted(groups):
        results.extend(_verify_group(key, groups[key]))
    return results
```

Implement helper functions with these exact behaviors:

- `_verify_group(key, indexed_rows)` splits one Rust row and one Python row,
  rejects duplicates/missing rows, handles explicit Python skip rows, compares
  `PAIR_METADATA_FIELDS`, validates completed row LER math, validates partial
  stop reasons, and emits one `VerifiedPair` on success.
- `_parse_completed_row(row_index, row)` parses `shots_used`, `batch_size`,
  `batches_completed`, `logical_errors`, `logical_error_rate`, and
  `errors_budget`, then verifies finite/nonnegative values and
  `logical_error_rate == logical_errors / shots_used`.
- `_pair_mismatch_message(key, field, rust, python)` returns a message
  containing `Rust/Python pair is no longer comparable` and the mismatched
  field.
- `_format_pair(pair)` returns one PASS line containing:
  `PASS {case_id} code_id={code_id} p={p} num_cycles={num_cycles}
  shots_used={shots_used} batches_completed={batches_completed}
  batch_size={batch_size} stop_reason={stop_reason}
  rbposd_logical_errors={rbposd_logical_errors}
  ldpc_bposd_logical_errors={value_or_skipped}`.
- `main(argv)` loads rows, prints all errors to stderr and returns 1 when any
  `VerificationError` exists; otherwise prints `PASS BB batched paired
  accounting` followed by one PASS line per verified pair and returns 0.

Use `math.isclose(actual, expected, rel_tol=0.0, abs_tol=TOLERANCE)` for LER
math. For `errors_budget_reached`, reject the group unless `errors_budget` is a
positive integer and at least one completed row has
`logical_errors >= errors_budget`.

- [ ] **Step 4: Verify GREEN**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py
```

Expected: all tests pass.

- [ ] **Step 5: Run CLI verifier on checked-in full CSV**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Expected: exit 0 and print `PASS` lines for BB72 and BB144
`errors_budget_reached` cases.

- [ ] **Step 6: Run issue negative control**

Run:

```bash
cp benchmarks/bb_circuit_bposd_compare/results/full/results.csv /tmp/bb_batched_unpaired_bad.csv
python3 - <<'PY'
import csv
from pathlib import Path
path = Path("/tmp/bb_batched_unpaired_bad.csv")
with path.open(newline="") as handle:
    rows = list(csv.DictReader(handle))
fieldnames = list(rows[0].keys())
for row in rows:
    if row["decoder_impl"] == "ldpc_bposd":
        row["shots_used"] = str(int(row["shots_used"]) + 1)
        break
with path.open("w", newline="") as handle:
    writer = csv.DictWriter(handle, fieldnames=fieldnames)
    writer.writeheader()
    writer.writerows(rows)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting /tmp/bb_batched_unpaired_bad.csv
```

Expected: command exits nonzero and stderr says the Rust/Python pair is no
longer comparable.

- [ ] **Step 7: Commit**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/verify_batched_accounting.py \
  benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py \
  docs/superpowers/plans/2026-06-28-issue-310-batched-accounting.md
git commit -m "test: validate bb batched accounting"
```

Expected: commit succeeds with only the verifier, tests, and plan staged.

---

### Task 2: Final Verification And PR Prep

**Files:**
- Modify only if required by Task 1 review findings.

**Interfaces:**
- Consumes: Task 1 verifier CLI and tests.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Run focused pytest**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_batched_accounting.py
```

Expected: all tests pass.

- [ ] **Step 2: Run full CSV verifier**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_batched_accounting \
  benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Expected: exit 0 with `PASS BB batched paired accounting` and PASS lines for
BB72/BB144 `errors_budget_reached` cases.

- [ ] **Step 3: Run negative control**

Run the same `/tmp/bb_batched_unpaired_bad.csv` mutation from Task 1 Step 6.

Expected: verifier exits nonzero and says the Rust/Python pair is no longer
comparable.

- [ ] **Step 4: Run required Rust verification**

Run:

```bash
cargo test
```

Expected: pass. If a pre-existing non-BB test hangs or fails, capture the exact
test name and output, then run the narrow BB-related Rust test command that
covers this repository area:

```bash
cargo test -p rsinter bench_cli
```

Do not claim `cargo test` passed unless the full command exits 0.

- [ ] **Step 5: Inspect final diff**

Run:

```bash
git status --short
git diff --stat origin/master..HEAD
git diff --stat
```

Expected: only #310 verifier/test/docs changes are present.
