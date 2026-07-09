# Expanded rstim-vs-Stim Correctness Evidence

Status: pass

## Checked Artifacts

- Distribution summary: `benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json`
- Expanded rollup: `benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json`
- Existing full fixture correctness summary: `benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`

## Scope

This evidence covers the source-grounded small-circuit distribution catalog and
the existing checked d11/r100 full fixture summary. It does not extend coverage
to issue #431 test artifacts or to unlisted Stim workloads.

## Verification

```sh
python3 tools/check_rstim_vs_stim_expanded_correctness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --distribution-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-summary benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
```
