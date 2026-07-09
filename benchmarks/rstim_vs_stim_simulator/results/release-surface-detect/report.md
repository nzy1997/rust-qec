# rstim Performance Evidence Report

## Gating Cases

### surface-detect-d13-r13

- workload: `detect`
- expected variants: `stim-cli`, `rstim-interpreted`, `rstim-compiled`
- present variants: `rstim-compiled`, `rstim-interpreted`, `stim-cli`
- rstim-compiled median wall time: `34339167` ns over `1` measured rounds
- rstim-interpreted median wall time: `32813250` ns over `1` measured rounds
- stim-cli median wall time: `266171583` ns over `1` measured rounds
- sampler_compiled_vs_interpreted: `rstim-compiled` / `rstim-interpreted` = `1.046503`

## Report-Only Cases

_None._

