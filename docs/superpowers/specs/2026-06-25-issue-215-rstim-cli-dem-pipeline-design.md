# Issue 215 rstim CLI DEM Pipeline Showcase Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #215, a showcase page for the existing rstim CLI and DEM
pipeline

## Summary

Issue #215 adds one user-facing showcase page under `docs/showcases/` that
walks a tiny deterministic circuit through the existing `rstim` CLI:
`stats`, `detect`, `analyze_errors`, and `sample_dem`.

The showcase should use exact observable stdout snippets and links to the
existing CLI documentation and CLI integration tests. Because the issue also
asks for a mechanically enforced negative control, the page will include one
deliberately invalid `stats` input and `rstim/tests/cli_integration.rs` will
gain a focused test that extracts that documented input from the page and
verifies that it still fails cleanly.

## Current State

The dependency issues are satisfied in the current base:

- #211 created `docs/showcases/README.md`, `docs/showcases/_template.md`, and
  `tools/check_showcase_docs.py`.
- #220 added the non-placeholder `Limits` policy to the showcase index,
  template, and checker self-tests.

Relevant current files:

- `docs/showcases/README.md` defines the required page sections.
- `tools/check_showcase_docs.py` validates showcase section structure,
  non-placeholder `Limits`, and repo-relative links.
- `rstim/doc/cli.md` documents the CLI command families.
- `rstim/tests/cli_stats.rs`, `rstim/tests/cli_sample_dem.rs`, and
  `rstim/tests/cli_integration.rs` already cover the commands this showcase
  will cite.

## Goals

- Add `docs/showcases/rstim-cli-dem-pipeline.md`.
- Use a tiny circuit that has one qubit, one measurement, one detector, and
  one observable.
- Show exact stdout snippets for:
  - `stats`
  - `detect --out_format dets`
  - `analyze_errors`
  - `sample_dem --out_format dets`
- Link from the page to:
  - `rstim/doc/cli.md`
  - `rstim/tests/cli_stats.rs`
  - `rstim/tests/cli_sample_dem.rs`
  - `rstim/tests/cli_integration.rs`
- Include a concrete `Limits` section with real scope boundaries.
- Include one documented bad-input example only because a focused Rust test
  will enforce that the documented input still fails.
- Keep all claims to current, high-confidence behavior.

## Non-Goals

- Do not add simulator features.
- Do not claim full Stim parity.
- Do not add a broad command-output parser to `tools/check_showcase_docs.py`.
- Do not require the showcase checker to execute shell commands.
- Do not reorganize the CLI docs or existing tests beyond the focused
  doc/CLI contract test.

## Approaches Considered

### 1. Showcase page plus focused doc/CLI contract test

Create the Markdown page and add a small `cli_integration` test that reads the
documented invalid circuit block between stable HTML comments, feeds it to
`rstim stats`, and asserts a clean nonzero exit with a parser error.

Benefits:

- directly satisfies the issue objective
- keeps the checker focused on Markdown structure and links
- makes the bad-input example mechanically enforced
- keeps the behavioral test near the existing CLI pipeline coverage

Cost:

- the page needs stable marker comments around the invalid input block so the
  test can find the exact documented fixture

This is the chosen approach.

### 2. Extend the showcase checker to understand CLI commands

Teach `tools/check_showcase_docs.py` to parse the page, discover command
blocks, run them, and compare stdout snippets.

Benefits:

- puts more validation behind one documentation command

Costs:

- expands a lightweight Markdown checker into a shell-command test harness
- creates cross-platform quoting and temp-file concerns
- exceeds the current checker contract from #211 and #220

This is rejected.

### 3. Documentation only with no new contract test

Add the page and rely on the existing CLI tests for the command families.

Benefits:

- smallest diff

Costs:

- does not satisfy the negative-control requirement for the documented
  bad-input example
- would allow the page to drift from the actual failure behavior

This is rejected.

## Documentation Design

`docs/showcases/rstim-cli-dem-pipeline.md` should follow the showcase page
contract exactly:

- `What This Shows`: name the four-command pipeline and state that the example
  uses deterministic probabilities.
- `Run It`: give repository-root commands that create a temporary `.stim`
  file, run the CLI commands, and write/read a `.dem` file.
- `Expected Result`: show exact stdout snippets and the documented failure
  stderr snippet.
- `Code`: link to the CLI docs and the three requested test files.
- `Verification`: list the issue-required checker and Cargo test commands.
- `Limits`: state that the example is tiny, deterministic, and not a parity or
  performance claim.

The main circuit should be:

```stim
R 0
X_ERROR(1) 0
M 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
```

The exact observed outputs for this circuit are:

```text
instruction_count: 5
repeat_blocks: 0
max_repeat_depth: 0
num_qubits: 1
num_measurements: 1
num_detectors: 1
num_observables: 1
num_ticks: 0
num_sweep_bits: 0
```

```text
shot D0 L0
```

```text
error(1) D0 L0
```

```text
shot D0 L0
```

The documented invalid input should remain:

```stim
REPEAT two {
  M 0
}
```

and the expected stderr snippet should include `Error: line 1: bad repeat
count`.

## Test Design

Add one test to `rstim/tests/cli_integration.rs`:

- read `../docs/showcases/rstim-cli-dem-pipeline.md`
- extract the fenced block between
  `<!-- rstim-cli-dem-pipeline-bad-input-start -->` and
  `<!-- rstim-cli-dem-pipeline-bad-input-end -->`
- run `rstim stats` with that block on stdin
- assert the command fails
- assert stderr contains `bad repeat count`
- assert stderr does not contain `panicked`

If the documented invalid input is replaced with a valid circuit, this test
will fail because `rstim stats` will exit successfully.

## Verification

Required commands:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-cli-dem-pipeline.md
cargo test -p rstim --test cli_stats --test cli_sample_dem --test cli_integration -q
```

Repository workflow command:

```sh
cargo test
```

Useful focused command while developing:

```sh
cargo test -p rstim --test cli_integration showcase_documented_bad_stats_input_still_fails -q
```
