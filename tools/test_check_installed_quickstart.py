from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
import hashlib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_installed_quickstart.py"


class StatsContractTest(unittest.TestCase):
    def test_wrong_double_digit_count_is_not_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bin_dir = Path(temporary) / "bin"
            bin_dir.mkdir()
            rustqec = bin_dir / "rustqec"
            rustqec.write_text(
                "#!/bin/sh\n"
                "if [ \"$1\" = capabilities ]; then\n"
                "  printf '%s\\n' '{\"commands\":[{\"name\":\"circuit.stats\",\"argv\":[\"circuit\",\"stats\"],\"formats\":[\"human\",\"json\"]}]}'\n"
                "elif [ \"$1\" = circuit ]; then\n"
                "  printf '%s\\n' '{\"result\":{\"instruction_count\":5,\"num_qubits\":10,\"num_measurements\":1,\"num_detectors\":1,\"num_observables\":1}}'\n"
                "fi\n",
                encoding="utf-8",
            )
            rstim = bin_dir / "rstim"
            rstim.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
            rustqec.chmod(0o755)
            rstim.chmod(0o755)
            result = subprocess.run([sys.executable, str(CHECKER), "--bin-dir", str(bin_dir)], cwd=REPO_ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("stats did not match the showcase", result.stderr)

    def test_selects_only_the_downloaded_archive_checksum(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            release = Path(temporary)
            archive = release / "rustqec-v0.2.1-x86_64-unknown-linux-gnu.tar.gz"
            archive.write_bytes(b"verified archive")
            other = "rustqec-v0.2.1-aarch64-apple-darwin.tar.gz"
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            (release / "SHA256SUMS").write_text(f"{digest}  {archive.name}\n{'0' * 64}  {other}\n", encoding="utf-8")
            command = "awk -v archive=\"$archive\" '$2 == archive { count++; record = $0 } END { if (count != 1) exit 1; print record }' SHA256SUMS > \"$archive.sha256\" && if command -v sha256sum >/dev/null; then sha256sum -c \"$archive.sha256\"; else shasum -a 256 -c \"$archive.sha256\"; fi"
            result = subprocess.run(["sh", "-c", command], cwd=release, env={"PATH": "/usr/bin:/bin", "archive": archive.name}, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            archive.write_bytes(b"tampered")
            tampered = subprocess.run(["sh", "-c", command], cwd=release, env={"PATH": "/usr/bin:/bin", "archive": archive.name}, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        self.assertNotEqual(tampered.returncode, 0)


class InstalledQuickstartCheckerTest(unittest.TestCase):
    def test_zero_output_stub_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bin_dir = Path(temporary) / "bin"
            bin_dir.mkdir()
            for name in ("rustqec", "rstim"):
                binary = bin_dir / name
                binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                binary.chmod(0o755)
            result = subprocess.run(
                [sys.executable, str(CHECKER), "--bin-dir", str(bin_dir)],
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("capabilities did not return its JSON contract", result.stderr)


if __name__ == "__main__":
    unittest.main()
