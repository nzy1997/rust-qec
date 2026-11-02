#!/usr/bin/env python3
"""Capture pinned original category/actions without quantum compilation."""
import argparse
from fractions import Fraction
import json
import os
from pathlib import Path
import sys
import tempfile

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import reference
from witness_paths import admit_destinations
from witness_environment import native_reference


def cases():
    two = ["0"] * 15
    two[5], two[10] = "1/3", "1/6"
    return [
        "M 0", "M(0) 0", "M(1/3) 0", "M 0 1", "M(0) 0 1",
        "MR 0", "MR(0) 0", "MPP X0 * Y1", "MPP(0) X0 * Y1",
        "MXX(0) 0 1", "MYY(1/7) 63 64", "MZZ(1) 128 0",
        "REPEAT 2 {\nM(0) 0\nMR 1\n}",
        "X_ERROR(0) 0\nY_ERROR(1) 0\nZ_ERROR(1/3) 128",
        "DEPOLARIZE1(1/7) 0 64", "DEPOLARIZE2(2/7) 64 0",
        "PAULI_CHANNEL_1(1/10,1/5,3/10) 2",
        "PAULI_CHANNEL_2(" + ",".join(two) + ") 1 0",
        "E(1/4) Y2 X1\nELSE_CORRELATED_ERROR(1/3) Z1 Y0\nELSE_CORRELATED_ERROR(1) X2",
        "E(0) Y2 X1\nELSE_CORRELATED_ERROR(1) Z1 Y0",
        "H 0\nM 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 1\nZ_ERROR(0) !ctrl[0] 128",
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nE(1/4) ctrl[0] Y2 X1\nELSE_CORRELATED_ERROR(1/3) ctrl[0] Z1 Y0",
        "X_ERROR(1/5) 0\nR 0\nY_ERROR(1/3) 0\nM(1/7) 0\nRY 0\nZ_ERROR(1) 0",
        "MPP(1/7) X0 * Y1 Z63 * X64\nM 128",
    ]


def capture(source):
    source = Path(source).resolve()
    rows = []
    calls = []
    rejected_inputs = []
    with native_reference(source) as captured:
        from magna import ControllerRegistry, parse_circuit
        from magna.file_api import _NoiseSchedule
        registry = ControllerRegistry()
        def echo(bits, config):
            calls.append(list(bits))
            return bits
        registry.register("echo", echo, version="original-noise-v1")
        for text in cases():
            circuit = parse_circuit(text, controllers=registry, source="original-noise.magna")
            schedule = _NoiseSchedule(circuit)
            instructions = {i.op_id: i for i in circuit.instructions}
            records = {"m" + str(r): i for i in circuit.instructions for r in i.records}
            def anchor(i):
                return dict(native_id=i.op_id, line=i.source.line, column=i.source.column, repeat=list(i.source.repeat))
            def guard(i):
                if i.guard is None:
                    return None
                expr = circuit.analysis.expression(i.guard)
                return dict(constant=expr.constant, terms=[dict(kind=t.kind, index=t.index, version=t.version) for t in sorted(expr.terms)])
            categories = []
            if len(schedule.actions) != len(schedule.probabilities):
                raise RuntimeError("native category/action shape differs")
            for actions, probabilities in zip(schedule.actions, schedule.probabilities):
                if len(actions) != len(probabilities):
                    raise RuntimeError("native channel/action shape differs")
                channels = []
                guards = []
                for (oid, value), (n, d) in zip(actions, probabilities):
                    instruction = records[value] if oid == "report" else instructions[oid]
                    action = dict(report=int(value[1:])) if oid == "report" else dict(paulis=[dict(basis=g["kind"], qubit=g["qubits"][0]) for g in value])
                    channels.append(dict(probability=str(Fraction(n, d)), anchor=anchor(instruction), action=action))
                    guards.append(guard(instruction))
                if any(g != guards[0] for g in guards):
                    raise RuntimeError("one category has inconsistent guards")
                identity = 1 - sum((Fraction(n, d) for n, d in probabilities), Fraction())
                categories.append(dict(identity=str(identity), guard=guards[0], channels=channels))
            rows.append(dict(text=text, categories=categories,
                internal_sites=len(categories) + sum(len(i.records) for i in circuit.instructions if not i.args),
                instructions=circuit.to_dict()["instructions"]))
        for text in ["M() 0", "H() 0", "DETECTOR()", "MPP() X0", "X_ERROR() 0"]:
            try:
                parse_circuit(text, controllers=registry, source="original-noise.magna")
            except ValueError as error:
                rejected_inputs.append(dict(text=text, error_type=type(error).__name__, message=str(error)))
            else:
                raise RuntimeError("native accepted an expected malformed empty argument list")
        if calls:
            raise RuntimeError("source metadata preparation invoked a controller")
        document = dict(schema="rmagna.original-noise-reference.v1", upstream_commit=captured.manifest["upstream"]["commit"],
            reference_manifest_sha256=captured.manifest_sha256, native_sha256=captured.native_sha256,
            generator_sha256=reference.digest(Path(__file__)), controller_calls=0, rows=rows, rejected_inputs=rejected_inputs)
    return document


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream)
    except ValueError as error:
        parser.error(str(error))
    document = capture(args.upstream.resolve())
    # Publish only after before/after native/source verification has completed.
    encoded = json.dumps(document, indent=2, sort_keys=True) + "\n"
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=destination.parent, delete=False) as stream:
        temporary = Path(stream.name)
        stream.write(encoded)
    try:
        os.link(temporary, destination)  # Atomic, exclusive publication; no overwrite.
    finally:
        temporary.unlink()
    print(f"Captured {len(document['rows'])} original noise metadata cases without quantum compilation")


if __name__ == "__main__":
    main()
