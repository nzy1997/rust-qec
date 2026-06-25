# Issue 211 Showcase Documentation Index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a showcase documentation index, reusable page template, and validation checker for future showcase pages.

**Architecture:** Keep user-facing showcase documentation under `docs/showcases/` and keep validation in one standard-library Python script under `tools/`. The checker classifies the showcase index, template, ordinary showcase pages, README validation, and arbitrary link checks explicitly so future showcase pages can rely on stable command modes.

**Tech Stack:** Markdown, Python 3 standard library, existing Cargo workspace verification.

## Global Constraints

- Create `docs/showcases/README.md`, `docs/showcases/_template.md`, and `tools/check_showcase_docs.py`.
- Do not write individual showcase pages in this issue.
- Individual showcase pages require these exact `##` sections: `What This Shows`, `Run It`, `Expected Result`, `Code`, `Verification`, and `Limits`.
- `Limits` must contain real non-placeholder text; empty content, `TBD`, and `TODO` fail.
- Repo-relative Markdown links resolve from the workspace root.
- The checker must explicitly handle the showcase index and template instead of applying the individual page rules to them accidentally.
- The checker must support `--self-test`, positional single-file validation, positional directory validation, `--readme`, and `--links`.
- The showcase index must not put `docs/plans/` or `docs/superpowers/` in the primary user path.
- Required issue verification commands:
  - `python3 tools/check_showcase_docs.py --self-test`
  - `python3 tools/check_showcase_docs.py docs/showcases/README.md`
  - `python3 tools/check_showcase_docs.py docs/showcases`
  - `python3 tools/check_showcase_docs.py --links docs/showcases/README.md`
- Additional requested verification command: `cargo test`.

---

### Task 1: Add Showcase Index And Template

**Files:**
- Create: `docs/showcases/README.md`
- Create: `docs/showcases/_template.md`

**Interfaces:**
- Produces: `docs/showcases/README.md` showcase index accepted by `validate_index(path: Path, repo_root: Path) -> list[str]`.
- Produces: `docs/showcases/_template.md` template accepted by `validate_template(path: Path, repo_root: Path) -> list[str]`.

- [ ] **Step 1: Create `docs/showcases/README.md`**

Create `docs/showcases/README.md` with this content:

```markdown
# Showcase Index

This directory is the stable front door for runnable rstim workspace
showcases. Each showcase page should explain what the example demonstrates,
how to run it, where the result appears, which code owns it, how to verify it,
and what its limits are.

Individual showcase pages will be added by follow-up issues. Use
[`_template.md`](_template.md) for new pages.

## Categories

### Simulator And CLI Workflows

Showcases in this category should demonstrate `rstim` circuit simulation,
sampling, detector event extraction, detector error model analysis, and CLI
input/output behavior.

Primary code and docs:

- [`rstim/`](rstim/)
- [`rstim/doc/getting_started.md`](rstim/doc/getting_started.md)
- [`rstim/doc/cli.md`](rstim/doc/cli.md)
- [`README.md`](README.md)

### Visualization And QP101 Artifacts

Showcases in this category should demonstrate built-in SVG rendering, seeded
sample-shot overlays, detector-error-model highlights, QP101 JSON export, and
the optional Typst renderer fixtures.

Primary code and docs:

- [`rstim/`](rstim/)
- [`qp101-viz/`](qp101-viz/)
- [`qp101-viz/README.md`](qp101-viz/README.md)
- [`qp101-viz/examples/`](qp101-viz/examples/)

### Decoder And Benchmark Workflows

Showcases in this category should demonstrate decoder integrations, benchmark
harnesses, smoke runs, and reproducible comparison outputs.

Primary code and docs:

- [`rsinter/`](rsinter/)
- [`rmatching/`](rmatching/)
- [`rbposd/`](rbposd/)
- [`rilpqec/`](rilpqec/)
- [`benchmarks/surface_decoder_compare/`](benchmarks/surface_decoder_compare/)
- [`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md)

### Code Construction Workflows

Showcases in this category should demonstrate CSS code construction, built-in
code families, sparse support exports, distance checks, and code-oriented CLI
flows.

Primary code and docs:

- [`qec-code/`](qec-code/)
- [`qec-ilp-core/`](qec-ilp-core/)
- [`docs/apm_decoder_hierarchy.md`](docs/apm_decoder_hierarchy.md)
- [`docs/bb144_circuit_bposd_reproduction.md`](docs/bb144_circuit_bposd_reproduction.md)

## Page Contract

Every individual showcase page must include these sections:

- `What This Shows`
- `Run It`
- `Expected Result`
- `Code`
- `Verification`
- `Limits`

Run the checker before opening a showcase documentation pull request:

```sh
python3 tools/check_showcase_docs.py docs/showcases
```
```

- [ ] **Step 2: Create `docs/showcases/_template.md`**

Create `docs/showcases/_template.md` with this content:

```markdown
# Showcase Title

Use this template for one runnable showcase. Replace this opening paragraph
with a short summary of the user-facing workflow.

## What This Shows

Describe the behavior, workflow, or result the showcase demonstrates. Mention
the intended audience when that affects how to read the result.

## Run It

List exact commands from the repository root. Prefer commands that a new user
can run after the normal workspace setup.

```sh
python3 tools/check_showcase_docs.py docs/showcases
```

## Expected Result

Describe the visible output, generated file, benchmark artifact, or terminal
result a successful run should produce.

## Code

Link to the owning code, examples, tests, fixtures, and documentation. Use
repo-relative links from the workspace root.

## Verification

List the command or commands that prove the showcase still works. Include
expected success criteria that are more specific than only "command exits 0".

## Limits

State real constraints, assumptions, cost, runtime, platform expectations, or
known gaps. Do not leave this section empty, and do not use placeholder text.
```

- [ ] **Step 3: Commit the documentation skeleton**

Run:

```sh
git add docs/showcases/README.md docs/showcases/_template.md
git commit -m "docs: add showcase index skeleton"
```

Expected: commit succeeds with only the two new Markdown files.

---

### Task 2: Add Checker Core Validation

**Files:**
- Create: `tools/check_showcase_docs.py`

**Interfaces:**
- Produces: `REQUIRED_SHOWCASE_SECTIONS: tuple[str, ...]`.
- Produces: `find_repo_root(start: Path | None = None) -> Path`.
- Produces: `validate_markdown_links(path: Path, repo_root: Path) -> list[str]`.
- Produces: `validate_showcase_page(path: Path, repo_root: Path) -> list[str]`.
- Produces: `validate_index(path: Path, repo_root: Path) -> list[str]`.
- Produces: `validate_template(path: Path, repo_root: Path) -> list[str]`.

- [ ] **Step 1: Create the checker with parser and validation helpers**

Create `tools/check_showcase_docs.py` with this implementation skeleton:

```python
#!/usr/bin/env python3
"""Validate showcase Markdown structure and repo-relative links."""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
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
```

- [ ] **Step 2: Add showcase, index, and template validation functions**

Append these functions:

```python
def heading_titles_by_level(text: str, level: int) -> set[str]:
    return {heading.title for heading in parse_headings(text) if heading.level == level}


def limits_is_placeholder(body: str) -> bool:
    normalized = re.sub(r"[\s`*_>-]+", " ", body).strip().lower()
    normalized = normalized.rstrip(".:;")
    return normalized in PLACEHOLDER_LIMITS


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
    if "docs/plans/" in text or "docs/superpowers/" in text:
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
```

- [ ] **Step 3: Run syntax check**

Run:

```sh
python3 -m py_compile tools/check_showcase_docs.py
```

Expected: command exits 0.

---

### Task 3: Add CLI Modes, Directory Validation, And Self-Test

**Files:**
- Modify: `tools/check_showcase_docs.py`

**Interfaces:**
- Consumes: validation functions from Task 2.
- Produces: `validate_path(path: Path, repo_root: Path) -> list[str]`.
- Produces: `validate_directory(path: Path, repo_root: Path) -> list[tuple[Path, list[str]]]`.
- Produces: CLI modes required by issue #211.

- [ ] **Step 1: Add dispatch and CLI functions**

Append this code to `tools/check_showcase_docs.py`:

```python
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
```

- [ ] **Step 2: Add self-test fixture helpers**

Append this code:

```python
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


def run_self_test() -> list[str]:
    errors: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_fixture(root, "Cargo.toml", "[workspace]\n")
        write_fixture(root, "README.md", "# Fixture Root\n")
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
        bad_link = write_fixture(
            root,
            "docs/showcases/bad-link.md",
            VALID_SHOWCASE.replace("[`README.md`](README.md)", "[missing](missing/file.md)"),
        )
        index = write_fixture(
            root,
            "docs/showcases/README.md",
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
        expected_failures = [
            (missing_expected, "Expected Result"),
            (missing_limits, "Limits"),
            (placeholder_limits, "non-placeholder"),
            (bad_link, "does not exist"),
        ]
        for fixture, expected_error in expected_failures:
            fixture_errors = validate_showcase_page(fixture, root)
            if not any(expected_error in error for error in fixture_errors):
                errors.append(f"{fixture.name} did not fail with {expected_error}: {fixture_errors}")
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
```

- [ ] **Step 3: Add `main`**

Append this code:

```python
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
```

- [ ] **Step 4: Run the self-test**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
```

Expected: exits 0 and prints `ok: self-test`.

- [ ] **Step 5: Commit the checker**

Run:

```sh
git add tools/check_showcase_docs.py
git commit -m "feat: add showcase docs checker"
```

Expected: commit succeeds with the new checker script.

---

### Task 4: Run Required Validation And Polish Failures

**Files:**
- Modify if needed: `docs/showcases/README.md`
- Modify if needed: `docs/showcases/_template.md`
- Modify if needed: `tools/check_showcase_docs.py`

**Interfaces:**
- Consumes: all files from prior tasks.
- Produces: passing issue verification commands.

- [ ] **Step 1: Run the required issue verification commands**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
python3 tools/check_showcase_docs.py docs/showcases/README.md
python3 tools/check_showcase_docs.py docs/showcases
python3 tools/check_showcase_docs.py --links docs/showcases/README.md
```

Expected: all commands exit 0.

- [ ] **Step 2: Run README mode**

Run:

```sh
python3 tools/check_showcase_docs.py --readme README.md
```

Expected: command exits 0.

- [ ] **Step 3: Run broad repository verification**

Run:

```sh
cargo test
git diff --check
```

Expected: both commands exit 0.

- [ ] **Step 4: Commit validation polish if any files changed**

Run:

```sh
git status --short
```

Expected: no output. If earlier verification required fixes, commit only those
fixes with:

```sh
git add docs/showcases/README.md docs/showcases/_template.md tools/check_showcase_docs.py
git commit -m "fix: polish showcase docs validation"
```
