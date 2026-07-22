# Circuit-Bound Sample Archive Design

Date: 2026-07-22
Status: approved in design review

## Need

`rstim` can already sample all circuit measurement results, write Stim-compatible
result formats, and convert measurements into detector events with `m2d`. The
existing result formats are intentionally simple interchange formats. They do
not bind data to a circuit, carry dimensions and version metadata, localize
corruption, or exploit the circuit's detector constraints to reduce storage.

This feature adds a v1, production-robust archive for losslessly storing only
the measurement results needed by the decoding data path. Decompression takes
the original circuit, recovers every measurement bit, and derives detector
events and logical observable flips from the recovered measurements.

## Goals

- Losslessly recover every measurement result for every shot.
- Recover detectors and logical observable flips from the supplied circuit.
- Compress substantially better than applying a general compressor directly
  to dense measurement bytes on representative low-noise QEC circuits.
- Process independent shot blocks sequentially with bounded working memory.
- Bind an archive to one normalized circuit and reject a different circuit.
- Detect unsupported versions, malformed structures, truncation, corruption,
  reordered blocks, resource-exhaustion attempts, and trailing data.
- Expose library APIs independently from the CLI.
- Reuse existing `rstim` result readers/writers and `m2d` semantics where their
  contracts apply.

## Non-goals

- DEM-only decompression.
- Circuits with sweep bits in v1.
- Saving internal noise branches, simulator state, visualization traces, or
  detector-error-model samples.
- Lossy compression.
- Random access by shot number.
- Salvaging or skipping corrupt blocks.
- Direct `rstim sample --out_format rsmp` integration in v1.
- A fixed wall-clock performance gate across heterogeneous CI machines.
- Treating semantically equivalent but structurally rewritten circuits as the
  same archive identity.

## Existing foundation

- `rstim sample` already uses `SampleOutputMode::MeasurementsOnly` and returns a
  `BitTable` of all measurement results.
- `rstim/src/output.rs` implements `01`, `b8`, `r8`, `hits`, and `ptb64`.
- `rstim/src/m2d.rs` converts a measurement table plus a circuit into detector
  and logical-observable tables.
- `BitTable` stores one row per measurement or detector and packs shots into
  `u64` words, matching the proposed block transform well.
- The workspace already depends on `sha2`.
- The checked d11/r100 surface-code fixture has 12,121 measurements, 12,000
  detectors, one observable, and no sweep bits.

Stim's documented result formats deliberately cover dense versus sparse and
human-readable versus binary use cases. `ptb64` requires the shot count, and
`r8` requires the number of bits per shot; `r8` is intended for sparse data such
as detector events. See
[Stim result formats](https://github.com/quantumlib/Stim/blob/main/doc/result_formats.md).
The new archive does not replace these interchange formats. It wraps a
circuit-derived reversible transform in a versioned, integrity-checked
container.

Zstandard is the general compression layer. Its frames are independently
decodable, streamable with bounded intermediate storage, and can carry content
sizes and checksums. See the
[Zstandard format specification](https://github.com/facebook/zstd/blob/dev/doc/zstd_compression_format.md)
and the [`zstd` Rust crate](https://docs.rs/zstd/latest/zstd/).

## Approaches considered

### 1. Circuit-derived reversible transform plus adaptive blocks and Zstandard

Transform each measurement vector into independent detector values plus the
free measurement coordinates left by detector constraints. Encode sparse and
dense streams separately, then apply Zstandard per stream and per block.

This is the selected approach. It is lossless, exploits QEC structure, supports
bounded streaming, and keeps a dense fallback for high-entropy inputs.

### 2. Dense `b8` or `ptb64` followed by Zstandard

This is simpler but does not exploit circuit constraints. Random-looking
intermediate measurements limit compression even when detector events are
sparse.

### 3. Circuit-derived transform with only a custom sparse codec

This avoids a new compression dependency but reinvents more entropy-coding
behavior and produced a weaker sizing estimate than the selected layered
approach.

## Mathematical representation

For a circuit with `M` measurements, let:

- `m` be one raw measurement vector in `GF(2)^M`;
- `r` be the circuit's noiseless reference measurement vector;
- `x = m XOR r` be the measurement-flip vector;
- `H` be the `D x M` detector parity matrix derived from `DETECTOR rec[-k]`;
- `G` be the `L x M` observable parity matrix derived from
  `OBSERVABLE_INCLUDE`.

Each detector or observable target is resolved to an absolute measurement
index at its point in the expanded circuit. Repeated references cancel by XOR.
Invalid record references are rejected using the same semantic rules as the
measurement-to-detection path.

Let `R = rank(H)`. Perform deterministic GF(2) elimination in original detector
order. At each row, use the highest measurement index as the pivot and reduce
against existing pivot rows. Record:

- the `R` original detector row indices selected as an independent set;
- the row operations needed to obtain echelon equations;
- the `R` pivot measurement columns;
- the `M-R` non-pivot, or free, measurement columns.

For each shot the archive stores:

1. the values of the selected original detectors, in selected-row order;
2. the values of `x` at the free measurement columns.

The selected original detector values are used instead of transformed detector
combinations so low-noise sparsity is preserved. During decoding, replay the
recorded row operations on their right-hand sides, then solve pivot variables
from lower to higher pivot index. The `R` independent equations plus `M-R`
free values uniquely recover all `M` bits of `x`.

After reconstruction:

```text
measurements = x XOR r
detectors    = H * x
observables  = G * x
```

The decoder computes all detector rows, including dependent rows not stored in
the archive. The transform also works when `R=0`, `D=0`, or `R=M`.

The reference strategy is `SimulateNoiseless`. It is part of the transform
version, not inferred from how the input measurements were produced. XOR with
the deterministic reference is reversible for arbitrary measurement input.

## Circuit identity and supported circuit subset

Parse the supplied Stim-like circuit and serialize it with
`circuit_to_string`. Compute SHA-256 over the resulting UTF-8 bytes. Comments
and insignificant whitespace therefore do not affect identity. Gate changes,
arguments, target changes, operation ordering, and repeat structure do affect
identity. Semantically equivalent loop folding or algebraic rewrites are not
normalized in v1.

The header also records `M`, `D`, `L`, `rank(H)`, the canonicalization scheme,
reference strategy, and transform algorithm version. A decoder validates all
of them before reading block payloads.

Reject circuits where `num_sweep_bits > 0` with
`RSMP_UNSUPPORTED_SWEEP`. Sweep values are per-shot external inputs and are not
present in a measurement-only archive, so v1 cannot reliably reconstruct the
reference behavior for such circuits.

## Container format

The format name is `rsmp`. Multi-byte integers are unsigned little-endian.
Every length and count uses checked conversion and checked arithmetic.

### Global header

The fixed prefix contains:

- 8-byte magic `RSTMSMP\0`;
- `format_major: u16`, initially `1`;
- `format_minor: u16`, initially `0`;
- `header_len: u32`;
- required feature flags and reserved flags;
- canonicalization, fingerprint, transform, reference, and codec-suite IDs;
- configured maximum shots per block, default `4096`;
- `M`, `D`, `L`, `rank(H)`, and total shots as `u64`;
- the 32-byte circuit SHA-256;
- the 32-byte SHA-256 of the header bytes excluding this digest field.

Reserved bits must be zero. Unknown major versions and unknown required flags
are rejected. `header_len` permits future optional minor-version fields, but a
v1.0 reader only skips fields explicitly marked optional by a compatible flag.

### Block layout

Blocks are sequential and have no random-access index. A block header contains:

- block magic and format;
- zero-based block sequence number;
- first shot index;
- shot count;
- syndrome and free-stream codec IDs;
- declared uncompressed and compressed lengths for each stream;
- the SHA-256 of the canonical uncompressed logical payload.

The header is followed by exactly one syndrome Zstandard frame and one free-bit
Zstandard frame. Each frame has a declared content size and content checksum.
Empty-dimension streams have a canonical zero-length representation defined by
the codec suite.

The canonical logical payload used by the block SHA-256 is independent of the
selected pre-codec: dense selected-detector bits followed by dense free bits,
both in the bit order defined below and both with zero final padding. A reader
reconstructs these canonical bytes before checking the digest. Changing a
sparse/dense encoding without changing logical values therefore does not change
the block digest.

Default blocks contain at most 4096 shots. The last block can be shorter. The
writer may accept a bounded alternative block size, but records it in the
global header and enforces it consistently.

### Syndrome pre-codec

For `R` selected detector values and `S` shots, form both raw-size candidates:

- `dense`: concatenate bits in `(shot, selected_detector)` order, least
  significant bit first, with no per-shot byte padding;
- `sparse`: for each shot write a canonical shortest-form unsigned LEB128 hit
  count, followed by canonical unsigned LEB128 deltas between strictly
  increasing selected-detector indices. The first delta is the first index;
  later deltas are `current - previous - 1`.

Choose the smaller raw representation; ties select dense. Compress only the
chosen representation with the configured Zstandard level. Codec IDs make the
choice explicit per block.

### Free-bit codec

Concatenate free bits in `(shot, free_measurement)` order, least significant
bit first, with no per-shot byte padding. Compress the stream as its own
Zstandard frame. High-entropy data is allowed to remain close to a raw
Zstandard block; compressibility is not assumed.

All unused final padding bits must be zero and are checked by the reader.

### Trailer

The trailer contains:

- trailer magic and version;
- block count;
- total shot count;
- SHA-256 over the global header, all block bytes, and the trailer prefix before
  the digest.

The reader rejects a missing trailer, count disagreement, digest mismatch, or
any bytes after the trailer.

## Streaming and memory behavior

The writer consumes at most one input block at a time, transforms it, writes
the two frames, and releases block storage. The reader validates and
reconstructs one block, passes it to the requested output writers, and releases
it before reading the next block.

The format contains independent frames for corruption localization and bounded
decoding, but v1 intentionally has no shot-range index and guarantees only
sequential reads.

## Library modules

### `measurement_transform`

Responsibilities:

- derive `r`, `H`, `G`, independent detector rows, elimination operations,
  pivots, and free columns from a circuit;
- expose dimensions and circuit-derived transform identity;
- encode one measurement block into selected-detector and free-bit tables;
- reconstruct measurements and derive full detectors and observables;
- verify that reconstructed measurements reproduce the stored independent
  detector values.

Suggested interface:

```text
MeasurementTransform::from_circuit(...)
MeasurementTransform::encode_block(...)
MeasurementTransform::decode_block(...)
```

### `sample_archive`

Responsibilities:

- encode and decode the binary container;
- select dense versus sparse syndrome representation;
- manage Zstandard frames;
- enforce versions, structural checks, checksums, and resource limits;
- provide typed, stable error codes.

Suggested interfaces:

```text
SampleArchiveWriter::new(...)
SampleArchiveWriter::write_block(...)
SampleArchiveWriter::finish(...)

SampleArchiveReader::open(...)
SampleArchiveReader::next_block(...)
SampleArchiveReader::finish(...)
```

The decoded block is:

```text
DecodedSampleBlock {
    measurements,
    detections,
    observable_flips,
}
```

### CLI integration

Add two commands instead of extending generic `convert`:

```sh
rstim pack_samples \
  --circuit circuit.stim \
  --shots 100000 \
  --in measurements.b8 \
  --in_format b8 \
  --out samples.rsmp
```

```sh
rstim unpack_samples \
  --circuit circuit.stim \
  --in samples.rsmp \
  --measurements_out recovered.b8 \
  --measurements_out_format b8 \
  --detectors_out detectors.b8 \
  --detectors_out_format b8 \
  --obs_out observables.b8 \
  --obs_out_format b8
```

`pack_samples` requires `--shots`, allowing the global header to be written
before payload data and making short or extra input an error. The initial
streaming input formats are `b8`, `ptb64`, and `01`. Binary inputs reject
non-zero unused padding. `r8` and `hits` measurement inputs are deferred because
they target sparse records, while measurement records are often high entropy.

`unpack_samples` requires the circuit and at least one requested output.
Measurement, detector, and observable outputs reuse existing result writers.
It reconstructs measurements internally even when only detector output is
requested, but does not retain the whole archive.

`unpack_samples --verify_only` performs a full read, reconstruction, and
integrity check without writing result data. On success it prints one stable,
reviewer-friendly line beginning with `PASS rsmp` and including version, shots,
blocks, `M/D/L`, and circuit hash prefix.

Direct `sample --out_format rsmp` is deferred until the archive contract is
stable. Existing `sample --out_format b8` can feed `pack_samples` through a file
or pipe.

## Error and integrity contract

Decoding is fail-closed. It does not skip or salvage corrupt blocks.

Validation order:

1. Validate magic, version, flags, and header digest.
2. Validate all dimensions and lengths with checked arithmetic and
   `ArchiveLimits` before allocation.
3. Parse the circuit, reject sweep, and compare circuit hash, dimensions, rank,
   and transform identifiers.
4. Require consecutive block sequence numbers, first-shot indices, and bounded
   shot counts.
5. Validate stream lengths, codec IDs, canonical varints, sorted in-range hit
   indices, and zero padding.
6. Bound the Zstandard window and require exactly the declared decompressed
   byte count.
7. Reconstruct measurements and recompute the stored independent detector
   values.
8. Verify the block logical-payload SHA-256.
9. Verify trailer counts and the archive SHA-256.
10. Reject trailing data.

Stable public error categories include:

```text
RSMP_BAD_MAGIC
RSMP_UNSUPPORTED_VERSION
RSMP_UNSUPPORTED_SWEEP
RSMP_CIRCUIT_MISMATCH
RSMP_SHAPE_MISMATCH
RSMP_LIMIT_EXCEEDED
RSMP_TRUNCATED
RSMP_CORRUPT_BLOCK
RSMP_CHECKSUM_MISMATCH
RSMP_TRAILING_DATA
RSMP_IO
```

CLI diagnostics use:

```text
rsmp error [<CODE>]: <plain-language detail>
```

`ArchiveLimits` bounds measurements, detectors, observables, total shots,
block shots, compressed bytes, decompressed bytes, and the Zstandard window.
Library defaults are safe and callers may opt into larger explicit limits.

When output targets are file paths, pack and unpack commands write sibling
temporary files and rename only after `finish()` succeeds. Each individual
file is committed atomically. Multiple output files cannot be committed as one
filesystem transaction: if a rename fails after another output was committed,
the command reports `RSMP_IO`, removes remaining temporary files, and names the
already-committed outputs in its diagnostic. At most one output may target
stdout. stdout cannot be rolled back; documentation states that a late error
may leave complete earlier blocks on stdout, while the process always exits
nonzero.

There is no `--ignore_checksum`, partial-success, or salvage option in v1.

## Verification catalog

Create one shared manifest consumed by transform, archive, CLI, and performance
tests. It contains at least seven valid semantic cases:

1. non-zero reference measurements;
2. `rank(H)=0`, with all measurement columns free;
3. repeated or linearly dependent detectors;
4. measurements and detectors inside `REPEAT`;
5. logical observable recovery;
6. loss-visible measurement operations and their expanded bit count;
7. the existing surface-code d11/r100 benchmark fixture.

The catalog records provenance, circuit path, measurement input or generation
command, expected `M/D/L/rank`, expected output hashes, and consuming tests.
Tiny issue-specific smoke fixtures may supplement it, but must not replace it.

Correctness hard gate:

```text
unpack(pack(measurements, circuit), circuit).measurements
    == original measurements
```

For each valid case, detectors and observables are compared bit-for-bit with
the existing measurement-to-detection path. Seeded property tests cover random
GF(2) matrices and bit tables, including zero rank, full rank, rank deficiency,
zero shots, and shot counts not divisible by 8 or 64.

Generate at least twelve deterministic corruption mutations from a known-good
archive, covering:

- bad magic, version, flags, and circuit;
- truncated header, block, Zstandard frame, and trailer;
- overlong or non-canonical varints and out-of-range indices;
- duplicated, omitted, and reordered blocks;
- changed payload, checksum, and declared lengths;
- resource-limit violations;
- non-zero padding and trailing data.

Each mutation asserts the exact stable error category. A valid archive with a
different circuit is the negative control for circuit binding. A sweep circuit
must fail with `RSMP_UNSUPPORTED_SWEEP`.

## Compression and performance acceptance

Use the existing
`benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`
case with 1024 shots and seed 7. It has:

- `M = 12,121`;
- `D = 12,000`;
- `L = 1`;
- `rank(H) = 12,000` under the selected transform;
- 121 free measurement bits per shot;
- 1,552,384 bytes of `b8` measurement data.

The checked size gate requires:

- archive size less than 20% of the original `b8` bytes;
- archive size less than 75% of applying the same Zstandard level directly to
  the original `b8` bytes.

This second comparison proves the semantic transform contributes material
value instead of merely wrapping measurements in a general compressor.

For at least 1 MiB of high-entropy input on a circuit with no detector
constraints, the archive must be no more than 102% of the original `b8` size.
This proves the dense fallback avoids pathological expansion.

The benchmark artifact also reports encode MiB/s, decode MiB/s, selected codec
per block, detector density, free-bit count, compression ratios, and maximum
logical block working set. v1 does not impose a fixed wall-clock threshold;
throughput becomes a hard gate only after checked evidence establishes a
portable baseline.

## Documentation

Document:

- the mathematical reason the transform is lossless;
- exact binary fields, bit order, varint rules, hash coverage, and versioning;
- CLI examples for pack, unpack, and verify-only;
- supported input/output formats;
- the circuit-only and no-sweep boundary;
- resource limits and transactional file behavior;
- stdout partial-output behavior on late errors;
- compression evidence and claim limits.

Maintain a tiny committed v1 reader fixture so future readers must retain v1
compatibility. Writer byte-for-byte determinism across Zstandard versions is
not required; semantic output, container validity, and v1 readability are.

## Risks and mitigations

- **Elimination bugs could silently change measurements.** Mitigate with
  independent encode/decode property tests, recomputed syndrome checks, and
  comparison to `m2d`.
- **Dependent detector rows could be mishandled.** Preserve selected original
  row identities and explicitly test rank-deficient matrices.
- **Sparse coding can expand high-density data.** Compare exact raw candidate
  sizes per block and select dense on ties.
- **Circuit normalization may surprise users.** Document that comments and
  whitespace normalize, but structural semantic rewrites do not.
- **Corrupt lengths can trigger oversized allocation.** Check arithmetic and
  limits before allocating or initializing Zstandard decoders.
- **Streaming output cannot always be transactional.** Use atomic file targets
  and clearly document stdout semantics.
- **Compression dependency changes writer bytes.** Treat Zstandard as a stable
  decoder format while testing semantic compatibility instead of requiring
  cross-version byte identity.

## Approved boundary summary

The v1 archive stores only the information required to reconstruct all
measurement results. Decompression requires the original circuit and produces
measurements, detectors, and logical observable flips. It accepts no DEM-only
mode, no sweep circuits, no random shot access, no lossy path, and no corrupt
block salvage. It combines circuit-derived reversible GF(2) coordinates with
adaptive per-block encoding and independent Zstandard frames, guarded by
versioning, circuit binding, resource limits, block checks, and whole-archive
integrity.
