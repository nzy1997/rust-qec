from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


PROTOCOL = "reference-build-v1"
REPO_ROOT = Path(__file__).resolve().parents[2]
COUNTER_KEYS = (
    "measurement_reset_batches",
    "canonical_materializations",
    "canonical_writebacks",
    "direct_inverse_batches",
    "transposed_collapse_batches",
    "collapse_pivots",
    "expanded_repeat_iterations",
    "measurement_bits",
)


class ProfileError(RuntimeError):
    pass


class WorkerSession:
    def __init__(self, command: list[str]) -> None:
        environment = dict(os.environ)
        python_path = environment.get("PYTHONPATH")
        environment["PYTHONPATH"] = (
            str(REPO_ROOT)
            if not python_path
            else f"{REPO_ROOT}{os.pathsep}{python_path}"
        )
        try:
            self.process = subprocess.Popen(
                command,
                cwd=REPO_ROOT,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                env=environment,
            )
        except OSError as error:
            raise ProfileError(f"could not start worker: {error}") from error

        assert self.process.stdin is not None
        assert self.process.stdout is not None
        assert self.process.stderr is not None
        self.stdin = self.process.stdin
        self.stdout = self.process.stdout
        self.stderr = self.process.stderr

    def request(self, payload: dict[str, Any]) -> dict[str, Any]:
        try:
            self.stdin.write(json.dumps(payload, sort_keys=True) + "\n")
            self.stdin.flush()
        except OSError as error:
            raise ProfileError(f"could not send worker request: {error}") from error

        line = self.stdout.readline()
        if not line:
            detail = self.stderr.read().strip()
            suffix = f": {detail}" if detail else ""
            raise ProfileError(f"worker exited before response{suffix}")
        try:
            response = json.loads(line)
        except json.JSONDecodeError as error:
            raise ProfileError(f"worker response is not valid JSON: {line!r}") from error
        if not isinstance(response, dict):
            raise ProfileError("worker response must be a JSON object")
        if response.get("type") == "error":
            raise ProfileError(f"worker error: {response.get('message')}")
        return response

    def close(self) -> None:
        if not self.stdin.closed:
            self.stdin.close()
        try:
            exit_code = self.process.wait(timeout=10)
        except subprocess.TimeoutExpired as error:
            raise ProfileError("worker did not exit after stdin closed") from error
        detail = self.stderr.read().strip()
        if exit_code != 0:
            suffix = f": {detail}" if detail else ""
            raise ProfileError(f"worker exited with status {exit_code}{suffix}")

    def abort(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait()


def _validate_counters(response: dict[str, Any]) -> dict[str, int]:
    counters = response.get("phase_counters")
    if not isinstance(counters, dict):
        raise ProfileError("phase_counters must be a dictionary")

    validated: dict[str, int] = {}
    for key in COUNTER_KEYS:
        if key not in counters:
            raise ProfileError(f"phase_counters missing {key!r}")
        value = counters[key]
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ProfileError(f"phase_counters[{key!r}] must be a nonnegative integer")
        validated[key] = value
    return validated


def _require_response_field(
    response: dict[str, Any], key: str, expected: object, context: str
) -> None:
    actual = response.get(key)
    if actual != expected or type(actual) is not type(expected):
        raise ProfileError(
            f"{context} {key} must be {expected!r}, got {actual!r}"
        )


def _profile(args: argparse.Namespace) -> dict[str, Any]:
    command = [str(args.worker), "--protocol", PROTOCOL]
    session = WorkerSession(command)
    try:
        loaded = session.request(
            {"protocol": PROTOCOL, "type": "load", "fixture_path": str(args.fixture)}
        )
        _require_response_field(loaded, "protocol", PROTOCOL, "load response")
        _require_response_field(loaded, "type", "loaded", "load response")
        response = session.request(
            {
                "protocol": PROTOCOL,
                "type": "build_reference",
                "request_id": 0,
                "include_phase_counters": True,
            }
        )
        _require_response_field(response, "protocol", PROTOCOL, "build response")
        _require_response_field(response, "type", "reference_built", "build response")
        _require_response_field(response, "request_id", 0, "build response")
        counters = _validate_counters(response)
        if response.get("backend") != "packed_inverse":
            raise ProfileError("backend must be 'packed_inverse'")
        if response.get("measurement_bits") != counters["measurement_bits"]:
            raise ProfileError("measurement_bits must match phase_counters['measurement_bits']")
        session.close()
    except Exception:
        session.abort()
        raise

    return {
        "protocol": PROTOCOL,
        "fixture_path": str(args.fixture),
        "worker_argv": command,
        "backend": response["backend"],
        "measurement_bits": response["measurement_bits"],
        "phase_counters": counters,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Profile reference-build phases.")
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--worker", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args(argv)

    try:
        payload = _profile(args)
        args.out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except (OSError, ProfileError) as error:
        print(error, file=sys.stderr)
        return 1

    counters = payload["phase_counters"]
    print(
        "PASS reference phase profile "
        f"batches={counters['measurement_reset_batches']} "
        f"canonical={counters['canonical_materializations']} "
        f"writebacks={counters['canonical_writebacks']} "
        f"repeats={counters['expanded_repeat_iterations']} "
        f"bits={counters['measurement_bits']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
