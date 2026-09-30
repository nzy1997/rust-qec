"""Regression tests for packaging current and historical native CLI layouts."""

from __future__ import annotations

import argparse
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools.build_release_archive import BuildError, package


class NativePackageLayoutTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "LICENSE").write_text("test license\n")
        (self.root / "rstim").write_bytes(b"rstim binary")
        (self.root / "rustqec").write_bytes(b"rustqec binary")
        (self.root / "runtime-linkage.txt").write_text("rstim:\n  system library\n")
        self.target = "aarch64-apple-darwin"
        self.tag = "v0.3.0"

    def arguments(self, *, legacy: bool) -> argparse.Namespace:
        return argparse.Namespace(
            repo_root=self.root,
            out_dir=self.root / "output",
            tag=self.tag,
            source_sha="a" * 40,
            target=self.target,
            cargo=self.root / "cargo",
            rustc=self.root / "rustc",
            rstim=self.root / "rstim",
            rustqec=self.root / "rustqec" if legacy else None,
            runtime_linkage=self.root / "runtime-linkage.txt",
        )

    def package_with_members(self, *, legacy: bool) -> tuple[set[str], str]:
        packages = [{"name": "rstim", "version": "0.3.0"}]
        if legacy:
            packages.append({"name": "rustqec-cli", "version": "0.3.0"})
        with (
            mock.patch("tools.build_release_archive.verify_source"),
            mock.patch("tools.build_release_archive.package_versions", return_value=packages),
            mock.patch("tools.build_release_archive.rustc_metadata", return_value={"host": self.target}),
            mock.patch("tools.build_release_archive.shot_assets", return_value={"rebuilt_from_tag": True}),
        ):
            package(self.arguments(legacy=legacy))
        archive = self.root / "output" / f"rustqec-{self.tag}-{self.target}.tar.gz"
        with tarfile.open(archive, "r:gz") as handle:
            names = {name.removeprefix(f"rustqec-{self.tag}-{self.target}/") for name in handle.getnames()}
            runtime = handle.extractfile(f"rustqec-{self.tag}-{self.target}/RUNTIME.md").read().decode()
        fragment = json.loads((self.root / "output" / f"release-fragment-{self.target}.json").read_text())
        self.assertEqual(fragment["packages"], packages)
        return names, runtime

    def test_current_workspace_packages_only_rstim(self):
        names, runtime = self.package_with_members(legacy=False)
        self.assertEqual(names, {"bin/rstim", "LICENSE", "RUNTIME.md"})
        self.assertIn("Binary:\n  bin/rstim", runtime)
        self.assertNotIn("bin/rustqec", runtime)

    def test_historical_workspace_packages_both_binaries(self):
        names, runtime = self.package_with_members(legacy=True)
        self.assertEqual(names, {"bin/rustqec", "bin/rstim", "LICENSE", "RUNTIME.md"})
        self.assertIn("Binaries:\n  bin/rustqec\n  bin/rstim", runtime)

    def test_rejects_binary_layout_that_disagrees_with_tagged_workspace(self):
        for legacy_workspace in (False, True):
            with self.subTest(legacy_workspace=legacy_workspace):
                packages = [{"name": "rstim", "version": "0.3.0"}]
                if legacy_workspace:
                    packages.append({"name": "rustqec-cli", "version": "0.3.0"})
                with (
                    mock.patch("tools.build_release_archive.verify_source"),
                    mock.patch("tools.build_release_archive.package_versions", return_value=packages),
                ):
                    with self.assertRaisesRegex(BuildError, "must be supplied exactly"):
                        package(self.arguments(legacy=not legacy_workspace))


if __name__ == "__main__":
    unittest.main()
