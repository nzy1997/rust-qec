# Sampler Performance Readiness

Status: **ready**

## Evidence Bundles
- [fair-cli-release](benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/): PASS fair CLI sampling evidence variants=2 measured=14
- [compiled-steady-release](benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/): PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9
- [reference-build-release](benchmarks/rstim_vs_stim_simulator/results/reference-build-release/): PASS packed reference-build evidence variants=3 direct_speedup=20.978277
- [frame-instruction-wide-release](benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/): PASS instruction-wide frame-noise evidence outcome=improved builds=803 legacy_setups=80362 candidate_over_baseline=0.775851 attempts=82290688

## Readiness Checks
- Reference direct/canonical speedup: `20.978276638796626`x (minimum `2.0x`).
- Direct reference canonical materializations: `0`; executed repeat iterations: `1`.
- Frame candidate/baseline ratio: `0.77585118126732` (maximum `1.05`); correctness: `pass`.
- Distribution correctness: `pass` across `8` cases.
- Historical #406 evidence: `preserved` (`261.34`x stim-cli/rstim-compiled).

## Claim Limits
- Readiness is limited to the committed evidence bundles and focused Rust tests.
- This is not a broad Stim parity claim and does not close #406.
- Site-facing #379 remains separate; this readiness artifact does not update the site or close #379.

## Issue Links
- [#38](https://github.com/nzy1997/rust-qec/issues/38)
- [#406](https://github.com/nzy1997/rust-qec/issues/406)
- [#379](https://github.com/nzy1997/rust-qec/issues/379)
