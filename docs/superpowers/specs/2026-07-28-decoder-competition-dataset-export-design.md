# Decoder Competition Dataset Export

Date: 2026-07-28
Status: approved in design discussion

## Goal

Add a competition-oriented dataset exporter that lets an organizer publish one
of two decoder inputs without publishing the per-shot logical answers:

1. the public circuit plus detector events; or
2. the public circuit plus all QEC measurement results, with the prepared
   logical state independently blinded on every shot.

The threat model includes contestants who inspect the implementation, reverse
engineer every public file, and recompute any quantity derivable from the
public circuit and samples. Security must therefore come from excluding the
answer in detector mode and from a private random logical mask in measurement
mode, not from an undocumented encoding.

## Scope and non-goals

The first version supports memory circuits with exactly one observable and an
explicit physical support for logical `X_L`. It covers the repository's
repetition-code and surface-code memory circuits.

The first version does not support multiple observables, sweep bits, arbitrary
automatic discovery of logical operators, or arbitrary experimental samples
that were acquired without logical-state randomization. It does not define a
new RSMP version and does not weaken or change RSMP v1 semantics.

RSMP v1 remains a lossless, circuit-bound private archive. A public competition
bundle is deliberately not an RSMP archive because an RSMP reader can recover
the complete measurement record and its observable values.

## Selected architecture

Add one independent CLI command, `export_decoder_dataset`. It samples a circuit
and publishes two separate directory bundles:

- a public bundle for contestants; and
- a private bundle for the evaluator.

The exporter reuses the existing circuit parser, sampler, measurement-to-
detection conversion, and `b8` writers. Competition policy, logical blinding,
manifest generation, validation, and directory publication live behind a
separate module boundary. `pack_samples` and `unpack_samples` remain unchanged.

The two export modes are `detectors` and `measurements_blinded`. They are
separate dataset generations by default and do not share public shot IDs or a
publicly recoverable row permutation.

## Public and private bundle contract

The public directory contains exactly:

```text
manifest.json
circuit.stim
shots.b8
```

The public manifest records only the bundle format version, dataset ID, mode,
shot count, row width and meaning, measurement/detector/observable counts,
`b8` bit ordering, and a digest of `circuit.stim`. It must not contain the RNG
seed, masks, answers, private paths, producer-circuit text, per-shot source
state, or row permutation.

The private directory contains:

```text
manifest.json
answers.b8
```

and, only for `measurements_blinded`:

```text
masks.b8
```

`answers.b8` and `masks.b8` have one bit per shot because the first version
requires exactly one observable. The private manifest repeats the public
dataset ID, records private artifact digests and generation metadata, and may
record an explicitly supplied seed. The dataset ID is the SHA-256 digest of a
canonical encoding of the public schema version, mode, circuit digest, shot
count, row width, and `shots.b8` digest. The manifest and dataset ID itself are
excluded from that encoding, avoiding a circular definition. This identifies
the corresponding private bundle without encoding a secret.

All binary shot files use the existing `b8` convention: rows are dense and
bits are least-significant-bit first within each byte. Unused high padding bits
in the last byte of every row are zero.

## Detector mode

The organizer invokes:

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --mode detectors \
  --public_out public-data \
  --private_out private-truth
```

The fixed logical-zero input circuit is sampled once, producing detector bits
`D` and observable bits `O` for each shot. The exporter writes `D` to public
`shots.b8` and writes `O` to private `answers.b8`. It publishes the Stim circuit
instead of a precomputed DEM; contestants may derive the DEM themselves.

This mode does not generate or apply a random logical mask. Knowing the circuit,
its `OBSERVABLE_INCLUDE` declarations, and the detector rows does not reveal
the missing per-shot measurement records. If a particular detector pattern
mathematically determines a logical answer, a decoder is expected to exploit
that fact; it is successful decoding rather than answer leakage.

The public bundle must not contain measurement rows, observable rows, a seed
that reproduces the published shots, or identifiers that align the rows with a
separately published measurement dataset.

## Blinded-measurement mode

The input circuit prepares logical zero and contains one standalone, top-level
marker after logical initialization and before the first measurement:

```stim
R 0 1 2
TICK[rstim:logical_flip_point]
```

The organizer invokes:

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --mode measurements_blinded \
  --logical_x_qubits 0,2,4 \
  --public_out public-data \
  --private_out private-truth
```

The qubit list is non-empty, contains no duplicates, and names valid qubits.
The exporter constructs a private logical-one producer circuit by inserting
one ideal `X` instruction on that support immediately after the marker. The
public `circuit.stim` remains the unmodified logical-zero analysis circuit.
Knowledge of the marker or the logical operator support is not treated as a
secret.

For every shot, the exporter generates an independent uniform private bit
`b`. Shots with `b=0` are sampled from the public logical-zero circuit; shots
with `b=1` are sampled from the private circuit containing the ideal `X_L`.
The two groups are merged according to a private random permutation. Only the
QEC measurement record `m` is published. No auxiliary measurement, mask bit,
producer-circuit label, or pre-permutation row number is public.

All detector and observable interpretation uses the public logical-zero
circuit and its reference sample:

```text
public_detector   = D_public(m)
public_observable = O_public(m)
answer            = public_observable XOR b
```

Thus a contestant can reconstruct the public observable, but that quantity is
the logical error XOR a uniform private bit. The useful syndrome and raw
stabilizer information remain present, so the intended task is still to infer
the logical error from error evidence. The masking does not claim that the
answer is statistically independent of all public features; detector-answer
correlation is the signal a decoder is supposed to use.

## Logical-operator validation

`measurements_blinded` is rejected unless the tagged `TICK` marker appears
exactly once at top level. A marker inside a `REPEAT` block is invalid. The
injected ideal `X_L` must execute once and must not be erased by later logical
initialization.

Before noisy sampling, the exporter obtains noiseless/reference measurement
records for both producer circuits and interprets both through the public
circuit's measurement-to-detection transform. Validation succeeds only when:

```text
D_public(m_ref_0) = D_public(m_ref_1)
O_public(m_ref_0) XOR O_public(m_ref_1) = 1
```

For a valid memory circuit both detector vectors are normally zero. The first
condition is stated as equality so validation remains about preservation of
the syndrome reference; the second requires exactly the sole observable to
flip. This catches a marker placed before a reset, an incomplete logical
support, and a physical operation that creates a syndrome.

## Randomness and reproducibility

Without `--seed`, generation uses the operating system random source. With
`--seed`, generation is deterministic for tests and private reproducibility.
Distinct domain-separated RNG streams control physical sampling, logical mask
generation, and row permutation so that exposing one stream's behavior does
not directly expose another. Neither the master seed nor derived seeds appear
in the public bundle.

The security requirement is that `b` is uniform, independent of simulated
physical noise, and unavailable to contestants. It must not influence public
metadata, row sizes, circuit selection labels, or ordering except through the
intended logical flip in the measurement record.

## CLI preflight and errors

The command requires a positive `--shots`, a filesystem circuit input, and two
new output-directory paths. It does not support stdin or stdout in the first
version.

Both modes reject circuits with zero or multiple observables, circuits with
sweep bits, and circuits for which a stable reference sample cannot be built.
The detector mode rejects `--logical_x_qubits`. The blinded-measurement mode
requires the qubit list and all marker and logical-operator checks described
above.

The public and private outputs must be different, non-existing directories.
After resolving their existing parents, neither output may contain the other
or alias the same destination. Preflight completes before sampling or creating
staging directories. Diagnostics identify the violated invariant without
printing seeds, masks, answers, or private sample contents.

## Publication semantics

Each bundle is built in a sibling temporary directory. All files are closed,
validated, and hashed before either final directory is visible. The private
directory is renamed into place first; the public directory is renamed last
and is the commit point. Therefore a visible public bundle always has a fully
published private counterpart. If public publication fails, the already
published private directory is retained and named in the error so the
organizer can recover or remove it deliberately.

This is atomic publication per directory, not a claim of a filesystem-wide
transaction across two paths. Temporary directories that were not published
are removed through scoped cleanup. On Unix, the private directory is created
with owner-only permissions before files are written.

## Testing

### Correctness

- Detector public rows and private answers equal the detector and observable
  outputs returned together by the standard sampling pipeline when driven by
  the exporter's physical-sampling RNG stream.
- Blinded measurement rows satisfy
  `answer = O_public(measurement) XOR mask` for every shot.
- A fixed seed reproduces all public and private bytes.
- Repetition-code and surface-code memory fixtures each run end to end in both
  modes.

### Logical-blinding properties

For paired executions with the same injected physical error and opposite mask
bits, tests require:

```text
D(m0) = D(m1)
O(m0) XOR O(m1) = 1
answer(m0) = answer(m1)
```

An adversarial regression recomputes observables using only public circuit and
measurement files and confirms that the recomputed value differs from the
private answer exactly where the private mask is one. A deterministic seed
fixture contains both mask values without relying on a flaky statistical test.

### Rejection coverage

Tests cover missing, duplicate, nested, and misplaced markers; empty,
duplicate, or out-of-range logical-qubit lists; logical supports that change a
detector or fail to flip the observable; multiple observables; sweep bits;
zero shots; invalid mode-specific options; and colliding, nested, aliased, or
pre-existing output directories.

### Publication and leakage coverage

- Public directories contain exactly the three allowed files.
- Public JSON is recursively checked for seed, mask, answer, private path,
  producer variant, and permutation fields.
- Fault injection before each rename proves that no partial public bundle is
  left behind and that a retained private bundle is reported.
- Non-byte-aligned widths prove that padding bits are zero.
- Existing RSMP compatibility, CLI, and full workspace tests continue to pass.

## Acceptance criteria

The feature is complete when both modes generate documented bundles for the
repetition and surface memory fixtures; the evaluator can score predictions
using only private `answers.b8`; the blinded mode passes the paired logical
invariance and adversarial-recomputation tests; malformed inputs fail before
public output appears; and all existing repository tests remain green.
