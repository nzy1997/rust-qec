#!/usr/bin/env python3
"""Validate showcase Markdown structure and repo-relative links."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote, urlsplit


REQUIRED_SHOWCASE_SECTIONS = (
    "What This Shows",
    "Run It",
    "Expected Result",
    "Code",
    "Verification",
    "Limits",
)

PLACEHOLDER_LIMITS = {"", "tbd", "todo", "n/a", "none"}
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*#*\s*$")
INLINE_LINK_RE = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")
REFERENCE_DEF_RE = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)", re.MULTILINE)
PLACEHOLDER_LIMITS_PREFIXES = (
    "tbd",
    "todo",
    "fixme",
    "xxx",
    "to be determined",
    "to be decided",
    "fill in later",
    "coming soon",
)


@dataclass(frozen=True)
class Heading:
    level: int
    title: str
    line_index: int


def find_repo_root(start: Path | None = None) -> Path:
    current = (start or Path(__file__)).resolve()
    if current.is_file():
        current = current.parent
    for candidate in (current, *current.parents):
        if (candidate / "Cargo.toml").is_file() and (candidate / "README.md").is_file():
            return candidate
    raise RuntimeError(f"could not find repository root from {current}")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def normalize_heading(raw: str) -> str:
    return raw.strip().rstrip("#").strip()


def parse_headings(text: str) -> list[Heading]:
    headings: list[Heading] = []
    for index, line in enumerate(text.splitlines()):
        match = HEADING_RE.match(line)
        if match:
            headings.append(
                Heading(
                    level=len(match.group(1)),
                    title=normalize_heading(match.group(2)),
                    line_index=index,
                )
            )
    return headings


def section_body(text: str, heading: Heading, headings: list[Heading]) -> str:
    lines = text.splitlines()
    end = len(lines)
    for next_heading in headings:
        if next_heading.line_index > heading.line_index and next_heading.level <= heading.level:
            end = next_heading.line_index
            break
    return "\n".join(lines[heading.line_index + 1 : end]).strip()


def is_external_or_anchor(target: str) -> bool:
    target = target.strip()
    if not target or target.startswith("#"):
        return True
    parsed = urlsplit(target)
    if parsed.scheme in {"http", "https", "mailto"}:
        return True
    return False


def clean_link_target(raw_target: str) -> str:
    target = raw_target.strip().split()[0]
    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1]
    return unquote(target)


def repo_relative_path(target: str) -> str | None:
    target = clean_link_target(target)
    if is_external_or_anchor(target):
        return None
    parsed = urlsplit(target)
    if parsed.scheme or parsed.netloc:
        return None
    return parsed.path


def markdown_link_targets(text: str) -> list[str]:
    targets = [match.group(1) for match in INLINE_LINK_RE.finditer(text)]
    targets.extend(match.group(1) for match in REFERENCE_DEF_RE.finditer(text))
    return targets


def validate_markdown_links(path: Path, repo_root: Path) -> list[str]:
    text = read_text(path)
    errors: list[str] = []
    for raw_target in markdown_link_targets(text):
        relative = repo_relative_path(raw_target)
        if relative is None:
            continue
        if not relative:
            continue
        resolved = (repo_root / relative).resolve()
        try:
            resolved.relative_to(repo_root.resolve())
        except ValueError:
            errors.append(f"link escapes repository root: {raw_target}")
            continue
        if not resolved.exists():
            errors.append(f"link target does not exist: {raw_target}")
    return errors


def heading_titles_by_level(text: str, level: int) -> set[str]:
    return {heading.title for heading in parse_headings(text) if heading.level == level}


def limits_is_placeholder(body: str) -> bool:
    normalized = re.sub(r"[\s`*_>-]+", " ", body).strip().lower()
    normalized = normalized.rstrip(".:;")
    if normalized in PLACEHOLDER_LIMITS:
        return True
    return any(
        normalized == prefix or re.match(rf"^{re.escape(prefix)}(?:\W|$)", normalized) is not None
        for prefix in PLACEHOLDER_LIMITS_PREFIXES
    )


def links_to_planning_docs(text: str) -> bool:
    for raw_target in markdown_link_targets(text):
        relative = repo_relative_path(raw_target)
        if relative is None:
            continue
        normalized = relative.lstrip("./")
        if normalized.startswith("docs/plans/") or normalized.startswith("docs/superpowers/"):
            return True
    return False


def validate_showcase_page(path: Path, repo_root: Path) -> list[str]:
    text = read_text(path)
    headings = parse_headings(text)
    second_level = {heading.title: heading for heading in headings if heading.level == 2}
    errors = validate_markdown_links(path, repo_root)

    for required in REQUIRED_SHOWCASE_SECTIONS:
        if required not in second_level:
            errors.append(f"missing required section: {required}")

    limits_heading = second_level.get("Limits")
    if limits_heading is None:
        errors.append("missing non-placeholder Limits section")
    else:
        limits_body = section_body(text, limits_heading, headings)
        if limits_is_placeholder(limits_body):
            errors.append("Limits section must contain non-placeholder text")

    return errors


def validate_index(path: Path, repo_root: Path) -> list[str]:
    text = read_text(path)
    headings = heading_titles_by_level(text, 2)
    errors = validate_markdown_links(path, repo_root)
    if "Categories" not in headings:
        errors.append("showcase index missing Categories section")
    if "Page Contract" not in headings:
        errors.append("showcase index missing Page Contract section")
    if links_to_planning_docs(text):
        errors.append("showcase index must not link primary users to planning docs")
    for section in REQUIRED_SHOWCASE_SECTIONS:
        if section not in text:
            errors.append(f"showcase index missing page contract entry: {section}")
    return errors


def validate_template(path: Path, repo_root: Path) -> list[str]:
    text = read_text(path)
    headings = heading_titles_by_level(text, 2)
    errors = validate_markdown_links(path, repo_root)
    for required in REQUIRED_SHOWCASE_SECTIONS:
        if required not in headings:
            errors.append(f"template missing required section heading: {required}")
    return errors
