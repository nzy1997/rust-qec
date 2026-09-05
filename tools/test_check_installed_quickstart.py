from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_installed_quickstart.py"


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
