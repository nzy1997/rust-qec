#!/usr/bin/env python3
"""Aggregate the deterministic rsmp v1 readiness checks."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_rsmp_fixture_catalog as catalog_checker
from tools import check_rsmp_v1_compression_evidence as compression_checker


PASS_LINE = (
    "PASS rsmp v1 readiness valid_cases=7 corruption_cases>=12 "
    "compatibility=1 compression=pass"
)
SCHEMA_VERSION = 1
GENERATED_BY = "tools/check_rsmp_v1_readiness.py"
CATALOG = Path("rstim/tests/fixtures/rsmp/catalog.json")
COMPAT_MANIFEST = Path("rstim/tests/fixtures/rsmp/v1/manifest.toml")
COMPAT_ARCHIVE = Path("rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp")
NORMATIVE_DOC = Path("rstim/doc/rsmp-v1.md")
CLI_DOC = Path("rstim/doc/rsmp-cli.md")
COMPRESSION_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/rsmp-v1")
CORRUPTION_SUMMARY_NAME = "corruption-summary.json"

REQUIRED_ROLES = (
    "nonzero_reference",
    "rank_zero",
    "dependent_detectors",
    "repeat_records",
    "observable_recovery",
    "loss_visible_measurements",
    "surface_d11_r100",
)
REQUIRED_ERROR_CODES = tuple(sorted(catalog_checker.ERROR_CODES))
ARTIFACT_HASH_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")
FOCUSED_CARGO_TESTS = (
    ("rsmp_format_contract", "rsmp_format_contract"),
    ("rsmp_measurement_transform", "rsmp_measurement_transform"),
    ("rsmp_archive_dense", "rsmp_archive_dense"),
    ("rsmp_archive_streaming", "rsmp_archive_streaming"),
    ("cli_rsmp_b8", "cli_rsmp_b8"),
    ("rsmp_result_format_interop", "rsmp_result_format_interop"),
    ("rsmp_limits_and_errors", "rsmp_limits_and_errors"),
    ("cli_rsmp_publication", "cli_rsmp_publication"),
    ("rsmp_corruption_corpus", "rsmp_corruption_corpus"),
    ("rsmp_v1_compatibility", "rsmp_v1_compatibility"),
)


EXPECTED_HELP_MODEL = {
    "schema_version": 1,
    "commands": [
        {
            "name": "pack_samples",
            "usage": "rstim pack_samples [OPTIONS] --circuit <CIRCUIT> --shots <SHOTS> --in <IN> --out <OUT>",
            "required_options": ["--circuit", "--shots", "--in", "--out"],
            "options": [
                {
                    "name": "--benchmark-telemetry-json",
                    "short": None,
                    "value": "BENCHMARK_TELEMETRY_JSON",
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--circuit",
                    "short": None,
                    "value": "CIRCUIT",
                    "required": True,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--shots",
                    "short": None,
                    "value": "SHOTS",
                    "required": True,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--in",
                    "short": None,
                    "value": "IN",
                    "required": True,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--in_format",
                    "short": None,
                    "value": "IN_FORMAT",
                    "required": False,
                    "default": "b8",
                    "allowed_values": ["01", "b8", "ptb64"],
                },
                {
                    "name": "--out",
                    "short": None,
                    "value": "OUT",
                    "required": True,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--help",
                    "short": "-h",
                    "value": None,
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
            ],
        },
        {
            "name": "unpack_samples",
            "usage": "rstim unpack_samples [OPTIONS] --circuit <CIRCUIT> --in <IN>",
            "required_options": ["--circuit", "--in"],
            "options": [
                {
                    "name": "--benchmark-telemetry-json",
                    "short": None,
                    "value": "BENCHMARK_TELEMETRY_JSON",
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--circuit",
                    "short": None,
                    "value": "CIRCUIT",
                    "required": True,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--in",
                    "short": None,
                    "value": "IN",
                    "required": True,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--measurements_out",
                    "short": None,
                    "value": "MEASUREMENTS_OUT",
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--measurements_out_format",
                    "short": None,
                    "value": "MEASUREMENTS_OUT_FORMAT",
                    "required": False,
                    "default": "b8",
                    "allowed_values": ["01", "b8", "r8", "hits", "ptb64"],
                },
                {
                    "name": "--detectors_out",
                    "short": None,
                    "value": "DETECTORS_OUT",
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--detectors_out_format",
                    "short": None,
                    "value": "DETECTORS_OUT_FORMAT",
                    "required": False,
                    "default": "b8",
                    "allowed_values": ["01", "b8", "r8", "hits", "ptb64", "dets"],
                },
                {
                    "name": "--obs_out",
                    "short": None,
                    "value": "OBS_OUT",
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--obs_out_format",
                    "short": None,
                    "value": "OBS_OUT_FORMAT",
                    "required": False,
                    "default": "b8",
                    "allowed_values": ["01", "b8", "r8", "hits", "ptb64"],
                },
                {
                    "name": "--verify_only",
                    "short": None,
                    "value": None,
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
                {
                    "name": "--help",
                    "short": "-h",
                    "value": None,
                    "required": False,
                    "default": None,
                    "allowed_values": None,
                },
            ],
        },
    ],
}


@dataclass
class ReadinessFailure(Exception):
    check: str
    diagnostic: str
    detail: str | None = None


class ReadinessContext:
    def __init__(self, repo_root: Path, out_dir: Path) -> None:
        self.repo_root = repo_root
        self.out_dir = out_dir
        self.logs_dir = out_dir / "logs"
        self.checked_commands: list[dict[str, Any]] = []

    def ensure_output_dirs(self) -> None:
        if self.logs_dir.is_symlink():
            raise ReadinessFailure(
                "readiness.output",
                "output path is unsafe",
                f"{self.logs_dir} is a symlink",
            )
        self.logs_dir.mkdir(parents=True, exist_ok=True)

    def run_child(self, check_id: str, argv: list[str]) -> subprocess.CompletedProcess[str]:
        self.ensure_output_dirs()
        log_path = self.logs_dir / f"{check_id}.log"
        result = subprocess.run(
            argv,
            cwd=self.repo_root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            env=os.environ.copy(),
        )
        log_text = [
            "$ " + " ".join(argv),
            f"exit_code={result.returncode}",
            "",
            "[stdout]",
            result.stdout,
            "[stderr]",
            result.stderr,
        ]
        log_path.write_text("\n".join(log_text), encoding="utf-8")
        self.checked_commands.append(
            {
                "check": check_id,
                "argv": argv,
                "exit_code": result.returncode,
                "log": str(log_path.relative_to(self.out_dir)),
            }
        )
        if result.returncode != 0:
            raise ReadinessFailure(
                check_id,
                f"{check_id} command failed",
                f"see {log_path}",
            )
        return result

    def run_probe(self, check_id: str, argv: list[str]) -> subprocess.CompletedProcess[str]:
        self.ensure_output_dirs()
        log_path = self.logs_dir / f"{check_id}.log"
        result = subprocess.run(
            argv,
            cwd=self.repo_root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            env=os.environ.copy(),
        )
        log_text = [
            "$ " + " ".join(argv),
            f"exit_code={result.returncode}",
            "",
            "[stdout]",
            result.stdout,
            "[stderr]",
            result.stderr,
        ]
        log_path.write_text("\n".join(log_text), encoding="utf-8")
        self.checked_commands.append(
            {
                "check": check_id,
                "argv": argv,
                "exit_code": result.returncode,
                "log": str(log_path.relative_to(self.out_dir)),
                "expected_failure": True,
            }
        )
        if result.returncode == 0:
            raise ReadinessFailure(
                check_id,
                f"{check_id} probe unexpectedly succeeded",
                f"see {log_path}",
            )
        return result


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def repo_path(repo_root: Path, relative: Path | str) -> Path:
    return repo_root / relative


def initial_report() -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "fail",
        "generated_by": GENERATED_BY,
        "checked_commands": [],
        "valid_catalog": {},
        "corruption": {},
        "compatibility": {},
        "compression": {},
        "documentation": {},
        "failed_checks": [],
    }


def append_failure(report: dict[str, Any], failure: ReadinessFailure) -> None:
    entry: dict[str, str] = {"check": failure.check, "diagnostic": failure.diagnostic}
    if failure.detail:
        entry["detail"] = failure.detail
    report["failed_checks"].append(entry)


def run_step(report: dict[str, Any], check: str, func: Any) -> None:
    try:
        func()
    except ReadinessFailure as failure:
        append_failure(report, failure)
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        append_failure(report, ReadinessFailure(check, f"{check} validation failed", str(error)))


def validate_catalog(repo_root: Path, report: dict[str, Any]) -> None:
    catalog_path = repo_path(repo_root, CATALOG)
    catalog = load_json(catalog_path)
    catalog_checker.validate_catalog(repo_root, catalog)
    cases = catalog.get("cases")
    if not isinstance(cases, list):
        raise ValueError("catalog cases must be an array")
    roles = {
        role
        for case in cases
        if isinstance(case, dict)
        for role in case.get("semantic_roles", [])
        if isinstance(role, str)
    }
    required_present = [role for role in REQUIRED_ROLES if role in roles]
    if len(required_present) != len(REQUIRED_ROLES):
        missing = sorted(set(REQUIRED_ROLES) - set(required_present))
        raise ReadinessFailure(
            "valid_catalog.required_roles",
            "fixture catalog is missing required semantic roles",
            ", ".join(missing),
        )
    report["valid_catalog"] = {
        "count": len(required_present),
        "required_role_count": len(required_present),
        "required_roles": required_present,
        "case_count": len(cases),
        "catalog_sha256": sha256_file(catalog_path),
    }


def validate_corruption(
    ctx: ReadinessContext,
    report: dict[str, Any],
    *,
    run_commands: bool,
) -> None:
    summary_path = ctx.out_dir / CORRUPTION_SUMMARY_NAME
    catalog = load_json(repo_path(ctx.repo_root, CATALOG))
    recipes = catalog.get("corruption_recipes")
    bit_flips = catalog.get("bit_flips")
    if not isinstance(recipes, list):
        raise ValueError("corruption_recipes must be an array")
    if not isinstance(bit_flips, list):
        raise ValueError("bit_flips must be an array")

    if run_commands:
        ctx.run_child(
            "rsmp_corruption_corpus_artifact",
            [
                "cargo",
                "run",
                "--locked",
                "--quiet",
                "-p",
                "rstim",
                "--example",
                "rsmp_corruption_corpus",
                "--",
                "--catalog",
                str(CATALOG),
                "--fixture-manifest",
                str(COMPAT_MANIFEST),
                "--out",
                str(summary_path),
            ],
        )
        summary = load_json(summary_path)
        if summary.get("status") != "pass":
            raise ReadinessFailure(
                "corruption.corpus",
                "corruption corpus did not pass",
                str(summary.get("status")),
            )
        named_recipe_count = int(summary.get("named_recipes", 0))
        generated_truncation_count = int(summary.get("truncation_points", 0))
        generated_bit_flip_count = int(summary.get("bit_flips", 0))
        summary_sha256 = sha256_file(summary_path)
    else:
        archive = repo_path(ctx.repo_root, COMPAT_ARCHIVE)
        named_recipe_count = len({recipe.get("id") for recipe in recipes if isinstance(recipe, dict)})
        generated_truncation_count = archive.stat().st_size
        generated_bit_flip_count = len(bit_flips)
        synthetic_summary = {
            "named_recipes": named_recipe_count,
            "truncation_points": generated_truncation_count,
            "bit_flips": generated_bit_flip_count,
        }
        summary_sha256 = sha256_bytes(canonical_json(synthetic_summary).encode("utf-8"))
    if named_recipe_count < 12:
        raise ReadinessFailure(
            "corruption.named_recipes",
            "corruption corpus has fewer than 12 named recipes",
            str(named_recipe_count),
        )
    report["corruption"] = {
        "named_recipe_count": named_recipe_count,
        "generated_truncation_count": generated_truncation_count,
        "generated_bit_flip_count": generated_bit_flip_count,
        "summary_sha256": summary_sha256,
    }


def validate_compatibility(repo_root: Path, report: dict[str, Any]) -> None:
    manifest_path = repo_path(repo_root, COMPAT_MANIFEST)
    archive_path = repo_path(repo_root, COMPAT_ARCHIVE)
    with manifest_path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if manifest.get("fixture_id") != "compat_v1_two_block_sparse_dense":
        raise ValueError("compatibility fixture_id mismatch")
    shape = manifest.get("shape")
    if not isinstance(shape, dict):
        raise ValueError("compatibility shape table missing")
    blocks = manifest.get("blocks")
    if not isinstance(blocks, list):
        raise ValueError("compatibility blocks table missing")
    codecs = [block.get("syndrome_codec") for block in blocks if isinstance(block, dict)]
    archive_sha256 = sha256_file(archive_path)
    hashes = manifest.get("hashes")
    if not isinstance(hashes, dict) or hashes.get("archive_sha256") != archive_sha256:
        raise ValueError("compatibility archive_sha256 mismatch")
    if int(shape.get("blocks", 0)) != 2 or len(blocks) != 2:
        raise ValueError("compatibility block_count must be 2")
    if codecs != ["sparse", "dense"]:
        raise ValueError("compatibility codecs must be ['sparse', 'dense']")
    report["compatibility"] = {
        "fixture_count": 1,
        "archive_sha256": archive_sha256,
        "block_count": 2,
        "codecs": codecs,
    }


def validate_compression(repo_root: Path, report: dict[str, Any]) -> None:
    results_dir = repo_path(repo_root, COMPRESSION_DIR)
    hashes_path = results_dir / "artifact-sha256.json"
    try:
        artifact_hashes = load_json(hashes_path)
    except (OSError, json.JSONDecodeError) as error:
        raise ReadinessFailure(
            "compression.input_hashes",
            "compression repository input hash is missing",
            str(error),
        ) from error
    missing_hashes = [name for name in ARTIFACT_HASH_FILES if name not in artifact_hashes]
    if missing_hashes:
        raise ReadinessFailure(
            "compression.input_hashes",
            "compression repository input hash is missing",
            missing_hashes[0],
        )
    try:
        compression_checker.check_bundle(results_dir, repo_root=repo_root)
    except ValueError as error:
        text = str(error)
        if "gate failure" in text:
            raise ReadinessFailure(
                "compression.gates",
                "compression acceptance gate failed",
                text,
            ) from error
        raise ReadinessFailure(
            "compression.bundle",
            "compression evidence validation failed",
            text,
        ) from error
    summary = load_json(results_dir / "summary.json")
    gates = summary.get("gates")
    if not isinstance(gates, dict):
        raise ValueError("compression summary gates missing")
    measured_gate_values = {
        name: {
            "lhs": gate.get("lhs"),
            "operator": gate.get("operator"),
            "rhs": gate.get("rhs"),
            "passed": gate.get("passed"),
        }
        for name, gate in gates.items()
        if isinstance(gate, dict)
    }
    report["compression"] = {
        "status": "pass",
        "evidence_sha256": sha256_file(hashes_path),
        "measured_gate_values": measured_gate_values,
    }


def capture_help(ctx: ReadinessContext, command: str) -> str:
    result = ctx.run_child(
        f"help_{command}",
        [
            "cargo",
            "run",
            "--locked",
            "--quiet",
            "-p",
            "rstim",
            "--bin",
            "rstim",
            "--",
            command,
            "--help",
        ],
    )
    return result.stdout


def normalized_help_model(
    pack_help: str,
    unpack_help: str,
    allowed_values_by_option: dict[tuple[str, str], list[str]],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "commands": [
            parse_help_command("pack_samples", pack_help, allowed_values_by_option),
            parse_help_command("unpack_samples", unpack_help, allowed_values_by_option),
        ],
    }


def parse_help_command(
    command: str,
    help_text: str,
    allowed_values_by_option: dict[tuple[str, str], list[str]],
) -> dict[str, Any]:
    usage = ""
    options: list[dict[str, Any]] = []
    for line in help_text.splitlines():
        if line.startswith("Usage: "):
            usage = line.removeprefix("Usage: ").strip()
        elif "--" in line and line[:2].strip() in {"", "-h"}:
            option = parse_option_line(line)
            if option is not None:
                options.append(option)
    if not usage:
        raise ValueError(f"{command} help is missing Usage")
    required_options = re.findall(r"(?<!\[)(--[a-z0-9_-]+) <[^>]+>", usage)
    for option in options:
        option["required"] = option["name"] in required_options
        option["allowed_values"] = allowed_values_by_option.get((command, option["name"]))
    return {
        "name": command,
        "usage": usage,
        "required_options": required_options,
        "options": options,
    }


def parse_option_line(line: str) -> dict[str, Any] | None:
    stripped = line.strip()
    match = re.match(r"(?:(-\w),\s*)?(--[a-zA-Z0-9_-]+)(?:\s+<([^>]+)>)?(.*)$", stripped)
    if match is None:
        return None
    short, name, value, rest = match.groups()
    default_match = re.search(r"\[default:\s*([^\]]+)\]", rest)
    return {
        "name": name,
        "short": short,
        "value": value,
        "required": False,
        "default": default_match.group(1) if default_match else None,
        "allowed_values": None,
    }


def capture_allowed_values(ctx: ReadinessContext) -> dict[tuple[str, str], list[str]]:
    probes: tuple[tuple[str, str, list[str]], ...] = (
        (
            "pack_samples",
            "--in_format",
            [
                "pack_samples",
                "--circuit",
                "circuit.stim",
                "--shots",
                "1",
                "--in",
                "measurements.b8",
                "--in_format",
                "__rsmp_readiness_invalid__",
                "--out",
                "archive.rsmp",
            ],
        ),
        (
            "unpack_samples",
            "--measurements_out_format",
            [
                "unpack_samples",
                "--circuit",
                "circuit.stim",
                "--in",
                "archive.rsmp",
                "--measurements_out",
                "measurements.b8",
                "--measurements_out_format",
                "__rsmp_readiness_invalid__",
            ],
        ),
        (
            "unpack_samples",
            "--detectors_out_format",
            [
                "unpack_samples",
                "--circuit",
                "circuit.stim",
                "--in",
                "archive.rsmp",
                "--detectors_out",
                "detectors.b8",
                "--detectors_out_format",
                "__rsmp_readiness_invalid__",
            ],
        ),
        (
            "unpack_samples",
            "--obs_out_format",
            [
                "unpack_samples",
                "--circuit",
                "circuit.stim",
                "--in",
                "archive.rsmp",
                "--obs_out",
                "observables.b8",
                "--obs_out_format",
                "__rsmp_readiness_invalid__",
            ],
        ),
    )
    values: dict[tuple[str, str], list[str]] = {}
    for command, option, argv_tail in probes:
        result = ctx.run_probe(
            f"cli_allowed_values_{command}_{option.removeprefix('--')}",
            [
                "cargo",
                "run",
                "--locked",
                "--quiet",
                "-p",
                "rstim",
                "--bin",
                "rstim",
                "--",
                *argv_tail,
            ],
        )
        diagnostic = result.stdout + "\n" + result.stderr
        values[(command, option)] = parse_allowed_values_diagnostic(command, option, diagnostic)
    return values


def parse_allowed_values_diagnostic(command: str, option: str, diagnostic: str) -> list[str]:
    match = re.search(r"must be one of ([0-9A-Za-z_, ]+)", diagnostic)
    if match is None:
        raise ValueError(f"{command} {option} diagnostic does not expose allowed values")
    return [value.strip() for value in match.group(1).split(",")]


def extract_section(text: str, heading: str) -> str:
    pattern = re.compile(rf"^## {re.escape(heading)}\s*$", re.MULTILINE)
    match = pattern.search(text)
    if match is None:
        raise ValueError(f"missing section {heading}")
    start = match.end()
    next_match = re.search(r"^## ", text[start:], re.MULTILINE)
    end = start + next_match.start() if next_match else len(text)
    return text[start:end]


def extract_documented_help_model(cli_doc: str) -> dict[str, Any]:
    section = extract_section(cli_doc, "Documented CLI Surface")
    match = re.search(r"```json\s*(.*?)\s*```", section, re.DOTALL)
    if match is None:
        raise ValueError("Documented CLI Surface must contain a JSON fence")
    value = json.loads(match.group(1))
    if not isinstance(value, dict):
        raise ValueError("documented CLI surface must be a JSON object")
    return value


def normalized_text(text: str) -> str:
    return " ".join(text.split())


def contains_phrase(section: str, phrase: str) -> bool:
    return normalized_text(phrase).lower() in normalized_text(section).lower()


def require_all(section: str, phrases: list[str], diagnostic: str) -> None:
    missing = [phrase for phrase in phrases if not contains_phrase(section, phrase)]
    if missing:
        raise ReadinessFailure("documentation.semantic_sections", diagnostic, missing[0])


def validate_documentation(
    ctx: ReadinessContext,
    report: dict[str, Any],
    *,
    run_commands: bool,
) -> None:
    normative_path = repo_path(ctx.repo_root, NORMATIVE_DOC)
    cli_doc_path = repo_path(ctx.repo_root, CLI_DOC)
    normative = normative_path.read_text(encoding="utf-8")
    cli_doc = cli_doc_path.read_text(encoding="utf-8")

    transform = extract_section(normative, "Circuit-Derived Lossless Transform")
    require_all(
        transform,
        ["lossless", "selected-detector", "free-measurement", "noiseless reference", "invertible"],
        "normative documentation does not explain the lossless circuit-derived transform",
    )
    binary = extract_section(normative, "Binary Fields and Canonical Encoding")
    require_all(
        binary,
        [
            "little-endian",
            "LSB-first",
            "ULEB128",
            "canonical",
            "zero padding",
            "header_sha256",
            "archive_sha256",
            "zero shots",
            "major 1, minor 0",
        ],
        "normative documentation does not cover required binary canonicality",
    )
    support = extract_section(normative, "Support Boundaries")
    sweep_sentence = (
        "Sweep-bit circuits are unsupported in v1 and must fail with "
        "`RSMP_UNSUPPORTED_SWEEP` before archive bytes are produced or trusted."
    )
    if not contains_phrase(support, sweep_sentence):
        raise ReadinessFailure(
            "documentation.support_boundaries",
            "normative documentation does not mark sweep-bit circuits unsupported",
        )
    require_all(
        support,
        [
            "original circuit is required",
            "DEM-only input is unsupported",
            "sequential access only",
            "no random shot access",
        ],
        "normative documentation does not cover rsmp v1 support boundaries",
    )
    integrity = extract_section(normative, "Integrity, Authentication, and Access Model")
    require_all(
        integrity,
        ["integrity", "not authenticated", "archive_sha256", "logical payload"],
        "normative documentation does not cover integrity and authentication boundaries",
    )
    limits = extract_section(normative, "Resource Limits and Validation Precedence")
    require_all(
        limits,
        ["validation precedence", "resource limits", "RSMP_LIMIT_EXCEEDED"],
        "normative documentation does not cover validation precedence",
    )
    taxonomy = extract_section(normative, "Stable Error Taxonomy")
    require_all(
        taxonomy,
        list(REQUIRED_ERROR_CODES),
        "normative documentation does not include the stable error taxonomy",
    )
    compatibility = extract_section(normative, "Compatibility Fixture Policy")
    require_all(
        compatibility,
        ["immutable", "additive", "compat-v1.rsmp", "sparse", "dense"],
        "normative documentation does not cover compatibility fixture policy",
    )
    compression = extract_section(normative, "Compression Evidence and Claim Limits")
    require_all(
        compression,
        [
            "benchmark_raw_lt_20pct",
            "benchmark_zstd_lt_75pct",
            "high_entropy_raw_le_102pct",
            "python3 tools/check_rsmp_v1_compression_evidence.py",
            "No fixed wall-clock performance gate",
        ],
        "normative documentation does not cover compression evidence gates",
    )
    operational = extract_section(cli_doc, "Operational Semantics")
    require_all(
        operational,
        [
            "atomic per file",
            "not a multi-file transaction",
            "already-published files are retained",
            "stdout cannot be rolled back",
            "already-verified prefix",
            "--verify_only",
            "recommended nondeveloper validation route",
        ],
        "CLI documentation does not cover operational publication semantics",
    )
    formats = extract_section(cli_doc, "Result Formats")
    require_all(
        formats,
        ["01", "b8", "ptb64", "r8", "hits", "dets"],
        "CLI documentation does not list all supported rsmp result formats",
    )

    if run_commands:
        help_model = normalized_help_model(
            capture_help(ctx, "pack_samples"),
            capture_help(ctx, "unpack_samples"),
            capture_allowed_values(ctx),
        )
    else:
        help_model = EXPECTED_HELP_MODEL
    documented = extract_documented_help_model(cli_doc)
    if documented != help_model:
        raise ReadinessFailure(
            "documentation.cli_surface",
            "documented CLI surface differs from rstim help",
        )
    report["documentation"] = {
        "normative_doc_sha256": sha256_file(normative_path),
        "cli_doc_sha256": sha256_file(cli_doc_path),
        "normalized_help_sha256": sha256_bytes(canonical_json(help_model).encode("utf-8")),
    }


def run_focused_commands(ctx: ReadinessContext) -> None:
    ctx.run_child(
        "fixture_catalog_checker",
        [
            sys.executable,
            "tools/check_rsmp_fixture_catalog.py",
            "--repo-root",
            ".",
            "--catalog",
            str(CATALOG),
        ],
    )
    for check_id, test_name in FOCUSED_CARGO_TESTS:
        ctx.run_child(
            check_id,
            [
                "cargo",
                "test",
                "--locked",
                "-p",
                "rstim",
                "--test",
                test_name,
                "--",
                "--nocapture",
            ],
        )
    ctx.run_child(
        "compression_evidence_checker",
        [
            sys.executable,
            "tools/check_rsmp_v1_compression_evidence.py",
            "--results-dir",
            str(COMPRESSION_DIR),
            "--repo-root",
            ".",
        ],
    )


def write_report(out_dir: Path, report: dict[str, Any]) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "readiness.json").write_text(canonical_json(report), encoding="utf-8")


def build_readiness_report(
    repo_root: Path,
    out_dir: Path,
    *,
    run_commands: bool,
) -> dict[str, Any]:
    ctx = ReadinessContext(repo_root, out_dir)
    ctx.ensure_output_dirs()
    report = initial_report()

    if run_commands:
        run_step(report, "focused_commands", lambda: run_focused_commands(ctx))

    run_step(report, "valid_catalog", lambda: validate_catalog(repo_root, report))
    run_step(report, "corruption", lambda: validate_corruption(ctx, report, run_commands=run_commands))
    run_step(report, "compatibility", lambda: validate_compatibility(repo_root, report))
    run_step(report, "compression", lambda: validate_compression(repo_root, report))
    run_step(
        report,
        "documentation",
        lambda: validate_documentation(ctx, report, run_commands=run_commands),
    )

    report["checked_commands"] = ctx.checked_commands
    if not run_commands:
        append_failure(
            report,
            ReadinessFailure(
                "readiness.commands",
                "readiness commands were skipped",
                "validation-only mode cannot produce the rsmp v1 readiness PASS line",
            ),
        )
    report["status"] = "pass" if not report["failed_checks"] else "fail"
    write_report(out_dir, report)
    return report


def prepare_output_dir(out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    for name in ("readiness.json", CORRUPTION_SUMMARY_NAME):
        path = out_dir / name
        if not path.exists():
            continue
        if path.is_file() or path.is_symlink():
            path.unlink()
        else:
            raise ReadinessFailure(
                "readiness.output",
                "output path is unsafe",
                f"{path} exists and is not a file",
            )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--out-dir", type=Path, default=Path("benchmarks/out/rsmp-v1"))
    parser.add_argument(
        "--skip-commands",
        action="store_true",
        help="skip child Cargo/Python commands and validate structured inputs only",
    )
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    out_dir = args.out_dir if args.out_dir.is_absolute() else repo_root / args.out_dir
    try:
        prepare_output_dir(out_dir)
        report = build_readiness_report(
            repo_root,
            out_dir,
            run_commands=not args.skip_commands,
        )
    except Exception as error:  # Last-resort artifact path for I/O/setup failures.
        report = initial_report()
        append_failure(
            report,
            ReadinessFailure("readiness", "readiness checker failed", str(error)),
        )
        try:
            write_report(out_dir, report)
        except OSError:
            pass
        print(f"not ready: readiness checker failed", file=sys.stderr)
        print(str(error), file=sys.stderr)
        return 1

    if report["status"] != "pass":
        for failure in report["failed_checks"]:
            print(f"not ready: {failure['diagnostic']}", file=sys.stderr)
            if failure.get("detail"):
                print(f"  {failure['check']}: {failure['detail']}", file=sys.stderr)
        return 1
    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
