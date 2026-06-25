# Issue 213 README Entry Point Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #213, refocusing the root README as the external user entry point

## Summary

Issue #213 should turn `README.md` into a concise front door for first-time
external users. The README should quickly answer what the repository can do,
show where the major workspace areas live, provide a minimal quick start, and
then point readers to the deeper documentation that already owns detailed
workflows.

The README should link to:

- `docs/showcases/README.md`
- `rstim/doc/getting_started.md`
- `rstim/doc/cli.md`
- crate and benchmark documentation

It should keep the stable command examples requested by the issue and avoid
moving benchmark implementation details, Pages/gallery generation, algorithm
explanations, or maintainer release notes into the first-time-user path.

## Current State

The current root README mixes several audiences:

- a workspace overview
- quick-start commands
- common CLI workflows
- static SVG and atom-loss visualization details
- Stim parity and benchmark evidence
- maintainer release instructions

The dependency for this issue is satisfied: issue #211 is closed, PR #226 is
merged, and the local checkout contains `docs/showcases/README.md`,
`docs/showcases/_template.md`, and `tools/check_showcase_docs.py`.

There are no comments on issue #213. Issue #211 also has no comments.

## Goals

- Keep `README.md` short enough to act as an external entry point.
- Open with a capability summary for simulator, CLI, circuit generation,
  visualization, decoding, and benchmarking workflows.
- Keep a workspace map with the major crates and documentation paths.
- Keep stable commands:
  - `cargo build --workspace`
  - `cargo test --workspace`
  - a small `rstim stats` example
- Link readers to the primary next steps:
  - `docs/showcases/README.md`
  - `rstim/doc/getting_started.md`
  - `rstim/doc/cli.md`
  - `rmatching/README.md`
  - `benchmarks/surface_decoder_compare/README.md`
- Preserve valid `render_svg` and `export_json` wording.
- Remove stale `rstim svg_render` wording if present.
- Keep all Markdown links valid under `tools/check_showcase_docs.py --readme`.

## Non-Goals

- Do not modify Rust code, CLI behavior, benchmark harnesses, visualization
  generation, or showcase checker behavior.
- Do not move Pages gallery generation into README.
- Do not move benchmark implementation details or algorithm explanations into
  README.
- Do not keep maintainer release flow as a primary README section.
- Do not link first-time users from README into `docs/plans/` or
  `docs/superpowers/`.

## Approaches Considered

### 1. Concise README front door with links to owned docs

Rewrite the README around four sections: capabilities, workspace map, quick
start, and primary next steps. Keep only stable command snippets and redirect
workflow depth to the showcase index, crate docs, CLI reference, decoder docs,
and benchmark docs.

Benefits:

- matches the issue objective directly
- keeps README maintainable as the workspace grows
- uses the new showcase index from issue #211
- avoids duplicating CLI, visualization, benchmark, and release documentation

Costs:

- some detailed examples leave the README and become link targets instead

This is the chosen approach.

### 2. Keep the long README and add a navigation section

Add a short "where to go next" block while retaining the existing workflow,
visualization, benchmark, and maintainer sections.

Benefits:

- smallest textual deletion

Costs:

- does not solve the first-time-user entry-point problem
- keeps multiple audiences mixed in one page
- conflicts with the issue objective to refocus README

This is rejected.

### 3. Split README details into new documents

Move detailed README sections into new docs and leave stubs in README.

Benefits:

- preserves more content in the repository

Costs:

- broader documentation migration than requested
- risks creating new ownership boundaries instead of linking to existing docs
- out of scope for this issue

This is rejected.

## README Design

`README.md` should use this shape:

1. Title, badges, and a short description of the workspace.
2. "What You Can Do" with concise bullets for simulation, CLI inspection,
   circuit generation, visualization, decoding, and benchmarks.
3. "Workspace Map" table with stable paths and user-facing roles.
4. "Quick Start" with build, `rstim stats`, and workspace test commands.
5. "Primary Next Steps" with 3 to 5 links to owned documentation.
6. "Notes" with only brief orientation about `render_svg`, `export_json`, and
   benchmark smoke commands if needed for link context.

The README should not include long atom-loss examples, DEM-highlight examples,
parity evidence, Pages gallery generation instructions, or release flow. Those
details are already better owned by `rstim/doc/cli.md`, showcase docs,
benchmark docs, `qp101-viz/README.md`, or maintainer-only project workflows.

## Verification Design

Required verification commands:

```sh
python3 tools/check_showcase_docs.py --readme README.md
cargo test -p rstim render_svg_documented_workflow_matches_cli -q
cargo test
```

Additional documentation checks:

```sh
git diff --check
```

Negative control:

```sh
tmp=$(mktemp)
cp README.md "$tmp"
python3 - <<'PY'
from pathlib import Path
path = Path("README.md")
text = path.read_text()
path.write_text(text.replace("docs/showcases/README.md", "docs/showcases/missing.md", 1))
PY
python3 tools/check_showcase_docs.py --readme README.md
cp "$tmp" README.md
rm "$tmp"
```

The negative-control checker run must fail with a missing-link error, then the
original README must be restored before final verification and commit.
