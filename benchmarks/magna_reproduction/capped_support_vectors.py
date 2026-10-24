#!/usr/bin/env python3
"""Capture complete small capped-support proposal witnesses from the pinned API."""
import argparse
import json
from pathlib import Path
import random
import sys

# Keep helper discovery explicit for isolated -I reference subprocesses.
sys.path.insert(0, str(Path(__file__).resolve().parent))


def generate(captured):
    from magna import ControllerRegistry, compile_circuit, parse_circuit
    from magna.file_api import NoiseDraw
    nonaffine = "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3"
    adaptive = nonaffine + "\nDETECTOR rec[-3]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 4\nDETECTOR rec[-4]\nPOSTSELECT det[1]"
    cases = [
        ("cancelled", "H 0\nT 0\nT_DAG 0\nH 0\nM 0", [0], (), 1),
        ("biased_repeat", "H 0\nT 0\nMX 0\nMX 0", [0, 1], (), 2),
        ("nonaffine", nonaffine, list(range(4)), (), 16),
        ("adaptive_flipped", adaptive, list(range(4)), (1,), 12),
    ]
    vectors = []
    def scalar(value):
        if "numerator" in value:
            denominator = int(value["denominator"])
            if denominator <= 0 or denominator & (denominator - 1):
                raise RuntimeError("proposal denominator must be a power of two")
            return dict(coefficients=[str(value["numerator"]), "0", "0", "0"],
                        power=1-denominator.bit_length())
        return dict(coefficients=list(map(str, value["coefficients"])), power=value["power"])
    for name, source, records, flips, coverage in cases:
        registry = ControllerRegistry()
        registry.register("echo", lambda bits, config: bits, version="1")
        compiled = compile_circuit(parse_circuit(source, controllers=registry),
                                   task=dict(sample_records=records), controllers=registry,
                                   options=dict(history_sampler="affine_support"))
        seen = {}
        for seed in range(4096):
            draw = compiled.sample(random.Random(seed), noise=NoiseDraw({}, tuple(f"m{j}" for j in flips)))
            word = tuple(draw["sampled_records"])
            row = dict(records=list(word), **{key: scalar(draw["exact"][key]) for key in
                       ("history_mass", "proposal_probability", "acceptance")})
            if word in seen and seen[word] != row:
                raise RuntimeError(f"proposal depends on more than declared observations: {name}")
            seen[word] = row
            if len(seen) == coverage:
                break
        if len(seen) != coverage:
            raise RuntimeError(f"incomplete proposal coverage for {name}: {len(seen)}")
        vectors.append(dict(case=name, source=source, sample_records=records,
                            report_flips=list(flips), outcomes=[seen[word] for word in sorted(seen)]))
    return dict(schema="rmagna.capped-support-reference.v1",
                upstream_commit=captured.manifest["upstream"]["commit"],
                native_sha256=captured.native_sha256,
                reference_manifest_sha256=captured.manifest_sha256, vectors=vectors)


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
