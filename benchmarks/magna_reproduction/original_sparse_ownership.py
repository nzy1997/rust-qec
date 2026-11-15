#!/usr/bin/env python3
"""Retain the complete original 100-draw sparse/dense ownership scenario."""
# SPDX-License-Identifier: MPL-2.0
from __future__ import annotations
import argparse
import copy
from fractions import Fraction
import importlib.util
import json
from pathlib import Path
import sys
import unittest
import reference
from witness_environment import native_reference
from witness_paths import admit_destinations

METHOD = "test_runtime_ownership.RuntimeOwnershipTests.test_sparse_noise_equals_dense_categories_and_isolation"
SOURCE = "tests/core/test_runtime_ownership.py"


def ratio(value):
    return str(Fraction(*value) if isinstance(value, tuple) else Fraction(value))


def capture(upstream, helper):
    upstream, helper = Path(upstream).resolve(), Path(helper).resolve()
    helper_hash = reference.digest(helper)
    with native_reference(upstream) as captured:
        if "_native_testing" in sys.modules:
            raise ValueError("fresh Python process required")
        spec = importlib.util.spec_from_file_location("_native_testing", helper)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        sys.modules["_native_testing"] = module
        sys.path.insert(0, str(upstream))
        sys.path.insert(0, str(upstream / "tests/core"))
        from tests.support import file_hooks as hooks
        dense_original, sparse_original = module.draw_file_noise, hooks.draw_many
        dense, sparse, tables = [], [], []

        def dense_draw(plan, seeds):
            seeds = tuple(seeds)
            result = dense_original(plan, seeds)
            dense.append(dict(seeds=seeds, rows=[dict(choices=list(c), weight=ratio(w), weight_raw=w) for c, w in result]))
            return result

        def sparse_draw(schedule, random, count):
            tables.append(copy.deepcopy(schedule.actions))
            result = sparse_original(schedule, random, count)
            sparse.append([dict(gates=row.gates, report_flips=row.report_flips,
                                weight=ratio(row.likelihood_ratio), weight_raw=row.likelihood_ratio) for row in result])
            return result

        module.draw_file_noise, hooks.draw_many = dense_draw, sparse_draw
        try:
            result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromName(METHOD))
        finally:
            module.draw_file_noise, hooks.draw_many = dense_original, sparse_original
        if not result.wasSuccessful() or result.testsRun != 1 or result.skipped:
            raise ValueError("the complete original sparse ownership method did not pass")
        if len(dense) != 1 or len(dense[0]["rows"]) != 100 or len(sparse) != 2 or any(len(r) != 100 for r in sparse):
            raise ValueError("original full 100-draw schedules changed")
        if sparse[0] != sparse[1] or tables[0] != tables[1]:
            raise ValueError("original action snapshots or draws changed after caller mutation")
        document = dict(schema="rmagna.original-sparse-ownership.v1",
                        upstream_commit=captured.manifest["upstream"]["commit"], method=METHOD,
                        source_sha256={SOURCE: captured.manifest["upstream"]["files"][SOURCE]},
                        generator_sha256=reference.digest(Path(__file__)),
                        reference_manifest_sha256=captured.manifest_sha256,
                        native_frontend_sha256=captured.native_sha256, native_helper_sha256=helper_hash,
                        seed=45, actions=tables[0], dense=dense[0], sparse=sparse[0],
                        original_methods_passed=1, original_dense_draws=100, original_sparse_draws=200)
    if reference.digest(helper) != helper_hash:
        raise ValueError("identified helper changed during capture")
    return document


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--native-helper", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream,
                                         protected_roots=[reference.HERE, args.native_helper.parent])
        result = capture(args.upstream, args.native_helper)
        with destination.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, separators=(",", ":"), allow_nan=False)
            stream.write("\n")
    except (ValueError, OSError) as error:
        parser.error(str(error))
    print("captured 100 dense and 200 sparse original draws; caller mutation and all malformed-table assertions passed")


if __name__ == "__main__":
    main()
