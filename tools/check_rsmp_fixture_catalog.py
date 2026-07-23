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

EXPECTED_KNOWN_ANSWERS = {
    "known_mpad_multi": {
        "circuit_path": "rstim/tests/fixtures/rsmp/known_mpad_multi.stim",
        "circuit_sha256": "c3693c4dc2b4ff09658be810a2547ee0567da0446af148407fda707eb4a5244f",
        "shots": 4,
        "shape": (3, 2, 1, 2),
        "measurement_input": {
            "path": "rstim/tests/fixtures/rsmp/known_mpad_multi.measurements.b8",
            "sha256": "9905474f01a1dc34e8c3db6657a297b6a3f69ee81e7bec97df171f79231aad1c",
        },
        "expected_files": {
            "measurements_b8": ("rstim/tests/fixtures/rsmp/known_mpad_multi.measurements.b8", 3, "9905474f01a1dc34e8c3db6657a297b6a3f69ee81e7bec97df171f79231aad1c"),
            "detectors_b8": ("rstim/tests/fixtures/rsmp/known_mpad_multi.detectors.b8", 2, "054edec1d0211f624fed0cbca9d4f9400b0e491c43742af2c5b0abebf0c990d8"),
            "observables_b8": ("rstim/tests/fixtures/rsmp/known_mpad_multi.observables.b8", 1, "d5e2d2ac07b741be58f6b9e50ede5fdcf16f3e8053ecef9350e7744b0d8bd90c"),
        },
        "stim_cross_check": {
            "stim_version": "1.15.0",
            "working_directory": "rstim/tests/fixtures/rsmp",
            "command": "stim m2d --circuit known_mpad_multi.stim --in known_mpad_multi.measurements.b8 --in_format b8 --out known_mpad_multi.detectors.check.b8 --out_format b8 --obs_out known_mpad_multi.observables.check.b8 --obs_out_format b8",
        },
    },
    "known_mpp_multi_product": {
        "circuit_path": "rstim/tests/fixtures/rsmp/known_mpp_multi_product.stim",
        "circuit_sha256": "e7fb467b2532098ac108259f14d8c51261ad712446501ea8970918ecf5d87175",
        "shots": 4,
        "shape": (3, 3, 1, 3),
        "measurement_input": {
            "path": "rstim/tests/fixtures/rsmp/known_mpp_multi_product.measurements.b8",
            "sha256": "ee09af6a127a747a1411a19a3a2366aafa80d005ea9f0cb22835284674405196",
        },
        "expected_files": {
            "measurements_b8": ("rstim/tests/fixtures/rsmp/known_mpp_multi_product.measurements.b8", 3, "ee09af6a127a747a1411a19a3a2366aafa80d005ea9f0cb22835284674405196"),
            "detectors_b8": ("rstim/tests/fixtures/rsmp/known_mpp_multi_product.detectors.b8", 3, "ee09af6a127a747a1411a19a3a2366aafa80d005ea9f0cb22835284674405196"),
            "observables_b8": ("rstim/tests/fixtures/rsmp/known_mpp_multi_product.observables.b8", 1, "d5e2d2ac07b741be58f6b9e50ede5fdcf16f3e8053ecef9350e7744b0d8bd90c"),
        },
        "stim_cross_check": {
            "stim_version": "1.15.0",
            "working_directory": "rstim/tests/fixtures/rsmp",
            "command": "stim m2d --circuit known_mpp_multi_product.stim --in known_mpp_multi_product.measurements.b8 --in_format b8 --out known_mpp_multi_product.detectors.check.b8 --out_format b8 --obs_out known_mpp_multi_product.observables.check.b8 --obs_out_format b8",
        },
    },
    "known_heralded_erase": {
        "circuit_path": "rstim/tests/fixtures/rsmp/known_heralded_erase.stim",
        "circuit_sha256": "d1c81a073865448121e7a9365ec441a28d484a016b80f1f6e0a5ec01009af34e",
        "shots": 4,
        "shape": (1, 1, 1, 1),
        "measurement_input": {
            "path": "rstim/tests/fixtures/rsmp/known_heralded_erase.measurements.b8",
            "sha256": "d5e2d2ac07b741be58f6b9e50ede5fdcf16f3e8053ecef9350e7744b0d8bd90c",
        },
        "expected_files": {
            "measurements_b8": ("rstim/tests/fixtures/rsmp/known_heralded_erase.measurements.b8", 1, "d5e2d2ac07b741be58f6b9e50ede5fdcf16f3e8053ecef9350e7744b0d8bd90c"),
            "detectors_b8": ("rstim/tests/fixtures/rsmp/known_heralded_erase.detectors.b8", 1, "d5e2d2ac07b741be58f6b9e50ede5fdcf16f3e8053ecef9350e7744b0d8bd90c"),
            "observables_b8": ("rstim/tests/fixtures/rsmp/known_heralded_erase.observables.b8", 1, "d5e2d2ac07b741be58f6b9e50ede5fdcf16f3e8053ecef9350e7744b0d8bd90c"),
        },
        "stim_cross_check": {
            "stim_version": "1.15.0",
            "working_directory": "rstim/tests/fixtures/rsmp",
            "command": "stim m2d --circuit known_heralded_erase.stim --in known_heralded_erase.measurements.b8 --in_format b8 --out known_heralded_erase.detectors.check.b8 --out_format b8 --obs_out known_heralded_erase.observables.check.b8 --obs_out_format b8",
        },
    },
    "known_heralded_pauli_channel_1": {
        "circuit_path": "rstim/tests/fixtures/rsmp/known_heralded_pauli_channel_1.stim",
        "circuit_sha256": "e36735f28ec4703ae95c6ea2429a469b326160b20c1b891c0eeab645b1a2687a",
        "shots": 4,
        "shape": (1, 1, 1, 1),
        "measurement_input": {
            "path": "rstim/tests/fixtures/rsmp/known_heralded_pauli_channel_1.measurements.b8",
            "sha256": "76cc5805dab9b4eacefdb477f498020fd82bccdbc9c6a2d9ce10586ac85512b4",
        },
        "expected_files": {
            "measurements_b8": ("rstim/tests/fixtures/rsmp/known_heralded_pauli_channel_1.measurements.b8", 1, "76cc5805dab9b4eacefdb477f498020fd82bccdbc9c6a2d9ce10586ac85512b4"),
            "detectors_b8": ("rstim/tests/fixtures/rsmp/known_heralded_pauli_channel_1.detectors.b8", 1, "76cc5805dab9b4eacefdb477f498020fd82bccdbc9c6a2d9ce10586ac85512b4"),
            "observables_b8": ("rstim/tests/fixtures/rsmp/known_heralded_pauli_channel_1.observables.b8", 1, "76cc5805dab9b4eacefdb477f498020fd82bccdbc9c6a2d9ce10586ac85512b4"),
        },
        "stim_cross_check": {
            "stim_version": "1.15.0",
            "working_directory": "rstim/tests/fixtures/rsmp",
            "command": "stim m2d --circuit known_heralded_pauli_channel_1.stim --in known_heralded_pauli_channel_1.measurements.b8 --in_format b8 --out known_heralded_pauli_channel_1.detectors.check.b8 --out_format b8 --obs_out known_heralded_pauli_channel_1.observables.check.b8 --obs_out_format b8",
        },
    },
}
EXPECTED_BENCHMARK_GENERATION = {
    "command": "stim sample --shots 1024 --seed 2 --out_format b8 --in benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
    "sha256": "3af9666507a0a73f14c5659f4814d6b47752aa455f9ceb00774d1495ee6c72a6",
}
EXPECTED_GENERATION_EVIDENCE = {
    "loss_visible_measurements": {
        "command": "cargo run -q -p rstim --bin rstim -- sample --shots 4 --seed 2 --out_format b8 --in rstim/tests/fixtures/rsmp/loss_visible_measurements.stim",
        "sha256": "df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119",
    },
    "surface_d11_r100": EXPECTED_BENCHMARK_GENERATION,
}
REQUIRED_RECIPES = {
    "bad_magic": ("nonzero_reference", "set(global.magic, 0x52534d00)", "RSMP_BAD_MAGIC", [], "global header magic"),
    "unsupported_version": ("nonzero_reference", "set(global.format_major, 2)", "RSMP_UNSUPPORTED_VERSION", [], "global version policy"),
    "unknown_required_feature": ("nonzero_reference", "set(global.required_flags, unknown_required_feature)", "RSMP_UNSUPPORTED_FEATURE", [], "required feature policy"),
    "circuit_mismatch": ("nonzero_reference", "control(different_circuit)", "RSMP_CIRCUIT_MISMATCH", [], "circuit identity"),
    "truncated_header": ("nonzero_reference", "truncate(global_header)", "RSMP_TRUNCATED", [], "global header decode"),
    "truncated_block": ("surface_d11_r100", "truncate(block)", "RSMP_TRUNCATED", [], "block decode"),
    "truncated_zstd_frame": ("surface_d11_r100", "truncate(block.zstd_frame)", "RSMP_TRUNCATED", [], "compressed frame decode"),
    "zstd_decode_failure": ("surface_d11_r100", "set(block.zstd_frame.payload, invalid_zstandard_frame)", "RSMP_DECOMPRESSION_FAILED", ["trailer.archive_sha256"], "compressed frame decode"),
    "truncated_trailer": ("surface_d11_r100", "truncate(trailer)", "RSMP_TRUNCATED", [], "trailer decode"),
    "overlong_varint": ("surface_d11_r100", "set(block.sparse_syndrome_payload.hit_count_uleb128, overlong_encoding(1))", "RSMP_MALFORMED_ARCHIVE", ["block.syndrome_uncompressed_len", "block.syndrome_compressed_len", "block.syndrome_zstd_frame.checksum", "trailer.archive_sha256"], "canonical integer decode"),
    "sparse_index_out_of_range": ("surface_d11_r100", "set(block.sparse_syndrome_payload.detector_index_delta, detector_count)", "RSMP_MALFORMED_ARCHIVE", ["block.syndrome_uncompressed_len", "block.syndrome_compressed_len", "block.syndrome_zstd_frame.checksum", "trailer.archive_sha256"], "sparse syndrome index validation"),
    "duplicate_block": ("surface_d11_r100", "duplicate(block)", "RSMP_MALFORMED_ARCHIVE", ["trailer.block_count", "trailer.archive_sha256"], "block ordering validation"),
    "omitted_block": ("surface_d11_r100", "omit(block)", "RSMP_MALFORMED_ARCHIVE", ["trailer.block_count", "trailer.archive_sha256"], "block coverage validation"),
    "reordered_blocks": ("surface_d11_r100", "reorder(blocks)", "RSMP_MALFORMED_ARCHIVE", ["trailer.archive_sha256"], "block sequence validation"),
    "changed_compressed_payload": ("surface_d11_r100", "flip(block.zstd_frame.payload.bit)", "RSMP_DECOMPRESSION_FAILED", ["trailer.archive_sha256"], "compressed frame checksum"),
    "checksum_mismatch": ("surface_d11_r100", "set(trailer.archive_sha256, alternate_digest)", "RSMP_CHECKSUM_MISMATCH", [], "archive checksum"),
    "logical_payload_mismatch": ("surface_d11_r100", "flip(block.canonical_logical_payload.free_bits.bit)", "RSMP_LOGICAL_DIGEST_MISMATCH", ["block.free_compressed_len", "block.free_zstd_frame.checksum", "trailer.archive_sha256"], "logical payload digest"),
    "declared_length_mismatch": ("surface_d11_r100", "set(block.syndrome_uncompressed_len, 0)", "RSMP_MALFORMED_ARCHIVE", ["trailer.archive_sha256"], "declared length validation"),
    "resource_limit_exceeded": ("surface_d11_r100", "limit(max_archive_bytes, global_header_plus_trailer)", "RSMP_LIMIT_EXCEEDED", [], "resource limit validation"),
    "nonzero_padding": ("surface_d11_r100", "set(block.syndrome_padding_bits, 1)", "RSMP_MALFORMED_ARCHIVE", ["block.syndrome_compressed_len", "block.syndrome_zstd_frame.checksum", "trailer.archive_sha256"], "zero padding validation"),
    "unknown_syndrome_codec": ("surface_d11_r100", "set(block.syndrome_codec_id, 99)", "RSMP_MALFORMED_ARCHIVE", ["trailer.archive_sha256"], "syndrome codec dispatch"),
    "trailing_data": ("surface_d11_r100", "append_trailing_byte(0)", "RSMP_TRAILING_DATA", [], "archive end-of-input validation"),
}
RAW_OFFSET_SELECTOR = re.compile(r"byte_offset|offset\s*\(|@|\[\s*\d+\s*\]", re.IGNORECASE)
SHA256_HEX = re.compile(r"[0-9a-f]{64}\Z")
GLOBAL_MUTATION_CODES = {
    "global.magic": "RSMP_BAD_MAGIC",
    "global.format_major": "RSMP_UNSUPPORTED_VERSION",
    "global.format_minor": "RSMP_UNSUPPORTED_VERSION",
    "global.required_flags": "RSMP_UNSUPPORTED_FEATURE",
    "global.header_len": "RSMP_MALFORMED_ARCHIVE",
    "global.optional_flags": "RSMP_MALFORMED_ARCHIVE",
    "global.reserved_flags": "RSMP_MALFORMED_ARCHIVE",
    "global.canonicalization_id": "RSMP_UNSUPPORTED_FEATURE",
    "global.fingerprint_id": "RSMP_UNSUPPORTED_FEATURE",
    "global.transform_id": "RSMP_UNSUPPORTED_FEATURE",
    "global.reference_id": "RSMP_UNSUPPORTED_FEATURE",
    "global.codec_suite_id": "RSMP_UNSUPPORTED_FEATURE",
    "global.reserved0": "RSMP_MALFORMED_ARCHIVE",
    "global.max_shots_per_block": "RSMP_LIMIT_EXCEEDED",
    "global.measurement_count": "RSMP_SHAPE_MISMATCH",
    "global.detector_count": "RSMP_SHAPE_MISMATCH",
    "global.observable_count": "RSMP_SHAPE_MISMATCH",
    "global.detector_rank": "RSMP_SHAPE_MISMATCH",
    "global.total_shots": "RSMP_SHAPE_MISMATCH",
    "global.circuit_sha256": "RSMP_CIRCUIT_MISMATCH",
    "global.header_sha256": "RSMP_CHECKSUM_MISMATCH",
}
BLOCK_MUTATION_CODES = {
    "block.magic": "RSMP_BAD_MAGIC",
    "block.format_major": "RSMP_UNSUPPORTED_VERSION",
    "block.format_minor": "RSMP_UNSUPPORTED_VERSION",
    "block.block_index": "RSMP_MALFORMED_ARCHIVE",
    "block.first_shot": "RSMP_MALFORMED_ARCHIVE",
    "block.shot_count": "RSMP_MALFORMED_ARCHIVE",
    "block.syndrome_codec_id": "RSMP_MALFORMED_ARCHIVE",
    "block.free_codec_id": "RSMP_MALFORMED_ARCHIVE",
    "block.reserved0": "RSMP_MALFORMED_ARCHIVE",
    "block.syndrome_uncompressed_len": "RSMP_MALFORMED_ARCHIVE",
    "block.syndrome_compressed_len": "RSMP_MALFORMED_ARCHIVE",
    "block.free_uncompressed_len": "RSMP_MALFORMED_ARCHIVE",
    "block.free_compressed_len": "RSMP_MALFORMED_ARCHIVE",
    "block.logical_payload_sha256": "RSMP_LOGICAL_DIGEST_MISMATCH",
    "block.zstd_frame.payload": "RSMP_DECOMPRESSION_FAILED",
    "block.zstd_frame.payload.bit": "RSMP_DECOMPRESSION_FAILED",
    "block.zstd_frame.checksum": "RSMP_DECOMPRESSION_FAILED",
    "block.syndrome_zstd_frame.checksum": "RSMP_DECOMPRESSION_FAILED",
    "block.free_zstd_frame.checksum": "RSMP_DECOMPRESSION_FAILED",
    "block.sparse_syndrome_payload.hit_count_uleb128": "RSMP_MALFORMED_ARCHIVE",
    "block.sparse_syndrome_payload.detector_index_delta": "RSMP_MALFORMED_ARCHIVE",
    "block.syndrome_padding_bits": "RSMP_MALFORMED_ARCHIVE",
    "block.free_padding_bits": "RSMP_MALFORMED_ARCHIVE",
    "block.canonical_logical_payload.syndrome_bits.bit": "RSMP_LOGICAL_DIGEST_MISMATCH",
    "block.canonical_logical_payload.free_bits.bit": "RSMP_LOGICAL_DIGEST_MISMATCH",
}
TRAILER_MUTATION_CODES = {
    "trailer.magic": "RSMP_BAD_MAGIC",
    "trailer.format_major": "RSMP_UNSUPPORTED_VERSION",
    "trailer.format_minor": "RSMP_UNSUPPORTED_VERSION",
    "trailer.reserved0": "RSMP_MALFORMED_ARCHIVE",
    "trailer.block_count": "RSMP_MALFORMED_ARCHIVE",
    "trailer.total_shots": "RSMP_MALFORMED_ARCHIVE",
    "trailer.archive_sha256": "RSMP_CHECKSUM_MISMATCH",
}
MUTATION_SELECTOR_CODES = {
    **GLOBAL_MUTATION_CODES,
    **BLOCK_MUTATION_CODES,
    **TRAILER_MUTATION_CODES,
}
TRUNCATE_SELECTORS = {"global_header", "block", "block.zstd_frame", "block.syndrome_zstd_frame", "block.free_zstd_frame", "trailer"}
RECOMPUTE_FIELDS = set(GLOBAL_MUTATION_CODES) | set(TRAILER_MUTATION_CODES) | {
    "block.syndrome_uncompressed_len",
    "block.syndrome_compressed_len",
    "block.free_uncompressed_len",
    "block.free_compressed_len",
    "block.syndrome_zstd_frame.checksum",
    "block.free_zstd_frame.checksum",
    "trailer.block_count",
    "trailer.archive_sha256",
}


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


def require_exact(value: object, expected: object, label: str) -> None:
    if value != expected:
        raise ValueError(f"{label} must be {expected}")


def validate_hash(path: Path, value: object, label: str) -> None:
    digest = require_string(value, label)
    if not SHA256_HEX.fullmatch(digest):
        raise ValueError(f"{label} must be a lowercase SHA-256 hex digest")
    if not path.is_file():
        raise ValueError(f"{label} references a missing committed file")
    if sha256_file(path) != digest:
        raise ValueError(f"{label} does not match committed file")


def validate_sha256_text(value: object, label: str) -> str:
    digest = require_string(value, label)
    if not SHA256_HEX.fullmatch(digest):
        raise ValueError(f"{label} must be a lowercase SHA-256 hex digest")
    return digest


def validate_b8_entry(repo_root: Path, entry: object, shots: int, label: str) -> Path:
    data = require_mapping(entry, label)
    path = repo_path(repo_root, data.get("path"), f"{label}.path")
    if data.get("format") != "b8":
        raise ValueError(f"{label}.format must be b8")
    bit_count = require_nonnegative_int(data.get("bit_count"), f"{label}.bit_count")
    validate_b8(path, shots, bit_count, label)
    validate_hash(path, data.get("sha256"), f"{label}.sha256")
    return path


def validate_measurement_input(repo_root: Path, entry: object, hashes: dict[str, Any], shots: int, measurements: int, label: str) -> None:
    data = require_mapping(entry, label)
    validate_b8_entry(repo_root, data, shots, label)
    if data.get("bit_count") != measurements:
        raise ValueError(f"{label}.bit_count must be measurement_count")
    if hashes.get("measurements_b8_sha256") != data.get("sha256"):
        raise ValueError(f"{label}.sha256 must match hashes.measurements_b8_sha256")


def validate_measurement_generation(generation: object, hashes: dict[str, Any], shots: int, measurements: int, label: str) -> dict[str, Any]:
    data = require_mapping(generation, label)
    require_string(data.get("command"), f"{label}.command")
    if data.get("format") != "b8":
        raise ValueError(f"{label}.format must be b8")
    if data.get("bit_count") != measurements:
        raise ValueError(f"{label}.bit_count must be measurement_count")
    has_output_bytes = "expected_output_bytes" in data
    has_sha = "sha256" in data
    if has_output_bytes != has_sha:
        raise ValueError(f"{label}.expected_output_bytes and {label}.sha256 must be recorded together")
    if has_output_bytes:
        expected_bytes = b8_len(shots, measurements)
        actual_bytes = require_nonnegative_int(data.get("expected_output_bytes"), f"{label}.expected_output_bytes")
        if actual_bytes != expected_bytes:
            raise ValueError(f"{label}.expected_output_bytes must be {expected_bytes}")
        digest = validate_sha256_text(data.get("sha256"), f"{label}.sha256")
        if hashes.get("measurements_b8_sha256") != digest:
            raise ValueError(f"{label}.sha256 must match hashes.measurements_b8_sha256")
    return data


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
    if case_id in REQUIRED_KNOWN_ANSWERS and not known_answer:
        raise ValueError(f"{case_id}.known_answer must be true")
    if known_answer:
        expected_case = EXPECTED_KNOWN_ANSWERS.get(case_id)
        if expected_case is None:
            raise ValueError(f"{case_id}.known_answer is not an approved known-answer case")
        require_exact(data.get("circuit_path"), expected_case["circuit_path"], f"{case_id}.circuit_path")
        require_exact(data.get("circuit_sha256"), expected_case["circuit_sha256"], f"{case_id}.circuit_sha256")
        require_exact(shots, expected_case["shots"], f"{case_id}.shots")
        expected_shape = expected_case["shape"]
        actual_shape = (measurements, detectors, observables, rank)
        for field, actual, expected in zip(
            ("measurement_count", "detector_count", "observable_count", "rank_H"),
            actual_shape,
            expected_shape,
        ):
            if actual != expected:
                raise ValueError(f"{case_id}.{field} must be {expected}")
        measurement_input_data = require_mapping(data.get("measurement_input"), f"{case_id}.measurement_input")
        measurement_input = validate_b8_entry(repo_root, measurement_input_data, shots, f"{case_id}.measurement_input")
        expected_input = require_mapping(expected_case["measurement_input"], f"{case_id}.expected_measurement_input")
        require_exact(measurement_input_data.get("path"), expected_input["path"], f"{case_id}.measurement_input.path")
        require_exact(measurement_input_data.get("bit_count"), measurements, f"{case_id}.measurement_input.bit_count")
        require_exact(measurement_input_data.get("sha256"), expected_input["sha256"], f"{case_id}.measurement_input.sha256")
        expected_files = require_mapping(data.get("expected_files"), f"{case_id}.expected_files")
        expected_bits = {
            "measurements_b8": measurements,
            "detectors_b8": detectors,
            "observables_b8": observables,
        }
        for name, bit_count in expected_bits.items():
            expected_path = validate_b8_entry(repo_root, expected_files.get(name), shots, f"{case_id}.expected_files.{name}")
            entry = require_mapping(expected_files[name], f"{case_id}.expected_files.{name}")
            expected_file_path, expected_file_bits, expected_file_sha = expected_case["expected_files"][name]  # type: ignore[index]
            require_exact(entry.get("path"), expected_file_path, f"{case_id}.expected_files.{name}.path")
            if entry.get("bit_count") != bit_count:
                raise ValueError(f"{case_id}.expected_files.{name}.bit_count must be {bit_count}")
            require_exact(entry.get("bit_count"), expected_file_bits, f"{case_id}.expected_files.{name}.bit_count")
            require_exact(entry.get("sha256"), expected_file_sha, f"{case_id}.expected_files.{name}.sha256")
            hash_field = f"{name}_sha256"
            if hashes.get(hash_field) != entry.get("sha256"):
                raise ValueError(f"{case_id}.hashes.{hash_field} must match expected file hash")
            require_exact(hashes.get(hash_field), expected_file_sha, f"{case_id}.hashes.{hash_field}")
            if name == "measurements_b8" and expected_path != measurement_input:
                raise ValueError(f"{case_id}.measurement_input.path must match expected measurements_b8")
        cross_check = require_mapping(data.get("stim_cross_check"), f"{case_id}.stim_cross_check")
        expected_cross_check = require_mapping(expected_case["stim_cross_check"], f"{case_id}.expected_stim_cross_check")
        for field in ("stim_version", "working_directory", "command"):
            require_exact(cross_check.get(field), expected_cross_check[field], f"{case_id}.stim_cross_check.{field}")
    else:
        has_input = "measurement_input" in data
        has_generation = "measurement_generation" in data
        if has_input == has_generation:
            raise ValueError(f"{case_id}.measurement_input must provide exactly one of measurement_input or measurement_generation")
        if case_id == "surface_d11_r100" and has_input:
            raise ValueError(f"{case_id}.measurement_input must not duplicate benchmark output")
        generation: dict[str, Any] | None = None
        if has_input:
            validate_measurement_input(
                repo_root,
                data.get("measurement_input"),
                hashes,
                shots,
                measurements,
                f"{case_id}.measurement_input",
            )
        else:
            generation = validate_measurement_generation(data.get("measurement_generation"), hashes, shots, measurements, f"{case_id}.measurement_generation")
        if case_id in EXPECTED_GENERATION_EVIDENCE:
            if generation is None:
                raise ValueError(f"{case_id}.measurement_generation must be an object")
            expected_generation = EXPECTED_GENERATION_EVIDENCE[case_id]
            require_exact(generation.get("command"), expected_generation["command"], f"{case_id}.measurement_generation.command")
            require_exact(generation.get("sha256"), expected_generation["sha256"], f"{case_id}.measurement_generation.sha256")
            require_exact(hashes.get("measurements_b8_sha256"), expected_generation["sha256"], f"{case_id}.hashes.measurements_b8_sha256")

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


def expected_code_for_mutation(mutation: str, label: str) -> str:
    if mutation == "control(different_circuit)":
        return "RSMP_CIRCUIT_MISMATCH"
    if mutation == "control(unsupported_sweep)":
        return "RSMP_UNSUPPORTED_SWEEP"
    if mutation == "limit(max_archive_bytes, global_header_plus_trailer)":
        return "RSMP_LIMIT_EXCEEDED"
    if mutation.startswith("append_trailing_byte("):
        return "RSMP_TRAILING_DATA"
    if mutation.startswith("truncate("):
        selector = mutation[len("truncate(") : -1] if mutation.endswith(")") else ""
        if selector in TRUNCATE_SELECTORS:
            return "RSMP_TRUNCATED"
        raise ValueError(f"{label}.mutation references unknown rsmp selector {selector}")
    if mutation in {"duplicate(block)", "omit(block)", "reorder(blocks)"}:
        return "RSMP_MALFORMED_ARCHIVE"
    match = re.fullmatch(r"(set|flip)\(([^,\)]+)(?:,\s*.*)?\)", mutation)
    if match is None:
        raise ValueError(f"{label}.mutation must use a supported semantic operation")
    selector = match.group(2).strip()
    code = MUTATION_SELECTOR_CODES.get(selector)
    if code is None:
        raise ValueError(f"{label}.mutation references unknown rsmp selector {selector}")
    return code


def validate_recompute_fields(recompute: list[str], label: str) -> None:
    for index, item in enumerate(recompute):
        if RAW_OFFSET_SELECTOR.search(item):
            raise ValueError(f"{label}.recompute[{index}] must use symbolic field paths, not raw byte offsets")
        if item not in RECOMPUTE_FIELDS:
            raise ValueError(f"{label}.recompute[{index}] must name a known rsmp field")


def validate_payload_recompute_contract(mutation: str, recompute: list[str], label: str) -> None:
    required: tuple[str, ...] = ()
    if "block.sparse_syndrome_payload" in mutation:
        required = (
            "block.syndrome_uncompressed_len",
            "block.syndrome_compressed_len",
            "block.syndrome_zstd_frame.checksum",
            "trailer.archive_sha256",
        )
    elif "block.syndrome_padding_bits" in mutation:
        required = (
            "block.syndrome_compressed_len",
            "block.syndrome_zstd_frame.checksum",
            "trailer.archive_sha256",
        )
    elif "block.canonical_logical_payload.syndrome_bits" in mutation:
        required = (
            "block.syndrome_compressed_len",
            "block.syndrome_zstd_frame.checksum",
            "trailer.archive_sha256",
        )
    elif "block.canonical_logical_payload.free_bits" in mutation:
        required = (
            "block.free_compressed_len",
            "block.free_zstd_frame.checksum",
            "trailer.archive_sha256",
        )
    for item in required:
        if item not in recompute:
            raise ValueError(f"{label}.recompute must include {item}")


def validate_recipes(recipes: object, known_roles: set[str]) -> None:
    if not isinstance(recipes, list) or len(recipes) < 12:
        raise ValueError("corruption_recipes must contain at least 12 recipes")
    seen_ids: set[str] = set()
    recipe_data: dict[str, dict[str, Any]] = {}
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
        expected_code = expected_code_for_mutation(mutation, label)
        if code != expected_code:
            raise ValueError(f"{label}.expected_code must be {expected_code}")
        recompute = data.get("recompute")
        if not isinstance(recompute, list) or not all(isinstance(item, str) and item for item in recompute):
            raise ValueError(f"{label}.recompute must be a list of non-empty strings")
        validate_recompute_fields(recompute, label)
        validate_payload_recompute_contract(mutation, recompute, label)
        require_string(data.get("validation_boundary"), f"{label}.validation_boundary")
        recipe_data[recipe_id] = data

    for recipe_id, (source_role, mutation, expected_code, recompute, validation_boundary) in REQUIRED_RECIPES.items():
        actual = recipe_data.get(recipe_id)
        if actual is None:
            raise ValueError(f"missing required corruption recipe {recipe_id}")
        if actual.get("source_role") != source_role:
            raise ValueError(f"{recipe_id}.source_role must be {source_role}")
        if actual.get("mutation") != mutation:
            raise ValueError(f"{recipe_id}.mutation must be {mutation}")
        if actual.get("expected_code") != expected_code:
            raise ValueError(f"{recipe_id}.expected_code must be {expected_code}")
        if actual.get("recompute") != recompute:
            raise ValueError(f"{recipe_id}.recompute must be {recompute}")
        if actual.get("validation_boundary") != validation_boundary:
            raise ValueError(f"{recipe_id}.validation_boundary must be {validation_boundary}")


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
