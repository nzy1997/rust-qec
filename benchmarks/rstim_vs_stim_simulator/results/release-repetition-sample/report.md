# rstim Performance Evidence Report

## Gating Cases

### rep-sample-d13-r13

- workload: `sample`
- expected variants: `stim-cli`, `rstim-interpreted`, `rstim-compiled`
- present variants: `rstim-compiled`, `rstim-interpreted`, `stim-cli`
- rstim-compiled median wall time: `1737666` ns over `1` measured rounds (`11509691.736` shots/s)
- rstim-interpreted median wall time: `1875666` ns over `1` measured rounds (`10662879.212` shots/s)
- stim-cli median wall time: `86111041` ns over `1` measured rounds (`232258.254` shots/s)
- report-only Stim comparison: `rstim-compiled` / `stim-cli` = `0.020179`
- sampler_compiled_vs_interpreted: `rstim-compiled` / `rstim-interpreted` = `0.926426`

## Report-Only Cases

_None._

