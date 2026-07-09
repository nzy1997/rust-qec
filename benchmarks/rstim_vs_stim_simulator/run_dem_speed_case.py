from __future__ import annotations

import json
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from typing import Any


PACKAGE_DIR = Path(__file__).resolve().parent
FIXTURES_DIR = PACKAGE_DIR / "fixtures"
FULL_CASE_LABEL = "stim-style-surface-dem-sample-d11-r100-b1024"


@dataclass(frozen=True, slots=True)
class DemCase:
    label: str
    dem_path: Path
    metadata_path: Path
    shots: int
    expected_detectors: int
    expected_observables: int


def sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


FULL_CASE = DemCase(
    label=FULL_CASE_LABEL,
    dem_path=FIXTURES_DIR / "stim_surface_code_rotated_memory_z_d11_r100.dem",
    metadata_path=FIXTURES_DIR / "stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json",
    shots=1024,
    expected_detectors=12000,
    expected_observables=1,
)

DEM_CASES = {FULL_CASE.label: FULL_CASE}


def case_by_label(label: str) -> DemCase:
    return DEM_CASES[label]


def _mismatch(detail: str) -> ValueError:
    return ValueError(f"DEM metadata mismatch: {detail}")


def _load_metadata(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise _mismatch(f"missing metadata file: {path}") from error
    except json.JSONDecodeError as error:
        raise _mismatch(f"invalid metadata JSON in {path}: {error}") from error
    if not isinstance(payload, dict):
        raise _mismatch(f"metadata file must contain a JSON object: {path}")
    return payload


def _require_value(metadata: dict[str, Any], key: str) -> Any:
    if key not in metadata:
        raise _mismatch(f'metadata missing required field "{key}"')
    return metadata[key]


def _require_equal(actual: Any, expected: Any, detail: str) -> None:
    if actual != expected:
        raise _mismatch(detail)


def _resolve_metadata_path(raw_path: object, *, metadata_path: Path) -> Path:
    path = Path(str(raw_path))
    if path.is_absolute():
        return path.resolve()
    return (metadata_path.parent / path).resolve()


def load_and_validate_dem_case(case: DemCase) -> tuple[str, dict[str, object]]:
    dem_text = case.dem_path.read_text()
    metadata = _load_metadata(case.metadata_path)

    _require_equal(_require_value(metadata, "case_label"), case.label, "case label does not match")
    _require_equal(
        _resolve_metadata_path(_require_value(metadata, "dem_path"), metadata_path=case.metadata_path),
        case.dem_path.resolve(),
        "dem path does not match",
    )
    _require_equal(
        _require_value(metadata, "dem_sha256"),
        sha256_file(case.dem_path),
        "dem hash does not match",
    )
    _require_equal(_require_value(metadata, "shots"), case.shots, "shot count does not match")
    _require_equal(
        _require_value(metadata, "expected_detectors"),
        case.expected_detectors,
        "detector count does not match",
    )
    _require_equal(
        _require_value(metadata, "expected_observables"),
        case.expected_observables,
        "observable count does not match",
    )

    source_path_value = metadata.get("source_circuit_path")
    if isinstance(source_path_value, str) and source_path_value.strip():
        source_path = _resolve_metadata_path(source_path_value, metadata_path=case.metadata_path)
        if source_path.exists():
            _require_equal(
                _require_value(metadata, "source_circuit_sha256"),
                sha256_file(source_path),
                "source circuit hash does not match",
            )

    return dem_text, metadata
