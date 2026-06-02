from __future__ import annotations

import time

from ilpqec import Decoder, get_available_solvers

from ..schema import CaseBundle, ResultRow
from .base import BenchmarkDriver, count_row_mismatches, load_b8_rows, unpack_b8_rows


def _preferred_solver() -> str:
    available = set(get_available_solvers())
    if "gurobi" in available:
        return "gurobi"
    return "highs"


class IlpqecDriver(BenchmarkDriver):
    name = "ilpqec"
    backend = ""

    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        solver = _preferred_solver()

        compile_started = time.perf_counter()
        decoder = Decoder.from_stim_dem(
            bundle.dem_path.read_text(),
            solver=solver,
            threads=1,
            verbose=False,
        )
        compile_us = (time.perf_counter() - compile_started) * 1e6
        backend = decoder.solver_name

        det_rows = unpack_b8_rows(
            load_b8_rows(bundle.dets_b8_path, bundle.num_shots, bundle.num_dets),
            bundle.num_dets,
        )
        obs_rows = unpack_b8_rows(
            load_b8_rows(bundle.obs_b8_path, bundle.num_shots, bundle.num_obs),
            bundle.num_obs,
        )

        total_decode_us = 0.0
        shots_used = 0
        logical_errors = 0

        while shots_used < bundle.num_shots and logical_errors < bundle.tier.max_errors:
            current = min(batch_size, bundle.num_shots - shots_used)
            det_batch = det_rows[shots_used : shots_used + current]
            obs_batch = obs_rows[shots_used : shots_used + current]

            started = time.perf_counter()
            pred_batch = decoder.decode_batch(det_batch)
            total_decode_us += (time.perf_counter() - started) * 1e6

            logical_errors += count_row_mismatches(pred_batch, obs_batch)
            shots_used += current

        row = self._finish(
            bundle=bundle,
            compile_us=compile_us,
            total_decode_us=total_decode_us,
            shots_used=shots_used,
            logical_errors=logical_errors,
        )
        return row.__class__(**{**row.to_csv_row(), "backend": backend})
