# Current CPU near-Clifford baseline

This is an additive baseline for deciding the next optimization. It preserves
the older 87-configuration refinement matrix. It does not establish a SOTA win.
The frozen manifest contains seven inputs and three shot sizes (1, 64, 1024),
including published magic-state cultivation d3/d5 circuits. The comparison
returns **every raw measurement record**, without postselection. Detector and
observable annotations are omitted identically before compilation. Unsupported
arbitrary rotations and U3 circuits remain explicitly listed in the manifest.
Counts-only, early-rejection and GPU throughput are different contracts and are
not covered by this CPU comparison.

Clifft 0.11.0 is tested with both its default compiler and the opt-in active-width
schedule pass. Each peer independently tunes batch sizes 1/64/256/1024/auto in
separate processes. The selected settings are frozen before final measurements.
SymFT is built from SOFT commit `c89b98514a919240b8afa53a271e08d926d3c987`,
whose package version is also 0.1.1; the version alone cannot identify this build.
The evidence records checkout file hashes, installed extension/package hashes
and SIMD backend. These are reproducibility records, not a cryptographic proof
that an installed extension was built from the supplied checkout.

## Reproduce

Use dedicated Python 3.12 environments, with pinned dependencies:

```sh
uv venv --python 3.12 drafts/sota-peers
uv pip install --python drafts/sota-peers/bin/python -r benchmarks/near_clifford/sota/requirements.txt
git clone https://github.com/haoliri0/SOFT.git drafts/sota-symft-source
git -C drafts/sota-symft-source checkout c89b98514a919240b8afa53a271e08d926d3c987
uv venv --python 3.12 drafts/sota-symft
uv pip install --python drafts/sota-symft/bin/python numpy==2.4.6
uv pip install --python drafts/sota-symft/bin/python --no-cache drafts/sota-symft-source/python
python3 benchmarks/near_clifford/sota/run.py \
  --python drafts/sota-peers/bin/python \
  --symft-python drafts/sota-symft/bin/python \
  --symft-source drafts/sota-symft-source \
  --out drafts/sota-baseline
python3 benchmarks/near_clifford/sota/verify.py drafts/sota-baseline/results.json \
  --git-sources --artifacts drafts/sota-baseline
```

The source checkout must be clean and the installed SymFT build must come from
it. The runner can use a standard-library-only launcher: backend imports and
inspection run in the specified interpreters. It builds its locked Rust probe
before timing, freezing source/harness inventories before and after the build.
It rejects a source, fixture, harness or binary change during the campaign.

`--only terminal msc3 --shots 64 --pairs 1 --repetitions 3` is a smoke run.
Its result is marked a subset and the verifier requires `--allow-subset`.
Publication evidence requires all 21 configurations, at least three independent
process pairs, seven repetitions and 8192 validation samples.

## Timing and correctness

Final process order rotates and reverses between pairs. Each warm observation
accumulates at least 50 ms of public API calls; per-call seed setup and output
destruction are excluded for all backends. First-call, compile and prepare times
are retained separately. Compiler imports, tuning and correctness are untimed.
The report shows medians and paired ranges, without treating them as confidence
intervals. The fastest independently tuned valid peer is the comparison target.
One CPU thread is requested; macOS core affinity is unavailable. Results are
host-specific, and Apple ARM scalar SymFT results do not establish x86 SIMD or
GPU superiority. Complex fp64 arithmetic is required on both CPU peers.

Correctness checks every marginal, adjacent-pair parity, contiguous groups of
four, full parity and original detector/observable parities. A conservative
two-sample Hoeffding union bound covers default and selected paths, with family
error budget 0.001. Selected paths collect 8192 samples using the **exact timed
shots per call and batch configuration**, including rstim. This finite set of
parity projections is not proof of equality of the entire joint distribution.
Lossless compressed raw transcripts accompany all counts, so the verifier can
recompute them without peer packages or local scratch files. Runtime checks
remain active under `python -O`.

The native MPP and noisy M/MR variants require a benchmark-only exact adapter
for rstim. MPP conjugates its signed Pauli product to a one-qubit Z measurement
and reverses the Clifford conjugation. Noisy readout copies the ideal eigenbit
to one reusable spectator, applies the readout flip to that spectator, measures
it, uncomputes and resets it. MR also resets the data qubit in its original basis.
This preserves conditional data state, reported bits, feedback and record order;
it introduces no extra records. Competitors receive their stronger native input.
Independent dense-instrument tests compare every unnormalized conditional density
matrix for complex input states, signed Y products, noncommuting measurements,
readout probabilities 0/0.37/1, feedback and reset. The spectator must end in |0>.

```sh
drafts/sota-peers/bin/python benchmarks/near_clifford/sota/test_lowering.py
python3 benchmarks/near_clifford/sota/test_contract.py
python3 -O benchmarks/near_clifford/sota/test_contract.py
```

Cultivation fixtures originate from the Apache-2.0 `unitaryfoundation/clifft-bench`
repository; exact source commits, paths and input hashes are in `manifest.json`.
