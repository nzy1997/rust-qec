from __future__ import annotations

import time

import numpy as np
from ldpc import BpOsdDecoder

from ..schema import CaseBundle, ResultRow
from .base import BenchmarkDriver, load_b8_rows, unpack_b8_rows
from .dem_matrix import lower_dem_to_matrix_problem


def _xor_indices(bits: list[bool], indices: list[int]) -> None:
    for index in indices:
        bits[index] ^= True


class LdpcDriver(BenchmarkDriver):
    name = "ldpc"
    backend = "native"

    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        problem = lower_dem_to_matrix_problem(bundle.dem_path.read_text())
        filtered_detector_columns: list[list[int]] = []
        filtered_observable_columns: list[list[int]] = []
        filtered_probabilities: list[float] = []
        forced_syndrome = [False] * problem.num_dets
        baseline_observables = [False] * problem.num_obs

        for dets, obs, probability in zip(
            problem.detector_columns,
            problem.observable_columns,
            problem.probabilities,
        ):
            if probability <= 0.0:
                continue
            if probability >= 1.0:
                _xor_indices(forced_syndrome, dets)
                _xor_indices(baseline_observables, obs)
                continue
            if not dets:
                if probability > 0.5:
                    _xor_indices(baseline_observables, obs)
                continue
            filtered_detector_columns.append(dets)
            filtered_observable_columns.append(obs)
            filtered_probabilities.append(probability)

        pcm = np.zeros((problem.num_dets, len(filtered_detector_columns)), dtype=np.uint8)
        for column, dets in enumerate(filtered_detector_columns):
            for det in dets:
                pcm[det, column] = 1

        compile_started = time.perf_counter()
        decoder = BpOsdDecoder(
            pcm,
            error_channel=filtered_probabilities,
            max_iter=30,
            bp_method="minimum_sum",
            schedule="parallel",
            osd_method="OSD_0",
            osd_order=0,
            input_vector_type="syndrome",
        )
        compile_us = (time.perf_counter() - compile_started) * 1e6

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
            batch_pred = np.zeros((current, bundle.num_obs), dtype=np.uint8)
            started = time.perf_counter()
            for row_index in range(current):
                syndrome = det_rows[shots_used + row_index].astype(np.uint8).copy()
                for det, forced in enumerate(forced_syndrome):
                    if forced:
                        syndrome[det] ^= 1
                correction = decoder.decode(syndrome)
                predicted = np.asarray(baseline_observables, dtype=np.uint8)
                for column, enabled in enumerate(correction.tolist()):
                    if not enabled:
                        continue
                    for obs in filtered_observable_columns[column]:
                        predicted[obs] ^= 1
                batch_pred[row_index] = predicted
            total_decode_us += (time.perf_counter() - started) * 1e6

            logical_errors += int(
                np.sum(
                    np.any(
                        batch_pred != obs_rows[shots_used : shots_used + current],
                        axis=1,
                    )
                )
            )
            shots_used += current

        return self._finish(
            bundle=bundle,
            compile_us=compile_us,
            total_decode_us=total_decode_us,
            shots_used=shots_used,
            logical_errors=logical_errors,
        )
