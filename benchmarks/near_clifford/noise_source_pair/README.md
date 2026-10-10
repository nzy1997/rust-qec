# Supplementary zero-noise sign source-pair screen

This separate fixed screen tests the zero-noise row certificate candidate against
its unchanged baseline. It supplements the frozen 36-cell protocol at
`3ee3f528ad2e260834e5b7300068649b69c76cac`; results and weights are never pooled.

28 cells: original full-rank-12/projected-rank-16 input names, six probabilities
0/0.0001/0.001/0.01/0.1/1, Strict/Fused, shots=1 (24 cells), plus four p=0.001
shots=64 packet controls. Input names do not certify active rank; each native
probe reports the actual peak rank. p=0 tests the best case; p=0.1/1 tests
hit-heavy overhead. The only gate transformation is eight DEPOLARIZE1 probability
substitutions per input. Targets, gate order, all other bytes and annotations are
preserved. manifest.json records original and derived hashes. The original13
inputs remain for provenance/schema regressions; only12 derived inputs run.

Every source's scalar129 and flat129 literal records plus16 carry words must
match within and across sources before timing (96 finite children). Six balanced
rounds use baseline, candidate and the identical baseline binary; each role
occupies each order position twice. All seven adaptive observations per child
reach at least50ms.504 timing children produce3528 observations;600 children
and every failure/raw stream are retained. Compile/prepare/first-call times,
cache reservation and whole-process RSS remain separate diagnostic quantities.
No outliers are dropped; paired ranges are not confidence intervals. Source-effect
evidence does not establish peer superiority, SOTA or production admission.

Python3.10+ and Rust1.93.1 native Release/locked are required. Preparation retains
phase-specialized FP and wide raw-record/RNG guards, requires the exact three
zero_noise_summary tests and existing inline-layout test, and snapshots all
Rust/Cargo sources and both unchanged public probe binaries. The compiler flags
are -C target-cpu=native. Cold setup is excluded from warm public-call timings;
per-call RNG and output destruction are included under the existing public probe
contract. Seed739 and SmallRng/rand-0.8.7 are unchanged.

Run test_contract.py with python3 -I -B, normally and with -O. Then use prepare.py
--baseline-ref EXACT_SHA --candidate-ref EXACT_SHA --out NEW/preparation;
run_retained.py --preparation NEW/preparation --out NEW/output --control
NEW/control; verify.py --seal NEW; verify.py NEW --git-sources. Pin collection
with taskset on one quiet logical CPU. The existing x86 application-counts
workflow at this exact diagnostic ref uses cdf_source_pair=true, source SHAs,
and profile_only=false to run this supplementary matrix. It uploads a
near-clifford-x86-noise-source-pair artifact; older36 artifacts use their own
frozen verifier. Strictly close original artifacts before interpretation.

Synthetic fixture timings, binaries and PID receipts exercise verifier corruption
checks only; they are not benchmark evidence. Downloaded originals retain their
seal; historical PID absence uses original-host receipts, never local unrelated
PIDs. Later analyses belong outside the sealed original bundle.
