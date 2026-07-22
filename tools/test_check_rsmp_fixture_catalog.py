#!/usr/bin/env python3
from __future__ import annotations

import json
import hashlib
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

    def load_fixture_tree_copy(self) -> tuple[tempfile.TemporaryDirectory[str], Path, Path, dict[str, object]]:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        temp_root = Path(tmpdir.name)
        shutil.copytree(REPO_ROOT / "rstim" / "tests" / "fixtures" / "rsmp", temp_root / "rstim" / "tests" / "fixtures" / "rsmp")
        benchmark_src = REPO_ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / "stim_surface_code_rotated_memory_z_d11_r100.stim"
        benchmark_dst = temp_root / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / benchmark_src.name
        benchmark_dst.parent.mkdir(parents=True)
        shutil.copy2(benchmark_src, benchmark_dst)
        catalog_path = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "catalog.json"
        catalog_data = json.loads(catalog_path.read_text(encoding="utf-8"))
        return tmpdir, temp_root, catalog_path, catalog_data

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

    def test_accepts_non_known_answer_committed_measurement_input(self) -> None:
        _, temp_root, catalog_path, catalog_data = self.load_fixture_tree_copy()
        fixture = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "nonzero_reference.measurements.b8"
        fixture.write_bytes(b"\x01\x01\x01\x01")
        digest = hashlib.sha256(fixture.read_bytes()).hexdigest()
        case = self.find_case(catalog_data, "nonzero_reference")
        case.pop("measurement_generation")
        case["measurement_input"] = {
            "path": "rstim/tests/fixtures/rsmp/nonzero_reference.measurements.b8",
            "format": "b8",
            "bit_count": 1,
            "sha256": digest,
        }
        hashes = case["hashes"]
        assert isinstance(hashes, dict)
        hashes["measurements_b8_sha256"] = digest
        self.write_catalog(catalog_path, catalog_data)
        result = self.run_checker(repo_root=temp_root, catalog=catalog_path)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_valid_case_with_incorrect_measurement_count(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["measurement_count"] = 4
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.measurement_count", result.stderr)

    def test_rejects_duplicate_case_id(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "rank_zero")["id"] = "nonzero_reference"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate case id nonzero_reference", result.stderr)

    def test_rejects_case_path_traversal(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["circuit_path"] = "../known_mpad_multi.stim"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.circuit_path", result.stderr)

    def test_rejects_rank_above_shape_bound(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "rank_zero")["rank_H"] = 1
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("rank_zero.rank_H", result.stderr)

    def test_rejects_demoted_required_known_answer(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "known_mpad_multi")
        case["known_answer"] = False
        case["measurement_generation"] = {
            "command": "stim sample --shots 4 --seed 2 --out_format b8 --in rstim/tests/fixtures/rsmp/known_mpad_multi.stim",
            "format": "b8",
            "bit_count": 3,
        }
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.known_answer", result.stderr)

    def test_rejects_changed_committed_fixture_sha256(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi")["circuit_sha256"] = "0" * 64
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.circuit_sha256", result.stderr)

    def test_rejects_changed_known_answer_bytes_even_when_hashes_match(self) -> None:
        _, temp_root, catalog_path, catalog_data = self.load_fixture_tree_copy()
        fixture = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "known_heralded_erase.detectors.b8"
        fixture.write_bytes(b"\x00\x00\x00\x00")
        digest = hashlib.sha256(fixture.read_bytes()).hexdigest()
        case = self.find_case(catalog_data, "known_heralded_erase")
        expected_files = case["expected_files"]
        assert isinstance(expected_files, dict)
        detectors = expected_files["detectors_b8"]
        assert isinstance(detectors, dict)
        detectors["sha256"] = digest
        hashes = case["hashes"]
        assert isinstance(hashes, dict)
        hashes["detectors_b8_sha256"] = digest
        self.write_catalog(catalog_path, catalog_data)
        result = self.run_checker(repo_root=temp_root, catalog=catalog_path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_heralded_erase.expected_files.detectors_b8.sha256", result.stderr)

    def test_rejects_known_answer_measurement_input_width_mismatch(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "known_mpad_multi")
        measurement_input = case["measurement_input"]
        assert isinstance(measurement_input, dict)
        measurement_input["bit_count"] = 8
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.measurement_input.bit_count", result.stderr)

    def test_rejects_b8_padding_bits(self) -> None:
        _, temp_root, catalog_path, _ = self.load_fixture_tree_copy()
        fixture = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "known_heralded_erase.measurements.b8"
        fixture.write_bytes(b"\x80\x01\x01\x00")
        result = self.run_checker(repo_root=temp_root, catalog=catalog_path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_heralded_erase.measurement_input.padding_bits", result.stderr)

    def test_rejects_missing_known_answer_cross_check(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_case(catalog_data, "known_mpad_multi").pop("stim_cross_check")
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("known_mpad_multi.stim_cross_check", result.stderr)

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

    def test_rejects_duplicate_recipe_id(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "unsupported_version")["id"] = "bad_magic"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate corruption recipe id bad_magic", result.stderr)

    def test_rejects_wrong_unknown_required_feature_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "unknown_required_feature")["expected_code"] = "RSMP_MALFORMED_ARCHIVE"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown_required_feature.expected_code must be RSMP_UNSUPPORTED_FEATURE", result.stderr)

    def test_rejects_required_recipe_mutation_change(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "unknown_required_feature")["mutation"] = "noop"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown_required_feature.mutation", result.stderr)

    def test_rejects_added_required_feature_recipe_wrong_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_required_feature_wrong_mapping",
                "source_role": "nonzero_reference",
                "mutation": "set(global.required_flags, another_unknown_required_feature)",
                "expected_code": "RSMP_IO",
                "recompute": [],
                "validation_boundary": "required feature policy",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("extra_required_feature_wrong_mapping.expected_code must be RSMP_UNSUPPORTED_FEATURE", result.stderr)

    def test_accepts_added_optional_flags_recipe_with_malformed_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_optional_flags_malformed",
                "source_role": "nonzero_reference",
                "mutation": "set(global.optional_flags, 1)",
                "expected_code": "RSMP_MALFORMED_ARCHIVE",
                "recompute": ["global.header_sha256", "trailer.archive_sha256"],
                "validation_boundary": "optional feature policy",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_added_unknown_codec_recipe_wrong_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_unknown_codec_wrong_mapping",
                "source_role": "surface_d11_r100",
                "mutation": "set(block.free_codec_id, 99)",
                "expected_code": "RSMP_IO",
                "recompute": ["trailer.archive_sha256"],
                "validation_boundary": "free codec dispatch",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("extra_unknown_codec_wrong_mapping.expected_code must be RSMP_MALFORMED_ARCHIVE", result.stderr)

    def test_rejects_unknown_symbolic_recipe_selector(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_unknown_symbolic_selector",
                "source_role": "surface_d11_r100",
                "mutation": "set(block.not_a_real_codec_id, 99)",
                "expected_code": "RSMP_MALFORMED_ARCHIVE",
                "recompute": ["trailer.archive_sha256"],
                "validation_boundary": "bogus selector",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("extra_unknown_symbolic_selector.mutation", result.stderr)

    def test_rejects_raw_byte_offset_recompute_selector(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_checksum_raw_recompute",
                "source_role": "surface_d11_r100",
                "mutation": "set(trailer.archive_sha256, alternate_digest)",
                "expected_code": "RSMP_CHECKSUM_MISMATCH",
                "recompute": ["offset(12)"],
                "validation_boundary": "archive checksum",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("extra_checksum_raw_recompute.recompute", result.stderr)

    def test_rejects_unknown_symbolic_recompute_selector(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_unknown_symbolic_recompute",
                "source_role": "surface_d11_r100",
                "mutation": "set(trailer.archive_sha256, alternate_digest)",
                "expected_code": "RSMP_CHECKSUM_MISMATCH",
                "recompute": ["block.not_a_real_recompute_field"],
                "validation_boundary": "archive checksum",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("extra_unknown_symbolic_recompute.recompute", result.stderr)

    def test_rejects_incomplete_payload_recipe_recompute_contract(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "nonzero_padding")["recompute"] = ["trailer.archive_sha256"]
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("nonzero_padding.recompute", result.stderr)

    def test_rejects_incomplete_logical_syndrome_payload_recompute_contract(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.recipes(catalog_data).append(
            {
                "id": "extra_logical_syndrome_payload_missing_recompute",
                "source_role": "surface_d11_r100",
                "mutation": "flip(block.canonical_logical_payload.syndrome_bits.bit)",
                "expected_code": "RSMP_LOGICAL_DIGEST_MISMATCH",
                "recompute": [],
                "validation_boundary": "logical payload digest",
            }
        )
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("extra_logical_syndrome_payload_missing_recompute.recompute", result.stderr)

    def test_rejects_changed_compressed_payload_wrong_mapping(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        self.find_recipe(catalog_data, "changed_compressed_payload")["expected_code"] = "RSMP_CHECKSUM_MISMATCH"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("changed_compressed_payload.expected_code must be RSMP_DECOMPRESSION_FAILED", result.stderr)

    def test_rejects_benchmark_duplicate_fixture_path(self) -> None:
        _, temp_root, catalog_path, catalog_data = self.load_fixture_tree_copy()
        benchmark_src = REPO_ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fixtures" / "stim_surface_code_rotated_memory_z_d11_r100.stim"
        duplicate = temp_root / "rstim" / "tests" / "fixtures" / "rsmp" / "surface_d11_r100_duplicate.stim"
        shutil.copy2(benchmark_src, duplicate)
        case = self.find_case(catalog_data, "surface_d11_r100")
        case["circuit_path"] = "rstim/tests/fixtures/rsmp/surface_d11_r100_duplicate.stim"
        case["circuit_sha256"] = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
        self.write_catalog(catalog_path, catalog_data)
        result = self.run_checker(repo_root=temp_root, catalog=catalog_path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("surface_d11_r100.circuit_path must reference existing benchmark fixture", result.stderr)

    def test_rejects_missing_benchmark_output_evidence(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "surface_d11_r100")
        generation = case["measurement_generation"]
        assert isinstance(generation, dict)
        generation.pop("expected_output_bytes")
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("surface_d11_r100.measurement_generation.expected_output_bytes", result.stderr)

    def test_rejects_benchmark_output_hash_mismatch(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "surface_d11_r100")
        generation = case["measurement_generation"]
        assert isinstance(generation, dict)
        generation["sha256"] = "0" * 64
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("surface_d11_r100.measurement_generation.sha256", result.stderr)

    def test_rejects_loss_visible_invalid_stim_producer(self) -> None:
        _, catalog_copy, catalog_data = self.load_catalog_copy()
        case = self.find_case(catalog_data, "loss_visible_measurements")
        generation = case["measurement_generation"]
        assert isinstance(generation, dict)
        generation["command"] = "stim sample --shots 4 --seed 2 --out_format b8 --in rstim/tests/fixtures/rsmp/loss_visible_measurements.stim"
        self.write_catalog(catalog_copy, catalog_data)
        result = self.run_checker(catalog=catalog_copy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("loss_visible_measurements.measurement_generation.command", result.stderr)

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
