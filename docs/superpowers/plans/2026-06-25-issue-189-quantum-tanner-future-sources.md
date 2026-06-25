# Quantum Tanner Future Sources Roadmap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a future-source roadmap for quantum Tanner import/search candidates and guard it with a doc-backed regression test.

**Architecture:** Put the roadmap in `qec-code/doc/quantum_tanner_sources.md` as a Markdown table with stable columns. Add a Rust integration test in `qec-code/tests/code.rs` that parses that table and verifies every required source has location, license, intended use, copying/import posture, and definition-of-done coverage.

**Tech Stack:** Rust 2024 integration tests, Markdown documentation, existing `qec-code` test helpers.

## Global Constraints

- Roadmap document path is exactly `qec-code/doc/quantum_tanner_sources.md`.
- Regression test name is exactly `quantum_tanner_future_sources_doc_has_reference_table`.
- Verification command from issue #189 is `cargo test -p qec-code quantum_tanner_future_sources_doc_has_reference_table -q`.
- The roadmap table must include source name, URL/local path, license status, implementation use, allowed copying/import posture, and definition of done for future importer/searcher work.
- Include qLDPC local clone with `drafts/qLDPC`, `drafts/qLDPC/src/qldpc/codes/quantum.py`, `drafts/qLDPC/src/qldpc/objects.py`, upstream `https://github.com/qLDPCOrg/qLDPC`, and Apache-2.0 in the local clone.
- Include QuantumExpanders.jl at `https://github.com/QuantumSavory/QuantumExpanders.jl` as mathematical/reference implementation unless license compatibility is confirmed.
- Include qTanner at `https://github.com/RebKatRad/qTanner` as source-grounded data/reference unless license compatibility is confirmed.
- Include Giacomo-Fregona/QTC at `https://github.com/Giacomo-Fregona/QTC` as a small Python reference whose license status must be confirmed before code reuse.
- Include quantum-tanner-sogrand at `https://github.com/grand-decoder/quantum-tanner-sogrand` as decoder-focused, with non-commercial academic license noted in README, likely not suitable for code copying into this repo.
- Include QUITS with `drafts/quits`, upstream `https://github.com/mkangquantum/quits`, and downstream matrix-consumption inspiration rather than a quantum Tanner constructor.
- Make clear that these are future adapters/searchers, not part of the initial constructor.
- Do not implement importers, call external tools, add new fixtures, modify the constructor, or run benchmark campaigns.

---

## File Structure

- Create `qec-code/doc/quantum_tanner_sources.md`: future-source roadmap table and boundary notes.
- Modify `qec-code/tests/code.rs`: add table parsing helpers and the focused roadmap regression test.

---

### Task 1: Future-Source Roadmap Doc Gate

**Files:**
- Create: `qec-code/doc/quantum_tanner_sources.md`
- Modify: `qec-code/tests/code.rs`

**Interfaces:**
- Consumes: `include_str!("../doc/quantum_tanner_sources.md")`.
- Produces: Markdown table headed by `| Source | URL or local path | License status | Intended use | Copying/import posture | Definition of done for future work |`.
- Produces: test `quantum_tanner_future_sources_doc_has_reference_table`.

- [ ] **Step 1: Write the failing doc-backed test**

Add this code near the existing quantum Tanner doc and fixture tests in
`qec-code/tests/code.rs`, before `quantum_tanner_contract_examples_compile`:

```rust
const QUANTUM_TANNER_SOURCES_DOC: &str = include_str!("../doc/quantum_tanner_sources.md");

#[derive(Debug)]
struct QuantumTannerSourceRow<'a> {
    source: &'a str,
    location: &'a str,
    license: &'a str,
    intended_use: &'a str,
    copying_posture: &'a str,
    definition_of_done: &'a str,
}

const QUANTUM_TANNER_SOURCE_TABLE_HEADER: &str =
    "| Source | URL or local path | License status | Intended use | Copying/import posture | Definition of done for future work |";

fn markdown_cells(row: &str) -> Vec<&str> {
    row.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

fn quantum_tanner_source_rows(doc: &str) -> Vec<QuantumTannerSourceRow<'_>> {
    let mut lines = doc.lines();
    while let Some(line) = lines.next() {
        if line.trim() == QUANTUM_TANNER_SOURCE_TABLE_HEADER {
            let separator = lines
                .next()
                .expect("source table should include a separator row");
            assert!(
                separator
                    .trim()
                    .starts_with("| --- | --- | --- | --- | --- | --- |"),
                "source table separator has unexpected shape: {separator}"
            );

            return lines
                .take_while(|line| line.trim_start().starts_with('|'))
                .map(|line| {
                    let cells = markdown_cells(line);
                    assert_eq!(cells.len(), 6, "source table row has unexpected shape: {line}");
                    QuantumTannerSourceRow {
                        source: cells[0],
                        location: cells[1],
                        license: cells[2],
                        intended_use: cells[3],
                        copying_posture: cells[4],
                        definition_of_done: cells[5],
                    }
                })
                .collect();
        }
    }
    panic!("missing quantum Tanner source roadmap table");
}

fn expect_quantum_tanner_source_row<'a>(
    rows: &'a [QuantumTannerSourceRow<'a>],
    source: &str,
) -> &'a QuantumTannerSourceRow<'a> {
    rows.iter()
        .find(|row| row.source == source)
        .unwrap_or_else(|| panic!("missing roadmap row for {source}"))
}

fn assert_source_row_complete(row: &QuantumTannerSourceRow<'_>) {
    for (column, value) in [
        ("URL or local path", row.location),
        ("License status", row.license),
        ("Intended use", row.intended_use),
        ("Copying/import posture", row.copying_posture),
        ("Definition of done", row.definition_of_done),
    ] {
        assert!(
            !value.trim().is_empty() && value != "-",
            "{} row must have a nonempty {column}",
            row.source
        );
    }
}

fn assert_cell_contains(row: &QuantumTannerSourceRow<'_>, column: &str, value: &str) {
    let cell = match column {
        "location" => row.location,
        "license" => row.license,
        "intended_use" => row.intended_use,
        "copying_posture" => row.copying_posture,
        "definition_of_done" => row.definition_of_done,
        _ => panic!("unknown roadmap column {column}"),
    };
    assert!(
        cell.contains(value),
        "{} {column} should contain {value:?}, got {cell:?}",
        row.source
    );
}

#[test]
fn quantum_tanner_future_sources_doc_has_reference_table() {
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("future adapters/searchers"));
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("not part of the initial constructor"));
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("does not search for good groups"));
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("does not call GAP or Oscar"));

    let rows = quantum_tanner_source_rows(QUANTUM_TANNER_SOURCES_DOC);
    assert_eq!(rows.len(), 6);
    for row in &rows {
        assert_source_row_complete(row);
    }

    let qldpc = expect_quantum_tanner_source_row(&rows, "qLDPC local clone");
    assert_cell_contains(qldpc, "location", "drafts/qLDPC");
    assert_cell_contains(qldpc, "location", "drafts/qLDPC/src/qldpc/codes/quantum.py");
    assert_cell_contains(qldpc, "location", "drafts/qLDPC/src/qldpc/objects.py");
    assert_cell_contains(qldpc, "location", "https://github.com/qLDPCOrg/qLDPC");
    assert_cell_contains(qldpc, "license", "Apache-2.0");
    assert_cell_contains(qldpc, "copying_posture", "cite");

    let quantum_expanders =
        expect_quantum_tanner_source_row(&rows, "QuantumExpanders.jl");
    assert_cell_contains(
        quantum_expanders,
        "location",
        "https://github.com/QuantumSavory/QuantumExpanders.jl",
    );
    assert_cell_contains(quantum_expanders, "intended_use", "mathematical/reference");
    assert_cell_contains(quantum_expanders, "copying_posture", "unless license compatibility is confirmed");

    let qtanner = expect_quantum_tanner_source_row(&rows, "qTanner");
    assert_cell_contains(qtanner, "location", "https://github.com/RebKatRad/qTanner");
    assert_cell_contains(qtanner, "intended_use", "source-grounded data/reference");
    assert_cell_contains(qtanner, "copying_posture", "unless license compatibility is confirmed");

    let qtc = expect_quantum_tanner_source_row(&rows, "Giacomo-Fregona/QTC");
    assert_cell_contains(qtc, "location", "https://github.com/Giacomo-Fregona/QTC");
    assert_cell_contains(qtc, "license", "confirm");
    assert_cell_contains(qtc, "copying_posture", "No code reuse before license review");

    let sogrand = expect_quantum_tanner_source_row(&rows, "quantum-tanner-sogrand");
    assert_cell_contains(
        sogrand,
        "location",
        "https://github.com/grand-decoder/quantum-tanner-sogrand",
    );
    assert_cell_contains(sogrand, "license", "non-commercial academic");
    assert_cell_contains(sogrand, "copying_posture", "not suitable for code copying");

    let quits = expect_quantum_tanner_source_row(&rows, "QUITS");
    assert_cell_contains(quits, "location", "drafts/quits");
    assert_cell_contains(quits, "location", "https://github.com/mkangquantum/quits");
    assert_cell_contains(quits, "intended_use", "matrix-consumption inspiration");
    assert_cell_contains(quits, "copying_posture", "not a quantum Tanner constructor");
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_future_sources_doc_has_reference_table -q
```

Expected: FAIL at compile time because `qec-code/doc/quantum_tanner_sources.md` does not exist yet. That proves the regression test is wired to the roadmap document.

- [ ] **Step 3: Add the roadmap document**

Create `qec-code/doc/quantum_tanner_sources.md` with this exact content:

```markdown
# Quantum Tanner Future Sources

This roadmap records candidate sources for future quantum Tanner importers,
adapters, and searchers. These entries are future adapters/searchers, not part
of the initial constructor.

The current `qec-code` boundary remains unchanged: Rust consumes explicit
finite-group specs and local GF(2) code matrices. It does not search for good
groups, does not call GAP or Oscar, and does not run qLDPC, Julia/Oscar, or
other external repository code at runtime.

Use this table when filing future implementation issues. A source can inform
tests or design only according to its license status and copying/import posture;
license-blocked rows stay reference-only until review clears a narrower use.

| Source | URL or local path | License status | Intended use | Copying/import posture | Definition of done for future work |
| --- | --- | --- | --- | --- | --- |
| qLDPC local clone | Local `drafts/qLDPC`, especially `drafts/qLDPC/src/qldpc/codes/quantum.py` and `drafts/qLDPC/src/qldpc/objects.py`; upstream https://github.com/qLDPCOrg/qLDPC | Apache-2.0 in the local clone | Grounded reference for qLDPC quantum Tanner vocabulary, Cayley-complex semantics, and known-answer test expectations | Compatible license permits careful reference use, but future work should cite provenance and avoid direct code copying unless a reviewed port plan names the copied/translated pieces | Future importer issue identifies exact qLDPC input objects to accept, maps them into the explicit `qec-code` spec schema, adds license/provenance notes, and includes positive and negative fixtures generated without runtime qLDPC dependency |
| QuantumExpanders.jl | https://github.com/QuantumSavory/QuantumExpanders.jl | License compatibility to confirm before implementation reuse | Mathematical/reference implementation for quantum Tanner and expander-code vocabulary | Reference-only; do not copy code or import formats unless license compatibility is confirmed | Future searcher issue documents the construction family being matched, confirms license posture, and adds independently generated explicit-group specs consumed by `qec-code` without Julia/Oscar runtime calls |
| qTanner | https://github.com/RebKatRad/qTanner | License compatibility to confirm before implementation reuse | Source-grounded data/reference for quantum Tanner examples and future fixture ideas | Reference-only; use as source-grounded data/reference unless license compatibility is confirmed | Future adapter issue names the accepted qTanner artifacts, confirms license terms for any copied data, and adds schema validation plus fixtures that still enter `qec-code` as explicit finite data |
| Giacomo-Fregona/QTC | https://github.com/Giacomo-Fregona/QTC | License status to confirm before code reuse | Small Python reference for comparing construction vocabulary and example shapes | No code reuse before license review; cite only high-level behavior until terms are confirmed | Future issue records the license decision, identifies any reusable examples, and adds an importer/searcher only after the accepted input contract and provenance are explicit |
| quantum-tanner-sogrand | https://github.com/grand-decoder/quantum-tanner-sogrand | README notes a non-commercial academic license | Decoder-focused reference for downstream experiments around quantum Tanner matrices | Likely not suitable for code copying into this repo; do not vendor decoder code or derive implementation from it without legal review | Future issue is limited to interoperability notes or externally generated matrix-consumption checks unless license review explicitly permits a narrower artifact use |
| QUITS | Local `drafts/quits`; upstream https://github.com/mkangquantum/quits | License status to confirm before code or data reuse | Downstream matrix-consumption inspiration, not a quantum Tanner constructor | Treat as reference-only for consumer workflow ideas; do not import as a constructor or copy code without license review | Future issue describes how exported `sparse_rows` matrices would be consumed downstream, with no changes to the quantum Tanner constructor and no runtime QUITS dependency |

## Future-Issue Checklist

Before any row becomes importer or searcher implementation work, the issue must
state:

- the exact external artifact or construction family being adapted
- the license status and allowed copying/import posture
- whether source material may be used for tests, reference-only comparison, or
  neither
- how the adapter emits the existing explicit finite-group/local-code spec
  consumed by `qec-code`
- positive and negative verification commands that do not call external tools at
  runtime
```

- [ ] **Step 4: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code quantum_tanner_future_sources_doc_has_reference_table -q
```

Expected: PASS with one matching test.

- [ ] **Step 5: Run broader qec-code tests**

Run:

```bash
cargo test -p qec-code -q
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-06-25-issue-189-quantum-tanner-future-sources-design.md docs/superpowers/plans/2026-06-25-issue-189-quantum-tanner-future-sources.md qec-code/doc/quantum_tanner_sources.md qec-code/tests/code.rs
git commit -m "docs: add quantum Tanner future source roadmap"
```

## Self-Review

- Spec coverage: the plan creates the requested roadmap, table columns, required rows, license posture, intended use, copying/import posture, definition of done, and regression test.
- Placeholder scan: no placeholder text remains.
- Type consistency: helper names and test name are consistent across the plan.
