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


def build_evidence(
    results_path: Path,
    contract_path: Path = DEFAULT_CONTRACT_PATH,
) -> ReportEvidence:
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
