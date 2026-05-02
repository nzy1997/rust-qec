from dataclasses import dataclass
from pathlib import Path

import numpy as np
import stim


REPO_ROOT = Path(__file__).resolve().parent.parent
SURFACE_CODE_ROOT = REPO_ROOT / "PyMatching" / "benchmarks" / "surface_codes"
DEFAULT_SHOTS = 64
DEFAULT_SEED = 12345
TARGETS = ((5, 0.001), (17, 0.001))


@dataclass(frozen=True)
class SurfaceDemCase:
    name: str
    distance: int
    p: float
    stim_path: str
    dem: str
    num_detectors: int
    num_errors: int
    num_observables: int
    num_syndromes: int
    syndromes: list[list[int]]
    observables: list[list[int]]


def _find_stim_path(distance: int, p: float) -> Path:
    suffix = f"_{distance}_{p:.3f}.stim"
    candidates = sorted(SURFACE_CODE_ROOT.glob(f"**/*{suffix}"))
    if not candidates:
        raise FileNotFoundError(
            f"Could not find rotated surface-code circuit for d={distance}, p={p}"
        )
    return candidates[0]


def _bits_to_lists(array_like) -> list[list[int]]:
    array = np.asarray(array_like, dtype=np.uint8)
    return [[int(value) for value in row] for row in array.tolist()]


def _build_case(distance: int, p: float, shots: int, seed: int) -> SurfaceDemCase:
    stim_path = _find_stim_path(distance, p)
    circuit = stim.Circuit.from_file(str(stim_path))
    dem = circuit.detector_error_model(decompose_errors=True)
    sampler = circuit.compile_detector_sampler(seed=seed + distance)
    syndromes, observables = sampler.sample(shots=shots, separate_observables=True)

    return SurfaceDemCase(
        name=f"surface-d{distance}-p{p:.3f}",
        distance=distance,
        p=p,
        stim_path=str(stim_path.relative_to(REPO_ROOT)),
        dem=str(dem),
        num_detectors=dem.num_detectors,
        num_errors=dem.num_errors,
        num_observables=dem.num_observables,
        num_syndromes=shots,
        syndromes=_bits_to_lists(syndromes),
        observables=_bits_to_lists(observables),
    )


def build_cases(shots: int = DEFAULT_SHOTS, seed: int = DEFAULT_SEED) -> list[SurfaceDemCase]:
    return [_build_case(distance, p, shots, seed) for distance, p in TARGETS]
