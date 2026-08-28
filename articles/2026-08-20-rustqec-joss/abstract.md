# Abstract (submission-form text; JOSS renders the Summary section in the paper)

Neutral-atom quantum computers lose atoms during operation, and a heralded
loss is valuable decoding side information. RustQEC is an open-source Rust
workspace that implements the full atom-loss-aware quantum error correction
loop as a reference pipeline: it generates native Mid-SWAP
syndrome-extraction circuits, samples them under a persistent per-shot loss
noise model with loss-visible measurement records, publishes hash-pinned,
schema-versioned public datasets, and decodes shot batches with exact
envelope-MLE and cached matching-based backends. The pipeline runs behind a
single command-line interface with machine-readable capability metadata,
structured error codes, and atomically published artifacts; optional per-shot
error traces and blinded logical-input markers provide shot-aligned ground
truth for training learned decoders without compromising public evaluation.
Decoder acceptance is validated against a published circuit-subset
specification, including a Stim-generated conformance circuit decoded
shot-for-shot against private answers. RustQEC targets QEC researchers, Rust
developers, and automated agents that need auditable, replayable experiments.
