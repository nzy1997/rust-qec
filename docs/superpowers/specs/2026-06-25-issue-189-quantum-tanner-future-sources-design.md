# Issue 189 Quantum Tanner Future Sources Roadmap Design

## Context

Issue #189 asks for a reviewer-friendly roadmap for future quantum Tanner
importers and searchers. The current `qec-code` implementation intentionally
stays in the middle: it consumes explicit finite-group specs and local GF(2)
code matrices, then emits deterministic CSS sparse-row matrices. It must not
quietly grow into group search, GAP/Oscar orchestration, qLDPC import, or broad
external repository format support.

Sibling issue #186 already added `qec-code/doc/quantum_tanner_cli.md`, which
documents the current CLI workflow and the reference/license caveats for the
initial implementation. This issue should extend that documentation boundary by
recording future-source posture in one place: what each source is useful for,
what license review is needed, and what a future issue must prove before
turning the source into implementation work.

## Chosen Approach

Create a separate roadmap document at
`qec-code/doc/quantum_tanner_sources.md`.

The document will include:

- an explicit boundary that these rows are future adapters/searchers, not part
  of the initial constructor
- a reference table with one row per required source
- columns for source name, URL or local path, license status, intended use,
  copying/import posture, and future definition of done
- guidance that test data may be derived only where the license/posture allows
  it, while reference-only sources remain blocked on license review before code
  reuse

Keeping the roadmap separate from `qec-code/doc/quantum_tanner.md` avoids
mixing future importer/searcher triage into the construction contract. It also
lets a focused regression test parse the required table without scanning a long
contract document.

## Alternatives Considered

1. Add a section to `qec-code/doc/quantum_tanner.md`.
   This keeps all quantum Tanner docs in one file, but the contract document is
   already responsible for input semantics and construction examples. Adding
   future repository triage there would make it harder to tell what is current
   behavior versus future work.

2. Extend `qec-code/doc/quantum_tanner_cli.md`.
   The CLI workflow already mentions references and license posture, but it is
   oriented around runnable commands. A detailed future-source table would make
   the workflow less concise and less user-facing.

3. Create `qec-code/doc/quantum_tanner_sources.md`.
   This is the selected approach. It creates a single roadmap surface for future
   issues and keeps the current constructor and CLI docs focused.

## Test Strategy

Add `quantum_tanner_future_sources_doc_has_reference_table` to
`qec-code/tests/code.rs`.

The test will include `qec-code/doc/quantum_tanner_sources.md`, parse the first
Markdown table that starts with the exact required header, and require every
issue-mandated row to provide:

- source name
- URL or local path
- license status
- intended use
- copying/import posture
- definition of done

The test will also check source-specific posture so the table protects future
implementation work:

- qLDPC local clone must mention `drafts/qLDPC`, both key local files, upstream
  qLDPC, Apache-2.0, and copied-code restrictions.
- QuantumExpanders.jl and qTanner must be reference/source-grounded only until
  license compatibility is confirmed.
- Giacomo-Fregona/QTC must be blocked on license confirmation before code reuse.
- quantum-tanner-sogrand must note the non-commercial academic license and must
  not be suitable for code copying into this repo.
- QUITS must mention `drafts/quits`, upstream QUITS, and matrix-consumption
  inspiration rather than quantum Tanner construction.

The issue's negative control is represented by the parser requiring nonempty
license status and copying/import posture cells for every required row. Removing
either cell from any row makes the test fail.

## Scope Boundary

This change is documentation plus a doc-backed regression test only. It must not
implement importers, call external tools, add fixtures, modify the constructor,
or run benchmark campaigns.

## Self-Review

- No placeholders remain.
- The selected file path and test name are explicit.
- Every required source from issue #189 is covered.
- The roadmap table columns match the issue's input-output contract.
- The test strategy includes the required negative-control behavior.
- Scope excludes constructor, importer, fixture, external-tool, and benchmark
  changes.
