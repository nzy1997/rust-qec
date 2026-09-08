#!/usr/bin/env python3
"""Run rsinter checks that intentionally validate repository benchmark configs."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tomllib


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    required = [
        repo_root / "benchmarks/surface_decoder/spec.toml",
        repo_root / "benchmarks/surface_decoder/full.toml",
        repo_root / "benchmarks/bb_circuit_bposd_compare/plot.toml",
        repo_root
        / "benchmarks/surface_decoder_compare/tests/fixtures/rsinter_plot_semantics.csv",
    ]
    missing = [str(path.relative_to(repo_root)) for path in required if not path.is_file()]
    if missing:
        raise SystemExit("missing repository benchmark configs: " + ", ".join(missing))

    bb_plot_path = repo_root / "benchmarks/bb_circuit_bposd_compare/plot.toml"
    with bb_plot_path.open("rb") as file:
        bb_plot = tomllib.load(file)
    plot = bb_plot.get("plot", {})
    panels = plot.get("panel", [])
    if plot.get("logical_rate_unit") != "per_round":
        raise SystemExit(f"{bb_plot_path.relative_to(repo_root)} must use logical_rate_unit = 'per_round'")
    if not panels or panels[0].get("metric") != "metrics.logical_error_rate":
        raise SystemExit(f"{bb_plot_path.relative_to(repo_root)} first panel must plot logical error rate")
    if panels[0].get("label") != "Logical Error Rate per Syndrome Cycle":
        raise SystemExit(f"{bb_plot_path.relative_to(repo_root)} first panel label has drifted")

    repo_csv = required[3]
    crate_csv = repo_root / "rsinter/tests/fixtures/bench/surface_compare_plot_semantics.csv"
    if repo_csv.read_bytes() != crate_csv.read_bytes():
        raise SystemExit(
            "rsinter surface comparison fixture differs from "
            + str(repo_csv.relative_to(repo_root))
        )

    note = (repo_root / "docs/bb144_circuit_bposd_reproduction.md").read_text()
    for token in (
        "0.003\t12\t5\t0", "--num-trials 50000", "--seed 12345",
        "95% one-sided Clopper-Pearson upper bound", "does not claim statistical agreement",
        "small_ldpc.png", "red [[144,12,12]] LDPC curve", "ldpc_vs_surface.png",
        "red-diamond LDPC [[144,12,12]] curve", "--max-bp-iterations 10000",
        "--osd-order 7", "physical_error_rate must be finite and lie in [0, 1)",
    ):
        if token not in note:
            raise SystemExit(f"BB144 reproduction note is missing evidence token: {token}")

    env = os.environ.copy()
    env["RSINTER_CHECK_REPO_CONFIGS"] = "1"
    return subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "rsinter",
            "--features",
            "rbposd-runner",
            "--test",
            "bench_specs",
        ],
        cwd=repo_root,
        env=env,
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
