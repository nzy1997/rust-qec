from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import replace
from pathlib import Path

import numpy as np

from ..schema import CaseBundle, ResultRow


def load_b8_rows(path: Path, shots: int, bits: int) -> np.ndarray:
    width = (bits + 7) // 8
    data = np.fromfile(path, dtype=np.uint8)
    return data.reshape(shots, width)


def unpack_b8_rows(rows: np.ndarray, bits: int) -> np.ndarray:
    unpacked = np.unpackbits(rows, axis=1, bitorder="little")
    return unpacked[:, :bits]


def count_row_mismatches(pred_rows: np.ndarray, obs_rows: np.ndarray) -> int:
    return int(np.sum(np.any(pred_rows != obs_rows, axis=1)))


class BenchmarkDriver(ABC):
    name: str
    backend: str = "native"

    @abstractmethod
    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        raise NotImplementedError

    def _base_result(self, bundle: CaseBundle) -> ResultRow:
        return ResultRow(
            tier=bundle.tier.name,
            decoder=self.name,
            backend=self.backend,
            distance=bundle.spec.distance,
            rounds=bundle.spec.rounds,
            p=bundle.spec.p,
            seed=bundle.seed,
            num_dets=bundle.num_dets,
            num_obs=bundle.num_obs,
            shots_budget=bundle.tier.max_shots,
            errors_budget=bundle.tier.max_errors,
            shots_used=0,
            logical_errors=0,
            logical_error_rate=0.0,
            compile_us=0.0,
            total_decode_us=0.0,
            decode_us_per_shot=0.0,
            status="ok",
            error="",
        )

    def _finish(
        self,
        bundle: CaseBundle,
        compile_us: float,
        total_decode_us: float,
        shots_used: int,
        logical_errors: int,
    ) -> ResultRow:
        base = self._base_result(bundle)
        logical_error_rate = logical_errors / shots_used if shots_used else 0.0
        decode_us_per_shot = total_decode_us / shots_used if shots_used else 0.0
        return replace(
            base,
            compile_us=compile_us,
            total_decode_us=total_decode_us,
            shots_used=shots_used,
            logical_errors=logical_errors,
            logical_error_rate=logical_error_rate,
            decode_us_per_shot=decode_us_per_shot,
        )
