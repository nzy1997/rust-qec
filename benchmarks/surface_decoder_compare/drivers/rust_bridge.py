from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

from ..schema import CaseBundle, ResultRow
from .base import BenchmarkDriver


REPO_ROOT = Path(__file__).resolve().parents[3]


def _gurobi_env_available() -> bool:
    return bool(os.environ.get("GUROBI_HOME") and os.environ.get("GRB_LICENSE_FILE"))


def _load_bridge_payload(stdout: str) -> dict[str, object]:
    lines = [line.strip() for line in stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError("rust bridge returned empty stdout")
    for line in reversed(lines):
        if line.startswith("{"):
            return json.loads(line)
    raise RuntimeError(f"rust bridge did not emit a JSON payload: {stdout.strip()}")


class RustBridgeDriver(BenchmarkDriver):
    backend = "native"

    def __init__(self, decoder_name: str):
        self.name = decoder_name

    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        request = {
            "decoder": self.name,
            "dem_path": str(bundle.dem_path),
            "dets_b8_path": str(bundle.dets_b8_path),
            "obs_b8_path": str(bundle.obs_b8_path),
            "num_shots": bundle.num_shots,
            "num_dets": bundle.num_dets,
            "num_obs": bundle.num_obs,
            "max_errors": bundle.tier.max_errors,
            "batch_size": batch_size,
        }
        command = [
            "cargo",
            "run",
            "--quiet",
            "--release",
            "-p",
            "surface_decoder_compare_bridge",
        ]
        if self.name == "rilpqec" and _gurobi_env_available():
            command.extend(["--features", "gurobi"])
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            input=json.dumps(request),
            text=True,
            capture_output=True,
            check=False,
        )
        if completed.returncode != 0:
            raise RuntimeError(
                f"rust bridge failed for {self.name}: {completed.stderr.strip()}"
            )

        payload = _load_bridge_payload(completed.stdout)
        if payload["status"] != "ok":
            raise RuntimeError(
                f"rust bridge reported error for {self.name}: {payload['error']}"
            )

        return ResultRow(
            tier=bundle.tier.name,
            decoder=payload["decoder"],
            backend=payload["backend"],
            distance=bundle.spec.distance,
            rounds=bundle.spec.rounds,
            p=bundle.spec.p,
            seed=bundle.seed,
            num_dets=bundle.num_dets,
            num_obs=bundle.num_obs,
            shots_budget=bundle.tier.max_shots,
            errors_budget=bundle.tier.max_errors,
            shots_used=payload["shots_used"],
            logical_errors=payload["logical_errors"],
            logical_error_rate=(
                payload["logical_errors"] / payload["shots_used"]
                if payload["shots_used"]
                else 0.0
            ),
            compile_us=payload["compile_us"],
            total_decode_us=payload["total_decode_us"],
            decode_us_per_shot=(
                payload["total_decode_us"] / payload["shots_used"]
                if payload["shots_used"]
                else 0.0
            ),
            status=payload["status"],
            error=payload["error"],
        )
