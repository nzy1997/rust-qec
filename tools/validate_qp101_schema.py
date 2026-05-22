#!/usr/bin/env python3
"""Validate QP101 JSON documents against the repository schema."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

import jsonschema


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate one or more QP101 JSON documents against a JSON Schema."
    )
    parser.add_argument("schema", type=Path, help="Path to qp101.schema.json")
    parser.add_argument(
        "documents",
        type=Path,
        nargs="+",
        help="QP101 JSON document paths to validate",
    )
    return parser.parse_args()


def load_json(path: Path) -> object:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def main() -> int:
    args = parse_args()
    schema = load_json(args.schema)
    validator_cls = jsonschema.validators.validator_for(schema)
    validator_cls.check_schema(schema)
    validator = validator_cls(schema)

    failed = False
    for document in args.documents:
        data = load_json(document)
        errors = sorted(validator.iter_errors(data), key=lambda err: list(err.path))
        if not errors:
            print(f"ok: {document}")
            continue

        failed = True
        print(f"error: {document}", file=sys.stderr)
        for error in errors:
            location = "/".join(str(part) for part in error.absolute_path) or "<root>"
            print(f"  {location}: {error.message}", file=sys.stderr)

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
