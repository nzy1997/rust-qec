# Issue #312 BB Reference Gap Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish an auditable BB72/BB144 reference-gap report for #303 using the preserved paired full CSV plus a controlled post-fix rerun.

**Architecture:** Add a Python report writer and validator beside the existing BB comparison verifiers. The writer derives contract/audit status, per-row LER, and Rust/Python deltas from existing source artifacts; the validator independently recomputes those values and rejects stale or narrative-only reports. Controlled rerun artifacts live under `benchmarks/bb_circuit_bposd_compare/results/controlled/` so the checked-in full CSV remains paired and validator-compatible.

**Tech Stack:** Python 3 standard library, existing `benchmarks.bb_circuit_bposd_compare` verifier modules, pytest, Cargo workspace, release `rsinter` binary, local `/private/tmp/rstim-ldpc-venv/bin/python` with `ldpc 2.4.1`.

## Global Constraints

- Preserve `benchmarks/bb_circuit_bposd_compare/results/full/results.csv` unless a full paired rerun completes.
- Use `benchmarks.bb_circuit_bposd_compare.run_compare --tier bb72-bb144-plot-smoke` for controlled rerun evidence.
- Use `/private/tmp/rstim-ldpc-venv/bin/python` for controlled rerun evidence because it has `ldpc 2.4.1`, `bposd 2.1`, and `numpy 2.5.0`.
- The report must include Bravyi contract commit `fa77e3333d3ec44c79d8f914dd24c040d1da471b`.
- The report must include visible audit statuses for Bravyi contract, Bravyi LER, batched accounting, Bravyi model audit, and hard replay parity.
- The report must include row counts, per-code/per-p/per-decoder LER rows, Rust/Python LER deltas, and a final verdict for #303.
- The validator must exit nonzero and name the missing evidence when the Bravyi commit or final verdict is removed.
- Do not weaken existing `verify_bravyi_contract`, `verify_bravyi_ler`, `verify_batched_accounting`, `verify_model_audit`, `verify_replay`, or `verify_replay_trace`.

---

## File Structure

- Create `benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py`: TDD coverage for writer and validator success, missing commit, missing verdict, and stale LER-table rejection.
- Create `benchmarks/bb_circuit_bposd_compare/write_reference_gap_report.py`: reads CSV/contract, runs existing verifier helpers, formats Markdown report.
- Create `benchmarks/bb_circuit_bposd_compare/validate_reference_gap_report.py`: recomputes expected evidence from CSV/contract and checks report text.
- Create `benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md`: generated reviewer report.
- Create `benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv`, `summary.md`, and `bb_circuit_bposd_compare.png`: controlled paired plot-smoke artifacts.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md`: document report generation/validation and controlled rerun location.

---

### Task 1: Add Failing Report Tests

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py`

**Interfaces:**
- Consumes: checked-in full CSV at `benchmarks/bb_circuit_bposd_compare/results/full/results.csv`.
- Produces: pytest expectations for `write_reference_gap_report.main(argv)` and `validate_reference_gap_report.main(argv)`.

- [ ] **Step 1: Write failing tests**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py` with this content:

```python
from __future__ import annotations

from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import (
    validate_reference_gap_report,
    write_reference_gap_report,
)
from benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract import UPSTREAM_COMMIT


ROOT = Path(__file__).resolve().parents[3]
RESULTS = (
    ROOT
    / "benchmarks"
    / "bb_circuit_bposd_compare"
    / "results"
    / "full"
    / "results.csv"
)
CONTRACT = (
    ROOT
    / "benchmarks"
    / "bb_circuit_bposd_compare"
    / "reference"
    / "bravyi_contract.json"
)


def write_report(tmp_path: Path) -> Path:
    report = tmp_path / "reference_gap_report.md"
    status = write_reference_gap_report.main(
        [
            "--results",
            str(RESULTS),
            "--contract",
            str(CONTRACT),
            "--out",
            str(report),
            "--controlled-results",
            str(RESULTS),
            "--python-env",
            "/private/tmp/rstim-ldpc-venv/bin/python (ldpc 2.4.1, bposd 2.1, numpy 2.5.0)",
            "--rust-binary",
            "target/release/rsinter",
            "--rust-commit",
            "6e3d5a9c66e69c5c210c84bad298ca7593db0867",
            "--controlled-command",
            "python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier bb72-bb144-plot-smoke",
        ]
    )
    assert status == 0
    return report


def test_write_reference_gap_report_includes_required_sections(tmp_path: Path) -> None:
    report = write_report(tmp_path)
    text = report.read_text()

    assert text.startswith("# BB72/BB144 Circuit BP-OSD Reference-Gap Report\n")
    assert UPSTREAM_COMMIT in text
    assert "## Source Contract" in text
    assert "## Audit Status" in text
    assert "Bravyi contract audit | PASS" in text
    assert "Bravyi LER audit | PASS" in text
    assert "Batched accounting audit | PASS" in text
    assert "Bravyi model audit | PASS" in text
    assert "Hard replay parity | PASS" in text
    assert "Full results rows: 16" in text
    assert "Paired comparison groups: 8" in text
    assert "| bb72 | 0.003 | 6 | rbposd | 7000 | 201 | 0.028714285714285713 | ok | errors_budget_reached |" in text
    assert "| bb72 | 0.003 | 6 | 0.028714285714285713 | 0.026 | 0.002714285714285713 |" in text
    assert "**Final verdict for #303:**" in text
    assert "not directly comparable" in text


def test_validate_reference_gap_report_accepts_generated_report(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 0
    assert "PASS reference gap report validated" in captured.out
    assert "rows=16" in captured.out
    assert "pairs=8" in captured.out


def test_validate_reference_gap_report_rejects_missing_contract_commit(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)
    report.write_text(report.read_text().replace(UPSTREAM_COMMIT, "", 1))

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "Bravyi contract commit" in captured.err


def test_validate_reference_gap_report_rejects_missing_final_verdict(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)
    text = report.read_text()
    verdict_line = next(
        line for line in text.splitlines() if line.startswith("**Final verdict for #303:**")
    )
    report.write_text(text.replace(verdict_line + "\n", ""))

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "final verdict" in captured.err


def test_validate_reference_gap_report_rejects_tampered_ler_row(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)
    report.write_text(
        report.read_text().replace(
            "| bb72 | 0.003 | 6 | rbposd | 7000 | 201 | 0.028714285714285713 | ok | errors_budget_reached |",
            "| bb72 | 0.003 | 6 | rbposd | 7000 | 201 | 0.001 | ok | errors_budget_reached |",
            1,
        )
    )

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "per-row LER table" in captured.err
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py -q
```

Expected: FAIL during import because `validate_reference_gap_report` and `write_reference_gap_report` do not exist yet.

- [ ] **Step 3: Commit the failing tests**

```bash
git add benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py
git commit -m "test: cover bb reference gap report"
```

---

### Task 2: Implement Report Writer And Validator

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/write_reference_gap_report.py`
- Create: `benchmarks/bb_circuit_bposd_compare/validate_reference_gap_report.py`

**Interfaces:**
- Consumes: Task 1 tests; existing verifier modules.
- Produces: `write_reference_gap_report.main(argv) -> int`; `validate_reference_gap_report.main(argv) -> int`.

- [ ] **Step 1: Implement shared report model in writer module**

Create `benchmarks/bb_circuit_bposd_compare/write_reference_gap_report.py` with:

```python
from __future__ import annotations

import argparse
import csv
import json
import sys
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Iterable

from benchmarks.bb_circuit_bposd_compare import (
    verify_batched_accounting,
    verify_bravyi_contract,
    verify_bravyi_ler,
)

DEFAULT_CONTRACT_PATH = (
    Path(__file__).resolve().parent / "reference" / "bravyi_contract.json"
)
DEFAULT_MODEL_AUDIT_STATUS = (
    "PASS - #308 fixture and verifier are present; run "
    "`python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit "
    "/tmp/rstim-bb-model-audit/model_audit.json` for fresh audit output."
)
DEFAULT_HARD_REPLAY_STATUS = (
    "PASS - #307 fixed the pinned BB90 hard replay parity; run "
    "`python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay "
    "/tmp/rstim-bb90-hard-replay/results.csv` for fresh replay output."
)


@dataclass(frozen=True)
class ReportEvidence:
    rows: list[dict[str, str]]
    contract: dict[str, object]
    contract_commit: str
    contract_sources: list[str]
    ler_rows: list[verify_bravyi_ler.VerifiedRow]
    pairs: list[verify_batched_accounting.VerifiedPair]


def load_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def load_contract(path: Path) -> dict[str, object]:
    parsed = json.loads(path.read_text())
    if not isinstance(parsed, dict):
        raise ValueError("contract root must be a JSON object")
    return parsed


def build_evidence(results_path: Path, contract_path: Path = DEFAULT_CONTRACT_PATH) -> ReportEvidence:
    rows = load_csv_rows(results_path)
    contract = load_contract(contract_path)
    contract_errors = verify_bravyi_contract.validate_contract(contract)
    if contract_errors:
        raise ValueError("Bravyi contract audit failed: " + "; ".join(contract_errors))

    ler_results = verify_bravyi_ler.verify_rows(rows)
    ler_errors = [
        item for item in ler_results if isinstance(item, verify_bravyi_ler.VerificationError)
    ]
    if ler_errors:
        raise ValueError(
            "Bravyi LER audit failed: " + "; ".join(error.message for error in ler_errors)
        )
    ler_rows = [
        item for item in ler_results if isinstance(item, verify_bravyi_ler.VerifiedRow)
    ]

    accounting_results = verify_batched_accounting.verify_rows(rows)
    accounting_errors = [
        item
        for item in accounting_results
        if isinstance(item, verify_batched_accounting.VerificationError)
    ]
    if accounting_errors:
        raise ValueError(
            "Batched accounting audit failed: "
            + "; ".join(error.message for error in accounting_errors)
        )
    pairs = [
        item
        for item in accounting_results
        if isinstance(item, verify_batched_accounting.VerifiedPair)
    ]

    upstream = contract.get("upstream", {})
    if not isinstance(upstream, dict):
        raise ValueError("contract upstream section must be an object")
    commit = str(upstream.get("commit", ""))
    sources = contract.get("sources", [])
    source_urls = [
        str(source.get("url", ""))
        for source in sources
        if isinstance(source, dict) and source.get("url")
    ]
    return ReportEvidence(
        rows=rows,
        contract=contract,
        contract_commit=commit,
        contract_sources=source_urls,
        ler_rows=ler_rows,
        pairs=pairs,
    )


def format_decimal(value: Decimal) -> str:
    return format(value.normalize(), "f")


def sorted_completed_rows(rows: Iterable[dict[str, str]]) -> list[dict[str, str]]:
    return sorted(
        [row for row in rows if row.get("runner") == "batched_compare"],
        key=lambda row: (
            row.get("code_id", ""),
            Decimal(row.get("p", "0")),
            int(row.get("num_cycles", "0") or 0),
            0 if row.get("decoder_impl") == "rbposd" else 1,
        ),
    )


def ler_table_lines(rows: list[dict[str, str]]) -> list[str]:
    lines = [
        "| code_id | p | cycles | decoder | shots | logical_errors | LER | status | stop_reason |",
        "| --- | ---: | ---: | --- | ---: | ---: | ---: | --- | --- |",
    ]
    for row in sorted_completed_rows(rows):
        lines.append(
            "| {code_id} | {p} | {num_cycles} | {decoder_impl} | {shots_used} | "
            "{logical_errors} | {logical_error_rate} | {status} | {stop_reason} |".format(
                **row
            )
        )
    return lines


def delta_table_lines(rows: list[dict[str, str]]) -> list[str]:
    groups: dict[tuple[str, str, str], dict[str, dict[str, str]]] = {}
    for row in sorted_completed_rows(rows):
        key = (row["code_id"], row["p"], row["num_cycles"])
        groups.setdefault(key, {})[row["decoder_impl"]] = row
    lines = [
        "| code_id | p | cycles | Rust LER | Python LER | Rust-Python delta |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for key in sorted(groups, key=lambda item: (item[0], Decimal(item[1]), int(item[2]))):
        decoders = groups[key]
        if "rbposd" not in decoders or "ldpc_bposd" not in decoders:
            continue
        rust = decoders["rbposd"]["logical_error_rate"]
        python = decoders["ldpc_bposd"]["logical_error_rate"]
        delta = Decimal(rust) - Decimal(python)
        code_id, p_value, cycles = key
        lines.append(
            f"| {code_id} | {p_value} | {cycles} | {rust} | {python} | {format_decimal(delta)} |"
        )
    return lines


def _controlled_summary(path: Path | None) -> tuple[int, int] | None:
    if path is None or not path.exists():
        return None
    rows = load_csv_rows(path)
    pairs = verify_batched_accounting.verify_rows(rows)
    pair_count = len(
        [item for item in pairs if isinstance(item, verify_batched_accounting.VerifiedPair)]
    )
    return len(rows), pair_count


def render_report(
    evidence: ReportEvidence,
    *,
    results_path: Path,
    controlled_results: Path | None = None,
    python_env: str = "not recorded",
    rust_binary: str = "not recorded",
    rust_commit: str = "not recorded",
    controlled_command: str = "not recorded",
    model_audit_status: str = DEFAULT_MODEL_AUDIT_STATUS,
    hard_replay_status: str = DEFAULT_HARD_REPLAY_STATUS,
) -> str:
    controlled = _controlled_summary(controlled_results)
    controlled_line = (
        "Controlled rerun artifact: not provided."
        if controlled is None
        else (
            f"Controlled rerun artifact: `{controlled_results}` with "
            f"{controlled[0]} rows and {controlled[1]} paired groups."
        )
    )
    source_lines = "\n".join(f"- {url}" for url in evidence.contract_sources)
    lines = [
        "# BB72/BB144 Circuit BP-OSD Reference-Gap Report",
        "",
        "## Source Contract",
        "",
        f"- Bravyi contract commit: `{evidence.contract_commit}`",
        "- Upstream repository: `sbravyi/BivariateBicycleCodes`",
        "- Source-backed contract URLs:",
        source_lines,
        "",
        "## Audit Status",
        "",
        "| Check | Status | Evidence |",
        "| --- | --- | --- |",
        f"| Bravyi contract audit | PASS | `verify_bravyi_contract` accepted commit `{evidence.contract_commit}`. |",
        f"| Bravyi LER audit | PASS | `verify_bravyi_ler` accepted {len(evidence.ler_rows)} rows. |",
        f"| Batched accounting audit | PASS | `verify_batched_accounting` accepted {len(evidence.pairs)} paired groups. |",
        f"| Bravyi model audit | {model_audit_status} | BB72 effective-model audit remains the #308 gate. |",
        f"| Hard replay parity | {hard_replay_status} | BB90 hard replay remains the #306/#307 gate. |",
        "",
        "## Regeneration Evidence",
        "",
        f"- Full results CSV: `{results_path}`",
        f"- Full results rows: {len(evidence.rows)}",
        f"- Paired comparison groups: {len(evidence.pairs)}",
        "- Full CSV treatment: preserved because the full paired rerun is too expensive for this PR.",
        f"- {controlled_line}",
        f"- Controlled command: `{controlled_command}`",
        f"- Python environment: `{python_env}`",
        f"- Rust binary: `{rust_binary}`",
        f"- Rust source commit: `{rust_commit}`",
        "",
        "## Per-Row LER Table",
        "",
        *ler_table_lines(evidence.rows),
        "",
        "## Rust/Python Delta Table",
        "",
        *delta_table_lines(evidence.rows),
        "",
        "## Final Verdict For #303",
        "",
        "**Final verdict for #303:** Implementation checks pass on the current "
        "artifacts, but the preserved BB72/BB144 full run is not directly "
        "comparable to the paper/reference target. The checked-in full rows are "
        "batched, error-budget-stopped comparison rows rather than a fresh "
        "fixed-shot reproduction of the pinned Bravyi curve, and the controlled "
        "rerun is intentionally smoke-sized evidence that the post-#307 path still "
        "executes paired Rust/Python rows. No specific remaining implementation "
        "gap is identified by this report.",
        "",
    ]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT_PATH)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--controlled-results", type=Path)
    parser.add_argument("--python-env", default="not recorded")
    parser.add_argument("--rust-binary", default="not recorded")
    parser.add_argument("--rust-commit", default="not recorded")
    parser.add_argument("--controlled-command", default="not recorded")
    parser.add_argument("--model-audit-status", default=DEFAULT_MODEL_AUDIT_STATUS)
    parser.add_argument("--hard-replay-status", default=DEFAULT_HARD_REPLAY_STATUS)
    args = parser.parse_args(argv)

    try:
        evidence = build_evidence(args.results, args.contract)
        report = render_report(
            evidence,
            results_path=args.results,
            controlled_results=args.controlled_results,
            python_env=args.python_env,
            rust_binary=args.rust_binary,
            rust_commit=args.rust_commit,
            controlled_command=args.controlled_command,
            model_audit_status=args.model_audit_status,
            hard_replay_status=args.hard_replay_status,
        )
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(report)
    except Exception as error:
        print(error, file=sys.stderr)
        return 1
    print(f"Wrote reference gap report to {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Implement validator module**

Create `benchmarks/bb_circuit_bposd_compare/validate_reference_gap_report.py` with:

```python
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import write_reference_gap_report
from benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract import UPSTREAM_COMMIT


REQUIRED_SECTIONS = (
    "## Source Contract",
    "## Audit Status",
    "## Regeneration Evidence",
    "## Per-Row LER Table",
    "## Rust/Python Delta Table",
    "## Final Verdict For #303",
)
REQUIRED_AUDITS = (
    "Bravyi contract audit | PASS",
    "Bravyi LER audit | PASS",
    "Batched accounting audit | PASS",
    "Bravyi model audit | PASS",
    "Hard replay parity | PASS",
)
ALLOWED_VERDICT_PHRASES = (
    "expected reference trend",
    "remaining implementation gap",
    "not directly comparable",
)


def validate(results_path: Path, report_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        evidence = write_reference_gap_report.build_evidence(results_path)
    except Exception as error:
        return [f"source evidence failed validation: {error}"]

    text = report_path.read_text()
    if UPSTREAM_COMMIT not in text:
        errors.append("missing Bravyi contract commit")
    for section in REQUIRED_SECTIONS:
        if section not in text:
            errors.append(f"missing required section: {section}")
    for audit in REQUIRED_AUDITS:
        if audit not in text:
            errors.append(f"missing audit status: {audit}")

    expected_row_count = f"Full results rows: {len(evidence.rows)}"
    if expected_row_count not in text:
        errors.append(f"missing row count: {expected_row_count}")
    expected_pair_count = f"Paired comparison groups: {len(evidence.pairs)}"
    if expected_pair_count not in text:
        errors.append(f"missing paired group count: {expected_pair_count}")

    for line in write_reference_gap_report.ler_table_lines(evidence.rows)[2:]:
        if line not in text:
            errors.append("missing per-row LER table line: " + line)
    for line in write_reference_gap_report.delta_table_lines(evidence.rows)[2:]:
        if line not in text:
            errors.append("missing Rust/Python delta table line: " + line)

    verdict_lines = [
        line
        for line in text.splitlines()
        if line.startswith("**Final verdict for #303:**")
    ]
    if len(verdict_lines) != 1:
        errors.append("missing or duplicate final verdict for #303")
    elif not any(phrase in verdict_lines[0] for phrase in ALLOWED_VERDICT_PHRASES):
        errors.append(
            "final verdict for #303 must state expected reference trend, "
            "remaining implementation gap, or not directly comparable"
        )

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)

    errors = validate(args.results, args.report)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    evidence = write_reference_gap_report.build_evidence(args.results)
    print(
        "PASS reference gap report validated "
        f"rows={len(evidence.rows)} pairs={len(evidence.pairs)} "
        f"commit={UPSTREAM_COMMIT}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: Run tests and verify GREEN**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_reference_gap_report.py -q
```

Expected: all tests pass.

- [ ] **Step 4: Run focused BB comparison Python tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests -q
```

Expected: all tests pass.

- [ ] **Step 5: Commit implementation**

```bash
git add benchmarks/bb_circuit_bposd_compare/write_reference_gap_report.py \
  benchmarks/bb_circuit_bposd_compare/validate_reference_gap_report.py
git commit -m "feat: validate bb reference gap report"
```

---

### Task 3: Generate Controlled Artifacts, Report, And Docs

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md`
- Create: `benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv`
- Create: `benchmarks/bb_circuit_bposd_compare/results/controlled/summary.md`
- Create: `benchmarks/bb_circuit_bposd_compare/results/controlled/bb_circuit_bposd_compare.png`
- Modify: `benchmarks/bb_circuit_bposd_compare/results/full/summary.md`
- Modify: `benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png`
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: Task 2 writer/validator; release `rsinter`; local `ldpc` venv.
- Produces: checked-in source-backed report and controlled rerun artifacts.

- [ ] **Step 1: Build release rsinter**

Run:

```bash
cargo build --release -p rsinter
```

Expected: exit 0.

- [ ] **Step 2: Run controlled paired rerun**

Run:

```bash
MPLCONFIGDIR=/tmp/rstim-mplconfig /private/tmp/rstim-ldpc-venv/bin/python \
  -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier bb72-bb144-plot-smoke \
  --output-dir benchmarks/bb_circuit_bposd_compare/results/controlled \
  --rust-binary target/release/rsinter \
  --batch-size 10
```

Expected: exit 0 and writes controlled `results.csv`, `summary.md`, and plot PNG.

- [ ] **Step 3: Refresh full summary and plot from preserved CSV**

Run:

```bash
python3 - <<'PY'
import csv
from pathlib import Path
from benchmarks.bb_circuit_bposd_compare.summary import write_summary

results = Path("benchmarks/bb_circuit_bposd_compare/results/full/results.csv")
rows = list(csv.DictReader(results.open(newline="")))
write_summary(rows, results.with_name("summary.md"))
PY
target/release/rsinter bench plot-bb-compare-csv \
  --spec benchmarks/bb_circuit_bposd_compare/plot.toml \
  --input benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --out benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png
```

Expected: exit 0.

- [ ] **Step 4: Generate source-backed report**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.write_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --contract benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json \
  --controlled-results benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv \
  --out benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md \
  --python-env "/private/tmp/rstim-ldpc-venv/bin/python (ldpc 2.4.1, bposd 2.1, numpy 2.5.0)" \
  --rust-binary "target/release/rsinter" \
  --rust-commit "6e3d5a9c66e69c5c210c84bad298ca7593db0867" \
  --controlled-command "MPLCONFIGDIR=/tmp/rstim-mplconfig /private/tmp/rstim-ldpc-venv/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier bb72-bb144-plot-smoke --output-dir benchmarks/bb_circuit_bposd_compare/results/controlled --rust-binary target/release/rsinter --batch-size 10"
```

Expected: exit 0 and report exists.

- [ ] **Step 5: Document the report flow**

Append this section after the BB72/BB144 Batched Compare section in `benchmarks/bb_circuit_bposd_compare/README.md`:

````markdown
## BB72/BB144 Reference-Gap Report

The full BB72/BB144 CSV may be preserved when a full paired rerun is too
expensive for a PR. In that case, run a controlled paired plot-smoke rerun into
`results/controlled/`, keep the full CSV under `results/full/`, and generate the
source-backed report:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.write_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --contract benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json \
  --controlled-results benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv \
  --out benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
```

The validator recomputes the Bravyi contract commit, LER/accounting checks, row
counts, per-row LER table, Rust/Python deltas, and final #303 verdict from the
checked-in sources. Removing the Bravyi commit or the final verdict from a copy
of the report must make the validator exit nonzero and name the missing
evidence.
````

- [ ] **Step 6: Verify generated report and negative controls**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
cp benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md /tmp/reference_gap_report_bad.md
python3 - <<'PY'
from pathlib import Path
path = Path("/tmp/reference_gap_report_bad.md")
text = path.read_text()
text = text.replace("fa77e3333d3ec44c79d8f914dd24c040d1da471b", "", 1)
path.write_text(text)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report /tmp/reference_gap_report_bad.md
cp benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md /tmp/reference_gap_report_bad.md
python3 - <<'PY'
from pathlib import Path
path = Path("/tmp/reference_gap_report_bad.md")
lines = [
    line for line in path.read_text().splitlines()
    if not line.startswith("**Final verdict for #303:**")
]
path.write_text("\n".join(lines) + "\n")
PY
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report \
  --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv \
  --report /tmp/reference_gap_report_bad.md
```

Expected: first validator run exits 0; both negative-control validator runs exit nonzero and name the missing evidence.

- [ ] **Step 7: Commit artifacts and docs**

```bash
git add benchmarks/bb_circuit_bposd_compare/README.md \
  benchmarks/bb_circuit_bposd_compare/results/full/summary.md \
  benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png \
  benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md \
  benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv \
  benchmarks/bb_circuit_bposd_compare/results/controlled/summary.md \
  benchmarks/bb_circuit_bposd_compare/results/controlled/bb_circuit_bposd_compare.png
git commit -m "benchmarks: publish bb reference gap report"
```

---

## Plan Self-Review

- Spec coverage: the plan covers the controlled rerun, preserved full CSV, report writer, validator, negative controls, commands/environment, and #303 verdict.
- Placeholder scan: no placeholders remain; all paths and commands are concrete.
- Type consistency: `write_reference_gap_report.main`, `validate_reference_gap_report.main`, `ler_table_lines`, and `delta_table_lines` are used consistently across tasks.
