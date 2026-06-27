from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any, Sequence


REFERENCE_DIR = Path(__file__).resolve().parent / "reference"
DEFAULT_EXPECTED_AUDIT_PATH = (
    REFERENCE_DIR / "bravyi_model_audit_bb72_p003_c6.json"
)
EXPECTED_AUDIT_PATH = Path(
    os.environ.get("BRAVYI_MODEL_AUDIT_EXPECTED_PATH", DEFAULT_EXPECTED_AUDIT_PATH)
)
AUDIT_VERSION = 1


def _load_json_object(path: Path) -> dict[str, Any]:
    parsed = json.loads(path.read_text())
    if not isinstance(parsed, dict):
        raise ValueError(f"{path} root must be a JSON object")
    return parsed


def load_expected_summary() -> dict[str, Any]:
    fixture = _load_json_object(EXPECTED_AUDIT_PATH)
    expected = fixture.get("expected")
    if not isinstance(expected, dict):
        raise ValueError("expected fixture missing object-valued 'expected'")
    return expected


def _compare_mapping(
    path: str,
    observed: Any,
    expected: Any,
    errors: list[str],
) -> None:
    if isinstance(expected, dict):
        if not isinstance(observed, dict):
            errors.append(path)
            return
        for key, expected_value in expected.items():
            child_path = f"{path}.{key}" if path else key
            if key not in observed:
                errors.append(child_path)
                continue
            _compare_mapping(child_path, observed[key], expected_value, errors)
        return
    if observed != expected:
        errors.append(path)


def _get_mapping(
    container: dict[str, Any], key: str, path: str, errors: list[str]
) -> dict[str, Any]:
    value = container.get(key)
    if not isinstance(value, dict):
        errors.append(path)
        return {}
    return value


def verify_audit_artifact(artifact: dict[str, object]) -> list[str]:
    expected = load_expected_summary()
    errors: list[str] = []

    if artifact.get("audit_version") != AUDIT_VERSION:
        errors.append("audit_version")
    if artifact.get("status") != "pass":
        errors.append("status")

    inputs = _get_mapping(artifact, "inputs", "inputs", errors)
    observed = _get_mapping(artifact, "observed", "observed", errors)
    code = _get_mapping(observed, "code", "code", errors)
    schedule = _get_mapping(observed, "schedule", "schedule", errors)
    tail = _get_mapping(observed, "syndrome_tail", "syndrome_tail", errors)
    models = _get_mapping(observed, "models", "models", errors)
    z_model = _get_mapping(models, "Z", "models.Z", errors)
    x_model = _get_mapping(models, "X", "models.X", errors)

    _compare_mapping("inputs", inputs, expected["inputs"], errors)
    _compare_mapping("code", code, expected["code"], errors)
    _compare_mapping("schedule", schedule, expected["schedule"], errors)
    _compare_mapping("syndrome_tail", tail, expected["syndrome_tail"], errors)
    _compare_mapping("models.Z", z_model, expected["models"]["Z"], errors)
    _compare_mapping("models.X", x_model, expected["models"]["X"], errors)
    return errors


def _pass_line(artifact: dict[str, Any]) -> str:
    inputs = artifact["inputs"]
    observed = artifact["observed"]
    code = observed["code"]
    schedule = observed["schedule"]
    tail = observed["syndrome_tail"]
    z_model = observed["models"]["Z"]
    x_model = observed["models"]["X"]
    return (
        "PASS Bravyi model audit "
        f"{inputs['code_id']} [[{code['n']},{code['k']}]] "
        f"schedule_ops={schedule['operation_count']} "
        f"num_cycles_plus_tail={tail['num_cycles_plus_tail']} "
        f"Z first_logical_row={z_model['first_logical_row']} "
        f"dims={z_model['decoder_rows']}x{z_model['decoder_columns']} "
        f"probability_total={z_model['probability_total']} "
        f"hashes={z_model['channel_probabilities_hash']}/{z_model['augmented_columns_hash']} "
        f"X first_logical_row={x_model['first_logical_row']} "
        f"dims={x_model['decoder_rows']}x{x_model['decoder_columns']} "
        f"probability_total={x_model['probability_total']} "
        f"hashes={x_model['channel_probabilities_hash']}/{x_model['augmented_columns_hash']}"
    )


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 1:
        print(
            "usage: python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit <artifact>",
            file=sys.stderr,
        )
        return 2

    artifact_path = Path(args[0])
    artifact = _load_json_object(artifact_path)
    errors = verify_audit_artifact(artifact)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(_pass_line(artifact))
    return 0


if __name__ == "__main__":
    sys.exit(main())
