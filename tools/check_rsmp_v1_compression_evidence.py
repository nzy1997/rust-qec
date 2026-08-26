#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]

RAW_SCHEMA_VERSION = 1
ENVIRONMENT_SCHEMA_VERSION = 1
EVIDENCE_FORMAT = "rsmp-v1-compression-evidence"

REQUIRED_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json", "artifact-sha256.json")
ARTIFACT_FILES = REQUIRED_FILES[:-1]

BENCHMARK_CASE_ID = "stim_surface_d11_r100"
HIGH_ENTROPY_CASE_ID = "high_entropy_control"
BENCHMARK_ROW_INDEX = 6
HIGH_ENTROPY_ROW_INDEX = 7

PINNED_BENCHMARK_RAW_BYTES = 1_552_384
PINNED_BENCHMARK_SHA256 = "a80d7503ee2d06d6b4e04a1c582b32ab89c6dd9c70f9d4e4e1d671f4386f278b"
REQUIRED_STIM_BASELINE_FORMATS = ("b8", "r8", "ptb64")
PINNED_BENCHMARK_SAMPLE_ARGV = [
    "target/release/rstim",
    "sample",
    "--shots",
    "1024",
    "--seed",
    "7",
    "--out_format",
    "b8",
    "--in",
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
]

HIGH_ENTROPY_CIRCUIT_TEXT = "M " + " ".join(str(i) for i in range(1024)) + "\n"
HIGH_ENTROPY_CIRCUIT_SHA256 = "681b3470c0a499c0e54c45bb413619fde5d1c3679b5ec4c8ddce52a58520cdc0"
HIGH_ENTROPY_RAW_BYTES = 1_048_576
HIGH_ENTROPY_RAW_SHA256 = "258c8b47bf6e074c19f90bc0e9b0dbdea38030ec7583804980eedce99b99ccf3"
HIGH_ENTROPY_GENERATOR_ID = "rstim-rsmp-v1-high-entropy-v1"
HIGH_ENTROPY_GENERATOR_SPEC = (
    'SHA256(b"rstim-rsmp-v1-high-entropy-v1:" || little_endian_u64(counter)); '
    "counters start at zero; truncate to 1048576 bytes"
)
HIGH_ENTROPY_GENERATOR_SHA256 = hashlib.sha256(HIGH_ENTROPY_GENERATOR_SPEC.encode("utf-8")).hexdigest()

ZSTD_CONTRACT = {
    "level": 3,
    "single_threaded": True,
    "dictionary": "none",
    "long_distance_matching": False,
    "pledged_source_size": True,
    "frame_content_size": True,
    "frame_checksum": True,
    "threshold_arithmetic": "integer_cross_multiplication",
}

CODEC_NAMES = {
    "empty",
    "syndrome_dense_v1",
    "syndrome_sparse_leb128_v1",
    "free_dense_v1",
}


@dataclass(frozen=True)
class RequiredRow:
    case_id: str
    semantic_role: str
    catalog_case_id: str | None


REQUIRED_ROWS = (
    RequiredRow("nonzero_reference", "nonzero_reference", "nonzero_reference"),
    RequiredRow("rank_zero", "rank_zero", "rank_zero"),
    RequiredRow("dependent_detectors", "dependent_detectors", "dependent_detectors"),
    RequiredRow("repeat_records", "repeat_records", "repeat_records"),
    RequiredRow("observable_recovery", "observable_recovery", "observable_recovery"),
    RequiredRow("loss_visible_measurements", "loss_visible_measurements", "loss_visible_measurements"),
    RequiredRow(BENCHMARK_CASE_ID, "surface_d11_r100", "surface_d11_r100"),
    RequiredRow(HIGH_ENTROPY_CASE_ID, "high_entropy_control", None),
)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json_text(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} is not valid JSON: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def load_raw_records(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ValueError(f"raw.jsonl could not be read: {error}") from error
    records: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if line == "":
            raise ValueError(f"raw.jsonl line {line_number} must not be blank")
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"raw.jsonl line {line_number} is not valid JSON") from error
        if not isinstance(value, dict):
            raise ValueError(f"raw.jsonl line {line_number} must be a JSON object")
        records.append(value)
    return records


def load_catalog_cases_from_path(catalog_path: Path) -> dict[str, dict[str, Any]]:
    catalog = load_json_object(catalog_path, "rsmp fixture catalog")
    cases = catalog.get("cases")
    if not isinstance(cases, list):
        raise ValueError("rsmp fixture catalog cases must be an array")
    by_id: dict[str, dict[str, Any]] = {}
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            raise ValueError("rsmp fixture catalog case must have an id")
        by_id[case["id"]] = case
    return by_id


def load_catalog_cases(repo_root: Path = REPO_ROOT) -> dict[str, dict[str, Any]]:
    return load_catalog_cases_from_path(repo_root / "rstim/tests/fixtures/rsmp/catalog.json")


def load_locked_package_versions(repo_root: Path = REPO_ROOT) -> dict[str, str]:
    with (repo_root / "Cargo.lock").open("rb") as handle:
        lock = tomllib.load(handle)
    packages = lock.get("package")
    if not isinstance(packages, list):
        raise ValueError("Cargo.lock package list is missing")
    versions: dict[str, str] = {}
    for package in packages:
        if not isinstance(package, dict):
            continue
        name = package.get("name")
        version = package.get("version")
        if name in {"zstd", "zstd-safe", "zstd-sys"} and isinstance(version, str):
            versions[name] = version
    return versions


def validate_required_files(results_dir: Path) -> None:
    try:
        entries = sorted(path.name for path in results_dir.iterdir())
    except OSError as error:
        raise ValueError(f"could not read results directory: {error}") from error
    unexpected = sorted(set(entries) - set(REQUIRED_FILES))
    if unexpected:
        raise ValueError(f"unexpected bundle file: {unexpected[0]}")
    for filename in REQUIRED_FILES:
        if not (results_dir / filename).is_file():
            raise ValueError(f"missing required bundle file: {filename}")


def is_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(ch in "0123456789abcdef" for ch in value)


def require_sha256(value: Any, field: str) -> str:
    if not is_sha256(value):
        raise ValueError(f"{field} must be a lowercase SHA-256 digest")
    return value


def require_int(value: Any, field: str, *, minimum: int | None = None) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"{field} must be an integer")
    if minimum is not None and value < minimum:
        raise ValueError(f"{field} must be >= {minimum}")
    return value


def require_bool(value: Any, field: str) -> bool:
    if not isinstance(value, bool):
        raise ValueError(f"{field} must be a boolean")
    return value


def require_object(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{field} must be a JSON object")
    return value


def require_list(value: Any, field: str) -> list[Any]:
    if not isinstance(value, list):
        raise ValueError(f"{field} must be an array")
    return value


def require_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value == "":
        raise ValueError(f"{field} must be a non-empty string")
    return value


def validate_direct_zstd_argv(argv: list[Any], label: str) -> None:
    if any(not isinstance(part, str) for part in argv):
        raise ValueError(f"{label} direct_zstd argv entries must be strings")
    if len(argv) < 4 or argv[1] != "rsmp_zstd_frame":
        raise ValueError(f"{label} direct_zstd argv must invoke rsmp_zstd_frame")
    level_positions = [index for index, part in enumerate(argv) if part == "--level"]
    if len(level_positions) != 1:
        raise ValueError(f"{label} direct_zstd argv must contain exactly one --level")
    level_index = level_positions[0]
    if level_index + 1 >= len(argv) or argv[level_index + 1] != "3":
        raise ValueError(f"{label} direct_zstd argv --level must be 3")


def repo_relative_path(raw: Any, field: str) -> Path:
    value = require_string(raw, field)
    posix = PurePosixPath(value)
    if posix.is_absolute() or "\\" in value or any(part in {"", ".", ".."} for part in value.split("/")):
        raise ValueError(f"{field} must be a repository-relative POSIX path")
    return REPO_ROOT / posix


def require_string_list(value: Any, field: str) -> list[Any]:
    argv = require_list(value, field)
    if any(not isinstance(part, str) for part in argv):
        raise ValueError(f"{field} entries must be strings")
    return argv


def validate_stim_baselines(row: dict[str, Any], label: str) -> None:
    measurement = require_object(row.get("measurement_input"), f"{label} measurement_input")
    measurement_sha256 = require_sha256(measurement.get("sha256"), f"{label} measurement_sha256")
    raw_b8_bytes = require_int(measurement.get("raw_b8_bytes"), f"{label} raw_b8_bytes", minimum=0)
    baselines = require_object(row.get("stim_baselines"), f"{label} stim_baselines")
    unexpected = sorted(set(baselines) - set(REQUIRED_STIM_BASELINE_FORMATS))
    if unexpected:
        raise ValueError(f"{label} unexpected Stim baseline format: {unexpected[0]}")
    for fmt in REQUIRED_STIM_BASELINE_FORMATS:
        if fmt not in baselines:
            raise ValueError(f"missing required Stim baseline format: {fmt}")
        baseline_label = f"{label} stim baseline {fmt}"
        baseline = require_object(baselines[fmt], baseline_label)
        artifact = require_object(baseline.get("artifact"), f"{baseline_label} artifact")
        artifact_bytes = require_int(artifact.get("bytes"), f"{baseline_label} artifact bytes", minimum=1)
        artifact_sha256 = require_sha256(artifact.get("sha256"), f"{baseline_label} artifact sha256")
        require_string_list(artifact.get("argv"), f"{baseline_label} artifact argv")
        direct = require_object(baseline.get("direct_zstd"), f"{baseline_label} direct_zstd")
        if direct.get("input_sha256") != artifact_sha256:
            raise ValueError(f"{baseline_label} direct_zstd input_sha256 must match artifact sha256")
        require_int(direct.get("bytes"), f"{baseline_label} direct_zstd bytes", minimum=1)
        require_sha256(direct.get("sha256"), f"{baseline_label} direct_zstd sha256")
        direct_argv = require_list(direct.get("argv"), f"{baseline_label} direct_zstd argv")
        validate_direct_zstd_argv(direct_argv, baseline_label)
        roundtrip = require_object(baseline.get("roundtrip_b8"), f"{baseline_label} roundtrip_b8")
        require_string_list(roundtrip.get("argv"), f"{baseline_label} roundtrip_b8 argv")
        roundtrip_sha256 = require_sha256(roundtrip.get("sha256"), f"{baseline_label} roundtrip_b8 sha256")
        if roundtrip_sha256 != measurement_sha256:
            raise ValueError(f"{fmt} round-trip measurement SHA-256 mismatch")
        if fmt == "b8":
            if artifact_bytes != raw_b8_bytes:
                raise ValueError(f"{baseline_label} artifact bytes must equal canonical raw b8 bytes")
            if artifact_sha256 != measurement_sha256:
                raise ValueError(f"{baseline_label} artifact must be the canonical measurement bytes")


def expected_b8_bytes(bits_per_shot: int, shots: int) -> int:
    return ((bits_per_shot + 7) // 8) * shots
def expected_blocks(shots: int) -> int:
    return (shots + 4095) // 4096


def measurement_logical_digest(
    case_id: str,
    circuit_sha256: str,
    dimensions: dict[str, Any],
    raw_b8_bytes: int,
    measurement_sha256: str,
) -> str:
    parts = [
        "rstim-rsmp-v1-measurement-logical-digest-v1",
        f"case_id={case_id}",
        f"circuit_sha256={circuit_sha256}",
        f"M={dimensions['M']}",
        f"D={dimensions['D']}",
        f"L={dimensions['L']}",
        f"rank={dimensions['rank']}",
        f"free_width={dimensions['free_width']}",
        f"shots={dimensions['shots']}",
        f"raw_b8_bytes={raw_b8_bytes}",
        f"measurement_sha256={measurement_sha256}",
    ]
    return sha256_bytes(("\n".join(parts) + "\n").encode("utf-8"))


def peak_working_set(row: dict[str, Any]) -> int:
    dimensions = row["dimensions"]
    m = int(dimensions["M"])
    rank = int(dimensions["rank"])
    free_width = int(dimensions["free_width"])
    peak = 0
    for block in row["rsmp_archive"]["blocks"]:
        shots = int(block["shot_count"])
        words_per_row = (shots + 63) // 64
        buffered_input = m * words_per_row * 8
        encoded_selected = rank * words_per_row * 8
        encoded_free = free_width * words_per_row * 8
        raw_buffers = int(block["syndrome_uncompressed_bytes"]) + int(block["free_uncompressed_bytes"])
        compressed_frames = int(block["syndrome_compressed_bytes"]) + int(block["free_compressed_bytes"])
        peak = max(peak, buffered_input + encoded_selected + encoded_free + raw_buffers + compressed_frames + 8 * 1024 * 1024)
    return peak


def validate_records(records: list[dict[str, Any]], repo_root: Path = REPO_ROOT) -> None:
    if len(records) != len(REQUIRED_ROWS):
        raise ValueError(f"raw.jsonl must contain exactly {len(REQUIRED_ROWS)} rows")
    if sha256_bytes(HIGH_ENTROPY_CIRCUIT_TEXT.encode("utf-8")) != HIGH_ENTROPY_CIRCUIT_SHA256:
        raise ValueError("internal high-entropy circuit SHA-256 constant is wrong")
    catalog_cases = load_catalog_cases(repo_root)
    for index, (row, requirement) in enumerate(zip(records, REQUIRED_ROWS, strict=True)):
        label = f"raw row {index} ({requirement.case_id})"
        if row.get("schema_version") != RAW_SCHEMA_VERSION:
            raise ValueError(f"{label} schema_version must be {RAW_SCHEMA_VERSION}")
        if row.get("evidence_format") != EVIDENCE_FORMAT:
            raise ValueError(f"{label} evidence_format must be {EVIDENCE_FORMAT}")
        if row.get("case_id") != requirement.case_id:
            raise ValueError(f"{label} case_id must be {requirement.case_id}")
        if row.get("semantic_role") != requirement.semantic_role:
            raise ValueError(f"{label} semantic_role must be {requirement.semantic_role}")
        if row.get("catalog_case_id") != requirement.catalog_case_id:
            raise ValueError(f"{label} catalog_case_id must be {requirement.catalog_case_id}")

        dimensions = require_object(row.get("dimensions"), f"{label} dimensions")
        m = require_int(dimensions.get("M"), f"{label} dimensions.M", minimum=0)
        d = require_int(dimensions.get("D"), f"{label} dimensions.D", minimum=0)
        l = require_int(dimensions.get("L"), f"{label} dimensions.L", minimum=0)
        rank = require_int(dimensions.get("rank"), f"{label} dimensions.rank", minimum=0)
        free_width = require_int(dimensions.get("free_width"), f"{label} dimensions.free_width", minimum=0)
        shots = require_int(dimensions.get("shots"), f"{label} dimensions.shots", minimum=0)
        blocks = require_int(dimensions.get("blocks"), f"{label} dimensions.blocks", minimum=0)
        if rank > m or rank > d:
            raise ValueError(f"{label} rank exceeds dimensions")
        if free_width != m - rank:
            raise ValueError(f"{label} free_width must equal M-rank")
        if blocks != expected_blocks(shots):
            raise ValueError(f"{label} blocks must equal default-size block count")

        circuit = require_object(row.get("circuit"), f"{label} circuit")
        circuit_sha256 = require_sha256(circuit.get("canonical_text_sha256"), f"{label} circuit canonical_text_sha256")
        require_sha256(circuit.get("source_sha256"), f"{label} circuit source_sha256")

        if requirement.catalog_case_id is None:
            if circuit.get("path") != "generated://high_entropy_no_detector":
                raise ValueError(f"{label} high entropy circuit path mismatch")
            if circuit_sha256 != HIGH_ENTROPY_CIRCUIT_SHA256:
                raise ValueError(f"{label} high entropy circuit_sha256 mismatch")
            expected_dims = (1024, 0, 0, 0, 1024, 8192, 2)
            if (m, d, l, rank, free_width, shots, blocks) != expected_dims:
                raise ValueError(f"{label} high entropy dimensions mismatch")
        else:
            catalog = catalog_cases.get(requirement.catalog_case_id)
            if catalog is None:
                raise ValueError(f"{label} missing catalog case {requirement.catalog_case_id}")
            if circuit.get("path") != catalog.get("circuit_path"):
                raise ValueError(f"{label} circuit path must match catalog")
            if circuit_sha256 != catalog.get("circuit_sha256"):
                raise ValueError(f"{label} circuit_sha256 must match catalog")
            expected_dims = (
                catalog.get("measurement_count"),
                catalog.get("detector_count"),
                catalog.get("observable_count"),
                catalog.get("rank_H"),
                int(catalog.get("measurement_count")) - int(catalog.get("rank_H")),
                1024 if requirement.case_id == BENCHMARK_CASE_ID else catalog.get("shots"),
            )
            if (m, d, l, rank, free_width, shots) != expected_dims:
                raise ValueError(f"{label} dimensions must match catalog")

        if row.get("is_benchmark") != (requirement.case_id == BENCHMARK_CASE_ID):
            raise ValueError(f"{label} is_benchmark flag mismatch")
        if row.get("is_high_entropy_control") != (requirement.case_id == HIGH_ENTROPY_CASE_ID):
            raise ValueError(f"{label} is_high_entropy_control flag mismatch")

        measurement = require_object(row.get("measurement_input"), f"{label} measurement_input")
        if measurement.get("format") != "b8":
            raise ValueError(f"{label} measurement format must be b8")
        raw_b8_bytes = require_int(measurement.get("raw_b8_bytes"), f"{label} raw_b8_bytes", minimum=0)
        measurement_sha256 = require_sha256(measurement.get("sha256"), f"{label} measurement_sha256")
        logical_digest = require_sha256(measurement.get("logical_digest"), f"{label} logical_digest")
        argv = require_list(measurement.get("argv"), f"{label} measurement argv")
        if any(not isinstance(part, str) for part in argv):
            raise ValueError(f"{label} measurement argv entries must be strings")
        if requirement.case_id == BENCHMARK_CASE_ID:
            if raw_b8_bytes != PINNED_BENCHMARK_RAW_BYTES:
                raise ValueError(f"{label} benchmark raw_b8_bytes mismatch")
            if measurement_sha256 != PINNED_BENCHMARK_SHA256:
                raise ValueError(f"{label} measurement_sha256 must match pinned benchmark")
            if argv != PINNED_BENCHMARK_SAMPLE_ARGV:
                raise ValueError(f"{label} benchmark sample argv mismatch")
        elif requirement.case_id == HIGH_ENTROPY_CASE_ID:
            if raw_b8_bytes != HIGH_ENTROPY_RAW_BYTES:
                raise ValueError(f"{label} high entropy raw_b8_bytes mismatch")
            if measurement_sha256 != HIGH_ENTROPY_RAW_SHA256:
                raise ValueError(f"{label} high entropy measurement_sha256 mismatch")
            if measurement.get("generator") != HIGH_ENTROPY_GENERATOR_ID:
                raise ValueError(f"{label} high entropy generator id mismatch")
            if measurement.get("generator_sha256") != HIGH_ENTROPY_GENERATOR_SHA256:
                raise ValueError(f"{label} high_entropy_generator_sha256 mismatch")
            if raw_b8_bytes != expected_b8_bytes(m, shots):
                raise ValueError(f"{label} high entropy raw bytes must equal shots*M/8")
        else:
            if measurement.get("generator") != "rstim_sample":
                raise ValueError(f"{label} semantic fixture generator must be rstim_sample")
            if raw_b8_bytes != expected_b8_bytes(m, shots):
                raise ValueError(f"{label} raw_b8_bytes must match b8 shape")
        expected_logical = measurement_logical_digest(requirement.case_id, circuit_sha256, dimensions, raw_b8_bytes, measurement_sha256)
        if logical_digest != expected_logical:
            raise ValueError(f"{label} logical_digest mismatch")

        direct = require_object(row.get("direct_zstd"), f"{label} direct_zstd")
        if direct.get("input_sha256") != measurement_sha256:
            raise ValueError(f"{label} direct_zstd input_sha256 must match measurement_sha256")
        require_int(direct.get("bytes"), f"{label} direct_zstd bytes", minimum=1)
        require_sha256(direct.get("sha256"), f"{label} direct_zstd sha256")
        direct_argv = require_list(direct.get("argv"), f"{label} direct_zstd argv")
        validate_direct_zstd_argv(direct_argv, label)

        archive = require_object(row.get("rsmp_archive"), f"{label} rsmp_archive")
        archive_argv = require_list(archive.get("argv"), f"{label} rsmp_archive argv")
        unpack_argv = require_list(archive.get("unpack_argv"), f"{label} rsmp_archive unpack_argv")
        if any(not isinstance(part, str) for part in archive_argv):
            raise ValueError(f"{label} rsmp_archive argv entries must be strings")
        if any(not isinstance(part, str) for part in unpack_argv):
            raise ValueError(f"{label} rsmp_archive unpack_argv entries must be strings")
        if archive.get("input_sha256") != measurement_sha256:
            raise ValueError(f"{label} rsmp_archive input_sha256 must match measurement_sha256")
        if archive.get("roundtrip_measurements_sha256") != measurement_sha256:
            raise ValueError(f"{label} roundtrip measurement SHA-256 mismatch")
        require_int(archive.get("bytes"), f"{label} archive bytes", minimum=1)
        require_sha256(archive.get("sha256"), f"{label} archive sha256")
        for field in ("encode_elapsed_ns", "decode_elapsed_ns", "encode_throughput_bytes_per_second", "decode_throughput_bytes_per_second"):
            require_int(archive.get(field), f"{label} {field}", minimum=0)
        block_rows = require_list(archive.get("blocks"), f"{label} archive blocks")
        if len(block_rows) != blocks:
            raise ValueError(f"{label} archive block count mismatch")
        first_shot = 0
        for block_index, block in enumerate(block_rows):
            block_label = f"{label} block {block_index}"
            block = require_object(block, block_label)
            if require_int(block.get("block_index"), f"{block_label} block_index", minimum=0) != block_index:
                raise ValueError(f"{block_label} block_index mismatch")
            if require_int(block.get("first_shot"), f"{block_label} first_shot", minimum=0) != first_shot:
                raise ValueError(f"{block_label} first_shot mismatch")
            shot_count = require_int(block.get("shot_count"), f"{block_label} shot_count", minimum=1)
            if shot_count > 4096:
                raise ValueError(f"{block_label} exceeds default block size")
            first_shot += shot_count
            for name in ("syndrome_codec", "free_codec"):
                if block.get(name) not in CODEC_NAMES:
                    raise ValueError(f"{block_label} {name} is unknown")
            for name in ("syndrome_uncompressed_bytes", "syndrome_compressed_bytes", "free_uncompressed_bytes", "free_compressed_bytes"):
                require_int(block.get(name), f"{block_label} {name}", minimum=0)
            require_sha256(block.get("logical_payload_sha256"), f"{block_label} logical_payload_sha256")
        if first_shot != shots:
            raise ValueError(f"{label} archive blocks do not cover all shots")
        if archive.get("peak_logical_block_working_set_bytes") != peak_working_set(row):
            raise ValueError(f"{label} peak_logical_block_working_set_bytes mismatch")

        density = require_object(row.get("detector_density"), f"{label} detector_density")
        one_count = require_int(density.get("one_count"), f"{label} detector_density one_count", minimum=0)
        total_bits = require_int(density.get("total_bits"), f"{label} detector_density total_bits", minimum=0)
        ppm = require_int(density.get("ppm"), f"{label} detector_density ppm", minimum=0)
        if total_bits != d * shots:
            raise ValueError(f"{label} detector_density total_bits mismatch")
        if one_count > total_bits:
            raise ValueError(f"{label} detector density exceeds total bits")
        expected_ppm = 0 if total_bits == 0 else one_count * 1_000_000 // total_bits
        if ppm != expected_ppm:
            raise ValueError(f"{label} detector density ppm mismatch")

        contract = require_object(row.get("zstd_contract"), f"{label} zstd_contract")
        if contract != ZSTD_CONTRACT:
            if contract.get("level") != 3:
                raise ValueError(f"{label} zstd level must be 3")
            if contract.get("threshold_arithmetic") != "integer_cross_multiplication":
                raise ValueError(f"{label} threshold_arithmetic must be integer_cross_multiplication")
            raise ValueError(f"{label} zstd_contract mismatch")

        if requirement.case_id == BENCHMARK_CASE_ID:
            validate_stim_baselines(row, label)


def ratio(numerator: int, denominator: int) -> dict[str, int]:
    return {
        "numerator": numerator,
        "denominator": denominator,
        "basis_points_floor": 0 if denominator == 0 else numerator * 10_000 // denominator,
    }


def gate(name: str, lhs: int, op: str, rhs: int, denominator_kind: str, passed: bool) -> dict[str, Any]:
    return {
        "name": name,
        "arithmetic": "integer_cross_multiplication",
        "lhs": lhs,
        "operator": op,
        "rhs": rhs,
        "denominator_kind": denominator_kind,
        "passed": passed,
    }


def row_summary(row: dict[str, Any]) -> dict[str, Any]:
    dimensions = row["dimensions"]
    archive_bytes = row["rsmp_archive"]["bytes"]
    raw_bytes = row["measurement_input"]["raw_b8_bytes"]
    direct_bytes = row["direct_zstd"]["bytes"]
    return {
        "case_id": row["case_id"],
        "semantic_role": row["semantic_role"],
        "dimensions": dimensions,
        "raw_b8_bytes": raw_bytes,
        "direct_zstd_bytes": direct_bytes,
        "archive_bytes": archive_bytes,
        "archive_to_raw": ratio(archive_bytes, raw_bytes),
        "archive_to_direct_zstd": ratio(archive_bytes, direct_bytes),
        "detector_density": row["detector_density"],
        "syndrome_codecs": [block["syndrome_codec"] for block in row["rsmp_archive"]["blocks"]],
        "free_codecs": [block["free_codec"] for block in row["rsmp_archive"]["blocks"]],
        "peak_logical_block_working_set_bytes": row["rsmp_archive"]["peak_logical_block_working_set_bytes"],
        "encode_throughput_bytes_per_second": row["rsmp_archive"]["encode_throughput_bytes_per_second"],
        "decode_throughput_bytes_per_second": row["rsmp_archive"]["decode_throughput_bytes_per_second"],
    }


def stim_baseline_summaries(benchmark: dict[str, Any]) -> dict[str, Any]:
    archive_bytes = int(benchmark["rsmp_archive"]["bytes"])
    summaries: dict[str, Any] = {}
    for fmt in REQUIRED_STIM_BASELINE_FORMATS:
        baseline = benchmark["stim_baselines"][fmt]
        raw_bytes = int(baseline["artifact"]["bytes"])
        direct_bytes = int(baseline["direct_zstd"]["bytes"])
        summaries[fmt] = {
            "raw_bytes": raw_bytes,
            "direct_zstd_bytes": direct_bytes,
            "artifact_sha256": baseline["artifact"]["sha256"],
            "roundtrip_b8_sha256": baseline["roundtrip_b8"]["sha256"],
            "archive_to_raw": ratio(archive_bytes, raw_bytes),
            "archive_to_direct_zstd": ratio(archive_bytes, direct_bytes),
        }
    return summaries


def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    benchmark = records[BENCHMARK_ROW_INDEX]
    high_entropy = records[HIGH_ENTROPY_ROW_INDEX]
    a_benchmark = int(benchmark["rsmp_archive"]["bytes"])
    r_benchmark = int(benchmark["measurement_input"]["raw_b8_bytes"])
    z_benchmark = int(benchmark["direct_zstd"]["bytes"])
    a_high_entropy = int(high_entropy["rsmp_archive"]["bytes"])
    h_high_entropy = int(high_entropy["measurement_input"]["raw_b8_bytes"])
    gates = {
        "benchmark_raw_lt_20pct": gate(
            "benchmark_raw_lt_20pct",
            5 * a_benchmark,
            "<",
            r_benchmark,
            "benchmark_raw_b8_bytes",
            5 * a_benchmark < r_benchmark,
        ),
        "benchmark_zstd_lt_75pct": gate(
            "benchmark_zstd_lt_75pct",
            4 * a_benchmark,
            "<",
            3 * z_benchmark,
            "benchmark_direct_zstd_bytes",
            4 * a_benchmark < 3 * z_benchmark,
        ),
        "high_entropy_raw_le_102pct": gate(
            "high_entropy_raw_le_102pct",
            50 * a_high_entropy,
            "<=",
            51 * h_high_entropy,
            "high_entropy_raw_b8_bytes",
            50 * a_high_entropy <= 51 * h_high_entropy,
        ),
    }
    return {
        "schema_version": RAW_SCHEMA_VERSION,
        "evidence_format": EVIDENCE_FORMAT,
        "case_count": len(records),
        "required_semantic_roles": [row.semantic_role for row in REQUIRED_ROWS[:-1]],
        "cases": [row_summary(row) for row in records],
        "benchmark": row_summary(benchmark),
        "stim_baselines": stim_baseline_summaries(benchmark),
        "high_entropy_control": {
            **row_summary(high_entropy),
            "direct_zstd_is_diagnostic_only": True,
        },
        "gates": gates,
        "pass": all(gate_record["passed"] for gate_record in gates.values()),
        "pass_line": pass_line(gates),
    }


def format_percent(numerator: int, denominator: int) -> str:
    if denominator == 0:
        return "n/a"
    hundredths = numerator * 10_000 // denominator
    return f"{hundredths // 100}.{hundredths % 100:02d}%"


def render_report(summary: dict[str, Any]) -> str:
    benchmark = summary["benchmark"]
    high_entropy = summary["high_entropy_control"]
    gates = summary["gates"]
    lines = [
        "# rsmp v1 Compression Evidence",
        "",
        f"Verdict: {'PASS' if summary['pass'] else 'FAIL'}",
        "",
        "## Gates",
    ]
    for name in ("benchmark_raw_lt_20pct", "benchmark_zstd_lt_75pct", "high_entropy_raw_le_102pct"):
        gate_record = gates[name]
        result = "PASS" if gate_record["passed"] else "FAIL"
        lines.append(
            f"- {name}: {result} ({gate_record['lhs']} {gate_record['operator']} {gate_record['rhs']}; denominator={gate_record['denominator_kind']})"
        )
    lines.extend(
        [
            "",
            "## Byte Counts",
            f"- Benchmark archive/raw: {benchmark['archive_bytes']} / {benchmark['raw_b8_bytes']} ({format_percent(benchmark['archive_bytes'], benchmark['raw_b8_bytes'])}).",
            f"- Benchmark archive/direct-zstd: {benchmark['archive_bytes']} / {benchmark['direct_zstd_bytes']} ({format_percent(benchmark['archive_bytes'], benchmark['direct_zstd_bytes'])}).",
            f"- High-entropy archive/raw: {high_entropy['archive_bytes']} / {high_entropy['raw_b8_bytes']} ({format_percent(high_entropy['archive_bytes'], high_entropy['raw_b8_bytes'])}).",
            f"- High-entropy direct Zstandard bytes: {high_entropy['direct_zstd_bytes']} (diagnostic only, not an acceptance denominator).",
            "",
            "## Cases",
            "| case | role | shots | M | D | L | rank | free | raw b8 | direct zstd | archive | syndrome codecs |",
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
        ]
    )
    for row in summary["cases"]:
        dims = row["dimensions"]
        lines.append(
            "| {case_id} | {role} | {shots} | {m} | {d} | {l} | {rank} | {free} | {raw} | {direct} | {archive} | {codecs} |".format(
                case_id=row["case_id"],
                role=row["semantic_role"],
                shots=dims["shots"],
                m=dims["M"],
                d=dims["D"],
                l=dims["L"],
                rank=dims["rank"],
                free=dims["free_width"],
                raw=row["raw_b8_bytes"],
                direct=row["direct_zstd_bytes"],
                archive=row["archive_bytes"],
                codecs=",".join(row["syndrome_codecs"]),
            )
        )
    lines.extend(
        [
            "",
            "## Stim Format Baselines",
            "Each row serializes the same canonical 1024-shot, seed-7 measurement batch in one Stim result format. Every format round-trips to the canonical b8 SHA-256; ratios are relative to the RSMP archive byte count.",
            "| format | raw bytes | direct zstd | archive/raw | archive/zstd | round-trip b8 SHA-256 |",
            "| --- | ---: | ---: | ---: | ---: | --- |",
        ]
    )
    archive_bytes = benchmark["archive_bytes"]
    for fmt in REQUIRED_STIM_BASELINE_FORMATS:
        baseline = summary["stim_baselines"][fmt]
        lines.append(
            "| {fmt} | {raw} | {direct} | {archive_raw} | {archive_zstd} | {roundtrip} |".format(
                fmt=fmt,
                raw=baseline["raw_bytes"],
                direct=baseline["direct_zstd_bytes"],
                archive_raw=format_percent(archive_bytes, baseline["raw_bytes"]),
                archive_zstd=format_percent(archive_bytes, baseline["direct_zstd_bytes"]),
                roundtrip=baseline["roundtrip_b8_sha256"],
            )
        )
    lines.extend(
        [
            "",
            "## Environment",
            "The exact producer, Git state, Rust target, Cargo.lock hash, zstd package versions, and complete command argv values are recorded in environment.json. This report is rendered from raw.jsonl-derived counts and gate arithmetic.",
            "",
            "## Throughput Observations",
        ]
    )
    for row in summary["cases"]:
        lines.append(
            f"- {row['case_id']}: encode {row['encode_throughput_bytes_per_second']} B/s, decode {row['decode_throughput_bytes_per_second']} B/s, peak logical block working set {row['peak_logical_block_working_set_bytes']} bytes."
        )
    lines.extend(
        [
            "",
            "## Claim Limitations",
            "- These gates prove the pinned `rsmp v1` evidence cases under the recorded producer and zstd settings.",
            "- Direct Zstandard for the high-entropy control is reported only as a diagnostic; the acceptance denominator is raw b8 bytes.",
            "- The Stim `b8`/`r8`/`ptb64` baselines report byte counts and ratios only; no universal cross-format compression superiority is claimed.",
            "- No fixed wall-clock performance gate or cross-version byte-for-byte writer determinism is claimed.",
            "",
        ]
    )
    return "\n".join(lines)


def pass_line(gates: dict[str, dict[str, Any]]) -> str:
    return (
        "PASS rsmp v1 compression "
        f"benchmark_raw_lt_20pct={1 if gates['benchmark_raw_lt_20pct']['passed'] else 0} "
        f"benchmark_zstd_lt_75pct={1 if gates['benchmark_zstd_lt_75pct']['passed'] else 0} "
        f"high_entropy_raw_le_102pct={1 if gates['high_entropy_raw_le_102pct']['passed'] else 0}"
    )


def first_json_difference(expected: Any, actual: Any, path: str = "$") -> str | None:
    if type(expected) is not type(actual):
        return path
    if isinstance(expected, dict):
        expected_keys = set(expected)
        actual_keys = set(actual)
        if expected_keys != actual_keys:
            missing = sorted(expected_keys - actual_keys)
            extra = sorted(actual_keys - expected_keys)
            if missing:
                return f"{path}.{missing[0]}"
            return f"{path}.{extra[0]}"
        for key in sorted(expected):
            diff = first_json_difference(expected[key], actual[key], f"{path}.{key}")
            if diff is not None:
                return diff
        return None
    if isinstance(expected, list):
        if len(expected) != len(actual):
            return path
        for index, (left, right) in enumerate(zip(expected, actual, strict=True)):
            diff = first_json_difference(left, right, f"{path}[{index}]")
            if diff is not None:
                return diff
        return None
    if expected != actual:
        return path
    return None


def validate_environment(environment: dict[str, Any], repo_root: Path = REPO_ROOT) -> None:
    if environment.get("schema_version") != ENVIRONMENT_SCHEMA_VERSION:
        raise ValueError(f"environment.json schema_version must be {ENVIRONMENT_SCHEMA_VERSION}")
    if environment.get("evidence_format") != EVIDENCE_FORMAT:
        raise ValueError(f"environment.json evidence_format must be {EVIDENCE_FORMAT}")
    if require_object(environment.get("zstd_contract"), "environment zstd_contract") != ZSTD_CONTRACT:
        raise ValueError("environment zstd_contract mismatch")
    cargo = require_object(environment.get("cargo"), "environment cargo")
    require_sha256(cargo.get("lock_sha256"), "environment cargo lock_sha256")
    locked_versions = load_locked_package_versions(repo_root)
    for package in ("zstd", "zstd-safe", "zstd-sys"):
        if cargo.get(package) != locked_versions.get(package):
            raise ValueError(f"environment cargo {package} version mismatch")
    zstd_info = require_object(environment.get("zstd_info"), "environment zstd_info")
    expected_info = {
        "crate_version": cargo.get("zstd"),
        "zstd_safe_crate_version": cargo.get("zstd-safe"),
        "zstd_sys_crate_version": cargo.get("zstd-sys"),
        "level": 3,
        "single_threaded": True,
        "dictionary": "none",
        "long_distance_matching": False,
        "pledged_source_size": True,
        "frame_content_size": True,
        "frame_checksum": True,
    }
    for field, expected in expected_info.items():
        if zstd_info.get(field) != expected:
            raise ValueError(f"environment zstd_info {field} mismatch")
    require_string(zstd_info.get("native_zstd_version"), "environment native_zstd_version")
    require_int(zstd_info.get("native_zstd_version_number"), "environment native_zstd_version_number", minimum=1)
    producer = require_object(environment.get("producer"), "environment producer")
    if producer.get("name") != "rstim":
        raise ValueError("environment producer must be rstim")
    binary = require_object(producer.get("rstim_binary"), "environment rstim_binary")
    require_sha256(binary.get("sha256"), "environment rstim_binary sha256")
    binary_path_raw = binary.get("path")
    if isinstance(binary_path_raw, str) and binary_path_raw:
        binary_path = Path(binary_path_raw)
        if not binary_path.is_absolute():
            binary_path = repo_root / binary_path
        if binary_path.exists() and sha256_file(binary_path) != binary["sha256"]:
            raise ValueError("environment rstim_binary sha256 mismatch")
    commands = require_object(environment.get("commands"), "environment commands")
    if commands.get("benchmark_sample") != PINNED_BENCHMARK_SAMPLE_ARGV:
        raise ValueError("environment benchmark sample argv mismatch")
    stim = require_object(environment.get("stim"), "environment stim")
    require_string(stim.get("version"), "environment stim version")
    require_string(stim.get("version_source"), "environment stim version_source")
    stim_binary = require_object(stim.get("binary"), "environment stim binary")
    stim_binary_path_raw = require_string(stim_binary.get("path"), "environment stim binary path")
    stim_binary_sha256 = require_sha256(stim_binary.get("sha256"), "environment stim binary sha256")
    stim_binary_path = Path(stim_binary_path_raw)
    if not stim_binary_path.is_absolute():
        stim_binary_path = repo_root / stim_binary_path
    if stim_binary_path.exists() and sha256_file(stim_binary_path) != stim_binary_sha256:
        raise ValueError("environment stim binary sha256 mismatch")
    stim_commands = require_object(commands.get("stim_baselines"), "environment commands stim_baselines")
    if sorted(stim_commands) != sorted(REQUIRED_STIM_BASELINE_FORMATS):
        raise ValueError("environment commands stim_baselines must cover exactly b8, r8, ptb64")
    for fmt in REQUIRED_STIM_BASELINE_FORMATS:
        entry = require_object(stim_commands[fmt], f"environment commands stim_baselines {fmt}")
        for key in ("serialize", "direct_zstd", "roundtrip"):
            require_string_list(entry.get(key), f"environment commands stim_baselines {fmt} {key}")
    require_object(environment.get("git"), "environment git")
    require_object(environment.get("platform"), "environment platform")
    require_object(environment.get("generator"), "environment generator")


def validate_artifact_hashes(results_dir: Path) -> None:
    hashes = load_json_object(results_dir / "artifact-sha256.json", "artifact-sha256.json")
    if set(hashes) != set(ARTIFACT_FILES):
        raise ValueError("artifact-sha256 must contain hashes for the four evidence files")
    for filename in ARTIFACT_FILES:
        expected = require_sha256(hashes.get(filename), f"artifact-sha256 {filename}")
        actual = sha256_file(results_dir / filename)
        if actual != expected:
            raise ValueError(f"artifact-sha256 mismatch for {filename}")


def validate_gates(summary: dict[str, Any]) -> None:
    failed = [name for name, record in summary["gates"].items() if not record["passed"]]
    if failed:
        raise ValueError("gate failure: " + ", ".join(failed))


def check_bundle(results_dir: Path, repo_root: Path = REPO_ROOT) -> str:
    validate_required_files(results_dir)
    records = load_raw_records(results_dir / "raw.jsonl")
    validate_records(records, repo_root)
    derived_summary = derive_summary(records)
    summary = load_json_object(results_dir / "summary.json", "summary.json")
    diff = first_json_difference(derived_summary, summary)
    if diff is not None:
        raise ValueError(f"summary.json is stale or not checker-derived at {diff}")
    expected_report = render_report(derived_summary)
    actual_report = (results_dir / "report.md").read_text(encoding="utf-8")
    if actual_report != expected_report:
        raise ValueError("report.md is stale or not checker-derived")
    environment = load_json_object(results_dir / "environment.json", "environment.json")
    validate_environment(environment, repo_root)
    validate_artifact_hashes(results_dir)
    validate_gates(derived_summary)
    return derived_summary["pass_line"]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check rsmp v1 compression evidence bundle")
    parser.add_argument("--results-dir", required=True, type=Path)
    parser.add_argument("--repo-root", default=REPO_ROOT, type=Path)
    args = parser.parse_args(argv)
    try:
        line = check_bundle(args.results_dir, args.repo_root.resolve())
    except ValueError as error:
        print(f"FAIL rsmp v1 compression {error}", file=sys.stderr)
        return 1
    print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
