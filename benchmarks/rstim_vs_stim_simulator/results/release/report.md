# rstim Performance Evidence Report

## Gating Cases

_None._

## Report-Only Cases

### stim-style-surface-sample-d11-r100-b1024

- workload: `sample`
- expected variants: `stim-cli`, `rstim-interpreted`, `rstim-compiled`
- present variants: `rstim-compiled`, `rstim-interpreted`, `stim-cli`
- rstim-compiled median wall time: `586794000` ns over `1` measured rounds (`1745.076` shots/s)
- rstim-interpreted median wall time: `590138250` ns over `1` measured rounds (`1735.187` shots/s)
- stim-cli median wall time: `182962958` ns over `1` measured rounds (`5596.761` shots/s)
- report-only Stim comparison: `rstim-compiled` / `stim-cli` = `3.207174`
- sampler_compiled_vs_interpreted: `rstim-compiled` / `rstim-interpreted` = `0.994333`

