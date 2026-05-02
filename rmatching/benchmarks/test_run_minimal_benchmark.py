import unittest

from benchmarks.run_minimal_benchmark import (
    measure_batch_decode,
    normalize_fault_ids,
    summarize_case,
)


class RunMinimalBenchmarkTest(unittest.TestCase):
    def test_normalize_fault_ids_returns_set(self):
        self.assertEqual(normalize_fault_ids([]), set())
        self.assertEqual(normalize_fault_ids([0, 2]), {0, 2})

    def test_measure_batch_decode_normalizes_once(self):
        decode_calls = 0
        normalize_calls = 0

        def decode_fn(batch):
            nonlocal decode_calls
            decode_calls += 1
            return [list(row) for row in batch]

        def normalize_fn(raw):
            nonlocal normalize_calls
            normalize_calls += 1
            return [["normalized", *row] for row in raw]

        stats = measure_batch_decode(
            decode_fn=decode_fn,
            normalize_fn=normalize_fn,
            syndromes=[[1, 0], [0, 1]],
            warmup_rounds=2,
            measure_rounds=3,
        )

        self.assertEqual(stats["predictions"], [["normalized", 1, 0], ["normalized", 0, 1]])
        self.assertEqual(decode_calls, 1 + 2 + 3)
        self.assertEqual(normalize_calls, 1)

    def test_summarize_case_reports_match_rate_and_mismatches(self):
        row, mismatches = summarize_case(
            case_name="square-4",
            num_detectors=4,
            num_edges=9,
            py_predictions=[[1], [0], [1]],
            rm_predictions=[[1], [1], [1]],
            rm_stats={
                "build_us": 10.0,
                "mean_decode_us": 2.0,
                "median_decode_us": 2.0,
                "p95_decode_us": 3.0,
            },
            syndromes=[[1, 0, 0, 1], [1, 1, 0, 0], [0, 0, 0, 0]],
        )
        self.assertEqual(row["prediction_match_rate"], 2 / 3)
        self.assertEqual(row["mismatch_cases"], 1)
        self.assertEqual(mismatches[0]["syndrome"], [1, 1, 0, 0])


if __name__ == "__main__":
    unittest.main()
