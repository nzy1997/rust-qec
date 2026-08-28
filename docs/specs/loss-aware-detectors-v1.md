# Loss-Aware Detectors and Superchecks v1

Status: **experimental public API contract**. This document defines how
`rstim` turns known missing measurement records into detector information. It
is consumed by the follow-up
[`delayed-erasure-conditioned-decoding-v1.md`](delayed-erasure-conditioned-decoding-v1.md)
contract for shot-conditioned MLE and matching.

The reference implementation is [`rstim/src/m2d.rs`](../../rstim/src/m2d.rs):

- `measurements_to_loss_aware_detections` derives loss from interleaved
  loss-visible `flag,value` records;
- `measurements_to_loss_aware_detections_with_loss_mask` accepts an explicit
  same-shape measurement-loss mask. Embedded flags remain authoritative and
  are unioned into that mask.

## 1. Why a lost value is not a binary measurement

The simulator historically stores a lost readout as `1`. That value is a
storage placeholder, not a physical eigenvalue. A detector whose parity uses
that record is unknown. Feeding its placeholder-derived 0/1 value to a
matching decoder invents syndrome information and makes the answer depend on
an arbitrary encoding choice.

For every shot, keep the two channels separate:

1. `measurement_bits[m]` stores the raw 0/1 payload;
2. `measurement_loss_mask[m]` says whether that payload is unknown.

Changing `measurement_bits[m]` at every masked position must not change any
loss-aware check value. The v1 tests enforce this API-level invariant; the
conditioned-decoding tests also enforce decoder-answer invariance.

For an `ML`/`MRL`-family instruction, records are interleaved as
`flag,value`. A set flag marks the paired value as lost. Flags are metadata and
may not be referenced by `DETECTOR` or `OBSERVABLE_INCLUDE`.

## 2. Detector algebra

Let `H` be the binary detector-to-measurement incidence matrix and let `L` be
the columns marked lost in one shot. An original detector row `H[d]` is valid
exactly when it has no support in `L`.

A loss-aware check is a binary combination `c` of original detectors such
that:

```text
(c^T H)[L] = 0.
```

Its value is the XOR of the source detector values. Lost measurement terms
occur an even number of times and cancel. `rstim` performs deterministic sparse
Gaussian elimination over the restricted matrix `H[:, L]` and returns a basis
for its left kernel. Therefore the number of returned independent checks is:

```text
num_detectors - rank(H[:, L]).
```

Unaffected detectors are returned as singleton source lists. Combinations of
two or more source detectors are superchecks. The basis is deterministic but
is not promised to minimize supercheck weight; consumers must use the listed
source indices instead of depending on an incidental basis choice.

### Temporal example

```text
D_t     = m_(t-1) xor m_t
D_(t+1) = m_t     xor m_(t+1)
```

If `m_t` is lost, neither original detector is valid, but their product is:

```text
D_t xor D_(t+1) = m_(t-1) xor m_(t+1).
```

The API returns one check with `source_detectors = [t, t+1]`.

### Scope: measurement-record erasure

The explicit-mask API operates on missing **measurement records**. A physical
data-atom loss can invalidate several neighboring stabilizer measurements
because later gates are skipped. Marking two distinct stabilizer records lost
in this v1 mask treats them as two independent unknown columns; it does **not**
assert that they contain the same latent contribution and therefore does not,
by itself, construct the spatial supercheck around a lost data atom.

The delayed-erasure compiler must propagate the physical loss window and
either rewrite the stabilizer/check rows or provide incidence from affected
records to shared latent loss variables. Only then is it valid to cancel a
common data-atom contribution across distinct measurement records. A flag at
final data readout alone is insufficient. V1 is consequently the complete
contract for measurement-record erasure and temporal superchecks, not yet for
spatial code deformation.

## 3. Output contract

Each `LossAwareDetectorShot` contains:

- `lost_measurements`: sorted measurement-record indices marked unknown;
- `detector_valid`: a fixed-width mask over original circuit detectors;
- `checks`: a maximal independent basis. Each entry contains sorted
  `source_detectors` and its binary `value`;
- `canonical_detector_values`: the fixed-width original detector pattern after
  masked measurement payloads are canonicalized to `1`. This compatibility
  channel is intended for Pauli-envelope matching; it is placeholder-invariant
  but does not claim that a masked measurement became physically known.

The loss-aware API intentionally does not return an observable parity. A
logical operator crossing lost support can be undefined for the same reason as
an invalid detector. Blinded decoder datasets instead carry the hidden logical
input introduced by issue 664 as the training/evaluation answer.

Legacy `measurements_to_detections` and CLI `m2d` remain unchanged. Their
binary output is suitable only when all referenced measurements are present,
or as an explicitly named baseline that does not claim to be loss aware.

`LossAwareM2dLimits` bounds detectors and pivots per shot, shots per batch,
each batch-shaped bit table, elimination steps, and cumulative sparse terms
materialized across the batch. Table-shape bounds are preflighted before the
merged loss mask and raw detector/observable tables are allocated. Exceeding a
bound returns an actionable error. The convenience APIs use conservative
defaults; callers processing untrusted or very large batches can call the
explicit limits entry point.

## 4. Decoder and neural-network representations

For a fixed-width neural-network batch, use separate channels rather than
converting loss to a measurement bit:

```text
measurement_bits       [shots, measurements]  bool
measurement_loss_mask  [shots, measurements]  bool
raw_detector_bits      [shots, detectors]     bool, optional
detector_valid         [shots, detectors]     bool
logical_input          [shots]                bool, private label
```

If superchecks are features, store their source indices as a ragged list, or
pad `[shots, checks, max_sources]` and accompany it with a padding mask. The
`checks[].value` channel is already invariant to the lost-bit placeholder.
One-hot ternary `{0, 1, lost}` measurement input is equivalent to the first
two measurement channels, but a separate mask is usually easier to audit and
pack.

Do not train a production loss-aware model using only detector bits computed
after mapping every loss to `0` or `1`. Such a model can learn correlations
from that convention, but it has no way to distinguish valid syndrome from an
invented parity. The 2025 neutral-atom experiment used ternary measurement
features and also formed superchecks; its bare baseline assigned a fixed bit to
loss, while the loss-aware MLE updated the error model per shot.

## 5. Boundary with shot-conditioned decoding

This v1 transform states which parity checks survive a known measurement-loss
pattern and also supplies the explicit fixed-gauge compatibility channel used
by Pauli-envelope matching. The conditioned-decoding follow-up combines these
outputs with native delayed-erasure onset envelopes; the separation remains
useful because the detector transform itself is decoder-neutral.

The follow-up delayed-erasure integration must:

1. enumerate loss onset envelopes consistent with each herald;
2. propagate skipped-gate effects;
3. derive shared latent-loss incidence or rewritten spatial superchecks;
4. transform every error mechanism through the shot's check basis;
5. merge mechanisms with the same detector/observable effect;
6. represent a shared unknown bit as one correlated gauge mechanism, not as
   independent 50% flips on adjacent detectors;
7. emit a shot-conditioned DEM/matching graph and verify decoder-level
   placeholder invariance.

## 6. Research basis and alternatives

- Stace, Barrett, and Doherty introduced loss-tolerant topological-code
  decoding by deforming stabilizers around known lost qubits and established
  the surface-code loss threshold: [arXiv:0904.3556](https://arxiv.org/abs/0904.3556).
- Stace and Barrett analyzed degeneracy and combined computational/loss errors
  in the surface code: [arXiv:0912.1159](https://arxiv.org/abs/0912.1159).
- Delfosse and Zémor gave a linear-time maximum-likelihood decoder for the
  known-location quantum erasure channel. Peeling/GF(2) erasure decoding is an
  alternative when the code and noise model fit that setting:
  [Phys. Rev. Research 2, 033042](https://doi.org/10.1103/PhysRevResearch.2.033042).
- Perrin, Jandura, and Pupillo adapt the decoding graph using neutral-atom loss
  locations, showing a large gain over a naive decoder:
  [arXiv:2412.07841](https://arxiv.org/abs/2412.07841).
- Baranes et al. treat a herald observed after an unknown loss time as a
  delayed erasure rather than a precisely located erasure:
  [arXiv:2502.20558](https://arxiv.org/abs/2502.20558).
- The neutral-atom architecture experiment constructs superchecks, updates its
  MLE circuit error model per shot, and supplies loss explicitly to learned
  decoders: [Nature 648, 1004–1011 (2025)](https://doi.org/10.1038/s41586-025-09848-5).

Other viable decoder families include exact/envelope MLE, erasure peeling,
union-find variants, belief propagation with erasure priors, tensor-network
decoding, and learned decoders with explicit loss channels. The present API is
decoder-neutral: all of them can consume the surviving parity basis without
interpreting a placeholder as a measurement.
