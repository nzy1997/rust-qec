# Experimental compiled CPU near-Clifford comparison

This standalone harness measures the opt-in `CompiledNearCliffordExecutor` API.
It does not establish a SOTA win. The earlier `../sota/` baseline and its results
are separate, immutable evidence. The frozen manifest preserves its seven
fixtures and three shot sizes (1, 64, 1024), including published magic-state
cultivation d3/d5 circuits. Every backend receives **the identical native
records-only input** and returns every raw measurement record, without
postselection. Detector and observable annotations alone are omitted before
compilation; their original parities remain in correctness checks. There is no
spectator qubit, MPP lowering, noisy-readout lowering or extra measurement.

The experimental compiled API natively supports signed MPP products with
distinct qubits within each product, and noisy M/MR variants in its supported
Clifford+T subset. Arbitrary rotations and U3 remain explicitly unsupported in
the manifest. This harness uses the public `sample_measurements_u8` entry point
with its default rank limit and sampler cache budget. `peak_active_rank` is the
static plan's rank; `cache_reserved_bytes` is the sampler's conservative admitted
coefficient-cache reservation after validation or warming, excluding the static
plan and arithmetic buffers. It is not RSS or a peak-process-memory claim.

The compiled API has its own RNG contract, which can differ from the legacy
executor's stream. The probe still supplies `SmallRng` from locked rand 0.8.7.
Distribution comparisons do not require matching random bits between simulators.
Both timed observations and default/selected validation transcripts must identify
`api: CompiledNearCliffordExecutor`; the verifier rejects legacy API evidence.
The manifest and result schemas use the separate compiled-sota v1 namespace.

Clifft 0.11.0 is tested with both its default compiler and the opt-in active-width
schedule pass. Clifft independently tunes batch sizes 1/64/256/1024/auto.
SymFT additionally tunes its distinct public scalar executor (`scalar` sentinel:
`compile_sampler(batch=False, batch_size=0)`); its other candidates use
`batch=True` with sizes 1/64/256/1024/auto. A batch size of 1 does not select
the scalar executor. Default SymFT validation uses its public scalar default;
selected validation and timing use the frozen winner, including scalar when it
wins. All candidates run in isolated processes, and selection is frozen before
final measurements. SymFT is built
from SOFT commit `c89b98514a919240b8afa53a271e08d926d3c987`, whose package
version is also 0.1.1. Checkout file hashes, installed package/extension hashes
and SIMD backend are recorded. Every package inspection and peer worker uses
Python `-I`, excluding `PYTHONPATH`, the current directory and user site packages.
Each worker hashes the actual imported backend wrapper and native extension
before and after its work; the runner and verifier bind those paths and hashes
to the initial distribution inventory. Campaign completion recaptures package
files, actual imports and the SOFT checkout, requiring equality with the initial
environment. The required before/after guard stores small SHA-256 summaries,
while absolute distribution file locations are recorded only once. Missing or
changed identities and guards fail verification. These are reproducibility records, not a
cryptographic proof that an extension came from the supplied checkout.

## Reproduce

Use dedicated Python 3.12 environments with pinned dependencies:

```sh
uv venv --python 3.12 drafts/compiled-sota-peers
uv pip install --python drafts/compiled-sota-peers/bin/python -r benchmarks/near_clifford/compiled_sota/requirements.txt
git clone https://github.com/haoliri0/SOFT.git drafts/compiled-sota-symft-source
git -C drafts/compiled-sota-symft-source checkout c89b98514a919240b8afa53a271e08d926d3c987
uv venv --python 3.12 drafts/compiled-sota-symft
uv pip install --python drafts/compiled-sota-symft/bin/python numpy==2.4.6
uv pip install --python drafts/compiled-sota-symft/bin/python --no-cache drafts/compiled-sota-symft-source/python
python3 benchmarks/near_clifford/compiled_sota/run.py \
  --python drafts/compiled-sota-peers/bin/python \
  --symft-python drafts/compiled-sota-symft/bin/python \
  --symft-source drafts/compiled-sota-symft-source \
  --out drafts/compiled-sota-campaign
python3 benchmarks/near_clifford/compiled_sota/verify.py drafts/compiled-sota-campaign/results.json \
  --git-sources --artifacts drafts/compiled-sota-campaign
```

The source checkout must be clean and the installed SymFT build must come from
it. The runner uses a standard-library-only launcher; backend imports and
inspection run in the specified interpreters. It builds the independently named
`near-clifford-compiled-sota` Rust probe with locked dependencies. Production and
harness inventories are frozen before the build, checked after the build, and
checked again at campaign completion, including after final peer environment
capture, together with the binary digest. The
Rust probe and peer worker hash the exact UTF-8 bytes they consume before timing.
Every dump, tuning and timing observation carries that digest; the launcher and
offline verifier require it to match the frozen native input. This binding also
applies when scratch circuit files are unavailable to the verifier. The
verifier's `--git-sources` option additionally checks the complete production
source-file set and each hash against the recorded Git revision. Commit the
source before collecting evidence intended to pass that check.

`--only terminal msc3 --shots 64 --pairs 1 --repetitions 3` is a smoke run.
Subset evidence is marked and requires `--allow-subset`. Publication evidence
requires all 21 configurations, at least three independent process pairs,
seven repetitions and 8192 validation samples. Partial or failed campaigns
cannot pass the publication verifier.

## Timing and correctness

Final process order rotates and reverses between pairs. Every warm observation
accumulates at least 50 ms of public API calls. Caller-side seed/RNG setup and
output destruction are outside measured windows for every backend; seed handling
inside a peer's public sampling API remains timed. Rust compile and prepare are
separate; peers' prepare work is included in compile. First calls use newly
compiled/prepared objects and are retained separately from warmed calls. Imports,
tuning, correctness checks and process startup are untimed. The report retains
compile, prepare, first-call and warm medians, plus paired warm speedup ranges
against the fastest independently tuned valid peer. These ranges are not
confidence intervals.

One CPU thread is requested; macOS core affinity is unavailable. Results are
host-specific. Apple ARM scalar SymFT results do not establish x86 SIMD or GPU
superiority. Complex fp64 arithmetic is required on both CPU peers. Counts-only,
early-rejection, postselection and GPU throughput are different output contracts
and are outside this comparison.

Correctness checks every marginal, adjacent-pair parity, contiguous groups of
four, full parity and original detector/observable parities. A conservative
two-sample Hoeffding union bound covers all six backend pairs for both default
paths and each of three selected shot sizes, with family error budget 0.001.
Selected paths collect 8192 samples using the **exact timed shots per call and
batch configuration**, including rstim and SymFT scalar/batch identity. This finite parity set does not prove
equality of the entire joint distribution. Lossless compressed raw transcripts
accompany all counts; verification recomputes every parity and input digest
without peer packages or local scratch files. Runtime checks remain active under
`python -O`. The original fixtures and their source/license metadata are frozen.

Lightweight consistency and adversarial contract tests do not build or benchmark:

```sh
python3 benchmarks/near_clifford/compiled_sota/test_contract.py
python3 -O benchmarks/near_clifford/compiled_sota/test_contract.py
```

Cultivation fixtures originate from the Apache-2.0
`unitaryfoundation/clifft-bench` repository. Exact source commits, paths, input
hashes and incompatible published workloads are in `manifest.json`; its fixture
license is included. Production dense-instrument and seeded compiled-API tests
provide separate mathematical validation and are not run by this harness.
