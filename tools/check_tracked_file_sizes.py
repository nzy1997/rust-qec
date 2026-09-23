#!/usr/bin/env python3
"""Reject oversized Git-tracked files introduced by a change."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MAX_BYTES = 10 * 1024 * 1024
ZERO_SHA = "0" * 40


def git(repo: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), *args])


def changed_paths(repo: Path, base: str | None, head: str) -> list[str]:
    if base and base != ZERO_SHA:
        output = git(
            repo,
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            "--no-renames",
            "-z",
            base,
            head,
            "--",
        )
    else:
        output = git(repo, "ls-tree", "-r", "--name-only", "-z", head)
    return [os.fsdecode(path) for path in output.split(b"\0") if path]


def blob_size(repo: Path, head: str, path: str) -> int:
    return int(git(repo, "cat-file", "-s", f"{head}:{path}").strip())


def oversized_files(
    repo: Path, base: str | None, head: str, max_bytes: int
) -> list[tuple[str, int]]:
    findings = []
    for path in changed_paths(repo, base, head):
        size = blob_size(repo, head, path)
        if size > max_bytes:
            findings.append((path, size))
    return findings


def format_size(size: int) -> str:
    return f"{size / (1024 * 1024):.2f} MiB"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=REPO_ROOT)
    parser.add_argument("--base", help="base commit; omit to inspect the complete head tree")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    args = parser.parse_args(argv)
    if args.max_bytes <= 0:
        parser.error("--max-bytes must be positive")

    repo = args.repo.resolve()
    paths = changed_paths(repo, args.base, args.head)
    findings = oversized_files(repo, args.base, args.head, args.max_bytes)
    limit = format_size(args.max_bytes)
    if findings:
        print(f"ERROR tracked file size check: limit is {limit}", file=sys.stderr)
        for path, size in findings:
            print(f"- {path}: {format_size(size)}", file=sys.stderr)
        return 1
    print(f"PASS tracked file size check: {len(paths)} changed files, limit {limit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
