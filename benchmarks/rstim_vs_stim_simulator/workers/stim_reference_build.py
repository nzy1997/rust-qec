from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


PROTOCOL = "reference-build-v1"
EXPECTED_STIM_VERSION = "1.15.0"
TIMER_SCOPE = "reference_build_only"
BACKEND = "stim_reference"


@dataclass
class WorkerState:
    circuit: Any | None = None
    parse_count: int = 0
    reference_build_count: int = 0
    measurement_bits: int = 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the reference-build Stim worker.")
    parser.add_argument("--protocol", required=True)
    return parser


def _write_json(value: dict[str, object]) -> None:
    print(json.dumps(value, separators=(",", ":")), flush=True)


def _write_error(message: object) -> None:
    _write_json({"protocol": PROTOCOL, "type": "error", "message": str(message)})


def _require_request(request: object, expected_type: str) -> dict[str, object]:
    if not isinstance(request, dict):
        raise ValueError("request must be a JSON object")
    if request.get("protocol") != PROTOCOL:
        raise ValueError(f"request protocol must be {PROTOCOL!r}")
    if request.get("type") != expected_type:
        raise ValueError(f"request type must be {expected_type!r}")
    return request


def _load(request: object, state: WorkerState, stim: Any) -> dict[str, object]:
    load = _require_request(request, "load")
    fixture_path = load.get("fixture_path")
    if not isinstance(fixture_path, str):
        raise ValueError("load fixture_path must be a string")

    input_text = Path(fixture_path).read_text(encoding="utf-8")
    state.circuit = stim.Circuit(input_text)
    state.parse_count += 1
    state.reference_build_count = 0
    state.measurement_bits = state.circuit.num_measurements
    return {
        "protocol": PROTOCOL,
        "type": "loaded",
        "parse_count": state.parse_count,
        "measurement_bits": state.measurement_bits,
    }


def _build_reference(request: object, state: WorkerState, numpy: Any) -> dict[str, object]:
    build = _require_request(request, "build_reference")
    if state.circuit is None:
        raise ValueError("cannot build reference before load")
    request_id = build.get("request_id")
    if not isinstance(request_id, int):
        raise ValueError("build_reference request_id must be an integer")

    started_ns = time.perf_counter_ns()
    bits = state.circuit.reference_sample()
    packed = numpy.packbits(bits, bitorder="little").tobytes()
    elapsed_ns = time.perf_counter_ns() - started_ns

    state.reference_build_count += 1
    return {
        "protocol": PROTOCOL,
        "type": "reference_built",
        "request_id": request_id,
        "backend": BACKEND,
        "parse_count": state.parse_count,
        "reference_build_count": state.reference_build_count,
        "measurement_bits": state.measurement_bits,
        "packed_bytes": len(packed),
        "packed_base64": base64.b64encode(packed).decode("ascii"),
        "byte_sha256": hashlib.sha256(packed).hexdigest(),
        "timer_scope": TIMER_SCOPE,
        "elapsed_ns": elapsed_ns,
    }


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.protocol != PROTOCOL:
        print(f"requires --protocol {PROTOCOL}, got {args.protocol}", file=sys.stderr)
        return 2

    try:
        import numpy
        import stim

        if stim.__version__ != EXPECTED_STIM_VERSION:
            raise RuntimeError(f"requires stim=={EXPECTED_STIM_VERSION}, got {stim.__version__}")
    except Exception as error:
        print(error, file=sys.stderr)
        return 1

    state = WorkerState()
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            request = json.loads(line)
            if isinstance(request, dict) and request.get("type") == "load":
                response = _load(request, state, stim)
            else:
                response = _build_reference(request, state, numpy)
            _write_json(response)
        except Exception as error:
            _write_error(error)
            print(error, file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
