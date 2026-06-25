# Issue 186 Quantum Tanner CLI Workflow Documentation Design

## Context

Issue #186 asks for user-facing documentation for the quantum Tanner path that
now exists in `qec-code`. The current repository already has:

- `qec-code/doc/quantum_tanner.md`, a developer-facing construction contract
  with explicit finite-group input semantics and reference provenance.
- `qec-code code css quantum-tanner --spec <path> hx|hz`, which exports
  quantum Tanner CSS checks as `sparse_rows` JSON.
- `qec-code code css-distance exact --quantum-tanner-spec <path> --json`, which
  computes exact CSS distance directly from the same spec.
- `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`, a committed fixture
  with expected distance `4`.

The user-facing workflow should not teach implementation internals. It should
show a pasteable shell path from fixture inspection to `Hx`/`Hz` export and CSS
distance verification, while repeating the important boundary: Rust consumes an
explicit finite-group spec and does not search groups or call GAP/Oscar.

## Chosen Approach

Create a separate short document at `qec-code/doc/quantum_tanner_cli.md`.
Keeping the CLI workflow separate avoids burying pasteable commands inside the
long construction contract, while allowing the contract doc to remain the
reference for schema and construction semantics.

The document will include:

- a quick "what this command does" description
- the required repo-root assumption
- commands to inspect the committed `toric_d4` fixture
- commands to export `Hx` and `Hz` to `target/qec-code-workflow/*.json`
- exact CSS distance commands, including the direct `--quantum-tanner-spec`
  route that must return distance `4`
- a documented invalid-spec command that should exit non-zero
- the middle-shape boundary and reference/license provenance requested by the
  issue

## Alternatives Considered

1. Add a section to `qec-code/doc/quantum_tanner.md`.
   This keeps all quantum Tanner material in one file, but the existing file is
   already a detailed contract and fixture document. A user trying to run the
   CLI would have to scan too much implementation material.

2. Add only README snippets.
   This would be visible, but it would be too far from the quantum Tanner
   contract and fixtures. It also makes doc-backed command testing less focused.

3. Create `qec-code/doc/quantum_tanner_cli.md`.
   This is the selected approach: one short workflow document, next to the
   contract, with command snippets that a focused integration test can keep
   current.

## Test Strategy

Add `quantum_tanner_cli_doc_commands_stay_current` to
`qec-code/tests/quantum_tanner_cli.rs`.

The test will include the workflow document, extract marked shell command
blocks, and verify every documented qec-code command that uses the committed
`toric_d4` fixture. To avoid invoking Cargo from inside Cargo's integration test
harness, the test will translate the documented prefix:

```text
cargo run -q -p qec-code -- ...
```

into the compiled `CARGO_BIN_EXE_qec-code` binary plus the documented CLI
arguments. Redirects are shell behavior, so the test will capture stdout and
write redirected matrix JSON into a temporary directory before running any
documented file-based distance command.

The test will assert:

- `toric_d4` `hx` and `hz` export commands exit successfully and produce
  `sparse_rows` JSON.
- the documented exact-distance command exits successfully and returns
  `"distance": 4`.
- the documented invalid-spec command exits non-zero and does not produce valid
  `sparse_rows` or distance JSON.

## Scope Boundary

This change is documentation plus a regression test only. It must not add
generation, import, benchmark, or `rsinter` functionality. The doc will state
that the Rust CLI consumes explicit finite-group specs and does not call GAP,
Oscar, qLDPC Python, Julia, group-search code, or external repository code at
runtime.

## Reference And License Notes

The document will include the issue-requested references:

- `drafts/qLDPC/src/qldpc/codes/quantum.py`
- `drafts/qLDPC/src/qldpc/objects.py`
- `drafts/qLDPC/src/qldpc/codes/quantum_test.py`
- <https://github.com/qLDPCOrg/qLDPC>
- <https://github.com/QuantumSavory/QuantumExpanders.jl>
- <https://github.com/RebKatRad/qTanner>

The doc will say that the local qLDPC clone used as a reference is
Apache-2.0, while the other repositories must be used according to their own
licenses and may be reference-only unless compatible licensing is confirmed.

## Self-Review

- No placeholders remain.
- The selected file path is explicit.
- The commands, expected distance, negative control, middle-shape boundary, and
  provenance/license requirements from issue #186 are covered.
- The implementation scope is one workflow doc plus one focused doc-backed test.
