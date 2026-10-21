#!/usr/bin/env python3
"""Capture exact target/recovery/moment task numerators from the pinned public API."""
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
    from magna import compile_circuit, parse_circuit
    from magna.file_api import NoiseDraw
    cases = [("mixed", "I 0", {}, dict(source="H 0\nCX 0 1", outputs=[1], postselection={}), None)]
    for physical in ("I 0", "X 0", "H 0"):
        cases.append(("projected-" + physical[0], physical, {}, dict(source="H 1\nT 1\nH 1\nCX 1 0", outputs=[0], postselection={1: 0}), None))
    cases.extend([
        ("recovery-local", "H 2\nT 2\nZ 2\nH 0\nM 0", dict(outputs=[2]), dict(source="H 0\nT 0", outputs=None, postselection={}), "M 1\nCX rec[-1] 0\nZ 0"),
        ("moments", "H 2", dict(outputs=[2], observables=[[1, 0], [1, 1], [0, 1]]), dict(source="I 0", outputs=None, postselection={}), "S 0"),
        ("logical", "X_ERROR(1/5) 0\nM(1/7) 0\nOBSERVABLE_INCLUDE(0) rec[-1]", dict(error_observables=[0]), None, None),
    ])
    vectors = []
    for name, source, task, ideal, recovery in cases:
        physical = parse_circuit(source)
        target = None if ideal is None else parse_circuit(ideal["source"])
        if ideal is not None and ideal["outputs"] is not None:
            target = dict(circuit=target, outputs=ideal["outputs"], postselection=ideal["postselection"])
        compiled = compile_circuit(physical, task=task, ideal_state=target,
                                   recovery=None if recovery is None else parse_circuit(recovery))
        for mask in range(4 if name == "logical" else 1):
            gates, faults, flips = {}, [], []
            if mask & 1:
                inst = next(i for i in physical.instructions if i.name == "X_ERROR")
                gates[inst.op_id] = (dict(kind="X", qubits=[0]),)
                faults = [dict(line=inst.source.line, alternative=0)]
            if mask & 2:
                flips = [0]
            exact = compiled.sample(random.Random(20261026), noise=NoiseDraw(gates, tuple(f"m{i}" for i in flips)))["exact"]
            def scalar(value):
                return None if value is None else dict(coefficients=list(map(str, value["coefficients"])), power=value["power"])
            vectors.append(dict(case=name, source=source, task=task, ideal=ideal, recovery=recovery,
                                faults=faults, report_flips=flips,
                                history_mass=scalar(exact["history_mass"]), acceptance=scalar(exact["acceptance"]), error=scalar(exact["error"]),
                                moments=list(map(scalar, exact["moments"])), target_success=scalar(exact.get("target_success")),
                                target_normalization=scalar(exact.get("target_normalization", dict(coefficients=[1, 0, 0, 0], power=0)))))
    return dict(schema="rmagna.task-reference.v1", upstream_commit=captured.manifest["upstream"]["commit"],
                native_sha256=captured.native_sha256, reference_manifest_sha256=captured.manifest_sha256, vectors=vectors)


if __name__ == "__main__":
    main()
