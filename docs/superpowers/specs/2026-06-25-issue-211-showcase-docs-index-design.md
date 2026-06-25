# Issue 211 Showcase Documentation Index Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #211, a showcase documentation index, page template, and validation skeleton

## Summary

Issue #211 creates the front door for future showcase pages without writing
those individual pages yet. The repository should gain:

- `docs/showcases/README.md` as a stable index of planned showcase categories
- `docs/showcases/_template.md` as the reusable page skeleton for future issues
- `tools/check_showcase_docs.py` as a lightweight checker for page structure and
  repo-relative Markdown links

The checker must expose the entry points that follow-up issues will use:
self-test fixtures, single showcase page validation, directory validation,
README validation, and arbitrary Markdown link validation.

## Current State

The repository already has working examples spread across the root README,
crate docs, tests, QP101 examples, and benchmark folders. There is no
`docs/showcases/` directory and no showcase-specific validation tool.

Relevant current files:

- `README.md` documents top-level workflows and links to existing examples.
- `benchmarks/surface_decoder_compare/README.md` documents benchmark setup.
- `rstim/doc/cli.md` documents CLI usage.
- `tools/` contains small Python utilities, each with a direct `argparse`
  command-line interface and no shared helper package.

There are no comments on issue #211 and no existing pull request for this
branch.

## Goals

- Create a concise showcase index at `docs/showcases/README.md`.
- Keep `docs/plans/` and `docs/superpowers/` out of that primary user path.
- Create a reusable template at `docs/showcases/_template.md`.
- Require individual showcase pages to include these exact second-level
  sections:
  - `What This Shows`
  - `Run It`
  - `Expected Result`
  - `Code`
  - `Verification`
  - `Limits`
- Reject individual showcase pages with missing required sections.
- Reject individual showcase pages whose `Limits` section is empty or
  placeholder content.
- Validate repo-relative Markdown links from the repository root.
- Treat the index and template explicitly instead of applying the individual
  page contract to them by accident.
- Include self-test fixtures for:
  - one valid showcase page
  - missing `Expected Result`
  - missing `Limits`
  - placeholder `Limits`
  - nonexistent repo-relative link
- Support these commands:

```sh
python3 tools/check_showcase_docs.py --self-test
python3 tools/check_showcase_docs.py docs/showcases/README.md
python3 tools/check_showcase_docs.py docs/showcases
python3 tools/check_showcase_docs.py --readme README.md
python3 tools/check_showcase_docs.py --links docs/showcases/README.md
```

## Non-Goals

- Do not write individual showcase pages in this issue.
- Do not add a website generator or documentation build pipeline.
- Do not add third-party Python dependencies.
- Do not validate external HTTP links.
- Do not reorganize existing README, crate docs, benchmarks, tests, or
  `docs/plans/` content.

## Approaches Considered

### 1. Purpose-built Python checker with explicit document types

Create a single `tools/check_showcase_docs.py` script using only the Python
standard library. It classifies `docs/showcases/README.md`,
`docs/showcases/_template.md`, and ordinary showcase pages explicitly. The
script validates individual showcase sections, rejects placeholder `Limits`
content, and checks repo-relative Markdown links.

Benefits:

- matches the requested interface exactly
- avoids new dependencies
- gives follow-up issues a stable local command
- keeps the index/template exceptions intentional and testable

Costs:

- Markdown parsing is intentionally lightweight, so the checker covers the link
  and heading patterns this repository uses rather than the full CommonMark
  grammar

This is the chosen approach.

### 2. Generic Markdown linter configuration

Add a generic Markdown linter and encode showcase-specific rules in its
configuration.

Benefits:

- useful for broad Markdown style validation later

Costs:

- adds tooling beyond the issue's skeleton request
- does not naturally express the showcase-page section contract
- makes self-test fixtures and repo-relative link semantics less direct

This is rejected.

### 3. Documentation-only index and template, no checker yet

Create the index and template now, leaving validation to follow-up issues.

Benefits:

- smallest initial diff

Costs:

- conflicts with the issue objective
- leaves follow-up issues without the required validation modes
- does not provide the negative controls requested by issue #211

This is rejected.

## Documentation Design

`docs/showcases/README.md` should:

- use a clear title such as `# Showcase Index`
- state that individual pages will be added by follow-up issues
- list planned categories that map to existing repository areas:
  - Simulator and CLI workflows
  - Visualization and QP101 artifacts
  - Decoder and benchmark workflows
  - Code construction workflows
- for each category, describe what examples in that category should
  demonstrate and where future showcase pages should point
- link only to stable user-facing repository paths such as `README.md`,
  `rstim/doc/cli.md`, `benchmarks/surface_decoder_compare/README.md`,
  `qp101-viz/README.md`, and crate or benchmark directories
- link to `_template.md`
- avoid links into `docs/plans/` and `docs/superpowers/`

`docs/showcases/_template.md` should:

- use a title that clearly marks it as a template
- contain the six required showcase page sections as `##` headings
- include concise instructional prose in each section
- include a `Limits` section that tells future authors to write real
  constraints and avoid placeholder text

The template is intentionally not validated as an individual page. It contains
authoring guidance rather than a real showcase result.

## Checker Design

`tools/check_showcase_docs.py` should be a focused Python script with these
responsibilities:

- discover the repository root from the script location
- parse Markdown headings into section ranges
- extract inline and reference-style Markdown links that are repo-relative
- ignore external URLs, mail links, anchors, and local page anchors
- strip fragment identifiers before resolving paths
- resolve repo-relative links from the repository root
- validate file or directory existence for repo-relative links
- validate one individual showcase page by enforcing the six required section
  headings and the non-placeholder `Limits` section
- validate `docs/showcases/README.md` as an index with expected top-level
  structure and valid links
- validate `docs/showcases/_template.md` as a template with the required
  headings and valid links, without rejecting instructional placeholder words
- validate a directory by checking the index, template, and any other Markdown
  showcase pages in that directory
- support `--readme` as link validation plus a light README shape check
- support `--links` as arbitrary Markdown link validation only
- support `--self-test` with temporary fixtures and expected failures

The checker should print `ok: <path>` for successful validations and
`error: <path>: <message>` for failures. It should return exit code `1` when
any validation fails.

## Testing Design

Self-test is the main test harness for the checker. It should create temporary
fixture Markdown files under a temporary repository-like root so it can test
valid links and missing links without modifying the working tree.

Self-test cases:

- valid showcase page with all six required sections and a valid repo-relative
  link
- invalid page missing `Expected Result`
- invalid page missing `Limits`
- invalid page with `Limits` set to `TBD`
- invalid page linking to a nonexistent repo path
- index fixture that is handled as an index rather than as an individual
  showcase page
- template fixture that is handled as a template rather than as an individual
  showcase page

Required verification commands:

```sh
python3 tools/check_showcase_docs.py --self-test
python3 tools/check_showcase_docs.py docs/showcases/README.md
python3 tools/check_showcase_docs.py docs/showcases
python3 tools/check_showcase_docs.py --links docs/showcases/README.md
cargo test
```

Also run:

```sh
python3 tools/check_showcase_docs.py --readme README.md
git diff --check
```
