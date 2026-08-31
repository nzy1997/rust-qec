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

Neutral-atom quantum computers lose atoms during operation. A heralded
loss — one the experiment detects and localizes — is valuable side
information: the Pauli Envelope framework [@Liu_2026] turns each observed
per-shot loss pattern into a well-defined maximum-likelihood decoding
problem through Mid-SWAP syndrome extraction, an atom-replenishing
extraction schedule, and envelope-aware decoders. Turning this framework
into a reproducible study places unusual demands on the software stack: the
simulator must carry a persistent per-shot loss state that suppresses gates
on lost atoms and re-randomizes them on reset, the measurement record must
preserve each `(loss flag, value bit)` pair, and the circuit, seed, noise
model, and decoder configuration behind a result must remain auditable.

RustQEC is an open-source Rust workspace that implements this complete loop
as a reference pipeline. It generates native Mid-SWAP syndrome-extraction
circuits, samples them under the persistent-loss noise model with adjacent
flag and value records, and publishes hash-pinned, schema-versioned public
datasets; optional per-shot error traces and blinded logical-input markers
expose shot-aligned ground truth for training learned decoders without
leaking it into public evaluation. The decoder compiles each circuit once
into loss envelopes and a matching graph, then decodes batches of shots —
each supplying only its measurement record and observed loss flags — with an
exact envelope-MLE backend and a matching-based backend that caches compiled
graphs for repeated loss patterns. Decoder acceptance is validated against a
published circuit-subset specification, and a conformance suite decodes a
Stim-generated circuit while reproducing its private answers shot for shot.

The full loop runs behind one `rustqec` command with machine-readable
capability metadata, structured error codes, and atomically published
artifacts; external datasets enter through the same validated contract,
serving interactive use, scripting, workflow engines, and automated agents
alike.

# Statement of need

Stim provides a high-performance foundation for stabilizer-circuit simulation
and detector error models [@Gidney_2021], while specialized decoders provide
efficient inference from detector events [@Higgott_2022]. A reproducible
study, however, needs more than fast components: contracts across circuit
generation, sampling, the packed layout of shot records, decoder compilation,
and per-shot evidence of which errors produced each shot. Ad hoc wrappers
silently violate these contracts — disagreeing about record ordering,
observable conventions, or record encoding, and blurring the line between an
infeasible decode and an all-zero prediction.

Heralded atom loss makes this interface problem scientifically important.
Because the onset of a loss is not itself heralded, a realized loss is
consistent with several mutually exclusive Pauli effects, and the observed
loss pattern changes the decoding problem from shot to shot. The Pauli
Envelope framework formalizes this relationship and introduces Mid-SWAP
syndrome extraction together with exact and matching-based decoders
[@Liu_2026]. Yet general-purpose tools do not close this simulate-to-decode
loop: the simulator must export loss-visible records in a layout the decoder
trusts, and the decoder must learn which loss pattern each shot realized,
under shared semantics. To our knowledge, no existing stack couples a
persistent-loss simulator to a loss-aware decoder under one auditable
contract.

RustQEC targets QEC researchers who need inspectable command-line experiments,
Rust developers embedding QEC components, and automated systems that require a
stable machine-facing contract. Its purpose is not to replace every simulator
or decoder, but to provide one integrated path in which circuits, datasets,
predictions, and statistics validate and replay together, and failures —
timeouts, infeasible decodes — surface as explicit recorded errors rather
than silent predictions.

# State of the field

Stim is the reference point for RustQEC's circuit language and detector-model
workflow [@Gidney_2021]. RustQEC preserves familiar circuit and detector
concepts, but its `rstim` implementation and extended loss instructions are
not presented as a drop-in replacement for every Stim feature or performance
regime; unsupported operations fail explicitly rather than being silently
approximated. The loss noise model is where the two deliberately diverge.
Stim's heralded Pauli channels record a herald bit and apply a sampled Pauli
— possibly identity — at the herald site. Atom loss in RustQEC is instead a
persistent per-shot state — suppressed gates, reset-triggered
re-randomization, and a stable `(flag, value)` readout layout that a
loss-aware decoder consumes directly.

Stim's companion tool sinter orchestrates large sampling campaigns but
inherits the same record model and offers no per-shot loss-driven decoding.
Nor is performance the differentiator: on the checked measurement-sampling
workloads (distance-11 surface code, distance-13 repetition code), RustQEC's
compiled sampling recorded 27–50x lower wall time than the Stim command
line, detection-event sampling showed a smaller 7.75x advantage, and
detector-error-model sampling ran about 8x slower. These report-only CLI
comparisons include process startup; a more conservative 3.60x
precompiled-to-precompiled figure, with methodology and claim limits, is
published on the [documentation
site](https://nzy1997.github.io/rust-qec/simulator/).

PyMatching supplies a mature minimum-weight perfect-matching decoder
[@Higgott_2022], mdopt demonstrates a code-agnostic tensor-network approach
to exact and approximate decoding [@Berezutskii_2025], and tqec compiles
high-level topological computations into detailed Stim circuits
[@Suau_2026]. RustQEC complements these projects by connecting circuit
processing, loss-visible dataset production, decoder compilation, and
reproducibility artifacts inside one workspace; its distinguishing focus is
the simulation-to-decoding boundary, where one public dataset contract
carries everything batched replay needs: circuit, bit-packed measurements,
hashes, and dimensions. This integration is justified by cross-layer
invariants — record ordering, wire mappings, detector references, and
failure semantics must agree across simulator and decoder — but RustQEC
reuses established formats and records its Stim- and PyMatching-derived
provenance rather than claiming an independent ecosystem.

# Software design

The unified `rustqec` command exposes the complete workflow as eight
discoverable commands covering circuit generation (including native Mid-SWAP
families), seeded measurement and detection-event sampling, detector-error-
model extraction, circuit statistics, public dataset export and import, and
loss-aware batch decoding. Crate boundaries separate circuit execution,
code construction, decoding, and evidence generation.

The simulation layer samples Stim-style circuits and exports structured or
bit-packed data. Native Mid-SWAP generation adds an alternating syndrome-
extraction schedule and a persistent logical-site-to-wire permutation. Loss-
visible `MRL` and `ML` instructions emit an adjacent flag and value for each
target. Detectors and logical observables reference value records, while the
flags retain the information required by a loss-aware decoder. Two optional
per-shot traces expose what the simulator realized. `rstim detect
--trace_out` streams a versioned JSONL record of every noise branch,
measurement event with its loss cause, and detector flip, shot-aligned with
the bit-packed outputs and published transactionally with them; a
test-pinned, standard-library Python loader turns the files directly into
neural-decoder training inputs. The dataset flow keeps the same noise
realization as a private trace sidecar, separating public evaluation shots
from private ground truth.

The decoder reads a public dataset — `manifest.json`, `circuit.stim`,
`shots.b8` — validating hashes, dimensions, row widths, padding, and the
supported circuit subset before publishing output. The circuit is compiled
once into measurement-to-detector relations, independent Pauli effects, loss
envelopes, and a matching graph. Each shot then supplies only its measurement
record and observed loss flags. One backend solves each shot as an integer
linear program for exact maximum-likelihood predictions at small scale; the
other conditions and caches a matching graph per observed loss pattern. Both
backends cross-validate against the `renvelope` reference decoders in the
test suite, so correctness claims trace to checked reference cases, not
optimized implementations alone.

The command-line contract enforces these contracts. Successful commands
write stable JSON reports alongside versioned artifact files (packed binary
or Stim-format text), while failures use documented error codes and nonzero
exits, so an infeasible decode or a timeout can never read as an all-zero
prediction. The `capabilities` command reports every verb's arguments,
inputs, outputs, artifacts, and error behavior in JSON, pinned by the test
suite so the advertised contract cannot drift. Decode statistics record
circuit hash, shot count, compile and decode time, distinct loss patterns,
cache reuse, timeouts, and infeasible shots; all artifacts are published
atomically, so a partial run cannot resemble a completed experiment.

The loss-aware compiler accepts a published, versioned circuit subset rather
than a fixed code family. Any flat circuit built from the documented
instruction set — loss-opportunity markers, Z-basis loss-visible readouts,
Hadamard and CNOT gates, resets, `X_ERROR`, and one- and two-qubit
depolarizing channels — is admitted provided it passes the published
capability checks: graphlike detector-error-model decomposition (each error
mechanism triggers at most two detectors), detector coordinates,
value-record references, and envelope resource limits. Acceptance is
therefore a capability check, not generator recognition. The conformance
suite decodes a distance-3 rotated surface-code circuit generated by Stim
and annotated into the loss-visible subset by a dedicated in-repository
tool: the exact backend reproduces the private logical answers shot by
shot, and RustQEC's detector error model for the shared circuit matches
Stim's own extraction. Extensions — further readout bases, repeat
constructs, additional gates — are future work versioned through the
specification, without widening the stable decoder interface.

# Research impact statement

RustQEC is used to build reproducible QEC experiments within this repository.
Tracked workflows compare multiple decoder implementations on the same
surface-code and bivariate-bicycle-code [@Bravyi_2024] workloads, publishing
fixed inputs, result tables, figures, methodology notes, and claim limits.
The loss-aware path decodes native distance-3 and distance-5 Mid-SWAP
datasets through the public CLI; external datasets — including the
Stim-generated conformance circuit described in the software-design section
— enter through `dataset import` and decode through the same contract.

The blinded split gives learned decoders ground-truth training labels while
keeping the public evaluation fair: a canonical circuit marker, placed before
any noise instruction, injects a known logical input that is recorded only in
private per-shot metadata. Together with the private error trace, this
metadata yields shot-aligned input–label pairs for neural-decoder training on
the exported public batches. A companion study will report scientific results
from this workflow. [TODO: add the companion-study citation before
submission.]

Correctness evidence is layered rather than represented by a single benchmark:
known-answer fixtures test circuit-to-detector and decoder behavior, negative
controls reject malformed inputs and failure states, and regression tests
compare compiled batched paths against explicit reference implementations and
check that repeated loss patterns reuse compiled state. The documentation
site exposes runnable showcases and checked artifacts, distinguishing
implementation smoke tests from publication-scale evidence.

Public issue and pull-request history, continuous integration,
an Apache-2.0 license, and reproducible build and test commands keep the
software inspectable to researchers who did not participate in its
development.

# AI usage disclosure

Generative AI tools (large-language-model coding assistants) assisted with
issue decomposition, code and documentation drafting, test suggestions, code
review, and language editing for this paper. The authors reviewed all
retained changes and remain responsible for the software and the manuscript.
Behavioral claims in this paper are backed by executable tests, fixed
fixtures, cross-implementation comparisons where available, and documented
negative controls; AI-generated text or code was not accepted as evidence of
correctness.

# Acknowledgements

TODO: Add funding, institutional support, and contributor acknowledgements.

# References
