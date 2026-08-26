---
title: "RustQEC: Loss-visible circuit sampling and Pauli-Envelope decoding for atom-loss-aware quantum error correction"
tags:
  - Rust
  - quantum error correction
  - atom loss
  - stabilizer simulation
  - decoding
authors:
  - name: "AUTHOR NAME"
    affiliation: 1
    corresponding: true
affiliations:
  - name: "AUTHOR AFFILIATION"
    index: 1
date: 20 August 2026
bibliography: paper.bib
---

# Summary

Neutral-atom quantum computers lose atoms during operation, and a heralded
loss is valuable side information: the Pauli Envelope framework
[@Liu_2026] turns each observed per-shot loss pattern into an exact decoding
problem through Mid-SWAP syndrome extraction and envelope-aware decoders.
Realizing this loop end to end places unusual demands on the software stack.
The simulator must carry a persistent per-shot loss state that suppresses
gates acting on lost atoms and re-randomizes them on reset; the measurement
record must preserve each `(loss flag, value bit)` pair; and the circuit,
seed, noise model, and decoder configuration behind a reported result must
remain auditable.

RustQEC is an open-source Rust workspace that implements this complete loop
as a reference pipeline. It generates native Mid-SWAP syndrome-extraction
circuits, samples them with the persistent-loss noise model while emitting
adjacent flag and value records, and publishes the results as hash-pinned,
schema-versioned public datasets. Its decoder compiles each circuit once into
loss envelopes and a matching graph, then decodes batches of shots — each
supplying only its syndrome and observed loss set — with an exact
envelope-MLE backend and a cached envelope-matching backend. Acceptance is
capability-checked against a published circuit-subset specification rather
than tied to the built-in generators, and the conformance suite proves this
with a Stim-generated circuit decoded shot-exactly against private answers.

The full loop runs behind one `rustqec` command whose machine-readable
capability metadata, structured error codes, and atomically published
artifacts are pinned against behavior by the test suite. Externally produced
datasets enter through the same validated contract, serving interactive use,
conventional scripting, workflow engines, and automated agents alike.

# Statement of need

Stim provides a high-performance foundation for stabilizer-circuit simulation
and detector error models [@Gidney_2021], while specialized decoders provide
efficient inference from detector events [@Higgott_2022]. In practice, a study
still needs contracts across circuit generation, sampling, binary row layouts,
decoder compilation, and evidence generation that ad hoc wrappers silently
violate — disagreeing about record ordering, observable conventions, or
padding bits, leaving interrupted batches unresumable, and blurring the line
between an infeasible decode and an all-zero prediction.

Heralded atom loss makes this interface problem scientifically important. A
realized loss can correspond to several mutually exclusive Pauli effects, and
the observed loss pattern changes the decoding problem from shot to shot. The
Pauli Envelope framework formalizes this relationship and introduces Mid-SWAP
syndrome extraction together with exact and matching-based decoders
[@Liu_2026]. A usable implementation must preserve each `(loss flag, value
bit)` pair, recover detector syndromes from the value bits, associate observed
losses with circuit locations, and compile the corresponding decoder state.

RustQEC targets QEC researchers who need inspectable command-line experiments,
Rust developers embedding QEC components, and automated systems that require a
stable machine-facing contract. Its purpose is not to replace every simulator
or decoder, but to provide an integrated path in which supported circuits,
datasets, predictions, statistics, and failure states validate and replay
together.

# State of the field

Stim is the reference point for RustQEC's circuit language and detector-model
workflow [@Gidney_2021]. RustQEC preserves familiar circuit and detector
concepts so that researchers do not need a new conceptual model, but its
`rstim` implementation and extended loss instructions are not presented as a
drop-in replacement for every Stim feature or performance regime.
Compatibility tests adapted from Stim retain source-level provenance, and
unsupported operations fail explicitly rather than being approximated
silently. The loss noise model is where the two deliberately diverge: Stim's
heralded Pauli channels record a herald bit and apply a Pauli error at the
herald site, whereas atom loss in RustQEC is a persistent per-shot state —
gates acting on a lost atom are suppressed, a reset clears the loss and
re-randomizes the qubit, and readouts emit a stable `(flag, value)` record
layout that a loss-aware decoder consumes directly. Stim's companion tool
sinter orchestrates large sampling campaigns and decoder statistics but
inherits the same record model and offers no per-shot loss-driven decoding.
On checked surface-code workloads, RustQEC's compiled sampling runs about
27–50x faster than the Stim command line while detector-error-model sampling
runs about 8x slower; both figures are published with their methodology,
environments, and claim limits on the project's benchmarked documentation
site rather than as broad performance parity claims.

PyMatching supplies a mature minimum-weight perfect-matching decoder
[@Higgott_2022], and mdopt demonstrates a code-agnostic tensor-network approach
to exact and approximate decoding [@Berezutskii_2025]. At a different layer,
tqec compiles high-level topological computations into detailed Stim circuits
[@Suau_2026]. RustQEC complements these projects by connecting circuit
processing, loss-visible dataset production, decoder compilation, and
reproducibility artifacts inside one Rust workspace. Its distinguishing focus
is the boundary between simulation and decoding: the same public dataset
contract carries the circuit, bit-packed measurements, hashes, and dimensions
needed for batched replay.

This integrated implementation is justified by cross-layer invariants that
cannot be enforced by adding an isolated decoder wrapper. Loss-record ordering,
persistent Mid-SWAP wire mappings, detector references, envelope compilation,
binary padding, atomic output publication, and structured failures must agree
across the simulator and decoder. RustQEC nevertheless reuses established
formats and records its Stim- and PyMatching-derived provenance rather than
claiming an independent replacement ecosystem.

# Software design

RustQEC organizes its central data path as a sequence from Stim-aligned circuit
inputs through simulation and loss-aware decoding to logical predictions and
versioned evidence. The unified `rustqec` command exposes this complete
workflow as eight discoverable commands covering circuit generation (including
native Mid-SWAP families), seeded measurement and detection-event sampling,
detector-error-model extraction, circuit statistics, public dataset export and
import, and loss-aware batch decoding. The full
loop from circuit generation to decoded predictions therefore runs behind one
machine-facing interface; crate boundaries separate circuit execution,
code construction, decoding, and evidence generation.

The simulation layer samples Stim-style circuits and exports structured or
bit-packed data. Native Mid-SWAP generation adds an alternating syndrome-
extraction schedule and a persistent logical-site-to-wire permutation. Loss-
visible `MRL` and `ML` instructions emit an adjacent flag and value for each
target. Detectors and logical observables reference value records, while the
flags retain the information required by a loss-aware decoder. An optional
per-shot trace records the exact noise realization — every Pauli branch and
loss onset — behind each exported shot.

The unified decoder reads a public dataset containing `manifest.json`,
`circuit.stim`, and `shots.b8`. It validates hashes, dimensions, row widths,
padding, and the supported circuit subset before publishing output. The circuit
is compiled once into measurement-to-detector relations, independent Pauli
effects, loss envelopes, and a matching graph. Each shot then supplies only its
syndrome and observed loss set. One backend uses an ILP model as an exact
small-scale correctness reference; the other updates and caches matching graphs
for repeated loss patterns. The `renvelope` crate holds reference decoders
for explicit, versioned envelope cases; the production backends
cross-validate against it in the test suite, so correctness claims trace to
checked reference cases rather than the optimized implementation alone.
Both backends are implementation choices behind one workflow, not
separate research products.

The command-line contract is the enforcement layer of this scientific
contract, and each of its properties exists to prevent a specific
reproducibility failure mode. Successful commands write stable JSON or packed
binary artifacts, while failures use documented error codes and nonzero
exits, so an infeasible decode or a timeout can never read as an all-zero
prediction. The `capabilities` command reports every verb's
arguments, input sources, output formats, artifacts, and error behavior in
JSON, and the test suite pins this document against the implementation so the
advertised contract cannot drift. Decode statistics record
the circuit hash, shot count, compile and decode time, distinct loss patterns,
cache reuse, timeouts, and infeasible shots. Prediction and statistics files
are published atomically so a partial run cannot resemble a completed
experiment.

The loss-aware compiler accepts a published, versioned circuit subset rather
than a fixed code family. Any flat circuit built from the documented
instruction set — loss-opportunity markers, Z-basis loss-visible readouts,
Hadamard and CNOT gates, resets, and Pauli noise channels — is admitted
provided its detector error model decomposes into graphlike components,
detectors carry coordinates, detectors and observables reference value records
rather than loss flags, and loss envelopes stay within declared resource
limits. Acceptance is therefore a capability check, not generator
recognition. The conformance suite decodes a distance-3 rotated surface-code
circuit generated by Stim and annotated independently of RustQEC's built-in
generators: the exact backend reproduces the private logical answers shot by
shot, and RustQEC's detector error model for the shared circuit matches
Stim's own extraction. Unsupported instructions, ambiguous graph mappings,
malformed loss-visible layouts, and excessive resource demands fail with
documented error codes, keeping the advertised behavior testable. Extending
the subset — further readout bases, repeat constructs, additional gates — is
future work versioned through the specification and does not require widening
the stable detector-error-model decoder interface.

# Research impact statement

RustQEC is used to build reproducible QEC experiments within this repository.
Tracked workflows compare multiple decoder implementations on shared surface-
code and bivariate-bicycle-code workloads. They publish fixed inputs, result
tables, figures, methodology notes, and claim limits. The loss-aware path
generates and decodes native distance-3 and distance-5 Mid-SWAP
datasets through the public CLI, and datasets produced outside the built-in
generators — including a Stim-generated surface-code circuit annotated by
independent tooling — enter through `dataset import` and decode through the
same validated contract. The blinded split and the private error trace give
learned decoders ground-truth training labels while keeping the public
evaluation fair. A companion study will report scientific
results from this workflow; its citation must be added before JOSS
submission.

Correctness evidence is layered rather than represented by a single benchmark.
Known-answer fixtures test circuit-to-detector and decoder behavior. Negative
controls reject malformed circuits, inconsistent manifests, nonzero padding,
missing files, timeouts, and infeasible models. Regression tests verify Mid-
SWAP schedule and record semantics, compare batched predictions with explicit
decoder kernels, and check that circuits compile once while repeated loss
patterns reuse state. The documentation site exposes runnable showcases and
checked artifacts, distinguishing implementation smoke tests from
publication-scale evidence.

A public issue and pull-request history, continuous integration,
an Apache-2.0 license, and reproducible build and test commands keep the
software inspectable by researchers who did not participate in its
development. Before submission, the authors will add explicit contribution and
support guidance, a tagged archive DOI, and the companion-paper citation. They
will report external research use without implying adoption that has not
occurred.

# AI usage disclosure

Generative-AI tools assisted with issue decomposition, code and documentation
drafting, test suggestions, code review, and language editing for this paper.
The authors reviewed all retained changes and remain responsible for the
software and manuscript. Behavioral claims were checked against executable
tests, fixed fixtures, cross-implementation comparisons where available, and
documented negative controls; AI-generated text or code was not accepted as
evidence of correctness.

# Acknowledgements

TODO: Add funding, institutional support, and contributor acknowledgements.

# References
