from __future__ import annotations

import argparse
import hashlib
import sys
import tomllib
from pathlib import Path
from typing import Any


EXPECTED_CASE = {
    "case_id": "stim_surface_d11_r100",
    "source_manifest_path": "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
    "source_manifest_case_id": "stim_surface_d11_r100",
    "canonical_input_path": "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
    "canonical_input_sha256": "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229",
    "stim_version": "1.15.0",
    "shots": 1024,
    "measurement_count": 12121,
    "output_format": "b8",
    "bytes_per_shot": 1516,
    "expected_output_bytes": 1552384,
    "timer_scope": "cli_end_to_end",
    "seed_policy": "round_index_0_through_8",
}
EXPECTED_ARGV = {
    "stim-cli-b8": [
        "stim",
        "sample",
        "--shots",
        "{shots}",
        "--seed",
        "{seed}",
        "--out_format",
        "b8",
        "--in",
        "{canonical_input_path}",
    ],
    "rstim-cli-b8": [
        "{rstim_binary}",
        "sample",
        "--shots",
        "{shots}",
        "--seed",
        "{seed}",
        "--out_format",
        "b8",
        "--in",
        "{canonical_input_path}",
    ],
}


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be a TOML table")
    return manifest


def find_case(manifest: dict[str, Any], case_id: str) -> dict[str, Any]:
    cases = manifest.get("cases")
    if not isinstance(cases, list):
        raise ValueError('manifest field "cases" must be an array')
    matches = [case for case in cases if isinstance(case, dict) and case.get("case_id") == case_id]
    if not matches:
        raise ValueError(f'manifest case not found: "{case_id}"')
    if len(matches) != 1:
        raise ValueError(f'duplicate manifest case: "{case_id}"')
    return matches[0]


def expand_argv(
    template: list[str],
    case: dict[str, Any],
    *,
    seed: int = 0,
    rstim_binary: str = "rstim",
) -> list[str]:
    values = {
        "shots": case["shots"],
        "seed": seed,
        "canonical_input_path": case["canonical_input_path"],
        "rstim_binary": rstim_binary,
    }
    return [value.format_map(values) for value in template]


def _argv_option(argv: list[str], option: str, errors: list[str]) -> str | None:
    try:
        index = argv.index(option)
    except ValueError:
        errors.append(f'missing option "{option}"')
        return None
    if index + 1 >= len(argv):
        errors.append(f'missing value for option "{option}"')
        return None
    return argv[index + 1]


def _path_from_repo(repo_root: Path, raw_path: str) -> Path:
    return (repo_root / raw_path).resolve()


def validate_case(case: dict[str, Any], *, manifest_path: Path, repo_root: Path) -> list[str]:
    errors: list[str] = []
    for field, expected in EXPECTED_CASE.items():
        if case.get(field) != expected:
            errors.append(f'{field}: expected {expected!r}, got {case.get(field)!r}')

    measurement_count = case.get("measurement_count")
    shots = case.get("shots")
    valid_measurement_count = isinstance(measurement_count, int) and not isinstance(
        measurement_count, bool
    )
    valid_shots = isinstance(shots, int) and not isinstance(shots, bool)
    if not valid_measurement_count:
        errors.append(f'measurement_count: expected integer, got {measurement_count!r}')
    if not valid_shots:
        errors.append(f'shots: expected integer, got {shots!r}')
    if valid_measurement_count:
        expected_bytes_per_shot = (measurement_count + 7) // 8
        if case.get("bytes_per_shot") != expected_bytes_per_shot:
            errors.append(f'bytes_per_shot: expected {expected_bytes_per_shot}')
        if valid_shots and case.get("expected_output_bytes") != expected_bytes_per_shot * shots:
            errors.append(
                f'expected_output_bytes: expected {expected_bytes_per_shot * shots}'
            )

    canonical_path = case.get("canonical_input_path")
    if isinstance(canonical_path, str):
        input_path = _path_from_repo(repo_root, canonical_path)
        if not input_path.is_file():
            errors.append(f'canonical_input_path: file does not exist: {canonical_path}')
        else:
            digest = hashlib.sha256(input_path.read_bytes()).hexdigest()
            if digest != case.get("canonical_input_sha256"):
                errors.append(
                    "canonical_input_sha256: expected "
                    f'{case.get("canonical_input_sha256")}, got {digest}'
                )

    source_manifest_path = case.get("source_manifest_path")
    source_case_id = case.get("source_manifest_case_id")
    if isinstance(source_manifest_path, str) and isinstance(source_case_id, str):
        source_manifest = load_manifest(_path_from_repo(repo_root, source_manifest_path))
        source_case = find_case(source_manifest, source_case_id)
        for fair_field, source_field in (
            ("shots", "shots"),
            ("measurement_count", "expected_measurements"),
            ("stim_version", "stim_version"),
        ):
            if case.get(fair_field) != source_case.get(source_field):
                errors.append(f'{fair_field}: does not match source manifest {source_field}')
        source_input = source_case.get("canonical_input_path")
        if not isinstance(source_input, str) or not isinstance(canonical_path, str):
            errors.append(
                "canonical_input_path: source manifest must provide a string path "
                "that resolves to the fair path"
            )
        else:
            source_path = (_path_from_repo(repo_root, source_manifest_path).parent / source_input).resolve()
            if source_path != _path_from_repo(repo_root, canonical_path):
                errors.append("canonical_input_path: source manifest resolves to a different file")

    argv_table = case.get("argv")
    if not isinstance(argv_table, dict):
        errors.append('argv: expected a table')
        return errors
    expanded: dict[str, list[str]] = {}
    for name, expected_template in EXPECTED_ARGV.items():
        template = argv_table.get(name)
        if template != expected_template:
            errors.append(f'argv.{name}: expected canonical template')
        if isinstance(template, list) and all(isinstance(value, str) for value in template):
            try:
                expanded[name] = expand_argv(template, case, seed=0)
            except (AttributeError, IndexError, KeyError, ValueError) as error:
                errors.append(f'argv.{name}: could not expand template: {error}')

    if len(expanded) == 2:
        formats = [_argv_option(argv, "--out_format", errors) for argv in expanded.values()]
        if any(output_format != "b8" for output_format in formats):
            errors.append("asymmetric output_format: expected b8")
        input_paths = [_argv_option(argv, "--in", errors) for argv in expanded.values()]
        if None not in input_paths:
            resolved = [Path(path).resolve() for path in input_paths if path is not None]
            if len(set(resolved)) != 1:
                errors.append("canonical_input_path: expanded argv paths differ")
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate the fair CLI benchmark contract.")
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--case", required=True)
    args = parser.parse_args(argv)
    try:
        manifest = load_manifest(args.manifest)
        case = find_case(manifest, args.case)
        errors = validate_case(
            case,
            manifest_path=args.manifest,
            repo_root=Path(__file__).resolve().parents[2],
        )
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1
    if errors:
        for error in errors:
            print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1
    print(
        "PASS fair CLI contract "
        f"case={case['case_id']} shots={case['shots']} measurements={case['measurement_count']} "
        f"format={case['output_format']} bytes_per_shot={case['bytes_per_shot']} "
        f"bytes={case['expected_output_bytes']} timer={case['timer_scope']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
