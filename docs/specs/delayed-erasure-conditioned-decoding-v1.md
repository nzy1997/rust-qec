# Delayed-Erasure Conditioned Decoding v1

Status: **experimental decoder contract**. This document specifies how the
native `rustqec decode` path combines delayed atom-loss envelopes with the
loss-aware detector basis from
[`loss-aware-detectors-v1.md`](loss-aware-detectors-v1.md).

## Inputs and compilation

The public measurement row keeps every loss-visible readout as an interleaved
`flag,value` pair. The flag is observed metadata; when it is set, the paired
value is an arbitrary storage placeholder.

Circuit compilation is performed once:

1. `CompiledLossAwareM2d` validates detector rows, records all loss-visible
   pairs, and computes the noiseless reference sample;
2. the delayed-erasure compiler records every physical loss-onset opportunity
   compatible with each later herald;
3. skipped-gate effects are probed through the noiseless circuit and collected
   as one categorical loss envelope per herald;
4. the ordinary decomposed DEM supplies independent Pauli mechanisms and the
   baseline matching graph.

The compiled measurement transform is reused for every input batch. It does
not rebuild the reference sample or measurement layout per batch.

## Shot-conditioned detector model

For one shot, let `B` contain the surviving detector checks returned by the
loss-aware transform. Each row of `B` lists original detector indices; its
observed value is their XOR. Every original detector effect `e` is projected
to the conditioned effect

```text
e_conditioned = B e  (mod 2).
```

This is the direct, in-memory equivalent of rewriting every DEM mechanism for
the shot. Duplicate lost-record contributions cancel inside a supercheck.
The decoder never receives an invalid original detector bit.

The no-loss basis consists only of singleton rows, so the conditioned model is
identical to ordinary detector decoding.

## Envelope MLE

`envelope-mle` builds and caches one ILP model per distinct loss pattern. Its
parity constraints are the shot's surviving checks, not the original detector
rows. Independent Pauli effects and every joint delayed-erasure candidate are
projected through the same check basis.

An active loss envelope selects exactly one joint candidate. The candidate may
flip several detectors and logical observables together. This preserves the
shared latent-loss correlation; it is not replaced by independent 50% flips
on adjacent detectors. Equal-weight envelope candidates represent the unknown
loss onset and skipped-gate history consistent with the herald.

## Envelope matching

`envelope-matching` caches one reweighted copy of the ordinary detector graph
per loss pattern. Its syndrome is the original detector pattern after every
flagged measurement placeholder is canonicalized to `1`; loss flags affect
the graph only through the delayed-erasure edge reweighting. Canonicalization
makes the result independent of the stored placeholder while retaining the
spatial constraints expected by the Pauli-envelope matching algorithm.

Matching remains an approximation to the full categorical envelope model.
In particular, the v1 measurement-record elimination basis cannot substitute
for the spatial code deformation around a physical data-atom loss: projecting
the graph through that incomplete basis deletes valid spatial constraints and
causes a logical-error-rate regression. `envelope-mle` continues to use the
conditioned check basis and is the exact categorical fallback within the
documented flat Mid-SWAP subset.

## Required invariants

For fixed circuit, loss flags, and all known measurement values:

- changing any flagged value placeholder from 0 to 1 must not change the
  surviving checks or the canonical detector pattern;
- it must not change the prediction from either supported decoder;
- shots with the same loss pattern must reuse the same cached graph/model;
- a mismatched detector or check basis for an existing cache key is rejected;
- no-loss predictions remain equal to the preconditioned decoder behavior.

Both a Rust-level known-answer test and a real-CLI two-shot test exercise the
decoder-answer placeholder invariant. The two CLI rows differ only in one
lost value bit.

## Limits and observability

The loss-aware transform retains its detector, batch-table, pivot,
elimination-step, and materialized-term limits. Conditioned matching and MLE
also cap mechanism/check counts, support-scanning work, resident artifact
count, and resident cached work before constructing a shot-specific
graph/model. Deterministic FIFO eviction keeps these limits effective for
zero-check patterns and for streams with many distinct loss patterns without
rejecting an otherwise valid shot merely because earlier patterns filled the
cache.

`matching_graph_builds` and `mle_model_builds` report cumulative artifact
builds, including rebuilds after eviction. `distinct_loss_patterns` and
cache-hit statistics make accidental per-shot recompilation visible. Compile-time
growth is reported by `primitive_probe_count`, `primitive_symptom_terms`, and
`loss_envelope_candidate_count`; the last value is zero for Envelope-Matching
because that backend consumes primitive-to-edge unions directly.

The private `logical_input` label introduced for blinded datasets remains the
evaluation answer source. It is independent of the arbitrary lost-value
placeholder and is not represented as a sampled physical error.

## Deliberate boundaries

- The public file format is unchanged; conditioned graphs and models are
  internal decoder artifacts.
- The v1 native compiler supports the documented flat Mid-SWAP subset and
  rejects unsupported gates or `REPEAT` blocks before envelope expansion.
- Generic hypergraph matching is not synthesized by splitting a correlated
  effect. Use envelope MLE or add a decoder with native hyperedge support.
- Learned decoders should still consume separate measurement-value and loss
  channels as described in the loss-aware detector specification.
