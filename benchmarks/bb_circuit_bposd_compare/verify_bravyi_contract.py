from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Sequence

from benchmarks.bb_circuit_bposd_compare.cases import (
    BB72_BB144_FULL_CASES,
    BB72_BB144_PLOT_SMOKE_CASES,
    DIAGNOSTIC_BP_METHOD,
    DIAGNOSTIC_CASES,
    DIAGNOSTIC_MAX_ITER,
    DIAGNOSTIC_OSD_METHOD,
    DIAGNOSTIC_OSD_ORDER,
    HARD_REPLAY_CASES,
    SMALL_LDPC_BP_METHOD,
    SMALL_LDPC_CASES,
    SMALL_LDPC_MAX_ITER,
    SMALL_LDPC_OSD_METHOD,
    SMALL_LDPC_OSD_ORDER,
    SMALL_LDPC_SCALING,
    SMOKE_CASES,
    CompareCase,
    small_ldpc_manifest_rows,
)
from benchmarks.bb_circuit_bposd_compare.run_compare import (
    PYTHON_FAILURE_PREDICATE,
    PYTHON_FAILURE_UNIT,
    PYTHON_UPSTREAM_BP_METHOD,
    PYTHON_UPSTREAM_MAX_ITER,
    PYTHON_UPSTREAM_MS_SCALING_FACTOR,
    PYTHON_UPSTREAM_OSD_METHOD,
    PYTHON_UPSTREAM_OSD_ORDER,
    _python_bposd_decoder_kwargs,
)

UPSTREAM_COMMIT = "fa77e3333d3ec44c79d8f914dd24c040d1da471b"
EXPECTED_RESULT_COLUMNS = [
    "physical_error_rate",
    "num_syndrome_cycles",
    "num_monte_carlo_trials",
    "num_failed_trials",
]
RUST_BB_CIRCUIT_MEMORY_PATH = (
    Path(__file__).resolve().parents[2] / "rsinter" / "src" / "bb_circuit_memory.rs"
)
RUST_TAIL_CYCLES_CONST = "BRAVYI_NOISELESS_TAIL_CYCLES"
EXPECTED_SOURCES = [
    {
        "file": "README.md",
        "lines": "16-21",
        "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/"
        f"{UPSTREAM_COMMIT}/README.md#L16-L21",
        "supports": ["result_row", "failure_unit"],
    },
    {
        "file": "decoder_setup.py",
        "lines": "511-618",
        "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/"
        f"{UPSTREAM_COMMIT}/decoder_setup.py#L511-L618",
        "supports": ["noiseless_tail_cycles", "effective_decoder_histories"],
    },
    {
        "file": "decoder_run.py",
        "lines": "67-72,329-349",
        "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/"
        f"{UPSTREAM_COMMIT}/decoder_run.py#L67-L72",
        "supports": ["decoder"],
    },
    {
        "file": "decoder_run.py",
        "lines": "364-415",
        "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/"
        f"{UPSTREAM_COMMIT}/decoder_run.py#L364-L415",
        "supports": ["failure_predicate"],
    },
]


def _load_contract(path: Path) -> dict[str, object]:
    parsed = json.loads(path.read_text())
    if not isinstance(parsed, dict):
        raise ValueError("contract root must be a JSON object")
    return parsed


def validate_contract(contract: dict[str, object]) -> list[str]:
    errors: list[str] = []
    _expect(contract, ("contract_version",), 1, errors)
    _expect(
        contract,
        ("upstream", "repository"),
        "sbravyi/BivariateBicycleCodes",
        errors,
    )
    _expect(contract, ("upstream", "commit"), UPSTREAM_COMMIT, errors)
    _expect(
        contract,
        ("upstream", "tree_url"),
        "https://github.com/sbravyi/BivariateBicycleCodes/tree/"
        f"{UPSTREAM_COMMIT}",
        errors,
    )
    _expect(contract, ("result_row", "columns"), EXPECTED_RESULT_COLUMNS, errors)
    _expect(contract, ("result_row", "failure_unit"), PYTHON_FAILURE_UNIT, errors)
    _expect(contract, ("decoder", "bp_method"), PYTHON_UPSTREAM_BP_METHOD, errors)
    _expect(contract, ("decoder", "max_iter"), PYTHON_UPSTREAM_MAX_ITER, errors)
    _expect(contract, ("decoder", "osd_method"), PYTHON_UPSTREAM_OSD_METHOD, errors)
    _expect(contract, ("decoder", "osd_order"), PYTHON_UPSTREAM_OSD_ORDER, errors)
    _expect(
        contract,
        ("decoder", "ms_scaling_factor"),
        PYTHON_UPSTREAM_MS_SCALING_FACTOR,
        errors,
    )
    _expect(
        contract,
        ("cycle_convention", "configured_noisy_cycles_field"),
        "num_cycles",
        errors,
    )
    _expect(contract, ("cycle_convention", "noiseless_tail_cycles"), 2, errors)
    _expect(contract, ("failure_predicate", "decode_order"), ["Z", "X"], errors)
    _expect(
        contract,
        ("failure_predicate", "x_decode_condition"),
        "only_if_z_succeeds",
        errors,
    )
    _expect(
        contract,
        ("failure_predicate", "failed_trial_condition"),
        "z_fails_or_x_fails_after_z_succeeds",
        errors,
    )
    _expect(contract, ("sources",), EXPECTED_SOURCES, errors)
    _validate_case_constants(contract, errors)
    _validate_all_compare_cases(contract, errors)
    _validate_manifest_scaling(contract, errors)
    _validate_python_decoder_kwargs(contract, errors)
    _validate_failure_contract(errors)
    _validate_rust_tail_cycle_source(contract, errors)
    return errors


def _validate_case_constants(
    contract: dict[str, object],
    errors: list[str],
) -> None:
    _expect(contract, ("decoder", "bp_method"), SMALL_LDPC_BP_METHOD, errors)
    _expect(contract, ("decoder", "max_iter"), SMALL_LDPC_MAX_ITER, errors)
    _expect(contract, ("decoder", "osd_method"), SMALL_LDPC_OSD_METHOD, errors)
    _expect(contract, ("decoder", "osd_order"), SMALL_LDPC_OSD_ORDER, errors)
    _expect(contract, ("decoder", "ms_scaling_factor"), SMALL_LDPC_SCALING, errors)
    _expect(contract, ("decoder", "bp_method"), DIAGNOSTIC_BP_METHOD, errors)
    _expect(contract, ("decoder", "max_iter"), DIAGNOSTIC_MAX_ITER, errors)
    _expect(contract, ("decoder", "osd_method"), DIAGNOSTIC_OSD_METHOD, errors)
    _expect(contract, ("decoder", "osd_order"), DIAGNOSTIC_OSD_ORDER, errors)


def _validate_all_compare_cases(
    contract: dict[str, object],
    errors: list[str],
) -> None:
    decoder = _as_mapping(_get(contract, ("decoder",)))
    if decoder is None:
        return
    expected = {
        "bp_method": decoder.get("bp_method"),
        "max_iter": decoder.get("max_iter"),
        "osd_method": decoder.get("osd_method"),
        "osd_order": decoder.get("osd_order"),
        "scaling": decoder.get("ms_scaling_factor"),
    }
    groups: tuple[tuple[str, Sequence[CompareCase]], ...] = (
        ("SMALL_LDPC_CASES", SMALL_LDPC_CASES),
        ("BB72_BB144_PLOT_SMOKE_CASES", BB72_BB144_PLOT_SMOKE_CASES),
        ("BB72_BB144_FULL_CASES", BB72_BB144_FULL_CASES),
        ("DIAGNOSTIC_CASES", DIAGNOSTIC_CASES),
        ("SMOKE_CASES", SMOKE_CASES),
        ("HARD_REPLAY_CASES", HARD_REPLAY_CASES),
    )
    for group_name, cases in groups:
        for case in cases:
            for field, expected_value in expected.items():
                actual = getattr(case, field)
                if actual != expected_value:
                    errors.append(
                        f"{group_name}.{case.case_id}.{field}: "
                        f"expected {expected_value!r}, got {actual!r}"
                    )


def _validate_manifest_scaling(
    contract: dict[str, object],
    errors: list[str],
) -> None:
    expected = _get(contract, ("decoder", "ms_scaling_factor"))
    for row in small_ldpc_manifest_rows():
        if row.get("scaling") != str(expected):
            errors.append(
                "small_ldpc_manifest_rows.scaling: expected "
                f"{expected!r}, got {row.get('scaling')!r} for {row.get('case_id')}"
            )


def _validate_python_decoder_kwargs(
    contract: dict[str, object],
    errors: list[str],
) -> None:
    decoder = _as_mapping(_get(contract, ("decoder",)))
    if decoder is None:
        return
    kwargs = _python_bposd_decoder_kwargs()
    expected = {
        "bp_method": decoder.get("bp_method"),
        "max_iter": decoder.get("max_iter"),
        "osd_method": decoder.get("osd_method"),
        "osd_order": decoder.get("osd_order"),
        "ms_scaling_factor": decoder.get("ms_scaling_factor"),
        "input_vector_type": "syndrome",
    }
    for field, expected_value in expected.items():
        if kwargs.get(field) != expected_value:
            errors.append(
                f"_python_bposd_decoder_kwargs.{field}: "
                f"expected {expected_value!r}, got {kwargs.get(field)!r}"
            )


def _validate_failure_contract(errors: list[str]) -> None:
    if PYTHON_FAILURE_UNIT != "monte_carlo_trial":
        errors.append(
            "PYTHON_FAILURE_UNIT: expected 'monte_carlo_trial', "
            f"got {PYTHON_FAILURE_UNIT!r}"
        )
    if PYTHON_FAILURE_PREDICATE != "z_first_x_only_if_z_succeeds":
        errors.append(
            "PYTHON_FAILURE_PREDICATE: expected "
            "'z_first_x_only_if_z_succeeds', "
            f"got {PYTHON_FAILURE_PREDICATE!r}"
        )


def _validate_rust_tail_cycle_source(
    contract: dict[str, object],
    errors: list[str],
) -> None:
    expected = _get(contract, ("cycle_convention", "noiseless_tail_cycles"))
    try:
        source = RUST_BB_CIRCUIT_MEMORY_PATH.read_text()
    except OSError as error:
        errors.append(f"rsinter.bb_circuit_memory: failed to read source: {error}")
        return

    expected_declaration = f"pub const {RUST_TAIL_CYCLES_CONST}: usize = {expected};"
    if expected_declaration not in source:
        errors.append(
            "rsinter.bb_circuit_memory."
            f"{RUST_TAIL_CYCLES_CONST}: expected declaration "
            f"{expected_declaration!r}"
        )

    expected_usages = {
        "effective_model_total_cycles": (
            "config.num_cycles + " f"{RUST_TAIL_CYCLES_CONST}"
        ),
        "trial_total_cycles": "num_cycles + " f"{RUST_TAIL_CYCLES_CONST}",
    }
    for usage_name, snippet in expected_usages.items():
        if snippet not in source:
            errors.append(
                f"rsinter.bb_circuit_memory.{usage_name}: "
                f"expected source usage {snippet!r}"
            )


def _expect(
    contract: dict[str, object],
    path: tuple[str, ...],
    expected: object,
    errors: list[str],
) -> None:
    actual = _get(contract, path)
    if actual != expected:
        errors.append(
            f"{'.'.join(path)}: expected {expected!r}, got {actual!r}"
        )


def _get(contract: dict[str, object], path: tuple[str, ...]) -> object:
    current: object = contract
    for key in path:
        if not isinstance(current, dict) or key not in current:
            return None
        current = current[key]
    return current


def _as_mapping(value: object) -> dict[str, object] | None:
    return value if isinstance(value, dict) else None


def _pass_line(contract: dict[str, object]) -> str:
    commit = _get(contract, ("upstream", "commit"))
    osd_method = _get(contract, ("decoder", "osd_method"))
    osd_order = _get(contract, ("decoder", "osd_order"))
    scaling = _get(contract, ("decoder", "ms_scaling_factor"))
    failure_unit = _get(contract, ("result_row", "failure_unit"))
    return (
        "PASS Bravyi BB contract "
        f"{commit} {osd_method} OSD order {osd_order} "
        f"ms_scaling_factor={scaling} two noiseless tail cycles "
        f"failure_unit={failure_unit}"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("contract_path", type=Path)
    args = parser.parse_args(argv)

    contract = _load_contract(args.contract_path)
    errors = validate_contract(contract)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(_pass_line(contract))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
