#!/usr/bin/env python3
"""Load and verify an rstim blinded decoder dataset using only the stdlib."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def read_b8_rows(path: Path, *, shots: int, bits: int) -> list[list[int]]:
    row_bytes = (bits + 7) // 8
    raw = path.read_bytes()
    expected = shots * row_bytes
    if len(raw) != expected:
        raise ValueError(f"{path}: expected {expected} bytes, found {len(raw)}")
    return [
        [
            (raw[shot * row_bytes + bit // 8] >> (bit % 8)) & 1
            for bit in range(bits)
        ]
        for shot in range(shots)
    ]


def load_blinded_training_rows(
    public_dir: Path,
    private_dir: Path,
    observable_rec_offsets: list[int],
) -> dict[str, Any]:
    public = json.loads((public_dir / "manifest.json").read_text())
    private = json.loads((private_dir / "manifest.json").read_text())
    if public["dataset_id"] != private["dataset_id"]:
        raise ValueError("public and private manifests have different dataset_id values")
    if public["mode"] != "measurements_blinded" or private["mode"] != public["mode"]:
        raise ValueError("loader requires matching measurements_blinded bundles")
    shots = public["shots"]
    if private["shots"] != shots:
        raise ValueError("public and private shot counts differ")

    measurement_bits = public["row"]["bits"]
    measurement_inputs = read_b8_rows(
        public_dir / public["shots_file"]["file"], shots=shots, bits=measurement_bits
    )
    answer_targets = read_b8_rows(
        private_dir / private["answers_file"]["file"], shots=shots, bits=1
    )
    logical_masks = read_b8_rows(
        private_dir / private["masks_file"]["file"], shots=shots, bits=1
    )

    trace_entry = private.get("trace_file")
    if trace_entry is None:
        raise ValueError("private manifest has no trace_file; export with --error_trace")
    trace_lines = [
        json.loads(line)
        for line in (private_dir / trace_entry["file"]).read_text().splitlines()
    ]
    if len(trace_lines) != shots:
        raise ValueError(f"trace has {len(trace_lines)} lines, expected {shots}")

    resolved_indices: list[int] = []
    for offset in observable_rec_offsets:
        if offset >= 0 or -offset > measurement_bits:
            raise ValueError(
                f"observable record offset {offset} is outside rec[-1]..rec[-{measurement_bits}]"
            )
        resolved_indices.append(measurement_bits + offset)

    for shot, trace in enumerate(trace_lines):
        if trace.get("shot") != shot:
            raise ValueError(f"trace line {shot} has shot={trace.get('shot')!r}")
        logical_input = trace.get("logical_input")
        if logical_input is None:
            raise ValueError(f"trace line {shot} has no logical_input metadata")
        mask = logical_masks[shot][0]
        input_bit = logical_input.get("bit")
        if type(input_bit) is not int or input_bit not in (0, 1):
            raise ValueError(f"trace line {shot} logical_input.bit is not a binary integer")
        if input_bit != mask:
            raise ValueError(f"trace line {shot} logical_input.bit disagrees with masks.b8")
        applied = logical_input.get("applied")
        if type(applied) is not bool or int(applied) != mask:
            raise ValueError(f"trace line {shot} logical_input.applied disagrees with masks.b8")
        if logical_input.get("pauli") not in ("X", "Z"):
            raise ValueError(f"trace line {shot} has invalid logical_input.pauli")
        if not isinstance(logical_input.get("support"), list):
            raise ValueError(f"trace line {shot} has invalid logical_input.support")

        public_observable = 0
        for index in resolved_indices:
            public_observable ^= measurement_inputs[shot][index]
        if answer_targets[shot][0] != public_observable ^ mask:
            raise ValueError(f"shot {shot} violates answer = O_public(measurement) XOR mask")

    return {
        "measurement_inputs": measurement_inputs,
        "answer_targets": answer_targets,
        "logical_masks": logical_masks,
        "trace_records": trace_lines,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public-dir", type=Path, required=True)
    parser.add_argument("--private-dir", type=Path, required=True)
    parser.add_argument(
        "--observable-rec",
        type=int,
        action="append",
        required=True,
        help="rec[-k] offset contributing to observable 0; repeat for XOR terms",
    )
    args = parser.parse_args()
    rows = load_blinded_training_rows(
        args.public_dir, args.private_dir, args.observable_rec
    )
    print(f"PASS training alignment shots={len(rows['measurement_inputs'])}")


if __name__ == "__main__":
    main()
