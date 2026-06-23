#!/usr/bin/env python3
"""Generate pinned P=96 APM sparse-row fixtures from table_a1_manifest.json.

Regenerate:
    python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py

Check committed fixtures:
    python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py --check
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
MANIFEST_PATH = SCRIPT_DIR / "table_a1_manifest.json"
HX_PATH = SCRIPT_DIR / "p96_hx.json"
HZ_PATH = SCRIPT_DIR / "p96_hz.json"


def affine_inverse(a: int, b: int, modulus: int) -> tuple[int, int]:
    for candidate in range(modulus):
        if (a * candidate) % modulus == 1:
            return candidate, (-candidate * b) % modulus
    raise ValueError(f"{a} is not invertible modulo {modulus}")


def apply_affine(coefficients: tuple[int, int], value: int, modulus: int) -> int:
    a, b = coefficients
    return (a * value + b) % modulus


def affine_family(entry: dict[str, object], key: str) -> list[tuple[int, int]]:
    family = entry[key]
    if not isinstance(family, list):
        raise ValueError(f"{key} must be an array")
    if len(family) != 6:
        raise ValueError(f"{key} must contain 6 affine maps")
    coefficients: list[tuple[int, int]] = []
    for expected_index, item in enumerate(family):
        if not isinstance(item, dict):
            raise ValueError(f"{key}[{expected_index}] must be an object")
        if item.get("i") != expected_index:
            raise ValueError(f"{key}[{expected_index}].i must be {expected_index}")
        if key == "f":
            coefficients.append((int(item["a"]), int(item["b"])))
        else:
            coefficients.append((int(item["c"]), int(item["d"])))
    return coefficients


def p96_entry() -> dict[str, object]:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    for entry in manifest["entries"]:
        if entry["code_id"] == "apm_kasai:p=96":
            if entry["P"] != 96 or entry["J"] != 3 or entry["L"] != 12 or entry["L2"] != 6:
                raise ValueError("apm_kasai:p=96 manifest dimensions changed")
            shape = entry["expected_code_shape"]
            if shape["n"] != 1152 or shape["mx"] != 288 or shape["mz"] != 288 or shape["k"] != 580:
                raise ValueError("apm_kasai:p=96 expected shape changed")
            return entry
    raise ValueError("missing apm_kasai:p=96 entry")


def build_hx_rows(p: int, f: list[tuple[int, int]], g: list[tuple[int, int]]) -> list[list[int]]:
    rows: list[list[int]] = []
    for block_row in range(3):
        for local_row in range(p):
            row: list[int] = []
            for block_col in range(12):
                family = f if block_col < 6 else g
                family_index = (block_col % 6 - block_row) % 6
                local_col = apply_affine(family[family_index], local_row, p)
                row.append(block_col * p + local_col)
            rows.append(sorted(row))
    return rows


def build_hz_rows(p: int, f: list[tuple[int, int]], g: list[tuple[int, int]]) -> list[list[int]]:
    rows: list[list[int]] = []
    inverse_f = [affine_inverse(a, b, p) for a, b in f]
    inverse_g = [affine_inverse(a, b, p) for a, b in g]
    for block_row in range(3):
        for local_row in range(p):
            row: list[int] = []
            for block_col in range(12):
                family = inverse_g if block_col < 6 else inverse_f
                family_index = (block_row - (block_col % 6)) % 6
                local_col = apply_affine(family[family_index], local_row, p)
                row.append(block_col * p + local_col)
            rows.append(sorted(row))
    return rows


def sparse_rows_json(num_cols: int, rows: list[list[int]]) -> str:
    return json.dumps(
        {"format": "sparse_rows", "num_cols": num_cols, "rows": rows},
        separators=(",", ":"),
    ) + "\n"


def generated_texts() -> tuple[str, str]:
    entry = p96_entry()
    p = int(entry["P"])
    num_cols = int(entry["expected_code_shape"]["n"])
    f = affine_family(entry, "f")
    g = affine_family(entry, "g")
    return (
        sparse_rows_json(num_cols, build_hx_rows(p, f, g)),
        sparse_rows_json(num_cols, build_hz_rows(p, f, g)),
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if committed fixtures differ")
    args = parser.parse_args()

    hx_text, hz_text = generated_texts()
    if args.check:
        mismatches = []
        if not HX_PATH.exists() or HX_PATH.read_text(encoding="utf-8") != hx_text:
            mismatches.append(str(HX_PATH))
        if not HZ_PATH.exists() or HZ_PATH.read_text(encoding="utf-8") != hz_text:
            mismatches.append(str(HZ_PATH))
        if mismatches:
            raise SystemExit("stale generated fixtures: " + ", ".join(mismatches))
        return

    HX_PATH.write_text(hx_text, encoding="utf-8")
    HZ_PATH.write_text(hz_text, encoding="utf-8")


if __name__ == "__main__":
    main()
