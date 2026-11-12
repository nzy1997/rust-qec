"""Exact references for the two pinned original FileApi gate-matrix methods.

Only selected NumPy helpers, the preparation prefix and two method bodies are
executed. Magna is never imported. Exact arithmetic uses Fraction in Q(omega).
"""
# SPDX-License-Identifier: MPL-2.0
from __future__ import annotations

import argparse
import ast
import copy
from fractions import Fraction
import hashlib
import io
import json
import math
from pathlib import Path
import platform
import unittest

PIN = "56847478c840395735146ef1e1ccdc3d8aedc10f"
SOURCE_SHA256 = "03e38048973e84253d6df3f90e8672bb4ae1f07f7f5b0168c3057530e0256e30"
METHODS = (
    "test_supported_clifford_gates_against_stim",
    "test_magic_and_controlled_extensions_against_independent_matrices",
)
ZERO = (Fraction(0),) * 4
ONE = (Fraction(1), *ZERO[1:])
IMAGINARY = (Fraction(0), Fraction(0), Fraction(1), Fraction(0))
OMEGA = (Fraction(0), Fraction(1), Fraction(0), Fraction(0))
SQRT_HALF = (Fraction(0), Fraction(1, 2), Fraction(0), Fraction(-1, 2))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def scale(a, factor):
    return tuple(x * factor for x in a)


def multiply(a, b):
    result = list(ZERO)
    for i, x in enumerate(a):
        for j, y in enumerate(b):
            degree = i + j
            result[degree % 4] += x * y * (-1 if degree >= 4 else 1)
    return tuple(result)


def conjugate(a):
    return a[0], -a[3], -a[2], -a[1]


def as_complex(a):
    omega = complex(math.sqrt(0.5), math.sqrt(0.5))
    return sum(float(value) * omega**power for power, value in enumerate(a))


def lift_component(value):
    candidates = (
        (0.0, ZERO), (1.0, ONE), (-1.0, scale(ONE, -1)),
        (0.5, scale(ONE, Fraction(1, 2))), (-0.5, scale(ONE, Fraction(-1, 2))),
        (math.sqrt(0.5), SQRT_HALF), (-math.sqrt(0.5), scale(SQRT_HALF, -1)),
    )
    approximation, exact = min(candidates, key=lambda item: abs(value - item[0]))
    require(math.isfinite(value) and abs(value - approximation) < 2e-7,
            "matrix component outside the pinned finite algebraic values")
    return exact


def lift(value):
    return add(lift_component(float(value.real)),
               multiply(IMAGINARY, lift_component(float(value.imag))))


def tensor(high, low):
    return [multiply(a, b) for a in high for b in low]


def prepared_input(width):
    result = [SQRT_HALF, SQRT_HALF]
    if width >= 2:
        result = tensor([SQRT_HALF, multiply(scale(IMAGINARY, -1), SQRT_HALF)], result)
    if width == 3:
        result = tensor([SQRT_HALF, multiply(OMEGA, SQRT_HALF)], result)
    return result


def apply(matrix, vector):
    return [sum_exact(multiply(a, b) for a, b in zip(row, vector)) for row in matrix]


def sum_exact(values):
    result = ZERO
    for value in values:
        result = add(result, value)
    return result


def exact_moment(vector, x, z):
    powers = (ONE, IMAGINARY, scale(ONE, -1), scale(IMAGINARY, -1))
    phase = powers[(x & z).bit_count() % 4]
    return sum_exact(
        scale(multiply(multiply(conjugate(vector[k ^ x]), vector[k]), phase),
              -1 if (k & z).bit_count() % 2 else 1)
        for k in range(len(vector))
    )


def validate_unitary(matrix):
    size = len(matrix)
    for i in range(size):
        for j in range(size):
            value = sum_exact(multiply(conjugate(matrix[k][i]), matrix[k][j]) for k in range(size))
            require(value == (ONE if i == j else ZERO), "lifted matrix is not exactly unitary")


def source_text(upstream):
    source = upstream / "tests/core/test_file_api.py"
    data = source.read_bytes()
    require(hashlib.sha256(data).hexdigest() == SOURCE_SHA256, "changed pinned file-api source")
    return data.decode()


def capture(upstream):
    text = source_text(upstream)
    # These optional reference dependencies are deliberately loaded only here.
    import numpy as np
    import stim

    tree = ast.parse(text)
    original = next(node for node in tree.body if isinstance(node, ast.ClassDef)
                    and node.name == "FileApiTests")
    methods = [node for node in original.body if isinstance(node, ast.FunctionDef) and node.name in METHODS]
    require(tuple(node.name for node in methods) == METHODS, "original method inventory changed")
    preparation = copy.deepcopy(next(node for node in original.body
                                    if isinstance(node, ast.FunctionDef) and node.name == "assert_unitary"))
    end = next(i for i, node in enumerate(preparation.body)
               if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "result"
                                                      for t in node.targets))
    preparation.body = preparation.body[:end] + [ast.Return(ast.Tuple(
        elts=[ast.Name(id=name, ctx=ast.Load()) for name in ("circuit", "masks", "expected")],
        ctx=ast.Load()))]
    preparation.name = "prepare_matrix"
    preparation.decorator_list = []
    constants = {"I", "X", "Y", "Z", "H", "S", "T"}
    helpers = [copy.deepcopy(node) for node in tree.body
               if (isinstance(node, ast.Assign) and len(node.targets) == 1
                   and isinstance(node.targets[0], ast.Name) and node.targets[0].id in constants)
               or (isinstance(node, ast.FunctionDef) and node.name in {"controlled", "moments"})]
    namespace = dict(np=np, math=math, unittest=unittest, stim=stim)
    module = ast.fix_missing_locations(ast.Module(body=helpers + [preparation], type_ignores=[]))
    exec(compile(module, "pinned-file-api-matrix-prefix", "exec"), namespace)
    cases = []

    class Collector(unittest.TestCase):
        def assert_unitary(self, gate, numpy_matrix):
            circuit, masks, expected = namespace["prepare_matrix"](self, gate, numpy_matrix)
            width = int(math.log2(len(numpy_matrix)))
            require(width in (1, 2, 3), "unexpected original matrix width")
            matrix = [[lift(value) for value in row] for row in numpy_matrix]
            validate_unitary(matrix)
            vector = apply(matrix, prepared_input(width))
            exact = [exact_moment(vector, x, z) for x, z in masks]
            require(all(value == conjugate(value) for value in exact), "nonreal Pauli expectation")
            np.testing.assert_allclose([as_complex(value).real for value in exact], expected, atol=2e-7)
            cases.append(dict(gate=gate, qubits=width, circuit=circuit, paulis=masks,
                exact=[[[str(c.numerator), str(c.denominator)] for c in value] for value in exact],
                numpy=[float(value) for value in expected]))

    namespace["Collector"] = Collector
    method_module = ast.fix_missing_locations(ast.Module(body=[ast.ClassDef(
        name="OriginalMatrixMethods", bases=[ast.Name(id="Collector", ctx=ast.Load())],
        keywords=[], body=copy.deepcopy(methods), decorator_list=[])], type_ignores=[]))
    exec(compile(method_module, "pinned-file-api-matrix-methods", "exec"), namespace)
    output = io.StringIO()
    result = unittest.TextTestRunner(stream=output).run(unittest.TestSuite(
        namespace["OriginalMatrixMethods"](name) for name in METHODS))
    require(result.wasSuccessful() and result.testsRun == 2 and not result.skipped, output.getvalue())
    require(len(cases) == 28 and len({case["gate"] for case in cases}) == 28, "original gate coverage")
    require(sum(len(case["paulis"]) for case in cases) == 312, "original Pauli coverage")
    return dict(schema="rstim.original-file-matrices.v1", upstream_commit=PIN,
        source_sha256=SOURCE_SHA256, generator_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        versions=dict(python=platform.python_version(), numpy=np.__version__, stim=stim.__version__),
        scope="original fixed prepared inputs, exact Fraction Q(omega) moments",
        method_source_sha256={node.name: hashlib.sha256(ast.get_source_segment(text, node).encode()).hexdigest()
                              for node in methods},
        cases=cases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    result = capture(args.upstream)
    with args.out.open("x") as output:
        json.dump(result, output, indent=2, allow_nan=False)
        output.write("\n")


if __name__ == "__main__":
    main()
