from __future__ import annotations

import argparse
import csv
import hashlib
import json
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Sequence

from benchmarks.bb_circuit_bposd_compare import ready_for_full

SNAPSHOT_PREFIX = "rstim-bb-readiness-snapshot:"


def _artifact_hash(path: Path) -> str:
    if not path.exists() or not path.is_file():
        return "missing"
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _read_csv_rows(path: Path) -> list[dict[str, str]]:
    if not path.exists():
        return []
    try:
        with path.open(newline="") as handle:
            return [dict(row) for row in csv.DictReader(handle)]
    except (OSError, UnicodeDecodeError, csv.Error):
        return []


def _read_json_object(path: Path) -> dict[str, object]:
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def _stringify(value: object) -> str:
    return "" if value is None else str(value)


def _select_rows(
    rows: Sequence[dict[str, str]], columns: Sequence[str]
) -> list[dict[str, str]]:
    return [
        {column: _stringify(row.get(column, "")) for column in columns}
        for row in rows
    ]


def _select_mapping(
    mapping: dict[str, object], keys: Sequence[str]
) -> dict[str, str]:
    return {key: _stringify(mapping.get(key, "")) for key in keys}


def _catalog_summary(rows: Sequence[dict[str, str]]) -> list[dict[str, str]]:
    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        grouped[row.get("code_id", "")].append(row)

    summary: list[dict[str, str]] = []
    def _sort_value(value: str) -> tuple[int, str]:
        try:
            return 0, f"{float(value):020.12f}"
        except ValueError:
            return 1, value

    for code_id in sorted(grouped):
        group = grouped[code_id]
        cycles = sorted({row.get("num_cycles", "") for row in group}, key=_sort_value)
        p_values = sorted({row.get("p", "") for row in group}, key=_sort_value)
        status = sorted({row.get("catalog_status", "") for row in group}, key=lambda value: value)
        summary.append(
            {
                "code_id": code_id,
                "cycles": ", ".join(cycles),
                "case_count": str(len(group)),
                "p_values": ", ".join(p_values),
                "catalog_status": ", ".join(status),
            }
        )
    return summary


def build_report_model(results_dir: Path) -> dict[str, object]:
    checks = ready_for_full.check_results_dir(results_dir)
    verdict = ready_for_full.readiness_verdict(checks)
    gate_checks = [
        {
            "name": check.name,
            "status": check.status,
            "artifact": check.artifact,
            "messages": list(check.messages),
        }
        for check in checks
    ]

    hard_replay_path = results_dir / ready_for_full.SEMANTIC_REPLAY_PATH
    hard_profile_path = results_dir / ready_for_full.HARD_PROFILE_PATH
    setup_run_path = results_dir / ready_for_full.SETUP_RUN_PATH
    catalog_path = results_dir / ready_for_full.CATALOG_PATH
    diagnostic_path = results_dir / ready_for_full.DIAGNOSTIC_PATH

    semantic_rows = _select_rows(
        _read_csv_rows(hard_replay_path),
        (
            "case_id",
            "decoder_impl",
            "status",
            "basis",
            "syndrome_weight",
            "logical_prediction",
            "expected_logical",
            "setup_seconds",
            "decode_seconds",
            "run_seconds",
            "logical_error_rate",
        ),
    )
    hard_profile = _select_mapping(
        _read_json_object(hard_profile_path),
        (
            "case_id",
            "basis",
            "osd_planner",
            "osd_order",
            "candidate_limit",
            "planned_candidate_count",
            "ldpc_cs_candidate_bound",
            "osd_candidate_count",
            "bp_iteration_count",
            "osd_use_count",
            "decode_call_count",
            "z_decode_call_count",
            "x_decode_call_count",
            "gf2_solve_count",
            "gf2_full_elimination_count",
            "decode_seconds",
            "bp_seconds",
            "osd_seconds",
        ),
    )
    setup_run = _select_mapping(
        _read_json_object(setup_run_path),
        (
            "code_id",
            "num_trials",
            "sample_count",
            "code_build_count",
            "syndrome_cycle_build_count",
            "effective_model_build_count",
            "decoder_build_count",
            "decode_call_count",
            "z_decode_call_count",
            "x_decode_call_count",
            "setup_seconds",
            "sample_seconds",
            "decode_seconds",
        ),
    )
    diagnostic_rows = _select_rows(
        _read_csv_rows(diagnostic_path),
        (
            "case_id",
            "decoder_impl",
            "status",
            "code_id",
            "p",
            "num_cycles",
            "num_trials",
            "setup_seconds",
            "decode_seconds",
            "run_seconds",
            "logical_error_rate",
            "decode_call_count",
            "bp_iteration_count",
            "osd_use_count",
            "osd_candidate_count",
            "gf2_solve_count",
            "gf2_full_elimination_count",
        ),
    )
    catalog_summary = _catalog_summary(_read_csv_rows(catalog_path))

    return {
        "schema_version": 1,
        "results_dir": str(results_dir),
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "verdict": verdict,
        "gate_checks": gate_checks,
        "artifact_hashes": {
            "semantic-replay": _artifact_hash(hard_replay_path),
            "hard-profile": _artifact_hash(hard_profile_path),
            "setup-run-separation": _artifact_hash(setup_run_path),
            "catalog-coverage": _artifact_hash(catalog_path),
            "diagnostic-compare": _artifact_hash(diagnostic_path),
        },
        "sections": {
            "semantic-replay": semantic_rows,
            "hard-profile": hard_profile,
            "setup-run-separation": setup_run,
            "diagnostic-compare": diagnostic_rows,
            "catalog-coverage": catalog_summary,
        },
    }


def snapshot_model(model: dict[str, object]) -> dict[str, object]:
    snapshot = dict(model)
    snapshot.pop("generated_at", None)
    snapshot.pop("results_dir", None)
    return json.loads(json.dumps(snapshot, sort_keys=True))


def _escape_markdown_cell(value: object) -> str:
    text = _stringify(value).replace("\r\n", "\n").replace("\r", "\n")
    text = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    text = text.replace("|", r"\|")
    return text.replace("\n", "<br>")


def _markdown_table(headers: Sequence[str], rows: Sequence[dict[str, str]]) -> list[str]:
    lines = [
        "| " + " | ".join(_escape_markdown_cell(header) for header in headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append(
            "| "
            + " | ".join(_escape_markdown_cell(row.get(header, "")) for header in headers)
            + " |"
        )
    return lines


def _gate_rows(model: dict[str, object]) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for check in model["gate_checks"]:  # type: ignore[index]
        rows.append(
            {
                "check": _stringify(check.get("name", "")),
                "status": _stringify(check.get("status", "")),
                "artifact": _stringify(check.get("artifact", "")),
                "messages": "; ".join(_stringify(message) for message in check.get("messages", [])),
            }
        )
    return rows


def _key_value_table(section: dict[str, str]) -> list[str]:
    return _markdown_table(["key", "value"], [{"key": key, "value": value} for key, value in section.items()])


def _semantic_columns() -> list[str]:
    return [
        "case_id",
        "decoder_impl",
        "status",
        "basis",
        "syndrome_weight",
        "logical_prediction",
        "expected_logical",
        "setup_seconds",
        "decode_seconds",
        "run_seconds",
        "logical_error_rate",
    ]


def _diagnostic_columns() -> list[str]:
    return [
        "case_id",
        "decoder_impl",
        "status",
        "code_id",
        "p",
        "num_cycles",
        "num_trials",
        "setup_seconds",
        "decode_seconds",
        "run_seconds",
        "logical_error_rate",
        "decode_call_count",
        "bp_iteration_count",
        "osd_use_count",
        "osd_candidate_count",
        "gf2_solve_count",
        "gf2_full_elimination_count",
    ]


def render_markdown(model: dict[str, object]) -> str:
    lines = [
        "# BB BP-OSD Full-Campaign Readiness Report",
        "",
        f"**Source results directory:** {model['results_dir']}",
        f"**Generated at:** {model['generated_at']}",
        f"**Final readiness verdict:** {model['verdict']}",
        "",
        "## Gate Summary",
        "",
    ]
    lines.extend(
        _markdown_table(["check", "status", "artifact", "messages"], _gate_rows(model))
    )
    lines.extend(["", "## Semantic Parity Replay", ""])
    lines.extend(
        _markdown_table(_semantic_columns(), model["sections"]["semantic-replay"])  # type: ignore[index]
    )
    lines.extend(["", "## BB90 Hard-Profile Counters", ""])
    lines.extend(_key_value_table(model["sections"]["hard-profile"]))  # type: ignore[index]
    lines.extend(["", "## Setup/Run Split Evidence", ""])
    lines.extend(
        _key_value_table(model["sections"]["setup-run-separation"])  # type: ignore[index]
    )
    lines.extend(["", "## Diagnostic Rust/Python Compare Rows", ""])
    lines.extend(
        _markdown_table(
            _diagnostic_columns(), model["sections"]["diagnostic-compare"]  # type: ignore[index]
        )
    )
    lines.extend(["", "## Small-LDPC Case Coverage", ""])
    lines.extend(
        _markdown_table(
            ["code_id", "cycles", "case_count", "p_values", "catalog_status"],
            model["sections"]["catalog-coverage"],  # type: ignore[index]
        )
    )
    snapshot = snapshot_model(model)
    lines.extend(
        [
            "",
            f"<!-- {SNAPSHOT_PREFIX} {json.dumps(snapshot, sort_keys=True, separators=(',', ':'))} -->",
            "",
        ]
    )
    return "\n".join(lines)


def write_report(results_dir: Path, out_path: Path) -> dict[str, object]:
    model = build_report_model(results_dir)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(render_markdown(model))
    return model


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    write_report(args.results_dir, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
