# Issue 213 README Entry Point Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the root README as a concise external-facing entry point for the rstim workspace.

**Architecture:** Keep the root README as a short orientation page and route detailed workflows to the existing showcase index, simulator docs, CLI reference, decoder docs, and benchmark docs. Do not introduce new validation tooling; use the issue #211 checker and existing Rust tests to verify README links and CLI wording.

**Tech Stack:** Markdown, Python 3 standard library checker, Cargo workspace verification.

## Global Constraints

- Modify only `README.md` for the user-facing implementation.
- Keep `README.md` concise and external-facing.
- Include a short capability summary, workspace map, quick start, and 3-5 primary next-step links.
- Link to `docs/showcases/README.md`, `rstim/doc/getting_started.md`, `rstim/doc/cli.md`, `rmatching/README.md`, and `benchmarks/surface_decoder_compare/README.md`.
- Keep stable commands: `cargo build --workspace`, `cargo test --workspace`, and a small `rstim stats` example.
- Keep valid `render_svg` and `export_json` wording.
- Do not include stale `rstim svg_render` wording.
- Do not move Pages gallery generation, benchmark implementation, algorithm explanations, or maintainer release notes into README.
- Do not add README links to `docs/plans/` or `docs/superpowers/`.
- Required issue verification commands:
  - `python3 tools/check_showcase_docs.py --readme README.md`
  - `cargo test -p rstim render_svg_documented_workflow_matches_cli -q`
- Additional Agent Desk verification command: `cargo test`.
- Negative control: changing the README showcase link to a missing file must make the checker fail.

---

### Task 1: Rewrite README As External Entry Point

**Files:**
- Modify: `README.md`
- Test: existing `tools/check_showcase_docs.py`
- Test: existing Rust test `render_svg_documented_workflow_matches_cli`

**Interfaces:**
- Consumes: `docs/showcases/README.md` as the showcase index linked from README.
- Consumes: `rstim/doc/cli.md` as the owner of detailed CLI, `render_svg`, and `export_json` usage.
- Produces: `README.md` that passes README link validation and gives external users a concise navigation surface.

- [ ] **Step 1: Run the focused RED check**

Run:

```sh
python3 - <<'PY'
from pathlib import Path
text = Path("README.md").read_text(encoding="utf-8")
assert "docs/showcases/README.md" in text, "README must link to docs/showcases/README.md"
PY
```

Expected: FAIL with `AssertionError: README must link to docs/showcases/README.md`.

- [ ] **Step 2: Replace `README.md`**

Replace `README.md` with:

```markdown
# rstim

[![CI](https://github.com/nzy1997/rstim/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rstim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rstim/branch/master/graph/badge.svg)](https://codecov.io/gh/nzy1997/rstim)

`rstim` is a Rust quantum error correction workspace for Stim-like circuit
simulation, command-line circuit workflows, decoder experiments, and benchmark
evidence. Use this README as the map; the detailed workflows live in the linked
docs.

## What You Can Do

- Parse, inspect, sample, and analyze Stim-like stabilizer circuits with the
  `rstim` crate and CLI.
- Generate standard QEC memory circuits and export detector error models for
  decoder workflows.
- Render static SVG circuit diagrams with `render_svg`, or export QP101 JSON
  with `export_json` for downstream tooling.
- Run Rust decoder and benchmark harnesses across `rmatching`, `rbposd`,
  `rilpqec`, and `rsinter`.
- Use showcase, CLI, and benchmark docs as stable starting points for runnable
  examples.

## Workspace Map

| Path | Role |
| --- | --- |
| `rstim/` | Simulator crate and `rstim` CLI for circuit parsing, sampling, DEM extraction, SVG rendering, and QP101 export |
| `rstim/doc/` | Simulator getting-started guide, CLI reference, QP101 notes, and parity documentation |
| `docs/showcases/` | Stable index for runnable workspace showcases |
| `rsinter/` | Parallel collection and benchmark harness for decoder experiments |
| `rmatching/` | Rust MWPM decoder for detector-error-model workflows |
| `rbposd/`, `rilpqec/` | Additional decoder components used by benchmark and comparison flows |
| `qec-code/`, `qec-ilp-core/` | Code construction helpers and ILP-backed checks |
| `benchmarks/surface_decoder_compare/` | Cross-decoder comparison harness and benchmark artifacts |
| `qp101-viz/` | Optional legacy/prototype Typst renderer and committed QP101 fixtures |

## Quick Start

Build the workspace:

```sh
cargo build --workspace
```

Inspect a small circuit with `rstim stats`:

```sh
printf 'H 0\nM 0\nDETECTOR rec[-1]\n' | cargo run -p rstim --bin rstim -- stats
```

Run the Rust test suite:

```sh
cargo test --workspace
```

## Primary Next Steps

- [Showcase index](docs/showcases/README.md): runnable workflow categories and
  the template used for future examples.
- [Getting started with `rstim`](rstim/doc/getting_started.md): simulator and
  Rust API orientation.
- [`rstim` CLI reference](rstim/doc/cli.md): `stats`, `sample`, `detect`,
  `analyze_errors`, `render_svg`, `export_json`, and related commands.
- [`rmatching` decoder docs](rmatching/README.md): MWPM decoder entry point for
  detector-error-model workflows.
- [Surface decoder benchmark docs](benchmarks/surface_decoder_compare/README.md):
  benchmark setup, smoke commands, and generated artifacts.

## CLI And Visualization Notes

The CLI reads from `--in <path>` or stdin and writes to `--out <path>` or
stdout for most commands. For static circuit diagrams, prefer:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

Use `export_json` when you need QP101 structured data for downstream tools,
fixtures, or the optional `qp101-viz` workflow:

```sh
rstim export_json --in circuit.stim --out circuit.json
```

Benchmark smoke runs are documented in
[`benchmarks/surface_decoder_compare/README.md`](benchmarks/surface_decoder_compare/README.md);
the README intentionally leaves algorithm details and benchmark implementation
notes to those dedicated docs.
```

- [ ] **Step 3: Run the focused GREEN check**

Run:

```sh
python3 - <<'PY'
from pathlib import Path
text = Path("README.md").read_text(encoding="utf-8")
assert "docs/showcases/README.md" in text, "README must link to docs/showcases/README.md"
assert "rstim svg_render" not in text, "README must not contain stale rstim svg_render wording"
assert "render_svg" in text, "README must keep render_svg wording"
assert "export_json" in text, "README must keep export_json wording"
PY
```

Expected: exits 0.

- [ ] **Step 4: Run README link validation**

Run:

```sh
python3 tools/check_showcase_docs.py --readme README.md
```

Expected: exits 0 and prints `ok: README.md`.

- [ ] **Step 5: Run the README showcase-link negative control**

Run:

```sh
tmp=$(mktemp)
cp README.md "$tmp"
python3 - <<'PY'
from pathlib import Path
path = Path("README.md")
text = path.read_text(encoding="utf-8")
path.write_text(text.replace("docs/showcases/README.md", "docs/showcases/missing.md", 1), encoding="utf-8")
PY
python3 tools/check_showcase_docs.py --readme README.md
rc=$?
cp "$tmp" README.md
rm "$tmp"
test "$rc" -ne 0
```

Expected: exits 0 overall because the checker fails while the README points to
`docs/showcases/missing.md`, then restores the original README.

- [ ] **Step 6: Run the documented SVG workflow test**

Run:

```sh
cargo test -p rstim render_svg_documented_workflow_matches_cli -q
```

Expected: exits 0.

- [ ] **Step 7: Run Agent Desk requested verification**

Run:

```sh
cargo test
```

Expected: exits 0. If the command fails before compiling because the sandbox
cannot reach the crates.io index, record the exact network failure and run all
available non-network documentation checks.

- [ ] **Step 8: Check diff hygiene**

Run:

```sh
git diff --check
```

Expected: exits 0.

- [ ] **Step 9: Commit or prepare PR branch update**

Preferred local command when git metadata is writable:

```sh
git add README.md docs/superpowers/specs/2026-06-25-issue-213-readme-entry-point-design.md docs/superpowers/plans/2026-06-25-issue-213-readme-entry-point.md
git commit -m "docs: refocus README entry point"
```

If Agent Desk sandbox permissions prevent writing local git metadata, create
the equivalent branch commit through the GitHub connector with the same three
file changes.
