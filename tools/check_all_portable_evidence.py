#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
import tomllib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator.portable_provenance import (  # noqa: E402
    SCHEMA_VERSION,
    load_catalog,
    validate_catalog,
)
from tools import check_rstim_vs_stim_compiled_steady_evidence as compiled_steady  # noqa: E402
from tools import check_rstim_vs_stim_fair_cli_evidence as fair_cli  # noqa: E402
from tools import check_rstim_vs_stim_instruction_wide_noise_evidence as instruction_wide  # noqa: E402
from tools import check_rstim_vs_stim_reference_build_evidence as reference_build  # noqa: E402


@dataclass(frozen=True)
class BundleChecker:
    validate: Callable[[Path], Any]
    pass_line: Callable[[Any], str]


def _fair_cli_pass_line(result: Any) -> str:
    variants, measured = result
    return f"PASS fair CLI sampling evidence variants={variants} measured={measured}"


def _compiled_steady_pass_line(result: Any) -> str:
    variants, measured, lifecycle = result
    return f"PASS compiled steady-state sampling evidence variants={variants} measured={measured} lifecycle={lifecycle}"


def _reference_build_pass_line(result: Any) -> str:
    return (
        "PASS packed reference-build evidence "
        f"variants=3 direct_speedup={result['direct_speedup']:.6f}"
    )


def _instruction_wide_pass_line(result: Any) -> str:
    builds, attempts, legacy_setups = result
    return (
        "PASS instruction-wide frame-noise evidence "
        f"builds={builds} attempts={attempts} legacy_setups={legacy_setups}"
    )


CHECKERS: dict[str, BundleChecker] = {
    "fair-cli-release": BundleChecker(fair_cli.validate_bundle, _fair_cli_pass_line),
    "compiled-steady-release": BundleChecker(compiled_steady.validate_bundle, _compiled_steady_pass_line),
    "reference-build-release": BundleChecker(reference_build.validate_bundle, _reference_build_pass_line),
    "frame-instruction-wide-release": BundleChecker(instruction_wide.validate_bundle, _instruction_wide_pass_line),
}


def _repo_root_from_catalog(catalog_path: Path) -> Path:
    return catalog_path.resolve().parents[2]


def _bundle_path(repo_root: Path, bundle: dict[str, Any]) -> Path:
    raw_path = bundle["bundle_path"]
    return repo_root / PurePosixPath(raw_path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate all portable checked evidence bundles.")
    parser.add_argument("--catalog", type=Path, required=True)
    args = parser.parse_args(argv)

    try:
        catalog = load_catalog(args.catalog)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.catalog}: {error}", file=sys.stderr)
        return 1

    errors = validate_catalog(catalog, args.catalog)
    if errors:
        for error in errors:
            print(f"{args.catalog}: {error}", file=sys.stderr)
        return 1

    bundles = catalog["bundles"]
    print(f"PASS portable evidence catalog bundles={len(bundles)} schema={SCHEMA_VERSION}")
    repo_root = _repo_root_from_catalog(args.catalog)
    for bundle in bundles:
        bundle_id = bundle["id"]
        checker = CHECKERS.get(bundle_id)
        if checker is None:
            print(
                f"FAIL portable checked evidence bundle={bundle_id}: no registered checker",
                file=sys.stderr,
            )
            return 1
        try:
            result = checker.validate(_bundle_path(repo_root, bundle))
        except Exception as error:
            print(f"FAIL portable checked evidence bundle={bundle_id}: {error}", file=sys.stderr)
            return 1
        print(checker.pass_line(result))

    print(f"PASS portable checked evidence bundles={len(bundles)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
