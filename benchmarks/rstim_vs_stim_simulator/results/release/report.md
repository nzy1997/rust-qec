# rstim Performance Evidence Report

## Gating Cases

_None._

## Report-Only Cases

### stim-style-surface-sample-d11-r100-b1024

- workload: `sample`
- expected variants: `stim-cli`, `rstim-interpreted`, `rstim-compiled`
- present variants: `rstim-compiled`, `rstim-interpreted`, `stim-cli`
- rstim-compiled median wall time: `566980167` ns over `1` measured rounds (`1806.060` shots/s)
- rstim-interpreted median wall time: `575145500` ns over `1` measured rounds (`1780.419` shots/s)
- stim-cli median wall time: `216811083` ns over `1` measured rounds (`4723.006` shots/s)
- report-only Stim comparison: `rstim-compiled` / `stim-cli` = `2.615088`
- sampler_compiled_vs_interpreted: `rstim-compiled` / `rstim-interpreted` = `0.985803`

