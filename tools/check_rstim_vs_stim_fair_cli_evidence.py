#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_fair_cli


REQUIRED_FILES = (
    "raw.jsonl",
    "summary.json",
    "baseline-summary.json",
    "comparison.json",
    "report.md",
    "environment.json",
    "artifact-sha256.json",
)
ARTIFACT_FILES = REQUIRED_FILES[:-1]
BASELINE_SUMMARY_SHA256 = "131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07"
BASELINE_RATIO = 3.576
REFERENCE_SUMMARY_REPO_PATH = run_fair_cli.REFERENCE_SUMMARY_REPO_PATH
REFERENCE_VARIANT = run_fair_cli.REFERENCE_VARIANT
REFERENCE_STRATEGY = run_fair_cli.REFERENCE_STRATEGY
REFERENCE_CHECKER = run_fair_cli.REFERENCE_CHECKER
PARITY_WORD_RE = re.compile(r"\bparity\b", re.IGNORECASE)
VARIANTS = ("stim-cli-b8", "rstim-cli-b8")
CANONICAL_FAIR_MANIFEST_PATH = "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml"
CANONICAL_SOURCE_MANIFEST_PATH = fair_cli_contract.EXPECTED_CASE["source_manifest_path"]
CANONICAL_FIXTURE_PATH = fair_cli_contract.EXPECTED_CASE["canonical_input_path"]
CANONICAL_SOURCE_MANIFEST = REPO_ROOT / CANONICAL_SOURCE_MANIFEST_PATH
CANONICAL_FIXTURE = REPO_ROOT / CANONICAL_FIXTURE_PATH
OLD_FULL_SUMMARY = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
OLD_FULL_SUMMARY_SHA256 = "97ae397e598fe447d206c6b07a26ceaa0a3336d1883a7f77bc194f7b4c491805"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {
    "stim-cli-b8": "tool://stim",
    "rstim-cli-b8": "tool://rstim",
}
EXPECTED_RUNTIME_IDENTITIES = (
    {
        "role": "tool://stim",
        "version": "1.15.0",
        "basename": "stim",
        "sha256": "e7f31b9ac1780080161b3992e70644ade97dbe97369a9464997645c437a29323",
    },
    {
        "role": "tool://rstim",
        "version": "rstim 0.1.1",
        "basename": "rstim",
        "sha256": "cae438197a15395cb397141a75d8a593b6ed502ffe6d8b7e0f548eea7f20a429",
    },
)
LIVE_RUNTIME_PATH_FIELDS = frozenset(
    {"stim_binary", "stim_binary_sha256", "rstim_binary", "rstim_binary_sha256"}
)
POSIX_ABSOLUTE_RE = re.compile(r"(^|[\s\"'=,:\[\(\{;|&<>])/(?!/)")
WINDOWS_ABSOLUTE_RE = re.compile(r"(^|[\s\"'=,:\[\(\{;|&<>])([A-Za-z]:[\\/]|\\\\)")


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
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ValueError(f"could not read raw.jsonl: {error}") from error
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


def require_equal(actual: Any, expected: Any, message: str) -> None:
    if actual != expected:
        raise ValueError(message)


def contains_host_absolute_path(value: object) -> bool:
    if isinstance(value, str):
        return (
            PurePosixPath(value).is_absolute()
            or bool(PureWindowsPath(value).drive and PureWindowsPath(value).is_absolute())
            or POSIX_ABSOLUTE_RE.search(value) is not None
            or WINDOWS_ABSOLUTE_RE.search(value) is not None
        )
    if isinstance(value, list):
        return any(contains_host_absolute_path(item) for item in value)
    if isinstance(value, tuple):
        return any(contains_host_absolute_path(item) for item in value)
    if isinstance(value, dict):
        return any(
            contains_host_absolute_path(key) or contains_host_absolute_path(item)
            for key, item in value.items()
        )
    return False


def _expected_argv(variant: str, *, seed: int) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        str(case["shots"]),
        "--seed",
        str(seed),
        "--out_format",
        case["output_format"],
        "--in",
        case["canonical_input_path"],
    ]


def _expected_preflight_argv(variant: str) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        "1",
        "--seed",
        "0",
        "--out_format",
        case["output_format"],
        "--in",
        KNOWN_ANSWER_INPUT_TOKEN,
    ]


def validate_raw_semantics(records: list[dict[str, Any]]) -> None:
    case = fair_cli_contract.EXPECTED_CASE
    derived_bytes_per_shot = (case["measurement_count"] + 7) // 8
    if derived_bytes_per_shot != case["bytes_per_shot"] or derived_bytes_per_shot != 1516:
        raise ValueError("canonical measurement count must derive 1516 bytes per shot")
    derived_output_bytes = derived_bytes_per_shot * case["shots"]
    if derived_output_bytes != case["expected_output_bytes"] or derived_output_bytes != 1552384:
        raise ValueError("canonical bytes per shot and shots must derive 1552384 bytes per run")
    if len(records) != 18:
        raise ValueError("raw.jsonl must contain exactly 18 records")
    if set(record.get("variant") for record in records) != set(VARIANTS):
        raise ValueError("raw.jsonl variants must be stim-cli-b8 and rstim-cli-b8")

    for variant in VARIANTS:
        variant_records = [record for record in records if record.get("variant") == variant]
        if len(variant_records) != 9:
            raise ValueError(f"{variant} must contain exactly 9 records")
        expected_phases = ["warmup", "warmup"] + ["measured"] * 7
        if [record.get("phase") for record in variant_records] != expected_phases:
            raise ValueError(f"{variant} phases must be two warmups followed by seven measured records")
        if [record.get("round_index") for record in variant_records] != [0, 1] + list(range(7)):
            raise ValueError(f"{variant} round indexes must be 0,1 then 0 through 6")
        if [record.get("seed") for record in variant_records] != list(range(9)):
            raise ValueError(f"{variant} seeds must be 0 through 8")

        for record in variant_records:
            for field in ("case_id", "shots", "measurement_count", "output_format", "timer_scope"):
                require_equal(record.get(field), case[field], f"{variant} {field} must be {case[field]}")
            require_equal(
                record.get("actual_output_bytes"),
                derived_output_bytes,
                f"{variant} actual_output_bytes must be {derived_output_bytes} "
                f"({derived_bytes_per_shot} bytes per shot * {case['shots']} shots)",
            )
            require_equal(record.get("exit_code"), 0, f"{variant} exit_code must be 0")
            if not isinstance(record.get("elapsed_ns"), int) or isinstance(record["elapsed_ns"], bool):
                raise ValueError(f"{variant} elapsed_ns must be an integer")
            stdout_sha256 = record.get("stdout_sha256")
            if not isinstance(stdout_sha256, str) or SHA256_RE.fullmatch(stdout_sha256) is None:
                raise ValueError(f"{variant} stdout_sha256 must be a lowercase SHA-256 digest")
            argv = record.get("argv")
            if contains_host_absolute_path(argv):
                raise ValueError(f"{variant} argv contains a host-absolute path")
            expected_argv = _expected_argv(variant, seed=record["seed"])
            require_equal(argv, expected_argv, f"{variant} argv must match canonical argv")


def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    return run_fair_cli._summary(records, case=fair_cli_contract.EXPECTED_CASE)


def render_report(summary: dict[str, Any], comparison: dict[str, Any]) -> str:
    return run_fair_cli._render_report(summary, comparison)


def validate_baseline_and_candidate(results_dir: Path, candidate_summary: dict[str, Any]) -> dict[str, Any]:
    baseline_path = results_dir / "baseline-summary.json"
    if sha256_file(baseline_path) != BASELINE_SUMMARY_SHA256:
        raise ValueError("baseline-summary.json SHA-256 must match pinned pre-optimization summary")
    if sha256_file(results_dir / "summary.json") == BASELINE_SUMMARY_SHA256:
        raise ValueError("candidate summary must differ from pinned baseline summary")
    baseline = load_json_object(baseline_path, "baseline-summary.json")
    if run_fair_cli._rounded_ratio(run_fair_cli._rstim_over_stim(baseline)) != BASELINE_RATIO:
        raise ValueError("baseline_rstim_over_stim must be 3.576")
    return baseline


def validate_reference_evidence(reference_evidence: object) -> dict[str, str]:
    if not isinstance(reference_evidence, dict):
        raise ValueError("environment reference_evidence must be an object")
    expected = {
        "slot": "reference-build-release",
        "summary_path": REFERENCE_SUMMARY_REPO_PATH,
        "reference_variant": REFERENCE_VARIANT,
        "reference_strategy": REFERENCE_STRATEGY,
        "checker": REFERENCE_CHECKER,
    }
    for field, value in expected.items():
        require_equal(
            reference_evidence.get(field),
            value,
            f"environment reference_evidence {field} must be {value}",
        )
    if contains_host_absolute_path(reference_evidence):
        raise ValueError("environment reference_evidence contains a host-absolute path")
    digest = reference_evidence.get("summary_sha256")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError("environment reference_evidence summary_sha256 must be a lowercase SHA-256 digest")
    reference_path = REPO_ROOT / REFERENCE_SUMMARY_REPO_PATH
    if sha256_file(reference_path) != digest:
        raise ValueError("reference_evidence summary_sha256 does not match reference summary")
    reference_summary = load_json_object(reference_path, "reference summary")
    direct = next(
        (
            item
            for item in reference_summary.get("variants", [])
            if isinstance(item, dict) and item.get("variant") == REFERENCE_VARIANT
        ),
        None,
    )
    if direct is None or direct.get("backend") != REFERENCE_STRATEGY:
        raise ValueError("reference summary must record direct_inverse_repeat_folded strategy")
    return {key: str(value) for key, value in reference_evidence.items()}


def validate_comparison(
    results_dir: Path,
    baseline_summary: dict[str, Any],
    candidate_summary: dict[str, Any],
    reference_evidence: dict[str, str],
) -> dict[str, Any]:
    expected = run_fair_cli._comparison(baseline_summary, candidate_summary, reference_evidence)
    actual = load_json_object(results_dir / "comparison.json", "comparison.json")
    if actual != expected:
        raise ValueError("comparison.json does not match comparison derived from baseline and candidate summaries")
    if actual["baseline_rstim_over_stim"] != BASELINE_RATIO:
        raise ValueError("comparison.json baseline_rstim_over_stim must be 3.576")
    if actual["reference_strategy"] != REFERENCE_STRATEGY:
        raise ValueError("comparison.json reference_strategy must be direct_inverse_repeat_folded")
    return actual


def _string_values(value: object) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [item for entry in value for item in _string_values(entry)]
    if isinstance(value, dict):
        return [item for entry in value.values() for item in _string_values(entry)]
    return []


def validate_no_unsupported_parity_claim(report_text: str, comparison: dict[str, Any]) -> None:
    candidate_ratio = comparison["candidate_rstim_over_stim"]
    if candidate_ratio <= 1.0:
        return
    checked_text = [report_text, *_string_values(comparison)]
    if any(PARITY_WORD_RE.search(text) for text in checked_text):
        raise ValueError("unsupported parity claim while candidate ratio exceeds 1.0")


def _resolve_recorded_path(raw: object, label: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise ValueError(f"environment {label} must be a nonempty path")
    path = Path(raw)
    return path.resolve() if path.is_absolute() else (REPO_ROOT / path).resolve()


def _validate_path_hash(environment: dict[str, Any], path_field: str, hash_field: str) -> None:
    path = _resolve_recorded_path(environment.get(path_field), path_field)
    if not path.is_file():
        raise ValueError(f"environment {path_field} does not exist: {environment.get(path_field)}")
    expected_hash = environment.get(hash_field)
    if not isinstance(expected_hash, str) or SHA256_RE.fullmatch(expected_hash) is None:
        raise ValueError(f"environment {hash_field} must be a lowercase SHA-256 digest")
    if sha256_file(path) != expected_hash:
        raise ValueError(f"environment {hash_field} does not match {path_field}")


def _validate_runtime_identities(environment: dict[str, Any]) -> None:
    forbidden = sorted(set(environment) & LIVE_RUNTIME_PATH_FIELDS)
    if forbidden:
        raise ValueError("environment must not contain live runtime path fields")
    identities = environment.get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError("environment runtime_identities must be a list")
    if identities != list(EXPECTED_RUNTIME_IDENTITIES):
        raise ValueError("environment runtime_identities must match canonical runtime identities")
    for identity in identities:
        if set(identity) != {"role", "version", "basename", "sha256"}:
            raise ValueError(
                "environment runtime identity must contain only role, version, basename, and sha256"
            )
        if contains_host_absolute_path(identity):
            raise ValueError("environment runtime identity contains a host-absolute path")
        digest = identity.get("sha256")
        if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
            raise ValueError("environment runtime identity sha256 must be a lowercase SHA-256 digest")


def _validate_preflight_detail(variant: str, detail: dict[str, Any]) -> None:
    require_equal(detail.get("variant"), variant, f"{variant} known-answer preflight variant must be {variant}")
    argv = detail.get("argv")
    if contains_host_absolute_path(argv):
        raise ValueError(f"{variant} known-answer preflight argv contains a host-absolute path")
    require_equal(
        argv,
        _expected_preflight_argv(variant),
        f"{variant} known-answer preflight argv must match canonical shape",
    )
    require_equal(detail.get("exit_code"), 0, f"{variant} known-answer preflight exit_code must be 0")
    require_equal(detail.get("stdout_hex"), "01", f"{variant} known-answer preflight stdout_hex must be 01")
    digest = detail.get("stdout_sha256")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError(f"{variant} known-answer preflight stdout_sha256 must be a lowercase SHA-256 digest")
    if digest != hashlib.sha256(bytes.fromhex(detail["stdout_hex"])).hexdigest():
        raise ValueError(f"{variant} known-answer preflight stdout_sha256 must hash stdout_hex")
    elapsed_ns = detail.get("elapsed_ns")
    if not isinstance(elapsed_ns, int) or isinstance(elapsed_ns, bool) or elapsed_ns < 0:
        raise ValueError(f"{variant} known-answer preflight elapsed_ns must be a nonnegative integer")


def validate_environment(environment: dict[str, Any], records: list[dict[str, Any]]) -> dict[str, str]:
    required_nonempty = ("git_commit", "os", "cpu_model", "rstim_version", "rustc_version")
    for field in required_nonempty:
        if not isinstance(environment.get(field), str) or not environment[field]:
            raise ValueError(f"environment {field} must be nonempty")
    case = fair_cli_contract.EXPECTED_CASE
    for field, expected in (
        ("profile", "release"),
        ("timer_scope", "cli_end_to_end"),
        ("seed_policy", "round_index_0_through_8"),
        ("stim_version", "1.15.0"),
        ("warmup_rounds", 2),
        ("measure_rounds", 7),
        ("known_answer_preflight", "passed"),
    ):
        require_equal(environment.get(field), expected, f"environment {field} must be {expected}")
    _validate_runtime_identities(environment)

    for path_field in ("manifest", "fair_manifest_path"):
        require_equal(
            environment.get(path_field),
            CANONICAL_FAIR_MANIFEST_PATH,
            f"environment {path_field} must be {CANONICAL_FAIR_MANIFEST_PATH}",
        )

    for path_field, expected in (
        ("source_manifest_path", CANONICAL_SOURCE_MANIFEST_PATH),
        ("fixture_path", CANONICAL_FIXTURE_PATH),
    ):
        require_equal(
            environment.get(path_field),
            expected,
            f"environment {path_field} must be {expected}",
        )

    for path_field, hash_field in (
        ("fair_manifest_path", "fair_manifest_sha256"),
        ("source_manifest_path", "source_manifest_sha256"),
        ("fixture_path", "fixture_sha256"),
    ):
        _validate_path_hash(environment, path_field, hash_field)

    aliases = (
        ("manifest", "manifest_sha256", "fair_manifest_path", "fair_manifest_sha256"),
        ("source_manifest", "source_manifest_sha256", "source_manifest_path", "source_manifest_sha256"),
        ("fixture", "fixture_sha256", "fixture_path", "fixture_sha256"),
    )
    for alias_path, alias_hash, path_field, hash_field in aliases:
        require_equal(environment.get(alias_path), environment.get(path_field), f"environment {alias_path} must match {path_field}")
        require_equal(environment.get(alias_hash), environment.get(hash_field), f"environment {alias_hash} must match {hash_field}")

    expected_round_argv = [
        {key: record[key] for key in ("variant", "phase", "round_index", "seed", "argv")}
        for record in records
    ]
    if contains_host_absolute_path(environment.get("round_argv")):
        raise ValueError("environment round_argv contains a host-absolute path")
    require_equal(environment.get("round_argv"), expected_round_argv, "environment round_argv must mirror raw.jsonl")
    details = environment.get("known_answer_preflight_details")
    if not isinstance(details, list) or len(details) != 2:
        raise ValueError("environment known_answer_preflight_details must contain both variants")
    by_variant = {detail.get("variant"): detail for detail in details if isinstance(detail, dict)}
    if set(by_variant) != set(VARIANTS):
        raise ValueError("environment known_answer_preflight_details must contain both variants")
    for variant in VARIANTS:
        _validate_preflight_detail(variant, by_variant[variant])

    require_equal(environment.get("timer_scope"), case["timer_scope"], "environment timer_scope must match case")
    return validate_reference_evidence(environment.get("reference_evidence"))


def validate_historical_separation(summary_path: Path) -> None:
    if sha256_file(OLD_FULL_SUMMARY) != OLD_FULL_SUMMARY_SHA256:
        raise ValueError("historical full speed-summary.json hash does not match pinned #406 digest")
    if sha256_file(summary_path) == OLD_FULL_SUMMARY_SHA256:
        raise ValueError("summary.json must not reuse the historical full speed summary")


def validate_artifact_hashes(results_dir: Path) -> None:
    hashes = load_json_object(results_dir / "artifact-sha256.json", "artifact-sha256.json")
    if set(hashes) != set(ARTIFACT_FILES):
        raise ValueError(
            "artifact-sha256.json must map exactly raw.jsonl, summary.json, baseline-summary.json, "
            "comparison.json, report.md, and environment.json"
        )
    for filename in ARTIFACT_FILES:
        digest = hashes[filename]
        if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
            raise ValueError(f"artifact-sha256.json {filename} must be a lowercase SHA-256 digest")
        if digest != sha256_file(results_dir / filename):
            raise ValueError(f"artifact-sha256.json digest does not match {filename}")


def validate_bundle(results_dir: Path) -> dict[str, Any]:
    validate_required_files(results_dir)
    environment = load_json_object(results_dir / "environment.json", "environment.json")
    records = load_raw_records(results_dir / "raw.jsonl")
    validate_raw_semantics(records)
    summary = derive_summary(records)
    baseline_summary = validate_baseline_and_candidate(results_dir, summary)
    if load_json_object(results_dir / "summary.json", "summary.json") != summary:
        raise ValueError("summary.json does not match summary derived from raw.jsonl")
    reference_evidence = validate_environment(environment, records)
    comparison = validate_comparison(results_dir, baseline_summary, summary, reference_evidence)
    report_text = (results_dir / "report.md").read_text(encoding="utf-8")
    validate_no_unsupported_parity_claim(report_text, comparison)
    if report_text != render_report(summary, comparison):
        raise ValueError("report.md does not match report derived from raw.jsonl")
    validate_historical_separation(results_dir / "summary.json")
    validate_artifact_hashes(results_dir)
    return {
        "variants": len(VARIANTS),
        "measured": summary["measured_record_count"],
        "baseline_rstim_over_stim": comparison["baseline_rstim_over_stim"],
        "candidate_rstim_over_stim": comparison["candidate_rstim_over_stim"],
        "reference_strategy": comparison["reference_strategy"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate checked fair CLI sampling evidence.")
    parser.add_argument("--dir", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        result = validate_bundle(args.dir)
    except Exception as error:
        print(str(error), file=sys.stderr)
        return 1
    print(f"PASS fair CLI sampling evidence variants={result['variants']} measured={result['measured']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
