#!/usr/bin/env python3
"""Validate rmatching's documented API, features, and workspace links."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlsplit


README = Path("rmatching/README.md")
CARGO_TOML = Path("rmatching/Cargo.toml")
CARGO_FEATURES_HEADING = "## Cargo Features"
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]*)\)")
FEATURE_LINE = re.compile(r"^- `([^`]+)`: .+$")
RUST_BLOCK = re.compile(r"```rust\n(.*?)\n```", re.DOTALL)


class ReadmeError(Exception):
    """The README does not match the crate's published contract."""


def local_link_targets(text: str) -> list[str]:
    targets: list[str] = []
    for match in MARKDOWN_LINK.finditer(text):
        parts = match.group(1).strip().split(maxsplit=1)
        if not parts:
            raise ReadmeError("malformed empty Markdown link")
        target = parts[0].strip("<>")
        if not target:
            raise ReadmeError("malformed empty Markdown link")
        parsed = urlsplit(target)
        if not target.startswith("#") and not parsed.scheme and not parsed.netloc:
            targets.append(parsed.path)
    return targets


def validate_local_links(repo_root: Path, text: str) -> None:
    readme_path = repo_root / README
    resolved_root = repo_root.resolve()
    for target in local_link_targets(text):
        candidate = (readme_path.parent / target).resolve()
        if not candidate.is_relative_to(resolved_root):
            raise ReadmeError(f"local README link escapes repository: {target}")
        if not candidate.exists():
            raise ReadmeError(
                f"missing local README link target: {candidate.relative_to(resolved_root).as_posix()}"
            )


def cargo_features(repo_root: Path) -> set[str]:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=repo_root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode:
        raise ReadmeError(f"cargo metadata failed: {result.stderr.strip()}")
    metadata = json.loads(result.stdout)
    packages = [package for package in metadata["packages"] if package["name"] == "rmatching"]
    if len(packages) != 1:
        raise ReadmeError("cargo metadata did not resolve exactly one rmatching package")
    return set(packages[0]["features"])


def advertised_features(text: str) -> set[str]:
    try:
        section = text.split(CARGO_FEATURES_HEADING, maxsplit=1)[1].split("\n## ", maxsplit=1)[0]
    except IndexError as error:
        raise ReadmeError("missing README Cargo Features section") from error

    features = {
        match.group(1)
        for line in section.splitlines()
        if (match := FEATURE_LINE.match(line)) is not None
    }
    if not features:
        raise ReadmeError("README Cargo Features section has no feature entries")
    return features


def documented_example(text: str) -> str:
    try:
        section = text.split("## Quick Start", maxsplit=1)[1].split("\n## ", maxsplit=1)[0]
    except IndexError as error:
        raise ReadmeError("missing README Quick Start section") from error
    match = RUST_BLOCK.search(section)
    if match is None:
        raise ReadmeError("Quick Start has no Rust example")
    return match.group(1)


def run_documented_example(repo_root: Path, source: str) -> None:
    crate_path = (repo_root / CARGO_TOML).parent.resolve().as_posix()
    with tempfile.TemporaryDirectory(prefix="rmatching-readme-") as temporary:
        downstream = Path(temporary)
        (downstream / "src").mkdir()
        (downstream / "Cargo.toml").write_text(
            "[package]\nname = \"rmatching-readme-example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n"
            f"[dependencies]\nrmatching = {{ path = \"{crate_path}\" }}\n",
            encoding="utf-8",
        )
        (downstream / "src" / "main.rs").write_text(source, encoding="utf-8")
        result = subprocess.run(
            ["cargo", "run", "--quiet"],
            cwd=downstream,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    if result.returncode:
        raise ReadmeError(f"documented downstream example failed: {result.stderr.strip()}")


def validate(repo_root: Path) -> None:
    readme_path = repo_root / README
    if not readme_path.is_file():
        raise ReadmeError(f"missing required file: {README.as_posix()}")
    if not (repo_root / CARGO_TOML).is_file():
        raise ReadmeError(f"missing required file: {CARGO_TOML.as_posix()}")

    text = readme_path.read_text(encoding="utf-8")
    validate_local_links(repo_root, text)
    advertised = advertised_features(text)
    actual = cargo_features(repo_root)
    if advertised != actual:
        missing = sorted(actual - advertised)
        extra = sorted(advertised - actual)
        details = []
        if missing:
            details.append(f"missing={','.join(missing)}")
        if extra:
            details.append(f"extra={','.join(extra)}")
        raise ReadmeError(f"advertised Cargo features do not match metadata: {' '.join(details)}")
    run_documented_example(repo_root, documented_example(text))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        validate(args.repo_root.resolve())
    except (ReadmeError, OSError, json.JSONDecodeError) as error:
        print(f"FAIL rmatching README: {error}", file=sys.stderr)
        return 1
    print("PASS rmatching README")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
