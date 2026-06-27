# Issue 309 Bravyi LER Normalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a PASS/FAIL verifier and tests proving BB compare CSV artifacts and plot inputs use Bravyi-style trial-level logical error-rate normalization.

**Architecture:** Add a pure Python CSV verifier under `benchmarks/bb_circuit_bposd_compare` and a focused Python test module for synthetic, checked-in, CLI, and negative-control cases. Add a Rust regression test in the existing BB compare CSV CLI test file to prove the adapter preserves the CSV trial-level metric and the plot fit uses per-shot counts.

**Tech Stack:** Python standard library (`argparse`, `csv`, `math`, `dataclasses`, `subprocess`, `pytest`) and Rust integration tests for the `rsinter` crate.

## Global Constraints

- Do not regenerate the checked-in full BB comparison CSV or PNG.
- Keep the verifier independent of matplotlib and PNG generation.
- Accepted batched rows are `status in {"ok", "partial"}` and `runner == "batched_compare"`.
- Every accepted row must satisfy `logical_error_rate == logical_errors / shots_used` within a tight floating-point tolerance.
- The reviewer table columns are `case_id`, `decoder_impl`, `shots_used`, `logical_errors`, `logical_error_rate`, and `bravyi_tuple`.
- The Bravyi tuple is exactly `(p, num_cycles, shots_used, logical_errors)`.
- Per-cycle-looking mismatches must be named in verifier failure output.
- Rust BB plot input must stay trial-level: default plot logical-rate unit is `per_shot`, and the BB CSV adapter must expose the CSV `logical_error_rate` unchanged.

---

### Task 1: Python Bravyi LER Verifier

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/verify_bravyi_ler.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py`

**Interfaces:**
- Produces: `verify_rows(rows: list[dict[str, str]]) -> list[VerifiedRow | VerificationError]`
- Produces: `VerifiedRow.bravyi_tuple -> tuple[str, int, int, int]`
- Produces: CLI `python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler <csv_path>`

- [ ] **Step 1: Write failing Python tests**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py` with tests for:

```python
from __future__ import annotations

import csv
import subprocess
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import verify_bravyi_ler
from benchmarks.bb_circuit_bposd_compare.cases import BATCHED_CSV_HEADER


FULL_RESULTS = (
    Path(__file__).resolve().parents[1] / "results" / "full" / "results.csv"
)


def make_row(**overrides: str) -> dict[str, str]:
    row = {column: "" for column in BATCHED_CSV_HEADER}
    row.update(
        {
            "case_id": "bb144-p0030-c12-t1000000-seed12345",
            "runner": "batched_compare",
            "decoder_impl": "rbposd",
            "code_id": "bb144",
            "p": "0.003",
            "num_cycles": "12",
            "shots_budget": "1000000",
            "errors_budget": "200",
            "shots_used": "40000",
            "logical_errors": "200",
            "logical_error_rate": "0.005",
            "status": "ok",
            "stop_reason": "errors_budget_reached",
        }
    )
    row.update(overrides)
    return row


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=BATCHED_CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)


def partition(
    result: list[verify_bravyi_ler.VerifiedRow | verify_bravyi_ler.VerificationError],
) -> tuple[list[verify_bravyi_ler.VerifiedRow], list[verify_bravyi_ler.VerificationError]]:
    verified_rows = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerifiedRow)
    ]
    verification_errors = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerificationError)
    ]
    return verified_rows, verification_errors


def test_verify_rows_accepts_ok_and_partial_trial_level_rows() -> None:
    rows = [
        make_row(),
        make_row(
            case_id="bb72-p0040-c6-t1000000-seed12345",
            decoder_impl="ldpc_bposd",
            code_id="bb72",
            p="0.004",
            num_cycles="6",
            shots_used="1000",
            logical_errors="25",
            logical_error_rate="0.025",
            status="partial",
            stop_reason="wall_budget_exhausted",
        ),
    ]

    result = verify_bravyi_ler.verify_rows(rows)
    verified_rows, verification_errors = partition(result)

    assert verification_errors == []
    assert [row.bravyi_tuple for row in verified_rows] == [
        ("0.003", 12, 40000, 200),
        ("0.004", 6, 1000, 25),
    ]


def test_verify_rows_rejects_per_cycle_normalized_row() -> None:
    row = make_row(logical_error_rate=str(200 / (40000 * 12)))

    result = verify_bravyi_ler.verify_rows([row])
    _, verification_errors = partition(result)

    assert verification_errors
    assert "appears per-cycle normalized" in verification_errors[0].message
    assert "trial-level LER" in verification_errors[0].message


def test_checked_in_full_results_are_trial_level_normalized() -> None:
    rows = verify_bravyi_ler.load_rows(FULL_RESULTS)

    result = verify_bravyi_ler.verify_rows(rows)
    verified_rows, verification_errors = partition(result)

    assert verification_errors == []
    assert verified_rows
    bb144_rows = [row for row in verified_rows if "bb144" in row.case_id]
    assert bb144_rows
    assert any(
        row.bravyi_tuple == ("0.003", 12, 40000, 200)
        for row in bb144_rows
    )


def test_cli_prints_pass_table_for_valid_csv(tmp_path: Path) -> None:
    csv_path = tmp_path / "results.csv"
    write_csv(csv_path, [make_row()])

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "PASS" in result.stdout
    assert "case_id" in result.stdout
    assert "bravyi_tuple=(0.003, 12, 40000, 200)" in result.stdout


def test_cli_negative_control_exits_nonzero_for_per_cycle_csv(tmp_path: Path) -> None:
    csv_path = tmp_path / "bb_ler_per_cycle_bad.csv"
    write_csv(csv_path, [make_row(logical_error_rate=str(200 / (40000 * 12)))])

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "appears per-cycle normalized" in result.stderr
```

- [ ] **Step 2: Verify RED**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
```

Expected: fail with an import error or missing module error for `verify_bravyi_ler`.

- [ ] **Step 3: Implement verifier**

Create `benchmarks/bb_circuit_bposd_compare/verify_bravyi_ler.py` with:

```python
from __future__ import annotations

import argparse
import csv
import math
import sys
from dataclasses import dataclass
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.cases import BATCHED_CSV_HEADER

ACCEPTED_STATUSES = {"ok", "partial"}
TOLERANCE = 1e-12
REQUIRED_COLUMNS = (
    "case_id",
    "runner",
    "decoder_impl",
    "p",
    "num_cycles",
    "shots_used",
    "logical_errors",
    "logical_error_rate",
    "status",
)


@dataclass(frozen=True)
class VerifiedRow:
    case_id: str
    decoder_impl: str
    shots_used: int
    logical_errors: int
    logical_error_rate: float
    bravyi_tuple: tuple[str, int, int, int]


@dataclass(frozen=True)
class VerificationError:
    message: str


def load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def verify_rows(rows: list[dict[str, str]]) -> list[VerifiedRow | VerificationError]:
    results: list[VerifiedRow | VerificationError] = []
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

    for row_index, row in enumerate(rows, start=2):
        if not _is_accepted_batched_row(row):
            continue
        parsed = _parse_row(row, row_index)
        if isinstance(parsed, VerificationError):
            results.append(parsed)
            continue
        case_id, decoder_impl, p, num_cycles, shots_used, logical_errors, actual = parsed
        expected = logical_errors / shots_used
        if not math.isclose(actual, expected, rel_tol=0.0, abs_tol=TOLERANCE):
            results.append(
                VerificationError(_mismatch_message(row_index, row, actual, expected))
            )
            continue
        results.append(
            VerifiedRow(
                case_id=case_id,
                decoder_impl=decoder_impl,
                shots_used=shots_used,
                logical_errors=logical_errors,
                logical_error_rate=actual,
                bravyi_tuple=(p, num_cycles, shots_used, logical_errors),
            )
        )

    if not results:
        return [VerificationError("CSV has no completed or partial batched rows to verify")]
    return results


def _is_accepted_batched_row(row: dict[str, str]) -> bool:
    return (
        row.get("runner") == "batched_compare"
        and row.get("status") in ACCEPTED_STATUSES
    )


def _parse_row(
    row: dict[str, str],
    row_index: int,
) -> tuple[str, str, str, int, int, int, float] | VerificationError:
    context = f"row {row_index} {row.get('case_id', '<missing case_id>')}"

    def parse_int(field_name: str) -> int | VerificationError:
        try:
            return int(row[field_name])
        except (TypeError, ValueError) as error:
            return VerificationError(
                f"{context}: failed to parse numeric field {field_name}: {error}"
            )

    def parse_float(field_name: str) -> float | VerificationError:
        try:
            return float(row[field_name])
        except (TypeError, ValueError) as error:
            return VerificationError(
                f"{context}: failed to parse numeric field {field_name}: {error}"
            )

    num_cycles = parse_int("num_cycles")
    if isinstance(num_cycles, VerificationError):
        return num_cycles
    shots_used = parse_int("shots_used")
    if isinstance(shots_used, VerificationError):
        return shots_used
    logical_errors = parse_int("logical_errors")
    if isinstance(logical_errors, VerificationError):
        return logical_errors
    actual = parse_float("logical_error_rate")
    if isinstance(actual, VerificationError):
        return actual

    if num_cycles <= 0:
        return VerificationError(f"{context}: num_cycles must be positive")
    if shots_used <= 0:
        return VerificationError(
            f"{context}: shots_used must be positive for trial-level LER"
        )
    if logical_errors < 0:
        return VerificationError(f"{context}: logical_errors must be nonnegative")
    if logical_errors > shots_used:
        return VerificationError(f"{context}: logical_errors must be <= shots_used")
    if not math.isfinite(actual):
        return VerificationError(f"{context}: logical_error_rate must be finite")
    return (
        row["case_id"],
        row["decoder_impl"],
        row["p"],
        num_cycles,
        shots_used,
        logical_errors,
        actual,
    )


def _mismatch_message(
    row_index: int,
    row: dict[str, str],
    actual: float,
    expected: float,
) -> str:
    shots_used = int(row["shots_used"])
    logical_errors = int(row["logical_errors"])
    num_cycles = int(row["num_cycles"])
    per_cycle = logical_errors / (shots_used * num_cycles)
    context = f"row {row_index} {row.get('case_id', '<missing case_id>')} {row.get('decoder_impl', '')}"
    if math.isclose(actual, per_cycle, rel_tol=0.0, abs_tol=TOLERANCE):
        return (
            f"{context}: logical_error_rate appears per-cycle normalized; "
            f"got {actual}, expected trial-level LER {expected} "
            f"from logical_errors/shots_used"
        )
    if 0.0 < actual < expected:
        ratio = expected / actual
        nearest = round(ratio)
        if nearest >= 2 and math.isclose(ratio, nearest, rel_tol=0.0, abs_tol=1e-9):
            return (
                f"{context}: logical_error_rate appears divided by {nearest} "
                f"before plotting; got {actual}, expected trial-level LER {expected}"
            )
    return (
        f"{context}: logical_error_rate mismatched trial-level LER; "
        f"got {actual}, expected {expected} from logical_errors/shots_used"
    )


def format_table(rows: list[VerifiedRow]) -> str:
    header = (
        "case_id decoder_impl shots_used logical_errors "
        "logical_error_rate bravyi_tuple"
    )
    lines = [header]
    for row in rows:
        p, num_cycles, shots_used, logical_errors = row.bravyi_tuple
        lines.append(
            f"{row.case_id} {row.decoder_impl} {row.shots_used} "
            f"{row.logical_errors} {row.logical_error_rate:.17g} "
            f"bravyi_tuple=({p}, {num_cycles}, {shots_used}, {logical_errors})"
        )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("csv_path", type=Path)
    args = parser.parse_args(argv)

    results = verify_rows(load_rows(args.csv_path))
    errors = [item for item in results if isinstance(item, VerificationError)]
    verified_rows = [item for item in results if isinstance(item, VerifiedRow)]
    if errors:
        for error in errors:
            print(error.message, file=sys.stderr)
        return 1
    print("PASS Bravyi trial-level LER normalization")
    print(format_table(verified_rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Verify GREEN**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Expected: both commands exit 0; verifier prints PASS rows including a BB144 tuple.

- [ ] **Step 5: Commit**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/verify_bravyi_ler.py benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
git commit -m "feat: verify bravyi ler normalization"
```

---

### Task 2: Rust BB Plot Adapter Regression

**Files:**
- Modify: `rsinter/tests/bench_cli.rs`

**Interfaces:**
- Consumes: `rsinter::bench::bb_compare_csv::read_bb_compare_csv`
- Consumes: `rsinter::bench::plot::logical_rate_fit_for_plot`
- Consumes: `rsinter::bench::spec::LogicalRateUnit`
- Produces: regression test `bb_compare_csv_adapter_preserves_trial_level_ler_for_plot_input`

- [ ] **Step 1: Write Rust regression test**

Add imports near the top of `rsinter/tests/bench_cli.rs`:

```rust
use rsinter::bench::bb_compare_csv::read_bb_compare_csv;
use rsinter::bench::plot::logical_rate_fit_for_plot;
use rsinter::bench::spec::LogicalRateUnit;
```

Add this test near the existing BB compare CSV test:

```rust
#[test]
fn bb_compare_csv_adapter_preserves_trial_level_ler_for_plot_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bb_results.csv");
    fs::write(
        &input,
        "case_id,runner,decoder_impl,code_id,p,num_cycles,shots_budget,errors_budget,shots_used,seed,bp_method,max_iter,osd_method,osd_order,batch_size,batches_completed,setup_seconds,sample_seconds,decode_seconds,run_seconds,logical_errors,logical_error_rate,bp_seconds,osd_seconds,decode_call_count,bp_iteration_count,osd_use_count,osd_candidate_count,gf2_solve_count,gf2_full_elimination_count,status,stop_reason,error\n\
bb144-p0030-c12-t1000000-seed12345,batched_compare,rbposd,bb144,0.003,12,1000000,200,40000,12345,ms,10000,osd_cs,7,500,80,1.0,2.0,3.0,6.0,200,0.005,1.0,2.0,20,10,1,16,1,1,ok,errors_budget_reached,\n",
    )
    .unwrap();

    let rows = read_bb_compare_csv(&input, "bb_circuit_bposd_compare").unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.params["rounds"], serde_json::json!(12));
    assert_eq!(row.case_summary["logical_observable_count"], serde_json::json!(1));
    assert_eq!(row.metrics["logical_errors"], 200.0);
    assert_eq!(row.metrics["shots_used"], 40000.0);
    assert_eq!(row.metrics["logical_error_rate"], 0.005);

    let fit = logical_rate_fit_for_plot(row, LogicalRateUnit::PerShot).unwrap();
    assert_eq!(fit.best, Some(0.005));
    assert_ne!(fit.best, Some(200.0 / (40000.0 * 12.0)));
}
```

- [ ] **Step 2: Verify focused Rust test**

Run:

```bash
cargo test -p rsinter --test bench_cli bb_compare_csv_adapter_preserves_trial_level_ler_for_plot_input
```

Expected: pass. If it fails because the adapter transforms the value, change the adapter so `logical_error_rate` is copied unchanged from the CSV and default plot fitting uses `LogicalRateUnit::PerShot`.

- [ ] **Step 3: Commit**

Run:

```bash
git add rsinter/tests/bench_cli.rs
git commit -m "test: lock bb plot trial-level ler"
```

---

### Task 3: Final Verification

**Files:**
- No new files unless fixes are required by verification.

**Interfaces:**
- Confirms the issue verification commands and required Rust gate.

- [ ] **Step 1: Run required Python test**

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py
```

Expected: exit 0.

- [ ] **Step 2: Run required verifier command**

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler benchmarks/bb_circuit_bposd_compare/results/full/results.csv
```

Expected: exit 0 and print PASS rows for the checked-in full CSV.

- [ ] **Step 3: Run negative control**

Create `/tmp/bb_ler_per_cycle_bad.csv` by copying the checked-in full CSV and mutating one accepted row's `logical_error_rate` to `logical_errors / (shots_used * num_cycles)`, then run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler /tmp/bb_ler_per_cycle_bad.csv
```

Expected: exit nonzero and mention per-cycle normalization or mismatched trial-level LER.

- [ ] **Step 4: Run required Rust gate**

```bash
cargo test
```

Expected: exit 0.

- [ ] **Step 5: Commit verification-only fixes if needed**

If verification reveals a code or test issue, fix it with a failing test first when applicable, rerun the focused check, and commit the fix with a scoped message.
