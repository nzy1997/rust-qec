from __future__ import annotations

from dataclasses import dataclass

import stim


@dataclass(frozen=True)
class MatrixProblem:
    num_dets: int
    num_obs: int
    detector_columns: list[list[int]]
    observable_columns: list[list[int]]
    probabilities: list[float]


def _toggle(items: set[int], value: int) -> None:
    if value in items:
        items.remove(value)
    else:
        items.add(value)


def lower_dem_to_matrix_problem(dem_text: str) -> MatrixProblem:
    dem = stim.DetectorErrorModel(dem_text).flattened()
    detector_columns: list[list[int]] = []
    observable_columns: list[list[int]] = []
    probabilities: list[float] = []

    for raw_line in str(dem).splitlines():
        line = raw_line.strip()
        if not line or not line.startswith("error("):
            continue
        probability = float(line.split("(", 1)[1].split(")", 1)[0])
        current_dets: set[int] = set()
        current_obs: set[int] = set()
        for token in line.replace("^", " ").split()[1:]:
            if token.startswith("D"):
                _toggle(current_dets, int(token[1:]))
            elif token.startswith("L"):
                _toggle(current_obs, int(token[1:]))
        detector_columns.append(sorted(current_dets))
        observable_columns.append(sorted(current_obs))
        probabilities.append(probability)

    return MatrixProblem(
        num_dets=dem.num_detectors,
        num_obs=dem.num_observables,
        detector_columns=detector_columns,
        observable_columns=observable_columns,
        probabilities=probabilities,
    )
