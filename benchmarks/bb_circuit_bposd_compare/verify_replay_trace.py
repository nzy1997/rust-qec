from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_TOP_LEVEL = (
    "schema_version",
    "case_id",
    "basis",
    "syndrome_support",
    "syndrome_weight",
    "expected_sampled_logical",
    "classification",
    "decoders",
)
REQUIRED_DECODER_FIELDS = (
    "decoder_impl",
    "status",
    "case_id",
    "basis",
    "syndrome_support",
    "expected_sampled_logical",
    "bp_osd_settings",
    "correction_support",
    "correction_weight",
    "residual_syndrome_matches",
    "residual_syndrome_weight",
    "residual_syndrome_support",
    "predicted_logical",
)


def verify_trace(trace: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for field in REQUIRED_TOP_LEVEL:
        if field not in trace:
            errors.append(f"trace missing {field}")
    if errors:
        return errors

    decoders = trace.get("decoders")
    if not isinstance(decoders, list):
        return ["trace decoders is not a list"]
    if len(decoders) != 2:
        return ["trace decoder entries must contain exactly two dict entries"]

    by_impl: dict[str, dict[str, Any]] = {}
    for entry in decoders:
        if not isinstance(entry, dict):
            return ["trace decoder entries must contain exactly two dict entries"]
        impl = entry.get("decoder_impl")
        if impl not in ("rbposd", "ldpc_bposd"):
            return [f"trace unexpected decoder entry {impl}"]
        if impl in by_impl:
            return [f"trace duplicate decoder entry {impl}"]
        by_impl[impl] = entry

    for impl in ("rbposd", "ldpc_bposd"):
        _verify_decoder_entry(trace, by_impl[impl], errors)

    pair_fields = (
        "case_id",
        "basis",
        "syndrome_support",
        "expected_sampled_logical",
    )
    if all(impl in by_impl for impl in ("rbposd", "ldpc_bposd")):
        if any(
            by_impl["rbposd"].get(field) != by_impl["ldpc_bposd"].get(field)
            for field in pair_fields
        ):
            errors.append("decoder entries are not paired on syndrome metadata")

    expected_classification = _expected_classification(by_impl)
    if trace.get("classification") != expected_classification:
        errors.append(
            "trace classification mismatch: "
            f"expected {expected_classification}, got {trace.get('classification')}"
        )
    return errors


def _verify_decoder_entry(
    trace: dict[str, Any],
    entry: dict[str, Any],
    errors: list[str],
) -> None:
    impl = str(entry.get("decoder_impl", "<unknown>"))
    if entry.get("status") != "ok":
        errors.append(f"{impl} decoder entry is not ok")
        return

    missing_fields = [field for field in REQUIRED_DECODER_FIELDS if field not in entry]
    for field in missing_fields:
        errors.append(f"{impl} missing {field}")
    if missing_fields:
        return

    for field in ("case_id", "basis", "syndrome_support", "expected_sampled_logical"):
        if entry.get(field) != trace.get(field):
            errors.append(f"{impl} is not paired with top-level {field}")

    correction_support = entry.get("correction_support")
    if not isinstance(correction_support, list) or not correction_support:
        errors.append(f"{impl} missing correction_support")
    elif entry.get("correction_weight") != len(correction_support):
        errors.append(f"{impl} correction_weight does not match correction_support")

    residual_support = entry.get("residual_syndrome_support")
    if not isinstance(residual_support, list):
        errors.append(f"{impl} residual_syndrome_support is not a list")
    elif entry.get("residual_syndrome_weight") != len(residual_support):
        errors.append(
            f"{impl} residual_syndrome_weight does not match residual_syndrome_support"
        )

    if not isinstance(entry.get("residual_syndrome_matches"), bool):
        errors.append(f"{impl} residual_syndrome_matches is not boolean")
    if not isinstance(entry.get("predicted_logical"), list) or not entry.get(
        "predicted_logical"
    ):
        errors.append(f"{impl} missing predicted_logical")


def _expected_classification(by_impl: dict[str, dict[str, Any]]) -> str:
    if any(entry.get("status") != "ok" for entry in by_impl.values()):
        return "incomplete"
    return (
        "matched"
        if by_impl["rbposd"].get("predicted_logical")
        == by_impl["ldpc_bposd"].get("predicted_logical")
        else "logical_prediction_mismatch"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace_path", type=Path)
    args = parser.parse_args(argv)

    trace = json.loads(args.trace_path.read_text())
    errors = verify_trace(trace)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(
        f"case_id={trace['case_id']} basis={trace['basis']} "
        f"classification={trace['classification']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
