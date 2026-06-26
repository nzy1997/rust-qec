from __future__ import annotations

import argparse
import csv
import json
import sys
import subprocess
import time
from pathlib import Path
from typing import Any, Callable, Sequence

from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    HARD_REPLAY_CASES,
    SMALL_LDPC_CASES,
    CompareCase,
    SMOKE_CASES,
    small_ldpc_manifest_rows,
    validate_small_ldpc_catalog,
)
from benchmarks.bb_circuit_bposd_compare.summary import write_summary


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT_DIR = (
    REPO_ROOT / "benchmarks" / "bb_circuit_bposd_compare" / "results"
)
HARD_REPLAY_FIXTURE_PATH = (
    REPO_ROOT
    / "rsinter"
    / "tests"
    / "fixtures"
    / "bb_circuit_bposd"
    / "bb90_hard_syndrome.json"
)
PYTHON_UPSTREAM_SEED = 12345
PYTHON_UPSTREAM_BP_METHOD = "ms"
PYTHON_UPSTREAM_MAX_ITER = 10000
PYTHON_UPSTREAM_OSD_METHOD = "osd_cs"
PYTHON_UPSTREAM_OSD_ORDER = 7
PYTHON_DEPENDENCY_HINTS = ("ldpc", "bposd", "numpy", "bposddecoder")


def _format_value(value: Any) -> str:
    if value is None:
        return ""
    return str(value)


def _format_json_list(values: Sequence[Any]) -> str:
    return json.dumps(list(values), separators=(",", ":"))


def _base_row(case: CompareCase, decoder_impl: str) -> dict[str, str]:
    return {
        "case_id": case.case_id,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": case.code_id,
        "p": _format_value(case.p),
        "num_cycles": _format_value(case.num_cycles),
        "num_trials": _format_value(case.num_trials),
        "seed": _format_value(case.seed),
        "bp_method": case.bp_method,
        "max_iter": _format_value(case.max_iter),
        "osd_method": case.osd_method,
        "osd_order": _format_value(case.osd_order),
        "setup_seconds": "",
        "decode_seconds": "",
        "run_seconds": "",
        "logical_error_rate": "",
        "status": "",
        "error": "",
    }


def _write_rows(rows: list[dict[str, str]], out_path: Path) -> None:
    with out_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_HEADER)
        writer.writeheader()
        for row in rows:
            writer.writerow({column: row.get(column, "") for column in CSV_HEADER})


def _write_manifest(rows: list[dict[str, str]], out_path: Path) -> None:
    with out_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CATALOG_HEADER)
        writer.writeheader()
        for row in rows:
            writer.writerow({column: row.get(column, "") for column in CATALOG_HEADER})


def _read_rows(in_path: Path) -> list[dict[str, str]]:
    with in_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def _python_upstream_settings() -> dict[str, str]:
    return {
        "seed": _format_value(PYTHON_UPSTREAM_SEED),
        "bp_method": PYTHON_UPSTREAM_BP_METHOD,
        "max_iter": _format_value(PYTHON_UPSTREAM_MAX_ITER),
        "osd_method": PYTHON_UPSTREAM_OSD_METHOD,
        "osd_order": _format_value(PYTHON_UPSTREAM_OSD_ORDER),
    }


def _run_rust_export(
    case: CompareCase,
    rust_binary: Path | None = None,
    osd_method: str | None = None,
) -> dict[str, Any]:
    if rust_binary is None:
        command = [
            "cargo",
            "run",
            "-q",
            "-p",
            "rsinter",
            "--bin",
            "rsinter",
            "--",
        ]
    else:
        command = [str(rust_binary)]
    command.extend(
        [
            "bb-circuit-bposd-memory",
            "--code-id",
            case.code_id,
            "--physical-error-rate",
            str(case.p),
            "--num-cycles",
            str(case.num_cycles),
            "--num-trials",
            str(case.num_trials),
            "--seed",
            str(case.seed),
            "--max-bp-iterations",
            str(case.max_iter),
            "--osd-order",
            str(case.osd_order),
        ]
    )
    if osd_method is not None:
        command.extend(["--osd-method", osd_method])
    command.append("--json-compare-case")
    result = subprocess.run(
        command,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            "Rust comparison export failed for "
            f"{case.case_id}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return json.loads(result.stdout)


def _call_exporter(
    exporter: Callable[..., dict[str, Any]],
    case: CompareCase,
    rust_binary: Path | None,
) -> dict[str, Any]:
    if exporter is _run_rust_export:
        return exporter(case, rust_binary=rust_binary, osd_method=case.osd_method)
    return exporter(case)


def _rust_row(case: CompareCase, export: dict[str, Any]) -> dict[str, str]:
    row = _base_row(case, "rbposd")
    profile = export["rust_result"]["profile"]
    setup_seconds = float(profile["setup_seconds"])
    decode_seconds = float(profile["decode_seconds"])
    row.update(
        {
            "setup_seconds": _format_value(setup_seconds),
            "decode_seconds": _format_value(decode_seconds),
            "run_seconds": _format_value(setup_seconds + decode_seconds),
            "logical_error_rate": _format_value(
                export["rust_result"]["num_failed_trials"] / case.num_trials
            ),
            "status": "ok",
        }
    )
    return row


def _rust_error_row(case: CompareCase, error: Exception) -> dict[str, str]:
    row = _base_row(case, "rbposd")
    row.update({"status": "error", "error": str(error)})
    return row


def _skipped_python_row(case: CompareCase, error: Exception) -> dict[str, str]:
    row = _base_row(case, "ldpc_bposd")
    row.update(_python_upstream_settings())
    row.update({"status": "skipped", "error": _python_dependency_error_text(error)})
    return row


def _python_dependency_error_text(error: Exception) -> str:
    return f"python dependency unavailable for ldpc_bposd replay: {error}"


def _missing_python_dependency_messages(rows: Sequence[dict[str, str]]) -> list[str]:
    messages: list[str] = []
    seen: set[str] = set()
    for row in rows:
        if row.get("decoder_impl") != "ldpc_bposd":
            continue
        if row.get("status") != "skipped":
            continue
        message = row.get("error", "")
        if not message or message in seen:
            continue
        seen.add(message)
        messages.append(message)
    return messages


def _is_missing_python_dependency(error: Exception) -> bool:
    if not isinstance(error, ImportError):
        return False

    current: BaseException | None = error
    while current is not None:
        message = str(current).lower()
        if any(hint in message for hint in PYTHON_DEPENDENCY_HINTS):
            return True
        current = current.__cause__ or current.__context__
    return False


def _dense_matrix(model: dict[str, Any], np: Any) -> Any:
    matrix = np.zeros((model["num_checks"], model["num_bits"]), dtype=np.uint8)
    for row_index, sparse_columns in enumerate(model["sparse_rows"]):
        for column_index in sparse_columns:
            matrix[row_index, column_index] = 1
    return matrix


def _predicted_logicals(
    correction: Any,
    model: dict[str, Any],
    num_logicals: int,
) -> list[bool]:
    logicals = [False] * num_logicals
    for column_index, enabled in enumerate(correction.tolist()):
        if not enabled:
            continue
        for row_index in model["augmented_columns"][column_index]:
            if row_index >= model["first_logical_row"]:
                logicals[row_index - model["first_logical_row"]] ^= True
    return logicals


def _load_hard_replay_fixture() -> dict[str, Any]:
    return json.loads(HARD_REPLAY_FIXTURE_PATH.read_text())


def _hard_replay_bundle(
    case: CompareCase,
    export: dict[str, Any],
    fixture: dict[str, Any],
) -> dict[str, Any]:
    if fixture["case_id"] != case.case_id:
        raise RuntimeError(f"hard replay fixture case_id mismatch: {fixture['case_id']}")
    basis = fixture["basis"]
    if basis != "Z":
        raise RuntimeError(f"unsupported hard replay basis: {basis}")
    trial = export["trials"][0]
    syndrome = list(trial["z_syndrome"])
    support = [index for index, bit in enumerate(syndrome) if bit]
    if support != list(fixture["syndrome_support"]):
        raise RuntimeError("hard replay syndrome support does not match fixture")
    expected_logical = list(trial["z_logical"])
    if expected_logical != list(fixture["expected_sampled_logical"]):
        raise RuntimeError("hard replay sampled logical does not match fixture")
    rust_prediction = trial.get("z_logical_prediction")
    if rust_prediction is None:
        raise RuntimeError("Rust hard replay export is missing z_logical_prediction")
    z_profile = trial.get("z_profile")
    if not isinstance(z_profile, dict):
        raise RuntimeError("Rust hard replay export is missing z_profile")
    return {
        "basis": basis,
        "model": export["z_model"],
        "syndrome": syndrome,
        "syndrome_support": support,
        "expected_logical": expected_logical,
        "rust_prediction": list(rust_prediction),
        "rust_profile": z_profile,
    }


def _update_replay_metadata(
    row: dict[str, str],
    bundle: dict[str, Any],
    logical_prediction: Sequence[bool] | None,
) -> None:
    row.update(
        {
            "basis": bundle["basis"],
            "syndrome_weight": _format_value(len(bundle["syndrome_support"])),
            "syndrome_support": _format_json_list(bundle["syndrome_support"]),
            "logical_prediction": ""
            if logical_prediction is None
            else _format_json_list(logical_prediction),
            "expected_logical": _format_json_list(bundle["expected_logical"]),
        }
    )


def _rust_hard_replay_row(
    case: CompareCase,
    export: dict[str, Any],
    fixture: dict[str, Any],
) -> dict[str, str]:
    bundle = _hard_replay_bundle(case, export, fixture)
    row = _base_row(case, "rbposd")
    setup_seconds = float(export["rust_result"]["profile"]["setup_seconds"])
    decode_seconds = float(bundle["rust_profile"]["decode_seconds"])
    logical_prediction = bundle["rust_prediction"]
    _update_replay_metadata(row, bundle, logical_prediction)
    row.update(
        {
            "setup_seconds": _format_value(setup_seconds),
            "decode_seconds": _format_value(decode_seconds),
            "run_seconds": _format_value(setup_seconds + decode_seconds),
            "logical_error_rate": _format_value(
                0.0 if logical_prediction == bundle["expected_logical"] else 1.0
            ),
            "bp_seconds": _format_value(bundle["rust_profile"]["bp_seconds"]),
            "osd_seconds": _format_value(bundle["rust_profile"]["osd_seconds"]),
            "decode_call_count": _format_value(bundle["rust_profile"]["decode_call_count"]),
            "bp_iteration_count": _format_value(bundle["rust_profile"]["bp_iteration_count"]),
            "osd_use_count": _format_value(bundle["rust_profile"]["osd_use_count"]),
            "osd_candidate_count": _format_value(bundle["rust_profile"]["osd_candidate_count"]),
            "gf2_solve_count": _format_value(bundle["rust_profile"]["gf2_solve_count"]),
            "gf2_full_elimination_count": _format_value(
                bundle["rust_profile"]["gf2_full_elimination_count"]
            ),
            "status": "ok",
        }
    )
    return row


def _python_hard_replay_row(
    case: CompareCase,
    export: dict[str, Any],
    fixture: dict[str, Any],
) -> dict[str, str]:
    import numpy as np
    from ldpc import BpOsdDecoder

    bundle = _hard_replay_bundle(case, export, fixture)
    setup_started = time.perf_counter()
    decoder = BpOsdDecoder(
        _dense_matrix(bundle["model"], np),
        error_channel=bundle["model"]["channel_probs"],
        max_iter=PYTHON_UPSTREAM_MAX_ITER,
        bp_method=PYTHON_UPSTREAM_BP_METHOD,
        osd_method=PYTHON_UPSTREAM_OSD_METHOD,
        osd_order=PYTHON_UPSTREAM_OSD_ORDER,
        input_vector_type="syndrome",
    )
    setup_seconds = time.perf_counter() - setup_started

    decode_started = time.perf_counter()
    correction = decoder.decode(np.asarray(bundle["syndrome"], dtype=np.uint8))
    logical_prediction = _predicted_logicals(
        correction,
        bundle["model"],
        len(bundle["expected_logical"]),
    )
    decode_seconds = time.perf_counter() - decode_started

    row = _base_row(case, "ldpc_bposd")
    row.update(_python_upstream_settings())
    _update_replay_metadata(row, bundle, logical_prediction)
    row.update(
        {
            "setup_seconds": _format_value(setup_seconds),
            "decode_seconds": _format_value(decode_seconds),
            "run_seconds": _format_value(setup_seconds + decode_seconds),
            "logical_error_rate": _format_value(
                0.0 if logical_prediction == bundle["expected_logical"] else 1.0
            ),
            "status": "ok",
        }
    )
    return row


def _python_row(case: CompareCase, export: dict[str, Any]) -> dict[str, str]:
    import numpy as np
    from ldpc import BpOsdDecoder

    setup_started = time.perf_counter()
    z_decoder = BpOsdDecoder(
        _dense_matrix(export["z_model"], np),
        error_channel=export["z_model"]["channel_probs"],
        max_iter=PYTHON_UPSTREAM_MAX_ITER,
        bp_method=PYTHON_UPSTREAM_BP_METHOD,
        osd_method=PYTHON_UPSTREAM_OSD_METHOD,
        osd_order=PYTHON_UPSTREAM_OSD_ORDER,
        input_vector_type="syndrome",
    )
    x_decoder = BpOsdDecoder(
        _dense_matrix(export["x_model"], np),
        error_channel=export["x_model"]["channel_probs"],
        max_iter=PYTHON_UPSTREAM_MAX_ITER,
        bp_method=PYTHON_UPSTREAM_BP_METHOD,
        osd_method=PYTHON_UPSTREAM_OSD_METHOD,
        osd_order=PYTHON_UPSTREAM_OSD_ORDER,
        input_vector_type="syndrome",
    )
    setup_seconds = time.perf_counter() - setup_started

    decode_started = time.perf_counter()
    num_failed_trials = 0
    for trial in export["trials"]:
        z_correction = z_decoder.decode(np.asarray(trial["z_syndrome"], dtype=np.uint8))
        z_predicted = _predicted_logicals(
            z_correction,
            export["z_model"],
            len(trial["z_logical"]),
        )
        if z_predicted != list(trial["z_logical"]):
            num_failed_trials += 1
            continue

        x_correction = x_decoder.decode(np.asarray(trial["x_syndrome"], dtype=np.uint8))
        x_predicted = _predicted_logicals(
            x_correction,
            export["x_model"],
            len(trial["x_logical"]),
        )
        if x_predicted != list(trial["x_logical"]):
            num_failed_trials += 1
    decode_seconds = time.perf_counter() - decode_started

    row = _base_row(case, "ldpc_bposd")
    row.update(_python_upstream_settings())
    row.update(
        {
            "setup_seconds": _format_value(setup_seconds),
            "decode_seconds": _format_value(decode_seconds),
            "run_seconds": _format_value(setup_seconds + decode_seconds),
            "logical_error_rate": _format_value(num_failed_trials / case.num_trials),
            "status": "ok",
        }
    )
    return row


def run_suite(
    output_dir: Path,
    allow_missing_python: bool = False,
    cases: Sequence[CompareCase] = SMOKE_CASES,
    rust_binary: Path | None = None,
    rust_exporter: Callable[..., dict[str, Any]] | None = None,
) -> int:
    exporter = rust_exporter or _run_rust_export
    output_dir.mkdir(parents=True, exist_ok=True)

    rows: list[dict[str, str]] = []
    saw_rust_error = False
    saw_skipped_python = False

    for case in cases:
        try:
            export = _call_exporter(exporter, case, rust_binary)
        except Exception as error:
            saw_rust_error = True
            rows.append(_rust_error_row(case, error))
            continue

        rows.append(_rust_row(case, export))
        try:
            rows.append(_python_row(case, export))
        except ImportError as error:
            if not _is_missing_python_dependency(error):
                raise
            saw_skipped_python = True
            rows.append(_skipped_python_row(case, error))

    _write_rows(rows, output_dir / "results.csv")
    write_summary(rows, output_dir / "summary.md")

    if saw_rust_error:
        return 1
    if saw_skipped_python and not allow_missing_python:
        return 1
    return 0


def run_hard_replay_suite(
    output_dir: Path,
    allow_missing_python: bool = False,
    rust_binary: Path | None = None,
    rust_exporter: Callable[..., dict[str, Any]] | None = None,
) -> int:
    exporter = rust_exporter or _run_rust_export
    output_dir.mkdir(parents=True, exist_ok=True)
    fixture = _load_hard_replay_fixture()
    rows: list[dict[str, str]] = []
    saw_rust_error = False
    saw_skipped_python = False

    for case in HARD_REPLAY_CASES:
        try:
            export = _call_exporter(exporter, case, rust_binary)
            rows.append(_rust_hard_replay_row(case, export, fixture))
        except Exception as error:
            saw_rust_error = True
            rows.append(_rust_error_row(case, error))
            continue

        try:
            rows.append(_python_hard_replay_row(case, export, fixture))
        except ImportError as error:
            if not _is_missing_python_dependency(error):
                raise
            saw_skipped_python = True
            skipped = _skipped_python_row(case, error)
            try:
                _update_replay_metadata(
                    skipped,
                    _hard_replay_bundle(case, export, fixture),
                    None,
                )
            except Exception:
                pass
            rows.append(skipped)

    _write_rows(rows, output_dir / "results.csv")
    write_summary(rows, output_dir / "summary.md")
    if saw_rust_error:
        return 1
    if saw_skipped_python and not allow_missing_python:
        return 1
    return 0


def run_small_ldpc_catalog_dry_run(
    output_dir: Path,
    cases: Sequence[CompareCase] = SMALL_LDPC_CASES,
) -> int:
    output_dir.mkdir(parents=True, exist_ok=True)
    errors = validate_small_ldpc_catalog(cases)
    _write_manifest(small_ldpc_manifest_rows(cases), output_dir / "manifest.csv")
    for error in errors:
        print(error, file=sys.stderr)
    return 1 if errors else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--tier",
        choices=("smoke", "small_ldpc_catalog", "hard-replay"),
        required=True,
    )
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--allow-missing-python", action="store_true")
    parser.add_argument("--rust-binary", type=Path)
    args = parser.parse_args(argv)

    if args.tier == "small_ldpc_catalog":
        return run_small_ldpc_catalog_dry_run(args.output_dir)

    if args.tier == "hard-replay":
        status = run_hard_replay_suite(
            output_dir=args.output_dir,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status != 0 and not args.allow_missing_python:
            for message in _missing_python_dependency_messages(
                _read_rows(args.output_dir / "results.csv")
            ):
                print(message, file=sys.stderr)
        return status

    status = run_suite(
        output_dir=args.output_dir,
        allow_missing_python=args.allow_missing_python,
        cases=SMOKE_CASES,
        rust_binary=args.rust_binary,
    )
    if status != 0 and not args.allow_missing_python:
        for message in _missing_python_dependency_messages(
            _read_rows(args.output_dir / "results.csv")
        ):
            print(message, file=sys.stderr)
    return status


if __name__ == "__main__":
    raise SystemExit(main())
