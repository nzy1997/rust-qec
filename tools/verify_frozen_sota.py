#!/usr/bin/env python3
"""Verify published SOTA results with their unchanged, hash-bound harness.

Current harness tests and builds run separately. The published results include
every harness file (including Cargo.lock), so dependency maintenance must not
rewrite the evidence or substitute a new lockfile into an old measurement.
"""

import argparse
import io
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
HARNESS_REVISION = "59b4991a4ec27bd13c950e67e26cc4c9467fc7b2"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("harness", choices=["sota", "compiled_sota"])
    parser.add_argument("result", type=Path)
    args = parser.parse_args()
    result = args.result.resolve(strict=True)
    relative = f"benchmarks/near_clifford/{args.harness}"
    archive = subprocess.check_output(
        ["git", "archive", HARNESS_REVISION, "--", relative], cwd=ROOT
    )
    git_dir = subprocess.check_output(
        ["git", "rev-parse", "--absolute-git-dir"], cwd=ROOT, text=True
    ).strip()
    with tempfile.TemporaryDirectory(prefix="rstim-frozen-harness-") as directory:
        snapshot = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive)) as contents:
            contents.extractall(snapshot, filter="data")
        # The verifier only reads Git objects to check recorded production hashes.
        (snapshot / ".git").write_text(f"gitdir: {git_dir}\n")
        command = [sys.executable]
        if sys.flags.optimize:
            command.append("-O")
        command += [str(snapshot / relative / "verify.py"), str(result), "--git-sources"]
        return subprocess.run(command, cwd=snapshot, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
