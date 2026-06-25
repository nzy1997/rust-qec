# Issue 218 QEC-Code CSS Construction Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a user-facing showcase page for existing `qec-code` CSS construction commands, exports, fixtures, and verification tests.

**Architecture:** Keep the implementation documentation-only. The new showcase page follows the existing page contract, links to stable `qec-code` docs/tests/fixtures, and the showcase index points users to the new page from the code-construction category.

**Tech Stack:** Markdown, existing Python showcase checker, existing Rust/Cargo `qec-code` tests.

## Global Constraints

- Create `docs/showcases/qec-code-css-construction.md`.
- Update `docs/showcases/README.md` so the new page is discoverable from `Code Construction Workflows`.
- Demonstrate only existing `qec-code` CLI commands and fixtures.
- Use stable examples: `steane`, `bb72`, and `apm_kasai:p=96`.
- Link to `qec-code/doc/apm_css.md`, `qec-code/doc/quantum_tanner.md`, `qec-code/tests/cli.rs`, and relevant fixtures.
- Keep at least one verification path tied to `qec-code/tests/cli.rs`.
- Do not duplicate the quantum Tanner CLI workflow or explain uncertain quantum Tanner algorithm details.
- Do not make new scientific distance claims.
- Required verification commands:
  - `python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md`
  - `cargo test -p qec-code --test cli -q`
  - `cargo test -p qec-code apm_contract_doc_examples_compile -q`
  - `cargo test -p qec-code apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values -q`
  - `cargo test`

---

### Task 1: Add QEC-Code CSS Construction Showcase

**Files:**
- Create: `docs/showcases/qec-code-css-construction.md`
- Modify: `docs/showcases/README.md`

**Interfaces:**
- Consumes: `tools/check_showcase_docs.py` individual-page validation.
- Consumes: existing `qec-code` CLI commands covered by `qec-code/tests/cli.rs`.
- Produces: one valid showcase page with the required sections.
- Produces: one index link under the existing code-construction category.

- [ ] **Step 1: Run the failing documentation check first**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
```

Expected: non-zero exit because the showcase page does not exist yet. This is
the RED step for the documentation validator.

- [ ] **Step 2: Add the showcase page**

Create `docs/showcases/qec-code-css-construction.md` with exactly this
structure and content:

````markdown
# QEC-Code CSS Construction

Use `qec-code` to inspect the built-in CSS catalog, export parity-check
matrices as sparse-row JSON, and run a small exact-distance check against the
Steane fixture.

## What This Shows

This showcase follows the stable CSS construction paths that are already
covered by `qec-code` tests. It demonstrates the CLI-facing workflow for:

- listing built-in CSS code identifiers
- exporting `Hx` and `Hz` matrices for fixed built-ins and APM/Kasai presets
- checking a small exact distance through the same CLI family

The examples use `steane`, `bb72`, and `apm_kasai:p=96` because those fixtures
are pinned in the repository today.

## Run It

Run these commands from the repository root:

```sh
cargo run -q -p qec-code -- code css list
cargo run -q -p qec-code -- code css export steane hx
cargo run -q -p qec-code -- code css export steane hz
cargo run -q -p qec-code -- code css export bb72 hx
cargo run -q -p qec-code -- code css export bb72 hz
cargo run -q -p qec-code -- code css export apm_kasai:p=96 hx > /tmp/apm_p96_hx.json
cargo run -q -p qec-code -- code css export apm_kasai:p=96 hz > /tmp/apm_p96_hz.json
cargo run -q -p qec-code -- code css-distance exact --code-id steane --json
```

## Expected Result

The list command prints `Built-in CSS codes:` and includes the stable entries
`steane`, `bb72`, `apm_kasai:p=96`, and `apm_kasai:p=192`.

Each export command prints a JSON object with `"format":"sparse_rows"`.
The Steane exports use `num_cols` 7, the `bb72` exports use `num_cols` 72, and
the `apm_kasai:p=96` exports use `num_cols` 1152. The APM/Kasai `p=96`
exports are redirected to `/tmp` above because their sparse-row output is much
larger than the Steane and `bb72` examples.

The exact-distance command returns JSON with `"status":"completed"` and
`"distance":3` for the Steane code.

## Code

Primary implementation and CLI coverage:

- [`qec-code/src/cli.rs`](qec-code/src/cli.rs)
- [`qec-code/src/codes/built_in_css.rs`](qec-code/src/codes/built_in_css.rs)
- [`qec-code/tests/cli.rs`](qec-code/tests/cli.rs)
- [`qec-code/tests/code.rs`](qec-code/tests/code.rs)

Construction notes and contracts:

- [`qec-code/doc/apm_css.md`](qec-code/doc/apm_css.md)
- [`qec-code/doc/quantum_tanner.md`](qec-code/doc/quantum_tanner.md)

Fixtures used by the documented examples and focused tests:

- [`qec-code/tests/fixtures/css/steane_hx.json`](qec-code/tests/fixtures/css/steane_hx.json)
- [`qec-code/tests/fixtures/css/steane_hz.json`](qec-code/tests/fixtures/css/steane_hz.json)
- [`qec-code/tests/fixtures/css/bb72_hx.json`](qec-code/tests/fixtures/css/bb72_hx.json)
- [`qec-code/tests/fixtures/css/bb72_hz.json`](qec-code/tests/fixtures/css/bb72_hz.json)
- [`qec-code/tests/fixtures/apm/table_a1_manifest.json`](qec-code/tests/fixtures/apm/table_a1_manifest.json)
- [`qec-code/tests/fixtures/apm/p96_hx.json`](qec-code/tests/fixtures/apm/p96_hx.json)
- [`qec-code/tests/fixtures/apm/p96_hz.json`](qec-code/tests/fixtures/apm/p96_hz.json)

## Verification

Run the showcase checker:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
```

Run the CLI coverage tied to this page:

```sh
cargo test -p qec-code --test cli -q
```

That integration test covers the documented list/export/distance-facing command
family, including `code css list`, `steane` and `bb72` sparse-row exports,
`apm_kasai:p=96` exports, Steane exact-distance JSON, and the unsupported
`apm_kasai:p=128` rejection path.

Run the focused APM contract checks:

```sh
cargo test -p qec-code apm_contract_doc_examples_compile -q
cargo test -p qec-code apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values -q
```

Those tests keep the APM construction contract examples compiling, verify the
`apm_kasai:p=96` sparse-row shape, and keep the negative control for
`apm_kasai:p=128` in place.

## Limits

This page documents existing CLI behavior and pinned fixtures only. It does not
claim new distances for `bb72` or APM/Kasai codes, and it treats APM Table A1
distance values as fixture metadata rather than as newly verified exact
minimum-distance results.

Quantum Tanner construction details are intentionally linked through the
existing contract document instead of being explained here. Use
[`qec-code/doc/quantum_tanner.md`](qec-code/doc/quantum_tanner.md) for the
current explicit-data contract and open a follow-up issue before adding new
algorithm claims to this showcase.
````

- [ ] **Step 3: Add the showcase to the index**

In `docs/showcases/README.md`, under `### Code Construction Workflows` and
before `Primary code and docs:`, add:

```markdown
Available showcases:

- [`qec-code CSS construction`](docs/showcases/qec-code-css-construction.md)
```

- [ ] **Step 4: Run focused documentation checks**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
python3 tools/check_showcase_docs.py docs/showcases/README.md
python3 tools/check_showcase_docs.py docs/showcases
```

Expected: all commands exit `0`. The directory command prints `ok:` lines for
`README.md`, `_template.md`, and `qec-code-css-construction.md`.

- [ ] **Step 5: Run required qec-code verification**

Run:

```sh
cargo test -p qec-code --test cli -q
cargo test -p qec-code apm_contract_doc_examples_compile -q
cargo test -p qec-code apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values -q
```

Expected: all commands exit `0`.

- [ ] **Step 6: Run final repository checks**

Run:

```sh
cargo test
git diff --check
```

Expected: both commands exit `0`. If `cargo test` cannot reach crates.io in the
Agent Desk sandbox, record the exact external dependency error and run the
issue-scoped qec-code commands with cached dependencies.

- [ ] **Step 7: Commit the implementation**

Run:

```sh
git add docs/showcases/README.md docs/showcases/qec-code-css-construction.md docs/superpowers/plans/2026-06-25-issue-218-qec-code-css-construction-showcase.md
git commit -m "docs: add qec-code css construction showcase"
```

Expected: commit succeeds with only the new showcase, the index link, and this
implementation plan.
