import copy
import json
import unittest
from pathlib import Path

from tools.check_native_support_matrix import MatrixError, validate_matrix


FIXTURE = Path(__file__).parent / "fixtures" / "native_support_matrix" / "passing_run.json"


class CheckNativeSupportMatrixTests(unittest.TestCase):
    def setUp(self):
        self.recorded = json.loads(FIXTURE.read_text())

    def test_accepts_complete_recorded_run(self):
        summaries = validate_matrix(
            self.recorded["jobs"], self.recorded["evidence"], self.recorded["packages"],
            self.recorded["run"], self.recorded["run"]["head_sha"]
        )
        self.assertEqual(len(summaries), 4)

    def test_rejects_recorded_run_with_skipped_cell(self):
        recorded = copy.deepcopy(self.recorded)
        recorded["jobs"][2]["conclusion"] = "skipped"
        with self.assertRaisesRegex(MatrixError, "not successful"):
            validate_matrix(recorded["jobs"], recorded["evidence"], recorded["packages"], recorded["run"], recorded["run"]["head_sha"])

    def test_rejects_recorded_run_with_missing_cell(self):
        recorded = copy.deepcopy(self.recorded)
        recorded["evidence"].pop()
        with self.assertRaisesRegex(MatrixError, "exactly one evidence artifact"):
            validate_matrix(recorded["jobs"], recorded["evidence"], recorded["packages"], recorded["run"], recorded["run"]["head_sha"])

    def test_rejects_msrv_below_a_supported_package_requirement(self):
        recorded = copy.deepcopy(self.recorded)
        for package in recorded["packages"]:
            package["rust_version"] = "1.89"
        with self.assertRaisesRegex(MatrixError, "below declared package requirement"):
            validate_matrix(recorded["jobs"], recorded["evidence"], recorded["packages"], recorded["run"], recorded["run"]["head_sha"])

    def test_rejects_compiler_identity_mismatch(self):
        recorded = copy.deepcopy(self.recorded)
        recorded["evidence"][0]["compiler"]["path"] = "/opt/homebrew/bin/rustc"
        with self.assertRaisesRegex(MatrixError, "requested rustup toolchain"):
            validate_matrix(recorded["jobs"], recorded["evidence"], recorded["packages"], recorded["run"], recorded["run"]["head_sha"])

    def test_rejects_an_old_run_for_the_checked_out_head(self):
        with self.assertRaisesRegex(MatrixError, "does not match checked-out HEAD"):
            validate_matrix(
                self.recorded["jobs"], self.recorded["evidence"], self.recorded["packages"],
                self.recorded["run"], "different-head"
            )


if __name__ == "__main__":
    unittest.main()
