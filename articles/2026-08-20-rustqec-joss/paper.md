---
title: "RustQEC: A CLI-first, Stim-aligned Rust toolkit for reproducible atom-loss-aware quantum error correction"
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

Quantum error-correction studies commonly connect circuit construction,
stabilizer simulation, detector extraction, decoding, and statistical analysis.
Each stage is individually familiar, but moving data between them often relies
on experiment-specific scripts. This fragmentation makes it difficult to audit
which circuit, noise model, random seed, decoder configuration, and output
format produced a reported result. Atom loss adds a further challenge because
the location of a loss is useful side information that must remain associated
with the corresponding measurement record and decoder model.

RustQEC is an open-source Rust workspace for reproducible quantum
error-correction workflows. It combines Stim-aligned circuit processing,
simulation, detector-error-model tooling, code construction, decoder
experiments, and checked benchmark evidence behind command-line interfaces.
For neutral-atom studies, RustQEC generates native Mid-SWAP syndrome-extraction
circuits, records loss flags alongside measurement values, and decodes public
loss-visible datasets in batches. A unified `rustqec` command exposes
machine-readable capability metadata, structured errors, and versioned output
artifacts. These interfaces support interactive use, conventional scripting,
workflow engines, and agentic clients without defining separate automation
paths for each consumer.

# Statement of need

Stim provides a high-performance foundation for stabilizer-circuit simulation
and detector error models [@Gidney_2021], while specialized decoders provide
efficient inference from detector events [@Higgott_2022]. In practice, a study
still needs to preserve contracts across circuit generation, sampling, binary
row layouts, decoder compilation, and evidence generation. Ad hoc wrappers can
silently disagree about record ordering, observable conventions, padding bits,
or failure handling. They also make it hard to reproduce an interrupted batch
or to distinguish an infeasible decode from an all-zero prediction.

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
or decoder. It provides an integrated path in which supported circuits,
datasets, predictions, statistics, and failure states can be validated and
replayed together.

# State of the field

Stim is the reference point for RustQEC's circuit language and detector-model
workflow [@Gidney_2021]. RustQEC preserves familiar circuit and detector
concepts so that researchers do not need a new conceptual model, but its
`rstim` implementation and extended loss instructions are not presented as a
drop-in replacement for every Stim feature or performance regime. Compatibility
tests adapted from Stim retain source-level provenance, and unsupported
operations fail explicitly rather than being approximated silently.

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
workflow as eight discoverable commands: `circuit gen` generates built-in code
families including native Mid-SWAP circuits; `circuit sample` and
`circuit detect` produce seeded measurement and detection-event streams;
`circuit dem` extracts detector error models; `circuit stats` inspects circuit
structure; `dataset export` publishes a public decoder dataset together with a
private answer bundle; `dataset import` packages externally produced circuits
and shot records into validated public datasets; and `decode` runs loss-aware
batch decoding. The full
loop from circuit generation to decoded predictions therefore runs behind one
machine-facing interface, while crate boundaries separate circuit execution,
code construction, decoding, and evidence generation. This keeps the
user-facing interface cohesive without forcing unrelated algorithms into one
library API.

The simulation layer parses Stim-style circuits, samples measurements and
detector events, extracts detector error models, and exports structured or
bit-packed data. Native Mid-SWAP generation adds an alternating syndrome-
extraction schedule and a persistent logical-site-to-wire permutation. Loss-
visible `MRL` and `ML` instructions emit an adjacent flag and value for each
target. Detectors and logical observables reference value records, while the
flags retain the information required by a loss-aware decoder.

The unified decoder reads a public dataset containing `manifest.json`,
`circuit.stim`, and `shots.b8`. It validates hashes, dimensions, row widths,
padding, and the supported circuit subset before publishing output. The circuit
is compiled once into measurement-to-detector relations, independent Pauli
effects, loss envelopes, and a matching graph. Each shot then supplies only its
syndrome and observed loss set. One backend uses an ILP model as an exact
small-scale correctness reference; the other updates and caches matching graphs
for repeated loss patterns. The `renvelope` crate holds the reference decoders
for explicit, versioned envelope cases, and the production backends
cross-validate against it in the test suite, so correctness claims trace to
checked reference cases rather than to the optimized implementation alone.
Both backends are implementation choices behind the same workflow, not
separate user-facing research products.

CLI-first design is part of the scientific contract. Successful commands write
stable JSON or packed binary artifacts, while failures use documented error
codes and nonzero exits. The `capabilities` command reports every verb's
argument list, input sources, output formats, artifacts, and error behavior in
JSON, and the test suite pins this document against the implementation so the
advertised contract cannot drift from behavior. Decode statistics record
the circuit hash, shot count, compile and decode time, distinct loss patterns,
cache reuse, timeouts, and infeasible shots. Prediction and statistics files
are published atomically so that a partial run cannot resemble a completed
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
tables, figures, methodology notes, and claim limits. The loss-aware path is
also used to generate and decode native distance-3 and distance-5 Mid-SWAP
datasets through the public CLI, and datasets produced outside the built-in
generators — including a Stim-generated surface-code circuit annotated by
independent tooling — enter through `dataset import` and decode through the
same validated contract. A companion study will report scientific
results produced with this workflow; its citation must be added before JOSS
submission.

Correctness evidence is layered rather than represented by a single benchmark.
Known-answer fixtures test circuit-to-detector and decoder behavior. Negative
controls reject malformed circuits, inconsistent manifests, nonzero padding,
missing files, timeouts, and infeasible models. Regression tests verify Mid-
SWAP schedule and record semantics, compare batched predictions with explicit
decoder kernels, and check that circuits compile once while repeated loss
patterns reuse state. The documentation site exposes runnable showcases and
checked artifacts so a reader can distinguish implementation smoke tests from
publication-scale evidence.

The project has a public issue and pull-request history, continuous integration,
an Apache-2.0 license, and reproducible build and test commands. These practices
make the software inspectable by researchers who did not participate in its
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
