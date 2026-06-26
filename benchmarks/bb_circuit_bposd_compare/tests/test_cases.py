from dataclasses import replace

from benchmarks.bb_circuit_bposd_compare import run_compare
from benchmarks.bb_circuit_bposd_compare.cases import (
    SMALL_LDPC_CASES,
    format_case_id,
    validate_small_ldpc_catalog,
)


EXPECTED_SWEEPS = {
    "bb72": (6, (0.0002, 0.0005, 0.001, 0.002, 0.003, 0.004, 0.005)),
    "bb90": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb108": (10, (0.0005, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb144": (12, (0.001, 0.002, 0.003, 0.004, 0.005, 0.006)),
    "bb288": (18, (0.0035, 0.004, 0.005, 0.006)),
}


def _cases_for(code_id: str):
    return [case for case in SMALL_LDPC_CASES if case.code_id == code_id]


def test_small_ldpc_catalog_has_complete_issue_209_targets() -> None:
    assert len(SMALL_LDPC_CASES) == 31
    assert validate_small_ldpc_catalog(SMALL_LDPC_CASES) == []

    for code_id, (cycles, p_values) in EXPECTED_SWEEPS.items():
        cases = _cases_for(code_id)
        assert len(cases) == len(p_values)
        assert tuple(case.p for case in cases) == p_values
        assert {case.num_cycles for case in cases} == {cycles}


def test_small_ldpc_catalog_case_ids_include_identity_fields() -> None:
    for case in SMALL_LDPC_CASES:
        assert case.case_id == format_case_id(case)
        assert case.code_id in case.case_id
        assert f"c{case.num_cycles}" in case.case_id
        assert f"t{case.num_trials}" in case.case_id
        assert f"seed{case.seed}" in case.case_id


def test_small_ldpc_catalog_decoder_settings_are_pinned() -> None:
    for case in SMALL_LDPC_CASES:
        assert case.num_trials == 50000
        assert case.seed == 12345
        assert case.bp_method == "ms"
        assert case.max_iter == 10000
        assert case.osd_method in {"ldpc_cs", "osd_cs"}
        assert case.osd_order == 7
        assert case.scaling == 0


def test_small_ldpc_catalog_marks_unsupported_rust_constructors() -> None:
    unsupported = {
        case.code_id for case in SMALL_LDPC_CASES if case.catalog_status != "supported"
    }
    assert unsupported == {"bb108", "bb288"}
    for case in SMALL_LDPC_CASES:
        if case.code_id in unsupported:
            assert case.catalog_status == "unsupported_rust_constructor"
            assert "Rust constructor" in case.catalog_note


def test_small_ldpc_catalog_negative_control_names_missing_bb108() -> None:
    copied = tuple(case for case in SMALL_LDPC_CASES if case.code_id != "bb108")
    errors = "\n".join(validate_small_ldpc_catalog(copied))
    assert "missing target: bb108 p=0.0005 cycles=10" in errors


def test_small_ldpc_catalog_negative_control_names_wrong_bb288_p_value() -> None:
    copied = list(SMALL_LDPC_CASES)
    index = next(i for i, case in enumerate(copied) if case.code_id == "bb288")
    copied[index] = replace(copied[index], p=0.0036)

    errors = "\n".join(validate_small_ldpc_catalog(tuple(copied)))
    assert "missing target: bb288 p=0.0035 cycles=18" in errors
    assert "unexpected target: bb288 p=0.0036 cycles=18" in errors


def test_small_ldpc_catalog_dry_run_surfaces_errors_and_skips_decoders(
    monkeypatch, tmp_path, capsys
) -> None:
    def _fail(*_args, **_kwargs):
        raise AssertionError("decoder path should not be reached")

    monkeypatch.setattr(
        run_compare,
        "validate_small_ldpc_catalog",
        lambda _cases: ["missing target: bb108 p=0.0005 cycles=10"],
    )
    monkeypatch.setattr(
        run_compare,
        "small_ldpc_manifest_rows",
        lambda _cases: [
            {
                "case_id": "bb72-p0002-c6-t50000-seed12345",
                "code_id": "bb72",
                "p": "0.0002",
                "num_cycles": "6",
                "num_trials": "50000",
                "seed": "12345",
                "bp_method": "ms",
                "max_iter": "10000",
                "osd_method": "osd_cs",
                "osd_order": "7",
                "scaling": "0",
                "catalog_status": "supported",
                "catalog_note": "",
            }
        ],
    )
    monkeypatch.setattr(run_compare, "run_suite", _fail)
    monkeypatch.setattr(run_compare, "_run_rust_export", _fail)
    monkeypatch.setattr(run_compare, "_python_row", _fail)

    status = run_compare.main(
        ["--tier", "small_ldpc_catalog", "--output-dir", str(tmp_path)]
    )

    captured = capsys.readouterr()
    manifest_path = tmp_path / "manifest.csv"
    assert status == 1
    assert "missing target: bb108 p=0.0005 cycles=10" in captured.err
    assert manifest_path.exists()
    assert "case_id,code_id,p,num_cycles" in manifest_path.read_text()
