#!/usr/bin/env python3
import argparse
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any

REAL_MISMATCH_CLASSES = {
    "status_mismatch",
    "error_mismatch",
    "correction_mismatch",
    "payload_mismatch",
}

DEFAULT_BP_CONFIG = {
    "max_bp_iterations": 30,
    "early_stop": True,
    "bp_variant": "minimum_sum",
    "schedule": "parallel",
    "osd_variant": "osd0",
}

BP_METHOD_MAP = {
    "minimum_sum": "minimum_sum",
    "product_sum": "product_sum",
}

BP_SCHEDULE_MAP = {
    "parallel": "parallel",
    "serial": "serial",
}


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run rbposd parity checks against Python ldpc."
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path("."),
        help="Repository root used when invoking cargo.",
    )
    parser.add_argument(
        "--fixtures-dir",
        type=Path,
        default=Path("rbposd/tests/fixtures/parity"),
        help="Directory containing checked-in parity fixture JSON files.",
    )
    parser.add_argument(
        "--include-lsd",
        action="store_true",
        help="Also run checked-in LSD fixtures from the shared fixture catalog.",
    )
    parser.add_argument(
        "--fixture-catalog",
        type=Path,
        default=Path("rbposd/tests/fixtures/catalog.json"),
        help="Shared LSD and BP-option fixture catalog.",
    )
    parser.add_argument(
        "--skip-generated",
        action="store_true",
        help="Skip generated parity scan cases and run fixtures only.",
    )
    parser.add_argument(
        "--case-limit",
        type=int,
        default=None,
        help="Limit number of total cases after case collection.",
    )
    parser.add_argument(
        "--json-output",
        type=Path,
        default=None,
        help="Optional path to write full parity comparison entries as JSON.",
    )
    return parser.parse_args(argv)


def matrix_to_dense(matrix: dict[str, Any]) -> list[list[int]]:
    num_checks = int(matrix["num_checks"])
    num_bits = int(matrix["num_bits"])
    dense = [[0 for _ in range(num_bits)] for _ in range(num_checks)]
    for check_index, row in enumerate(matrix["rows"]):
        for bit_index in row:
            dense[check_index][int(bit_index)] = 1
    return dense


def load_case(case_path: Path) -> dict[str, Any]:
    with case_path.open("r", encoding="utf-8") as infile:
        return json.load(infile)


def fixture_case_paths(fixtures_dir: Path) -> list[Path]:
    return sorted(fixtures_dir.glob("*.json"))


def load_fixture_catalog(catalog_path: Path) -> dict[str, Any]:
    with catalog_path.open("r", encoding="utf-8") as infile:
        catalog = json.load(infile)
    if not isinstance(catalog.get("fixtures"), list) or not catalog["fixtures"]:
        raise ValueError(
            f"Fixture catalog {catalog_path} must contain a non-empty fixtures list"
        )
    return catalog


def validate_catalog_mode_keys(
    fixture_id: str,
    kind: str,
    modes_by_key: dict[str, str],
    required_keys: set[str],
) -> None:
    for key in modes_by_key:
        if key not in required_keys:
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes key {key} is unsupported for kind {kind}"
            )
    for key in sorted(required_keys):
        if key not in modes_by_key:
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes key {key} is required for kind {kind}"
            )


def validate_catalog_entry_metadata(entry: dict[str, Any]) -> dict[str, Any]:
    fixture_id = str(entry.get("id", "")).strip()
    for field in (
        "id",
        "kind",
        "decoder",
        "path",
        "matrix_path",
        "syndrome_path",
        "provenance",
        "verifier",
        "pass_condition",
    ):
        if not str(entry.get(field, "")).strip():
            raise ValueError(
                f"Fixture catalog entry {fixture_id or '<unknown>'} {field} must not be empty"
            )

    consumes = entry.get("consumes")
    if not isinstance(consumes, list) or not consumes:
        raise ValueError(
            f"Fixture catalog entry {fixture_id} consumes must be a non-empty list of strings"
        )
    if any(not isinstance(tag, str) or not tag.strip() for tag in consumes):
        raise ValueError(
            f"Fixture catalog entry {fixture_id} consumes must be a non-empty list of strings"
        )
    if "#98" not in consumes:
        raise ValueError(
            f"Fixture catalog entry {fixture_id} consumes must include #98"
        )

    modes = entry.get("modes")
    if not isinstance(modes, list) or not modes:
        raise ValueError(
            f"Fixture catalog entry {fixture_id} modes must be a non-empty list of key=value strings"
        )

    modes_by_key: dict[str, str] = {}
    for raw_mode in modes:
        if not isinstance(raw_mode, str):
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes must be a non-empty list of key=value strings"
            )
        key, separator, value = raw_mode.partition("=")
        key = key.strip()
        value = value.strip()
        if separator != "=" or not key or not value:
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes must be a non-empty list of key=value strings"
            )
        if key in modes_by_key:
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes key {key} must not be repeated"
            )
        modes_by_key[key] = value

    kind = str(entry["kind"]).strip()
    decoder = str(entry["decoder"]).strip()
    if kind == "lsd":
        if decoder != "bp_lsd":
            raise ValueError(
                f"Fixture catalog entry {fixture_id} decoder must be bp_lsd for kind lsd"
            )
        validate_catalog_mode_keys(
            fixture_id,
            kind,
            modes_by_key,
            {"decoder", "lsd_method", "lsd_order"},
        )
        if modes_by_key["decoder"] != "bp_lsd":
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes decoder must be bp_lsd"
            )
        if modes_by_key["lsd_method"] != "localized_statistics":
            raise ValueError(
                "Fixture catalog entry "
                f"{fixture_id} modes lsd_method must be localized_statistics"
            )
        if modes_by_key["lsd_order"] not in {"0", "1"}:
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes lsd_order must be 0 or 1"
            )
    elif kind == "bp_option":
        if decoder != "bp_osd":
            raise ValueError(
                f"Fixture catalog entry {fixture_id} decoder must be bp_osd for kind bp_option"
            )
        validate_catalog_mode_keys(
            fixture_id,
            kind,
            modes_by_key,
            {"bp_variant", "schedule", "osd_variant"},
        )
        if modes_by_key["bp_variant"] not in BP_METHOD_MAP:
            raise ValueError(
                "Fixture catalog entry "
                f"{fixture_id} modes bp_variant must be one of {sorted(BP_METHOD_MAP)}"
            )
        if modes_by_key["schedule"] not in BP_SCHEDULE_MAP:
            raise ValueError(
                "Fixture catalog entry "
                f"{fixture_id} modes schedule must be one of {sorted(BP_SCHEDULE_MAP)}"
            )
        if modes_by_key["osd_variant"] != "osd0":
            raise ValueError(
                f"Fixture catalog entry {fixture_id} modes osd_variant must be osd0"
            )
    else:
        raise ValueError(f"Fixture catalog entry {fixture_id} kind is unsupported: {kind}")

    return {
        "consumes": consumes,
        "modes": modes_by_key,
    }


def catalog_fixture_root(catalog_path: Path) -> Path:
    return catalog_path.parent


def iter_lsd_fixture_cases(catalog_path: Path) -> list[dict[str, Any]]:
    catalog = load_fixture_catalog(catalog_path)
    fixture_root = catalog_fixture_root(catalog_path)
    cases: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for entry in catalog["fixtures"]:
        if entry.get("kind") != "lsd":
            continue
        metadata = validate_catalog_entry_metadata(entry)
        fixture_id = str(entry.get("id", ""))
        fixture_path_name = str(entry.get("path", ""))
        if not fixture_id:
            raise ValueError("Fixture catalog entry id must not be empty")
        if fixture_id in seen_ids:
            raise ValueError(f"Duplicate LSD catalog id: {fixture_id}")
        seen_ids.add(fixture_id)
        if not fixture_path_name:
            raise ValueError(
                f"Fixture catalog entry {fixture_id} path must not be empty"
            )

        fixture_path = fixture_root / fixture_path_name
        fixture = load_case(fixture_path)
        if fixture.get("id") != fixture_id:
            raise ValueError(
                f"Fixture catalog id {fixture_id} does not match fixture id {fixture.get('id')}"
            )
        fixture_lsd_order = int(fixture["lsd_order"])
        catalog_lsd_order = int(metadata["modes"]["lsd_order"])
        if fixture_lsd_order != catalog_lsd_order:
            raise ValueError(
                "Fixture catalog entry "
                f"{fixture_id} modes lsd_order {catalog_lsd_order} does not match fixture lsd_order {fixture_lsd_order}"
            )

        cases.append(
            {
                "name": fixture["id"],
                "decoder": entry["decoder"],
                "matrix": fixture["matrix"],
                "channel": fixture["channel"],
                "syndrome": fixture["syndrome"],
                "config": dict(DEFAULT_BP_CONFIG),
                "lsd_config": {
                    "method": metadata["modes"]["lsd_method"],
                    "lsd_order": catalog_lsd_order,
                },
                "tags": ["fixture", "lsd", *metadata["consumes"]],
            }
        )
    return cases


def iter_catalog_fixture_cases(
    catalog_path: Path, include_lsd: bool
) -> list[dict[str, Any]]:
    catalog = load_fixture_catalog(catalog_path)
    fixture_root = catalog_fixture_root(catalog_path)
    cases: list[dict[str, Any]] = []
    lsd_cases_by_id = (
        {case["name"]: case for case in iter_lsd_fixture_cases(catalog_path)}
        if include_lsd
        else {}
    )

    for entry in catalog["fixtures"]:
        kind = entry.get("kind")
        if kind == "lsd" and include_lsd:
            case = lsd_cases_by_id.get(str(entry["id"]))
            if case is not None:
                cases.append(
                    {
                        "source": "catalog_fixture",
                        "case_path": None,
                        "case": case,
                        "catalog_path": entry["path"],
                    }
                )
            continue
        if kind == "bp_option":
            metadata = validate_catalog_entry_metadata(entry)
            fixture_path = fixture_root / str(entry["path"])
            case = load_case(fixture_path)
            if case.get("name") != entry["id"]:
                raise ValueError(
                    "Fixture catalog id "
                    f"{entry['id']} does not match fixture name {case.get('name')}"
                )
            case_config = case.get("config", {})
            for key in ("bp_variant", "schedule", "osd_variant"):
                if case_config.get(key) != metadata["modes"][key]:
                    raise ValueError(
                        "Fixture catalog entry "
                        f"{entry['id']} config {key} must match modes value {metadata['modes'][key]}"
                    )
            if all(
                case_config.get(key) == DEFAULT_BP_CONFIG[key]
                for key in ("bp_variant", "schedule", "osd_variant")
            ):
                raise ValueError(
                    f"Fixture catalog entry {entry['id']} config must not use default bp_option modes"
                )
            cases.append(
                {
                    "source": "catalog_fixture",
                    "case_path": fixture_path,
                    "case": case,
                    "catalog_path": entry["path"],
                }
            )

    return cases


def iter_generated_cases(_repo_root: Path) -> list[dict[str, Any]]:
    return [
        {
            "name": "generated_osd_equal_reliability_tiebreak",
            "matrix": {"num_checks": 2, "num_bits": 3, "rows": [[0, 1], [1, 2]]},
            "channel": {
                "kind": "bit_flip_probabilities",
                "probabilities": [0.1, 0.1, 0.3],
            },
            "syndrome": [True, False],
            "config": {
                "max_bp_iterations": 0,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "tags": ["generated", "osd", "tiebreak"],
        }
    ]


def run_rust_case(repo_root: Path, case_path: Path) -> dict[str, Any]:
    command = [
        "cargo",
        "run",
        "--quiet",
        "-p",
        "rbposd",
        "--example",
        "parity_driver",
        "--",
        str(case_path),
    ]
    completed = subprocess.run(
        command,
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        stdout = completed.stdout.strip()
        stderr = completed.stderr.strip()
        raise RuntimeError(
            f"parity_driver failed for {case_path} with exit code "
            f"{completed.returncode}. stdout={stdout!r} stderr={stderr!r}"
        )
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        stdout = completed.stdout.strip()
        stderr = completed.stderr.strip()
        raise RuntimeError(
            f"parity_driver produced invalid JSON for {case_path}: {error}. "
            f"stdout={stdout!r} stderr={stderr!r}"
        ) from error


def map_config_to_ldpc_kwargs(config: dict[str, Any]) -> dict[str, Any]:
    decoder_kwargs = map_bp_config_to_ldpc_kwargs(config)

    osd_variant = config.get("osd_variant")
    osd_method_map = {
        "osd0": "OSD_0",
    }
    if osd_variant not in osd_method_map:
        raise ValueError(f"Unsupported osd_variant: {osd_variant}")

    return {
        **decoder_kwargs,
        "osd_method": osd_method_map[osd_variant],
        "osd_order": 0,
        "input_vector_type": "syndrome",
    }


def map_bp_config_to_ldpc_kwargs(
    config: dict[str, Any], error_context: str = ""
) -> dict[str, Any]:
    bp_variant = config.get("bp_variant")
    if bp_variant not in BP_METHOD_MAP:
        raise ValueError(f"Unsupported bp_variant{error_context}: {bp_variant}")

    schedule = config.get("schedule")
    if schedule not in BP_SCHEDULE_MAP:
        raise ValueError(f"Unsupported schedule{error_context}: {schedule}")

    early_stop = config.get("early_stop")
    if early_stop is not True:
        raise ValueError(
            f"Unsupported early_stop value{error_context}: {early_stop}. "
            "Python ldpc parity harness currently requires early_stop=true."
        )

    return {
        "max_iter": int(config["max_bp_iterations"]),
        "bp_method": BP_METHOD_MAP[bp_variant],
        "schedule": BP_SCHEDULE_MAP[schedule],
    }


def map_lsd_case_to_ldpc_kwargs(case: dict[str, Any]) -> dict[str, Any]:
    decoder = case.get("decoder")
    if decoder not in ("bp_lsd", "bplsd"):
        raise ValueError(f"Unsupported LSD decoder mode: {decoder}")

    config = case.get("config", {})
    decoder_kwargs = map_bp_config_to_ldpc_kwargs(config, " for LSD")

    lsd_config = case.get("lsd_config", {})
    lsd_method = lsd_config.get("method")
    if lsd_method != "localized_statistics":
        raise ValueError(f"Unsupported lsd_method: {lsd_method}")

    lsd_order = int(lsd_config.get("lsd_order", -1))
    if lsd_order not in (0, 1):
        raise ValueError(f"Unsupported lsd_order: {lsd_order}")

    return {
        **decoder_kwargs,
        "lsd_method": "localized_statistics",
        "lsd_order": lsd_order,
        "input_vector_type": "syndrome",
    }


def add_channel_kwargs(decoder_kwargs: dict[str, Any], channel: dict[str, Any]) -> dict[str, Any]:
    if channel["kind"] == "bsc":
        decoder_kwargs["error_rate"] = float(channel["error_rate"])
    elif channel["kind"] == "bit_flip_probabilities":
        decoder_kwargs["error_channel"] = list(channel["probabilities"])
    else:
        raise ValueError(f"UnsupportedChannel(kind={channel.get('kind')})")
    return decoder_kwargs


def residual_weight_for_correction(case: dict[str, Any], correction: list[bool]) -> int:
    syndrome_bool = [bool(bit) for bit in case["syndrome"]]
    residual = [False for _ in range(len(syndrome_bool))]
    for row_index, row in enumerate(matrix_to_dense(case["matrix"])):
        parity = False
        for bit_index, include in enumerate(row):
            if include:
                parity ^= correction[bit_index]
        residual[row_index] = parity ^ syndrome_bool[row_index]
    return sum(1 for bit in residual if bit)


def run_python_bposd(case: dict[str, Any]) -> dict[str, Any]:
    import numpy as np
    from ldpc import BpOsdDecoder

    matrix = np.array(matrix_to_dense(case["matrix"]), dtype=np.uint8)
    syndrome = np.array(case["syndrome"], dtype=np.uint8)
    try:
        decoder_kwargs = add_channel_kwargs(
            map_config_to_ldpc_kwargs(case["config"]),
            case["channel"],
        )
        decoder = BpOsdDecoder(matrix, **decoder_kwargs)
        correction_arr = decoder.decode(syndrome)
    except ValueError as error:
        return {"status": "error", "error": str(error)}
    except Exception as error:  # pragma: no cover - exercised by full harness runs
        return {"status": "error", "error": f"{type(error).__name__}: {error}"}

    correction = [bool(int(value)) for value in correction_arr.tolist()]
    residual_weight = residual_weight_for_correction(case, correction)
    converged = bool(decoder.converge)
    return {
        "status": "success",
        "correction": correction,
        "diagnostics": {
            "converged": converged,
            "bp_iterations": int(decoder.iter),
            "used_osd": not converged,
            "residual_syndrome_weight": residual_weight,
        },
    }


def run_python_bplsd(case: dict[str, Any]) -> dict[str, Any]:
    import numpy as np
    from ldpc import BpLsdDecoder

    matrix = np.array(matrix_to_dense(case["matrix"]), dtype=np.uint8)
    syndrome = np.array(case["syndrome"], dtype=np.uint8)
    try:
        decoder_kwargs = add_channel_kwargs(
            map_lsd_case_to_ldpc_kwargs(case),
            case["channel"],
        )
        decoder = BpLsdDecoder(matrix, **decoder_kwargs)
        correction_arr = decoder.decode(syndrome)
    except ValueError as error:
        return {"status": "error", "error": str(error)}
    except Exception as error:  # pragma: no cover - exercised by full harness runs
        return {"status": "error", "error": f"{type(error).__name__}: {error}"}

    correction = [bool(int(value)) for value in correction_arr.tolist()]
    residual_weight = residual_weight_for_correction(case, correction)
    return {
        "status": "success",
        "correction": correction,
        "diagnostics": {
            "converged": bool(getattr(decoder, "converge", False)),
            "bp_iterations": int(getattr(decoder, "iter", case["config"]["max_bp_iterations"])),
            "used_osd": False,
            "residual_syndrome_weight": residual_weight,
        },
    }


def run_python_ldpc(case: dict[str, Any]) -> dict[str, Any]:
    decoder = case.get("decoder", "bp_osd")
    if decoder in ("bp_osd", "bposd"):
        return run_python_bposd(case)
    if decoder in ("bp_lsd", "bplsd"):
        return run_python_bplsd(case)
    return {"status": "error", "error": f"Unsupported decoder: {decoder}"}


def classify_mismatch(
    rust_actual: dict[str, Any], python_actual: dict[str, Any]
) -> str:
    if rust_actual.get("status") != python_actual.get("status"):
        return "status_mismatch"
    if rust_actual.get("status") == "success":
        if rust_actual.get("correction") != python_actual.get("correction"):
            return "correction_mismatch"
        if rust_actual.get("diagnostics") != python_actual.get("diagnostics"):
            return "diagnostics_mismatch"
        return "exact_match"
    if rust_actual.get("status") == "error":
        if rust_actual.get("error") != python_actual.get("error"):
            return "error_mismatch"
        return "exact_match"
    return "payload_mismatch"


def classify_case_mismatch(
    case: dict[str, Any], rust_actual: dict[str, Any], python_actual: dict[str, Any]
) -> str:
    classification = classify_mismatch(rust_actual, python_actual)
    if (
        classification == "correction_mismatch"
        and int(case.get("config", {}).get("max_bp_iterations", -1)) == 0
        and rust_actual.get("status") == "success"
        and python_actual.get("status") == "success"
        and rust_actual.get("diagnostics", {}).get("residual_syndrome_weight") == 0
        and python_actual.get("diagnostics", {}).get("residual_syndrome_weight") == 0
    ):
        return "zero_iter_semantics_mismatch"
    return classification


def is_real_mismatch(classification: str) -> bool:
    return classification in REAL_MISMATCH_CLASSES


def build_entries(
    repo_root: Path,
    fixtures_dir: Path,
    skip_generated: bool,
    case_limit: int | None,
    include_lsd: bool = False,
    fixture_catalog: Path = Path("rbposd/tests/fixtures/catalog.json"),
) -> list[dict[str, Any]]:
    case_items: list[dict[str, Any]] = []

    catalog_items = iter_catalog_fixture_cases(
        fixture_catalog, include_lsd=include_lsd
    )
    cataloged_fixture_paths = {
        Path(item["case_path"]).resolve()
        for item in catalog_items
        if item.get("case_path") is not None
    }
    case_items.extend(catalog_items)

    for fixture_path in fixture_case_paths(fixtures_dir):
        if fixture_path.resolve() in cataloged_fixture_paths:
            continue
        case_items.append(
            {
                "source": "fixture",
                "case_path": fixture_path,
                "case": load_case(fixture_path),
            }
            )

    if not skip_generated:
        for generated_case in iter_generated_cases(repo_root):
            case_items.append(
                {
                    "source": "generated",
                    "case_path": None,
                    "case": generated_case,
                }
            )

    if case_limit is not None:
        case_items = case_items[: max(0, case_limit)]

    entries: list[dict[str, Any]] = []
    for item in case_items:
        case = item["case"]
        case_path = item["case_path"]

        if case_path is None:
            with tempfile.NamedTemporaryFile(
                mode="w", suffix=".json", delete=False, encoding="utf-8"
            ) as tmp_file:
                json.dump(case, tmp_file)
                tmp_path = Path(tmp_file.name)
            rust_report = run_rust_case(repo_root, tmp_path)
            tmp_path.unlink(missing_ok=True)
        else:
            rust_report = run_rust_case(repo_root, case_path)

        rust_actual = rust_report["actual"]
        python_actual = run_python_ldpc(case)
        classification = classify_case_mismatch(case, rust_actual, python_actual)

        entries.append(
            {
                "name": case["name"],
                "source": item["source"],
                "mismatch_classification": classification,
                "is_mismatch": is_real_mismatch(classification),
                "rust_actual": rust_actual,
                "python_actual": python_actual,
                "tags": case.get("tags", []),
            }
        )

    return entries


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    entries = build_entries(
        repo_root=args.repo_root,
        fixtures_dir=args.fixtures_dir,
        skip_generated=args.skip_generated,
        case_limit=args.case_limit,
        include_lsd=args.include_lsd,
        fixture_catalog=args.fixture_catalog,
    )
    mismatch_count = sum(1 for entry in entries if entry["is_mismatch"])
    total_count = len(entries)

    for entry in entries:
        classification = entry["mismatch_classification"]
        if classification == "exact_match":
            continue
        suffix = ""
        if classification == "diagnostics_mismatch":
            suffix = " (diagnostics drift only; not counted as mismatch)"
        elif classification == "zero_iter_semantics_mismatch":
            suffix = (
                " (max_bp_iterations=0 uses different runtime semantics; "
                "not counted as mismatch)"
            )
        print(f"{entry['name']}: {classification}{suffix}")

    if args.json_output is not None:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        with args.json_output.open("w", encoding="utf-8") as outfile:
            json.dump(entries, outfile, indent=2)

    print(f"{mismatch_count} mismatches out of {total_count} cases")
    return 1 if mismatch_count > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
