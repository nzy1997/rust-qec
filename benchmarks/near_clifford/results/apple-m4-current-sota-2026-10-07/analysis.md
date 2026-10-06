# Current CPU baseline after PR #777

This comparison does not establish SOTA superiority. The retained [raw data](results.json)
and [generated report](report.md) contain all 21 frozen configurations, three rotated
process pairs and seven observations per pair. Every default and selected calling
path passed the fixed parity-projection checks. The lossless raw transcripts allow
those checks to be recomputed independently of local peer installations.

At 1024 shots the retained incumbent wins the terminal fixture by 1.1951×, but
loses brick16 by about 65×, parity129 by 2163×, rounds129 by 5083×, cultivation d3
by 321× and d5 by 27×. These are host-specific median warm ratios, with paired
ranges shown in the report; they are not confidence intervals or postselection
throughput. First-call, compilation and preparation observations remain in JSON.

The concrete next hypothesis is an offline Clifford-frame plan with per-shot
virtual Pauli updates and compact coherent amplitudes. Physical Clifford updates,
Pauli reconstruction and basis changes currently repeat across stochastic shots.
The new CPU peers remove much of this work during compilation and batch remaining
classical dependencies. This gap warrants a measured architecture prototype;
small reconstruction-loop improvements alone do not address it.

The [protocol](../../sota/README.md) discloses independently tuned default/scheduled
Clifft 0.11.0 and SOFT c89b985, complex-fp64 CPU arithmetic, native peer inputs,
exact rstim-only MPP/readout lowering, output destruction exclusion and the finite
statistical witness. The frozen corpus includes genuine cultivation applications
and retains unsupported arbitrary rotations/U3 in the manifest. The older scale,
entangled and refinement evidence is unchanged.

The [host attestation](host-attestation.json) records Apple M4, ten cores and
32 GiB RAM after the campaign. CPU affinity and peak RSS were not measured. This
does not cover x86 SIMD, GPU, counts-only or early-rejection contracts.

Verification on the final retained result:

```sh
python3 benchmarks/near_clifford/sota/verify.py \
  benchmarks/near_clifford/results/apple-m4-current-sota-2026-10-07/results.json \
  --git-sources --artifacts drafts/near-clifford-sota-20261007/baseline-full-v1
```

Observed: `evidence verified`. The scratch artifact flag is optional: all raw
transcripts needed to recompute statistics are embedded in `results.json`.
The exact frozen-tuning text is also reconstructible from that file and its hash;
its duplicate 6.7 MB file is intentionally not retained.
