from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from types import ModuleType
from unittest import mock

from benchmarks.bb_circuit_bposd_compare import run_compare
from benchmarks.bb_circuit_bposd_compare import verify_bravyi_contract
from benchmarks.bb_circuit_bposd_compare.cases import SMOKE_CASES
from benchmarks.bb_circuit_bposd_compare.run_compare import _python_row
from benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract import (
    _load_contract,
    validate_contract,
)


CONTRACT_PATH = (
    Path(__file__).resolve().parents[1] / "reference" / "bravyi_contract.json"
)


def test_checked_in_bravyi_contract_matches_repository_defaults() -> None:
    assert validate_contract(_load_contract(CONTRACT_PATH)) == []


def test_contract_negative_controls_name_mismatched_fields() -> None:
    contract = _load_contract(CONTRACT_PATH)

    mutated = json.loads(json.dumps(contract))
    mutated["result_row"]["failure_unit"] = "per_cycle"
    assert any("result_row.failure_unit" in err for err in validate_contract(mutated))

    mutated = json.loads(json.dumps(contract))
    mutated["decoder"]["osd_order"] = 0
    assert any("decoder.osd_order" in err for err in validate_contract(mutated))

    mutated = json.loads(json.dumps(contract))
    mutated["decoder"]["ms_scaling_factor"] = 1
    assert any("decoder.ms_scaling_factor" in err for err in validate_contract(mutated))


def test_contract_validator_checks_rust_tail_cycle_source(
    tmp_path: Path,
    monkeypatch,
) -> None:
    source_path = tmp_path / "bb_circuit_memory.rs"
    source_path.write_text(
        "pub const BRAVYI_NOISELESS_TAIL_CYCLES: usize = 1;\n"
        "let total_cycles = config.num_cycles + BRAVYI_NOISELESS_TAIL_CYCLES;\n"
        "let total_cycles = num_cycles + BRAVYI_NOISELESS_TAIL_CYCLES;\n"
    )
    monkeypatch.setattr(
        verify_bravyi_contract,
        "RUST_BB_CIRCUIT_MEMORY_PATH",
        source_path,
    )

    errors = validate_contract(_load_contract(CONTRACT_PATH))

    assert any("BRAVYI_NOISELESS_TAIL_CYCLES" in err for err in errors)


def test_verify_bravyi_contract_cli_prints_required_pass_line() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract",
            str(CONTRACT_PATH),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    stdout = result.stdout
    assert "PASS" in stdout
    assert "fa77e3333d3ec44c79d8f914dd24c040d1da471b" in stdout
    assert "osd_cs" in stdout
    assert "OSD order 7" in stdout
    assert "ms_scaling_factor=0" in stdout
    assert "two noiseless tail cycles" in stdout
    assert "failure_unit=monte_carlo_trial" in stdout


def test_python_decoder_kwargs_expose_upstream_scaling() -> None:
    kwargs = run_compare._python_bposd_decoder_kwargs()
    assert kwargs["bp_method"] == "ms"
    assert kwargs["max_iter"] == 10000
    assert kwargs["osd_method"] == "osd_cs"
    assert kwargs["osd_order"] == 7
    assert kwargs["ms_scaling_factor"] == 0
    assert kwargs["input_vector_type"] == "syndrome"


class FakeMatrix:
    def __init__(self, shape: tuple[int, int]):
        rows, cols = shape
        self.rows = [[0 for _ in range(cols)] for _ in range(rows)]

    def __setitem__(self, key: tuple[int, int], value: int) -> None:
        row_index, column_index = key
        self.rows[row_index][column_index] = value


class FakeNumpy(ModuleType):
    uint8 = "uint8"

    def __init__(self) -> None:
        super().__init__("numpy")

    def zeros(self, shape: tuple[int, int], dtype: object = None) -> FakeMatrix:
        return FakeMatrix(shape)

    def asarray(self, values: list[bool], dtype: object = None) -> list[bool]:
        return list(values)


class FakeVector:
    def __init__(self, values: list[bool]):
        self._values = list(values)

    def tolist(self) -> list[bool]:
        return list(self._values)


def test_python_row_counts_trial_failure_once_and_skips_x_when_z_fails() -> None:
    class FakeDecoder:
        calls: list["FakeDecoder"] = []

        def __init__(self, matrix: FakeMatrix, **kwargs: object) -> None:
            self.matrix = matrix
            self.kwargs = kwargs
            self.decode_calls = 0
            FakeDecoder.calls.append(self)

        def decode(self, syndrome: list[bool]) -> FakeVector:
            self.decode_calls += 1
            if self is FakeDecoder.calls[1]:
                raise AssertionError("X decoder must not run after Z failure")
            return FakeVector([True])

    fake_ldpc = ModuleType("ldpc")
    fake_ldpc.BpOsdDecoder = FakeDecoder
    export = {
        "z_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[0]],
            "augmented_columns": [[1]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "x_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[]],
            "augmented_columns": [[]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "trials": [
            {
                "z_syndrome": [True],
                "x_syndrome": [False],
                "z_logical": [False],
                "x_logical": [False],
            }
        ],
    }

    with mock.patch.dict("sys.modules", {"numpy": FakeNumpy(), "ldpc": fake_ldpc}):
        row = _python_row(SMOKE_CASES[0], export)

    assert row["logical_error_rate"] == "1.0"
    assert FakeDecoder.calls[0].decode_calls == 1
    assert FakeDecoder.calls[1].decode_calls == 0
