from __future__ import annotations

import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator import build_atom_loss_fixture


PACKAGE_DIR = Path(__file__).resolve().parents[1]
BASELINE = PACKAGE_DIR / "fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
ATOM_LOSS = PACKAGE_DIR / "fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"


class AtomLossFixtureTest(unittest.TestCase):
    def test_per_event_probability_preserves_aggregate_error_rate(self) -> None:
        p = build_atom_loss_fixture.PER_EVENT_PROBABILITY
        self.assertEqual(build_atom_loss_fixture.PER_EVENT_PROBABILITY_TEXT, "0.0003334445062")
        self.assertAlmostEqual(1.0 - (1.0 - p) ** 3, 0.001, places=12)

    def test_transform_inserts_target_matched_independent_loss(self) -> None:
        source = "CX 0 1 2 3\nDEPOLARIZE2(0.001) 0 1 2 3\nTICK\n"
        self.assertEqual(
            build_atom_loss_fixture.transform_circuit(source),
            "CX 0 1 2 3\n"
            "LOSS(0.0003334445062) 0 1 2 3\n"
            "DEPOLARIZE2(0.0003334445062) 0 1 2 3\n"
            "TICK\n",
        )

    def test_transform_rejects_mismatched_two_qubit_noise_targets(self) -> None:
        source = "CX 0 1\nDEPOLARIZE2(0.001) 1 0\n"
        with self.assertRaisesRegex(ValueError, "targets do not match"):
            build_atom_loss_fixture.transform_circuit(source)

    def test_checked_fixture_is_exact_transformation_and_has_no_single_qubit_loss(self) -> None:
        baseline = BASELINE.read_text(encoding="utf-8")
        atom_loss = ATOM_LOSS.read_text(encoding="utf-8")
        self.assertEqual(atom_loss, build_atom_loss_fixture.transform_circuit(baseline))

        lines = atom_loss.splitlines()
        cx_indices = [index for index, line in enumerate(lines) if line.startswith("CX ")]
        self.assertEqual(len(cx_indices), 4)
        for index in cx_indices:
            targets = lines[index].removeprefix("CX ")
            self.assertEqual(lines[index + 1], f"LOSS(0.0003334445062) {targets}")
            self.assertEqual(lines[index + 2], f"DEPOLARIZE2(0.0003334445062) {targets}")
        self.assertFalse(
            any(
                line.startswith("H ") and lines[index + 1].startswith("LOSS(")
                for index, line in enumerate(lines[:-1])
            )
        )


if __name__ == "__main__":
    unittest.main()
