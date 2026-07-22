#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rsmp_fixture_catalog.py"
CATALOG = REPO_ROOT / "rstim" / "tests" / "fixtures" / "rsmp" / "catalog.json"
PASS_LINE = "PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12"


class RsmpFixtureCatalogCheckerTest(unittest.TestCase):
    def run_checker(self, *, repo_root: Path = REPO_ROOT, catalog: Path = CATALOG) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--repo-root", str(repo_root), "--catalog", str(catalog)],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def load_catalog_copy(self) -> tuple[tempfile.TemporaryDirectory[str], Path, dict[str, object]]:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        catalog_copy = Path(tmpdir.name) / "catalog.json"
        catalog_data = json.loads(CATALOG.read_text(encoding="utf-8"))
        catalog_copy.write_text(json.dumps(catalog_data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return tmpdir, catalog_copy, catalog_data

    def write_catalog(self, path: Path, catalog_data: dict[str, object]) -> None:
        path.write_text(json.dumps(catalog_data, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def cases(self, catalog_data: dict[str, object]) -> list[dict[str, object]]:
        cases = catalog_data["cases"]
        assert isinstance(cases, list)
        return cases  # type: ignore[return-value]

    def recipes(self, catalog_data: dict[str, object]) -> list[dict[str, object]]:
        recipes = catalog_data["corruption_recipes"]
        assert isinstance(recipes, list)
        return recipes  # type: ignore[return-value]

    def find_case(self, catalog_data: dict[str, object], case_id: str) -> dict[str, object]:
        for case in self.cases(catalog_data):
            if case.get("id") == case_id:
                return case
        raise AssertionError(f"missing case {case_id}")

    def find_recipe(self, catalog_data: dict[str, object], recipe_id: str) -> dict[str, object]:
        for recipe in self.recipes(catalog_data):
            if recipe.get("id") == recipe_id:
                return recipe
        raise AssertionError(f"missing recipe {recipe_id}")

    def test_accepts_repository_catalog(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, PASS_LINE + "\n")

    def test_rejects_valid_case_with_incorrect_measurement_count(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["measurement_count"] = 4
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.measurement_count", result.stderr)

    def test_rejects_changed_committed_fixture_sha256(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["circuit_sha256"] = "0" * 64
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.circuit_sha256", result.stderr)

    def test_rejects_removed_required_semantic_role(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "rank_zero")
        case["semantic_roles"] = []
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing semantic role rank_zero", result.stderr)

    def test_rejects_corruption_recipe_without_expected_code(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "bad_magic")["expected_code"] = ""
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bad_magic.expected_code", result.stderr)

    def test_rejects_raw_byte_offset_recipe_selector(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "bad_magic")["mutation"] = "set(byte_offset:0, 0)"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bad_magic.mutation", result.stderr)

    def test_rejects_wrong_unknown_required_feature_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "unknown_required_feature")["expected_code"] = "RSMP_MALFORMED_ARCHIVE"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown_required_feature.expected_code must be RSMP_UNSUPPORTED_FEATURE", result.stderr)

    def test_rejects_benchmark_duplicate_fixture_path(self) -> None:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        temp_root = Path(tmpdir.name)
        shutil.copytree(REPO_ROOT / "rstim" / "tests" / "fixtures" / "rsmp", temp_root / "rstim" / "tests" / "fixtures" / "rsmp")
        benchmark_src = REPO_ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / "stim_surface_code_rotated_memory_z_d11_r100.stim"
        benchmark_dst = temp_root / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / benchmark_src.name
        benchmark_dst.parent.mkdir(parents=True)
        shutil.copy2(benchmark_src, benchmark_dst)
        duplicate = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "surface_d11_r100_duplicate.stim"
        shutil.copy2(benchmark_src, duplicate)
        catalog_path = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "catalog.json"
        catalog_data = json.loads(catalog_path.read_text(encoding="utf-8"))
        case = self.find_case(catalog_data, "surface_d11_r100")
        case["circuit_path"] = "rstim/tests/fixtures/rsmp/surface_d11_r100_duplicate.stim"
        case["circuit_sha256"] = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
        self.write_catalog(catalog_path, catalog_data)
        result = self.run_checker(repo_root=temp_root, catalog=catalog_path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("surface_d11_r100.circuit_path must reference existing benchmark fixture", result.stderr)

    def test_rejects_removed_required_known_answer(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        catalog_data["cases"] = [case for case in self.cases(catalog_data) if case.get("id") != "known_mpp_multi_product"]
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing known-answer case known_mpp_multi_product", result.stderr)

    def test_rejects_changed_known_answer_expected_sha256(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "known_heralded_erase")
        expected_files = case["expected_files"]
        assert isinstance(expected_files, dict)
        measurements = expected_files["measurements_b8"]
        assert isinstance(measurements, dict)
        measurements["sha256"] = "f" * 64
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_heralded_erase.expected_files.measurements_b8.sha256", result.stderr)


if __name__ == "__main__":
    unittest.main()
