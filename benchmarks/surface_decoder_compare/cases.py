import json
from pathlib import Path

import numpy as np
import stim

from .schema import CaseBundle, CaseSpec, DEFAULT_DISTANCES, DEFAULT_P_VALUES, TierConfig


def build_case_specs() -> list[CaseSpec]:
    return [
        CaseSpec(distance=distance, rounds=distance, p=p)
        for distance in DEFAULT_DISTANCES
        for p in DEFAULT_P_VALUES
    ]


def _case_dir(root: Path, spec: CaseSpec, tier: TierConfig) -> Path:
    return root / tier.name / "cases" / spec.slug


def materialize_case_bundle(
    root: Path,
    spec: CaseSpec,
    tier: TierConfig,
    seed: int,
) -> CaseBundle:
    case_dir = _case_dir(root, spec, tier)
    case_dir.mkdir(parents=True, exist_ok=True)

    circuit = stim.Circuit.generated(
        "surface_code:rotated_memory_x",
        distance=spec.distance,
        rounds=spec.rounds,
        after_clifford_depolarization=spec.p,
    )
    dem = circuit.detector_error_model(
        decompose_errors=True,
        flatten_loops=True,
    )
    sampler = circuit.compile_detector_sampler(
        seed=seed + spec.distance * 10_000 + int(spec.p * 1_000_000)
    )
    dets_b8, obs_b8 = sampler.sample(
        shots=tier.max_shots,
        separate_observables=True,
        bit_packed=True,
    )

    circuit_path = case_dir / "circuit.stim"
    dem_path = case_dir / "model.dem"
    dets_b8_path = case_dir / "detections.b8"
    obs_b8_path = case_dir / "observables.b8"
    metadata_path = case_dir / "metadata.json"

    circuit_path.write_text(str(circuit))
    dem_path.write_text(str(dem))
    np.asarray(dets_b8, dtype=np.uint8).tofile(dets_b8_path)
    np.asarray(obs_b8, dtype=np.uint8).tofile(obs_b8_path)
    metadata_path.write_text(
        json.dumps(
            {
                "distance": spec.distance,
                "rounds": spec.rounds,
                "p": spec.p,
                "seed": seed,
                "tier": tier.name,
                "num_dets": dem.num_detectors,
                "num_obs": dem.num_observables,
                "num_shots": tier.max_shots,
                "circuit_path": str(circuit_path),
                "dem_path": str(dem_path),
                "dets_b8_path": str(dets_b8_path),
                "obs_b8_path": str(obs_b8_path),
            },
            indent=2,
        )
    )

    return CaseBundle(
        spec=spec,
        tier=tier,
        seed=seed,
        num_dets=dem.num_detectors,
        num_obs=dem.num_observables,
        num_shots=tier.max_shots,
        circuit_path=circuit_path,
        dem_path=dem_path,
        dets_b8_path=dets_b8_path,
        obs_b8_path=obs_b8_path,
        metadata_path=metadata_path,
    )
