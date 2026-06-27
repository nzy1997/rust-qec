from __future__ import annotations

import argparse
import csv
import json
import sys
import subprocess
import time
from dataclasses import replace
from pathlib import Path
from typing import Any, Callable, Sequence

from benchmarks.bb_circuit_bposd_compare.cases import (
    BATCHED_CSV_HEADER,
    BB72_BB144_FULL_CASES,
    BB72_BB144_PLOT_SMOKE_CASES,
    CATALOG_HEADER,
    CSV_HEADER,
    DIAGNOSTIC_CASES,
    HARD_REPLAY_CASES,
    SMALL_LDPC_CASES,
    CompareCase,
    SMOKE_CASES,
    small_ldpc_manifest_rows,
    validate_diagnostic_cases,
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
RUST_PROFILE_COUNTER_FIELDS = (
    "bp_seconds",
    "osd_seconds",
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)
BATCHED_DEFAULT_BATCH_SIZE = 500
BB_COMPARE_PLOT_SPEC_PATH = (
    REPO_ROOT / "benchmarks" / "bb_circuit_bposd_compare" / "plot.toml"
)


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


def _write_batched_rows(rows: list[dict[str, str]], out_path: Path) -> None:
    with out_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=BATCHED_CSV_HEADER)
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {column: row.get(column, "") for column in BATCHED_CSV_HEADER}
            )


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
    for field in RUST_PROFILE_COUNTER_FIELDS:
        if field in profile:
            row[field] = _format_value(profile[field])
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


def _default_progress(message: str) -> None:
    print(f"[bb-bposd] {message}", file=sys.stderr, flush=True)


def _results_signature(output_dir: Path) -> tuple[int, int] | None:
    results_path = output_dir / "results.csv"
    try:
        stat = results_path.stat()
    except FileNotFoundError:
        return None
    return (stat.st_mtime_ns, stat.st_size)


def _print_missing_python_dependency_messages(
    output_dir: Path,
    previous_results_signature: tuple[int, int] | None = None,
) -> None:
    current_results_signature = _results_signature(output_dir)
    if (
        current_results_signature is None
        or current_results_signature == previous_results_signature
    ):
        return
    results_path = output_dir / "results.csv"
    for message in _missing_python_dependency_messages(_read_rows(results_path)):
        print(message, file=sys.stderr)


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


def _python_batch_stats(case: CompareCase, export: dict[str, Any]) -> dict[str, float | int]:
    row = _python_row(case, export)
    return {
        "setup_seconds": float(row["setup_seconds"]),
        "decode_seconds": float(row["decode_seconds"]),
        "num_failed_trials": int(
            round(float(row["logical_error_rate"]) * case.num_trials)
        ),
    }


def _empty_profile() -> dict[str, float | int]:
    return {
        "setup_seconds": 0.0,
        "sample_seconds": 0.0,
        "decode_seconds": 0.0,
        "bp_seconds": 0.0,
        "osd_seconds": 0.0,
        "decode_call_count": 0,
        "bp_iteration_count": 0,
        "osd_use_count": 0,
        "osd_candidate_count": 0,
        "gf2_solve_count": 0,
        "gf2_full_elimination_count": 0,
    }


def _add_profile(accumulator: dict[str, float | int], profile: dict[str, Any]) -> None:
    for field in accumulator:
        accumulator[field] += profile.get(field, 0)


def _batch_case(case: CompareCase, batch_index: int, num_trials: int) -> CompareCase:
    return replace(
        case,
        case_id=f"{case.case_id}-batch{batch_index:05d}",
        num_trials=num_trials,
        seed=case.seed + batch_index,
    )


def _batched_row(
    case: CompareCase,
    decoder_impl: str,
    *,
    batch_size: int,
    batches_completed: int,
    shots_used: int,
    logical_errors: int,
    setup_seconds: float,
    sample_seconds: float,
    decode_seconds: float,
    profile: dict[str, float | int] | None,
    status: str,
    stop_reason: str,
    error: str = "",
) -> dict[str, str]:
    profile = profile or {}
    logical_error_rate = logical_errors / shots_used if shots_used else 0.0
    return {
        "case_id": case.case_id,
        "runner": "batched_compare",
        "decoder_impl": decoder_impl,
        "code_id": case.code_id,
        "p": _format_value(case.p),
        "num_cycles": _format_value(case.num_cycles),
        "shots_budget": _format_value(case.num_trials),
        "errors_budget": _format_value(case.max_errors),
        "shots_used": _format_value(shots_used),
        "seed": _format_value(case.seed),
        "bp_method": case.bp_method,
        "max_iter": _format_value(case.max_iter),
        "osd_method": case.osd_method,
        "osd_order": _format_value(case.osd_order),
        "batch_size": _format_value(batch_size),
        "batches_completed": _format_value(batches_completed),
        "setup_seconds": _format_value(setup_seconds),
        "sample_seconds": _format_value(sample_seconds),
        "decode_seconds": _format_value(decode_seconds),
        "run_seconds": _format_value(setup_seconds + sample_seconds + decode_seconds),
        "logical_errors": _format_value(logical_errors),
        "logical_error_rate": _format_value(logical_error_rate),
        "bp_seconds": _format_value(profile.get("bp_seconds")),
        "osd_seconds": _format_value(profile.get("osd_seconds")),
        "decode_call_count": _format_value(profile.get("decode_call_count")),
        "bp_iteration_count": _format_value(profile.get("bp_iteration_count")),
        "osd_use_count": _format_value(profile.get("osd_use_count")),
        "osd_candidate_count": _format_value(profile.get("osd_candidate_count")),
        "gf2_solve_count": _format_value(profile.get("gf2_solve_count")),
        "gf2_full_elimination_count": _format_value(
            profile.get("gf2_full_elimination_count")
        ),
        "status": status,
        "stop_reason": stop_reason,
        "error": error,
    }


def run_batched_suite(
    output_dir: Path,
    cases: Sequence[CompareCase],
    batch_size: int = BATCHED_DEFAULT_BATCH_SIZE,
    wall_budget_seconds: float | None = None,
    allow_missing_python: bool = False,
    rust_binary: Path | None = None,
    rust_exporter: Callable[..., dict[str, Any]] | None = None,
    python_batch_stats: Callable[[CompareCase, dict[str, Any]], dict[str, float | int]]
    | None = None,
    monotonic: Callable[[], float] = time.monotonic,
    progress: Callable[[str], None] | None = _default_progress,
) -> int:
    if batch_size <= 0:
        raise ValueError("batch_size must be positive")

    exporter = rust_exporter or _run_rust_export
    python_stats = python_batch_stats or _python_batch_stats
    output_dir.mkdir(parents=True, exist_ok=True)

    rows: list[dict[str, str]] = []
    saw_error = False
    saw_skipped_python = False
    started_at = monotonic()
    budget_exhausted = False

    for case_index, case in enumerate(cases, start=1):
        if budget_exhausted:
            break

        rust_profile = _empty_profile()
        py_setup_seconds = 0.0
        py_decode_seconds = 0.0
        rust_logical_errors = 0
        py_logical_errors = 0
        shots_used = 0
        batches_completed = 0
        stop_reason = "completed"
        case_had_error = False
        if progress is not None:
            progress(
                f"case {case_index}/{len(cases)} start "
                f"{case.code_id} p={case.p} cycles={case.num_cycles} "
                f"shots_budget={case.num_trials} errors_budget={case.max_errors} "
                f"batch_size={batch_size}"
            )

        for batch_index, start in enumerate(range(0, case.num_trials, batch_size)):
            if (
                wall_budget_seconds is not None
                and monotonic() - started_at >= wall_budget_seconds
            ):
                stop_reason = "wall_budget_exhausted"
                budget_exhausted = True
                break

            current_batch_size = min(batch_size, case.num_trials - start)
            batch_case = _batch_case(case, batch_index, current_batch_size)
            try:
                export = _call_exporter(exporter, batch_case, rust_binary)
            except Exception as error:
                saw_error = True
                case_had_error = True
                rows.append(
                    _batched_row(
                        case,
                        "rbposd",
                        batch_size=batch_size,
                        batches_completed=batches_completed,
                        shots_used=shots_used,
                        logical_errors=rust_logical_errors,
                        setup_seconds=float(rust_profile["setup_seconds"]),
                        sample_seconds=float(rust_profile["sample_seconds"]),
                        decode_seconds=float(rust_profile["decode_seconds"]),
                        profile=rust_profile,
                        status="error",
                        stop_reason="rust_error",
                        error=str(error),
                    )
                )
                break

            profile = export["rust_result"]["profile"]
            _add_profile(rust_profile, profile)
            rust_logical_errors += int(export["rust_result"]["num_failed_trials"])
            shots_used += current_batch_size
            batches_completed += 1
            try:
                py_stats = python_stats(batch_case, export)
            except ImportError as error:
                if not _is_missing_python_dependency(error):
                    raise
                saw_skipped_python = True
                stop_reason = "python_dependency_missing"
                rows.append(
                    _batched_row(
                        case,
                        "ldpc_bposd",
                        batch_size=batch_size,
                        batches_completed=batches_completed,
                        shots_used=shots_used,
                        logical_errors=py_logical_errors,
                        setup_seconds=py_setup_seconds,
                        sample_seconds=0.0,
                        decode_seconds=py_decode_seconds,
                        profile=None,
                        status="skipped",
                        stop_reason=stop_reason,
                        error=_python_dependency_error_text(error),
                    )
                )
                break

            py_setup_seconds += float(py_stats["setup_seconds"])
            py_decode_seconds += float(py_stats["decode_seconds"])
            py_logical_errors += int(py_stats["num_failed_trials"])
            if progress is not None:
                progress(
                    f"case {case_index}/{len(cases)} batch {batches_completed} "
                    f"shots={shots_used}/{case.num_trials} "
                    f"rust_errors={rust_logical_errors} "
                    f"ldpc_errors={py_logical_errors}"
                )
            if case.max_errors is not None and max(
                rust_logical_errors, py_logical_errors
            ) >= int(case.max_errors):
                stop_reason = "errors_budget_reached"
                break

        if case_had_error:
            continue

        status = (
            "partial"
            if stop_reason in {"wall_budget_exhausted", "python_dependency_missing"}
            else "ok"
        )
        rows.append(
            _batched_row(
                case,
                "rbposd",
                batch_size=batch_size,
                batches_completed=batches_completed,
                shots_used=shots_used,
                logical_errors=rust_logical_errors,
                setup_seconds=float(rust_profile["setup_seconds"]),
                sample_seconds=float(rust_profile["sample_seconds"]),
                decode_seconds=float(rust_profile["decode_seconds"]),
                profile=rust_profile,
                status=status,
                stop_reason=stop_reason,
            )
        )
        if not (saw_skipped_python and stop_reason == "python_dependency_missing"):
            rows.append(
                _batched_row(
                    case,
                    "ldpc_bposd",
                    batch_size=batch_size,
                    batches_completed=batches_completed,
                    shots_used=shots_used,
                    logical_errors=py_logical_errors,
                    setup_seconds=py_setup_seconds,
                    sample_seconds=0.0,
                    decode_seconds=py_decode_seconds,
                    profile=None,
                    status=status,
                    stop_reason=stop_reason,
                )
            )

        if progress is not None:
            progress(
                f"case {case_index}/{len(cases)} done "
                f"status={status} stop_reason={stop_reason} "
                f"shots={shots_used}/{case.num_trials} "
                f"rust_errors={rust_logical_errors} "
                f"ldpc_errors={py_logical_errors}"
            )

    _write_batched_rows(rows, output_dir / "results.csv")
    write_summary(rows, output_dir / "summary.md")
    if saw_error:
        return 1
    if saw_skipped_python and not allow_missing_python:
        return 1
    return 0


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


def run_diagnostic_suite(
    output_dir: Path,
    allow_missing_python: bool = False,
    rust_binary: Path | None = None,
    rust_exporter: Callable[..., dict[str, Any]] | None = None,
) -> int:
    errors = validate_diagnostic_cases(DIAGNOSTIC_CASES)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    return run_suite(
        output_dir=output_dir,
        allow_missing_python=allow_missing_python,
        cases=DIAGNOSTIC_CASES,
        rust_binary=rust_binary,
        rust_exporter=rust_exporter,
    )


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


def _rust_plot_command(
    results_path: Path,
    out_path: Path,
    rust_binary: Path | None = None,
) -> list[str]:
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
            "bench",
            "plot-bb-compare-csv",
            "--spec",
            str(BB_COMPARE_PLOT_SPEC_PATH),
            "--input",
            str(results_path),
            "--out",
            str(out_path),
        ]
    )
    return command


def _render_batched_plot(output_dir: Path, rust_binary: Path | None = None) -> None:
    command = _rust_plot_command(
        output_dir / "results.csv",
        output_dir / "bb_circuit_bposd_compare.png",
        rust_binary=rust_binary,
    )
    result = subprocess.run(
        command,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            "Rust BB comparison plot failed\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--tier",
        choices=(
            "smoke",
            "small_ldpc_catalog",
            "hard-replay",
            "diagnostic",
            "bb72-bb144-plot-smoke",
            "full",
        ),
        required=True,
    )
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--allow-missing-python", action="store_true")
    parser.add_argument("--rust-binary", type=Path)
    parser.add_argument("--batch-size", type=int, default=BATCHED_DEFAULT_BATCH_SIZE)
    parser.add_argument("--wall-budget-seconds", type=float)
    args = parser.parse_args(argv)

    if args.tier == "small_ldpc_catalog":
        return run_small_ldpc_catalog_dry_run(args.output_dir)

    if args.tier == "bb72-bb144-plot-smoke":
        status = run_batched_suite(
            output_dir=args.output_dir,
            cases=BB72_BB144_PLOT_SMOKE_CASES,
            batch_size=args.batch_size,
            wall_budget_seconds=args.wall_budget_seconds,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status == 0:
            _render_batched_plot(args.output_dir, args.rust_binary)
        return status

    if args.tier == "full":
        status = run_batched_suite(
            output_dir=args.output_dir,
            cases=BB72_BB144_FULL_CASES,
            batch_size=args.batch_size,
            wall_budget_seconds=args.wall_budget_seconds,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status == 0:
            _render_batched_plot(args.output_dir, args.rust_binary)
        return status

    if args.tier == "hard-replay":
        previous_results_signature = _results_signature(args.output_dir)
        status = run_hard_replay_suite(
            output_dir=args.output_dir,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status != 0 and not args.allow_missing_python:
            _print_missing_python_dependency_messages(
                args.output_dir,
                previous_results_signature,
            )
        return status

    if args.tier == "diagnostic":
        previous_results_signature = _results_signature(args.output_dir)
        status = run_diagnostic_suite(
            output_dir=args.output_dir,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status != 0 and not args.allow_missing_python:
            _print_missing_python_dependency_messages(
                args.output_dir,
                previous_results_signature,
            )
        return status

    previous_results_signature = _results_signature(args.output_dir)
    status = run_suite(
        output_dir=args.output_dir,
        allow_missing_python=args.allow_missing_python,
        cases=SMOKE_CASES,
        rust_binary=args.rust_binary,
    )
    if status != 0 and not args.allow_missing_python:
        _print_missing_python_dependency_messages(
            args.output_dir,
            previous_results_signature,
        )
    return status


if __name__ == "__main__":
    raise SystemExit(main())
