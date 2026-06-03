import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import numpy as np

from benchmarks.surface_decoder_compare.drivers import build_driver_registry
from benchmarks.surface_decoder_compare.drivers.base import BenchmarkDriver
from benchmarks.surface_decoder_compare.drivers.ilpqec_driver import IlpqecDriver
from benchmarks.surface_decoder_compare.drivers.ilpqec_driver import _preferred_solver
from benchmarks.surface_decoder_compare.drivers.pymatching_driver import PymatchingDriver
from benchmarks.surface_decoder_compare.drivers.rust_bridge import (
    RustBridgeDriver,
    _gurobi_env_available,
    _load_bridge_payload,
)
from benchmarks.surface_decoder_compare.schema import CaseBundle, CaseSpec, TIER_CONFIGS


class DriverRegistryTest(unittest.TestCase):
    def test_registry_exposes_all_six_decoders(self) -> None:
        registry = build_driver_registry()
        self.assertEqual(
            set(registry),
            {"pymatching", "ilpqec", "ldpc", "rmatching", "rbposd", "rilpqec"},
        )

    @mock.patch("subprocess.run")
    def test_rust_bridge_driver_parses_json_response(self, run_mock: mock.Mock) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["bridge"],
            returncode=0,
            stdout=json.dumps(
                {
                    "status": "ok",
                    "decoder": "rmatching",
                    "backend": "native",
                    "shots_used": 10,
                    "logical_errors": 2,
                    "compile_us": 100.0,
                    "total_decode_us": 200.0,
                    "error": "",
                }
            ),
            stderr="",
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            dem_path = root / "model.dem"
            dets_path = root / "detections.b8"
            obs_path = root / "observables.b8"
            metadata_path = root / "metadata.json"
            circuit_path = root / "circuit.stim"

            dem_path.write_text("error(0.125) D0 L0\nerror(0.25) D1\n")
            circuit_path.write_text("M 0\n")
            np.asarray([[0b0000_0001]], dtype=np.uint8).tofile(dets_path)
            np.asarray([[0b0000_0001]], dtype=np.uint8).tofile(obs_path)
            metadata_path.write_text("{}")

            bundle = CaseBundle(
                spec=CaseSpec(distance=3, rounds=3, p=0.001),
                tier=TIER_CONFIGS["smoke"],
                seed=12345,
                num_dets=2,
                num_obs=1,
                num_shots=1,
                circuit_path=circuit_path,
                dem_path=dem_path,
                dets_b8_path=dets_path,
                obs_b8_path=obs_path,
                metadata_path=metadata_path,
            )

            row = RustBridgeDriver("rmatching").run_case(bundle, batch_size=32)

        self.assertEqual(row.decoder, "rmatching")
        self.assertEqual(row.backend, "native")
        self.assertEqual(row.shots_used, 10)
        self.assertEqual(row.logical_errors, 2)
        self.assertEqual(row.compile_us, 100.0)
        self.assertEqual(row.total_decode_us, 200.0)

    @mock.patch("subprocess.run")
    def test_rust_bridge_driver_parses_json_after_gurobi_banner(
        self, run_mock: mock.Mock
    ) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["bridge"],
            returncode=0,
            stdout=(
                "Set parameter Username\n"
                "Academic license - for non-commercial use only\n"
                + json.dumps(
                    {
                        "status": "ok",
                        "decoder": "rilpqec",
                        "backend": "gurobi",
                        "shots_used": 10,
                        "logical_errors": 2,
                        "compile_us": 100.0,
                        "total_decode_us": 200.0,
                        "error": "",
                    }
                )
                + "\n"
            ),
            stderr="",
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            row = RustBridgeDriver("rilpqec").run_case(bundle, batch_size=32)

        self.assertEqual(row.backend, "gurobi")
        self.assertEqual(row.shots_used, 10)

    @mock.patch.dict(
        os.environ,
        {
            "GUROBI_HOME": "/tmp/gurobi",
            "GRB_LICENSE_FILE": "/tmp/gurobi.lic",
        },
        clear=False,
    )
    @mock.patch("subprocess.run")
    def test_rust_bridge_driver_enables_gurobi_feature_when_env_present(
        self, run_mock: mock.Mock
    ) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["bridge"],
            returncode=0,
            stdout=json.dumps(
                {
                    "status": "ok",
                    "decoder": "rilpqec",
                    "backend": "gurobi",
                    "shots_used": 1,
                    "logical_errors": 0,
                    "compile_us": 100.0,
                    "total_decode_us": 200.0,
                    "error": "",
                }
            ),
            stderr="",
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            RustBridgeDriver("rilpqec").run_case(bundle, batch_size=32)

        command = run_mock.call_args.args[0]
        self.assertIn("--features", command)
        self.assertIn("gurobi", command)

    def test_pymatching_driver_decodes_a_synthetic_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            row = PymatchingDriver().run_case(bundle, batch_size=1)

        self.assertEqual(row.decoder, "pymatching")
        self.assertEqual(row.backend, "native")
        self.assertEqual(row.shots_used, 1)
        self.assertEqual(row.logical_errors, 0)

    def test_ilpqec_driver_decodes_a_synthetic_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            row = IlpqecDriver().run_case(bundle, batch_size=1)

        self.assertEqual(row.decoder, "ilpqec")
        self.assertIn(row.backend, {"highs", "gurobi"})
        self.assertEqual(row.shots_used, 1)
        self.assertEqual(row.logical_errors, 0)

    def test_load_bridge_payload_rejects_empty_stdout(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "empty stdout"):
            _load_bridge_payload("")

    def test_load_bridge_payload_rejects_missing_json(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "did not emit a JSON payload"):
            _load_bridge_payload("Set parameter Username")

    @mock.patch("subprocess.run")
    def test_rust_bridge_driver_raises_on_nonzero_exit(self, run_mock: mock.Mock) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["bridge"],
            returncode=1,
            stdout="",
            stderr="bridge failed",
        )
        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            with self.assertRaisesRegex(RuntimeError, "bridge failed"):
                RustBridgeDriver("rmatching").run_case(bundle, batch_size=32)

    @mock.patch("subprocess.run")
    def test_rust_bridge_driver_raises_on_error_payload(self, run_mock: mock.Mock) -> None:
        run_mock.return_value = subprocess.CompletedProcess(
            args=["bridge"],
            returncode=0,
            stdout=json.dumps(
                {
                    "status": "error",
                    "decoder": "rilpqec",
                    "backend": "gurobi",
                    "shots_used": 0,
                    "logical_errors": 0,
                    "compile_us": 0.0,
                    "total_decode_us": 0.0,
                    "error": "solver exploded",
                }
            ),
            stderr="",
        )
        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            with self.assertRaisesRegex(RuntimeError, "solver exploded"):
                RustBridgeDriver("rilpqec").run_case(bundle, batch_size=32)

    @mock.patch.dict(os.environ, {}, clear=True)
    def test_gurobi_env_available_requires_both_vars(self) -> None:
        self.assertFalse(_gurobi_env_available())

    @mock.patch("benchmarks.surface_decoder_compare.drivers.ilpqec_driver.get_available_solvers")
    def test_preferred_solver_falls_back_to_highs(self, solvers_mock: mock.Mock) -> None:
        solvers_mock.return_value = ["highs"]
        self.assertEqual(_preferred_solver(), "highs")

    def test_base_finish_handles_zero_shots(self) -> None:
        class MinimalDriver(BenchmarkDriver):
            name = "minimal"

            def run_case(self, bundle: CaseBundle, batch_size: int) -> ResultRow:
                raise NotImplementedError

        driver = MinimalDriver()
        with tempfile.TemporaryDirectory() as tmpdir:
            bundle = make_synthetic_bundle(Path(tmpdir))
            row = driver._finish(
                bundle=bundle,
                compile_us=1.0,
                total_decode_us=2.0,
                shots_used=0,
                logical_errors=0,
            )
        self.assertEqual(row.logical_error_rate, 0.0)
        self.assertEqual(row.decode_us_per_shot, 0.0)


def make_synthetic_bundle(root: Path) -> CaseBundle:
    dem_path = root / "model.dem"
    dets_path = root / "detections.b8"
    obs_path = root / "observables.b8"
    metadata_path = root / "metadata.json"
    circuit_path = root / "circuit.stim"

    dem_path.write_text("error(0.125) D0 L0\nerror(0.25) D1\n")
    circuit_path.write_text("M 0\n")
    np.asarray([[0b0000_0001]], dtype=np.uint8).tofile(dets_path)
    np.asarray([[0b0000_0001]], dtype=np.uint8).tofile(obs_path)
    metadata_path.write_text("{}")

    return CaseBundle(
        spec=CaseSpec(distance=3, rounds=3, p=0.001),
        tier=TIER_CONFIGS["smoke"],
        seed=12345,
        num_dets=2,
        num_obs=1,
        num_shots=1,
        circuit_path=circuit_path,
        dem_path=dem_path,
        dets_b8_path=dets_path,
        obs_b8_path=obs_path,
        metadata_path=metadata_path,
    )


if __name__ == "__main__":
    unittest.main()
