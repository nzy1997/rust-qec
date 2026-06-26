from __future__ import annotations

import argparse
import csv
import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    CompareCase,
    validate_small_ldpc_catalog,
)
from benchmarks.bb_circuit_bposd_compare.verify_diagnostic import (
    verify_rows as verify_diagnostic_rows,
)
from benchmarks.bb_circuit_bposd_compare.verify_replay import (
    verify_rows as verify_replay_rows,
)

PASS = "PASS"
WARN = "WARN"
FAIL = "FAIL"

SEMANTIC_REPLAY_PATH = Path("hard-replay/results.csv")
HARD_PROFILE_PATH = Path("hard-profile/profile.json")
SETUP_RUN_PATH = Path("setup-run/profile.json")
CATALOG_PATH = Path("small-ldpc-catalog/manifest.csv")
DIAGNOSTIC_PATH = Path("diagnostic/results.csv")
PROVENANCE_PATH = Path("provenance.json")


@dataclass(frozen=True)
class CheckResult:
    name: str
    status: str
    artifact: str
    messages: tuple[str, ...] = ()


def _pass(name: str, artifact: Path, message: str = "ok") -> CheckResult:
    return CheckResult(name, PASS, artifact.as_posix(), (message,))


def _warn(name: str, artifact: Path, message: str) -> CheckResult:
    return CheckResult(name, WARN, artifact.as_posix(), (message,))


def _fail(name: str, artifact: Path, message: str) -> CheckResult:
    return CheckResult(name, FAIL, artifact.as_posix(), (message,))


def _load_csv(
    results_dir: Path, relative_path: Path, header: Sequence[str]
) -> tuple[list[dict[str, str]] | None, list[str]]:
    path = results_dir / relative_path
    if not path.exists():
        return None, [f"missing artifact: {relative_path.as_posix()}"]
    try:
        with path.open(newline="") as handle:
            reader = csv.DictReader(handle)
            fieldnames = list(reader.fieldnames or [])
            rows = list(reader)
    except csv.Error as error:
        return None, [f"malformed CSV {relative_path.as_posix()}: {error}"]
    except OSError as error:
        return None, [f"cannot read {relative_path.as_posix()}: {error}"]
    missing = [column for column in header if column not in fieldnames]
    if missing:
        return rows, [
            f"{relative_path.as_posix()} is missing required CSV column(s): "
            + ", ".join(missing)
        ]
    return rows, []


def _load_json_object(
    results_dir: Path, relative_path: Path
) -> tuple[dict[str, object] | None, list[str]]:
    path = results_dir / relative_path
    if not path.exists():
        return None, [f"missing artifact: {relative_path.as_posix()}"]
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as error:
        return None, [f"malformed JSON {relative_path.as_posix()}: {error}"]
    except OSError as error:
        return None, [f"cannot read {relative_path.as_posix()}: {error}"]
    if not isinstance(data, dict):
        return None, [f"{relative_path.as_posix()} must contain a JSON object"]
    return data, []


def _require_field(data: dict[str, object], field: str, errors: list[str]) -> object | None:
    if field not in data:
        errors.append(f"{field} is required")
        return None
    return data[field]


def _require_nonempty_string(
    data: dict[str, object], field: str, errors: list[str]
) -> str | None:
    value = _require_field(data, field, errors)
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{field} must be a nonempty string")
        return None
    return value


def _as_int(data: dict[str, object], field: str, errors: list[str]) -> int | None:
    value = _require_field(data, field, errors)
    if value is None:
        return None
    if isinstance(value, bool) or value is None:
        errors.append(f"{field} must be an integer")
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and math.isfinite(value) and value.is_integer():
        return int(value)
    if isinstance(value, str):
        try:
            parsed = float(value)
        except ValueError:
            errors.append(f"{field} must be an integer")
            return None
        if math.isfinite(parsed) and parsed.is_integer():
            return int(parsed)
    errors.append(f"{field} must be an integer")
    return None


def _require_nonnegative_int(
    data: dict[str, object], field: str, errors: list[str]
) -> int | None:
    value = _as_int(data, field, errors)
    if value is not None and value < 0:
        errors.append(f"{field} must be non-negative")
        return None
    return value


def _require_positive_int(
    data: dict[str, object], field: str, errors: list[str]
) -> int | None:
    value = _as_int(data, field, errors)
    if value is not None and value <= 0:
        errors.append(f"{field} must be positive")
        return None
    return value


def _as_seconds(data: dict[str, object], field: str, errors: list[str]) -> float | None:
    value = _require_field(data, field, errors)
    if value is None:
        return None
    if isinstance(value, bool) or value is None:
        errors.append(f"{field} must be finite and non-negative")
        return None
    try:
        seconds = float(value)
    except (TypeError, ValueError):
        errors.append(f"{field} must be finite and non-negative")
        return None
    if not math.isfinite(seconds) or seconds < 0.0:
        errors.append(f"{field} must be finite and non-negative")
        return None
    return seconds


def _check_semantic_replay(results_dir: Path) -> CheckResult:
    rows, errors = _load_csv(results_dir, SEMANTIC_REPLAY_PATH, CSV_HEADER)
    if rows is not None:
        errors.extend(verify_replay_rows(rows, allow_missing_python=False))
    if errors:
        return _fail("semantic-replay", SEMANTIC_REPLAY_PATH, "; ".join(errors))
    return _pass("semantic-replay", SEMANTIC_REPLAY_PATH, "hard replay rows verified")


def _check_hard_profile(results_dir: Path) -> CheckResult:
    data, errors = _load_json_object(results_dir, HARD_PROFILE_PATH)
    if data is None:
        return _fail("hard-profile", HARD_PROFILE_PATH, "; ".join(errors))
    case_id = _require_nonempty_string(data, "case_id", errors)
    basis = _require_nonempty_string(data, "basis", errors)
    osd_planner = _require_nonempty_string(data, "osd_planner", errors)
    osd_order = _require_positive_int(data, "osd_order", errors)
    candidate_limit = _require_positive_int(data, "candidate_limit", errors)
    planned = _require_positive_int(data, "planned_candidate_count", errors)
    bound = _require_positive_int(data, "ldpc_cs_candidate_bound", errors)
    osd_candidates = _require_positive_int(data, "osd_candidate_count", errors)
    bp_iterations = _require_positive_int(data, "bp_iteration_count", errors)
    osd_uses = _require_positive_int(data, "osd_use_count", errors)
    gf2_solves = _require_nonnegative_int(data, "gf2_solve_count", errors)
    gf2_eliminations = _require_nonnegative_int(
        data, "gf2_full_elimination_count", errors
    )
    decode_calls = _require_positive_int(data, "decode_call_count", errors)
    z_calls = _require_nonnegative_int(data, "z_decode_call_count", errors)
    x_calls = _require_nonnegative_int(data, "x_decode_call_count", errors)
    for field in ("decode_seconds", "bp_seconds", "osd_seconds"):
        _as_seconds(data, field, errors)
    if case_id is not None and case_id != "bb90-p006-c10-seed12345-order7-hard-syndrome":
        errors.append(
            "case_id must be bb90-p006-c10-seed12345-order7-hard-syndrome"
        )
    if basis is not None and basis != "Z":
        errors.append("basis must be Z")
    if osd_planner is not None and osd_planner != "ldpc_osd_cs":
        errors.append("osd_planner must be ldpc_osd_cs")
    if osd_order is not None and osd_order != 7:
        errors.append("osd_order must be 7")
    if candidate_limit is not None and candidate_limit != 16:
        errors.append(f"candidate_limit must be 16, got {candidate_limit}")
    if bound is not None and planned is not None and bound != planned:
        errors.append("ldpc_cs_candidate_bound must match planned_candidate_count")
    if (
        osd_candidates is not None
        and candidate_limit is not None
        and planned is not None
        and osd_candidates > min(candidate_limit, planned)
    ):
        errors.append(
            "osd_candidate_count exceeds candidate_limit/planned_candidate_count"
        )
    if gf2_solves is not None and gf2_solves != 1:
        errors.append(f"gf2_solve_count must be 1, got {gf2_solves}")
    if gf2_eliminations is not None and gf2_eliminations != 1:
        errors.append(
            f"gf2_full_elimination_count must be 1, got {gf2_eliminations}"
        )
    if (
        decode_calls is not None
        and z_calls is not None
        and x_calls is not None
        and decode_calls != z_calls + x_calls
    ):
        errors.append(
            "decode_call_count must equal z_decode_call_count + x_decode_call_count"
        )
    if errors:
        return _fail("hard-profile", HARD_PROFILE_PATH, "; ".join(errors))
    return _pass("hard-profile", HARD_PROFILE_PATH, "counter-bounded profile verified")


def _check_setup_run(results_dir: Path) -> CheckResult:
    data, errors = _load_json_object(results_dir, SETUP_RUN_PATH)
    if data is None:
        return _fail("setup-run-separation", SETUP_RUN_PATH, "; ".join(errors))
    _require_nonempty_string(data, "code_id", errors)
    for field in (
        "code_build_count",
        "syndrome_cycle_build_count",
        "effective_model_build_count",
        "decoder_build_count",
    ):
        value = _require_nonnegative_int(data, field, errors)
        if value is not None and value != 1:
            errors.append(f"{field} must be 1")
    num_trials = _require_positive_int(data, "num_trials", errors)
    sample_count = _require_positive_int(data, "sample_count", errors)
    decode_calls = _require_positive_int(data, "decode_call_count", errors)
    z_calls = _require_nonnegative_int(data, "z_decode_call_count", errors)
    x_calls = _require_nonnegative_int(data, "x_decode_call_count", errors)
    for field in ("setup_seconds", "sample_seconds", "decode_seconds"):
        _as_seconds(data, field, errors)
    if num_trials is not None and sample_count is not None and sample_count != num_trials:
        errors.append("sample_count must equal num_trials")
    if (
        decode_calls is not None
        and z_calls is not None
        and x_calls is not None
        and decode_calls != z_calls + x_calls
    ):
        errors.append(
            "decode_call_count must equal z_decode_call_count + x_decode_call_count"
        )
    if errors:
        return _fail("setup-run-separation", SETUP_RUN_PATH, "; ".join(errors))
    return _pass("setup-run-separation", SETUP_RUN_PATH, "setup counters verified")


def _case_from_manifest_row(
    row: dict[str, str], row_number: int, errors: list[str]
) -> CompareCase | None:
    try:
        return CompareCase(
            case_id=row["case_id"],
            code_id=row["code_id"],
            p=float(row["p"]),
            num_cycles=int(row["num_cycles"]),
            num_trials=int(row["num_trials"]),
            seed=int(row["seed"]),
            bp_method=row["bp_method"],
            max_iter=int(row["max_iter"]),
            osd_method=row["osd_method"],
            osd_order=int(row["osd_order"]),
            scaling=int(row["scaling"]),
            catalog_status=row["catalog_status"],
            catalog_note=row["catalog_note"],
        )
    except (KeyError, TypeError, ValueError) as error:
        errors.append(f"manifest row {row_number} is malformed: {error}")
        return None


def _check_catalog(results_dir: Path) -> CheckResult:
    rows, errors = _load_csv(results_dir, CATALOG_PATH, CATALOG_HEADER)
    cases: list[CompareCase] = []
    if rows is not None:
        for index, row in enumerate(rows, start=2):
            case = _case_from_manifest_row(row, index, errors)
            if case is not None:
                cases.append(case)
        if not errors:
            errors.extend(validate_small_ldpc_catalog(tuple(cases)))
    if errors:
        return _fail("catalog-coverage", CATALOG_PATH, "; ".join(errors))
    return _pass("catalog-coverage", CATALOG_PATH, "small-LDPC catalog verified")


def _check_diagnostic(results_dir: Path) -> CheckResult:
    rows, errors = _load_csv(results_dir, DIAGNOSTIC_PATH, CSV_HEADER)
    if rows is not None:
        errors.extend(verify_diagnostic_rows(rows, allow_missing_python=False))
    if errors:
        return _fail("diagnostic-compare", DIAGNOSTIC_PATH, "; ".join(errors))
    return _pass("diagnostic-compare", DIAGNOSTIC_PATH, "diagnostic rows verified")


def _check_provenance(results_dir: Path) -> CheckResult:
    path = results_dir / PROVENANCE_PATH
    if not path.exists():
        return _warn(
            "provenance", PROVENANCE_PATH, "optional provenance.json is missing"
        )
    data, errors = _load_json_object(results_dir, PROVENANCE_PATH)
    if data is None:
        return _warn("provenance", PROVENANCE_PATH, "; ".join(errors))
    fields = ("artifact_hash", "command", "timestamp")
    recognized = [f"{field}={data[field]}" for field in fields if data.get(field)]
    if len(recognized) == len(fields):
        return _pass("provenance", PROVENANCE_PATH, ", ".join(recognized))
    missing = [field for field in fields if not data.get(field)]
    if recognized:
        return _warn(
            "provenance",
            PROVENANCE_PATH,
            "incomplete provenance: missing " + ", ".join(missing),
        )
    return _warn(
        "provenance",
        PROVENANCE_PATH,
        "incomplete provenance: no recognized provenance fields; missing "
        + ", ".join(missing),
    )


def check_results_dir(results_dir: Path) -> list[CheckResult]:
    return [
        _check_semantic_replay(results_dir),
        _check_hard_profile(results_dir),
        _check_setup_run(results_dir),
        _check_catalog(results_dir),
        _check_diagnostic(results_dir),
        _check_provenance(results_dir),
    ]


def readiness_verdict(results: Sequence[CheckResult]) -> str:
    if any(result.status == FAIL for result in results):
        return FAIL
    if any(result.status == WARN for result in results):
        return WARN
    return PASS


def _print_summary(results: Sequence[CheckResult], verdict: str) -> None:
    for result in results:
        detail = "; ".join(result.messages)
        print(f"{result.status} {result.name}: {result.artifact} - {detail}")
    if verdict == FAIL:
        print("FAIL readiness verdict: required prerequisites failed")
    elif verdict == WARN:
        print("WARN readiness verdict: all required prerequisites passed with warnings")
    else:
        print("PASS readiness verdict: all prerequisites passed")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    results = check_results_dir(args.results_dir)
    verdict = readiness_verdict(results)
    _print_summary(results, verdict)
    return 1 if verdict == FAIL else 0


if __name__ == "__main__":
    raise SystemExit(main())
