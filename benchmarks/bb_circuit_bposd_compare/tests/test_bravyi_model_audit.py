from __future__ import annotations

import json
import subprocess
import sys
from types import SimpleNamespace
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import bravyi_model_audit
from benchmarks.bb_circuit_bposd_compare import verify_model_audit


REFERENCE_DIR = Path(__file__).resolve().parents[1] / "reference"
EXPECTED_FIXTURE_PATH = REFERENCE_DIR / "bravyi_model_audit_bb72_p003_c6.json"

FAKE_RUST_EXPORT = {
    "code_id": "bb72",
    "physical_error_rate": 0.003,
    "num_cycles": 6,
    "noiseless_tail_cycles": 2,
    "num_cycles_plus_tail": 8,
    "code": {
        "ell": 6,
        "m": 6,
        "n2": 36,
        "n": 72,
        "k": 12,
        "x_check_count": 36,
        "z_check_count": 36,
        "data_qubit_count": 72,
    },
    "schedule": {
        "sx_labels": ["idle", "1", "4", "3", "5", "0", "2"],
        "sz_labels": ["3", "5", "0", "1", "2", "4", "idle"],
        "operation_count": 720,
        "operation_count_by_kind": {
            "cnot": 432,
            "idle": 144,
            "meas_x": 36,
            "meas_z": 36,
            "prep_x": 36,
            "prep_z": 36,
        },
    },
    "z_model": {
        "num_checks": 288,
        "num_bits": 2233,
        "first_logical_row": 288,
        "sparse_rows": [[0, 2], [1]],
        "augmented_columns": [[0], [1, 2], []],
        "channel_probs": [0.003, 0.0032, 0.0030448],
    },
    "x_model": {
        "num_checks": 288,
        "num_bits": 2269,
        "first_logical_row": 288,
        "sparse_rows": [[3], [0, 4]],
        "augmented_columns": [[1], [], [0, 2]],
        "channel_probs": [0.0031, 0.003, 0.0031448],
    },
}


def _expected_fixture() -> dict[str, object]:
    return {
        "fixture_version": 1,
        "description": "Test fixture for normalized BB72 Bravyi model audit evidence.",
        "provenance": {
            "upstream_repository": "sbravyi/BivariateBicycleCodes",
            "upstream_commit": "fa77e3333d3ec44c79d8f914dd24c040d1da471b",
            "contract_path": "benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json",
        },
        "expected": {
            "inputs": {
                "code_id": "bb72",
                "physical_error_rate": "0.003",
                "num_cycles": 6,
            },
            "code": FAKE_RUST_EXPORT["code"],
            "schedule": FAKE_RUST_EXPORT["schedule"],
            "syndrome_tail": {
                "configured_noisy_cycles": 6,
                "noiseless_tail_cycles": 2,
                "num_cycles_plus_tail": 8,
            },
            "models": {
                "Z": {
                    "decoder_rows": 288,
                    "decoder_columns": 2233,
                    "first_logical_row": 288,
                    "grouped_column_count": 3,
                    "sparse_rows_hash": "bb8bfd012cead4feda132268c59b45440da98e238ce0cb1e182e708cdaa52495",
                    "augmented_columns_hash": "0dd5c712a923766ef893872c865f253ee5605ac3b456e976d57bc9b1431f00f7",
                    "channel_probabilities_hash": "d54bce9a5c487630f00a47655480048cfc09f212e6dbaa214ac8cd497eed8c96",
                    "probability_total": "0.0092448000000000009",
                    "probability_min": "0.0030000000000000001",
                    "probability_max": "0.0032000000000000002",
                },
                "X": {
                    "decoder_rows": 288,
                    "decoder_columns": 2269,
                    "first_logical_row": 288,
                    "grouped_column_count": 3,
                    "sparse_rows_hash": "f26400ae740ba22e7e78afb5fe7196e5bbaa5143ec68dcb3702fb41c42ccd6da",
                    "augmented_columns_hash": "2453c2ad03c138efe60a6299acb6ec36a400d212457db6472046c8c6fe2ab713",
                    "channel_probabilities_hash": "3f2d2ea73366f5cc41c93c22b6421fae84fd8749fcfe86792840f84afdada145",
                    "probability_total": "0.0092448000000000009",
                    "probability_min": "0.0030000000000000001",
                    "probability_max": "0.0031448000000000001",
                },
            },
        },
    }


def test_build_audit_artifact_passes_when_export_matches_expected_fixture(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)

    assert artifact["audit_version"] == 1
    assert artifact["status"] == "pass"
    assert artifact["mismatches"] == []
    assert artifact["inputs"] == {
        "code_id": "bb72",
        "physical_error_rate": "0.003",
        "num_cycles": 6,
    }
    assert artifact["observed"]["syndrome_tail"]["num_cycles_plus_tail"] == 8
    assert artifact["observed"]["schedule"]["operation_count"] == 720
    assert artifact["observed"]["models"]["Z"]["first_logical_row"] == 288
    assert artifact["observed"]["models"]["X"]["decoder_columns"] == 2269
    assert artifact["expected"] == _expected_fixture()["expected"]


def test_build_audit_artifact_reports_named_mismatches(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    mutated = json.loads(json.dumps(FAKE_RUST_EXPORT))
    mutated["schedule"]["operation_count_by_kind"]["idle"] = 145
    mutated["x_model"]["first_logical_row"] = 289

    artifact = bravyi_model_audit.build_audit_artifact(mutated)

    assert artifact["status"] == "fail"
    assert "schedule.operation_count_by_kind.idle" in artifact["mismatches"]
    assert "models.X.first_logical_row" in artifact["mismatches"]


def test_audit_cli_writes_json_with_mocked_rust_export(
    tmp_path: Path, monkeypatch
) -> None:
    out = tmp_path / "model_audit.json"
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(
        bravyi_model_audit,
        "_run_rust_model_audit_export",
        lambda *args, **kwargs: json.loads(json.dumps(FAKE_RUST_EXPORT)),
    )

    status = bravyi_model_audit.main(
        [
            "--code-id",
            "bb72",
            "--physical-error-rate",
            "0.003",
            "--num-cycles",
            "6",
            "--out",
            str(out),
        ]
    )

    assert status == 0
    artifact = json.loads(out.read_text())
    assert artifact["status"] == "pass"
    assert artifact["provenance"]["expected_fixture"] == str(expected_path)
    assert artifact["observed"]["models"]["Z"]["probability_total"] == "0.0092448000000000009"


def test_rust_model_audit_export_uses_trial_free_json_audit_command(
    monkeypatch,
) -> None:
    captured: dict[str, object] = {}

    def fake_run(command, **kwargs):
        captured["command"] = command
        captured["kwargs"] = kwargs
        return SimpleNamespace(returncode=0, stdout="{}", stderr="")

    monkeypatch.setattr(bravyi_model_audit.subprocess, "run", fake_run)

    result = bravyi_model_audit._run_rust_model_audit_export("bb72", "0.003", 6)

    assert result == {}
    command = captured["command"]
    assert "--json-model-audit" in command
    assert "--num-trials" not in command
    assert "--json-compare-case" not in command


def test_checked_in_expected_fixture_has_required_provenance() -> None:
    fixture = json.loads(EXPECTED_FIXTURE_PATH.read_text())

    assert fixture["fixture_version"] == 1
    assert fixture["provenance"]["upstream_commit"] == (
        "fa77e3333d3ec44c79d8f914dd24c040d1da471b"
    )
    assert fixture["provenance"]["contract_path"] == (
        "benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json"
    )


def test_verify_model_audit_accepts_good_artifact(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(verify_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)

    assert verify_model_audit.verify_audit_artifact(artifact) == []


def test_verify_model_audit_rejects_tail_cycle_drift(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(verify_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact["observed"]["syndrome_tail"]["noiseless_tail_cycles"] = 1

    errors = verify_model_audit.verify_audit_artifact(artifact)

    assert any(
        "syndrome_tail.noiseless_tail_cycles" in error for error in errors
    )


def test_verify_model_audit_rejects_schedule_label_drift(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(verify_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact["observed"]["schedule"]["sx_labels"][0] = "changed"

    errors = verify_model_audit.verify_audit_artifact(artifact)

    assert any("schedule.sx_labels" in error for error in errors)


def test_verify_model_audit_rejects_model_summary_drift(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(verify_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact["expected"]["models"]["Z"]["decoder_columns"] = -1
    artifact["observed"]["models"]["Z"]["decoder_columns"] = 999

    errors = verify_model_audit.verify_audit_artifact(artifact)

    assert any("models.Z.decoder_columns" in error for error in errors)


def test_verify_model_audit_cli_reports_pass(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(verify_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact_path = tmp_path / "artifact.json"
    artifact_path.write_text(json.dumps(artifact))

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_model_audit",
            str(artifact_path),
        ],
        cwd=REFERENCE_DIR.parents[2],
        capture_output=True,
        text=True,
        check=False,
        env={
            **__import__("os").environ,
            "BRAVYI_MODEL_AUDIT_EXPECTED_PATH": str(expected_path),
        },
    )

    assert result.returncode == 0
    assert "PASS Bravyi model audit bb72 [[72,12]]" in result.stdout
    assert "num_cycles_plus_tail=8" in result.stdout
    assert result.stderr == ""


def test_verify_model_audit_cli_reports_failure(
    tmp_path: Path, monkeypatch
) -> None:
    expected_path = tmp_path / "expected.json"
    expected_path.write_text(json.dumps(_expected_fixture()))
    monkeypatch.setattr(bravyi_model_audit, "EXPECTED_AUDIT_PATH", expected_path)
    monkeypatch.setattr(verify_model_audit, "EXPECTED_AUDIT_PATH", expected_path)

    artifact = bravyi_model_audit.build_audit_artifact(FAKE_RUST_EXPORT)
    artifact["observed"]["schedule"]["operation_count"] = 721
    artifact_path = tmp_path / "artifact_bad.json"
    artifact_path.write_text(json.dumps(artifact))

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_model_audit",
            str(artifact_path),
        ],
        cwd=REFERENCE_DIR.parents[2],
        capture_output=True,
        text=True,
        check=False,
        env={
            **__import__("os").environ,
            "BRAVYI_MODEL_AUDIT_EXPECTED_PATH": str(expected_path),
        },
    )

    assert result.returncode != 0
    assert "schedule.operation_count" in result.stderr
