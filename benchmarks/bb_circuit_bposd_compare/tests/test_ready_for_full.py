import csv
import json
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import ready_for_full
from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    DIAGNOSTIC_CASES,
    small_ldpc_manifest_rows,
)


HARD_CASE_ID = "bb90-p006-c10-seed12345-order7-hard-syndrome"
HARD_PREDICTION = "[false,true,false,true,false,false,false,true]"
HARD_SUPPORT = (
    "[5,8,14,18,22,23,25,26,40,50,64,69,71,72,74,83,89,93,99,101,113,116,122,"
    "130,148,156,158,179,186,192,193,194,201,216,224,228,232,236,237,239,242,"
    "246,247,249,252,253,254,257,260,261,269,274,278,279,280,293,295,299,304,"
    "310,316,324,340,345,347,355,366,367,378,385,386,390,392,401,413,414,429,"
    "430,435,439,444,446]"
)


def _write_csv(path: Path, fieldnames: list[str], rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field, "") for field in fieldnames})


def _hard_replay_row(decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": HARD_CASE_ID,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": "bb90",
        "p": "0.006",
        "num_cycles": "10",
        "num_trials": "1",
        "seed": "12345",
        "bp_method": "ms",
        "max_iter": "10000",
        "osd_method": "osd_cs",
        "osd_order": "7",
        "basis": "Z",
        "syndrome_weight": "82",
        "syndrome_support": HARD_SUPPORT,
        "logical_prediction": HARD_PREDICTION,
        "expected_logical": HARD_PREDICTION,
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "1" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "10000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "16" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "1" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


def _diagnostic_row(case, decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": case.case_id,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": case.code_id,
        "p": str(case.p),
        "num_cycles": str(case.num_cycles),
        "num_trials": str(case.num_trials),
        "seed": str(case.seed),
        "bp_method": case.bp_method,
        "max_iter": str(case.max_iter),
        "osd_method": case.osd_method,
        "osd_order": str(case.osd_order),
        "basis": "",
        "syndrome_weight": "",
        "syndrome_support": "",
        "logical_prediction": "",
        "expected_logical": "",
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "2" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "20000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "16" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "1" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


def _write_json(path: Path, data: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, sort_keys=True))


def _hard_profile_fields() -> dict[str, object]:
    return {
        "case_id": HARD_CASE_ID,
        "basis": "Z",
        "osd_planner": "ldpc_osd_cs",
        "osd_order": 7,
        "candidate_limit": 16,
        "planned_candidate_count": 4100,
        "ldpc_cs_candidate_bound": 4100,
        "decode_seconds": 0.3,
        "bp_seconds": 0.2,
        "osd_seconds": 0.1,
        "decode_call_count": 1,
        "z_decode_call_count": 1,
        "x_decode_call_count": 0,
        "bp_iteration_count": 10000,
        "osd_use_count": 1,
        "osd_candidate_count": 16,
        "gf2_solve_count": 1,
        "gf2_full_elimination_count": 1,
    }


def _hard_profile() -> dict[str, object]:
    return _hard_profile_fields()


def _setup_profile_fields() -> dict[str, object]:
    return {
        "code_id": "bb72",
        "num_trials": 8,
        "setup_seconds": 0.1,
        "sample_seconds": 0.2,
        "decode_seconds": 0.3,
        "code_build_count": 1,
        "syndrome_cycle_build_count": 1,
        "effective_model_build_count": 1,
        "decoder_build_count": 1,
        "sample_count": 8,
        "decode_call_count": 16,
        "z_decode_call_count": 8,
        "x_decode_call_count": 8,
    }


def _setup_profile() -> dict[str, object]:
    return _setup_profile_fields()


def write_ready_tree(results_dir: Path, *, provenance: bool = True) -> None:
    _write_csv(
        results_dir / "hard-replay" / "results.csv",
        CSV_HEADER,
        [_hard_replay_row("rbposd"), _hard_replay_row("ldpc_bposd")],
    )
    _write_json(results_dir / "hard-profile" / "profile.json", _hard_profile())
    _write_json(results_dir / "setup-run" / "profile.json", _setup_profile())
    _write_csv(
        results_dir / "small-ldpc-catalog" / "manifest.csv",
        CATALOG_HEADER,
        small_ldpc_manifest_rows(),
    )
    diagnostic_rows = []
    for case in DIAGNOSTIC_CASES:
        diagnostic_rows.append(_diagnostic_row(case, "rbposd"))
        diagnostic_rows.append(_diagnostic_row(case, "ldpc_bposd"))
    _write_csv(results_dir / "diagnostic" / "results.csv", CSV_HEADER, diagnostic_rows)
    if provenance:
        _write_json(
            results_dir / "provenance.json",
            {
                "artifact_hash": "sha256:example",
                "command": "agent desk test fixture",
                "timestamp": "2026-06-27T00:00:00+08:00",
            },
        )


def test_ready_for_full_passes_complete_artifact_tree(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "PASS semantic-replay" in output
    assert "PASS hard-profile" in output
    assert "PASS setup-run-separation" in output
    assert "PASS catalog-coverage" in output
    assert "PASS diagnostic-compare" in output
    assert "PASS readiness verdict" in output


def test_ready_for_full_fails_missing_hard_replay(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    (tmp_path / "hard-replay" / "results.csv").unlink()

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL semantic-replay" in output
    assert "hard-replay/results.csv" in output
    assert "FAIL readiness verdict" in output


def test_ready_for_full_fails_without_setup_run_artifact(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    (tmp_path / "setup-run" / "profile.json").unlink()

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL setup-run-separation" in output
    assert "setup-run/profile.json" in output


def test_ready_for_full_fails_missing_hard_profile_field(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    hard_profile = _hard_profile_fields()
    hard_profile.pop("bp_iteration_count")
    _write_json(tmp_path / "hard-profile" / "profile.json", hard_profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "bp_iteration_count" in output


def test_ready_for_full_fails_missing_setup_run_code_id(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    setup_profile = _setup_profile_fields()
    setup_profile.pop("code_id")
    _write_json(tmp_path / "setup-run" / "profile.json", setup_profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL setup-run-separation" in output
    assert "code_id" in output
