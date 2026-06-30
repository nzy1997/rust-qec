from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path
from typing import Any


SUITE = "qec_code_random_window"
MANIFEST_VERSION = 1
DISTANCE_SIDES = {"any", "x", "z"}
REQUIRED_CASE_FIELDS = {
    "case_id",
    "code_id",
    "distance_side",
    "iterations",
    "restarts",
    "seed",
    "baseline_key",
    "baseline_required",
}
NO_TARGET_LADDER_REQUIRED_CASE_IDS = {
    "surface_rotated_d5",
    "toric_d5",
    "bb72",
    "bb144",
}


def _is_int(value: object) -> bool:
    return type(value) is int


def _usable_baseline_key(value: object) -> bool:
    if not isinstance(value, str):
        return False
    normalized = value.strip().lower()
    return normalized not in {"", "none", "null", "n/a"} and not normalized.startswith(
        "unmapped:"
    )


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


def _require_nonnegative_int(
    case: dict[str, Any],
    field: str,
    case_label: str,
    errors: list[str],
) -> int | None:
    value = case.get(field)
    if not _is_int(value) or value < 0:
        errors.append(f'{case_label} field "{field}" must be a non-negative integer')
        return None
    return value


def validate_manifest(manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []

    if manifest.get("manifest_version") != MANIFEST_VERSION:
        errors.append('manifest_version must be 1')
    if manifest.get("suite") != SUITE:
        errors.append(f'suite must be "{SUITE}"')

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

        _require_str(raw_case, "code_id", case_label, errors)
        distance_side = _require_str(raw_case, "distance_side", case_label, errors)
        if distance_side is not None and distance_side not in DISTANCE_SIDES:
            errors.append(f'{case_label} field "distance_side" must be one of: any, x, z')

        _require_positive_int(raw_case, "iterations", case_label, errors)
        _require_positive_int(raw_case, "restarts", case_label, errors)
        _require_nonnegative_int(raw_case, "seed", case_label, errors)
        target_weight = None
        if "target_weight" in raw_case:
            target_weight = _require_positive_int(raw_case, "target_weight", case_label, errors)

        target_upper_bound = raw_case.get("target_upper_bound")
        if target_upper_bound is not None:
            if not _is_int(target_upper_bound) or target_upper_bound <= 0:
                errors.append(f'{case_label} field "target_upper_bound" must be a positive integer')
            elif target_weight is not None and target_weight > target_upper_bound:
                errors.append(f'{case_label} target_weight must be <= target_upper_bound')

        baseline_key = raw_case.get("baseline_key")
        if not isinstance(baseline_key, str):
            errors.append(f'{case_label} field "baseline_key" must be a string')

        baseline_required = raw_case.get("baseline_required")
        if type(baseline_required) is not bool:
            errors.append(f'{case_label} field "baseline_required" must be a boolean')
        elif baseline_required and not _usable_baseline_key(baseline_key):
            errors.append(
                f'{case_label} has baseline_required = true but no usable baseline_key'
            )

    return errors


def validate_no_target_ladder_manifest(
    manifest: dict[str, Any],
    required_case_ids: set[str] | None = None,
) -> list[str]:
    errors = validate_manifest(manifest)
    cases = manifest.get("cases")
    if not isinstance(cases, list):
        return errors

    required = required_case_ids or NO_TARGET_LADDER_REQUIRED_CASE_IDS
    present = {
        raw_case.get("case_id")
        for raw_case in cases
        if isinstance(raw_case, dict) and isinstance(raw_case.get("case_id"), str)
    }
    for missing_case_id in sorted(required - present):
        errors.append(f'no-target ladder manifest missing required case "{missing_case_id}"')

    for index, raw_case in enumerate(cases):
        if not isinstance(raw_case, dict):
            continue
        case_id = raw_case.get("case_id")
        case_label = f'case "{case_id}"' if isinstance(case_id, str) else f"case[{index}]"
        if "target_weight" in raw_case:
            errors.append(
                f'{case_label} must omit field "target_weight" for no-target ladder runs'
            )
    return errors


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be a TOML table")
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate qec-code random-window case manifests.")
    parser.add_argument("manifest", type=Path)
    parser.add_argument(
        "--no-target-ladder-smoke",
        action="store_true",
        help="Require no-target issue-225 ladder smoke semantics.",
    )
    args = parser.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    if args.no_target_ladder_smoke:
        errors = validate_no_target_ladder_manifest(manifest)
    else:
        errors = validate_manifest(manifest)
    if errors:
        for error in errors:
            print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    print("PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
