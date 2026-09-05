#!/usr/bin/env python3
from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_agent_entry.py"


class AgentEntryCheckerTest(unittest.TestCase):
    def run_checker(self, repo_root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--repo-root", str(repo_root)],
            cwd=REPO_ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @contextmanager
    def temporary_copy(self) -> Iterator[Path]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            shutil.copytree(
                REPO_ROOT,
                root,
                ignore=shutil.ignore_patterns(".git", "target", "__pycache__", ".worktrees"),
            )
            yield root

    def test_current_entry_passes(self) -> None:
        result = self.run_checker(REPO_ROOT)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS agent entry members=11\n")
        self.assertEqual(result.stderr, "")

    def test_missing_root_routing_target_fails_with_path(self) -> None:
        with self.temporary_copy() as root:
            (root / ".AGENTS" / "AGENTS.md").unlink()

            result = self.run_checker(root)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing local link target: .AGENTS/AGENTS.md", result.stderr)

    def test_missing_workspace_member_fails_with_name(self) -> None:
        with self.temporary_copy() as root:
            guide = root / ".AGENTS" / "AGENTS.md"
            guide.write_text(
                guide.read_text(encoding="utf-8").replace("- `rstim` — `rstim/`\n", ""),
                encoding="utf-8",
            )

            result = self.run_checker(root)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing workspace members: rstim", result.stderr)


if __name__ == "__main__":
    unittest.main()
