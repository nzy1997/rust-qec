from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
from typing import Sequence

CSV_HEADER = [
    "case_id",
    "runner",
    "decoder_impl",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
    "error",
]

CATALOG_HEADER = [
    "case_id",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
    "scaling",
    "catalog_status",
    "catalog_note",
]

SMALL_LDPC_TRIALS = 50_000
SMALL_LDPC_SEED = 12345
SMALL_LDPC_BP_METHOD = "ms"
SMALL_LDPC_MAX_ITER = 10000
SMALL_LDPC_OSD_METHOD = "osd_cs"
SMALL_LDPC_OSD_ORDER = 7
SMALL_LDPC_SCALING = 0
SUPPORTED_RUST_CONSTRUCTORS = {"bb72", "bb90", "bb144"}
UNSUPPORTED_RUST_CONSTRUCTOR_STATUS = "unsupported_rust_constructor"
UNSUPPORTED_RUST_CONSTRUCTOR_NOTE = (
    "Rust constructor support is not available in this branch."
)


@dataclass(frozen=True)
class CompareCase:
    case_id: str
    code_id: str
    p: float
    num_cycles: int
    num_trials: int
    seed: int = 12345
    bp_method: str = "ms"
    max_iter: int = 10000
    osd_method: str = "osd_cs"
    osd_order: int = 7
    scaling: int = 0
    catalog_status: str = "supported"
    catalog_note: str = ""


def _decimal(value: float) -> Decimal:
    return Decimal(str(value)).quantize(Decimal("0.0001"))


def _format_p_value(value: float) -> str:
    return format(_decimal(value).normalize(), "f")


def _format_p_label(value: float) -> str:
    return "p" + format(_decimal(value), "f").removeprefix("0.").replace(".", "")


def format_case_id(case: CompareCase) -> str:
    return (
        f"{case.code_id}-{_format_p_label(case.p)}-c{case.num_cycles}"
        f"-t{case.num_trials}-seed{case.seed}"
    )


def _small_ldpc_case(code_id: str, p: float, cycles: int) -> CompareCase:
    status = "supported"
    note = ""
    if code_id not in SUPPORTED_RUST_CONSTRUCTORS:
        status = UNSUPPORTED_RUST_CONSTRUCTOR_STATUS
        note = UNSUPPORTED_RUST_CONSTRUCTOR_NOTE
    case = CompareCase(
        case_id="",
        code_id=code_id,
        p=p,
        num_cycles=cycles,
        num_trials=SMALL_LDPC_TRIALS,
        seed=SMALL_LDPC_SEED,
        bp_method=SMALL_LDPC_BP_METHOD,
        max_iter=SMALL_LDPC_MAX_ITER,
        osd_method=SMALL_LDPC_OSD_METHOD,
        osd_order=SMALL_LDPC_OSD_ORDER,
        scaling=SMALL_LDPC_SCALING,
        catalog_status=status,
        catalog_note=note,
    )
    return CompareCase(
        case_id=format_case_id(case),
        code_id=code_id,
        p=p,
        num_cycles=cycles,
        num_trials=SMALL_LDPC_TRIALS,
        seed=SMALL_LDPC_SEED,
        bp_method=SMALL_LDPC_BP_METHOD,
        max_iter=SMALL_LDPC_MAX_ITER,
        osd_method=SMALL_LDPC_OSD_METHOD,
        osd_order=SMALL_LDPC_OSD_ORDER,
        scaling=SMALL_LDPC_SCALING,
        catalog_status=status,
        catalog_note=note,
    )


SMALL_LDPC_TARGETS = {
    "bb72": (6, (0.0002, 0.0005, 0.001, 0.002, 0.003, 0.004, 0.005)),
    "bb90": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb108": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb144": (12, (0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb288": (18, (0.0035, 0.004, 0.005, 0.006)),
}

SMALL_LDPC_CASES = tuple(
    _small_ldpc_case(code_id, p, cycles)
    for code_id, (cycles, p_values) in SMALL_LDPC_TARGETS.items()
    for p in p_values
)

SMOKE_CASES = (
    CompareCase("bb72-p0005-c1-t1-seed12345", "bb72", 0.0005, 1, 1),
    CompareCase("bb90-p0005-c1-t1-seed12345", "bb90", 0.0005, 1, 1),
)


def _target_key(case: CompareCase) -> tuple[str, str, int]:
    return (case.code_id, _format_p_value(case.p), case.num_cycles)


def _target_label(key: tuple[str, str, int]) -> str:
    code_id, p_value, cycles = key
    return f"{code_id} p={p_value} cycles={cycles}"


def small_ldpc_manifest_rows(
    cases: Sequence[CompareCase] = SMALL_LDPC_CASES,
) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for case in cases:
        rows.append(
            {
                "case_id": case.case_id,
                "code_id": case.code_id,
                "p": _format_p_value(case.p),
                "num_cycles": str(case.num_cycles),
                "num_trials": str(case.num_trials),
                "seed": str(case.seed),
                "bp_method": case.bp_method,
                "max_iter": str(case.max_iter),
                "osd_method": case.osd_method,
                "osd_order": str(case.osd_order),
                "scaling": str(case.scaling),
                "catalog_status": case.catalog_status,
                "catalog_note": case.catalog_note,
            }
        )
    return rows


def validate_small_ldpc_catalog(
    cases: Sequence[CompareCase] = SMALL_LDPC_CASES,
) -> list[str]:
    errors: list[str] = []
    expected_keys = {
        (code_id, _format_p_value(p), cycles)
        for code_id, (cycles, p_values) in SMALL_LDPC_TARGETS.items()
        for p in p_values
    }
    actual_keys = {_target_key(case) for case in cases}

    if len(cases) != 31:
        errors.append(
            f"small-LDPC catalog must contain exactly 31 cases, got {len(cases)}"
        )

    for key in sorted(expected_keys - actual_keys):
        errors.append(f"missing target: {_target_label(key)}")
    for key in sorted(actual_keys - expected_keys):
        errors.append(f"unexpected target: {_target_label(key)}")

    seen_case_ids: set[str] = set()
    for case in cases:
        if case.case_id in seen_case_ids:
            errors.append(f"duplicate case_id: {case.case_id}")
        seen_case_ids.add(case.case_id)

        expected_case_id = format_case_id(case)
        if case.case_id != expected_case_id:
            errors.append(
                "case_id mismatch for "
                f"{_target_label(_target_key(case))}: expected {expected_case_id}, "
                f"got {case.case_id}"
            )

        if case.num_trials != SMALL_LDPC_TRIALS:
            errors.append(
                "trial budget mismatch for "
                f"{_target_label(_target_key(case))}: expected {SMALL_LDPC_TRIALS}, "
                f"got {case.num_trials}"
            )
        if case.seed != SMALL_LDPC_SEED:
            errors.append(
                "seed mismatch for "
                f"{_target_label(_target_key(case))}: expected {SMALL_LDPC_SEED}, "
                f"got {case.seed}"
            )
        if case.bp_method != SMALL_LDPC_BP_METHOD:
            errors.append(
                "BP method mismatch for "
                f"{_target_label(_target_key(case))}: expected {SMALL_LDPC_BP_METHOD}, "
                f"got {case.bp_method}"
            )
        if case.max_iter != SMALL_LDPC_MAX_ITER:
            errors.append(
                "max_iter mismatch for "
                f"{_target_label(_target_key(case))}: expected {SMALL_LDPC_MAX_ITER}, "
                f"got {case.max_iter}"
            )
        if case.osd_method not in {"ldpc_cs", SMALL_LDPC_OSD_METHOD}:
            errors.append(
                "OSD method mismatch for "
                f"{_target_label(_target_key(case))}: expected ldpc_cs or "
                f"{SMALL_LDPC_OSD_METHOD}, got {case.osd_method}"
            )
        if case.osd_order != SMALL_LDPC_OSD_ORDER:
            errors.append(
                "OSD order mismatch for "
                f"{_target_label(_target_key(case))}: expected {SMALL_LDPC_OSD_ORDER}, "
                f"got {case.osd_order}"
            )
        if case.scaling != SMALL_LDPC_SCALING:
            errors.append(
                "scaling mismatch for "
                f"{_target_label(_target_key(case))}: expected {SMALL_LDPC_SCALING}, "
                f"got {case.scaling}"
            )

        if case.code_id in SUPPORTED_RUST_CONSTRUCTORS:
            if case.catalog_status != "supported":
                errors.append(
                    "supported target has wrong catalog status for "
                    f"{_target_label(_target_key(case))}: {case.catalog_status}"
                )
        elif case.catalog_status != UNSUPPORTED_RUST_CONSTRUCTOR_STATUS:
            errors.append(
                "unsupported target has wrong catalog status for "
                f"{_target_label(_target_key(case))}: {case.catalog_status}"
            )
        elif "Rust constructor" not in case.catalog_note:
            errors.append(
                "unsupported target note is missing constructor explanation for "
                f"{_target_label(_target_key(case))}: {case.catalog_note}"
            )

    return errors
