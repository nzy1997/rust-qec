from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path
from typing import Any, Sequence


REPO_ROOT = Path(__file__).resolve().parents[2]
REFERENCE_DIR = Path(__file__).resolve().parent / "reference"
CONTRACT_PATH = REFERENCE_DIR / "bravyi_contract.json"
EXPECTED_AUDIT_PATH = REFERENCE_DIR / "bravyi_model_audit_bb72_p003_c6.json"
AUDIT_VERSION = 1


def _load_json_object(path: Path) -> dict[str, Any]:
    parsed = json.loads(path.read_text())
    if not isinstance(parsed, dict):
        raise ValueError(f"{path} root must be a JSON object")
    return parsed


def _format_float(value: float) -> str:
    return format(value, ".17g")


def _hash_json(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def _normalize_probability(probability: Any) -> str:
    return _format_float(float(probability))


def _normalize_input_float(value: Any) -> str:
    return str(float(value))


def _model_summary(model: dict[str, Any]) -> dict[str, Any]:
    probabilities = [float(value) for value in model["channel_probs"]]
    return {
        "decoder_rows": model["num_checks"],
        "decoder_columns": model["num_bits"],
        "first_logical_row": model["first_logical_row"],
        "grouped_column_count": len(model["augmented_columns"]),
        "sparse_rows_hash": _hash_json(model["sparse_rows"]),
        "augmented_columns_hash": _hash_json(model["augmented_columns"]),
        "channel_probabilities_hash": _hash_json(
            [_normalize_probability(value) for value in probabilities]
        ),
        "probability_total": _format_float(math.fsum(probabilities)),
        "probability_min": _format_float(min(probabilities)),
        "probability_max": _format_float(max(probabilities)),
    }


def _observed_summary(rust_export: dict[str, Any]) -> dict[str, Any]:
    return {
        "code": rust_export["code"],
        "schedule": rust_export["schedule"],
        "syndrome_tail": {
            "configured_noisy_cycles": rust_export["num_cycles"],
            "noiseless_tail_cycles": rust_export["noiseless_tail_cycles"],
            "num_cycles_plus_tail": rust_export["num_cycles_plus_tail"],
        },
        "models": {
            "Z": _model_summary(rust_export["z_model"]),
            "X": _model_summary(rust_export["x_model"]),
        },
    }


def _compare_expected(
    observed: Any,
    expected: Any,
    path: str = "",
) -> list[str]:
    mismatches: list[str] = []
    if isinstance(expected, dict):
        if not isinstance(observed, dict):
            return [path or "<root>"]
        for key, expected_value in expected.items():
            child_path = f"{path}.{key}" if path else key
            if key not in observed:
                mismatches.append(child_path)
                continue
            mismatches.extend(
                _compare_expected(observed[key], expected_value, child_path)
            )
        return mismatches
    if isinstance(expected, list):
        if observed != expected:
            return [path or "<root>"]
        return []
    if observed != expected:
        return [path or "<root>"]
    return []


def _run_rust_model_audit_export(
    code_id: str,
    physical_error_rate: str,
    num_cycles: int,
) -> dict[str, Any]:
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
        code_id,
        "--physical-error-rate",
        physical_error_rate,
        "--num-cycles",
        str(num_cycles),
        "--max-bp-iterations",
        "10000",
        "--osd-order",
        "7",
        "--json-model-audit",
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
            "Rust model audit export failed\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
    parsed = json.loads(result.stdout)
    if not isinstance(parsed, dict):
        raise ValueError("Rust model audit export must be a JSON object")
    return parsed


def build_audit_artifact(rust_export: dict[str, Any]) -> dict[str, Any]:
    contract = _load_json_object(CONTRACT_PATH)
    expected_fixture = _load_json_object(EXPECTED_AUDIT_PATH)
    expected = expected_fixture["expected"]
    observed = _observed_summary(rust_export)
    inputs = {
        "code_id": rust_export["code_id"],
        "physical_error_rate": _normalize_input_float(
            rust_export["physical_error_rate"]
        ),
        "num_cycles": rust_export["num_cycles"],
    }
    mismatches = _compare_expected(
        {"inputs": inputs, **observed},
        expected,
    )
    status = "pass" if not mismatches else "fail"
    return {
        "audit_version": AUDIT_VERSION,
        "inputs": inputs,
        "provenance": {
            "contract_path": str(CONTRACT_PATH),
            "contract_version": contract.get("contract_version"),
            "expected_fixture": str(EXPECTED_AUDIT_PATH),
            "expected_fixture_version": expected_fixture.get("fixture_version"),
            "upstream_repository": expected_fixture.get("provenance", {}).get(
                "upstream_repository"
            ),
            "upstream_commit": expected_fixture.get("provenance", {}).get(
                "upstream_commit"
            ),
        },
        "observed": observed,
        "expected": expected,
        "status": status,
        "mismatches": mismatches,
    }


def write_audit(path: Path, artifact: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Audit the BB72 Bravyi effective decoder model export."
    )
    parser.add_argument("--code-id", required=True)
    parser.add_argument("--physical-error-rate", required=True)
    parser.add_argument("--num-cycles", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)

    rust_export = _run_rust_model_audit_export(
        args.code_id,
        args.physical_error_rate,
        args.num_cycles,
    )
    artifact = build_audit_artifact(rust_export)
    write_audit(args.out, artifact)
    return 0 if artifact["status"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
