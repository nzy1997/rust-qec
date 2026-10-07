# Compiled near-Clifford CPU comparison

This campaign does not establish broad SOTA superiority. The experimental
`CompiledNearCliffordExecutor` has a 1.4011× geometric mean warm speedup against
the fastest valid, independently tuned peer in each of the 21 configurations.
The geometric means at 1, 64 and 1024 shots are 2.1378×, 1.2781× and 1.0066×.
Sixteen of the 21 median ratios exceed one. The cultivation family geometric
mean is 0.9382×; terminal at 1024 shots, cultivation d3 at 1024 shots, and all
three d5 configurations remain slower than their fastest peer.

The [raw observations](results.json) and [derived report](report.md) retain all
seven fixtures at all three shot counts, 336 tuning trials, three rotated process
pairs, seven observations per pair, and separately measured compilation,
preparation and first calls. The finite distribution witnesses passed for every
default and selected path. Their lossless transcripts are embedded for replay;
they do not prove arbitrary joint-distribution correctness. Independent physical
oracles and fixed-plan raw-record/RNG-continuation tests cover implementation
correctness separately.

Cold costs remain actionable. At d5/64 the median first call is 3.736 ms versus
1.333 ms for default Clifft and 1.326 ms for scheduled Clifft, with approximately
64 MiB of retained rstim cache. First calls and warm throughput must therefore
remain separate. Remaining directions include cache admission, cultivation
rotation kernels, random-event preparation, and terminal measurement/broadcast
work. These results do not justify stopping optimization.

The frozen source is `e514459851a3873eb6f4d93293ffef875cb4f19c`, with the explicit
immutable `Fused` FP64 rotation policy. `Strict` remains the public default.
The [protocol](../../compiled_sota/README.md) documents identical native
records-only inputs, current Clifft 0.11.0 and SOFT commit c89b985, actual import
and binary provenance, source guards, timing boundaries and arithmetic limits.
Peer wheel compiler flags are not attested. This experiment excludes arbitrary
RX/RZ/U3, postselection, counts-only APIs, GPUs and x86 claims.

The [host attestation](host-attestation.json) records a shared Apple M4 with
ten logical CPUs and 32 GiB RAM before and after the campaign. Unrelated CPU
workers remained running. This is neither an idle-host nor a pinned-core result;
process ranges in the report are not confidence intervals. Peak RSS was not
measured. Historical campaigns ran under different host conditions and do not
provide paired before/after speedups for this implementation.

Verification:

```sh
python3 benchmarks/near_clifford/compiled_sota/verify.py \
  benchmarks/near_clifford/results/apple-m4-compiled-sota-2026-10-07/results.json \
  --git-sources
python3 -O benchmarks/near_clifford/compiled_sota/verify.py \
  benchmarks/near_clifford/results/apple-m4-compiled-sota-2026-10-07/results.json \
  --git-sources
```

Both modes verified the original campaign with its duplicate scratch artifacts.
The retained bundle is checked again at publication. Scratch artifacts are not
needed to replay the embedded statistical transcripts or reconstruct the frozen
tuning text.
