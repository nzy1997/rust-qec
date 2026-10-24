#!/usr/bin/env python3
"""Capture unweighted original-category responses from pinned enumeration."""
import argparse
import json
from pathlib import Path


def generate(captured):
    from magna import ControllerRegistry, enumerate_errors, parse_circuit
    cases = [
        dict(case="joint_report_selection", source="X_ERROR(1/4) 0\nX_ERROR(1/5) 1\nM(1/7) 0\nM 1\nDETECTOR rec[-2]\nPOSTSELECT det[0]",
             task={"outputs": [0, 1], "observables": [[0, 1], [0, 2], [0, 3]]}, ideal="I 0 1", max_weight=3),
        dict(case="categorical_chain", source="PAULI_CHANNEL_1(1/10,1/5,3/10) 0\nE(1/10) X0\nELSE_CORRELATED_ERROR(2/9) Z0\nY_ERROR(0) 0",
             task={"outputs": [0], "observables": [[0, 1]]}, ideal="I 0", max_weight=1),
        dict(case="forced_empty", source="X_ERROR(1) 0", task={"outputs": [0]}, ideal="I 0", max_weight=0),
        dict(case="forced_full", source="X_ERROR(1) 0", task={"outputs": [0]}, ideal="I 0", max_weight=1),
        dict(case="nonlinear_guard", source="H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nX_ERROR(1/5) ctrl[0] 1\nM 1",
             task={"outputs": [1], "observables": [[0, 1]]}, ideal="I 0", max_weight=1),
        dict(case="projected_target", source="H 0\nT 0\nH 0\nX_ERROR(1/5) 0", task={"outputs": [0], "observables": [[0, 1]]},
             ideal="H 0\nCX 0 1", target_outputs=[0], target_postselection={1: 1}, max_weight=1),
    ]
    vectors = []
    for case in cases:
        registry = ControllerRegistry()
        registry.register("copy", lambda bits, config: bits, version="1")
        target = parse_circuit(case["ideal"])
        if "target_outputs" in case:
            target = dict(circuit=target, outputs=case["target_outputs"], postselection=case["target_postselection"])
        parsed = parse_circuit(case["source"], controllers=registry)
        operations = {inst.op_id: j for j, inst in enumerate(parsed.instructions)}
        records = {f"m{record}": j for j, inst in enumerate(parsed.instructions) for record in inst.records}
        result = enumerate_errors(parsed,
                                  task=case["task"], ideal_state=target, controllers=registry,
                                  max_weight=case["max_weight"], include_patterns=True)
        if result["status"] != "complete":
            raise RuntimeError(f"native pattern capture failed: {case['case']}: {result['failures']}")
        category_sources = [[records[channel["report"]] if channel["operation_id"] == "report"
                             else operations[channel["operation_id"]] for channel in category["channels"]]
                            for category in result["category_definitions"]]
        vectors.append({**case, "patterns": result["patterns"], "category_source_operations": category_sources,
                        "histories": result["feedback_branches"],
                        "target_normalization": result["totals"]["exact"].get("target_normalization", {
                            "coefficients_hex": ["0x1", "0x0", "0x0", "0x0"], "exponent2": 0})})
    return dict(schema="rmagna.pattern-response-reference.v1",
                upstream_commit=captured.manifest["upstream"]["commit"],
                native_sha256=captured.native_sha256,
                reference_manifest_sha256=captured.manifest_sha256, vectors=vectors)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    from witness_environment import native_reference
    with native_reference(args.upstream) as captured:
        data = generate(captured)
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
