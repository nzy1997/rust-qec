#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_compiled_steady


REQUIRED_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json", "artifact-sha256.json")
ARTIFACT_FILES = REQUIRED_FILES[:-1]
RAW_VARIANTS = ("stim", "rstim")
RELEASE_VARIANTS = {"stim": "stim-compiled-steady-b8", "rstim": "rstim-compiled-steady-b8"}
CANONICAL_FAIR_MANIFEST = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml"
CANONICAL_STIM_WORKER_MODULE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/workers/stim_compiled_steady.py"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
WINDOWS_ABSOLUTE_RE = re.compile(r"[A-Za-z]:[\\/]")
REQUIRED_RUNTIME_IDENTITY_ROLES = (
    "tool://python",
    "tool://stim-extension",
    "tool://stim-worker",
    "tool://rstim-worker",
)
RUNTIME_IDENTITY_KEYS = {"role", "version", "basename", "sha256"}
ENVIRONMENT_KEYS = {
    "git_commit",
    "os",
    "cpu_model",
    "profile",
    "timer_scope",
    "seed_policy",
    "stim_version",
    "stim_python_probe",
    "rstim_version",
    "rustc_version",
    "fair_manifest_path",
    "fair_manifest_sha256",
    "source_manifest_path",
    "source_manifest_sha256",
    "fixture_path",
    "fixture_sha256",
    "worker_argv",
    "canonical_worker_argv",
    "stim_worker_module_path",
    "stim_worker_module_sha256",
    "runtime_identities",
    "protocol_version",
    "seed",
    "warmup_rounds",
    "measure_rounds",
    "known_answer_preflight",
    "workers",
    "lifecycle",
}
OBSOLETE_LIVE_ENVIRONMENT_KEYS = {
    "python_executable",
    "loaded_stim_extension_path",
    "rstim_worker_binary_path",
    "python_executable_sha256",
    "loaded_stim_extension_sha256",
    "rstim_worker_binary_sha256",
}
STIM_PYTHON_PROBE_KEYS = {"status", "version", "extension_module"}
OBSOLETE_LIVE_STIM_PYTHON_PROBE_KEYS = {"path", "package_path", "sha256"}


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


def _release_variant(variant: str) -> str:
    return RELEASE_VARIANTS[variant]


def _require_equal(actual: Any, expected: Any, message: str) -> None:
    if actual != expected:
        raise ValueError(f"{message}, got {actual!r}")


def _require_int_equal(actual: Any, expected: int, message: str) -> None:
    if not isinstance(actual, int) or isinstance(actual, bool):
        raise ValueError(f"{message} must be integer {expected}, got {actual!r}")
    if actual != expected:
        raise ValueError(f"{message} must be {expected}, got {actual!r}")


def _require_json_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def _repo_relative_path(raw: Any, field: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise ValueError(f"{field} must be repository-relative")
    path = PurePosixPath(raw)
    if path.is_absolute() or "\\" in raw or any(part in {"", ".", ".."} for part in raw.split("/")):
        raise ValueError(f"{field} must be repository-relative")
    return REPO_ROOT / path


def _portable_worker_argv(role: str, input_path: str) -> list[str]:
    if role == "stim":
        return [
            "tool://python",
            "-m",
            "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
            "--input",
            input_path,
            "--seed",
            "0",
        ]
    return ["tool://rstim-worker", "--input", input_path, "--seed", "0"]


def _contains_host_absolute_path(value: Any) -> bool:
    if isinstance(value, str):
        return value.startswith(("/", "\\")) or WINDOWS_ABSOLUTE_RE.match(value) is not None
    if isinstance(value, list):
        return any(_contains_host_absolute_path(item) for item in value)
    return False


def _validate_telemetry(
    telemetry: Any,
    *,
    variant: str,
    stage: str,
    fixture_sha256: str | None,
    sample_call_count: int,
) -> None:
    label = _release_variant(variant)
    if not isinstance(telemetry, dict):
        raise ValueError(f"{label} {stage} telemetry must be a JSON object")
    _require_equal(telemetry.get("variant"), variant, f"{label} {stage} variant must be {variant}")
    for field, expected in (
        ("compile_count", 1),
        ("reference_build_count", 1),
        ("sample_call_count", sample_call_count),
        ("measurement_count", fair_cli_contract.EXPECTED_CASE["measurement_count"]),
        ("bytes_per_shot", fair_cli_contract.EXPECTED_CASE["bytes_per_shot"]),
    ):
        _require_int_equal(telemetry.get(field), expected, f"{label} {stage} {field}")
    digest = telemetry.get("fixture_sha256")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError(f"{label} {stage} fixture_sha256 must be a lowercase SHA-256 digest")
    if fixture_sha256 is not None:
        _require_equal(digest, fixture_sha256, f"{label} {stage} fixture_sha256 must match ready telemetry")


def validate_raw_semantics(records: list[dict[str, Any]]) -> dict[str, Any]:
    if set(record.get("variant") for record in records) != set(RAW_VARIANTS):
        raise ValueError("raw.jsonl variants must be stim and rstim")

    case = fair_cli_contract.EXPECTED_CASE
    lifecycle: dict[str, dict[str, int]] = {}
    for variant in RAW_VARIANTS:
        label = _release_variant(variant)
        variant_records = [record for record in records if record.get("variant") == variant]
        ready_records = [record for record in variant_records if record.get("record_type") == "ready"]
        sample_records = [record for record in variant_records if record.get("record_type") == "sample"]
        final_records = [record for record in variant_records if record.get("record_type") == "final"]
        if len(ready_records) != 1:
            raise ValueError(f"{label} must contain exactly one ready record")
        if len(sample_records) != 9:
            raise ValueError(f"{label} must contain exactly 9 sample records")
        if len(final_records) != 1:
            raise ValueError(f"{label} must contain exactly one final record")
        if len(variant_records) != 11:
            raise ValueError(f"{label} must contain exactly 11 lifecycle records")
        if [record.get("record_type") for record in variant_records] != ["ready"] + ["sample"] * 9 + ["final"]:
            raise ValueError(f"{label} records must appear as ready, nine samples, then final")

        ready = ready_records[0]
        _validate_telemetry(
            ready.get("telemetry"), variant=variant, stage="ready", fixture_sha256=None, sample_call_count=0
        )
        ready_telemetry = ready["telemetry"]
        request_ids = [record.get("request_id") for record in sample_records]
        if any(not isinstance(request_id, int) or isinstance(request_id, bool) for request_id in request_ids):
            raise ValueError(f"{label} request IDs must be integers 0 through 8")
        if request_ids != list(range(9)):
            raise ValueError(f"{label} request IDs must be 0 through 8")
        if [record.get("warmup") for record in sample_records] != [True, True] + [False] * 7:
            raise ValueError(f"{label} samples must contain two warmups followed by seven measured records")
        for request_id, record in enumerate(sample_records):
            _require_int_equal(
                record.get("sample_call_count"), request_id + 1,
                f"{label} sample_call_count for request {request_id}",
            )
            _require_int_equal(
                record.get("shots"),
                case["shots"],
                f"{label} shots for request {request_id}",
            )
            _require_equal(
                record.get("output_format"),
                case["output_format"],
                f"{label} output_format for request {request_id} must be {case['output_format']}",
            )
            elapsed_ns = record.get("elapsed_ns")
            if not isinstance(elapsed_ns, int) or isinstance(elapsed_ns, bool) or elapsed_ns < 0:
                raise ValueError(f"{label} elapsed_ns for request {request_id} must be a nonnegative integer")
            _require_int_equal(
                record.get("output_bytes"), case["expected_output_bytes"],
                f"{label} output_bytes for request {request_id}",
            )
        _validate_telemetry(
            final_records[0].get("telemetry"),
            variant=variant,
            stage="final",
            fixture_sha256=ready_telemetry["fixture_sha256"],
            sample_call_count=9,
        )
        lifecycle[variant] = {"compile_count": 1, "reference_build_count": 1, "sample_call_count": 9}

    if len(records) != 22:
        raise ValueError("raw.jsonl must contain exactly 22 lifecycle records")
    return {"lifecycle": lifecycle, "measured_records": 14, "variants": 2}


def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    return run_compiled_steady._summary(records)


def render_report(summary: dict[str, Any]) -> str:
    return run_compiled_steady._render_report(summary)


def _resolve_environment_path(environment: dict[str, Any], field: str) -> Path:
    return _repo_relative_path(environment.get(field), field).resolve()


def _validate_path_hash(environment: dict[str, Any], path_field: str, hash_field: str) -> None:
    path = _resolve_environment_path(environment, path_field)
    if not path.is_file():
        raise ValueError(f"environment {path_field} does not exist: {environment.get(path_field)}")
    digest = environment.get(hash_field)
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError(f"environment {hash_field} must be a lowercase SHA-256 digest")
    if sha256_file(path) != digest:
        raise ValueError(f"environment {hash_field} does not match {path_field}")


def _validate_canonical_path(
    environment: dict[str, Any],
    field: str,
    canonical_path: Path,
    description: str,
) -> Path:
    path = _resolve_environment_path(environment, field)
    if path != canonical_path.resolve():
        raise ValueError(f"environment {field} must name the canonical {description}")
    return path


def _validate_fair_manifest_contract(path: Path) -> None:
    try:
        manifest = fair_cli_contract.load_manifest(path)
        case = fair_cli_contract.find_case(manifest, fair_cli_contract.EXPECTED_CASE["case_id"])
        errors = fair_cli_contract.validate_case(case, manifest_path=path, repo_root=REPO_ROOT)
    except Exception as error:
        raise ValueError(f"environment fair_manifest_path must contain the canonical fair manifest case: {error}") from error
    if errors:
        raise ValueError("environment fair_manifest_path must contain the canonical fair manifest case: " + "; ".join(errors))


def _validate_command(value: Any, label: str) -> list[str]:
    if isinstance(value, list) and _contains_host_absolute_path(value):
        raise ValueError("worker argv contains host-absolute path")
    if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{label} must be a nonempty string array")
    return value


def _expected_worker_argv(input_path: str) -> dict[str, list[str]]:
    return {variant: _portable_worker_argv(variant, input_path) for variant in RAW_VARIANTS}


def _validate_runtime_identities(environment: dict[str, Any], *, stim_worker_module_path: Path) -> None:
    identities = environment.get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError("environment runtime_identities must contain logical runtime identities")

    by_role: dict[str, dict[str, Any]] = {}
    for identity in identities:
        if not isinstance(identity, dict):
            raise ValueError("environment runtime_identities entries must be JSON objects")
        if identity.get("required_live_path") is True:
            raise ValueError("checked evidence must not require a live runtime path")
        if set(identity) != RUNTIME_IDENTITY_KEYS:
            raise ValueError(
                "environment runtime_identities entries must contain exactly role, version, basename, and sha256"
            )
        role = identity.get("role")
        version = identity.get("version")
        basename = identity.get("basename")
        digest = identity.get("sha256")
        if not isinstance(role, str) or not role:
            raise ValueError("environment runtime_identities role must be nonempty")
        if role in by_role:
            raise ValueError("environment runtime_identities must not contain duplicate roles")
        if not isinstance(version, str) or not version:
            raise ValueError(f"environment runtime identity {role} version must be nonempty")
        if not isinstance(basename, str) or not basename or "/" in basename or "\\" in basename:
            raise ValueError(f"environment runtime identity {role} basename must be a filename")
        if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
            raise ValueError(f"environment runtime identity {role} sha256 must be a lowercase SHA-256 digest")
        by_role[role] = identity

    if set(by_role) != set(REQUIRED_RUNTIME_IDENTITY_ROLES):
        raise ValueError("environment runtime_identities must contain exactly the required logical roles")
    _require_equal(
        by_role["tool://stim-extension"]["version"],
        "1.15.0",
        "environment runtime identity tool://stim-extension version must be '1.15.0'",
    )
    _require_equal(
        by_role["tool://stim-worker"]["version"],
        "1.15.0",
        "environment runtime identity tool://stim-worker version must be '1.15.0'",
    )
    _require_equal(
        by_role["tool://rstim-worker"]["version"],
        environment["rstim_version"],
        "environment runtime identity tool://rstim-worker version must match rstim_version",
    )
    _require_equal(
        by_role["tool://stim-worker"]["sha256"],
        sha256_file(stim_worker_module_path),
        "environment runtime identity tool://stim-worker sha256 must match stim_worker_module_path",
    )


def _validate_environment_keys(environment: dict[str, Any]) -> None:
    for field in sorted(set(environment) - ENVIRONMENT_KEYS):
        if field in OBSOLETE_LIVE_ENVIRONMENT_KEYS:
            raise ValueError(f"environment contains obsolete live runtime provenance field: {field}")
        raise ValueError(f"environment contains unsupported field: {field}")


def _validate_stim_python_probe(probe: Any, *, expected_version: str) -> None:
    if probe is None:
        return
    if not isinstance(probe, dict):
        raise ValueError("environment stim_python_probe must be a JSON object")
    for field in sorted(set(probe) - STIM_PYTHON_PROBE_KEYS):
        if field in OBSOLETE_LIVE_STIM_PYTHON_PROBE_KEYS:
            raise ValueError(f"environment stim_python_probe contains obsolete live runtime provenance field: {field}")
        raise ValueError(f"environment stim_python_probe contains unsupported field: {field}")
    _require_equal(probe.get("status"), "ok", "environment stim_python_probe status must be 'ok'")
    _require_equal(
        probe.get("version"),
        expected_version,
        f"environment stim_python_probe version must be {expected_version!r}",
    )


def _validate_preflight_telemetry(
    telemetry: Any,
    *,
    variant: str,
    stage: str,
    sample_call_count: int,
    fixture_sha256: str | None,
) -> str:
    payload = _require_json_object(telemetry, f"environment {variant} preflight {stage}")
    _require_equal(payload.get("variant"), variant, f"environment {variant} preflight {stage} variant must be {variant}")
    for field, expected in (
        ("compile_count", 1),
        ("reference_build_count", 1),
        ("sample_call_count", sample_call_count),
        ("measurement_count", 1),
        ("bytes_per_shot", 1),
    ):
        _require_int_equal(payload.get(field), expected, f"environment {variant} preflight {stage} {field}")
    digest = payload.get("fixture_sha256")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError(f"environment {variant} preflight {stage} fixture_sha256 must be a lowercase SHA-256 digest")
    if fixture_sha256 is not None:
        _require_equal(
            digest,
            fixture_sha256,
            f"environment {variant} preflight {stage} fixture_sha256 must match ready telemetry",
        )
    return digest


def _validate_preflight_argv(item: dict[str, Any], *, variant: str) -> None:
    argv = _validate_command(item.get("argv"), f"environment {variant} preflight argv")
    if argv != _portable_worker_argv(variant, "fixture://compiled-steady-known-answer"):
        raise ValueError(f"environment {variant} preflight argv must match canonical shape")


def validate_environment(environment: dict[str, Any], derived: dict[str, Any], records: list[dict[str, Any]]) -> None:
    case = fair_cli_contract.EXPECTED_CASE
    _validate_environment_keys(environment)
    for field in ("git_commit", "os", "cpu_model", "rstim_version", "rustc_version"):
        if not isinstance(environment.get(field), str) or not environment[field]:
            raise ValueError(f"environment {field} must be nonempty")
    for field, expected in (
        ("profile", "release"),
        ("timer_scope", case["timer_scope"]),
        ("seed_policy", "seed_once_then_advance_across_9_calls"),
        ("stim_version", case["stim_version"]),
        ("protocol_version", 1),
        ("seed", 0),
        ("warmup_rounds", 2),
        ("measure_rounds", 7),
    ):
        _require_equal(environment.get(field), expected, f"environment {field} must be {expected!r}")

    canonical_paths = (
        ("fair_manifest_path", CANONICAL_FAIR_MANIFEST, "fair manifest"),
        ("source_manifest_path", REPO_ROOT / case["source_manifest_path"], "source manifest"),
        ("fixture_path", REPO_ROOT / case["canonical_input_path"], "fixture"),
        ("stim_worker_module_path", CANONICAL_STIM_WORKER_MODULE, "Stim worker module"),
    )
    for field, canonical_path, description in canonical_paths:
        _validate_canonical_path(environment, field, canonical_path, description)
    fixture_input = environment.get("fixture_path")
    stim_worker_module_path = _resolve_environment_path(environment, "stim_worker_module_path")
    if environment.get("fixture_sha256") != case["canonical_input_sha256"]:
        raise ValueError("fixture_sha256 must match canonical fixture SHA-256")
    _validate_fair_manifest_contract(_resolve_environment_path(environment, "fair_manifest_path"))
    for path_field, hash_field in (
        ("fair_manifest_path", "fair_manifest_sha256"),
        ("source_manifest_path", "source_manifest_sha256"),
        ("fixture_path", "fixture_sha256"),
        ("stim_worker_module_path", "stim_worker_module_sha256"),
    ):
        _validate_path_hash(environment, path_field, hash_field)
    _validate_runtime_identities(environment, stim_worker_module_path=stim_worker_module_path)
    _validate_stim_python_probe(environment.get("stim_python_probe"), expected_version=case["stim_version"])

    worker_argv = environment.get("worker_argv")
    workers = environment.get("workers")
    if not isinstance(worker_argv, dict) or set(worker_argv) != set(RAW_VARIANTS):
        raise ValueError("environment worker_argv must contain stim and rstim")
    if not isinstance(workers, list) or len(workers) != 2:
        raise ValueError("environment workers must contain both variants")
    worker_commands: dict[Any, list[str]] = {}
    for worker in workers:
        if not isinstance(worker, dict):
            raise ValueError("environment workers entries must be JSON objects")
        worker_commands[worker.get("variant")] = _validate_command(
            worker.get("command"),
            f"environment workers {worker.get('variant')} command",
        )
    if worker_commands != worker_argv:
        raise ValueError("environment workers must match worker_argv")
    canonical_worker_argv = environment.get("canonical_worker_argv")
    if not isinstance(canonical_worker_argv, dict) or set(canonical_worker_argv) != set(RAW_VARIANTS):
        raise ValueError("environment canonical_worker_argv must contain stim and rstim")
    for variant in RAW_VARIANTS:
        _validate_command(worker_argv[variant], f"environment worker_argv {variant}")
        _validate_command(canonical_worker_argv[variant], f"environment canonical_worker_argv {variant}")
    if worker_argv != canonical_worker_argv:
        raise ValueError("environment worker_argv must match canonical_worker_argv")
    if canonical_worker_argv != _expected_worker_argv(str(fixture_input)):
        raise ValueError("environment canonical_worker_argv must match release worker commands")

    preflight = environment.get("known_answer_preflight")
    if not isinstance(preflight, list) or len(preflight) != 2:
        raise ValueError("environment known_answer_preflight must contain both variants")
    preflight_by_variant = {item.get("variant"): item for item in preflight if isinstance(item, dict)}
    if set(preflight_by_variant) != set(RAW_VARIANTS):
        raise ValueError("environment known_answer_preflight must contain stim and rstim")
    for variant in RAW_VARIANTS:
        item = preflight_by_variant[variant]
        _require_equal(item.get("result_hex"), "01", f"environment {variant} preflight result_hex must be '01'")
        _validate_preflight_argv(item, variant=variant)
        preflight_fixture_sha256 = _validate_preflight_telemetry(
            item.get("ready"),
            variant=variant,
            stage="ready",
            sample_call_count=0,
            fixture_sha256=None,
        )
        _validate_preflight_telemetry(
            item.get("final"),
            variant=variant,
            stage="final",
            sample_call_count=1,
            fixture_sha256=preflight_fixture_sha256,
        )

    expected_lifecycle = derived["lifecycle"]
    lifecycle = environment.get("lifecycle")
    if lifecycle is not None:
        lifecycle_payload = _require_json_object(lifecycle, "environment lifecycle")
        if set(lifecycle_payload) != {"compile_count", "reference_build_count", "sample_call_count"}:
            raise ValueError("environment lifecycle must contain exactly compile_count, reference_build_count, and sample_call_count")
        _require_int_equal(lifecycle_payload.get("compile_count"), 1, "environment lifecycle compile_count")
        _require_int_equal(lifecycle_payload.get("reference_build_count"), 1, "environment lifecycle reference_build_count")
        _require_int_equal(lifecycle_payload.get("sample_call_count"), 9, "environment lifecycle sample_call_count")
    for variant in RAW_VARIANTS:
        telemetry = next(record["telemetry"] for record in records if record.get("variant") == variant and record.get("record_type") == "ready")
        _require_equal(
            telemetry["fixture_sha256"], environment["fixture_sha256"],
            f"environment fixture_sha256 must match {variant} ready telemetry",
        )
        if expected_lifecycle[variant] != {"compile_count": 1, "reference_build_count": 1, "sample_call_count": 9}:
            raise ValueError(f"raw {variant} lifecycle is not canonical")


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


def validate_bundle(results_dir: Path) -> tuple[int, int, str]:
    validate_required_files(results_dir)
    records = load_raw_records(results_dir / "raw.jsonl")
    derived = validate_raw_semantics(records)
    summary = derive_summary(records)
    expected_summary = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if (results_dir / "summary.json").read_text(encoding="utf-8") != expected_summary:
        raise ValueError("summary.json does not match summary derived from raw.jsonl")
    if (results_dir / "report.md").read_text(encoding="utf-8") != render_report(summary):
        raise ValueError("report.md does not match report derived from raw.jsonl")
    environment = load_json_object(results_dir / "environment.json", "environment.json")
    validate_environment(environment, derived, records)
    validate_artifact_hashes(results_dir)
    return derived["variants"], derived["measured_records"], "1/1/9"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate compiled steady-state sampling evidence.")
    parser.add_argument("--dir", type=Path, required=True, dest="results_dir")
    args = parser.parse_args(argv)
    try:
        variants, measured, lifecycle = validate_bundle(args.results_dir)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    print(f"PASS compiled steady-state sampling evidence variants={variants} measured={measured} lifecycle={lifecycle}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
