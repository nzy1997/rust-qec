#!/usr/bin/env python3
"""Capture all 64 source ownership queries with an identified prebuilt helper."""
# SPDX-License-Identifier: MPL-2.0
# Source method: itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
# tests/core/test_query_reuse.py::QueryReuse.test_owned_preparation_matches_public_with_actual_controls_and_faults.
from __future__ import annotations
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import unittest
import reference
from witness_environment import native_reference
from witness_paths import admit_destinations

METHOD = "test_query_reuse.QueryReuse.test_owned_preparation_matches_public_with_actual_controls_and_faults"
SOURCE = "tests/core/test_query_reuse.py"


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
        from magna.operations import NativeBoundaryTrace
        observations = []
        original = NativeBoundaryTrace.query

        def query(self, *args, **kwargs):
            result = original(self, *args, **kwargs)
            observations.append(dict(outcomes=list(kwargs["outcomes"]),
                                     x=kwargs["x"], z=kwargs["z"], raw=result.raw, value=result.total().to_dict()))
            return result

        NativeBoundaryTrace.query = query
        try:
            result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromName(METHOD))
        finally:
            NativeBoundaryTrace.query = original
        if not result.wasSuccessful() or result.testsRun != 1 or result.skipped:
            raise ValueError("the complete original ownership method did not pass")
        if len(observations) != 128:
            raise ValueError("original full two-plan query schedule changed")
        cases = []
        for index in range(64):
            public, owned = observations[2 * index:2 * index + 2]
            if public != owned:
                raise ValueError("original public/owned observations differ")
            x, z = ((0, 0), (1, 0), (0, 1), (1, 1))[index % 4]
            bit = index // 4 % 2
            if (public["x"], public["z"], public["outcomes"]) != (x, z, [bit]):
                raise ValueError("original query ordering changed")
            cases.append(dict(pauli=("I", "X", "Y", "Z")[index // 16],
                              flipped=bool(index // 8 % 2), outcome=bool(bit),
                              x=x, z=z, raw=public["raw"], expected=public["value"]))
        document = dict(schema="rmagna.original-snapshot-queries.v1",
                        upstream_commit=captured.manifest["upstream"]["commit"],
                        method=METHOD, source_sha256={SOURCE: captured.manifest["upstream"]["files"][SOURCE]},
                        generator_sha256=reference.digest(Path(__file__)),
                        reference_manifest_sha256=captured.manifest_sha256,
                        native_frontend_sha256=captured.native_sha256, native_helper_sha256=helper_hash,
                        original_methods_passed=1, original_queries=128, cases=cases)
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
    print(f"captured {len(result['cases'])} ownership cases from {result['original_queries']} original queries")


if __name__ == "__main__":
    main()
