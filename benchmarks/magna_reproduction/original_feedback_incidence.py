#!/usr/bin/env python3
"""Capture the complete 162-case original partial-decision incidence witness."""
# SPDX-License-Identifier: MPL-2.0
# Source: itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f, test_feedback_incidence.py.
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

METHOD = "test_feedback_incidence.FeedbackIncidenceTest.test_partial_decisions_xors_versions_constants_and_wire_remap"
SOURCE = "tests/core/test_feedback_incidence.py"


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
        from tests.support.feedback_template import FeedbackTemplate
        observations, templates = [], []
        original = FeedbackTemplate.faults
        def target(t):
            return [t.kind, t.index, t.version]
        def faults(self, values, *, noise=None, prefix="", wires=None):
            found = list(original(self, values, noise=noise, prefix=prefix, wires=wires))
            anchors = [row[0] for row in self.updates]
            snapshot = dict(wire_count=self.wire_count,
                updates=[dict(anchor=a, wire=w, pauli=p, terms=[target(t) for t in row],
                              offset=self.selection_offsets.get(a, 0)) for a,w,p,row in self.updates])
            if not templates:
                templates.append(snapshot)
            elif templates[0] != snapshot:
                raise ValueError("original immutable template changed")
            observations.append(dict(values=[[target(t), b] for t,b in values.items()], prefix=prefix,
                faults=[dict(row=anchors.index(f["after"][len(prefix):]),wire=f["qubit"],pauli=f["pauli"]) for f in found]))
            return iter(found)
        FeedbackTemplate.faults = faults
        try:
            result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromName(METHOD))
        finally:
            FeedbackTemplate.faults = original
        if not result.wasSuccessful() or result.testsRun != 1 or result.skipped:
            raise ValueError("the complete original incidence method did not pass")
        if len(observations) != 162 or len(templates) != 1:
            raise ValueError("original full partial-decision schedule changed")
        document = dict(schema="rmagna.original-feedback-incidence.v1",
                        upstream_commit=captured.manifest["upstream"]["commit"],
                        method=METHOD, source_sha256={SOURCE: captured.manifest["upstream"]["files"][SOURCE]},
                        generator_sha256=reference.digest(Path(__file__)),
                        reference_manifest_sha256=captured.manifest_sha256,
                        native_frontend_sha256=captured.native_sha256, native_helper_sha256=helper_hash,
                        original_methods_passed=1, original_queries=162, template=templates[0], cases=observations)
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
    print(f"captured {len(result['cases'])} incidence cases from {result['original_queries']} original queries")


if __name__ == "__main__":
    main()
