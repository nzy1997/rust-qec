from __future__ import annotations

import unittest

from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    compare_sample_sets,
    inject_bitflip,
    parse_01_samples,
    select_columns,
    select_pairs,
)


class VerifyCorrectnessHelpersTest(unittest.TestCase):
    def test_parse_01_samples_requires_rectangular_output(self) -> None:
        self.assertEqual(
            parse_01_samples("01\n10\n", expected_bits=2, expected_shots=2),
            [[0, 1], [1, 0]],
        )
        with self.assertRaisesRegex(ValueError, "expected 2 bits"):
            parse_01_samples("0\n11\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "expected 2 shots"):
            parse_01_samples("01\n", expected_bits=2, expected_shots=2)

    def test_selectors_include_observable_tail_even_when_limited(self) -> None:
        columns = select_columns(8, observable_count=2, limit=1)
        self.assertEqual(columns, [0, 6, 7])

    def test_selectors_include_observable_tail_and_pairs(self) -> None:
        columns = select_columns(25, observable_count=2, limit=10)
        self.assertIn(0, columns)
        self.assertIn(23, columns)
        self.assertIn(24, columns)
        pairs = select_pairs(columns, bit_count=25, observable_count=2, limit=10)
        self.assertTrue(any(pair[1] >= 23 for pair in pairs))

    def test_compare_sample_sets_accepts_close_rates(self) -> None:
        stim = [[0, 1], [1, 1], [0, 0], [1, 0]]
        rstim = [[0, 1], [1, 1], [0, 0], [1, 0]]
        result = compare_sample_sets(stim, rstim, columns=[0, 1], pairs=[(0, 1)])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 4)

    def test_compare_sample_sets_flags_large_mismatch(self) -> None:
        stim = [[0] for _ in range(100)]
        rstim = [[1] for _ in range(100)]
        result = compare_sample_sets(stim, rstim, columns=[0], pairs=[])
        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertGreater(result["max_delta"], result["max_tolerance"])

    def test_inject_bitflip_is_deterministic_and_changes_bits(self) -> None:
        samples = [[0, 0], [1, 1]]
        self.assertEqual(
            inject_bitflip(samples, rate=1.0, seed=7),
            [[1, 1], [0, 0]],
        )
        self.assertEqual(samples, [[0, 0], [1, 1]])


if __name__ == "__main__":
    unittest.main()
