#!/usr/bin/env python3
"""Load aligned rstim detector, observable, and sample-trace files."""

import argparse
import json
from pathlib import Path


def read_b8(path: Path, bits_per_shot: int, shots: int) -> list[list[int]]:
    raw = path.read_bytes()
    bytes_per_shot = (bits_per_shot + 7) // 8
    expected = bytes_per_shot * shots
    if len(raw) != expected:
        raise ValueError(f"{path} has {len(raw)} bytes; expected {expected}")
    return [
        [
            (raw[shot * bytes_per_shot + bit // 8] >> (bit % 8)) & 1
            for bit in range(bits_per_shot)
        ]
        for shot in range(shots)
    ]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--detectors", type=Path, required=True)
    parser.add_argument("--observables", type=Path, required=True)
    parser.add_argument("--trace", type=Path, required=True)
    args = parser.parse_args()

    records = [json.loads(line) for line in args.trace.read_text().splitlines()]
    if not records or records[0].get("schema_version") != "rstim.sample_trace.v1":
        raise ValueError("trace does not start with an rstim.sample_trace.v1 manifest")
    manifest, shot_records = records[0], records[1:]
    shots = manifest["shots"]
    if len(shot_records) != shots:
        raise ValueError(f"trace contains {len(shot_records)} shots; expected {shots}")

    detector_inputs = read_b8(
        args.detectors, manifest["num_detectors"], shots
    )
    observable_targets = read_b8(
        args.observables, manifest["num_observables"], shots
    )
    for shot, record in enumerate(shot_records):
        if record["shot_index"] != shot:
            raise ValueError(f"trace shot index {record['shot_index']} is not {shot}")
        if [int(bit) for bit in record["detectors"]] != detector_inputs[shot]:
            raise ValueError(f"detector mismatch at shot {shot}")
        if [int(bit) for bit in record["observables"]] != observable_targets[shot]:
            raise ValueError(f"observable mismatch at shot {shot}")

    # Fixed-width simulator-only example features: declared noise sites,
    # occurred noise sites, loss-caused measurements, and suppressed sites.
    # More specialized trainers can encode raw_error_records by op_path.
    raw_error_records = [record["noise_events"] for record in shot_records]
    error_features = [
        [
            len(record["noise_events"]),
            sum(event["occurred"] for event in record["noise_events"]),
            sum(event["loss_cause"] for event in record["measurement_events"]),
            len(record["inapplicable_noise_events"]),
        ]
        for record in shot_records
    ]
    assert len(detector_inputs) == len(observable_targets) == len(error_features)
    assert len(raw_error_records) == shots
    print(f"PASS training alignment shots={shots}")


if __name__ == "__main__":
    main()
