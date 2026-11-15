#!/usr/bin/env python3
"""Capture all inputs of four pinned optimization methods, without changing loops."""
# SPDX-License-Identifier: MPL-2.0
# Source-derived methods: itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
from __future__ import annotations

import argparse
import ast
import copy
import importlib.util
import json
from pathlib import Path
import random
import sys
import unittest

import reference
from witness_environment import native_reference
from witness_paths import admit_destinations

SOURCE = "tests/core/test_simulator_optimization.py"
SOURCE_SHA256 = "2391f35f00a33244677f0a903bbdb34a0ab4448da7cc454850e2b27e33596d3d"
METHODS = {
    "test_batched_inverse_same_free_coordinate_convention": "inverse",
    "test_common_support_overlap_including_affine_shift_and_phase": "overlap",
    "test_exact_sparse_contraction_with_dependent_boundaries": "contract",
    "test_packed_pullback_against_direct_quadratic_values": "pullback",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def capture(upstream, helper):
    upstream, helper = Path(upstream).resolve(), Path(helper).resolve()
    require(reference.digest(upstream / SOURCE) == SOURCE_SHA256, "changed original optimization source")
    helper_hash = reference.digest(helper)
    tree = ast.parse((upstream / SOURCE).read_text())
    cls = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "OptimizationTests")
    methods = [copy.deepcopy(node) for node in cls.body if isinstance(node, ast.FunctionDef) and node.name in METHODS]
    require(set(node.name for node in methods) == set(METHODS), "original method inventory changed")
    rows = {key: [] for key in METHODS.values()}
    with native_reference(upstream) as captured:
        # Admit a prebuilt, caller-identified helper. Never invoke the upstream
        # loader's fallback compiler or make another copy of its environment.
        require("_native_testing" not in sys.modules, "fresh Python process required")
        spec = importlib.util.spec_from_file_location("_native_testing", helper)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        sys.modules["_native_testing"] = module
        sys.path.insert(0, str(upstream))
        from magna.gf2 import image_basis
        from magna.operations import StabilizerKet
        from magna.scalar import ExactScalar
        from tests.support import operation_hooks as hooks

        def ket_data(ket):
            return {key: getattr(ket, key) for key in ("width", "base", "columns", "linear", "upper", "power", "phase")}

        def inverse(matrix, width):
            # Include the dependent-row rejection, not just successful calls.
            row = dict(width=width, rows=matrix)
            rows["inverse"].append(row)
            try:
                result = hooks.right_inverse_columns(matrix, width)
            except ValueError:
                row["expected"] = None
                raise
            row["expected"] = result
            return result

        def overlap(left, right):
            rows["overlap"].append(dict(left=ket_data(left), right=ket_data(right)))
            return hooks.overlap(left, right)

        def contract(width, base, columns, linear, upper, power, phase):
            rows["contract"].append(dict(width=width, base=base, columns=columns, linear=linear,
                                         upper=upper, power=power, phase=phase))
            return StabilizerKet.contract(width, base, columns, linear, upper, power, phase)

        def restrict(linear, upper, shift, basis):
            rows["pullback"].append(dict(linear=linear, upper=upper, shift=shift, basis=basis))
            return hooks.restrict(linear, upper, shift, basis)

        class CaptureContract(ast.NodeTransformer):
            def visit_Attribute(self, node):
                if isinstance(node.value, ast.Name) and node.value.id == "StabilizerKet" and node.attr == "contract":
                    return ast.copy_location(ast.Name(id="capture_contract", ctx=ast.Load()), node)
                return self.generic_visit(node)

        namespace = dict(random=random, unittest=unittest, image_basis=image_basis,
                         StabilizerKet=StabilizerKet, ExactScalar=ExactScalar, scalar=hooks.scalar,
                         solve=hooks.solve, right_inverse_columns=inverse, overlap=overlap,
                         restrict=restrict, capture_contract=contract)
        helpers = [copy.deepcopy(node) for node in tree.body if isinstance(node, ast.FunctionDef)
                   and node.name in ("quadratic", "tensor")]
        require([node.name for node in helpers] == ["quadratic", "tensor"], "original helper inventory changed")
        transformed = CaptureContract().visit(ast.Module(body=helpers + methods, type_ignores=[]))
        ast.fix_missing_locations(transformed)
        exec(compile(transformed, SOURCE, "exec"), namespace)
        original = unittest.TestCase()
        for method in methods:
            namespace[method.name](original)
        require({key: len(value) for key, value in rows.items()} == dict(inverse=157, overlap=140, contract=240, pullback=108),
                "original loop counts changed")
        result = dict(schema="rmagna.original-optimization-inputs.v1",
                      upstream_commit=captured.manifest["upstream"]["commit"],
                      source_sha256={SOURCE: SOURCE_SHA256}, methods=METHODS,
                      generator_sha256=reference.digest(Path(__file__)),
                      reference_manifest_sha256=captured.manifest_sha256,
                      native_frontend_sha256=captured.native_sha256,
                      native_helper_sha256=helper_hash,
                      native_usage="Input image-basis ordering and original assertion validation; Rust tensor and pullback expectations use independent exhaustive oracles",
                      seeds=dict(inverse=617108, overlap=617109, contract=617106, pullback=617107), **rows)
    require(reference.digest(helper) == helper_hash, "native helper changed during capture")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--native-helper", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream,
                                         protected_roots=[reference.HERE, args.native_helper.parent])
        document = capture(args.upstream, args.native_helper)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    with destination.open("x", encoding="utf-8") as stream:
        json.dump(document, stream, separators=(",", ":"), allow_nan=False)
        stream.write("\n")


if __name__ == "__main__":
    main()
