import argparse
import hashlib
import io
import json
import platform
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools.verify_release_archive import VerificationError, verify


TAG = "v0.2.1"
SOURCE_SHA = "1" * 40


def host_target() -> str:
    if platform.system() == "Darwin" and platform.machine() == "arm64":
        return "aarch64-apple-darwin"
    if platform.system() == "Linux" and platform.machine() == "x86_64":
        return "x86_64-unknown-linux-gnu"
    raise unittest.SkipTest("release archive checker test requires a supported host")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class ReleaseFixture:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.target = host_target()
        self.marker = self.root / "executed"
        self.archive = self.root / f"rustqec-{TAG}-{self.target}.tar.gz"
        self.checksums = self.root / "SHA256SUMS"
        self.manifest = self.root / "release-manifest.json"
        self._write_archive()
        self._write_identity()

    def close(self):
        self.temporary.cleanup()

    def script(self, output: object) -> bytes:
        return (
            "#!/bin/sh\n"
            "cat >/dev/null\n"
            f": > {self.marker!s}\n"
            f"printf '%s\\n' '{json.dumps(output, separators=(',', ':'))}'\n"
        ).encode()

    def _write_archive(self, unsafe: bool = False, runtime_source: str = SOURCE_SHA):
        archive_root = self.archive.name.removesuffix(".tar.gz")
        rustqec = {
            "schema_version": "rustqec.cli.v1", "status": "ok", "command": "circuit.stats",
            "result": {"instruction_count": 2, "repeat_blocks": 0, "max_repeat_depth": 0,
                       "num_qubits": 1, "num_measurements": 1, "num_detectors": 1,
                       "num_observables": 0, "num_ticks": 0, "num_sweep_bits": 0},
            "warnings": [], "artifacts": [],
        }
        rstim = {"instruction_count": 2, "repeat_blocks": 0, "max_repeat_depth": 0,
                 "num_qubits": 1, "num_measurements": 1, "num_detectors": 1,
                 "num_observables": 0, "num_ticks": 0, "num_sweep_bits": 0}
        members = {
            f"{archive_root}/bin/rustqec": (self.script(rustqec), 0o755),
            f"{archive_root}/bin/rstim": (self.script(rstim), 0o755),
            f"{archive_root}/LICENSE": (b"test license\n", 0o644),
            f"{archive_root}/RUNTIME.md": (
                f"RustQEC native command-line archive\n\nTag: {TAG}\n"
                f"Source commit: {runtime_source}\nTarget: {self.target}\n".encode(), 0o644
            ),
        }
        if unsafe:
            members["../escape"] = (b"escape\n", 0o644)
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, (contents, mode) in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(contents)
                info.mode = mode
                archive.addfile(info, io.BytesIO(contents))

    def _write_identity(self):
        archive_hash = digest(self.archive)
        other_target = (
            "x86_64-unknown-linux-gnu"
            if self.target == "aarch64-apple-darwin"
            else "aarch64-apple-darwin"
        )
        other_name = f"rustqec-{TAG}-{other_target}.tar.gz"
        other_hash = "2" * 64
        self.checksums.write_text(
            f"{archive_hash}  {self.archive.name}\n{other_hash}  {other_name}\n"
        )
        common_archive = {
            "compiler": {"release": "1.88.0", "host": self.target, "commit_hash": "3" * 40},
            "runtime": {"baseline": "test baseline", "linkage": ["system library"]},
        }
        payload = {
            "schema_version": "rustqec.release-manifest.v1",
            "tag": TAG,
            "source_sha": SOURCE_SHA,
            "packages": [
                {"name": "rustqec-cli", "version": "0.1.0", "rust_version": "1.88"},
                {"name": "rstim", "version": "0.2.1", "rust_version": "1.88"},
            ],
            "shot_lab_assets": {
                "rebuilt_from_tag": True,
                "manifest_sha256": "4" * 64,
                "manifest": {"format_version": "rstim-shot-assets-v1", "files": {}},
            },
            "archives": {
                self.archive.name: {
                    **common_archive,
                    "filename": self.archive.name,
                    "root_directory": self.archive.name.removesuffix(".tar.gz"),
                    "sha256": archive_hash,
                    "size": self.archive.stat().st_size,
                    "target": self.target,
                },
                other_name: {
                    "filename": other_name,
                    "root_directory": other_name.removesuffix(".tar.gz"),
                    "sha256": other_hash,
                    "size": 1,
                    "target": other_target,
                    "compiler": {"release": "1.88.0", "host": other_target, "commit_hash": "3" * 40},
                    "runtime": {"baseline": "test baseline", "linkage": ["system library"]},
                },
            },
        }
        self.manifest.write_text(json.dumps(payload))

    def arguments(self, expected_tag: str = TAG) -> argparse.Namespace:
        return argparse.Namespace(
            archive=self.archive, checksums=self.checksums, manifest=self.manifest,
            expected_tag=expected_tag,
        )


class VerifyReleaseArchiveTests(unittest.TestCase):
    def setUp(self):
        self.fixture = ReleaseFixture()

    def tearDown(self):
        self.fixture.close()

    def test_executes_verified_archive_binaries(self):
        with mock.patch("tools.verify_release_archive.check_linkage"):
            self.assertEqual(verify(self.fixture.arguments()), (self.fixture.target, TAG))
        self.assertTrue(self.fixture.marker.exists())

    def test_rejects_tampered_archive_before_execution(self):
        with self.fixture.archive.open("ab") as handle:
            handle.write(b"tampered")
        with self.assertRaisesRegex(VerificationError, "SHA-256 mismatch"):
            verify(self.fixture.arguments())
        self.assertFalse(self.fixture.marker.exists())

    def test_rejects_manifest_from_another_tag_before_execution(self):
        with self.assertRaisesRegex(VerificationError, "release tag mismatch"):
            verify(self.fixture.arguments(expected_tag="v9.9.9"))
        self.assertFalse(self.fixture.marker.exists())

    def test_rejects_path_traversal_member_before_execution(self):
        self.fixture._write_archive(unsafe=True)
        self.fixture._write_identity()
        with self.assertRaisesRegex(VerificationError, "members do not match contract|unsafe archive member"):
            verify(self.fixture.arguments())
        self.assertFalse(self.fixture.marker.exists())

    def test_rejects_symlink_member_before_execution(self):
        archive_root = self.fixture.archive.name.removesuffix(".tar.gz")
        with tarfile.open(self.fixture.archive, "w:gz") as archive:
            for relative in ("bin/rstim", "LICENSE", "RUNTIME.md"):
                contents = b"placeholder"
                info = tarfile.TarInfo(f"{archive_root}/{relative}")
                info.size = len(contents)
                archive.addfile(info, io.BytesIO(contents))
            link = tarfile.TarInfo(f"{archive_root}/bin/rustqec")
            link.type = tarfile.SYMTYPE
            link.linkname = "/bin/sh"
            archive.addfile(link)
        self.fixture._write_identity()
        with self.assertRaisesRegex(VerificationError, "members do not match contract|unsupported archive member type"):
            verify(self.fixture.arguments())
        self.assertFalse(self.fixture.marker.exists())

    def test_rejects_runtime_source_mismatch_before_execution(self):
        self.fixture._write_archive(runtime_source="9" * 40)
        self.fixture._write_identity()
        with self.assertRaisesRegex(VerificationError, "RUNTIME.md identity"):
            verify(self.fixture.arguments())
        self.assertFalse(self.fixture.marker.exists())


if __name__ == "__main__":
    unittest.main()
