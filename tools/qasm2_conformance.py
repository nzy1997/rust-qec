#!/usr/bin/env python3
"""Independent QASM2 checks; run after installing tools/qasm2-requirements.txt.

Qiskit's strict parser checks every rstim export, and its operators check full
unitaries and outcome-labelled quantum channels. Stim supplies a second Clifford
oracle and real upstream-exported input. All fixtures are deliberately small.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import json
from pathlib import Path
import subprocess
import tempfile

import numpy as np
import qiskit
from qiskit import qasm2
from qiskit.circuit import ClassicalRegister
from qiskit.quantum_info import Operator
import stim

ATOL = 1e-12
RTOL = 1e-11


def same_operator(expected, actual, *, atol=ATOL):
    """Choose one global phase for the entire operator, never one per column."""
    expected, actual = np.asarray(expected), np.asarray(actual)
    assert expected.shape == actual.shape
    pivot = np.unravel_index(np.argmax(np.abs(expected)), expected.shape)
    phase = actual[pivot] / expected[pivot]
    assert abs(abs(phase) - 1) < atol
    np.testing.assert_allclose(actual, phase * expected, atol=atol, rtol=RTOL)


def expanded(matrix, qubits, width):
    """Embed an independent Qiskit operator with its little-endian bit order."""
    size = 1 << width
    out = np.zeros((size, size), dtype=complex)
    mask = sum(1 << q for q in qubits)
    for col in range(size):
        local_col = sum(((col >> q) & 1) << j for j, q in enumerate(qubits))
        for local_row in range(1 << len(qubits)):
            row = (col & ~mask) | sum(
                ((local_row >> j) & 1) << q for j, q in enumerate(qubits)
            )
            out[row, col] = matrix[local_row, local_col]
    return out


def instrument(circuit):
    """Enumerate Kraus branches, retaining records and final classical storage.

    Kraus matrices contain every input column, so the Choi comparison also
    checks coherence with an untouched entangled reference. Reset branches are
    hidden; they are summed inside each visible classical label.
    """
    width = circuit.num_qubits
    assert width <= 3
    branches = [(np.eye(1 << width, dtype=complex), [False] * circuit.num_clbits, ())]

    def execute(block, current, qmap, cmap):
        for instruction in block.data:
            operation = instruction.operation
            qs = [qmap[block.find_bit(q).index] for q in instruction.qubits]
            cs = [cmap[block.find_bit(c).index] for c in instruction.clbits]
            if operation.name == "barrier":
                continue
            if operation.name == "if_else":
                register, expected = operation.condition
                condition_bits = list(register) if isinstance(register, ClassicalRegister) else [register]
                condition_indices = [cmap[block.find_bit(b).index] for b in condition_bits]
                updated = []
                for branch in current:
                    value = sum(int(branch[1][bit]) << j for j, bit in enumerate(condition_indices))
                    selected = 0 if value == expected else 1
                    if selected < len(operation.blocks):
                        updated.extend(execute(operation.blocks[selected], [branch], qs, cs))
                    else:
                        updated.append(branch)
                current = updated
                continue
            if operation.name in ("measure", "reset"):
                if operation.name == "measure":
                    matrices = [np.diag([1, 0]), np.diag([0, 1])]
                else:
                    matrices = [np.array([[1, 0], [0, 0]]), np.array([[0, 1], [0, 0]])]
                updated = []
                for kraus, bits, record in current:
                    for value, matrix in enumerate(matrices):
                        next_kraus = expanded(matrix, qs, width) @ kraus
                        if np.max(np.abs(next_kraus)) < 1e-14:
                            continue
                        next_bits, next_record = bits.copy(), record
                        if operation.name == "measure":
                            next_bits[cs[0]] = bool(value)
                            next_record += (bool(value),)
                        updated.append((next_kraus, next_bits, next_record))
                current = updated
                continue
            matrix = expanded(Operator(operation).data, qs, width)
            current = [(matrix @ kraus, bits, record) for kraus, bits, record in current]
        return current

    branches = execute(circuit, branches, list(range(width)), list(range(circuit.num_clbits)))
    result = defaultdict(lambda: np.zeros((4**width, 4**width), dtype=complex))
    for kraus, bits, record in branches:
        vector = kraus.reshape(-1)
        result[(record, tuple(bits))] += np.outer(vector, vector.conj())
    return dict(result)


def same_instrument(expected, actual):
    assert expected.keys() == actual.keys(), (expected.keys(), actual.keys())
    for key in expected:
        np.testing.assert_allclose(actual[key], expected[key], atol=ATOL, rtol=RTOL)


class Check:
    def __init__(self, binary, directory):
        self.binary = str(binary)
        self.directory = Path(directory)
        self.index = 0
        self.cases = 0

    def invoke(self, *arguments):
        completed = subprocess.run(
            [self.binary, *map(str, arguments)], capture_output=True, text=True, check=False, timeout=30
        )
        if completed.returncode:
            raise AssertionError(f"rstim failed: {arguments}\n{completed.stdout}\n{completed.stderr}")
        report = json.loads(completed.stdout)
        assert report["status"] == "ok", report
        return report

    def paths(self):
        self.index += 1
        prefix = self.directory / str(self.index)
        return prefix.with_suffix(".in"), prefix.with_suffix(".qasm2.json"), prefix.with_suffix(".qasm")

    def roundtrip(self, source, backend="near-clifford"):
        input_path, document, output = self.paths()
        input_path.write_text(source)
        self.invoke("circuit", "import-qasm", "--in", input_path, "--out", document, "--backend", backend)
        self.invoke("circuit", "export-qasm", "--in", document, "--in-format", "qasm2-json", "--out", output)
        exported = output.read_text()
        # Empty QASM is a legal rstim no-op. Qiskit also accepts this in strict mode.
        restored = qasm2.loads(exported, strict=True)
        self.cases += 1
        return restored, document

    def native_export(self, source):
        input_path, _, output = self.paths()
        input_path.write_text(source)
        self.invoke("circuit", "export-qasm", "--in", input_path, "--out", output)
        self.cases += 1
        return qasm2.loads(output.read_text(), strict=True)

    def sample_classical(self, document, expected):
        output = document.with_suffix(".01")
        self.invoke("circuit", "sample", "--in", document, "--in-format", "qasm2-json", "--backend", "near-clifford", "--result-view", "classical", "--shots", 16, "--seed", 71, "--out", output)
        assert output.read_text().splitlines() == [expected] * 16


def run(check):
    # Qiskit primitives avoid phase-ambiguous tool extensions of qelib1.inc.
    unitary_inputs = [
        "OPENQASM 2.0; qreg q[2]; U(0.37,-0.81,1.25) q[0]; CX q[0],q[1]; U(-pi/7,pi/3,2*pi) q[1];",
        "OPENQASM 2.0; qreg a[1]; qreg b[2]; U(pi/2,0,pi) a[0]; CX a[0],b[1]; U(0,0,pi/4) b[0];",
        "OPENQASM 2.0; gate twist(a,b) q { U(a,b,-a) q; } qreg q[2]; twist(pi/7,-pi/3) q; CX q[1],q[0];",
        "OPENQASM 2.0; qreg h[1]; creg x[1]; U(pi/2,0,pi) h[0];",
        "OPENQASM 2.0;",
    ]
    for source in unitary_inputs:
        restored, _ = check.roundtrip(source)
        same_operator(Operator(qasm2.loads(source, strict=True)).data, Operator(restored).data)

    clifford_inputs = [
        "H 0\nCX 0 1\nS 1\nH 2\nCZ 1 2",
        "SQRT_X 0\nSQRT_Y_DAG 1\nCY 0 1\nSWAP 1 2\nS_DAG 2",
        "X 0\nY 1\nZ 2\nSQRT_Y 0\nSQRT_X_DAG 1\nCX 2 0",
    ]
    for native in clifford_inputs:
        source = stim.Circuit(native)
        expected = stim.Tableau.from_circuit(source).to_unitary_matrix(endian="little")
        exported = check.native_export(native)
        # Stim's tableau API returns complex64; allow its ~1e-8 amplitude
        # rounding here. All Qiskit-to-Qiskit comparisons keep 1e-12/1e-11.
        assert expected.dtype == np.complex64
        same_operator(expected, Operator(exported).data, atol=2e-7)
    # The upstream exporter can emit undeclared sx/sxdg/swap extensions. Restrict imported upstream text to its portable common subset;
    # rstim deliberately does not assume extended qelib1 definitions.
    for native in [clifford_inputs[0], "X 0\nY 1\nZ 2\nCY 0 1\nCX 1 2\nS_DAG 2"]:
        source = stim.Circuit(native)
        expected = stim.Tableau.from_circuit(source).to_unitary_matrix(endian="little")
        restored, _ = check.roundtrip(source.to_qasm(open_qasm_version=2), backend="stabilizer")
        same_operator(expected, Operator(restored).data, atol=2e-7)

    # The Bell fixture is adapted from yao-rs 4a398fe's test_bell_state.
    # The second fixture combines its common U/rotation/T/CX unitary profile.
    # Its incomplete measurement path is not used as an oracle.
    # Source: https://github.com/GiggleLiu/yao-rs/blob/4a398fe0a641b98792f8d3c28c56c2b79e84cedb/src/unit_tests/qasm.rs
    yao_profiles = [
        'OPENQASM 2.0; include "qelib1.inc"; qreg q[2]; h q[0]; cx q[0],q[1];',
        'OPENQASM 2.0; include "qelib1.inc"; qreg q[2]; h q[0]; t q[1]; rx(0.3) q[0]; ry(-0.4) q[1]; rz(pi/7) q[0]; u3(0.2,-0.1,0.7) q[1]; cx q[0],q[1];',
    ]
    for source in yao_profiles:
        restored, _ = check.roundtrip(source)
        same_operator(Operator(qasm2.loads(source, strict=True)).data, Operator(restored).data)

    # Native half-turn rotations are compared to independently specified QASM
    # operators, including a reset-name collision regression (R_Z vs RZ).
    native = "R_X(0.25) 0\nR_Y(-0.125) 1\nR_Z(0.1) 0\nT 1\nCX 1 0\nT_DAG 0"
    expected = 'OPENQASM 2.0; include "qelib1.inc"; qreg q[2]; rx(pi/4) q[0]; ry(-pi/8) q[1]; rz(pi/10) q[0]; t q[1]; cx q[1],q[0]; tdg q[0];'
    same_operator(Operator(qasm2.loads(expected, strict=True)).data, Operator(check.native_export(native)).data)

    dynamic_inputs = [
        "OPENQASM 2.0; qreg q[1]; creg c[2]; U(pi/2,0,pi) q[0]; measure q[0] -> c[0]; U(pi/2,0,pi) q[0]; measure q[0] -> c[1];",
        "OPENQASM 2.0; qreg q[2]; creg c[1]; creg d[1]; U(pi/2,0,pi) q[0]; measure q[0] -> c[0]; if(c==1) U(pi,0,pi) q[1]; measure q[1] -> c[0]; if(c==0) U(0,0,pi) q[0]; measure q[0] -> d[0];",
        "OPENQASM 2.0; qreg q[2]; creg c[1]; U(pi/2,0,pi) q[0]; CX q[0],q[1]; reset q[0]; measure q[1] -> c[0];",
        "OPENQASM 2.0; qreg q[1]; creg unused[2]; creg c[1]; if(c==0) U(pi,0,pi) q[0]; measure q[0] -> c[0]; reset q[0]; measure q[0] -> c[0];",
    ]
    for source in dynamic_inputs:
        restored, _ = check.roundtrip(source)
        same_instrument(instrument(qasm2.loads(source, strict=True)), instrument(restored))

    deterministic = "OPENQASM 2.0; qreg q[1]; creg unused[2]; creg c[2]; U(pi,0,pi) q[0]; measure q[0] -> c[1]; reset q[0]; measure q[0] -> c[0];"
    restored, document = check.roundtrip(deterministic, backend="stabilizer")
    same_instrument(instrument(qasm2.loads(deterministic, strict=True)), instrument(restored))
    check.sample_classical(document, "0001")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    if qiskit.__version__ != "2.2.3" or stim.__version__ != "1.15.0":
        raise SystemExit("Install the pinned tools/qasm2-requirements.txt versions")
    with tempfile.TemporaryDirectory(prefix="rstim-qasm2-") as directory:
        check = Check(args.binary.resolve(), directory)
        run(check)
        print(json.dumps({"ok": True, "cases": check.cases, "qiskit": qiskit.__version__, "stim": stim.__version__}))


if __name__ == "__main__":
    main()
