from __future__ import annotations

from pathlib import Path


SUMMARY_COLUMNS = (
    "case_id",
    "decoder_impl",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
)


def write_summary(rows: list[dict[str, str]], out_path: Path) -> None:
    ok_rows = [row for row in rows if row.get("status") == "ok"]
    lines = [
        "| " + " | ".join(SUMMARY_COLUMNS) + " |",
        "| " + " | ".join("---" for _ in SUMMARY_COLUMNS) + " |",
    ]
    for row in ok_rows:
        lines.append(
            "| "
            + " | ".join(row.get(column, "") for column in SUMMARY_COLUMNS)
            + " |"
        )
    out_path.write_text("\n".join(lines) + "\n")
