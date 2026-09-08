"""Negative controls against the actual workspace publication contract."""

import copy
import json
from pathlib import Path
import subprocess
import unittest

from tools.check_crates_io import metadata_errors


class PublicationContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[1]
        cls.policy = json.loads((root / "tools/crates_io_packages.json").read_text())
        cls.packages = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, text=True))["packages"]

    def altered(self, name):
        packages = copy.deepcopy(self.packages)
        return packages, next(package for package in packages if package["name"] == name)

    def test_reviewed_publication_order_is_complete(self):
        self.assertEqual(metadata_errors(self.packages, self.policy), [])

    def test_path_only_dependency_cannot_enter_publication(self):
        packages, cli = self.altered("rustqec-cli")
        next(dep for dep in cli["dependencies"] if dep["name"] == "rstim")["req"] = "*"
        self.assertTrue(any("rstim needs a registry version" in error for error in metadata_errors(packages, self.policy)))

    def test_optional_dependency_is_still_required_before_its_consumer(self):
        policy = copy.deepcopy(self.policy)
        policy["publish_order"].remove("qec-ilp-core")
        policy["publish_order"].append("qec-ilp-core")
        self.assertTrue(any("qec-code: qec-ilp-core must be published earlier" in error
                            for error in metadata_errors(self.packages, policy)))

    def test_unrestricted_benchmark_bridge_is_rejected(self):
        packages, bridge = self.altered("surface_decoder_compare_bridge")
        bridge["publish"] = None
        self.assertIn("surface_decoder_compare_bridge: deferred packages must set publish=false",
                      metadata_errors(packages, self.policy))

    def test_internal_worker_cannot_become_a_default_install_target(self):
        packages, rstim = self.altered("rstim")
        next(target for target in rstim["targets"] if target["name"] == "rstim_compiled_steady_worker").pop("required-features")
        self.assertTrue(any("default installation exposes" in error for error in metadata_errors(packages, self.policy)))


if __name__ == "__main__":
    unittest.main()
