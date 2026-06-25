# Quantum Tanner Future Sources

This roadmap records candidate sources for future quantum Tanner importers,
adapters, and searchers. These entries are future adapters/searchers, not part of the initial constructor.

The current `qec-code` boundary remains unchanged: Rust consumes explicit
finite-group specs and local GF(2) code matrices. It does not search for good groups,
does not call GAP or Oscar, and does not run qLDPC, Julia/Oscar, or other external
repository code at runtime.

Use this table when filing future implementation issues. A source can inform
tests or design only according to its license status and copying/import posture;
license-blocked rows stay reference-only until review clears a narrower use.

| Source | URL or local path | License status | Intended use | Copying/import posture | Definition of done for future work |
| --- | --- | --- | --- | --- | --- |
| qLDPC local clone | Local `drafts/qLDPC`, especially `drafts/qLDPC/src/qldpc/codes/quantum.py` and `drafts/qLDPC/src/qldpc/objects.py`; upstream https://github.com/qLDPCOrg/qLDPC | Apache-2.0 in the local clone | Grounded reference for qLDPC quantum Tanner vocabulary, Cayley-complex semantics, and known-answer test expectations | Compatible license permits careful reference use, but future work should cite provenance and avoid direct code copying unless a reviewed port plan names the copied/translated pieces | Future importer issue identifies exact qLDPC input objects to accept, maps them into the explicit `qec-code` spec schema, adds license/provenance notes, and includes positive and negative fixtures generated without runtime qLDPC dependency |
| QuantumExpanders.jl | https://github.com/QuantumSavory/QuantumExpanders.jl | License compatibility to confirm before implementation reuse | mathematical/reference implementation for quantum Tanner and expander-code vocabulary | Reference-only; do not copy code or import formats unless license compatibility is confirmed | Future searcher issue documents the construction family being matched, confirms license posture, and adds independently generated explicit-group specs consumed by `qec-code` without Julia/Oscar runtime calls |
| qTanner | https://github.com/RebKatRad/qTanner | License compatibility to confirm before implementation reuse | source-grounded data/reference for quantum Tanner examples and future fixture ideas | Reference-only; use as source-grounded data/reference unless license compatibility is confirmed | Future adapter issue names the accepted qTanner artifacts, confirms license terms for any copied data, and adds schema validation plus fixtures that still enter `qec-code` as explicit finite data |
| Giacomo-Fregona/QTC | https://github.com/Giacomo-Fregona/QTC | License status to confirm before code reuse | Small Python reference for comparing construction vocabulary and example shapes | No code reuse before license review; cite only high-level behavior until terms are confirmed | Future issue records the license decision, identifies any reusable examples, and adds an importer/searcher only after the accepted input contract and provenance are explicit |
| quantum-tanner-sogrand | https://github.com/grand-decoder/quantum-tanner-sogrand | README notes a non-commercial academic license | Decoder-focused reference for downstream experiments around quantum Tanner matrices | Likely not suitable for code copying into this repo; do not vendor decoder code or derive implementation from it without legal review | Future issue is limited to interoperability notes or externally generated matrix-consumption checks unless license review explicitly permits a narrower artifact use |
| QUITS | Local `drafts/quits`; upstream https://github.com/mkangquantum/quits | License status to confirm before code or data reuse | Downstream matrix-consumption inspiration, not a quantum Tanner constructor | Treat as reference-only for consumer workflow ideas; not a quantum Tanner constructor, and do not import as a constructor or copy code without license review | Future issue describes how exported `sparse_rows` matrices would be consumed downstream, with no changes to the quantum Tanner constructor and no runtime QUITS dependency |

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
