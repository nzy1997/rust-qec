# Issue 100 rbposd Evidence Reconciliation Design

Date: 2026-06-21
Status: Approved by non-interactive Standing Answer Policy
Scope: Evidence-facing documentation for checked-in `rbposd` surface-decoder
benchmark results and post-LSD/BP-option alignment notes

## Summary

Issue #100 reconciles the stale `rbposd` core performance design narrative with
the tracked full-tier comparison artifact at
`benchmarks/surface_decoder_compare/results/full/results.csv`.

The old design summary said the checked-in full benchmark showed `rbposd`
trailing `ldpc` by `39.7x` to `131.0x`. The tracked CSV no longer supports that
claim. In the checked-in native `full` rows for `distance in {3, 5}` and
`p in {0.002, 0.005, 0.010}`, `rbposd` has lower `decode_us_per_shot` than
`ldpc` for every paired case.

This issue should update docs and tests, not decoder behavior or benchmark
artifacts.

## Evidence Source

All numeric claims in this issue are scoped to the tracked CSV artifact:

`benchmarks/surface_decoder_compare/results/full/results.csv`

That artifact is evidence of the checked-in benchmark run, not a fresh claim
about current local machine speed. The comparison rows are native backend,
full-tier, one seed, decode-time-only measurements using the recorded shot and
error budgets.

## Checked-In ldpc Versus rbposd Rows

| distance | rounds | p | ldpc decode_us_per_shot | rbposd decode_us_per_shot | rbposd / ldpc |
| --- | --- | --- | ---: | ---: | ---: |
| 3 | 3 | 0.002 | 9.28949949957314 | 5.533358 | 0.596 |
| 3 | 3 | 0.005 | 15.255653700023686 | 9.888312299999999 | 0.648 |
| 3 | 3 | 0.010 | 22.875337890445515 | 18.083490234375002 | 0.791 |
| 5 | 5 | 0.002 | 194.81863700011675 | 128.28873740000003 | 0.659 |
| 5 | 5 | 0.005 | 386.04600850012497 | 322.0114498 | 0.834 |
| 5 | 5 | 0.010 | 737.693568638826 | 639.9339513020834 | 0.867 |

The docs may summarize this as "`rbposd` is faster than `ldpc` in the tracked
checked-in full-tier native rows." The docs must not summarize it as "`rbposd`
is currently faster than `ldpc` in general" or as a broader benchmark guarantee.

## Alignment Context After LSD And BP-Option Work

The relevant milestone outputs now present in the repo are:

- Issue #92: `rsinter` can run LSD-backed `rbposd` DEM decoding.
- Issue #93: LSD benchmark rows record normalized LSD params and have focused
  artifact coverage.
- Issue #94: `rbposd::DecoderConfig` exposes `ProductSum` and `Serial`.
- Issue #95: non-default BP method and schedule paths are behavior-affecting.
- Issue #96: `rsinter` parses, validates, and records BP method/schedule
  selections.
- Issue #97: Rust and Python parity-harness tests give teeth to the BP option
  mapping against upstream `ldpc` names.
- Issue #99: checked-in `rsinter` benchmark specs include
  `rbposd_lsd_order1` and `rbposd_product_sum_serial` runner entries.

The remaining alignment gaps are evidence gaps, not blockers for the default
tracked comparison rows:

- `benchmarks/surface_decoder_compare/results/full/results.csv` contains the
  default comparison runner rows only; it does not contain checked-in timing
  rows for `rbposd_lsd_order1` or `rbposd_product_sum_serial`.
- The LSD and BP-option work covers a narrow supported surface:
  `localized_statistics` LSD order 1 and `product_sum` with `serial` BP
  schedule. It does not claim full upstream `ldpc` option parity.
- The tracked comparison is decode-time evidence for checked-in artifacts. It
  should not be used as a machine-independent speed promise.

## Documentation Plan

Update `docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md` so
its summary no longer says the current checked-in artifact shows `rbposd`
trailing `ldpc` by `39.7x` to `131.0x`. Preserve the historical performance
refactor rationale, but add an evidence update that:

- cites the tracked CSV path;
- lists the paired `ldpc` and `rbposd` rows or a summarized table;
- distinguishes tracked results from current-speed claims;
- distinguishes remaining feature-alignment gaps from speed evidence.

Update `benchmarks/surface_decoder_compare/tests/test_docs_contract.py` so the
contract:

- parses the tracked CSV with the standard library `csv` module;
- verifies the docs cite the required paired rows from issue #100;
- verifies the docs summarize the tracked native full-tier rows consistently
  with the CSV;
- rejects the stale `39.7x` to `131.0x` categorical slower claim.

## Verification Commands

```bash
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py -k stale
cargo test
```

## Out Of Scope

- Raw decoder implementation changes.
- Plot-style edits.
- Benchmark artifact regeneration.
- New benchmark framework architecture.
