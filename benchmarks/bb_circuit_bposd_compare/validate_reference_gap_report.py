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
    contract_commit_line = f"- Bravyi contract commit: `{UPSTREAM_COMMIT}`"
    if contract_commit_line not in text:
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
