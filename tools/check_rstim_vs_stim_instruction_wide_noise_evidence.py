#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator import run_frame_instruction_wide_benchmark as runner
from benchmarks.rstim_vs_stim_simulator.portable_provenance import load_catalog


REQUIRED_FILES = (
    "raw.jsonl",
    "summary.json",
    "report.md",
    "environment.json",
    "fixture-load.json",
    "correctness-summary.json",
    "artifact-sha256.json",
)
ARTIFACT_FILES = REQUIRED_FILES[:-1]
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
GIT_COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")
CANONICAL_FIXTURE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
CANONICAL_MANIFEST = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
CATALOG_PATH = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
BUNDLE_ID = "frame-instruction-wide-release"
RUNTIME_ROLE = "tool://rstim"
RUNTIME_IDENTITY_FIELDS = frozenset({"role", "version", "basename", "sha256"})


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
    records = []
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
    for filename in REQUIRED_FILES:
        if not (results_dir / filename).is_file():
            raise ValueError(f"missing required bundle file: {filename}")


def _require_equal(actual: Any, expected: Any, message: str) -> None:
    if actual != expected:
        raise ValueError(f"{message}, got {actual!r}")


def _require_int(value: Any, expected: int, message: str) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value != expected:
        raise ValueError(f"{message} must be {expected}, got {value!r}")


def _require_digest(value: Any, message: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        raise ValueError(f"{message} must be a lowercase SHA-256 digest")
    return value


def validate_raw_semantics(records: list[dict[str, Any]]) -> dict[str, int]:
    if len(records) != 3:
        raise ValueError("raw.jsonl must contain exactly three measured operation rows")
    if [record.get("operation") for record in records] != list(runner.OPERATION_ORDER):
        raise ValueError("raw.jsonl operations must be X_ERROR, DEPOLARIZE1, DEPOLARIZE2")
    for record in records:
        _require_equal(record.get("case_id"), runner.EXPECTED_CASE_ID, "raw case_id must be stim_surface_d11_r100")
        _require_equal(record.get("phase"), "measured", "raw phase must be measured")
        _require_int(record.get("round_index"), 0, "raw round_index")
        _require_int(record.get("seed"), 7, "raw seed")
        _require_equal(record.get("sampling_path"), "sparse", "raw sampling_path must be sparse")
        _require_equal(record.get("timer_scope"), runner.TIMER_SCOPE, "raw timer_scope must match measurement timer")
        _require_equal(record.get("output_format"), runner.OUTPUT_FORMAT, "raw output_format must be b8")
        _require_int(record.get("output_bits"), runner.EXPECTED_OUTPUT_BITS, "raw output_bits")
        _require_int(record.get("bytes_per_shot"), runner.EXPECTED_BYTES_PER_SHOT, "raw bytes_per_shot")
        _require_int(record.get("actual_output_bytes"), runner.EXPECTED_OUTPUT_BYTES, "raw actual_output_bytes")
        _require_int(record.get("expected_output_bytes"), runner.EXPECTED_OUTPUT_BYTES, "raw expected_output_bytes")
        _require_digest(record.get("stdout_sha256"), "raw stdout_sha256")
        elapsed = record.get("elapsed_ns")
        if not isinstance(elapsed, int) or isinstance(elapsed, bool) or elapsed <= 0:
            raise ValueError(f"raw elapsed_ns must be a positive integer, got {elapsed!r}")
    measurement_fields = (
        "elapsed_ns",
        "stdout_sha256",
        "actual_output_bytes",
        "expected_output_bytes",
        "output_bits",
        "bytes_per_shot",
        "output_format",
        "timer_scope",
    )
    first = records[0]
    for record in records[1:]:
        for field in measurement_fields:
            if record.get(field) != first.get(field):
                raise ValueError(f"raw measurement field {field} must be identical across operation rows")

    for operation, expected in runner.EXPECTED_OPERATION_TOTALS.items():
        row = next(record for record in records if record.get("operation") == operation)
        for field, value in expected.items():
            _require_int(row.get(field), value, f"{operation} {field}")
    total_builds = sum(record["iterator_builds"] for record in records)
    total_attempts = sum(record["attempt_count"] for record in records)
    if total_builds != 803:
        raise ValueError(f"total iterator_builds must be 803, got {total_builds!r}")
    if total_attempts != 82_290_688:
        raise ValueError(f"total attempt_count must be 82290688, got {total_attempts!r}")
    return {"iterator_builds": total_builds, "attempt_count": total_attempts}


def validate_summary_and_report(records: list[dict[str, Any]], summary: dict[str, Any], report: str) -> None:
    expected_summary = runner.derive_summary(records)
    if summary != expected_summary:
        raise ValueError("summary.json does not match summary derived from raw.jsonl")
    expected_report = runner.render_report(expected_summary)
    if report != expected_report:
        raise ValueError("report.md does not match report derived from raw.jsonl")


def validate_fixture_load(payload: dict[str, Any]) -> int:
    _require_equal(payload.get("case_id"), runner.EXPECTED_CASE_ID, "fixture-load case_id must match")
    _require_equal(payload.get("status"), "pass", "fixture-load status must be pass")
    _require_int(payload.get("actual_measurements"), runner.EXPECTED_OUTPUT_BITS, "fixture-load actual_measurements")
    _require_int(payload.get("actual_detectors"), runner.EXPECTED_DETECTORS, "fixture-load actual_detectors")
    _require_int(payload.get("actual_observables"), runner.EXPECTED_OBSERVABLES, "fixture-load actual_observables")
    operations = payload.get("operations")
    if not isinstance(operations, dict):
        raise ValueError("fixture-load operations must be an object")
    x_targets = _operation_target_count(operations, "X_ERROR")
    d1_targets = _operation_target_count(operations, "DEPOLARIZE1")
    d2_targets = _operation_target_count(operations, "DEPOLARIZE2")
    if x_targets != 24_362:
        raise ValueError(f"fixture-load X_ERROR targets must be 24362, got {x_targets!r}")
    if d1_targets != 12_000:
        raise ValueError(f"fixture-load DEPOLARIZE1 targets must be 12000, got {d1_targets!r}")
    if d2_targets != 88_000:
        raise ValueError(f"fixture-load DEPOLARIZE2 targets must be 88000, got {d2_targets!r}")
    legacy_setups = x_targets + d1_targets + d2_targets // 2
    if legacy_setups != 80_362:
        raise ValueError(f"fixture-load legacy setups must be 80362, got {legacy_setups!r}")
    return legacy_setups


def _operation_target_count(operations: dict[str, Any], operation: str) -> int:
    entry = operations.get(operation)
    if not isinstance(entry, dict):
        raise ValueError(f"fixture-load missing {operation}")
    value = entry.get("target_count")
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"fixture-load {operation} target_count must be an integer")
    return value


def validate_correctness(payload: dict[str, Any]) -> None:
    _require_equal(payload.get("status"), "pass", "correctness-summary status must be pass")
    _require_equal(payload.get("mode"), "detect", "correctness-summary mode must be detect")
    _require_equal(payload.get("output_format"), "01", "correctness-summary output_format must be 01")
    _require_int(payload.get("seed"), 7, "correctness-summary seed")
    _require_int(payload.get("shots"), 1024, "correctness-summary shots")
    _require_int(payload.get("detectors"), 12_000, "correctness-summary detectors")
    _require_int(payload.get("observables"), 1, "correctness-summary observables")
    expected_bytes = (12_000 + 1 + 1) * 1024
    _require_int(payload.get("expected_output_bytes"), expected_bytes, "correctness-summary expected_output_bytes")
    _require_int(payload.get("stim_output_bytes"), expected_bytes, "correctness-summary stim_output_bytes")
    _require_int(payload.get("rstim_output_bytes"), expected_bytes, "correctness-summary rstim_output_bytes")
    _require_digest(payload.get("stim_stdout_sha256"), "correctness-summary stim_stdout_sha256")
    _require_digest(payload.get("rstim_stdout_sha256"), "correctness-summary rstim_stdout_sha256")
    _require_int(payload.get("sample_count"), 1024, "correctness-summary sample_count")
    failure_reasons = payload.get("failure_reasons")
    if failure_reasons != []:
        raise ValueError("correctness-summary failure_reasons must be empty")
    max_delta = payload.get("max_delta")
    max_tolerance = payload.get("max_tolerance")
    if not isinstance(max_delta, (int, float)) or isinstance(max_delta, bool):
        raise ValueError("correctness-summary max_delta must be numeric")
    if not isinstance(max_tolerance, (int, float)) or isinstance(max_tolerance, bool):
        raise ValueError("correctness-summary max_tolerance must be numeric")
    if float(max_delta) > float(max_tolerance):
        raise ValueError("correctness-summary max_delta must be within max_tolerance")


def _resolve_recorded_path(raw: Any, field: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise ValueError(f"environment {field} must be a nonempty path")
    path = Path(raw)
    return path.resolve() if path.is_absolute() else (REPO_ROOT / path).resolve()


def _validate_path_hash(environment: dict[str, Any], path_field: str, hash_field: str) -> Path:
    path = _resolve_recorded_path(environment.get(path_field), path_field)
    if not path.is_file():
        raise ValueError(f"environment {path_field} does not exist: {environment.get(path_field)}")
    digest = _require_digest(environment.get(hash_field), f"environment {hash_field}")
    if sha256_file(path) != digest:
        raise ValueError(f"environment {hash_field} does not match {path_field}")
    return path


def _normalize_runtime_identity(raw_identity: Any, label: str) -> dict[str, str]:
    if not isinstance(raw_identity, dict):
        raise ValueError(f"{label} must be an object")
    unsupported = sorted(set(raw_identity) - RUNTIME_IDENTITY_FIELDS)
    if unsupported:
        raise ValueError(f"{label} unsupported field(s): {', '.join(unsupported)}")
    missing = [field for field in ("role", "version", "basename", "sha256") if field not in raw_identity]
    if missing:
        raise ValueError(f"{label} missing required field(s): {', '.join(missing)}")
    role = raw_identity["role"]
    version = raw_identity["version"]
    basename = raw_identity["basename"]
    digest = _require_digest(raw_identity["sha256"], f"{label} sha256")
    if role != RUNTIME_ROLE:
        raise ValueError(f"{label} role must be {RUNTIME_ROLE}")
    if not isinstance(version, str) or not version:
        raise ValueError(f'{label} field "version" must be a nonempty string')
    if not isinstance(basename, str) or not basename:
        raise ValueError(f'{label} field "basename" must be a nonempty string')
    if "/" in basename or "\\" in basename:
        raise ValueError(f'{label} field "basename" must not contain path separators')
    return {"role": role, "version": version, "basename": basename, "sha256": digest}


def load_catalog_runtime_identity() -> dict[str, str]:
    catalog = load_catalog(CATALOG_PATH)
    if catalog.get("schema") != 2:
        raise ValueError("evidence catalog schema must be 2")
    bundles = catalog.get("bundles")
    if not isinstance(bundles, list):
        raise ValueError("evidence catalog bundles must be an array")
    matching_bundles = [bundle for bundle in bundles if isinstance(bundle, dict) and bundle.get("id") == BUNDLE_ID]
    if len(matching_bundles) != 1:
        raise ValueError(f'evidence catalog must contain exactly one bundle "{BUNDLE_ID}"')
    identities = matching_bundles[0].get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError(f'evidence catalog bundle "{BUNDLE_ID}" runtime_identities must be an array')
    if len(identities) != 1:
        raise ValueError(f'evidence catalog bundle "{BUNDLE_ID}" must contain exactly one runtime identity')
    return _normalize_runtime_identity(identities[0], f'evidence catalog bundle "{BUNDLE_ID}" runtime identity')


def validate_environment_runtime_identity(environment: dict[str, Any]) -> dict[str, str]:
    identities = environment.get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError("environment runtime_identities must contain exactly one tool://rstim identity")
    matches = [
        _normalize_runtime_identity(identity, f"environment runtime_identities[{index}]")
        for index, identity in enumerate(identities)
        if isinstance(identity, dict) and identity.get("role") == RUNTIME_ROLE
    ]
    if len(matches) != 1:
        raise ValueError("environment runtime_identities must contain exactly one tool://rstim identity")
    identity = matches[0]
    catalog_identity = load_catalog_runtime_identity()
    if identity != catalog_identity:
        raise ValueError("environment runtime identity must match schema-v2 catalog identity")
    return identity


def validate_runtime_binary(runtime_binary: Path, identity: dict[str, str]) -> None:
    if not runtime_binary.is_file():
        raise ValueError(f"runtime binary path does not exist: {runtime_binary}")
    if sha256_file(runtime_binary) != identity["sha256"]:
        raise ValueError("runtime binary SHA-256 does not match recorded identity")


def validate_environment(
    environment: dict[str, Any], results_dir: Path, verify_runtime_binary: Path | None = None
) -> None:
    if not isinstance(environment.get("git_commit"), str) or GIT_COMMIT_RE.fullmatch(environment["git_commit"]) is None:
        raise ValueError("environment git_commit must be a 40-character lowercase hex commit SHA")
    if not isinstance(environment.get("git_dirty"), bool):
        raise ValueError("environment git_dirty must be boolean")
    if environment["git_dirty"]:
        raise ValueError("environment git_dirty must be false for published release evidence")
    for field, expected in (
        ("profile", "release"),
        ("case_id", runner.EXPECTED_CASE_ID),
        ("shots", 1024),
        ("seed", 7),
        ("warmup_rounds", 0),
        ("measure_rounds", 1),
        ("timer_scope", runner.TIMER_SCOPE),
        ("stim_version", runner.EXPECTED_STIM_VERSION),
    ):
        _require_equal(environment.get(field), expected, f"environment {field} must be {expected}")
    for field in ("rstim_version", "rustc_version", "os", "cpu_model"):
        if not isinstance(environment.get(field), str) or not environment[field]:
            raise ValueError(f"environment {field} must be nonempty")
    fixture = _validate_path_hash(environment, "fixture", "fixture_sha256")
    manifest = _validate_path_hash(environment, "manifest", "manifest_sha256")
    runtime_identity = validate_environment_runtime_identity(environment)
    _require_equal(
        environment["rstim_version"],
        runtime_identity["version"],
        "environment rstim_version must match runtime identity version",
    )
    if verify_runtime_binary is not None:
        validate_runtime_binary(verify_runtime_binary, runtime_identity)
    if fixture != CANONICAL_FIXTURE.resolve():
        raise ValueError("environment fixture must name the canonical fixture")
    if manifest != CANONICAL_MANIFEST.resolve():
        raise ValueError("environment manifest must name the canonical manifest")
    if environment["fixture_sha256"] != runner.EXPECTED_FIXTURE_SHA256:
        raise ValueError("environment fixture_sha256 must match canonical fixture digest")
    if environment["manifest_sha256"] != runner.EXPECTED_MANIFEST_SHA256:
        raise ValueError("environment manifest_sha256 must match canonical manifest digest")
    runner_argv = environment.get("runner_argv")
    if not isinstance(runner_argv, list) or not all(isinstance(item, str) and item for item in runner_argv):
        raise ValueError("environment runner_argv must be a string array")
    child_argv = environment.get("child_argv")
    if not isinstance(child_argv, dict) or set(child_argv) != {"measurement", "correctness_stim", "correctness_rstim"}:
        raise ValueError("environment child_argv must contain measurement and correctness commands")
    for key, argv in child_argv.items():
        if not isinstance(argv, list) or not all(isinstance(item, str) and item for item in argv):
            raise ValueError(f"environment child_argv {key} must be a string array")
    recorded = environment.get("artifact_sha256")
    if not isinstance(recorded, dict):
        raise ValueError("environment artifact_sha256 must be an object")
    for filename in ARTIFACT_FILES:
        if filename == "environment.json":
            continue
        digest = _require_digest(recorded.get(filename), f"environment artifact_sha256 {filename}")
        if digest != sha256_file(results_dir / filename):
            raise ValueError(f"environment artifact_sha256 {filename} does not match {filename}")


def validate_artifact_hashes(results_dir: Path) -> None:
    manifest = load_json_object(results_dir / "artifact-sha256.json", "artifact-sha256.json")
    if set(manifest) != set(ARTIFACT_FILES):
        raise ValueError("artifact-sha256.json must hash exactly the six non-hash-manifest files")
    for filename in ARTIFACT_FILES:
        digest = _require_digest(manifest.get(filename), f"artifact-sha256.json {filename}")
        if digest != sha256_file(results_dir / filename):
            raise ValueError(f"artifact-sha256.json {filename} does not match {filename}")


def validate_bundle(results_dir: Path, verify_runtime_binary: Path | None = None) -> tuple[int, int, int]:
    validate_required_files(results_dir)
    records = load_raw_records(results_dir / "raw.jsonl")
    raw_totals = validate_raw_semantics(records)
    summary = load_json_object(results_dir / "summary.json", "summary.json")
    report = (results_dir / "report.md").read_text(encoding="utf-8")
    validate_summary_and_report(records, summary, report)
    legacy_setups = validate_fixture_load(load_json_object(results_dir / "fixture-load.json", "fixture-load.json"))
    validate_correctness(load_json_object(results_dir / "correctness-summary.json", "correctness-summary.json"))
    validate_environment(
        load_json_object(results_dir / "environment.json", "environment.json"), results_dir, verify_runtime_binary
    )
    validate_artifact_hashes(results_dir)
    return raw_totals["iterator_builds"], raw_totals["attempt_count"], legacy_setups


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Check instruction-wide frame-noise evidence bundle.")
    parser.add_argument("--dir", type=Path, required=True)
    parser.add_argument("--verify-runtime-binary", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        builds, attempts, legacy_setups = validate_bundle(args.dir, args.verify_runtime_binary)
    except (OSError, ValueError, runner.RunnerError) as error:
        print(error, file=sys.stderr)
        return 1
    print(
        "PASS instruction-wide frame-noise evidence "
        f"builds={builds} attempts={attempts} legacy_setups={legacy_setups}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
