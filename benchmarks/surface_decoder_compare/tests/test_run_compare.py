import csv
import tempfile
import unittest
from pathlib import Path

from benchmarks.surface_decoder_compare.run_compare import (
    _filter_case_specs,
    _parse_csv_floats,
    _parse_csv_ints,
    run_suite,
)
from benchmarks.surface_decoder_compare.schema import (
    CaseBundle,
    CaseSpec,
    ResultRow,
    TIER_CONFIGS,
)


class FakeDriver:
    def __init__(self, name: str) -> None:
        self.name = name

    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        return ResultRow(
            tier=bundle.tier.name,
            decoder=self.name,
            backend="native",
            distance=bundle.spec.distance,
            rounds=bundle.spec.rounds,
            p=bundle.spec.p,
            seed=bundle.seed,
            num_dets=bundle.num_dets,
            num_obs=bundle.num_obs,
            shots_budget=bundle.tier.max_shots,
            errors_budget=bundle.tier.max_errors,
            shots_used=10,
            logical_errors=2,
            logical_error_rate=0.2,
            compile_us=100.0,
            total_decode_us=200.0,
            decode_us_per_shot=20.0,
            status="ok",
            error="",
        )


class FailingDriver:
    def __init__(self, name: str) -> None:
        self.name = name

    def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
        raise RuntimeError(f"{self.name} exploded")


class RunCompareTest(unittest.TestCase):
    def test_cli_filters_parse_and_apply(self) -> None:
        specs = [
            CaseSpec(distance=3, rounds=3, p=0.001),
            CaseSpec(distance=5, rounds=5, p=0.002),
            CaseSpec(distance=7, rounds=7, p=0.003),
        ]
        self.assertEqual(_parse_csv_ints("3,7"), {3, 7})
        self.assertEqual(_parse_csv_floats("0.001,0.003"), {0.001, 0.003})
        filtered = _filter_case_specs(specs, {3, 7}, {0.003})
        self.assertEqual(filtered, [CaseSpec(distance=7, rounds=7, p=0.003)])

    def test_run_suite_writes_one_row_per_case_and_driver(self) -> None:
        specs = [
            CaseSpec(distance=3, rounds=3, p=0.001),
            CaseSpec(distance=5, rounds=5, p=0.002),
        ]

        def fake_bundle_factory(root: Path, spec: CaseSpec, tier, seed: int) -> CaseBundle:
            return CaseBundle(
                spec=spec,
                tier=tier,
                seed=seed,
                num_dets=2,
                num_obs=1,
                num_shots=tier.max_shots,
                circuit_path=root / f"{spec.slug}.stim",
                dem_path=root / f"{spec.slug}.dem",
                dets_b8_path=root / f"{spec.slug}.dets.b8",
                obs_b8_path=root / f"{spec.slug}.obs.b8",
                metadata_path=root / f"{spec.slug}.json",
            )

        with tempfile.TemporaryDirectory() as tmpdir:
            output_dir = Path(tmpdir)
            rows = run_suite(
                tier_name="smoke",
                output_dir=output_dir,
                seed=12345,
                drivers={
                    "alpha": FakeDriver("alpha"),
                    "beta": FakeDriver("beta"),
                    "gamma": FailingDriver("gamma"),
                },
                case_specs=specs,
                case_bundle_factory=fake_bundle_factory,
                batch_size=32,
            )

            self.assertEqual(len(rows), 6)
            results_path = output_dir / "smoke" / "results.csv"
            with results_path.open() as handle:
                written = list(csv.DictReader(handle))
            self.assertEqual(len(written), 6)
            self.assertEqual(written[0]["tier"], "smoke")
            self.assertIn(written[0]["decoder"], {"alpha", "beta", "gamma"})
            error_rows = [row for row in written if row["status"] == "error"]
            self.assertEqual(len(error_rows), 2)
            self.assertTrue(all("exploded" in row["error"] for row in error_rows))


if __name__ == "__main__":
    unittest.main()
