from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
import time
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator import run_compiled_steady as protocol


def _fixture_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_error(message: object) -> None:
    protocol.write_frame(sys.stdout.buffer, protocol.ERROR, str(message).encode())


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the compiled steady-state Stim worker.")
    parser.add_argument("--variant", choices=("stim-precompiled", "stim-direct"), required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--seed", type=int, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        args = parser.parse_args(argv)
    except SystemExit as error:
        if error.code == 0:
            return 0
        _write_error(parser.format_usage().strip())
        return int(error.code) if isinstance(error.code, int) else 2

    try:
        import stim

        if stim.__version__ != "1.15.0":
            raise RuntimeError(f"requires stim==1.15.0, got {stim.__version__}")

        input_text = args.input.read_text(encoding="utf-8")
        circuit = stim.Circuit(input_text)
        sampler = circuit.compile_sampler(seed=args.seed) if args.variant == "stim-precompiled" else None
        measurement_count = circuit.num_measurements
        telemetry = {
            "variant": args.variant,
            "compile_count": 1 if sampler is not None else 0,
            "reference_build_count": 1 if sampler is not None else 0,
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
            started_ns = time.perf_counter_ns()
            if sampler is None:
                call_sampler = circuit.compile_sampler(seed=args.seed + telemetry["sample_call_count"])
                data = call_sampler.sample(shots=shots, bit_packed=True).tobytes(order="C")
            else:
                data = sampler.sample(shots=shots, bit_packed=True).tobytes(order="C")
            sample_b8_elapsed_ns = time.perf_counter_ns() - started_ns
        except Exception as error:
            _write_error(error)
            continue
        telemetry["sample_call_count"] += 1
        if sampler is None:
            telemetry["compile_count"] += 1
            telemetry["reference_build_count"] += 1
        result = struct.pack(
            "<QQQ", request_id, telemetry["sample_call_count"], sample_b8_elapsed_ns
        ) + data
        protocol.write_frame(sys.stdout.buffer, protocol.RESULT, result)


if __name__ == "__main__":
    raise SystemExit(main())
