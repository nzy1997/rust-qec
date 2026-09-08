#!/usr/bin/env python3
"""Check repository-owned rbposd parity tooling that is outside the crate API."""

from __future__ import annotations

import argparse
from pathlib import Path


REQUIRED_PATHS = (
    ".github/workflows/rbposd-parity.yml",
    "rbposd/examples/parity_driver.rs",
    "rbposd/scripts/parity_harness.py",
    "rbposd/scripts/requirements-parity.txt",
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="repository root (defaults to the parent of tools/)",
    )
    args = parser.parse_args()
    root = args.repo_root.resolve()
    missing = [relative for relative in REQUIRED_PATHS if not (root / relative).is_file()]
    if missing:
        for relative in missing:
            print(f"missing required rbposd repository tooling: {relative}")
        return 1
    print("rbposd repository tooling surfaces are present")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
