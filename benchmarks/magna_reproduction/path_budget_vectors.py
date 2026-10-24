#!/usr/bin/env python3
"""Capture minimum native raw-path caps for matched Clifford lowerings."""
import argparse
import json
from pathlib import Path
import random


def generate(captured):
    from magna import ResourceLimit, compile_circuit, parse_circuit
    cases = [
        ("zero_paths", "I 2\nX 1\nS 2"),
        ("cancelled_pair", "H 0\nH 0\nI 2"),
        ("eight_cancelled", "H 0\n" * 8 + "I 2"),
        ("entangled", "H 0\nCX 0 1\nH 1\nS 2\nH 0\nH 2"),
        ("measurement_reset", "H 0\nH 0\nM 0\nR 0\nH 0\nH 0\nM 0\nI 2"),
        ("wide_cancelled", "I 128\nH 128\nH 128\nH 64\nH 64"),
    ]
    vectors = []
    for name, source in cases:
        failures = []
        for cap in range(33):
            try:
                compiled = compile_circuit(parse_circuit(source), options={"max_path_bits": cap})
                compiled.sample(random.Random(731))
            except ResourceLimit as error:
                if "max_path_bits" not in str(error):
                    raise RuntimeError(f"unrelated resource failure in {name}: {error}") from error
                failures.append(cap)
                continue
            vectors.append({"case": name, "source": source, "minimum_successful_cap": cap,
                            "rejected_caps": failures})
            break
        else:
            raise RuntimeError(f"raw-path threshold not found for {name}")
    return {"schema": "rmagna.raw-path-budget-reference.v1",
            "upstream_commit": captured.manifest["upstream"]["commit"],
            "native_sha256": captured.native_sha256,
            "reference_manifest_sha256": captured.manifest_sha256,
            "scope": "matched Clifford lowerings only; phase observation lowerings can differ",
            "vectors": vectors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    from witness_environment import native_reference
    with native_reference(args.upstream) as captured:
        data = generate(captured)
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
