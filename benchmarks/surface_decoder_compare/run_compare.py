from __future__ import annotations

import argparse
import csv
from pathlib import Path

from .cases import build_case_specs, materialize_case_bundle
from .drivers import build_driver_registry
from .schema import CSV_HEADER, DEFAULT_BATCH_SIZE, ResultRow, TIER_CONFIGS


def write_results(rows: list[dict[str, object]] | list[ResultRow], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    normalized_rows: list[dict[str, object]] = []
    for row in rows:
        if isinstance(row, ResultRow):
            normalized_rows.append(row.to_csv_row())
        else:
            normalized_rows.append(dict(row))
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_HEADER)
        writer.writeheader()
        for row in normalized_rows:
            writer.writerow(row)


def _load_existing_rows(path: Path) -> list[dict[str, str]]:
    if not path.exists():
        return []
    with path.open() as handle:
        return list(csv.DictReader(handle))


def _row_identity(row: dict[str, object]) -> tuple[str, str, str, str, str, str]:
    return (
        str(row["tier"]),
        str(row["decoder"]),
        str(row["distance"]),
        str(row["rounds"]),
        str(row["p"]),
        str(row["seed"]),
    )


def _sort_key(row: dict[str, object]) -> tuple[str, float, float, int]:
    return (
        str(row["decoder"]),
        float(row["distance"]),
        float(row["p"]),
        int(row["seed"]),
    )


def _merge_rows(
    existing_rows: list[dict[str, str]],
    new_rows: list[ResultRow],
) -> list[dict[str, object]]:
    normalized_new_rows = [row.to_csv_row() for row in new_rows]
    replacement_keys = {_row_identity(row) for row in normalized_new_rows}
    kept_rows = [
        row for row in existing_rows if _row_identity(row) not in replacement_keys
    ]
    merged = kept_rows + normalized_new_rows
    return sorted(merged, key=_sort_key)


def run_suite(
    *,
    tier_name: str,
    output_dir: Path,
    seed: int,
    drivers=None,
    case_specs=None,
    case_bundle_factory=materialize_case_bundle,
    batch_size: int = DEFAULT_BATCH_SIZE,
    write_output: bool = True,
) -> list[ResultRow]:
    tier = TIER_CONFIGS[tier_name]
    specs = (
        list(case_specs)
        if case_specs is not None
        else build_case_specs(tier_name=tier_name)
    )
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

    if write_output:
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
    parser.add_argument(
        "--merge-into-existing",
        action="store_true",
        help="Replace matching rows inside the canonical tier results.csv",
    )
    args = parser.parse_args(argv)

    registry = build_driver_registry()
    if args.decoders:
        wanted = {name.strip() for name in args.decoders.split(",") if name.strip()}
        registry = {name: driver for name, driver in registry.items() if name in wanted}

    if args.merge_into_existing and not args.decoders:
        parser.error("--merge-into-existing requires --decoders")

    case_specs = _filter_case_specs(
        build_case_specs(tier_name=args.tier),
        _parse_csv_ints(args.distances),
        _parse_csv_floats(args.p_values),
    )

    rows = run_suite(
        tier_name=args.tier,
        output_dir=args.output_dir,
        seed=args.seed,
        batch_size=args.batch_size,
        drivers=registry,
        case_specs=case_specs,
        write_output=not args.merge_into_existing,
    )

    if args.merge_into_existing:
        results_path = args.output_dir / args.tier / "results.csv"
        merged_rows = _merge_rows(_load_existing_rows(results_path), rows)
        write_results(merged_rows, results_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
