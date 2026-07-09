from __future__ import annotations

import argparse
import math
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


SUITE = "rstim_vs_stim_simulator"
MANIFEST_VERSION = 1
PINNED_SOURCE_COMMIT = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
SOURCE_URL = (
    "https://github.com/quantumlib/Stim/blob/"
    f"{PINNED_SOURCE_COMMIT}/src/stim/cmd/command_sample.test.cc"
)
DEFAULT_DISTRIBUTION_TOLERANCE = 1e-9
REQUIRED_CASE_FIELDS = {
    "case_id",
    "source_url",
    "source_commit",
    "source_line_start",
    "source_line_end",
    "circuit",
    "shots",
    "expected_distribution",
}
BITSTRING_RE = re.compile(r"^[01]+$")


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
    return int(value)


def _require_tolerance(value: object, label: str, errors: list[str]) -> float | None:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        errors.append(f"{label} must be a positive numeric tolerance")
        return None
    tolerance = float(value)
    if not math.isfinite(tolerance) or tolerance <= 0:
        errors.append(f"{label} must be a positive numeric tolerance")
        return None
    return tolerance


def _validate_expected_distribution(
    case: dict[str, Any],
    case_label: str,
    tolerance: float,
    errors: list[str],
) -> None:
    distribution = case.get("expected_distribution")
    if not isinstance(distribution, dict) or not distribution:
        errors.append(f'{case_label} field "expected_distribution" must be a non-empty table')
        return

    total = 0.0
    bit_width: int | None = None
    for outcome, raw_probability in distribution.items():
        if not isinstance(outcome, str) or BITSTRING_RE.fullmatch(outcome) is None:
            errors.append(
                f'{case_label} expected_distribution key "{outcome}" must be a non-empty 01 bitstring'
            )
            continue
        if bit_width is None:
            bit_width = len(outcome)
        elif len(outcome) != bit_width:
            errors.append(
                f"{case_label} expected_distribution outcomes must all have the same bit width"
            )

        if not isinstance(raw_probability, (int, float)) or isinstance(raw_probability, bool):
            errors.append(f'{case_label} expected_distribution["{outcome}"] must be a probability')
            continue
        probability = float(raw_probability)
        if not math.isfinite(probability) or not 0 <= probability <= 1:
            errors.append(
                f'{case_label} expected_distribution["{outcome}"] must be between 0 and 1'
            )
            continue
        total += probability

    if not math.isclose(total, 1.0, rel_tol=0.0, abs_tol=tolerance):
        errors.append(
            f"{case_label} expected distribution probabilities must sum to 1 within {tolerance:g}; "
            f"got {total:.17g}"
        )


def validate_manifest(manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if manifest.get("manifest_version") != MANIFEST_VERSION:
        errors.append("manifest_version must be 1")
    if manifest.get("suite") != SUITE:
        errors.append(f'suite must be "{SUITE}"')

    default_tolerance = DEFAULT_DISTRIBUTION_TOLERANCE
    if "distribution_tolerance" in manifest:
        parsed = _require_tolerance(
            manifest["distribution_tolerance"],
            "distribution_tolerance",
            errors,
        )
        if parsed is not None:
            default_tolerance = parsed

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

        source_url = _require_str(raw_case, "source_url", case_label, errors)
        if source_url is not None and source_url != SOURCE_URL:
            errors.append(
                f'{case_label} field "source_url" must be the pinned Stim command_sample.test.cc URL'
            )

        source_commit = _require_str(raw_case, "source_commit", case_label, errors)
        if source_commit is not None and source_commit != PINNED_SOURCE_COMMIT:
            errors.append(f'{case_label} field "source_commit" must be "{PINNED_SOURCE_COMMIT}"')

        line_start = _require_positive_int(raw_case, "source_line_start", case_label, errors)
        line_end = _require_positive_int(raw_case, "source_line_end", case_label, errors)
        if line_start is not None and line_end is not None and line_start > line_end:
            errors.append(f"{case_label} source_line_start must be <= source_line_end")

        _require_str(raw_case, "circuit", case_label, errors)
        _require_positive_int(raw_case, "shots", case_label, errors)

        case_tolerance = default_tolerance
        if "tolerance" in raw_case:
            parsed = _require_tolerance(raw_case["tolerance"], f'{case_label} field "tolerance"', errors)
            if parsed is not None:
                case_tolerance = parsed

        _validate_expected_distribution(raw_case, case_label, case_tolerance, errors)

    return errors


def load_manifest(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        manifest = tomllib.load(handle)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be a TOML table")
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate rstim-vs-Stim distribution case manifests.")
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    errors = validate_manifest(manifest)
    if errors:
        for error in errors:
            print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    cases = manifest["cases"]
    print(f"PASS {len(cases)} distribution cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
