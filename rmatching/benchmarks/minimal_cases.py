from dataclasses import dataclass
from itertools import product


@dataclass(frozen=True)
class MinimalCase:
    name: str
    dem: str
    num_detectors: int
    num_edges: int
    syndromes: list[list[int]]


def exhaustive_syndromes(num_detectors: int) -> list[list[int]]:
    return [list(bits) for bits in product((0, 1), repeat=num_detectors)]


def build_cases() -> list[MinimalCase]:
    return [
        MinimalCase(
            name="boundary-2",
            dem=(
                "error(0.1) D0 D1\n"
                "error(0.05) D0\n"
                "error(0.05) D1\n"
            ),
            num_detectors=2,
            num_edges=3,
            syndromes=exhaustive_syndromes(2),
        ),
        MinimalCase(
            name="square-4",
            dem=(
                "error(0.1) D0 D1\n"
                "error(0.1) D2 D3\n"
                "error(0.1) D0 D2\n"
                "error(0.1) D1 D3\n"
                "error(0.1) D0 D3 L0\n"
                "error(0.05) D0\n"
                "error(0.05) D1\n"
                "error(0.05) D2\n"
                "error(0.05) D3\n"
            ),
            num_detectors=4,
            num_edges=9,
            syndromes=exhaustive_syndromes(4),
        ),
        MinimalCase(
            name="blossom-3",
            dem=(
                "error(0.1) D0 D1\n"
                "error(0.1) D1 D2\n"
                "error(0.1) D0 D2 L0\n"
                "error(0.05) D0\n"
                "error(0.05) D1\n"
                "error(0.05) D2\n"
            ),
            num_detectors=3,
            num_edges=6,
            syndromes=exhaustive_syndromes(3),
        ),
    ]
