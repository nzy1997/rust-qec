#!/usr/bin/env python3
"""Validate the committed rsmp fixture catalog."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


PASS_LINE = "PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12"
REQUIRED_ROLES = {
    "nonzero_reference",
    "rank_zero",
    "dependent_detectors",
    "repeat_records",
    "observable_recovery",
    "loss_visible_measurements",
    "surface_d11_r100",
}
REQUIRED_KNOWN_ANSWERS = {
    "known_mpad_multi",
    "known_mpp_multi_product",
    "known_heralded_erase",
    "known_heralded_pauli_channel_1",
}
EXPECTED_BENCHMARK_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
ERROR_CODES = {
    "RSMP_BAD_MAGIC",
    "RSMP_UNSUPPORTED_VERSION",
    "RSMP_UNSUPPORTED_FEATURE",
    "RSMP_UNSUPPORTED_SWEEP",
    "RSMP_CIRCUIT_MISMATCH",
    "RSMP_SHAPE_MISMATCH",
    "RSMP_LIMIT_EXCEEDED",
    "RSMP_TRUNCATED",
    "RSMP_MALFORMED_ARCHIVE",
    "RSMP_DECOMPRESSION_FAILED",
    "RSMP_CHECKSUM_MISMATCH",
    "RSMP_LOGICAL_DIGEST_MISMATCH",
    "RSMP_TRAILING_DATA",
    "RSMP_IO",
}

KNOWN_ANSWER_SHAPES = {
    "known_mpad_multi": (3, 2, 1, 2),
    "known_mpp_multi_product": (3, 3, 1, 3),
    "known_heralded_erase": (1, 1, 1, 1),
    "known_heralded_pauli_channel_1": (1, 1, 1, 1),
}
REQUIRED_RECIPES = {
    "bad_magic": "RSMP_BAD_MAGIC",
    "unsupported_version": "RSMP_UNSUPPORTED_VERSION",
    "unknown_required_feature": "RSMP_UNSUPPORTED_FEATURE",
    "circuit_mismatch": "RSMP_CIRCUIT_MISMATCH",
    "truncated_header": "RSMP_TRUNCATED",
    "truncated_block": "RSMP_TRUNCATED",
    "truncated_zstd_frame": "RSMP_TRUNCATED",
    "zstd_decode_failure": "RSMP_DECOMPRESSION_FAILED",
    "truncated_trailer": "RSMP_TRUNCATED",
    "overlong_varint": "RSMP_MALFORMED_ARCHIVE",
    "sparse_index_out_of_range": "RSMP_MALFORMED_ARCHIVE",
    "duplicate_block": "RSMP_MALFORMED_ARCHIVE",
    "omitted_block": "RSMP_MALFORMED_ARCHIVE",
    "reordered_blocks": "RSMP_MALFORMED_ARCHIVE",
    "changed_compressed_payload": "RSMP_CHECKSUM_MISMATCH",
    "checksum_mismatch": "RSMP_CHECKSUM_MISMATCH",
    "logical_payload_mismatch": "RSMP_LOGICAL_DIGEST_MISMATCH",
    "declared_length_mismatch": "RSMP_MALFORMED_ARCHIVE",
    "resource_limit_exceeded": "RSMP_LIMIT_EXCEEDED",
    "nonzero_padding": "RSMP_MALFORMED_ARCHIVE",
    "unknown_syndrome_codec": "RSMP_MALFORMED_ARCHIVE",
    "trailing_data": "RSMP_TRAILING_DATA",
}
RAW_OFFSET_SELECTOR = re.compile(r"byte_offset|offset\s*\(|@|\[\s*\d+\s*\]", re.IGNORECASE)
SHA256_HEX = re.compile(r"[0-9a-f]{64}\Z")


def sha256_file(path: Path) -> str:
    """Return the lowercase SHA-256 hex digest of a committed file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def repo_path(repo_root: Path, value: object, label: str) -> Path:
    """Return repo_root/value after rejecting absolute paths and '..' components."""
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a non-empty repository-relative path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError(f"{label} must be a repository-relative path without '..'")
    return repo_root / relative


def b8_len(shots: int, bit_count: int) -> int:
    """Return the number of bytes for byte-aligned b8 rows."""
    return shots * ((bit_count + 7) // 8)


def validate_b8(path: Path, shots: int, bit_count: int, label: str) -> None:
    """Validate byte length and zero unused final padding bits."""
    if not path.is_file():
        raise ValueError(f"{label}.path does not reference a committed file")
    # Stim b8 stores each sample as a byte-aligned row.
    row_len = b8_len(1, bit_count)
    expected_len = shots * row_len
    data = path.read_bytes()
    if len(data) != expected_len:
        raise ValueError(f"{label}.length must be {expected_len}, got {len(data)}")
    remainder = bit_count % 8
    if remainder:
        padding_mask = ~((1 << remainder) - 1) & 0xFF
        if any(data[index] & padding_mask for index in range(row_len - 1, len(data), row_len)):
            raise ValueError(f"{label}.padding_bits must be zero")


def require_mapping(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def require_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{label} must be a non-empty string")
    return value


def require_nonnegative_int(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{label} must be a non-negative integer")
    return value


def validate_hash(path: Path, value: object, label: str) -> None:
    digest = require_string(value, label)
    if not SHA256_HEX.fullmatch(digest):
        raise ValueError(f"{label} must be a lowercase SHA-256 hex digest")
    if not path.is_file():
        raise ValueError(f"{label} references a missing committed file")
    if sha256_file(path) != digest:
        raise ValueError(f"{label} does not match committed file")


def validate_b8_entry(repo_root: Path, entry: object, shots: int, label: str) -> Path:
    data = require_mapping(entry, label)
    path = repo_path(repo_root, data.get("path"), f"{label}.path")
    if data.get("format") != "b8":
        raise ValueError(f"{label}.format must be b8")
    bit_count = require_nonnegative_int(data.get("bit_count"), f"{label}.bit_count")
    validate_b8(path, shots, bit_count, label)
    validate_hash(path, data.get("sha256"), f"{label}.sha256")
    return path


def validate_case(repo_root: Path, case: object, seen_ids: set[str]) -> tuple[str, set[str]]:
    data = require_mapping(case, "case")
    case_id = require_string(data.get("id"), "case.id")
    if case_id in seen_ids:
        raise ValueError(f"duplicate case id {case_id}")
    seen_ids.add(case_id)

    for field in ("purpose", "provenance"):
        require_string(data.get(field), f"{case_id}.{field}")
    roles = data.get("semantic_roles")
    if not isinstance(roles, list) or not all(isinstance(role, str) and role for role in roles):
        raise ValueError(f"{case_id}.semantic_roles must be a list of non-empty strings")
    if len(set(roles)) != len(roles):
        raise ValueError(f"{case_id}.semantic_roles must not contain duplicates")
    consumers = data.get("consumers")
    if not isinstance(consumers, list) or not consumers or not all(isinstance(item, str) and item for item in consumers):
        raise ValueError(f"{case_id}.consumers must be a non-empty list of strings")

    circuit = repo_path(repo_root, data.get("circuit_path"), f"{case_id}.circuit_path")
    validate_hash(circuit, data.get("circuit_sha256"), f"{case_id}.circuit_sha256")
    hashes = require_mapping(data.get("hashes"), f"{case_id}.hashes")
    if hashes.get("circuit_sha256") != data.get("circuit_sha256"):
        raise ValueError(f"{case_id}.hashes.circuit_sha256 must match circuit_sha256")

    shots = require_nonnegative_int(data.get("shots"), f"{case_id}.shots")
    if shots == 0:
        raise ValueError(f"{case_id}.shots must be positive")
    measurements = require_nonnegative_int(data.get("measurement_count"), f"{case_id}.measurement_count")
    detectors = require_nonnegative_int(data.get("detector_count"), f"{case_id}.detector_count")
    observables = require_nonnegative_int(data.get("observable_count"), f"{case_id}.observable_count")
    rank = require_nonnegative_int(data.get("rank_H"), f"{case_id}.rank_H")
    if rank > min(measurements, detectors):
        raise ValueError(f"{case_id}.rank_H must be at most min(measurement_count, detector_count)")

    if case_id == "surface_d11_r100" and data.get("circuit_path") != EXPECTED_BENCHMARK_PATH:
        raise ValueError(f"{case_id}.circuit_path must reference existing benchmark fixture")

    known_answer = data.get("known_answer", False)
    if not isinstance(known_answer, bool):
        raise ValueError(f"{case_id}.known_answer must be boolean")
    if known_answer:
        expected_shape = KNOWN_ANSWER_SHAPES.get(case_id)
        if expected_shape is None:
            raise ValueError(f"{case_id}.known_answer is not an approved known-answer case")
        actual_shape = (measurements, detectors, observables, rank)
        for field, actual, expected in zip(
            ("measurement_count", "detector_count", "observable_count", "rank_H"),
            actual_shape,
            expected_shape,
        ):
            if actual != expected:
                raise ValueError(f"{case_id}.{field} must be {expected}")
        measurement_input = validate_b8_entry(repo_root, data.get("measurement_input"), shots, f"{case_id}.measurement_input")
        expected_files = require_mapping(data.get("expected_files"), f"{case_id}.expected_files")
        expected_bits = {
            "measurements_b8": measurements,
            "detectors_b8": detectors,
            "observables_b8": observables,
        }
        for name, bit_count in expected_bits.items():
            expected_path = validate_b8_entry(repo_root, expected_files.get(name), shots, f"{case_id}.expected_files.{name}")
            entry = require_mapping(expected_files[name], f"{case_id}.expected_files.{name}")
            if entry.get("bit_count") != bit_count:
                raise ValueError(f"{case_id}.expected_files.{name}.bit_count must be {bit_count}")
            hash_field = f"{name}_sha256"
            if hashes.get(hash_field) != entry.get("sha256"):
                raise ValueError(f"{case_id}.hashes.{hash_field} must match expected file hash")
            if name == "measurements_b8" and expected_path != measurement_input:
                raise ValueError(f"{case_id}.measurement_input.path must match expected measurements_b8")
    else:
        generation = require_mapping(data.get("measurement_generation"), f"{case_id}.measurement_generation")
        require_string(generation.get("command"), f"{case_id}.measurement_generation.command")
        if generation.get("format") != "b8":
            raise ValueError(f"{case_id}.measurement_generation.format must be b8")
        if generation.get("bit_count") != measurements:
            raise ValueError(f"{case_id}.measurement_generation.bit_count must be measurement_count")

    return case_id, set(roles)


def validate_cases(repo_root: Path, cases: object) -> set[str]:
    if not isinstance(cases, list):
        raise ValueError("cases must be a list")
    seen_ids: set[str] = set()
    roles: set[str] = set()
    for case in cases:
        _, case_roles = validate_case(repo_root, case, seen_ids)
        roles.update(case_roles)
    for role in sorted(REQUIRED_ROLES - roles):
        raise ValueError(f"missing semantic role {role}")
    for case_id in sorted(REQUIRED_KNOWN_ANSWERS - seen_ids):
        raise ValueError(f"missing known-answer case {case_id}")
    if "surface_d11_r100" not in seen_ids:
        raise ValueError("missing benchmark case surface_d11_r100")
    return roles


def validate_recipes(recipes: object, known_roles: set[str]) -> None:
    if not isinstance(recipes, list) or len(recipes) < 12:
        raise ValueError("corruption_recipes must contain at least 12 recipes")
    seen_ids: set[str] = set()
    recipe_codes: dict[str, str] = {}
    for recipe in recipes:
        data = require_mapping(recipe, "corruption recipe")
        recipe_id = require_string(data.get("id"), "corruption_recipe.id")
        if recipe_id in seen_ids:
            raise ValueError(f"duplicate corruption recipe id {recipe_id}")
        seen_ids.add(recipe_id)
        label = recipe_id
        source_role = require_string(data.get("source_role"), f"{label}.source_role")
        if source_role not in known_roles:
            raise ValueError(f"{label}.source_role must name a semantic role")
        mutation = require_string(data.get("mutation"), f"{label}.mutation")
        if RAW_OFFSET_SELECTOR.search(mutation):
            raise ValueError(f"{label}.mutation must use symbolic field paths, not raw byte offsets")
        code = require_string(data.get("expected_code"), f"{label}.expected_code")
        if code not in ERROR_CODES:
            raise ValueError(f"{label}.expected_code must be a public RSMP error code")
        recompute = data.get("recompute")
        if not isinstance(recompute, list) or not all(isinstance(item, str) and item for item in recompute):
            raise ValueError(f"{label}.recompute must be a list of non-empty strings")
        require_string(data.get("validation_boundary"), f"{label}.validation_boundary")
        recipe_codes[recipe_id] = code

    for recipe_id, expected_code in REQUIRED_RECIPES.items():
        actual_code = recipe_codes.get(recipe_id)
        if actual_code is None:
            raise ValueError(f"missing required corruption recipe {recipe_id}")
        if actual_code != expected_code:
            raise ValueError(f"{recipe_id}.expected_code must be {expected_code}")


def validate_catalog(repo_root: Path, catalog: object) -> None:
    data = require_mapping(catalog, "catalog")
    if data.get("schema_version") != 1:
        raise ValueError("schema_version must be 1")
    if data.get("format") != "rsmp-fixture-catalog-v1":
        raise ValueError("format must be rsmp-fixture-catalog-v1")
    roles = validate_cases(repo_root, data.get("cases"))
    validate_recipes(data.get("corruption_recipes"), roles)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    default_root = Path(__file__).resolve().parents[1]
    parser.add_argument("--repo-root", type=Path, default=default_root)
    parser.add_argument("--catalog", type=Path)
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    catalog_path = args.catalog if args.catalog is not None else repo_root / "rstim/tests/fixtures/rsmp/catalog.json"
    try:
        with catalog_path.open(encoding="utf-8") as handle:
            validate_catalog(repo_root, json.load(handle))
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
