from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator.portable_provenance import (
    SCHEMA_VERSION,
    load_catalog,
    validate_catalog,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate portable rstim-vs-Stim evidence bundles.")
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
    print(f"PASS portable evidence catalog bundles={len(catalog['bundles'])} schema={SCHEMA_VERSION}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
