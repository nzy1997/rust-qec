#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_rsmp_v1_compression_evidence as compression_checker


CHECKER = REPO_ROOT / "tools" / "check_rsmp_v1_readiness.py"
PASS_LINE = (
    "PASS rsmp v1 readiness valid_cases=7 corruption_cases>=12 "
    "compatibility=1 compression=pass"
)
COMPRESSION_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/rsmp-v1")


class RsmpV1ReadinessNegativeControls(unittest.TestCase):
    def test_rejects_missing_compression_input_hash(self) -> None:
        self.expect_mutation_failure(
            mutate=self.remove_first_artifact_hash,
            expected="not ready: compression repository input hash is missing",
        )

    def test_rejects_failed_compression_gate(self) -> None:
        self.expect_mutation_failure(
            mutate=self.increase_benchmark_archive_bytes,
            expected="not ready: compression acceptance gate failed",
        )

    def test_rejects_missing_sweep_unsupported_normative_statement(self) -> None:
        self.expect_mutation_failure(
            mutate=self.remove_sweep_support_boundary,
            expected=(
                "not ready: normative documentation does not mark "
                "sweep-bit circuits unsupported"
            ),
        )

    def test_rejects_documented_cli_surface_drift(self) -> None:
        self.expect_mutation_failure(
            mutate=self.rename_documented_verify_only_option,
            expected="not ready: documented CLI surface differs from rstim help",
        )

    def expect_mutation_failure(
        self,
        *,
        mutate: Callable[[Path], None],
        expected: str,
    ) -> None:
        with tempfile.TemporaryDirectory(prefix="rstim-rsmp-readiness-test-") as raw_tmp:
            temp_root = Path(raw_tmp) / "repo"
            artifact_dir = Path(raw_tmp) / "out"
            self.copy_required_inputs(temp_root)
            mutate(temp_root)

            result = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER),
                    "--repo-root",
                    str(temp_root),
                    "--out-dir",
                    str(artifact_dir),
                    "--skip-commands",
                ],
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            combined = result.stdout + result.stderr
            self.assertIn(expected, combined)
            self.assertNotIn(PASS_LINE, combined)
            artifact = json.loads((artifact_dir / "readiness.json").read_text())
            self.assertEqual(artifact["status"], "fail")
            self.assertTrue(artifact["failed_checks"], artifact)

    def copy_required_inputs(self, temp_root: Path) -> None:
        temp_root.mkdir(parents=True)
        shutil.copy2(REPO_ROOT / "Cargo.lock", temp_root / "Cargo.lock")
        self.copy_tree("rstim/tests/fixtures/rsmp", temp_root)
        self.copy_tree("rstim/doc", temp_root)
        self.copy_tree(COMPRESSION_DIR, temp_root)
        benchmark = Path(
            "benchmarks/rstim_vs_stim_simulator/fixtures/"
            "stim_surface_code_rotated_memory_z_d11_r100.stim"
        )
        self.copy_file(benchmark, temp_root)

    def copy_tree(self, relative: str | Path, temp_root: Path) -> None:
        source = REPO_ROOT / relative
        target = temp_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, target)

    def copy_file(self, relative: str | Path, temp_root: Path) -> None:
        source = REPO_ROOT / relative
        target = temp_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)

    def remove_first_artifact_hash(self, temp_root: Path) -> None:
        path = temp_root / COMPRESSION_DIR / "artifact-sha256.json"
        hashes = self.read_json(path)
        hashes.pop("raw.jsonl")
        self.write_json(path, hashes)

    def increase_benchmark_archive_bytes(self, temp_root: Path) -> None:
        bundle = temp_root / COMPRESSION_DIR
        records = [
            json.loads(line)
            for line in (bundle / "raw.jsonl").read_text(encoding="utf-8").splitlines()
        ]
        records[compression_checker.BENCHMARK_ROW_INDEX]["rsmp_archive"]["bytes"] = (
            compression_checker.PINNED_BENCHMARK_RAW_BYTES // 5 + 1
        )
        (bundle / "raw.jsonl").write_text(
            "".join(json.dumps(row, sort_keys=True) + "\n" for row in records),
            encoding="utf-8",
        )
        summary = compression_checker.derive_summary(records)
        self.write_json(bundle / "summary.json", summary)
        (bundle / "report.md").write_text(
            compression_checker.render_report(summary),
            encoding="utf-8",
        )
        self.write_json(
            bundle / "artifact-sha256.json",
            {
                name: compression_checker.sha256_file(bundle / name)
                for name in ("raw.jsonl", "summary.json", "report.md", "environment.json")
            },
        )

    def remove_sweep_support_boundary(self, temp_root: Path) -> None:
        path = temp_root / "rstim/doc/rsmp-v1.md"
        text = path.read_text(encoding="utf-8")
        required = (
            "Sweep-bit circuits are unsupported in v1 and must fail with "
            "`RSMP_UNSUPPORTED_SWEEP` before archive bytes are produced or trusted."
        )
        self.assertIn(required, text)
        path.write_text(text.replace(required, ""), encoding="utf-8")

    def rename_documented_verify_only_option(self, temp_root: Path) -> None:
        path = temp_root / "rstim/doc/rsmp-cli.md"
        text = path.read_text(encoding="utf-8")
        required = '"name": "--verify_only"'
        self.assertIn(required, text)
        path.write_text(text.replace(required, '"name": "--verify"', 1), encoding="utf-8")

    def read_json(self, path: Path) -> dict[str, Any]:
        return json.loads(path.read_text(encoding="utf-8"))

    def write_json(self, path: Path, value: dict[str, Any]) -> None:
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


class RsmpV1ReadinessRepoRootControls(unittest.TestCase):
    def test_compression_validation_uses_supplied_repo_root_cargo_lock(self) -> None:
        with tempfile.TemporaryDirectory(prefix="rstim-rsmp-readiness-root-test-") as raw_tmp:
            temp_root = Path(raw_tmp) / "repo"
            artifact_dir = Path(raw_tmp) / "out"
            control = RsmpV1ReadinessNegativeControls()
            control.copy_required_inputs(temp_root)
            cargo_lock = temp_root / "Cargo.lock"
            cargo_lock.write_text(
                cargo_lock.read_text(encoding="utf-8") + "\n# readiness regression\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER),
                    "--repo-root",
                    str(temp_root),
                    "--out-dir",
                    str(artifact_dir),
                    "--skip-commands",
                ],
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            combined = result.stdout + result.stderr
            self.assertIn("compression evidence validation failed", combined)
            self.assertIn("environment Cargo.lock sha256 mismatch", combined)
            self.assertNotIn(PASS_LINE, combined)
            artifact = json.loads((artifact_dir / "readiness.json").read_text())
            self.assertEqual(artifact["status"], "fail")
            self.assertTrue(
                any(item["check"] == "compression.bundle" for item in artifact["failed_checks"]),
                artifact["failed_checks"],
            )


if __name__ == "__main__":
    unittest.main()
