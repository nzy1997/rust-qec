from __future__ import annotations

import time

import pymatching
import stim

from ..schema import CaseBundle, ResultRow
from .base import BenchmarkDriver, count_row_mismatches, load_b8_rows


class PymatchingDriver(BenchmarkDriver):
    name = "pymatching"
    backend = "native"

    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        dem = stim.DetectorErrorModel(bundle.dem_path.read_text())

        compile_started = time.perf_counter()
        matcher = pymatching.Matching.from_detector_error_model(dem)
        compile_us = (time.perf_counter() - compile_started) * 1e6

        det_rows = load_b8_rows(bundle.dets_b8_path, bundle.num_shots, bundle.num_dets)
        obs_rows = load_b8_rows(bundle.obs_b8_path, bundle.num_shots, bundle.num_obs)

        total_decode_us = 0.0
        shots_used = 0
        logical_errors = 0

        while shots_used < bundle.num_shots and logical_errors < bundle.tier.max_errors:
            current = min(batch_size, bundle.num_shots - shots_used)
            det_batch = det_rows[shots_used : shots_used + current]
            obs_batch = obs_rows[shots_used : shots_used + current]

            started = time.perf_counter()
            pred_batch = matcher.decode_batch(
                det_batch,
                bit_packed_shots=True,
                bit_packed_predictions=True,
            )
            total_decode_us += (time.perf_counter() - started) * 1e6

            logical_errors += count_row_mismatches(pred_batch, obs_batch)
            shots_used += current

        return self._finish(
            bundle=bundle,
            compile_us=compile_us,
            total_decode_us=total_decode_us,
            shots_used=shots_used,
            logical_errors=logical_errors,
        )
