#!/usr/bin/env python3
"""Select expensive CI work without dropping required check reports."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess


def documentation_only(path: str) -> bool:
    # Source and build inputs take precedence even inside documentation trees.
    if path.endswith((".rs", ".py", ".sh", "/Cargo.toml", "/Cargo.lock")):
        return False
    if path.startswith(("site/", "qp101-viz/")):
        return True
    # Package doc/docs trees can contain include_str! inputs and contract tests.
    # Keep them in the Rust gate even after the input file has been deleted.
    if re.match(r"^[^/]+/docs?/", path):
        return False
    if path.startswith(("docs/", ".AGENTS/")):
        # Documentation examples and schemas can be executable test inputs.
        return Path(path).suffix in {".md", ".rst", ".png", ".jpg", ".jpeg", ".svg", ".pdf"}
    if path in {"AGENTS.md", "LICENSE", "LICENSE-MIT", "LICENSE-APACHE"}:
        return True
    if "/" not in path and re.fullmatch(r"(?:README|CHANGELOG|CONTRIBUTING)(?:\..+)?", path):
        return True
    return bool(re.match(r"^[^/]+/(?:README(?:\.|$)|LICENSE(?:[.-]|$))", path))


def select_scope(event: str, changed: list[str]) -> dict[str, bool]:
    # Unknown files default to the full Rust gate; only known prose/site paths
    # can opt a PR out. All default-branch, scheduled, and manual runs stay full.
    return {
        "rust": event != "pull_request" or any(not documentation_only(p) for p in changed),
        "coverage": event != "pull_request",
    }


def changed_paths(base: str, head: str = "HEAD", *, cwd: Path | None = None) -> list[str]:
    if not base:
        raise ValueError("pull_request scope requires a base commit")
    # Resolve refs before forming a diff range, reject options, and disable
    # rename detection so both the deleted and added paths select their scopes.
    def resolve(ref: str) -> str:
        return subprocess.check_output(
            ["git", "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}"],
            cwd=cwd, text=True,
        ).strip()

    base_sha, head_sha = resolve(base), resolve(head)
    output = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "-z", f"{base_sha}...{head_sha}", "--"],
        cwd=cwd,
    )
    return [os.fsdecode(p) for p in output.split(b"\0") if p]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--base", default="")
    args = parser.parse_args()
    changed = changed_paths(args.base) if args.event == "pull_request" else []
    scope = select_scope(args.event, changed)
    print(json.dumps({"changed": changed, **scope}, indent=2))
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as stream:
            for name, enabled in scope.items():
                stream.write(f"{name}={str(enabled).lower()}\n")


if __name__ == "__main__":
    main()
