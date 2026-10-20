#!/usr/bin/env python3
"""Capture prescribed affine-instrument masses from the pinned native backend."""

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
    import reference
    from witness_environment import native_reference

    with native_reference(args.upstream) as captured:
        data = generate(captured, args.upstream.resolve(), reference)
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


def generate(captured, source, reference):
    from magna import compile_circuit
    from magna.circuit_file import parse_circuit
    from magna.file_api import NoiseDraw

    def scalar(value):
        return dict(coefficients=[str(c) for c in value["coefficients"]], power=value["power"])

    examples = [
        ("interference", "H 0\nT 0\nH 0\nM(1/30) 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n", "I 0\n", [0]),
        ("mixed-reset", "H 0\nCX 0 1\nR 0\nH 0\nT 0\nY_ERROR(1/7) 1\nM(1/5) 1\nCY rec[-1] 0\n", "H 0\nT 0\n", [0]),
        ("correlated", "RY 0 1\nE(1/10) X0 Y1\nELSE_CORRELATED_ERROR(2/9) Z1\nMPP(1/11) Y0*Y1\nDETECTOR rec[-1]\nCS rec[-1] 1\nPOSTSELECT det[0]\n", "H 0 1\nS 0\nT 1\n", [0, 1]),
    ]
    bh_path = source / "examples/bh2-T/physical.magna"
    ideal_path = source / "examples/bh2-T/ideal.magna"
    fixtures = Path(__file__).resolve().parents[2] / "rmagna/tests/data/magna-bh2"
    for name, original in (("physical.magna", bh_path), ("ideal.magna", ideal_path)):
        if reference.digest(fixtures / name) != reference.digest(original):
            raise RuntimeError("committed BH2 circuit input differs from the pinned reference")
    examples.append(("bh2-T", bh_path.read_text(), ideal_path.read_text(), [14, 15]))
    result = []
    for name, physical, ideal, outputs in examples:
        parsed = parse_circuit(physical)
        compiled = compile_circuit(parsed, task=dict(outputs=outputs), ideal_state=parse_circuit(ideal),
                                   options=dict(max_quotient_rank=8, max_terms=256))
        noise = [i for i in parsed.instructions if i.name in {"X_ERROR", "Y_ERROR", "Z_ERROR", "CORRELATED_ERROR", "ELSE_CORRELATED_ERROR"}]
        assignments = [((), ())]
        if name == "bh2-T":
            for indices in ((0,), (1,), (13,), (0, 1), (0, 2), (0, 13), (4, 5), (0, 1, 2), (0, 5, 12), (0, 1, 8), (0, 1, 11), (0, 2, 9)):
                assignments.append((indices, ()))
            assignments.extend([((), (0,)), ((), (27,)), ((0,), (0, 27))])
        else:
            assignments.extend([((j,), ()) for j in range(len(noise))])
            assignments.extend([((), (0,)), (tuple(range(min(1, len(noise)))), (0,))])
        for selected, flips in assignments:
            gates, faults = {}, []
            for index in selected:
                inst = noise[index]
                if inst.name.endswith("_ERROR") and inst.name not in {"CORRELATED_ERROR", "ELSE_CORRELATED_ERROR"}:
                    factors = [(t.index, inst.name[0]) for t in inst.targets]
                else:
                    factors = [(t.index, t.pauli) for t in inst.targets]
                gates[inst.op_id] = tuple(dict(kind=basis, qubits=[qubit]) for qubit, basis in factors)
                faults.append(dict(line=inst.source.line, paulis=[dict(qubit=q, basis=b) for q, b in factors]))
            row = compiled.sample(random.Random(20261023), noise=NoiseDraw(gates, tuple(f"m{i}" for i in flips)))
            exact = row["exact"]
            result.append(dict(case=name, source=None if name == "bh2-T" else physical,
                               ideal=None if name == "bh2-T" else ideal, fixture="magna-bh2" if name == "bh2-T" else None,
                               outputs=outputs, faults=faults, report_flips=list(flips),
                               accepted=scalar(exact["acceptance"]), target_success=scalar(exact["target_success"]),
                               error=scalar(exact["error"]), history_mass=scalar(exact["history_mass"])))
    return dict(schema="rmagna.instrument-reference.v1", seed=20261023,
                upstream_commit=captured.manifest["upstream"]["commit"],
                reference_manifest_sha256=captured.manifest_sha256, native_sha256=captured.native_sha256,
                vectors=result)


if __name__ == "__main__":
    main()
