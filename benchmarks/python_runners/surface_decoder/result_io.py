from __future__ import annotations

import json
from typing import Iterable, TextIO


def write_results_jsonl(rows: Iterable[dict[str, object]], handle: TextIO) -> None:
    for row in rows:
        handle.write(json.dumps(row))
        handle.write("\n")


def read_results_jsonl(handle: TextIO) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for line in handle:
        line = line.strip()
        if not line:
            continue
        rows.append(json.loads(line))
    return rows
