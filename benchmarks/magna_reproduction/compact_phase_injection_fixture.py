#!/usr/bin/env python3
"""Preserve every captured JSON value while bounding tracked fixture bytes."""
import argparse
import hashlib
import json
from pathlib import Path


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error("output already exists; choose a fresh path")
    raw = args.source.read_bytes()
    value = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(value, dict) or value.get("schema") != "rstim.phase-injection-native.v1":
        parser.error("expected a native injection witness")
    compact = json.dumps(value, separators=(",", ":"), allow_nan=False).encode() + b"\n"
    if len(compact) > 2 * 1024 * 1024:
        parser.error("compact fixture exceeds the repository single-file limit")
    with args.out.open("xb") as stream:
        stream.write(compact)
    print(json.dumps(dict(preserved_all_values=True, source_bytes=len(raw),
                          fixture_bytes=len(compact),
                          sha256=hashlib.sha256(compact).hexdigest())))


if __name__ == "__main__":
    main()
