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


def _write_error(message: object) -> None:
    protocol.write_frame(sys.stdout.buffer, protocol.ERROR, str(message).encode())


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the compiled steady-state Stim worker.")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--seed", type=int, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    try:
        import stim

        if stim.__version__ != "1.15.0":
            raise RuntimeError(f"requires stim==1.15.0, got {stim.__version__}")

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
    except Exception as error:
        _write_error(error)
        print(error, file=sys.stderr)
        return 1

    while True:
        try:
            frame_type, payload = protocol.read_frame(sys.stdin.buffer)
        except Exception as error:
            _write_error(error)
            return 1
        if frame_type == protocol.STOP:
            protocol.write_frame(sys.stdout.buffer, protocol.FINAL, json.dumps(telemetry).encode())
            return 0
        if frame_type != protocol.SAMPLE:
            _write_error(f"unexpected frame: {frame_type!r}")
            continue

        try:
            request = json.loads(payload)
            if not isinstance(request, dict):
                raise ValueError("expected object")
            request_id = request["request_id"]
            shots = request["shots"]
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
            _write_error(f"invalid SAMPLE JSON: {error}")
            continue
        except KeyError as error:
            _write_error(f"invalid SAMPLE JSON: missing {error}")
            continue
        try:
            data = sampler.sample(shots=shots, bit_packed=True).tobytes(order="C")
        except Exception as error:
            _write_error(error)
            continue
        telemetry["sample_call_count"] += 1
        result = struct.pack("<QQ", request_id, telemetry["sample_call_count"]) + data
        protocol.write_frame(sys.stdout.buffer, protocol.RESULT, result)


if __name__ == "__main__":
    raise SystemExit(main())
