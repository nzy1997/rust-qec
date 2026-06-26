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


def test_ready_for_full_verification_command_accepts_named_tmp_fixture(
    tmp_path, capsys
) -> None:
    fixture_dir = tmp_path / "rstim-bb-ready"
    write_ready_tree(fixture_dir)

    status = ready_for_full.main(["--results-dir", str(fixture_dir)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
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


def test_ready_for_full_fails_malformed_hard_profile_json(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    (tmp_path / "hard-profile" / "profile.json").write_text("{not-json")

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "malformed JSON" in output
    assert "hard-profile/profile.json" in output


def test_ready_for_full_fails_stale_hard_profile_basis(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    hard_profile = _hard_profile_fields()
    hard_profile["basis"] = "X"
    _write_json(tmp_path / "hard-profile" / "profile.json", hard_profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "basis" in output


def test_ready_for_full_fails_stale_setup_run_sample_count(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    setup_profile = _setup_profile_fields()
    setup_profile["sample_count"] = 7
    _write_json(tmp_path / "setup-run" / "profile.json", setup_profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL setup-run-separation" in output
    assert "sample_count" in output


def test_ready_for_full_fails_stale_catalog_manifest(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    manifest_path = tmp_path / "small-ldpc-catalog" / "manifest.csv"
    rows = small_ldpc_manifest_rows()
    rows[0] = {**rows[0], "p": "0.0099"}
    _write_csv(manifest_path, CATALOG_HEADER, rows)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL catalog-coverage" in output
    assert "small-ldpc-catalog/manifest.csv" in output
    assert "unexpected target" in output


def test_ready_for_full_fails_malformed_catalog_csv(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    manifest_path = tmp_path / "small-ldpc-catalog" / "manifest.csv"
    manifest_path.write_text("case_id,code_id,p\nonly,three,columns\n")

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL catalog-coverage" in output
    assert "missing required CSV column(s)" in output
    assert "small-ldpc-catalog/manifest.csv" in output


def test_ready_for_full_fails_hard_profile_counter_regression(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path)
    profile = _hard_profile()
    profile["gf2_solve_count"] = 4101
    _write_json(tmp_path / "hard-profile" / "profile.json", profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "gf2_solve_count" in output
    assert "hard-profile/profile.json" in output


def test_ready_for_full_fails_negative_hard_profile_decode_counters(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path)
    profile = _hard_profile_fields()
    profile["decode_call_count"] = -1
    profile["z_decode_call_count"] = -1
    profile["x_decode_call_count"] = 0
    _write_json(tmp_path / "hard-profile" / "profile.json", profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "decode_call_count" in output
    assert "z_decode_call_count" in output


def test_ready_for_full_fails_zero_hard_profile_decode_count(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path)
    profile = _hard_profile_fields()
    profile["decode_call_count"] = 0
    profile["z_decode_call_count"] = 0
    profile["x_decode_call_count"] = 0
    _write_json(tmp_path / "hard-profile" / "profile.json", profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "decode_call_count" in output


def test_ready_for_full_fails_negative_setup_run_counters(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    profile = _setup_profile_fields()
    profile["num_trials"] = -8
    profile["sample_count"] = -8
    profile["decode_call_count"] = -16
    profile["z_decode_call_count"] = -8
    profile["x_decode_call_count"] = -8
    _write_json(tmp_path / "setup-run" / "profile.json", profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL setup-run-separation" in output
    assert "num_trials" in output
    assert "sample_count" in output
    assert "decode_call_count" in output
    assert "z_decode_call_count" in output
    assert "x_decode_call_count" in output


def test_ready_for_full_fails_zero_setup_run_evidence_counters(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path)
    profile = _setup_profile_fields()
    profile["num_trials"] = 0
    profile["sample_count"] = 0
    profile["decode_call_count"] = 0
    profile["z_decode_call_count"] = 0
    profile["x_decode_call_count"] = 0
    _write_json(tmp_path / "setup-run" / "profile.json", profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL setup-run-separation" in output
    assert "num_trials" in output
    assert "sample_count" in output
    assert "decode_call_count" in output


def test_ready_for_full_fails_skipped_diagnostic_python_row(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path)
    rows = []
    for case in DIAGNOSTIC_CASES:
        rows.append(_diagnostic_row(case, "rbposd"))
        rows.append(
            _diagnostic_row(
                case,
                "ldpc_bposd",
                status="skipped",
                setup_seconds="",
                decode_seconds="",
                run_seconds="",
                logical_error_rate="",
                error=(
                    "python dependency unavailable for ldpc_bposd replay: "
                    "No module named 'ldpc'"
                ),
            )
        )
    _write_csv(tmp_path / "diagnostic" / "results.csv", CSV_HEADER, rows)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL diagnostic-compare" in output
    assert "diagnostic/results.csv" in output
    assert "Python ldpc_bposd diagnostic row is skipped" in output


def test_ready_for_full_warns_without_optional_provenance(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path, provenance=False)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "WARN provenance" in output
    assert "provenance.json" in output
    assert "WARN readiness verdict" in output


def test_ready_for_full_warns_without_recognized_provenance_fields(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path)
    _write_json(tmp_path / "provenance.json", {"note": "unrecognized"})

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "WARN provenance" in output
    assert "no recognized provenance fields" in output
    assert "provenance.json" in output
    assert "WARN readiness verdict" in output


def test_ready_for_full_warns_on_partial_provenance(
    tmp_path, capsys
) -> None:
    write_ready_tree(tmp_path, provenance=False)
    _write_json(
        tmp_path / "provenance.json",
        {"timestamp": "2026-06-27T00:00:00+08:00"},
    )

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "WARN provenance" in output
    assert "provenance.json" in output
    assert "incomplete" in output or "artifact_hash" in output or "command" in output
    assert "WARN readiness verdict" in output
