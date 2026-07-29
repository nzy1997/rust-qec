from __future__ import annotations

import os
import shutil
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class MakefileReleaseTest(unittest.TestCase):
    def test_release_refreshes_lock_before_locked_check_and_stages_it(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            fixture = Path(tmp)
            shutil.copy2(REPO_ROOT / "Makefile", fixture / "Makefile")
            for crate in ("rstim", "rsinter", "rbposd", "rmatching"):
                crate_dir = fixture / crate
                crate_dir.mkdir()
                (crate_dir / "Cargo.toml").write_text(
                    textwrap.dedent(
                        """\
                        [package]
                        name = "fixture"
                        version = "0.1.1"
                        edition = "2021"
                        """
                    ),
                    encoding="utf-8",
                )
            (fixture / "Cargo.lock").write_text("# fixture lock\n", encoding="utf-8")

            bin_dir = fixture / "bin"
            bin_dir.mkdir()
            log_path = fixture / "commands.log"
            lock_ready = fixture / ".lock-refreshed"

            self._write_executable(
                bin_dir / "cargo",
                f"""\
                #!/usr/bin/env python3
                from pathlib import Path
                import sys
                log = Path({str(log_path)!r})
                ready = Path({str(lock_ready)!r})
                line = "cargo " + " ".join(sys.argv[1:]) + "\\n"
                log.write_text(log.read_text() + line if log.exists() else line)
                if sys.argv[1:] == ["check", "--offline", "--workspace"]:
                    ready.write_text("ok\\n")
                    raise SystemExit(0)
                if sys.argv[1:] == ["check", "--locked", "--workspace"]:
                    if not ready.exists():
                        print("locked check ran before lock refresh", file=sys.stderr)
                        raise SystemExit(42)
                    raise SystemExit(0)
                print("unexpected cargo argv: " + repr(sys.argv[1:]), file=sys.stderr)
                raise SystemExit(2)
                """,
            )
            self._write_executable(
                bin_dir / "git",
                f"""\
                #!/usr/bin/env python3
                from pathlib import Path
                import sys
                log = Path({str(log_path)!r})
                line = "git " + " ".join(sys.argv[1:]) + "\\n"
                log.write_text(log.read_text() + line if log.exists() else line)
                if sys.argv[1:2] == ["add"] and "Cargo.lock" not in sys.argv[2:]:
                    print("release commit did not stage Cargo.lock", file=sys.stderr)
                    raise SystemExit(43)
                raise SystemExit(0)
                """,
            )

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            result = subprocess.run(
                ["make", "release", "V=0.2.0"],
                cwd=fixture,
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )

            output = result.stdout + result.stderr
            self.assertEqual(result.returncode, 0, output)
            self.assertEqual(
                (fixture / "rstim" / "Cargo.toml").read_text(encoding="utf-8").splitlines()[2],
                'version = "0.2.0"',
            )
            command_log = log_path.read_text(encoding="utf-8")
            self.assertIn("cargo check --offline --workspace\ncargo check --locked --workspace", command_log)
            self.assertIn(
                "git add rstim/Cargo.toml rsinter/Cargo.toml rbposd/Cargo.toml rmatching/Cargo.toml Cargo.lock",
                command_log,
            )

    def _write_executable(self, path: Path, text: str) -> None:
        path.write_text(textwrap.dedent(text), encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)


if __name__ == "__main__":
    unittest.main()
