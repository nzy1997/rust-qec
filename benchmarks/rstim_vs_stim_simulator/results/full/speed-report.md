# rstim Performance Evidence Report

## Gating Cases

_None._

## Report-Only Cases

### stim-style-surface-sample-d11-r100-b1024

- workload: `sample`
- expected variants: `stim-cli`, `rstim-interpreted`, `rstim-compiled`
- present variants: `rstim-compiled`, `rstim-interpreted`, `stim-cli`
- rstim-compiled median wall time: `47026641750` ns over `1` measured rounds (`21.775` shots/s)
- rstim-interpreted median wall time: `46425263750` ns over `1` measured rounds (`22.057` shots/s)
- stim-cli median wall time: `179944333` ns over `1` measured rounds (`5690.649` shots/s)
- report-only Stim comparison: `rstim-compiled` / `stim-cli` = `261.339943`
- sampler_compiled_vs_interpreted: `rstim-compiled` / `rstim-interpreted` = `1.012954`

