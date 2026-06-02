from dataclasses import asdict, dataclass
from pathlib import Path


DEFAULT_DISTANCES = (3, 5, 7)
DEFAULT_P_VALUES = (0.001, 0.002, 0.003, 0.005, 0.007, 0.010, 0.015)
DEFAULT_BATCH_SIZE = 256


@dataclass(frozen=True)
class TierConfig:
    name: str
    max_shots: int
    max_errors: int


TIER_CONFIGS = {
    "smoke": TierConfig(name="smoke", max_shots=2_000, max_errors=20),
    "full": TierConfig(name="full", max_shots=100_000, max_errors=1_000),
}


@dataclass(frozen=True)
class CaseSpec:
    distance: int
    rounds: int
    p: float

    @property
    def slug(self) -> str:
        return f"d{self.distance}_p{self.p:.3f}"


@dataclass(frozen=True)
class CaseBundle:
    spec: CaseSpec
    tier: TierConfig
    seed: int
    num_dets: int
    num_obs: int
    num_shots: int
    circuit_path: Path
    dem_path: Path
    dets_b8_path: Path
    obs_b8_path: Path
    metadata_path: Path


@dataclass(frozen=True)
class ResultRow:
    tier: str
    decoder: str
    backend: str
    distance: int
    rounds: int
    p: float
    seed: int
    num_dets: int
    num_obs: int
    shots_budget: int
    errors_budget: int
    shots_used: int
    logical_errors: int
    logical_error_rate: float
    compile_us: float
    total_decode_us: float
    decode_us_per_shot: float
    status: str
    error: str

    def to_csv_row(self) -> dict[str, object]:
        return asdict(self)


CSV_HEADER = [
    "tier",
    "decoder",
    "backend",
    "distance",
    "rounds",
    "p",
    "seed",
    "num_dets",
    "num_obs",
    "shots_budget",
    "errors_budget",
    "shots_used",
    "logical_errors",
    "logical_error_rate",
    "compile_us",
    "total_decode_us",
    "decode_us_per_shot",
    "status",
    "error",
]
