#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator import run_reference_build_benchmark as runner


REQUIRED_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json", "artifact-sha256.json")
ARTIFACT_FILES = REQUIRED_FILES[:-1]
VARIANTS = (runner.STIM_VARIANT, runner.RSTIM_VARIANT)
BACKENDS = {
    runner.STIM_VARIANT: runner.STIM_BACKEND,
    runner.RSTIM_VARIANT: runner.RSTIM_BACKEND,
}
CANONICAL_FIXTURE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
CANONICAL_MANIFEST = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
EXPECTED_FIXTURE_SHA256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
GIT_COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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
        raise ValueError(f"could not read raw.jsonl: {error}") from error
    records: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            raise ValueError(f"raw.jsonl line {line_number} must not be blank")
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"raw.jsonl line {line_number} is not valid JSON") from error
        if not isinstance(record, dict):
            raise ValueError(f"raw.jsonl line {line_number} must be a JSON object")
        records.append(record)
    return records


def validate_required_files(results_dir: Path) -> None:
    try:
        entries = sorted(path.name for path in results_dir.iterdir())
    except OSError as error:
        raise ValueError(f"could not read bundle directory: {error}") from error
    unexpected = sorted(set(entries) - set(REQUIRED_FILES))
    if unexpected:
        raise ValueError(f"unexpected bundle file: {unexpected[0]}")
    for filename in REQUIRED_FILES:
        if not (results_dir / filename).is_file():
            raise ValueError(f"missing required bundle file: {filename}")


def _require_equal(actual: Any, expected: Any, message: str) -> None:
    if actual != expected:
        raise ValueError(f"{message}, got {actual!r}")


def _require_int_equal(actual: Any, expected: int, message: str) -> None:
    if not isinstance(actual, int) or isinstance(actual, bool) or actual != expected:
        raise ValueError(f"{message} must be integer {expected}, got {actual!r}")


def _require_positive_int(value: Any, message: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError(f"{message} must be a positive integer, got {value!r}")
    return value


def _decode_packed_base64(value: Any, variant: str, round_index: int) -> bytes:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{variant} round {round_index} packed_base64 must be a nonempty string")
    try:
        return base64.b64decode(value.encode("ascii"), validate=True)
    except (UnicodeEncodeError, binascii.Error) as error:
        raise ValueError(f"{variant} round {round_index} packed_base64 is not valid base64") from error


def validate_raw_semantics(records: list[dict[str, Any]]) -> dict[str, Any]:
    if len(records) != 18:
        raise ValueError("raw.jsonl must contain exactly 18 records")
    if set(record.get("variant") for record in records) != set(VARIANTS):
        raise ValueError("raw.jsonl variants must be stim-reference-b8 and rstim-packed-reference-b8")

    for variant in VARIANTS:
        backend = BACKENDS[variant]
        variant_records = [record for record in records if record.get("variant") == variant]
        if len(variant_records) != 9:
            raise ValueError(f"{variant} must contain exactly 9 records")
        expected_phases = ["warmup", "warmup", *["measured"] * 7]
        if [record.get("phase") for record in variant_records] != expected_phases:
            raise ValueError(f"{variant} phases must be two warmups followed by seven measured records")
        if [record.get("round") for record in variant_records] != list(range(9)):
            raise ValueError(f"{variant} rounds must be 0 through 8")

        for expected_round, record in enumerate(variant_records):
            _require_equal(record.get("protocol"), runner.PROTOCOL, f"{variant} protocol must be {runner.PROTOCOL}")
            _require_positive_int(record.get("elapsed_ns"), f"{variant} elapsed_ns")
            _require_equal(record.get("backend"), backend, f"{variant} backend must be {backend}")
            _require_equal(record.get("timer_scope"), runner.TIMER_SCOPE, f"{variant} timer_scope must be {runner.TIMER_SCOPE}")
            _require_int_equal(record.get("parse_count"), 1, f"{variant} parse_count")
            _require_int_equal(
                record.get("measurement_bits"),
                runner.EXPECTED_MEASUREMENT_BITS,
                f"{variant} measurement_bits",
            )
            _require_int_equal(record.get("packed_bytes"), runner.EXPECTED_PACKED_BYTES, f"{variant} packed_bytes")
            _require_equal(
                record.get("byte_sha256"),
                runner.EXPECTED_REFERENCE_SHA256,
                f"{variant} byte_sha256 must be {runner.EXPECTED_REFERENCE_SHA256}",
            )
            packed = _decode_packed_base64(record.get("packed_base64"), variant, expected_round)
            if len(packed) != runner.EXPECTED_PACKED_BYTES:
                raise ValueError(
                    f"{variant} round {expected_round} decoded packed bytes length must be "
                    f"{runner.EXPECTED_PACKED_BYTES}"
                )
            decoded_sha256 = hashlib.sha256(packed).hexdigest()
            if decoded_sha256 != runner.EXPECTED_REFERENCE_SHA256:
                raise ValueError(
                    f"{variant} round {expected_round} decoded packed bytes SHA-256 must be "
                    f"{runner.EXPECTED_REFERENCE_SHA256}, got {decoded_sha256}"
                )
            reference_build_count = record.get("reference_build_count")
            if (
                not isinstance(reference_build_count, int)
                or isinstance(reference_build_count, bool)
                or reference_build_count != expected_round + 1
            ):
                if expected_round == 8:
                    raise ValueError(f"{variant} final reference_build_count must be 9")
                raise ValueError(
                    f"{variant} reference_build_count for round {expected_round} "
                    f"must be integer {expected_round + 1}, got {reference_build_count!r}"
                )

    return {"variants": 2, "measured_records": 14, "final_reference_build_count": 9}


def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    return runner.derive_summary(records)


def render_report(summary: dict[str, Any]) -> str:
    return runner.render_report(summary)


def _resolve_recorded_path(raw: Any, field: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise ValueError(f"environment {field} must be a nonempty path")
    path = Path(raw)
    return path.resolve() if path.is_absolute() else (REPO_ROOT / path).resolve()


def _validate_path_hash(environment: dict[str, Any], path_field: str, hash_field: str) -> Path:
    path = _resolve_recorded_path(environment.get(path_field), path_field)
    if not path.is_file():
        raise ValueError(f"environment {path_field} does not exist: {environment.get(path_field)}")
    digest = environment.get(hash_field)
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError(f"environment {hash_field} must be a lowercase SHA-256 digest")
    if sha256_file(path) != digest:
        raise ValueError(f"environment {hash_field} does not match {path_field}")
    return path


def _validate_git_commit(value: Any) -> None:
    if not isinstance(value, str) or GIT_COMMIT_RE.fullmatch(value) is None:
        raise ValueError("environment git_commit must be a 40-character lowercase hex commit SHA")
    completed = subprocess.run(
        ["git", "cat-file", "-e", f"{value}^{{commit}}"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip()
        suffix = f": {detail}" if detail else ""
        raise ValueError(f"environment git_commit must exist in the local git object database{suffix}")


def _validate_canonical_path(
    environment: dict[str, Any],
    field: str,
    canonical_path: Path,
    description: str,
) -> Path:
    path = _resolve_recorded_path(environment.get(field), field)
    if path != canonical_path.resolve():
        raise ValueError(f"environment {field} must name the canonical {description}")
    return path


def _validate_string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{label} must be a nonempty string array")
    return value


def _resolve_command_executable(raw: str, label: str) -> Path:
    path = Path(raw)
    if path.is_absolute() or len(path.parts) > 1:
        candidate = path if path.is_absolute() else REPO_ROOT / path
        if candidate.is_file():
            return candidate.resolve()
        raise ValueError(f"{label} executable does not exist: {raw}")
    resolved = shutil.which(raw)
    if resolved is not None:
        return Path(resolved).resolve()
    repo_candidate = REPO_ROOT / raw
    if repo_candidate.is_file():
        return repo_candidate.resolve()
    raise ValueError(f"{label} executable does not exist: {raw}")


def _validate_worker_argv(environment: dict[str, Any]) -> None:
    worker_argv = environment.get("worker_argv")
    if not isinstance(worker_argv, dict) or set(worker_argv) != set(VARIANTS):
        raise ValueError("environment worker_argv must contain both reference-build variants")
    canonical_worker_argv = environment.get("canonical_worker_argv")
    if not isinstance(canonical_worker_argv, dict) or set(canonical_worker_argv) != set(VARIANTS):
        raise ValueError("environment canonical_worker_argv must contain both reference-build variants")

    expected_canonical = {
        runner.STIM_VARIANT: runner.default_stim_worker_argv("python3"),
        runner.RSTIM_VARIANT: runner.default_rstim_worker_argv("target/release/rstim_reference_build_worker"),
    }
    if canonical_worker_argv != expected_canonical:
        raise ValueError("environment canonical_worker_argv must match release reference-build commands")

    stim_argv = _validate_string_list(worker_argv[runner.STIM_VARIANT], f"environment worker_argv {runner.STIM_VARIANT}")
    rstim_argv = _validate_string_list(worker_argv[runner.RSTIM_VARIANT], f"environment worker_argv {runner.RSTIM_VARIANT}")
    expected_stim_suffix = runner.default_stim_worker_argv(stim_argv[0])[1:]
    expected_rstim_suffix = runner.default_rstim_worker_argv(rstim_argv[0])[1:]
    if stim_argv[1:] != expected_stim_suffix:
        raise ValueError(f"environment worker_argv {runner.STIM_VARIANT} must run the canonical Stim worker")
    if rstim_argv[1:] != expected_rstim_suffix:
        raise ValueError(f"environment worker_argv {runner.RSTIM_VARIANT} must run the canonical rstim worker")

    python_path = _resolve_recorded_path(environment.get("python_executable"), "python_executable")
    rstim_path = _resolve_recorded_path(environment.get("rstim_worker_binary_path"), "rstim_worker_binary_path")
    if _resolve_command_executable(stim_argv[0], f"environment worker_argv {runner.STIM_VARIANT}") != python_path:
        raise ValueError("environment worker_argv stim executable must match python_executable")
    if _resolve_command_executable(rstim_argv[0], f"environment worker_argv {runner.RSTIM_VARIANT}") != rstim_path:
        raise ValueError("environment worker_argv rstim executable must match rstim_worker_binary_path")


def _validate_runner_executable(raw: str) -> None:
    if raw == "python3":
        return
    path = Path(raw)
    if not path.is_absolute() or not path.name.lower().startswith("python"):
        raise ValueError("environment runner_argv executable must be python3 or an absolute Python executable")


def _validate_runner_argv(environment: dict[str, Any], results_dir: Path, runner_python_path: Path) -> None:
    argv = _validate_string_list(environment.get("runner_argv"), "environment runner_argv")
    if len(argv) != 17:
        raise ValueError("environment runner_argv must match the full canonical runner command")
    _validate_runner_executable(argv[0])
    if _resolve_command_executable(argv[0], "environment runner_argv") != runner_python_path:
        raise ValueError("environment runner_argv executable must match runner_python_executable")
    if argv[1:4] != ["-m", runner.MODULE_NAME, "--fixture"] or argv[5] != "--manifest":
        raise ValueError("environment runner_argv must invoke the canonical runner module")
    if _resolve_recorded_path(argv[4], "runner_argv fixture") != _resolve_recorded_path(environment.get("fixture_path"), "fixture_path"):
        raise ValueError("environment runner_argv fixture must match fixture_path")
    if _resolve_recorded_path(argv[6], "runner_argv manifest") != _resolve_recorded_path(environment.get("manifest_path"), "manifest_path"):
        raise ValueError("environment runner_argv manifest must match manifest_path")

    expected_tail = [
        "--stim-python",
        environment["worker_argv"][runner.STIM_VARIANT][0],
        "--rstim-worker",
        environment["worker_argv"][runner.RSTIM_VARIANT][0],
        "--warmup-rounds",
        "2",
        "--measure-rounds",
        "7",
        "--out-dir",
    ]
    if argv[7:16] != expected_tail or not argv[16]:
        raise ValueError("environment runner_argv must match the full canonical runner command")
    if _resolve_recorded_path(argv[16], "runner_argv --out-dir") != results_dir.resolve():
        raise ValueError("environment runner_argv --out-dir must match checked bundle directory")


def validate_environment(
    environment: dict[str, Any],
    derived: dict[str, Any],
    records: list[dict[str, Any]],
    results_dir: Path,
) -> None:
    del derived, records
    _validate_git_commit(environment.get("git_commit"))
    for field in ("os", "cpu_model", "rustc_version", "cargo_version", "python_version"):
        if not isinstance(environment.get(field), str) or not environment[field]:
            raise ValueError(f"environment {field} must be nonempty")
    if not isinstance(environment.get("git_dirty"), bool):
        raise ValueError("environment git_dirty must be a boolean")
    for field, expected in (
        ("profile", "release"),
        ("protocol", runner.PROTOCOL),
        ("timer_scope", runner.TIMER_SCOPE),
        ("seed_policy", runner.SEED_POLICY),
        ("stim_version", runner.EXPECTED_STIM_VERSION),
        ("manifest_sha256", runner.EXPECTED_MANIFEST_SHA256),
        ("warmup_rounds", 2),
        ("measure_rounds", 7),
    ):
        _require_equal(environment.get(field), expected, f"environment {field} must be {expected!r}")

    fixture_path = _validate_canonical_path(environment, "fixture_path", CANONICAL_FIXTURE, "reference-build fixture")
    manifest_path = _validate_canonical_path(environment, "manifest_path", CANONICAL_MANIFEST, "full case manifest")
    if environment.get("fixture_sha256") != EXPECTED_FIXTURE_SHA256:
        raise ValueError("environment fixture_sha256 must be canonical reference-build fixture SHA-256")
    if sha256_file(fixture_path) != EXPECTED_FIXTURE_SHA256:
        raise ValueError("canonical reference-build fixture file SHA-256 does not match expected digest")
    if sha256_file(manifest_path) != environment.get("manifest_sha256"):
        raise ValueError("environment manifest_sha256 does not match manifest_path")
    _validate_path_hash(environment, "python_executable", "python_executable_sha256")
    _validate_path_hash(environment, "rstim_worker_binary_path", "rstim_worker_binary_sha256")
    runner_python_path = _validate_path_hash(
        environment,
        "runner_python_executable",
        "runner_python_executable_sha256",
    )
    _validate_worker_argv(environment)
    _validate_runner_argv(environment, results_dir, runner_python_path)


def validate_artifact_hashes(results_dir: Path) -> None:
    hashes = load_json_object(results_dir / "artifact-sha256.json", "artifact-sha256.json")
    if set(hashes) != set(ARTIFACT_FILES):
        raise ValueError("artifact-sha256.json must map exactly raw.jsonl, summary.json, report.md, and environment.json")
    for filename in ARTIFACT_FILES:
        digest = hashes[filename]
        if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
            raise ValueError(f"artifact-sha256.json {filename} must be a lowercase SHA-256 digest")
        if digest != sha256_file(results_dir / filename):
            raise ValueError(f"artifact-sha256.json digest does not match {filename}")


def validate_bundle(results_dir: Path) -> None:
    validate_required_files(results_dir)
    records = load_raw_records(results_dir / "raw.jsonl")
    derived = validate_raw_semantics(records)
    summary = derive_summary(records)
    if load_json_object(results_dir / "summary.json", "summary.json") != summary:
        raise ValueError("summary.json does not match summary derived from raw.jsonl")
    if (results_dir / "report.md").read_text(encoding="utf-8") != render_report(summary):
        raise ValueError("report.md does not match summary.json")
    environment = load_json_object(results_dir / "environment.json", "environment.json")
    validate_environment(environment, derived, records, results_dir)
    validate_artifact_hashes(results_dir)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate packed reference-build evidence.")
    parser.add_argument("--dir", type=Path, required=True, dest="results_dir")
    args = parser.parse_args(argv)
    try:
        validate_bundle(args.results_dir)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    print("PASS packed reference-build evidence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
