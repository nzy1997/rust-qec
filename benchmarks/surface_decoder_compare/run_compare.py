from __future__ import annotations

import argparse
import csv
from pathlib import Path

from .cases import build_case_specs, materialize_case_bundle
from .drivers import build_driver_registry
from .schema import CSV_HEADER, DEFAULT_BATCH_SIZE, ResultRow, TIER_CONFIGS


def write_results(rows: list[ResultRow], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_HEADER)
        writer.writeheader()
        for row in rows:
            writer.writerow(row.to_csv_row())


def run_suite(
    *,
    tier_name: str,
    output_dir: Path,
    seed: int,
    drivers=None,
    case_specs=None,
    case_bundle_factory=materialize_case_bundle,
    batch_size: int = DEFAULT_BATCH_SIZE,
) -> list[ResultRow]:
    tier = TIER_CONFIGS[tier_name]
    specs = list(case_specs) if case_specs is not None else build_case_specs()
    registry = drivers or build_driver_registry()

    rows: list[ResultRow] = []
    for spec in specs:
        bundle = case_bundle_factory(output_dir, spec, tier, seed)
        for driver in registry.values():
            print(
                f"[{tier.name}] case={spec.slug} decoder={driver.name}",
                flush=True,
            )
            try:
                rows.append(driver.run_case(bundle, batch_size=batch_size))
            except Exception as error:
                rows.append(
                    ResultRow(
                        tier=bundle.tier.name,
                        decoder=driver.name,
                        backend=getattr(driver, "backend", ""),
                        distance=bundle.spec.distance,
                        rounds=bundle.spec.rounds,
                        p=bundle.spec.p,
                        seed=bundle.seed,
                        num_dets=bundle.num_dets,
                        num_obs=bundle.num_obs,
                        shots_budget=bundle.tier.max_shots,
                        errors_budget=bundle.tier.max_errors,
                        shots_used=0,
                        logical_errors=0,
                        logical_error_rate=0.0,
                        compile_us=0.0,
                        total_decode_us=0.0,
                        decode_us_per_shot=0.0,
                        status="error",
                        error=str(error),
                    )
                )

    write_results(rows, output_dir / tier.name / "results.csv")
    return rows


def _parse_csv_ints(value: str | None) -> set[int] | None:
    if not value:
        return None
    return {int(item.strip()) for item in value.split(",") if item.strip()}


def _parse_csv_floats(value: str | None) -> set[float] | None:
    if not value:
        return None
    return {float(item.strip()) for item in value.split(",") if item.strip()}


def _filter_case_specs(specs, distances: set[int] | None, p_values: set[float] | None):
    filtered = []
    for spec in specs:
        if distances is not None and spec.distance not in distances:
            continue
        if p_values is not None and spec.p not in p_values:
            continue
        filtered.append(spec)
    return filtered


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", choices=sorted(TIER_CONFIGS), required=True)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("benchmarks/surface_decoder_compare/results"),
    )
    parser.add_argument("--seed", type=int, default=12345)
    parser.add_argument("--batch-size", type=int, default=DEFAULT_BATCH_SIZE)
    parser.add_argument(
        "--distances",
        help="Optional comma-separated distance filter, e.g. 3,5",
    )
    parser.add_argument(
        "--p-values",
        help="Optional comma-separated p filter, e.g. 0.001,0.005",
    )
    parser.add_argument(
        "--decoders",
        help="Optional comma-separated decoder filter",
    )
    args = parser.parse_args(argv)

    registry = build_driver_registry()
    if args.decoders:
        wanted = {name.strip() for name in args.decoders.split(",") if name.strip()}
        registry = {name: driver for name, driver in registry.items() if name in wanted}

    case_specs = _filter_case_specs(
        build_case_specs(),
        _parse_csv_ints(args.distances),
        _parse_csv_floats(args.p_values),
    )

    run_suite(
        tier_name=args.tier,
        output_dir=args.output_dir,
        seed=args.seed,
        batch_size=args.batch_size,
        drivers=registry,
        case_specs=case_specs,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
