from __future__ import annotations

import argparse
import csv
import json
import subprocess
import time
from pathlib import Path
from typing import Any, Callable, Sequence

from benchmarks.bb_circuit_bposd_compare.cases import CSV_HEADER, CompareCase, SMOKE_CASES
from benchmarks.bb_circuit_bposd_compare.summary import write_summary


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT_DIR = (
    REPO_ROOT / "benchmarks" / "bb_circuit_bposd_compare" / "results"
)


def _format_value(value: Any) -> str:
    if value is None:
        return ""
    return str(value)


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


def _run_rust_export(case: CompareCase) -> dict[str, Any]:
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "rsinter",
        "--bin",
        "rsinter",
        "--",
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
        "--json-compare-case",
    ]
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
    row.update({"status": "skipped", "error": str(error)})
    return row


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


def _python_row(case: CompareCase, export: dict[str, Any]) -> dict[str, str]:
    import numpy as np
    from ldpc import BpOsdDecoder

    setup_started = time.perf_counter()
    z_decoder = BpOsdDecoder(
        _dense_matrix(export["z_model"], np),
        error_channel=export["z_model"]["channel_probs"],
        max_iter=case.max_iter,
        bp_method="ms",
        osd_method="osd_cs",
        osd_order=case.osd_order,
        input_vector_type="syndrome",
    )
    x_decoder = BpOsdDecoder(
        _dense_matrix(export["x_model"], np),
        error_channel=export["x_model"]["channel_probs"],
        max_iter=case.max_iter,
        bp_method="ms",
        osd_method="osd_cs",
        osd_order=case.osd_order,
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
    rust_exporter: Callable[[CompareCase], dict[str, Any]] | None = None,
) -> int:
    exporter = rust_exporter or _run_rust_export
    output_dir.mkdir(parents=True, exist_ok=True)

    rows: list[dict[str, str]] = []
    saw_rust_error = False
    saw_skipped_python = False

    for case in cases:
        try:
            export = exporter(case)
        except Exception as error:
            saw_rust_error = True
            rows.append(_rust_error_row(case, error))
            continue

        rows.append(_rust_row(case, export))
        try:
            rows.append(_python_row(case, export))
        except ModuleNotFoundError as error:
            saw_skipped_python = True
            rows.append(_skipped_python_row(case, error))

    _write_rows(rows, output_dir / "results.csv")
    write_summary(rows, output_dir / "summary.md")

    if saw_rust_error:
        return 1
    if saw_skipped_python and not allow_missing_python:
        return 1
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", choices=("smoke",), required=True)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--allow-missing-python", action="store_true")
    args = parser.parse_args(argv)

    return run_suite(
        output_dir=args.output_dir,
        allow_missing_python=args.allow_missing_python,
        cases=SMOKE_CASES,
    )


if __name__ == "__main__":
    raise SystemExit(main())
