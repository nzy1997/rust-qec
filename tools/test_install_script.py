#!/usr/bin/env python3
"""Offline behavioural tests for the public POSIX install script."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
INSTALLER = REPO_ROOT / "site/static/install.sh"
HASHES = {
    "linux": "3ebafcd684f06478ee9d6f4fc85da61e10de4e498020b86c0e76806b67c35aff",
    "mac": "a74da165d4cd562aaa9e9cbc06d0a42bdf508e65bf283868df1dede4f0a6e8f7",
}
TARGETS = {
    "linux": ("Linux", "x86_64", "x86_64-unknown-linux-gnu"),
    "mac": ("Darwin", "arm64", "aarch64-apple-darwin"),
}


class InstallScriptTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.fake_bin = self.root / "fake-bin"
        self.fake_bin.mkdir()
        (self.root / "temporary").mkdir()
        self.download = self.root / "download.tar.gz"
        self.curl_log = self.root / "curl.log"
        self._write_fake_tools()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _write_executable(self, name: str, source: str) -> None:
        path = self.fake_bin / name
        path.write_text(source, encoding="utf-8")
        path.chmod(0o755)

    def _write_fake_tools(self) -> None:
        self._write_executable(
            "uname",
            "#!/bin/sh\ncase $1 in -s) printf '%s\\n' \"$FAKE_UNAME_S\" ;; -m) printf '%s\\n' \"$FAKE_UNAME_M\" ;; esac\n",
        )
        self._write_executable(
            "curl",
            "#!/bin/sh\nout= url=\nwhile [ $# -gt 0 ]; do\n  case $1 in -o) out=$2; shift 2 ;; -*) shift ;; *) url=$1; shift ;; esac\ndone\nprintf '%s %s\\n' \"$url\" \"$out\" >> \"$CURL_LOG\"\ncp \"$FAKE_DOWNLOAD\" \"$out\"\n",
        )
        self._write_executable(
            "sha256sum",
            "#!/bin/sh\nprintf '%s  %s\\n' \"$FAKE_SHA256\" \"$1\"\n",
        )

    def _make_archive(self, platform: str, *, runnable: bool = True) -> None:
        _system, _machine, target = TARGETS[platform]
        root = self.root / f"rustqec-v0.3.0-{target}"
        binaries = root / "bin"
        binaries.mkdir(parents=True)
        for command in ("rustqec", "rstim"):
            binary = binaries / command
            if command == "rustqec" and not runnable:
                binary.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
            else:
                binary.write_text(f"#!/bin/sh\necho {command}\n", encoding="utf-8")
            binary.chmod(0o755)
        with tarfile.open(self.download, "w:gz") as archive:
            archive.add(root, arcname=root.name)

    def _environment(
        self,
        platform: str = "linux",
        *,
        home: Path | None = None,
        sha256: str | None = None,
    ) -> dict[str, str]:
        system, machine, _target = TARGETS[platform]
        return os.environ | {
            "PATH": f"{self.fake_bin}:{os.environ['PATH']}",
            "FAKE_UNAME_S": system,
            "FAKE_UNAME_M": machine,
            "FAKE_DOWNLOAD": str(self.download),
            "CURL_LOG": str(self.curl_log),
            "HOME": str(home or self.root / "home"),
            "TMPDIR": str(self.root / "temporary"),
            "FAKE_SHA256": sha256 or HASHES[platform],
        }

    def _run(
        self,
        platform: str = "linux",
        *args: str,
        home: Path | None = None,
        sha256: str | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["sh", str(INSTALLER), *args],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=self._environment(platform, home=home, sha256=sha256),
            cwd=self.root,
            check=False,
        )

    def test_truncated_piped_script_does_not_start_installing(self) -> None:
        prefix = INSTALLER.read_text().rsplit('main "$@"', 1)[0]
        target = self.root / "truncated-bin"
        result = subprocess.run(["sh", "-s", "--", "--bin-dir", str(target)], input=prefix,
                                text=True, capture_output=True, env=self._environment("linux"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(target.exists())
        self.assertFalse(self.curl_log.exists())

    def test_linux_and_macos_select_pinned_archives_and_install_executables(self) -> None:
        for platform in HASHES:
            with self.subTest(platform=platform):
                self._make_archive(platform)
                bin_dir = self.root / f"{platform} bin"
                result = self._run(platform, "--bin-dir", str(bin_dir))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(TARGETS[platform][2], result.stdout)
                for command in ("rustqec", "rstim"):
                    binary = bin_dir / command
                    self.assertTrue(binary.is_file())
                    self.assertTrue(os.access(binary, os.X_OK))
                self.assertIn(
                    f"/v0.3.0/rustqec-v0.3.0-{TARGETS[platform][2]}.tar.gz",
                    self.curl_log.read_text(encoding="utf-8"),
                )

    def test_unsupported_platform_does_not_download_or_write(self) -> None:
        self._make_archive("linux")
        env = os.environ | {
            "PATH": f"{self.fake_bin}:{os.environ['PATH']}",
            "FAKE_UNAME_S": "FreeBSD",
            "FAKE_UNAME_M": "x86_64",
            "FAKE_DOWNLOAD": str(self.download),
            "CURL_LOG": str(self.curl_log),
            "HOME": str(self.root / "home"),
            "TMPDIR": str(self.root / "temporary"),
            "FAKE_SHA256": HASHES["linux"],
        }
        unsupported = subprocess.run(
            ["sh", str(INSTALLER), "--bin-dir", str(self.root / "unsupported")],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            check=False,
        )
        self.assertNotEqual(unsupported.returncode, 0)
        self.assertIn("no validated native archive", unsupported.stderr)
        self.assertFalse((self.root / "unsupported").exists())

    def test_checksum_failure_leaves_target_untouched_and_cleans_temporary_download(self) -> None:
        self._make_archive("linux")
        bin_dir = self.root / "bin"
        temporary = self.root / "temporary"
        before = set(temporary.iterdir())
        result = self._run("linux", "--bin-dir", str(bin_dir), sha256="0" * 64)
        after = set(temporary.iterdir())
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum", result.stderr)
        self.assertFalse(bin_dir.exists())
        self.assertEqual(before, after)

    def test_existing_file_or_link_is_never_replaced(self) -> None:
        self._make_archive("linux")
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        existing = bin_dir / "rustqec"
        existing.write_text("do not replace", encoding="utf-8")
        target = self.root / "existing-rstim"
        target.write_text("do not replace link", encoding="utf-8")
        (bin_dir / "rstim").symlink_to(target)
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already exists", result.stderr)
        self.assertEqual(existing.read_text(encoding="utf-8"), "do not replace")
        self.assertEqual(target.read_text(encoding="utf-8"), "do not replace link")
        self.assertTrue((bin_dir / "rstim").is_symlink())
        self.assertFalse(self.curl_log.exists(), "occupied targets must fail before download")

    def test_help_and_path_with_spaces(self) -> None:
        help_result = self._run("linux", "--help")
        self.assertEqual(help_result.returncode, 0)
        self.assertIn("--bin-dir DIRECTORY", help_result.stdout)

        self._make_archive("linux")
        bin_dir = self.root / "space dir" / "bin tools"
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"export PATH='{bin_dir.resolve()}':$PATH", result.stdout)

        quote_dir = self.root / "quote's path" / "bin"
        quoted = "'" + str(quote_dir.resolve()).replace("'", "'\\''") + "'"
        quoted_result = self._run("linux", "--bin-dir", str(quote_dir))
        self.assertEqual(quoted_result.returncode, 0, quoted_result.stderr)
        self.assertIn(f"export PATH={quoted}:$PATH", quoted_result.stdout)

    def test_relative_bin_dir_is_reported_as_a_canonical_absolute_path(self) -> None:
        self._make_archive("linux")
        result = self._run("linux", "--bin-dir", "relative/../relative/bin")
        expected = (self.root / "relative" / "bin").resolve()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((expected / "rustqec").is_file())
        self.assertIn(f"Installed rustqec and rstim in {expected}", result.stdout)
        self.assertIn(f"export PATH='{expected}':$PATH", result.stdout)

    def test_second_copy_failure_rolls_back_the_first_binary(self) -> None:
        self._make_archive("linux")
        self._write_executable(
            "cp",
            "#!/bin/sh\ncase $2 in */rstim) printf partial > \"$2\"; exit 1 ;; esac\n/bin/cp \"$@\"\n",
        )
        bin_dir = self.root / "bin"
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("could not prepare rstim", result.stderr)
        self.assertFalse((bin_dir / "rustqec").exists())
        self.assertFalse((bin_dir / "rstim").exists())

    def test_non_runnable_verified_binary_does_not_install(self) -> None:
        self._make_archive("linux", runnable=False)
        bin_dir = self.root / "bin"
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cannot run on this system", result.stderr)
        self.assertFalse(bin_dir.exists())

    def test_signal_exits_nonzero_and_cleans_the_temporary_directory(self) -> None:
        self._write_executable("curl", "#!/bin/sh\nkill -TERM \"$PPID\"\n")
        result = self._run("linux", "--bin-dir", str(self.root / "bin"))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list((self.root / "temporary").iterdir()), [])

    def test_racing_file_or_symlink_is_not_overwritten(self) -> None:
        self._make_archive("linux")
        for kind in ("file", "symlink"):
            with self.subTest(kind=kind):
                bin_dir = self.root / f"{kind}-bin"
                third_party = self.root / f"{kind}-third-party"
                third_party.write_text("third party", encoding="utf-8")
                if kind == "file":
                    race = "printf 'third party' > \"$2/rustqec\""
                else:
                    race = f"ln -s '{third_party}' \"$2/rustqec\""
                self._write_executable(
                    "ln",
                    "#!/bin/sh\ncase $1 in */rustqec) " + race + ";; esac\n/bin/ln \"$@\"\n",
                )
                result = self._run("linux", "--bin-dir", str(bin_dir))
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("appeared during installation", result.stderr)
                racing_target = bin_dir / "rustqec"
                if kind == "file":
                    self.assertEqual(racing_target.read_text(encoding="utf-8"), "third party")
                else:
                    self.assertTrue(racing_target.is_symlink())
                    self.assertEqual(racing_target.resolve(), third_party.resolve())
                self.assertFalse((bin_dir / "rstim").exists())

    def test_signal_after_first_publish_leaves_the_complete_published_binary(self) -> None:
        self._make_archive("linux")
        self._write_executable(
            "ln",
            "#!/bin/sh\n/bin/ln \"$@\" || exit $?\ncase $1 in */rustqec) kill -TERM \"$PPID\" ;; esac\n",
        )
        bin_dir = self.root / "bin"
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((bin_dir / "rustqec").is_file())
        self.assertTrue(os.access(bin_dir / "rustqec", os.X_OK))
        self.assertFalse((bin_dir / "rstim").exists())
        self.assertEqual(list(bin_dir.glob(".rustqec-install.*")), [])
        self.assertIn("any published binaries were left", result.stderr)

    def test_signal_after_second_publish_leaves_both_complete_binaries(self) -> None:
        self._make_archive("linux")
        self._write_executable(
            "ln",
            "#!/bin/sh\n/bin/ln \"$@\" || exit $?\ncase $1 in */rstim) kill -TERM \"$PPID\" ;; esac\n",
        )
        bin_dir = self.root / "bin"
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertNotEqual(result.returncode, 0)
        for command in ("rustqec", "rstim"):
            self.assertTrue((bin_dir / command).is_file())
            self.assertTrue(os.access(bin_dir / command, os.X_OK))
        self.assertEqual(list(bin_dir.glob(".rustqec-install.*")), [])
        self.assertIn("any published binaries were left", result.stderr)

    def test_cleanup_preserves_a_third_party_replacement(self) -> None:
        self._make_archive("linux")
        self._write_executable(
            "ln",
            "#!/bin/sh\ncase $1 in\n  */rstim) rm -f \"$2/rustqec\"; printf outsider > \"$2/rustqec\"; exit 1 ;;\nesac\n/bin/ln \"$@\"\n",
        )
        bin_dir = self.root / "bin"
        result = self._run("linux", "--bin-dir", str(bin_dir))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((bin_dir / "rustqec").read_text(encoding="utf-8"), "outsider")
        self.assertFalse((bin_dir / "rstim").exists())
        self.assertIn("any published binaries were left", result.stderr)


if __name__ == "__main__":
    unittest.main()
