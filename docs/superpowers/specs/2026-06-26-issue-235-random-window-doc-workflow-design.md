# Issue 235 Random-Window Upper-Bound Workflow Design

## Objective

Document the `qec-code code css-distance random-window-upper-bound` workflow so
users know when to use it, how to run it on built-in and file-backed CSS codes,
how to interpret its JSON, and how it compares to the older
`randomized-upper-bound` baseline.

## Context

Issue #233 added the CLI command. Issue #234 added issue-225 ladder evidence
tests and exact smoke/full-ladder commands. The current repository has
developer-facing `qec-code/doc/` notes, but no user-facing CSS distance workflow
document.

## Approach Options

1. Create `qec-code/doc/css_distance.md` and a focused doc-contract test.
   This keeps the CSS distance workflow near the qec-code crate, avoids
   rewriting unrelated docs, and gives the issue's verification command a
   direct target. This is the selected approach.
2. Extend `qec-code/doc/quantum_tanner_cli.md`. This would reuse existing
   doc-command parser patterns, but it would mix general CSS distance guidance
   into a quantum Tanner-specific workflow.
3. Add a README section only. This would be visible from the repository root but
   would not be close to qec-code's existing construction and CLI documents.

## User-Facing Document

Create `qec-code/doc/css_distance.md` with:

- a short "when to use" section for randomized upper bounds;
- one built-in-code `random-window-upper-bound` example;
- one `--hx/--hz` file example using existing sparse-row fixtures;
- a JSON field guide for `status`, `method`, `bound_type`, `upper_bound`,
  `logical_class`, `witness`, `options`, and `provenance`;
- an explicit warning that `bound_type: "upper"` means the result is an upper
  bound and is not a certified exact distance;
- a comparison note that `randomized-upper-bound` remains available as a simple
  baseline and negative-control style comparison;
- the exact issue #234 smoke and full-ladder evidence commands, included as
  provenance and not executed by the doc-contract test.

## Doc-Contract Test

Add `random_window_upper_bound_doc_contract` to `qec-code/tests/cli.rs`.

The test will:

- include `qec-code/doc/css_distance.md` at compile time;
- assert the required phrases are present, including `random-window-upper-bound`,
  `randomized-upper-bound`, `bound_type`, `upper`, and the exact smoke and
  full-ladder evidence commands from #234;
- assert the warning sentence ties `bound_type: "upper"` to the "not certified
  exact" interpretation so the negative control fails if that sentence is
  removed;
- parse documented command blocks identified by HTML markers;
- run only the built-in and file-backed examples from the doc;
- verify each example exits successfully, emits no stderr, and returns completed
  JSON with `method = "random-window-upper-bound"` and `bound_type = "upper"`;
- confirm the documented file-backed example references existing sparse-row
  fixtures.

The test will not run the full issue-225 ladder command.

## Scope Boundaries

Do not rewrite the qec-code documentation set. Do not add benchmark plots or
external baseline tables. Do not claim exact distance certification. Do not
close #225 in this PR.

## Approval

Non-interactive Agent Desk standing policy selects the recommended focused
document plus doc-contract test approach and treats this design as approved.
