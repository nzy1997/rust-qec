# Issue 218 QEC-Code CSS Construction Showcase Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #218, a showcase page for existing `qec-code` CSS construction CLI workflows

## Summary

Issue #218 adds one individual showcase page under `docs/showcases/` for
stable `qec-code` CSS construction workflows. The page should show how a user
can list built-in CSS codes, export `Hx` and `Hz` sparse-row JSON for known
fixtures, and run a small Steane exact-distance check without introducing new
scientific claims.

The change should be documentation-only:

- create `docs/showcases/qec-code-css-construction.md`
- add the page to the code-construction category in
  `docs/showcases/README.md`
- link to the existing APM and quantum Tanner documentation, CLI coverage, and
  sparse-row fixtures

## Current State

The dependencies are satisfied:

- issue #211 is closed and provided the showcase index, page template, and
  checker
- issue #220 is closed and provided the follow-up policy for uncertain claims

Relevant existing files:

- `docs/showcases/README.md` defines the showcase categories and page contract.
- `docs/showcases/_template.md` defines the required sections:
  `What This Shows`, `Run It`, `Expected Result`, `Code`, `Verification`, and
  `Limits`.
- `tools/check_showcase_docs.py` validates individual pages and repo-relative
  links.
- `qec-code/tests/cli.rs` covers `code css list`, built-in sparse-row exports,
  APM/Kasai `p=96` and `p=192` export shape, `apm_kasai:p=128` rejection, and
  Steane exact-distance CLI behavior.
- `qec-code/doc/apm_css.md` documents the APM-CSS construction contract.
- `qec-code/doc/quantum_tanner.md` documents the quantum Tanner construction
  contract without requiring this showcase to restate the algorithm.

There are no comments on issue #218 and no existing pull request for it.

## Goals

- Add `docs/showcases/qec-code-css-construction.md`.
- Keep the page runnable from the repository root.
- Demonstrate `cargo run -q -p qec-code -- code css list`.
- Demonstrate `Hx`/`Hz` exports for stable examples such as `steane`, `bb72`,
  and `apm_kasai:p=96`.
- Demonstrate a small distance-facing command using Steane exact distance.
- Link to:
  - `qec-code/doc/apm_css.md`
  - `qec-code/doc/quantum_tanner.md`
  - `qec-code/tests/cli.rs`
  - relevant built-in CSS and APM fixtures
- State the expected output shape without over-claiming new code distances.
- Document that the `apm_kasai:p=128` negative control is covered by the
  focused CLI tests.
- Add an index entry so the new showcase is discoverable.

## Non-Goals

- Do not change `qec-code` behavior.
- Do not add new scientific distance claims.
- Do not duplicate the quantum Tanner CLI workflow documentation from issue
  #186.
- Do not explain the quantum Tanner algorithm in this showcase.
- Do not add new fixture files, benchmark output, or generated sparse matrices.
- Do not file follow-up issues unless a concrete uncertain claim is discovered.

## Approaches Considered

### 1. Concise showcase page over existing CLI tests and fixtures

Write one page that uses only already-supported commands and known fixtures.
Use Steane for the small exact-distance command, use `bb72` and
`apm_kasai:p=96` for sparse-row export examples, and route algorithm details to
the existing APM and quantum Tanner docs.

Benefits:

- matches the issue objective directly
- keeps verification tied to `qec-code/tests/cli.rs`
- avoids new behavior and new scientific claims
- keeps the page stable under the showcase checker

This is the chosen approach.

### 2. Add doc-backed CLI tests for every command in the page

Parse command blocks from the new page and execute them in an integration test.

Benefits:

- strong command drift protection

Costs:

- exceeds the issue request for a documentation page
- duplicates existing CLI test coverage
- adds maintenance complexity to a docs-only change

This is rejected.

### 3. Expand the showcase into algorithm background

Include explanatory sections for APM-CSS and quantum Tanner construction
internals.

Benefits:

- more standalone educational content

Costs:

- conflicts with the instruction to avoid uncertain quantum Tanner details
- risks duplicating issue #186 and the existing construction contracts
- increases chance of unreviewed scientific claims

This is rejected.

## Documentation Design

`docs/showcases/qec-code-css-construction.md` should follow the showcase
template exactly.

`What This Shows` should describe the CLI workflow and name the stable example
families without claiming new performance or distance results.

`Run It` should include commands that work from the repository root:

- list built-in CSS codes
- export `steane` `hx`
- export `bb72` `hz`
- export `apm_kasai:p=96` `hx`
- run exact Steane distance with JSON output

`Expected Result` should describe stable observable output:

- the list includes `steane`, `bb72`, and `apm_kasai:p=96`
- export commands print `sparse_rows` JSON
- Steane exports have `num_cols` 7
- `bb72` exports have `num_cols` 72
- `apm_kasai:p=96` exports have `num_cols` 1152
- Steane exact-distance JSON reports `distance` 3

`Code` should link to owning docs, tests, and fixtures.

`Verification` should list the issue's required commands and explain what each
one proves. It should explicitly mention that `qec-code/tests/cli.rs` covers the
CLI-facing list/export/distance paths and the `apm_kasai:p=128` rejection
control.

`Limits` should state that the page documents existing CLI behavior only, that
APM table metadata such as distance is treated as fixture metadata rather than
a new exact-distance claim, and that quantum Tanner details are intentionally
linked rather than explained.

`docs/showcases/README.md` should add one bullet under `Code Construction
Workflows` linking to the new page.

## Testing Design

Use a documentation TDD loop:

1. Run the showcase checker against the missing page and observe failure.
2. Add the page and index link.
3. Run the required checker and `qec-code` tests.

Required verification commands:

```sh
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
cargo test -p qec-code --test cli -q
cargo test -p qec-code apm_contract_doc_examples_compile -q
cargo test -p qec-code apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values -q
cargo test
```

Additional useful checks:

```sh
python3 tools/check_showcase_docs.py docs/showcases
git diff --check
```
