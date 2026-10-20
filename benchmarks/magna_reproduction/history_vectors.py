#!/usr/bin/env python3
"""Capture causal controller histories using the pinned public native file API."""
import argparse
import json
from pathlib import Path
import random
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from witness_environment import native_reference
    with native_reference(args.upstream) as captured:
        data = generate(captured)
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


def generate(captured):
    from magna import ControllerRegistry, compile_circuit, parse_circuit
    from magna.file_api import NoiseDraw
    cases = [
        ("majority", "H 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[decision] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nPOSTSELECT det[0]\n", [3], 8),
        ("sequential", "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[decision] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[decision] det[1] ctrl[0] -> ctrl[0]\nCX ctrl[0] 2\n", [2], 2),
        ("guarded", "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[decision] det[0] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 1\n", [1], 2),
        ("irrational", "H 0\nT 0\nH 0\nM(1/5) 0\nDETECTOR rec[-1]\nCONTROL[decision] det[0] -> ctrl[0]\nCX ctrl[0] 1\nPOSTSELECT det[0]\n", [1], 2),
    ]
    vectors = []
    for name, source, outputs, coverage in cases:
        calls = []
        def decision(bits, config):
            result = int(sum(bits) >= 2) if name == "majority" else sum(bits) % 2
            calls.append(dict(inputs=list(map(int, bits)), outputs=[result]))
            return (result,)
        registry = ControllerRegistry()
        registry.register("decision", decision, version="1")
        parsed = parse_circuit(source, controllers=registry)
        compiled = compile_circuit(parsed, task=dict(outputs=outputs), ideal_state=parse_circuit("I 0\n"),
                                   controllers=registry, options=dict(history_sampler="exact_marginal"))
        variants = [False, True] if name in {"guarded", "irrational"} else [False]
        for faulted in variants:
            gates, faults, flips = {}, [], []
            if faulted and name == "guarded":
                inst = next(i for i in parsed.instructions if i.name == "X_ERROR")
                gates[inst.op_id] = (dict(kind="X", qubits=[1]),)
                faults = [dict(line=inst.source.line, alternative=0)]
            if faulted and name == "irrational":
                flips = [0]
            seen = set()
            for seed in range(1024):
                calls.clear()
                exact = compiled.sample(random.Random(seed), noise=NoiseDraw(gates, tuple(f"m{i}" for i in flips)))["exact"]
                signature = tuple(tuple(call["inputs"]) for call in calls)
                if signature in seen:
                    continue
                seen.add(signature)
                scalar = lambda value: dict(coefficients=list(map(str, value["coefficients"])), power=value["power"])
                vectors.append(dict(case=name, source=source, ideal="I 0\n", outputs=outputs,
                                    calls=[dict(inputs=c["inputs"].copy(), outputs=c["outputs"].copy()) for c in calls],
                                    faults=faults, report_flips=flips, seed=seed,
                                    **{key: scalar(exact[key]) for key in ("history_mass", "acceptance", "error", "target_success")}))
                if len(seen) == coverage:
                    break
            if len(seen) != coverage:
                raise RuntimeError(f"incomplete prescribed history coverage for {name}: {len(seen)}")
    return dict(schema="rmagna.history-reference.v1", upstream_commit=captured.manifest["upstream"]["commit"],
                native_sha256=captured.native_sha256, reference_manifest_sha256=captured.manifest_sha256, vectors=vectors)


if __name__ == "__main__":
    main()
