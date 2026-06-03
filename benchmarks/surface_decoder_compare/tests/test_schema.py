import unittest

from benchmarks.surface_decoder_compare.schema import (
    CSV_HEADER,
    DEFAULT_BATCH_SIZE,
    DEFAULT_DISTANCES,
    DEFAULT_P_VALUES,
    FULL_DISTANCES,
    FULL_P_VALUES,
    TIER_CONFIGS,
)


class SchemaTest(unittest.TestCase):
    def test_sweep_and_tiers_are_pinned(self) -> None:
        self.assertEqual(DEFAULT_DISTANCES, (3,))
        self.assertEqual(
            DEFAULT_P_VALUES,
            (0.002, 0.005, 0.010),
        )
        self.assertEqual(FULL_DISTANCES, (3, 5))
        self.assertEqual(FULL_P_VALUES, (0.002, 0.005, 0.010))
        self.assertEqual(DEFAULT_BATCH_SIZE, 256)
        self.assertEqual(TIER_CONFIGS["smoke"].max_shots, 2_000)
        self.assertEqual(TIER_CONFIGS["smoke"].max_errors, 20)
        self.assertEqual(TIER_CONFIGS["full"].max_shots, 10_000)
        self.assertEqual(TIER_CONFIGS["full"].max_errors, 200)

    def test_csv_header_keeps_accuracy_and_timing_columns(self) -> None:
        self.assertEqual(
            CSV_HEADER[:8],
            [
                "tier",
                "decoder",
                "backend",
                "distance",
                "rounds",
                "p",
                "seed",
                "num_dets",
            ],
        )
        self.assertIn("logical_error_rate", CSV_HEADER)
        self.assertIn("compile_us", CSV_HEADER)
        self.assertIn("total_decode_us", CSV_HEADER)
        self.assertIn("decode_us_per_shot", CSV_HEADER)


if __name__ == "__main__":
    unittest.main()
