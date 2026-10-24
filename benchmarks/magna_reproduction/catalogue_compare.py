#!/usr/bin/env python3
"""Certify the complete pinned external catalogue with the Rust validator."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--rust-example", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    from witness_environment import native_reference
    from reference import digest
    binary = args.rust_example.resolve(strict=True)
    with native_reference(args.upstream) as captured:
        source = args.upstream.resolve() / "core/magna/data/phase_decompositions.json"
        native = json.loads(source.read_text(encoding="utf-8"))
        result = subprocess.run([str(binary), str(source), "--check-canonical-cores"], capture_output=True,
                                text=True, timeout=60, check=True)
        actual = json.loads(result.stdout)
        expected = sorted([dict(dimension=e["dimension"], phase_table=e["core_phase_mod8"],
                                terms=len(e["terms"]), canonical_core=True) for e in native["entries"]],
                          key=lambda e: (e["dimension"], e["phase_table"]))
        if actual.get("status") != "certified" or actual.get("entries") != expected:
            raise RuntimeError("Rust did not certify the complete native catalogue")
        if actual.get("canonical_cores_checked") is not True:
            raise RuntimeError("Rust did not normalize every certified catalogue core")
        if actual.get("source_sha256") != digest(source):
            raise RuntimeError("Rust certified a different catalogue source")
        if actual["usage"]["entries"] != len(expected):
            raise RuntimeError("certified entry count differs from the complete source")
        receipt = dict(schema="rmagna.phase-catalogue-reference.v1",
                       upstream_commit=captured.manifest["upstream"]["commit"],
                       native_sha256=captured.native_sha256,
                       reference_manifest_sha256=captured.manifest_sha256,
                       rust_validator_sha256=digest(binary), certificate=actual,
                       scope="external catalogue reconstruction; no execution integration claim")
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(f"Certified all {len(expected)} pinned phase-catalogue entries")


if __name__ == "__main__":
    main()
