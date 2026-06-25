# Issue 219 Benchmark Evidence Showcase Design

Date: 2026-06-25
Status: Approved by non-interactive Standing Answer Policy
Scope: Documentation-only showcase for checked-in benchmark and reproduction
evidence

## Summary

Issue #219 adds a showcase page that maps the repository's existing benchmark
and reproduction evidence without turning smoke output into statistical claims.
The page lives at `docs/showcases/benchmark-evidence.md` and follows the
showcase contract from `docs/showcases/README.md`.

The page is a guide to existing surfaces, not a new benchmark result. It should
link readers to the surface-decoder comparison README, checked-in comparison
artifacts, the BB144 circuit-level BP-OSD reproduction note, and the benchmark
spec/registry tests that keep documented runner keys current.

## Evidence Sources

- `benchmarks/surface_decoder_compare/README.md` documents the cross-decoder
  comparison harness, smoke commands, full/manual campaigns, and checked-in
  full-tier artifact locations.
- `benchmarks/surface_decoder_compare/results/full/results.csv` and
  `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`
  are tracked artifacts from the full tier.
- `benchmarks/surface_decoder/spec.toml` and
  `benchmarks/surface_decoder/full.toml` are the `rsinter` surface-decoder
  smoke and full specs.
- `docs/bb144_circuit_bposd_reproduction.md` records BB144 circuit-level
  BP-OSD smoke evidence and the manual upstream-budget command.
- `docs/figures/bb144_reference/small_ldpc.png` and
  `docs/figures/bb144_reference/ldpc_vs_surface.png` are reference figures for
  the BB144 reproduction note.

There is no committed `benchmarks/bb_circuit_bposd/README.md` on this branch.
The showcase should not add one as a hidden prerequisite; it should link the
already committed BB144 reproduction note instead.

## Documentation Approach

Use one showcase page with the required sections:

- `What This Shows`: state that the page maps benchmark evidence surfaces and
  separates smoke evidence from manual/full campaigns.
- `Run It`: list smoke commands first, then manual/full campaign commands in a
  clearly labeled subsection.
- `Expected Result`: describe where smoke and full artifacts appear, and state
  which artifacts are checked in.
- `Code`: link the owning docs, specs, results, Makefile, and BB144 evidence
  note.
- `Verification`: list the issue-required checker, Python docs contract, and
  Rust benchmark spec/registry tests. Name the Python docs contract test that
  owns the typo negative control.
- `Limits`: explicitly avoid statistical or algorithmic conclusions beyond
  the checked evidence.

Update `docs/showcases/README.md` with a single link to the new page under
`Decoder And Benchmark Workflows` so users can discover it from the showcase
index.

## Negative Control

The issue requires a documented runner-key typo, `bb-circuit-bposd-memroy`, to
be rejected. Enforce this with a narrow Python docs-contract test in
`benchmarks/surface_decoder_compare/tests/test_docs_contract.py`. The test
should parse backtick-delimited BB circuit command keys in the showcase, allow
only `bb-circuit-bposd-memory`, and include a mutation-style negative control
that replaces the valid key with the typo and expects rejection.

This keeps the checker generic while ensuring the exact typo is covered by an
existing required verification command.

## Verification Commands

```bash
python3 tools/check_showcase_docs.py docs/showcases/benchmark-evidence.md
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
cargo test -p rsinter --test bench_specs --test bench_registry -q
cargo test
```

## Out Of Scope

- New benchmark results.
- New benchmark campaigns or changed benchmark budgets.
- Implementing issue #124.
- Resolving issue #110.
- Algorithmic or statistical claims not already supported by checked evidence.
