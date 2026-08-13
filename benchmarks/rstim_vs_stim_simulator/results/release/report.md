# rstim Performance Evidence Report

## Gating Cases

_None._

## Report-Only Cases

### stim-style-surface-sample-d11-r100-b1024

- workload: `sample`
- expected variants: `stim-cli`, `rstim-interpreted`, `rstim-compiled`, `rstim-interpreted-atom-loss`
- present variants: `rstim-compiled`, `rstim-interpreted`, `rstim-interpreted-atom-loss`, `stim-cli`
- atom-loss probability: each two-qubit gate has one depolarization event and two independent per-atom loss events; using `p = 1 - 0.999^(1/3) ~= 0.0003334445062` keeps the probability of at least one error equal to `0.001`.
- atom-loss execution: a per-qubit loss/reset dataflow proof selects between a 64-shot reference-frame kernel and a parallel packed stabilizer-trajectory kernel; this fixture requires trajectories because loss can suppress later CX gates on the same qubits.
- rstim-compiled median wall time: `8926334` ns over `11` measured rounds (`114716.747` shots/s)
- rstim-interpreted median wall time: `10567292` ns over `11` measured rounds (`96902.783` shots/s)
- rstim-interpreted-atom-loss median wall time: `1203423792` ns over `11` measured rounds (`850.906` shots/s)
- stim-cli median wall time: `243594417` ns over `11` measured rounds (`4203.709` shots/s)
- report-only Stim comparison: `rstim-compiled` / `stim-cli` = `0.036644`
- sampler_compiled_vs_interpreted: `rstim-compiled` / `rstim-interpreted` = `0.844713`
- sampler_atom_loss_vs_interpreted: `rstim-interpreted-atom-loss` / `rstim-interpreted` = `113.881947`
