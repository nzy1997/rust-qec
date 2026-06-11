import csv
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.surface_decoder_compare.run_compare import (
    _filter_case_specs,
    _load_existing_rows,
    _merge_rows,
    _parse_csv_floats,
    _parse_csv_ints,
    main,
    run_suite,
    write_results,
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
    def test_new_surface_benchmark_spec_exists(self) -> None:
        self.assertTrue(Path("benchmarks/surface_decoder/spec.toml").exists())

    def test_python_runner_entrypoint_exists(self) -> None:
        self.assertTrue(Path("benchmarks/python_runners/surface_decoder/run.py").exists())

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

    def test_run_suite_uses_focused_full_defaults(self) -> None:
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
            rows = run_suite(
                tier_name="full",
                output_dir=Path(tmpdir),
                seed=12345,
                drivers={"alpha": FakeDriver("alpha")},
                case_bundle_factory=fake_bundle_factory,
                batch_size=32,
            )

        self.assertEqual(len(rows), 6)
        self.assertEqual({row.distance for row in rows}, {3, 5})
        self.assertEqual({row.p for row in rows}, {0.002, 0.005, 0.01})
        self.assertTrue(all(row.shots_budget == 10_000 for row in rows))
        self.assertTrue(all(row.errors_budget == 200 for row in rows))

    def test_parse_csv_helpers_return_none_for_empty_values(self) -> None:
        self.assertIsNone(_parse_csv_ints(None))
        self.assertIsNone(_parse_csv_ints(""))
        self.assertIsNone(_parse_csv_floats(None))
        self.assertIsNone(_parse_csv_floats(""))

    def test_merge_rows_replaces_matching_keys_and_preserves_others(self) -> None:
        existing_rows = [
            {
                "tier": "full",
                "decoder": "ldpc",
                "backend": "native",
                "distance": "3",
                "rounds": "3",
                "p": "0.002",
                "seed": "12345",
                "num_dets": "24",
                "num_obs": "1",
                "shots_budget": "10000",
                "errors_budget": "200",
                "shots_used": "10000",
                "logical_errors": "11",
                "logical_error_rate": "0.0011",
                "compile_us": "10.0",
                "total_decode_us": "20.0",
                "decode_us_per_shot": "0.002",
                "status": "ok",
                "error": "",
            },
            {
                "tier": "full",
                "decoder": "pymatching",
                "backend": "native",
                "distance": "3",
                "rounds": "3",
                "p": "0.002",
                "seed": "12345",
                "num_dets": "24",
                "num_obs": "1",
                "shots_budget": "10000",
                "errors_budget": "200",
                "shots_used": "10000",
                "logical_errors": "2",
                "logical_error_rate": "0.0002",
                "compile_us": "5.0",
                "total_decode_us": "6.0",
                "decode_us_per_shot": "0.0006",
                "status": "ok",
                "error": "",
            },
        ]
        new_rows = [
            ResultRow(
                tier="full",
                decoder="ldpc",
                backend="gurobi",
                distance=3,
                rounds=3,
                p=0.002,
                seed=12345,
                num_dets=24,
                num_obs=1,
                shots_budget=10000,
                errors_budget=200,
                shots_used=8192,
                logical_errors=7,
                logical_error_rate=7 / 8192,
                compile_us=12.0,
                total_decode_us=24.0,
                decode_us_per_shot=24.0 / 8192,
                status="error",
                error="solver failed",
            )
        ]

        merged = _merge_rows(existing_rows, new_rows)

        self.assertEqual(len(merged), 2)
        self.assertEqual([row["decoder"] for row in merged], ["ldpc", "pymatching"])
        self.assertEqual(merged[0]["backend"], "gurobi")
        self.assertEqual(merged[0]["status"], "error")
        self.assertEqual(merged[0]["error"], "solver failed")
        self.assertEqual(merged[1]["decoder"], "pymatching")

    def test_load_existing_rows_returns_empty_when_file_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            missing = Path(tmpdir) / "missing.csv"
            self.assertEqual(_load_existing_rows(missing), [])

    def test_main_rejects_merge_without_decoders(self) -> None:
        with self.assertRaises(SystemExit) as ctx:
            main(["--tier", "full", "--merge-into-existing"])
        self.assertNotEqual(ctx.exception.code, 0)

    @mock.patch("benchmarks.surface_decoder_compare.run_compare.run_suite")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_case_specs")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_driver_registry")
    def test_main_filters_decoders_and_dispatches(
        self,
        registry_mock: mock.Mock,
        case_specs_mock: mock.Mock,
        run_suite_mock: mock.Mock,
    ) -> None:
        registry_mock.return_value = {"alpha": object(), "beta": object()}
        case_specs_mock.return_value = [
            CaseSpec(distance=3, rounds=3, p=0.002),
            CaseSpec(distance=5, rounds=5, p=0.005),
        ]

        exit_code = main(
            [
                "--tier",
                "smoke",
                "--decoders",
                "beta",
                "--distances",
                "5",
                "--p-values",
                "0.005",
            ]
        )

        self.assertEqual(exit_code, 0)
        case_specs_mock.assert_called_once_with(tier_name="smoke")
        run_suite_mock.assert_called_once()
        kwargs = run_suite_mock.call_args.kwargs
        self.assertEqual(kwargs["tier_name"], "smoke")
        self.assertEqual(set(kwargs["drivers"]), {"beta"})
        self.assertEqual(
            kwargs["case_specs"],
            [CaseSpec(distance=5, rounds=5, p=0.005)],
        )

    @mock.patch("benchmarks.surface_decoder_compare.run_compare.write_results")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare._merge_rows")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare._load_existing_rows")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.run_suite")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_case_specs")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_driver_registry")
    def test_main_merges_into_existing_results_when_requested(
        self,
        registry_mock: mock.Mock,
        case_specs_mock: mock.Mock,
        run_suite_mock: mock.Mock,
        load_existing_rows_mock: mock.Mock,
        merge_rows_mock: mock.Mock,
        write_results_mock: mock.Mock,
    ) -> None:
        registry_mock.return_value = {"ldpc": object(), "rbposd": object()}
        case_specs_mock.return_value = [CaseSpec(distance=3, rounds=3, p=0.002)]
        run_suite_mock.return_value = [
            ResultRow(
                tier="full",
                decoder="ldpc",
                backend="native",
                distance=3,
                rounds=3,
                p=0.002,
                seed=12345,
                num_dets=24,
                num_obs=1,
                shots_budget=10000,
                errors_budget=200,
                shots_used=10000,
                logical_errors=3,
                logical_error_rate=0.0003,
                compile_us=1.0,
                total_decode_us=2.0,
                decode_us_per_shot=0.0002,
                status="ok",
                error="",
            )
        ]
        load_existing_rows_mock.return_value = [{"decoder": "pymatching"}]
        merge_rows_mock.return_value = [{"decoder": "ldpc"}, {"decoder": "pymatching"}]

        exit_code = main(
            [
                "--tier",
                "full",
                "--decoders",
                "ldpc",
                "--merge-into-existing",
            ]
        )

        self.assertEqual(exit_code, 0)
        write_results_mock.assert_called_once_with(
            merge_rows_mock.return_value,
            Path("benchmarks/surface_decoder_compare/results/full/results.csv"),
        )
        load_existing_rows_mock.assert_called_once_with(
            Path("benchmarks/surface_decoder_compare/results/full/results.csv")
        )
        merge_rows_mock.assert_called_once_with(
            load_existing_rows_mock.return_value,
            run_suite_mock.return_value,
        )

    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_case_specs")
    @mock.patch("benchmarks.surface_decoder_compare.run_compare.build_driver_registry")
    def test_main_merge_mode_preserves_existing_rows_even_if_run_suite_writes_subset(
        self,
        registry_mock: mock.Mock,
        case_specs_mock: mock.Mock,
    ) -> None:
        registry_mock.return_value = {"ldpc": object()}
        case_specs_mock.return_value = [CaseSpec(distance=3, rounds=3, p=0.002)]

        existing_rows = [
            {
                "tier": "full",
                "decoder": "pymatching",
                "backend": "native",
                "distance": "3",
                "rounds": "3",
                "p": "0.002",
                "seed": "12345",
                "num_dets": "24",
                "num_obs": "1",
                "shots_budget": "10000",
                "errors_budget": "200",
                "shots_used": "10000",
                "logical_errors": "2",
                "logical_error_rate": "0.0002",
                "compile_us": "5.0",
                "total_decode_us": "6.0",
                "decode_us_per_shot": "0.0006",
                "status": "ok",
                "error": "",
            }
        ]
        new_rows = [
            ResultRow(
                tier="full",
                decoder="ldpc",
                backend="native",
                distance=3,
                rounds=3,
                p=0.002,
                seed=12345,
                num_dets=24,
                num_obs=1,
                shots_budget=10000,
                errors_budget=200,
                shots_used=10000,
                logical_errors=3,
                logical_error_rate=0.0003,
                compile_us=1.0,
                total_decode_us=2.0,
                decode_us_per_shot=0.0002,
                status="ok",
                error="",
            )
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            output_dir = Path(tmpdir) / "results"
            results_path = output_dir / "full" / "results.csv"
            write_results(existing_rows, results_path)

            def fake_run_suite(**kwargs):
                if kwargs["write_output"]:
                    write_results(new_rows, results_path)
                return new_rows

            with mock.patch(
                "benchmarks.surface_decoder_compare.run_compare.run_suite",
                side_effect=fake_run_suite,
            ):
                exit_code = main(
                    [
                        "--tier",
                        "full",
                        "--output-dir",
                        str(output_dir),
                        "--decoders",
                        "ldpc",
                        "--merge-into-existing",
                    ]
                )

            self.assertEqual(exit_code, 0)
            with results_path.open() as handle:
                merged_rows = list(csv.DictReader(handle))
            self.assertEqual(
                [row["decoder"] for row in merged_rows],
                ["ldpc", "pymatching"],
            )


if __name__ == "__main__":
    unittest.main()
