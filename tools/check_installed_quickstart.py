#!/usr/bin/env python3
"""Exercise the installed rustqec and rstim quickstart without Cargo or PATH."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path


CIRCUIT = "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
BAD_REPEAT = "REPEAT two {\n  M 0\n}\n"
EXPECTED_STATS = {"instruction_count": 5, "num_qubits": 1, "num_measurements": 1, "num_detectors": 1, "num_observables": 1}
EVENT = "shot D0 L0"


class QuickstartError(Exception):
    pass


def run(binary: Path, *args: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run([str(binary), *args], cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)


def validate(bin_dir: Path) -> None:
    rustqec, rstim = bin_dir / "rustqec", bin_dir / "rstim"
    for binary in (rustqec, rstim):
        if not binary.is_file() or not binary.stat().st_mode & 0o111:
            raise QuickstartError(f"missing executable: {binary}")
    with tempfile.TemporaryDirectory(prefix="installed-quickstart-") as temporary:
        work = Path(temporary)
        circuit, dem, bad = work / "pipeline.stim", work / "pipeline.dem", work / "bad-repeat.stim"
        circuit.write_text(CIRCUIT, encoding="utf-8")
        bad.write_text(BAD_REPEAT, encoding="utf-8")
        capabilities = run(rustqec, "capabilities", "--format", "json", cwd=work)
        if capabilities.returncode:
            raise QuickstartError(f"capabilities failed: {capabilities.stderr.strip()}")
        try:
            commands = json.loads(capabilities.stdout)["commands"]
        except (json.JSONDecodeError, KeyError, TypeError) as error:
            raise QuickstartError("capabilities did not return its JSON contract") from error
        stats_capability = next((command for command in commands if command.get("name") == "circuit.stats"), None)
        if stats_capability is None or stats_capability.get("argv") != ["circuit", "stats"] or "json" not in stats_capability.get("formats", []):
            raise QuickstartError("capabilities does not advertise circuit.stats")
        stats = run(rustqec, "circuit", "stats", "--format", "json", "--in", str(circuit), cwd=work)
        try:
            observed_stats = json.loads(stats.stdout)
        except json.JSONDecodeError:
            observed_stats = {}
        observed_result = observed_stats.get("result", {})
        if stats.returncode or {key: observed_result.get(key) for key in EXPECTED_STATS} != EXPECTED_STATS:
            raise QuickstartError(f"stats did not match the showcase: {stats.stderr.strip() or stats.stdout.strip()}")
        detect = run(rstim, "detect", "--shots", "1", "--out_format", "dets", "--in", str(circuit), cwd=work)
        if detect.returncode or detect.stdout.strip() != EVENT:
            raise QuickstartError("detect did not produce 'shot D0 L0'")
        analyze = run(rstim, "analyze_errors", "--in", str(circuit), "--out", str(dem), cwd=work)
        if analyze.returncode or dem.read_text(encoding="utf-8").strip() != "error(1) D0 L0":
            raise QuickstartError("analyze_errors did not produce 'error(1) D0 L0'")
        sample = run(rstim, "sample_dem", "--shots", "1", "--out_format", "dets", "--in", str(dem), cwd=work)
        if sample.returncode or sample.stdout.strip() != EVENT:
            raise QuickstartError("sample_dem did not produce 'shot D0 L0'")
        rejected = run(rstim, "stats", "--in", str(bad), cwd=work)
        if rejected.returncode == 0 or "bad repeat count" not in rejected.stderr:
            raise QuickstartError("bad repeat count was not rejected")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        validate(args.bin_dir.resolve())
    except (QuickstartError, OSError) as error:
        print(f"FAIL installed quickstart: {error}", file=sys.stderr)
        return 1
    print("PASS installed quickstart")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
