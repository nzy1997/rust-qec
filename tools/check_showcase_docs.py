#!/usr/bin/env python3
"""Validate showcase Markdown structure and repo-relative links."""

from __future__ import annotations

import re
import argparse
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote, urlsplit

try:
    from tools import check_site_manifest
except ModuleNotFoundError:
    import check_site_manifest  # type: ignore[no-redef]


REQUIRED_SHOWCASE_SECTIONS = (
    "What This Shows",
    "Run It",
    "Expected Result",
    "Code",
    "Verification",
    "Limits",
)

REQUIRED_INDEX_SECTIONS = (
    "Categories",
    "Documentation Follow-Up Policy",
    "Page Contract",
)

RSTIM_VS_STIM_SHOWCASE = Path("docs/showcases/rstim-vs-stim-simulator.md")
RSTIM_VS_STIM_COMMAND_REQUIREMENTS = (
    (
        "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness",
        "missing rstim-vs-Stim correctness command link",
    ),
    (
        "cargo run -p rstim --bin rstim -- perf run",
        "missing rstim-vs-Stim speed command link",
    ),
)

PLACEHOLDER_LIMITS = {"", "tbd", "todo", "n/a", "none"}
LIMITS_NORMALIZATION_RE = re.compile(r"[\s`*_>.,:;!()\[\]-]+")
BOILERPLATE_LIMITS = {
    "state real constraints assumptions cost runtime platform expectations or known gaps do not leave this section empty and do not use placeholder text",
    "state real constraints assumptions cost runtime platform expectations known gaps and follow up issue links for uncertainties readers should know about do not leave this section empty and do not use placeholder text",
}
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
    resolved_repo_root = repo_root.resolve()
    for raw_target in markdown_link_targets(text):
        relative = repo_relative_path(raw_target)
        if relative is None:
            continue
        if not relative:
            continue
        repo_candidate = (repo_root / relative).resolve()
        if not repo_candidate.is_relative_to(resolved_repo_root):
            errors.append(f"link escapes repository root: {raw_target}")
            continue
        if not repo_candidate.exists():
            errors.append(f"link target does not exist: {raw_target}")
    return errors


def heading_titles_by_level(text: str, level: int) -> set[str]:
    return {heading.title for heading in parse_headings(text) if heading.level == level}


def normalize_limits_body(body: str) -> str:
    return LIMITS_NORMALIZATION_RE.sub(" ", body).strip().lower()


def limits_is_placeholder(body: str) -> bool:
    normalized = normalize_limits_body(body)
    if normalized in PLACEHOLDER_LIMITS or normalized in BOILERPLATE_LIMITS:
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


def is_rstim_vs_stim_showcase(path: Path, repo_root: Path) -> bool:
    try:
        relative = path.resolve().relative_to(repo_root.resolve())
    except ValueError:
        return False
    return relative == RSTIM_VS_STIM_SHOWCASE


def validate_rstim_vs_stim_commands(path: Path, repo_root: Path, text: str) -> list[str]:
    if not is_rstim_vs_stim_showcase(path, repo_root):
        return []
    errors = [
        error
        for required_command, error in RSTIM_VS_STIM_COMMAND_REQUIREMENTS
        if required_command not in text
    ]
    match = check_site_manifest.find_broad_rstim_vs_stim_claim(text)
    if match is not None:
        errors.append(f"broad rstim-vs-Stim claim is not allowed: {match}")
    return errors


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

    errors.extend(validate_rstim_vs_stim_commands(path, repo_root, text))
    return errors


def validate_index(path: Path, repo_root: Path) -> list[str]:
    text = read_text(path)
    headings = heading_titles_by_level(text, 2)
    errors = validate_markdown_links(path, repo_root)
    for section in REQUIRED_INDEX_SECTIONS:
        if section not in headings:
            errors.append(f"showcase index missing {section} section")
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


def validate_readme(path: Path, repo_root: Path) -> list[str]:
    text = read_text(path)
    errors = validate_markdown_links(path, repo_root)
    if path.name == "README.md" and not text.lstrip().startswith("# "):
        errors.append("README should start with a top-level heading")
    return errors


def validate_path(path: Path, repo_root: Path) -> list[str]:
    resolved = path.resolve()
    index = (repo_root / "docs/showcases/README.md").resolve()
    template = (repo_root / "docs/showcases/_template.md").resolve()
    if resolved == index:
        return validate_index(path, repo_root)
    if resolved == template:
        return validate_template(path, repo_root)
    return validate_showcase_page(path, repo_root)


def validate_directory(path: Path, repo_root: Path) -> list[tuple[Path, list[str]]]:
    results: list[tuple[Path, list[str]]] = []
    markdown_files = sorted(path.glob("*.md"))
    if not markdown_files:
        return [(path, ["directory contains no Markdown files"])]
    for markdown in markdown_files:
        results.append((markdown, validate_path(markdown, repo_root)))
    return results


def print_results(results: list[tuple[Path, list[str]]]) -> int:
    failed = False
    for path, errors in results:
        if errors:
            failed = True
            for error in errors:
                print(f"error: {path}: {error}", file=sys.stderr)
        else:
            print(f"ok: {path}")
    return 1 if failed else 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate showcase Markdown structure and repo-relative links."
    )
    parser.add_argument("paths", nargs="*", type=Path, help="Showcase file or directory")
    parser.add_argument("--self-test", action="store_true", help="Run built-in fixtures")
    parser.add_argument("--readme", nargs="+", type=Path, help="Validate README-style Markdown")
    parser.add_argument("--links", nargs="+", type=Path, help="Validate Markdown links only")
    return parser.parse_args(argv)


def write_fixture(root: Path, relative: str, text: str) -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


VALID_SHOWCASE = """# Valid Showcase

## What This Shows

This shows a runnable workflow.

## Run It

```sh
python3 tools/check_showcase_docs.py --self-test
```

## Expected Result

The command prints ok lines for valid fixtures.

## Code

See [`README.md`](README.md).

## Verification

Run the self-test command and expect exit code 0.

## Limits

This fixture covers checker structure only, not full documentation prose.
"""

RSTIM_VS_STIM_VALID_SHOWCASE = """# rstim-vs-Stim Simulator Evidence

## What This Shows

This fixture shows the specialized command contract.

## Run It

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness \\
  --cases benchmarks/rstim_vs_stim_simulator/cases.smoke.toml \\
  --shots 20000 \\
  --out /tmp/rstim-vs-stim-correctness.json

cargo run -p rstim --bin rstim -- perf run \\
  --case stim-style-surface-sample-d11-r100-b1024 \\
  --warmup-rounds 0 \\
  --measure-rounds 1 \\
  --out /tmp/rstim-vs-stim-speed.jsonl
```

## Expected Result

The commands write reviewer-readable evidence.

## Code

See [`benchmarks/rstim_vs_stim_simulator/README.md`](benchmarks/rstim_vs_stim_simulator/README.md).

## Verification

Run the showcase checker.

## Limits

This fixture covers checker command validation only.
"""


def run_self_test() -> list[str]:
    errors: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_fixture(root, "Cargo.toml", "[workspace]\n")
        write_fixture(root, "README.md", "# Fixture Root\n")
        write_fixture(
            root,
            "benchmarks/rstim_vs_stim_simulator/README.md",
            "# Rstim vs Stim Simulator\n",
        )
        showcase_dir = root / "docs/showcases"
        valid = write_fixture(root, "docs/showcases/valid.md", VALID_SHOWCASE)
        missing_expected = write_fixture(
            root,
            "docs/showcases/missing-expected.md",
            VALID_SHOWCASE.replace("## Expected Result\n\nThe command prints ok lines for valid fixtures.\n\n", ""),
        )
        missing_limits = write_fixture(
            root,
            "docs/showcases/missing-limits.md",
            VALID_SHOWCASE.replace(
                "## Limits\n\nThis fixture covers checker structure only, not full documentation prose.\n",
                "",
            ),
        )
        placeholder_limits = write_fixture(
            root,
            "docs/showcases/placeholder-limits.md",
            VALID_SHOWCASE.replace(
                "This fixture covers checker structure only, not full documentation prose.",
                "TBD",
            ),
        )
        template_limits_text = (
            "State real constraints, assumptions, cost, runtime, platform expectations, or\n"
            "known gaps. Do not leave this section empty, and do not use placeholder text."
        )
        boilerplate_limits = write_fixture(
            root,
            "docs/showcases/boilerplate-limits.md",
            VALID_SHOWCASE.replace(
                "This fixture covers checker structure only, not full documentation prose.",
                template_limits_text,
            ),
        )
        rstim_vs_stim = write_fixture(
            root,
            "docs/showcases/rstim-vs-stim-simulator.md",
            RSTIM_VS_STIM_VALID_SHOWCASE,
        )
        bad_link = write_fixture(
            root,
            "docs/showcases/bad-link.md",
            VALID_SHOWCASE.replace("[`README.md`](README.md)", "[missing](missing/file.md)"),
        )
        index = write_fixture(
            root,
            "docs/showcases/README.md",
            "# Showcase Index\n\n## Categories\n\n### Example\n\nSee [`README.md`](README.md).\n\n"
            "## Documentation Follow-Up Policy\n\n"
            "Write only high-confidence existing behavior. Open follow-up issues for claims "
            "that need algorithm review, benchmark interpretation, or scientific review.\n\n"
            "## Page Contract\n\n"
            + "\n".join(f"- `{section}`" for section in REQUIRED_SHOWCASE_SECTIONS)
            + "\n",
        )
        index_missing_policy = write_fixture(
            root,
            "docs/showcases/index-missing-policy.md",
            "# Showcase Index\n\n## Categories\n\n### Example\n\nSee [`README.md`](README.md).\n\n## Page Contract\n\n"
            + "\n".join(f"- `{section}`" for section in REQUIRED_SHOWCASE_SECTIONS)
            + "\n",
        )
        template = write_fixture(
            root,
            "docs/showcases/_template.md",
            "# Showcase Title\n\n"
            + "\n\n".join(f"## {section}\n\nAuthor guidance." for section in REQUIRED_SHOWCASE_SECTIONS)
            + "\n",
        )

        if validate_showcase_page(valid, root):
            errors.append("valid showcase fixture should pass")
        if validate_showcase_page(rstim_vs_stim, root):
            errors.append("rstim-vs-Stim showcase fixture should pass")
        for phrase in ("rstim is faster than Stim", "rstim beats Stim", "full Stim parity"):
            rstim_vs_stim.write_text(
                RSTIM_VS_STIM_VALID_SHOWCASE + f"\n{phrase}\n",
                encoding="utf-8",
            )
            broad_claim_errors = validate_showcase_page(rstim_vs_stim, root)
            if not any(
                "broad rstim-vs-Stim claim is not allowed" in error
                for error in broad_claim_errors
            ):
                errors.append(
                    f"rstim-vs-Stim fixture with broad claim {phrase!r} did not fail: "
                    f"{broad_claim_errors}"
                )
        rstim_vs_stim.write_text(
            RSTIM_VS_STIM_VALID_SHOWCASE.replace(
                "python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness",
                "python3 -m benchmarks.rstim_vs_stim_simulator.missing_correctness",
            ),
            encoding="utf-8",
        )
        missing_correctness_errors = validate_showcase_page(rstim_vs_stim, root)
        if not any("correctness command" in error for error in missing_correctness_errors):
            errors.append(
                "rstim-vs-Stim fixture without correctness command did not fail: "
                f"{missing_correctness_errors}"
            )
        rstim_vs_stim.write_text(
            RSTIM_VS_STIM_VALID_SHOWCASE.replace(
                "cargo run -p rstim --bin rstim -- perf run",
                "cargo run -p rstim --bin rstim -- perf missing-run",
            ),
            encoding="utf-8",
        )
        missing_speed_errors = validate_showcase_page(rstim_vs_stim, root)
        if not any("speed command" in error for error in missing_speed_errors):
            errors.append(
                "rstim-vs-Stim fixture without speed command did not fail: "
                f"{missing_speed_errors}"
            )
        rstim_vs_stim.write_text(RSTIM_VS_STIM_VALID_SHOWCASE, encoding="utf-8")
        expected_failures = [
            (missing_expected, "Expected Result"),
            (missing_limits, "Limits"),
            (placeholder_limits, "non-placeholder"),
            (boilerplate_limits, "non-placeholder"),
            (bad_link, "does not exist"),
        ]
        for fixture, expected_error in expected_failures:
            fixture_errors = validate_showcase_page(fixture, root)
            if not any(expected_error in error for error in fixture_errors):
                errors.append(f"{fixture.name} did not fail with {expected_error}: {fixture_errors}")
        missing_policy_errors = validate_index(index_missing_policy, root)
        if not any("Documentation Follow-Up Policy" in error for error in missing_policy_errors):
            errors.append(
                "index without policy did not fail with Documentation Follow-Up Policy: "
                f"{missing_policy_errors}"
            )
        if validate_path(index, root):
            errors.append("index fixture should pass index validation")
        if validate_path(template, root):
            errors.append("template fixture should pass template validation")
        directory_errors = [
            error
            for markdown_path, path_errors in validate_directory(showcase_dir, root)
            if markdown_path.name in {"README.md", "_template.md", "valid.md"}
            for error in path_errors
        ]
        if directory_errors:
            errors.append(f"directory validation failed for valid fixtures: {directory_errors}")
    return errors


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    selected_modes = sum(
        1
        for active in (
            args.self_test,
            bool(args.readme),
            bool(args.links),
            bool(args.paths),
        )
        if active
    )
    if selected_modes != 1:
        print("error: choose exactly one validation mode", file=sys.stderr)
        return 2

    repo_root = find_repo_root()

    if args.self_test:
        errors = run_self_test()
        if errors:
            for error in errors:
                print(f"error: self-test: {error}", file=sys.stderr)
            return 1
        print("ok: self-test")
        return 0

    results: list[tuple[Path, list[str]]] = []
    if args.readme:
        results = [(path, validate_readme(path, repo_root)) for path in args.readme]
    elif args.links:
        results = [(path, validate_markdown_links(path, repo_root)) for path in args.links]
    else:
        for path in args.paths:
            if path.is_dir():
                results.extend(validate_directory(path, repo_root))
            else:
                results.append((path, validate_path(path, repo_root)))
    return print_results(results)


if __name__ == "__main__":
    raise SystemExit(main())
