#!/usr/bin/env python3
"""Validate the root agent entry and its authoritative workspace map."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlsplit


ROOT_ENTRY = Path("AGENTS.md")
AUTHORITATIVE_GUIDE = Path(".AGENTS/AGENTS.md")
MEMBERS_HEADING = "## Current Workspace Members"
MEMBER_LINE = re.compile(r"^- `([^`]+)` — `([^`]+)`$")
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]*)\)")


class EntryError(Exception):
    """A repository instruction entry does not match its declared contract."""


def local_link_targets(text: str) -> list[str]:
    targets: list[str] = []
    for match in MARKDOWN_LINK.finditer(text):
        parts = match.group(1).strip().split(maxsplit=1)
        if not parts:
            raise EntryError("malformed empty Markdown link")
        target = parts[0].strip("<>")
        if not target:
            raise EntryError("malformed empty Markdown link")
        parsed = urlsplit(target)
        if target and not target.startswith("#") and not parsed.scheme and not parsed.netloc:
            targets.append(parsed.path)
    return targets


def resolve_local_links(repo_root: Path, document: Path) -> list[str]:
    errors: list[str] = []
    document_path = repo_root / document
    if not document_path.is_file():
        return [f"missing required file: {document.as_posix()}"]

    resolved_root = repo_root.resolve()
    for target in local_link_targets(document_path.read_text(encoding="utf-8")):
        candidate = (document_path.parent / target).resolve()
        if not candidate.is_relative_to(resolved_root):
            errors.append(f"local link escapes repository: {target} referenced by {document.as_posix()}")
        elif not candidate.exists():
            errors.append(f"missing local link target: {candidate.relative_to(resolved_root).as_posix()} referenced by {document.as_posix()}")
    return errors


def validate_root_entry(repo_root: Path) -> list[str]:
    root_path = repo_root / ROOT_ENTRY
    if not root_path.is_file():
        return [f"missing required file: {ROOT_ENTRY.as_posix()}"]

    targets = local_link_targets(root_path.read_text(encoding="utf-8"))
    if AUTHORITATIVE_GUIDE.as_posix() not in targets:
        return [f"missing root routing link: {AUTHORITATIVE_GUIDE.as_posix()}"]

    return resolve_local_links(repo_root, ROOT_ENTRY)


def guide_member_lines(text: str) -> list[tuple[str, str]]:
    try:
        section = text.split(MEMBERS_HEADING, maxsplit=1)[1].split("\n## ", maxsplit=1)[0]
    except IndexError as error:
        raise EntryError(f"missing guide section: {MEMBERS_HEADING.removeprefix('## ')}") from error

    members = []
    for line in section.splitlines():
        match = MEMBER_LINE.match(line)
        if match:
            members.append((match.group(1), match.group(2)))
    if not members:
        raise EntryError("guide has no workspace members")
    return members


def cargo_workspace_members(repo_root: Path) -> dict[str, str]:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=repo_root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode:
        raise EntryError(f"cargo metadata failed: {result.stderr.strip()}")

    metadata = json.loads(result.stdout)
    member_ids = set(metadata["workspace_members"])
    members: dict[str, str] = {}
    for package in metadata["packages"]:
        if package["id"] not in member_ids:
            continue
        manifest = Path(package["manifest_path"])
        try:
            members[package["name"]] = manifest.parent.resolve().relative_to(repo_root.resolve()).as_posix() + "/"
        except ValueError as error:
            raise EntryError(f"workspace package is outside repository: {package['name']}") from error
    return members


def validate_workspace_members(repo_root: Path) -> int:
    guide_path = repo_root / AUTHORITATIVE_GUIDE
    if not guide_path.is_file():
        raise EntryError(f"missing required file: {AUTHORITATIVE_GUIDE.as_posix()}")

    listed_members = guide_member_lines(guide_path.read_text(encoding="utf-8"))
    guide_members = dict(listed_members)
    metadata_members = cargo_workspace_members(repo_root)
    if len(guide_members) != len(listed_members):
        raise EntryError("guide lists a workspace package more than once")

    missing = sorted(set(metadata_members) - set(guide_members))
    unexpected = sorted(set(guide_members) - set(metadata_members))
    errors = []
    if missing:
        errors.append(f"missing workspace members: {', '.join(missing)}")
    if unexpected:
        errors.append(f"advertised package names do not resolve: {', '.join(unexpected)}")
    for name in sorted(set(guide_members) & set(metadata_members)):
        if guide_members[name] != metadata_members[name]:
            errors.append(
                f"workspace member path mismatch for {name}: guide={guide_members[name]} metadata={metadata_members[name]}"
            )
    if errors:
        raise EntryError("; ".join(errors))
    return len(metadata_members)


def validate(repo_root: Path) -> int:
    errors = validate_root_entry(repo_root)
    errors.extend(resolve_local_links(repo_root, AUTHORITATIVE_GUIDE))
    if errors:
        raise EntryError("; ".join(errors))
    return validate_workspace_members(repo_root)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()

    try:
        count = validate(args.repo_root.resolve())
    except (EntryError, OSError, json.JSONDecodeError) as error:
        print(f"FAIL agent entry: {error}", file=sys.stderr)
        return 1
    print(f"PASS agent entry members={count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
