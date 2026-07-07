from __future__ import annotations

import argparse
import math
import sys
import tomllib
from pathlib import Path
from typing import Any

import stim


SUITE = "rstim_vs_stim_simulator"
MANIFEST_VERSION = 1
GENERATED_OUTPUTS_ROOT = "benchmarks/out/rstim_vs_stim_simulator"
TIERS = {"smoke", "full", "documentation-only"}
SOURCES = {"stim"}
FULL_CASE_ID = "stim_surface_d11_r100"
FULL_CASE_NOISE = {
    "after_clifford_depolarization": 0.001,
    "after_reset_flip_probability": 0.001,
    "before_measure_flip_probability": 0.001,
    "before_round_data_depolarization": 0.0,
}
REQUIRED_CASE_FIELDS = {
    "case_id",
    "tier",
    "source",
    "workload",
    "code",
    "task",
    "distance",
    "rounds",
    "canonical_input_path",
    "generation_command",
    "after_clifford_depolarization",
    "after_reset_flip_probability",
    "before_measure_flip_probability",
    "before_round_data_depolarization",
    "shots",
    "expected_qubits",
    "expected_measurements",
    "expected_detectors",
    "expected_observables",
    "stim_version",
    "provenance",
}


def _is_int(value: object) -> bool:
    return type(value) is int


def _require_str(case: dict[str, Any], field: str, case_label: str, errors: list[str]) -> str | None:
    value = case.get(field)
    if not isinstance(value, str) or not value.strip():
        errors.append(f'{case_label} field "{field}" must be a non-empty string')
        return None
    return value


def _require_positive_int(
    case: dict[str, Any],
    field: str,
    case_label: str,
    errors: list[str],
) -> int | None:
    value = case.get(field)
    if not _is_int(value) or value <= 0:
        errors.append(f'{case_label} field "{field}" must be a positive integer')
        return None
    return value


def _require_probability(
    case: dict[str, Any],
    field: str,
    case_label: str,
    errors: list[str],
) -> float | None:
    value = case.get(field)
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        errors.append(f'{case_label} field "{field}" must be a probability')
        return None
    probability = float(value)
    if not 0 <= probability <= 1:
        errors.append(f'{case_label} field "{field}" must be between 0 and 1')
        return None
    return probability


def _case_input_path(raw_path: str, base_dir: Path, benchmark_dir: Path) -> Path:
    candidate = (base_dir / raw_path).resolve()
    if candidate.is_relative_to(benchmark_dir.resolve()):
        return candidate
    return (benchmark_dir / raw_path).resolve()


def _validate_full_case_noise(case: dict[str, Any], case_label: str, errors: list[str]) -> None:
    if case.get("case_id") != FULL_CASE_ID:
        return

    for field, expected in FULL_CASE_NOISE.items():
        value = case.get(field)
        if not isinstance(value, (int, float)) or isinstance(value, bool):
            continue
        actual = float(value)
        if not math.isclose(actual, expected, rel_tol=0.0, abs_tol=1e-12):
            if field == "before_round_data_depolarization":
                errors.append(f"before_round_data_depolarization must be 0 for {FULL_CASE_ID}")
            else:
                errors.append(f'{case_label} field "{field}" must be {expected:g}')


def _validate_circuit_counts(case: dict[str, Any], case_label: str, input_path: Path, errors: list[str]) -> None:
    if not input_path.is_file():
        errors.append(f'{case_label} canonical input "{input_path}" does not exist')
        return

    try:
        circuit = stim.Circuit(input_path.read_text())
    except Exception as error:  # Stim raises ValueError for parse failures.
        errors.append(f'{case_label} canonical input failed to parse as Stim: {error}')
        return

    expected_counts = {
        "expected_qubits": circuit.num_qubits,
        "expected_measurements": circuit.num_measurements,
        "expected_detectors": circuit.num_detectors,
        "expected_observables": circuit.num_observables,
    }
    for field, actual in expected_counts.items():
        expected = case.get(field)
        if expected != actual:
            errors.append(f'{case_label} field "{field}" must be {actual}')


def validate_manifest(manifest: dict[str, Any], base_dir: Path) -> list[str]:
    errors: list[str] = []
    benchmark_dir = Path(__file__).resolve().parent
    base_dir = base_dir.resolve()

    if manifest.get("manifest_version") != MANIFEST_VERSION:
        errors.append("manifest_version must be 1")
    if manifest.get("suite") != SUITE:
        errors.append(f'suite must be "{SUITE}"')
    if manifest.get("generated_outputs_root") != GENERATED_OUTPUTS_ROOT:
        errors.append(f'generated_outputs_root must be "{GENERATED_OUTPUTS_ROOT}"')

    cases = manifest.get("cases")
    if not isinstance(cases, list) or not cases:
        errors.append('manifest field "cases" must be a non-empty array')
        return errors

    seen: set[str] = set()
    for index, raw_case in enumerate(cases):
        case_label = f"case[{index}]"
        if not isinstance(raw_case, dict):
            errors.append(f"{case_label} must be a TOML table")
            continue

        missing = sorted(REQUIRED_CASE_FIELDS - set(raw_case))
        if missing:
            errors.append(f"{case_label} missing required field(s): {', '.join(missing)}")

        case_id = _require_str(raw_case, "case_id", case_label, errors)
        if case_id is not None:
            case_label = f'case "{case_id}"'
            if case_id in seen:
                errors.append(f'duplicate case_id "{case_id}"')
            seen.add(case_id)

        tier = _require_str(raw_case, "tier", case_label, errors)
        if tier is not None and tier not in TIERS:
            errors.append(
                f'{case_label} field "tier" must be one of: documentation-only, full, smoke'
            )
        source = _require_str(raw_case, "source", case_label, errors)
        if source is not None and source not in SOURCES:
            errors.append(f'{case_label} field "source" must be "stim"')

        code = _require_str(raw_case, "code", case_label, errors)
        task = _require_str(raw_case, "task", case_label, errors)
        workload = _require_str(raw_case, "workload", case_label, errors)
        if code is not None and task is not None and workload is not None:
            expected_workload = f"{code}:{task}"
            if workload != expected_workload:
                errors.append(f'{case_label} field "workload" must be "{expected_workload}"')

        _require_positive_int(raw_case, "distance", case_label, errors)
        _require_positive_int(raw_case, "rounds", case_label, errors)
        _require_positive_int(raw_case, "shots", case_label, errors)
        _require_positive_int(raw_case, "expected_qubits", case_label, errors)
        _require_positive_int(raw_case, "expected_measurements", case_label, errors)
        _require_positive_int(raw_case, "expected_detectors", case_label, errors)
        _require_positive_int(raw_case, "expected_observables", case_label, errors)

        _require_probability(raw_case, "after_clifford_depolarization", case_label, errors)
        _require_probability(raw_case, "after_reset_flip_probability", case_label, errors)
        _require_probability(raw_case, "before_measure_flip_probability", case_label, errors)
        _require_probability(raw_case, "before_round_data_depolarization", case_label, errors)
        _validate_full_case_noise(raw_case, case_label, errors)

        command = _require_str(raw_case, "generation_command", case_label, errors)
        if command is not None and "stim gen" not in command:
            errors.append(f'{case_label} field "generation_command" must include "stim gen"')
        _require_str(raw_case, "stim_version", case_label, errors)
        _require_str(raw_case, "provenance", case_label, errors)

        input_value = _require_str(raw_case, "canonical_input_path", case_label, errors)
        if input_value is not None:
            input_path = _case_input_path(input_value, base_dir, benchmark_dir)
            try:
                input_path.relative_to(benchmark_dir)
            except ValueError:
                errors.append(f'{case_label} canonical_input_path must stay under {benchmark_dir}')
            else:
                _validate_circuit_counts(raw_case, case_label, input_path, errors)

    return errors


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be a TOML table")
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate rstim-vs-Stim simulator fixture manifests.")
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    errors = validate_manifest(manifest, args.manifest.parent)
    if errors:
        for error in errors:
            print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    cases = manifest["cases"]
    print(f"PASS {len(cases)} fixture cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
