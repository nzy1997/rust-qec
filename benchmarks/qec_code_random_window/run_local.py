from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from benchmarks.qec_code_random_window.validate_cases import load_manifest, validate_manifest


ROOT = Path(__file__).resolve().parents[2]
METHOD = "random-window-upper-bound"
CONTEXT_LIMIT = 4000


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return parsed


def _nonnegative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be a non-negative integer")
    return parsed


def _clip_context(text: str, limit: int = CONTEXT_LIMIT) -> str:
    if len(text) <= limit:
        return text
    marker = "\n...[truncated]...\n"
    keep = max(0, (limit - len(marker)) // 2)
    return f"{text[:keep]}{marker}{text[-keep:]}"


def _default_qec_code_bin() -> str:
    env_bin = os.environ.get("QEC_CODE_BIN")
    if env_bin:
        return env_bin

    suffix = ".exe" if os.name == "nt" else ""
    workspace_bin = ROOT / "target" / "debug" / f"qec-code{suffix}"
    if workspace_bin.exists():
        return str(workspace_bin)

    return "qec-code"


def _case_int(case: dict[str, Any], field: str) -> int:
    value = case[field]
    if type(value) is not int:
        raise TypeError(f'{case["case_id"]} field "{field}" must be an integer')
    return value


def _case_str(case: dict[str, Any], field: str) -> str:
    value = case[field]
    if not isinstance(value, str):
        raise TypeError(f'{case["case_id"]} field "{field}" must be a string')
    return value


def _build_command(
    qec_code_bin: str,
    code_id: str,
    seed: int,
    iterations: int,
    restarts: int,
    target_weight: int,
) -> list[str]:
    return [
        qec_code_bin,
        "code",
        "css-distance",
        METHOD,
        "--code-id",
        code_id,
        "--iterations",
        str(iterations),
        "--restarts",
        str(restarts),
        "--seed",
        str(seed),
        "--target-weight",
        str(target_weight),
        "--json",
    ]


def _row_prefix(
    case: dict[str, Any],
    command: list[str],
    seed: int,
    iterations: int,
    restarts: int,
    target_weight: int,
    elapsed_s: float,
) -> dict[str, Any]:
    return {
        "case_id": _case_str(case, "case_id"),
        "code_id": _case_str(case, "code_id"),
        "distance_side": _case_str(case, "distance_side"),
        "seed": seed,
        "iterations": iterations,
        "restarts": restarts,
        "target_weight": target_weight,
        "target_upper_bound": case.get("target_upper_bound"),
        "baseline_key": case.get("baseline_key"),
        "baseline_required": case.get("baseline_required"),
        "command": command,
        "elapsed_s": elapsed_s,
        "upper_bound": None,
        "raw_cli_json": None,
    }


def _classify_completed(
    row: dict[str, Any],
    completed: subprocess.CompletedProcess[str],
) -> dict[str, Any]:
    row["returncode"] = completed.returncode

    if completed.returncode != 0:
        row["status"] = "cli_error"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        return row

    try:
        parsed = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        row["status"] = "non_json_stdout"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        row["error"] = str(error)
        return row

    row["raw_cli_json"] = parsed
    if not isinstance(parsed, dict):
        row["status"] = "invalid_cli_json"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        row["error"] = "parsed CLI JSON must be an object"
        return row

    if parsed.get("status") != "completed":
        row["status"] = "cli_not_completed"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        return row

    if parsed.get("method") != METHOD:
        row["status"] = "unexpected_method"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        return row

    upper_bound = parsed.get("upper_bound")
    if "upper_bound" not in parsed:
        row["status"] = "missing_upper_bound"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        return row

    if type(upper_bound) is not int or upper_bound <= 0:
        row["status"] = "invalid_upper_bound"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        return row

    row["upper_bound"] = upper_bound
    row["status"] = "ok"
    return row


def _run_case_seed(
    case: dict[str, Any],
    qec_code_bin: str,
    seed: int,
    iterations: int,
    restarts: int,
    target_weight: int,
) -> dict[str, Any]:
    command = _build_command(
        qec_code_bin,
        _case_str(case, "code_id"),
        seed,
        iterations,
        restarts,
        target_weight,
    )
    start = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        elapsed_s = time.perf_counter() - start
    except OSError as error:
        elapsed_s = time.perf_counter() - start
        row = _row_prefix(case, command, seed, iterations, restarts, target_weight, elapsed_s)
        row["status"] = "spawn_error"
        row["returncode"] = None
        row["error"] = str(error)
        row["stdout_context"] = ""
        row["stderr_context"] = ""
        return row

    row = _row_prefix(case, command, seed, iterations, restarts, target_weight, elapsed_s)
    return _classify_completed(row, completed)


def run(args: argparse.Namespace) -> int:
    try:
        manifest = load_manifest(args.cases)
    except Exception as error:
        print(f"{args.cases}: {error}", file=sys.stderr)
        return 2

    errors = validate_manifest(manifest)
    if errors:
        for error in errors:
            print(f"{args.cases}: {error}", file=sys.stderr)
        return 2

    cases = manifest["cases"]
    qec_code_bin = args.qec_code_bin or _default_qec_code_bin()
    rows_ok = True

    try:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with args.out.open("w", encoding="utf-8") as handle:
            for case in cases:
                assert isinstance(case, dict)
                seeds = (
                    args.seeds if args.seeds is not None else [_case_int(case, "seed")]
                )
                iterations = (
                    args.iterations
                    if args.iterations is not None
                    else _case_int(case, "iterations")
                )
                restarts = (
                    args.restarts
                    if args.restarts is not None
                    else _case_int(case, "restarts")
                )
                target_weight = (
                    args.target_weight
                    if args.target_weight is not None
                    else _case_int(case, "target_weight")
                )

                for seed in seeds:
                    row = _run_case_seed(
                        case,
                        qec_code_bin,
                        seed,
                        iterations,
                        restarts,
                        target_weight,
                    )
                    rows_ok = rows_ok and row["status"] == "ok"
                    handle.write(json.dumps(row, sort_keys=True) + "\n")
    except OSError as error:
        print(f"{args.out}: {error}", file=sys.stderr)
        return 2

    return 0 if rows_ok else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run local qec-code random-window upper-bound benchmarks."
    )
    parser.add_argument("--cases", required=True, type=Path, help="Path to a #321 case manifest.")
    parser.add_argument("--out", required=True, type=Path, help="Path to write JSONL results.")
    parser.add_argument("--seeds", nargs="+", type=_nonnegative_int, help="Override manifest seeds.")
    parser.add_argument("--iterations", type=_positive_int, help="Override manifest iterations.")
    parser.add_argument("--restarts", type=_positive_int, help="Override manifest restarts.")
    parser.add_argument(
        "--target-weight",
        type=_positive_int,
        help="Override manifest target_weight.",
    )
    parser.add_argument(
        "--qec-code-bin",
        help=(
            "Path to qec-code executable. Defaults to QEC_CODE_BIN, "
            "target/debug/qec-code, then PATH."
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
