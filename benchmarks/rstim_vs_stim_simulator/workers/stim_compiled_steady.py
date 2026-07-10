from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator import run_compiled_steady as protocol


def _fixture_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the compiled steady-state Stim worker.")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--seed", type=int, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    import stim

    if stim.__version__ != "1.15.0":
        print(f"requires stim==1.15.0, got {stim.__version__}", file=sys.stderr)
        return 1

    input_text = args.input.read_text(encoding="utf-8")
    circuit = stim.Circuit(input_text)
    sampler = circuit.compile_sampler(seed=args.seed)
    measurement_count = circuit.num_measurements
    telemetry = {
        "variant": "stim",
        "compile_count": 1,
        "reference_build_count": 1,
        "sample_call_count": 0,
        "fixture_sha256": _fixture_sha256(args.input),
        "measurement_count": measurement_count,
        "bytes_per_shot": (measurement_count + 7) // 8,
    }
    protocol.write_frame(sys.stdout.buffer, protocol.READY, json.dumps(telemetry).encode())

    while True:
        frame_type, payload = protocol.read_frame(sys.stdin.buffer)
        if frame_type == protocol.STOP:
            protocol.write_frame(sys.stdout.buffer, protocol.FINAL, json.dumps(telemetry).encode())
            return 0
        if frame_type != protocol.SAMPLE:
            raise RuntimeError(f"unexpected frame: {frame_type!r}")

        request = json.loads(payload)
        telemetry["sample_call_count"] += 1
        data = sampler.sample(shots=request["shots"], bit_packed=True).tobytes(order="C")
        result = struct.pack("<QQ", request["request_id"], telemetry["sample_call_count"]) + data
        protocol.write_frame(sys.stdout.buffer, protocol.RESULT, result)


if __name__ == "__main__":
    raise SystemExit(main())
