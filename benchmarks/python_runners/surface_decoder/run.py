from __future__ import annotations

import argparse
import json
from pathlib import Path

from benchmarks.python_runners.surface_decoder.result_io import write_results_jsonl
from benchmarks.python_runners.surface_decoder.spec_runtime import (
    load_spec,
    python_runners_from_spec,
)
from benchmarks.surface_decoder_compare.cases import materialize_case_bundle
from benchmarks.surface_decoder_compare.drivers import build_driver_registry
from benchmarks.surface_decoder_compare.schema import CaseSpec, TierConfig


def _expand_points(params: dict[str, object]) -> list[CaseSpec]:
    distances = params["distance"]
    rounds = params["rounds"]
    p_values = params["p"]
    return [
        CaseSpec(distance=int(distance), rounds=int(rounds_), p=float(p))
        for distance in distances
        for rounds_ in rounds
        for p in p_values
    ]


def _tier_from_params(params: dict[str, object]) -> TierConfig:
    return TierConfig(
        name="generated",
        max_shots=int(params["max_shots"]),
        max_errors=int(params["max_errors"]),
    )


def _result_to_row(
    benchmark_name: str,
    runner_name: str,
    batch_size: int,
    result,
) -> dict[str, object]:
    error = result.error or None
    return {
        "benchmark": benchmark_name,
        "runner": runner_name,
        "language": "python",
        "status": result.status,
        "params": {
            "distance": result.distance,
            "rounds": result.rounds,
            "p": result.p,
            "max_shots": result.shots_budget,
            "max_errors": result.errors_budget,
            "batch_size": batch_size,
        },
        "case_summary": {
            "num_dets": result.num_dets,
            "num_obs": result.num_obs,
            "num_shots_generated": result.shots_budget,
        },
        "metrics": {
            "shots_used": result.shots_used,
            "logical_errors": result.logical_errors,
            "logical_error_rate": result.logical_error_rate,
            "compile_us": result.compile_us,
            "total_decode_us": result.total_decode_us,
            "decode_us_per_shot": result.decode_us_per_shot,
        },
        "artifacts": {},
        "error": error,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--language", choices=("python",), required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)

    spec = load_spec(args.spec)
    registry = build_driver_registry()
    args.out.mkdir(parents=True, exist_ok=True)

    for runner in python_runners_from_spec(spec):
        artifact_dir = args.out / runner.name / "test-run"
        if artifact_dir.exists():
            for child in artifact_dir.iterdir():
                if child.is_dir():
                    raise RuntimeError(f"unexpected directory inside artifact dir: {child}")
                child.unlink()
        artifact_dir.mkdir(parents=True, exist_ok=True)

        manifest = {
            "benchmark": spec["name"],
            "benchmark_version": spec["version"],
            "runner": runner.name,
            "language": "python",
            "output_dir": str(artifact_dir),
        }
        (artifact_dir / "run_manifest.json").write_text(json.dumps(manifest, indent=2))

        driver = registry[runner.impl_key]
        rows = []
        tier = _tier_from_params(runner.params)
        batch_size = int(runner.params["batch_size"])
        for case_spec in _expand_points(runner.params):
            bundle = materialize_case_bundle(args.out / "_cases", case_spec, tier, 12345)
            result = driver.run_case(bundle, batch_size=batch_size)
            rows.append(_result_to_row(spec["name"], runner.name, batch_size, result))

        with (artifact_dir / "results.jsonl").open("w") as handle:
            write_results_jsonl(rows, handle)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
