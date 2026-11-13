#!/usr/bin/env python3
"""Capture the pinned original full-record kernel schedule with physical matrices."""
# SPDX-License-Identifier: MPL-2.0
from __future__ import annotations
import argparse
import ast
import copy
import json
import math
from pathlib import Path
import platform
import sys

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import reference
from witness_environment import native_reference
from witness_paths import admit_destinations

PIN = "56847478c840395735146ef1e1ccdc3d8aedc10f"
SOURCES = {
    "tests/core/test_kernel_contract.py": "8118024904c89019028d756dc412974f67347ba7c5809efdfd05b5ac26ec4307",
    "tests/core/test_unified_simulator.py": "aa9bf8aaba473d4fc0c8490c99ef6374eff5a81197b7469eaade744c790f704e",
}
METHOD = "test_random_full_records_states_faults_and_moments_against_independent_matrices"
PAULIS = ((1, 0), (1, 1), (0, 1), (3, 2))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_inputs(upstream):
    manifest = reference.load_manifest()
    require(manifest["upstream"]["commit"] == PIN, "wrong pinned revision")
    before = reference.verify_source(upstream, manifest)
    require(before["source_files"] == 475, "wrong original source inventory")
    for name, sha in SOURCES.items():
        require(reference.digest(upstream / name) == sha, "changed pinned kernel/matrix source")
    return before


def finite_pair(value):
    result = [float(value.real), float(value.imag)]
    require(all(math.isfinite(x) for x in result), "nonfinite independent matrix component")
    return result


def capture(upstream):
    upstream = Path(upstream).resolve()
    before = verify_inputs(upstream)
    # Optional dependencies and the native frontend are admitted only after the
    # complete source inventory. Expected vectors use untouched NumPy helpers.
    with native_reference(upstream) as captured:
        import itertools
        import random
        import numpy as np
        from magna.operations import InstrumentIR, SampleAssignment, StabilizerKet
        from magna.ir import instantiate_branch

        matrix_tree = ast.parse((upstream / "tests/core/test_unified_simulator.py").read_text())
        helpers = [node for node in matrix_tree.body if isinstance(node, ast.FunctionDef)
                   and node.name in ("ket_vector", "physical_vector")]
        require([node.name for node in helpers] == ["ket_vector", "physical_vector"], "matrix helper inventory changed")
        namespace = dict(np=np)
        exec(compile(ast.Module(body=helpers, type_ignores=[]), "pinned-independent-physical-matrices", "exec"), namespace)
        tree = ast.parse((upstream / "tests/core/test_kernel_contract.py").read_text())
        cls = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "KernelContractTests")
        original = next(node for node in cls.body if isinstance(node, ast.FunctionDef) and node.name == METHOD)
        method = copy.deepcopy(original)
        method.name = "capture_original_schedule"
        method.args.args = []
        outer = method.body[1]
        require(isinstance(outer, ast.For), "original trial loop changed")
        stop = next(i for i, node in enumerate(outer.body) if isinstance(node, ast.Assign)
                    and any(isinstance(t, ast.Name) and t.id == "new" for t in node.targets))
        branches = copy.deepcopy(next(node for node in outer.body if isinstance(node, ast.For)
                                     and isinstance(node.target, ast.Tuple)))
        require([ast.unparse(node.targets[0]) for node in branches.body[:2]] == ["assignment", "expected"], "original branch prefix changed")
        branches.body = branches.body[:2] + [ast.Expr(ast.Call(func=ast.Name(id="capture_branch", ctx=ast.Load()),
            args=[ast.Name(id=name, ctx=ast.Load()) for name in ("ir", "assignment", "expected", "trial")], keywords=[]))]
        outer.body = outer.body[:stop] + [branches]
        method.body = method.body[:2]
        ast.fix_missing_locations(method)
        rows = []
        def capture_branch(ir, assignment, vector, trial):
            require(len(vector) == 8, "original output/reset vector width changed")
            ket = ir.initial
            norm = float(np.vdot(vector, vector).real)
            moments = []
            for x, z in PAULIS:
                moments.append(finite_pair(sum(vector[y ^ x].conjugate() * v * (1j)**((x & z).bit_count())
                                               * (-1)**((z & y).bit_count()) for y, v in enumerate(vector))))
            rows.append(dict(trial=trial, inputs=list(ir.inputs), outputs=list(ir.outputs),
                initial={name: getattr(ket, name) for name in ("width", "base", "columns", "linear", "upper", "power", "phase")},
                operations=ir.operations, true_values=assignment.true, faults=assignment.faults,
                report_flips=assignment.report_flips, vector=[finite_pair(v) for v in vector],
                probability=norm, moments=moments))
        namespace.update(dict(itertools=itertools, random=random, InstrumentIR=InstrumentIR,
            SampleAssignment=SampleAssignment, StabilizerKet=StabilizerKet,
            instantiate_branch=instantiate_branch, capture_branch=capture_branch))
        exec(compile(ast.Module(body=[method], type_ignores=[]), "pinned-original-kernel-schedule", "exec"), namespace)
        namespace[method.name]()
        require(len(rows) == 96, "original branch inventory changed")
        require([row["trial"] for row in rows] == [trial for trial in range(24) for _ in range(4)], "original trial order changed")
        for trial in range(24):
            require(abs(sum(row["probability"] for row in rows if row["trial"] == trial) - 1) < 4e-13, "incomplete original record distribution")
        document = dict(schema="rmagna.original-kernel-matrices.v1", upstream_commit=PIN,
            original_method="test_kernel_contract.KernelContractTests." + METHOD, seed=26092393,
            source_files=before["source_files"], source_sha256=SOURCES,
            reference_manifest_sha256=captured.manifest_sha256, native_frontend_sha256=captured.native_sha256,
            native_usage="IR metadata validation and branch materialization; no quantum probability or decomposition expectations",
            generator_sha256=reference.digest(Path(__file__)), numpy_version=np.__version__,
            python_version=platform.python_version(), paulis=PAULIS, rows=rows)
    require(reference.verify_source(upstream, reference.load_manifest()) == before, "reference inventory changed")
    return document


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream, protected_roots=[reference.HERE])
        document = capture(args.upstream)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    # Exclusive creation also rejects a destination created during capture.
    with destination.open("x", encoding="utf-8") as stream:
        json.dump(document, stream, indent=2, allow_nan=False)
        stream.write("\n")


if __name__ == "__main__":
    main()
