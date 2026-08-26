# RSMP v1 Binary Envelope

This document is normative for the RSMP v1 archive format. RSMP stores result
sample measurements together with the circuit identity needed to recover
detectors and observables. v1 is a circuit-bound archive format, not a
standalone detector-error-model sample format.

## Circuit-Derived Lossless Transform

The v1 transform is lossless because it is derived from the original circuit,
the circuit's noiseless reference sample, and the exact detector/observable
linear relations over measurement bits. For each shot, the encoder maps the
measurement vector into selected-detector bits plus free-measurement bits. The
selected-detector part has rank equal to the detector matrix rank; the
free-measurement part carries the remaining independent measurement columns.

Given the same original circuit, the decoder rebuilds the same transform,
checks the circuit SHA-256 identity, recombines selected-detector and
free-measurement coordinates, and applies the noiseless reference to reconstruct
the original measurement bits. That map is invertible for the circuit and shape
recorded in the header: no measurement bit is dropped, and detector and
observable outputs are recomputed from the reconstructed measurements.

## Binary Fields and Canonical Encoding

All fixed-width integers are unsigned little-endian. Bit-packed result streams
use LSB-first bit order within each byte. Sparse syndrome payload counts and
index deltas use canonical ULEB128: overlong encodings are malformed. Unused
padding bits in dense final bytes must be zero padding.

| Global header field | Offset | Width |
|---|---:|---:|
| magic | 0 | 8 |
| format_major | 8 | 2 |
| format_minor | 10 | 2 |
| header_len | 12 | 4 |
| required_flags | 16 | 4 |
| optional_flags | 20 | 4 |
| reserved_flags | 24 | 4 |
| canonicalization_id | 28 | 2 |
| fingerprint_id | 30 | 2 |
| transform_id | 32 | 2 |
| reference_id | 34 | 2 |
| codec_suite_id | 36 | 2 |
| reserved0 | 38 | 2 |
| max_shots_per_block | 40 | 8 |
| measurement_count | 48 | 8 |
| detector_count | 56 | 8 |
| observable_count | 64 | 8 |
| detector_rank | 72 | 8 |
| total_shots | 80 | 8 |
| circuit_sha256 | 88 | 32 |
| header_sha256 | 120 | 32 |

The global header is exactly 152 bytes. `header_len` is 152 in v1.0 and must
be checked before the value is trusted. It must be between 152 and 65,535.

| Block header field | Offset | Width |
|---|---:|---:|
| magic | 0 | 8 |
| format_major | 8 | 2 |
| format_minor | 10 | 2 |
| block_index | 12 | 8 |
| first_shot | 20 | 8 |
| shot_count | 28 | 8 |
| syndrome_codec_id | 36 | 2 |
| free_codec_id | 38 | 2 |
| reserved0 | 40 | 4 |
| syndrome_uncompressed_len | 44 | 8 |
| syndrome_compressed_len | 52 | 8 |
| free_uncompressed_len | 60 | 8 |
| free_compressed_len | 68 | 8 |
| logical_payload_sha256 | 76 | 32 |

The block header is exactly 108 bytes.

| Archive trailer field | Offset | Width |
|---|---:|---:|
| magic | 0 | 8 |
| format_major | 8 | 2 |
| format_minor | 10 | 2 |
| reserved0 | 12 | 4 |
| block_count | 16 | 8 |
| total_shots | 24 | 8 |
| archive_sha256 | 32 | 32 |

The archive trailer is exactly 64 bytes.

The magic values are `RSTMSMP\0`, `RSMPBLK\0`, and `RSMPEND\0`. v1 readers
accept only major 1, minor 0. A nonzero `required_flags` value is
`RSMP_UNSUPPORTED_FEATURE`; v1.0 has no defined optional flags, so a nonzero
`optional_flags` value is malformed. All reserved fields and reserved flags
must be zero. The global identifiers must be respectively 1 for canonical
circuit text, SHA-256 canonical-circuit fingerprint, selected-detector/free-
measurement transform, noiseless reference, and Zstandard frame codec suite.

`shot_count` is nonzero, and `first_shot + shot_count` must not overflow. The
only syndrome stream codecs are empty (0), dense (1), and sparse LEB128 (2).
The only free stream codecs are empty (0) and dense (3). A zero-length stream
is canonical only as codec 0 with both declared lengths zero. Nonempty stream
length sums must use checked arithmetic.

An archive for zero shots is exactly a global header followed by a trailer,
with both trailer counts zero and no block magic between them. Empty-dimension
streams in nonempty blocks use the canonical empty-stream representation above.

`header_sha256` covers global-header bytes `[0, 120)`. The block logical digest
covers canonical uncompressed dense selected-detector bytes followed by dense
free-measurement bytes. `archive_sha256` covers the complete global header,
all complete block bytes, and the trailer prefix `[0, 32)`.

## Support Boundaries

The original circuit is required for unpack and verify-only validation. The
archive stores a circuit identity digest and shape, but it does not store the
circuit text or a DEM that can replace the circuit. DEM-only input is
unsupported for pack, unpack, and verify-only.

Sweep-bit circuits are unsupported in v1 and must fail with `RSMP_UNSUPPORTED_SWEEP` before archive bytes are produced or trusted.

RSMP v1 has sequential access only and no random shot access. A reader consumes
blocks in order, validates the trailer at end of stream, and exposes no index
that can seek directly to an arbitrary shot.

## Integrity, Authentication, and Access Model

RSMP v1 checks archive integrity but archives are not authenticated. The
`header_sha256`, per-block `logical_payload_sha256`, Zstandard frame checksums,
and final `archive_sha256` detect accidental or adversarial byte changes within
their hash coverage, including the logical payload reconstructed from selected
detector and free measurement streams. They do not prove who produced an
archive, bind an external identity, or replace a signature/MAC layer.

A reader may return already-verified earlier blocks before a late trailer or
trailing-data failure is discovered. Callers that need whole-archive acceptance
must read through end of stream and finish the reader.

## Resource Limits and Validation Precedence

Resource limits are checked before allocations or decompression work that would
exceed those limits. The v1 defaults bound total shots, block shots, archive
bytes, stream lengths, transform dimensions, and logical block working sets.
Limit failures use `RSMP_LIMIT_EXCEEDED`.

Validation precedence is stable at the public error-code level. Readers check
fixed record shape, magic/version/feature support, reserved fields, canonical
integer and padding rules, circuit identity, shape agreement, declared lengths,
decompression, logical digest, archive digest, trailer consistency, and trailing
data in that order when the corresponding bytes are available. I/O failures are
reported as `RSMP_IO`.

## Stable Error Taxonomy

The public codes are:

- `RSMP_BAD_MAGIC`
- `RSMP_UNSUPPORTED_VERSION`
- `RSMP_UNSUPPORTED_FEATURE`
- `RSMP_UNSUPPORTED_SWEEP`
- `RSMP_CIRCUIT_MISMATCH`
- `RSMP_SHAPE_MISMATCH`
- `RSMP_LIMIT_EXCEEDED`
- `RSMP_TRUNCATED`
- `RSMP_MALFORMED_ARCHIVE`
- `RSMP_DECOMPRESSION_FAILED`
- `RSMP_CHECKSUM_MISMATCH`
- `RSMP_LOGICAL_DIGEST_MISMATCH`
- `RSMP_TRAILING_DATA`
- `RSMP_IO`

Structural parsing maps a bad magic to `RSMP_BAD_MAGIC`, a non-v1.0 version to
`RSMP_UNSUPPORTED_VERSION`, unknown required features or unknown global v1
identifiers to `RSMP_UNSUPPORTED_FEATURE`, unsupported sweep circuits to
`RSMP_UNSUPPORTED_SWEEP`, a mismatched circuit digest to
`RSMP_CIRCUIT_MISMATCH`, shape disagreement to `RSMP_SHAPE_MISMATCH`, short
records or streams to `RSMP_TRUNCATED`, reserved fields, canonical stream
encodings, header lengths, parsed shot ranges, block ordering, padding, or
declared-length arithmetic to `RSMP_MALFORMED_ARCHIVE`, Zstandard failures to
`RSMP_DECOMPRESSION_FAILED`, archive/header checksum failures to
`RSMP_CHECKSUM_MISMATCH`, logical payload digest failures to
`RSMP_LOGICAL_DIGEST_MISMATCH`, bytes after a valid trailer to
`RSMP_TRAILING_DATA`, and filesystem/stdin/stdout failures to `RSMP_IO`.

## Compatibility Fixture Policy

The immutable v1 reader fixture is
`rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp`, described by
`rstim/tests/fixtures/rsmp/v1/manifest.toml` and cataloged as
`compat_v1_two_block_sparse_dense`. It contains one fixture, two blocks, and
the syndrome codecs `sparse` and `dense`.

The policy is immutable/additive. Existing compatibility fixture bytes,
manifest hashes, and catalog identity fields are not rewritten after
publication. Future compatibility coverage must add new fixtures or new
manifest entries instead of mutating this specimen.

## Compression Evidence and Claim Limits

Committed compression evidence lives under
`benchmarks/rstim_vs_stim_simulator/results/rsmp-v1/`. The checked gate names
are `benchmark_raw_lt_20pct`, `benchmark_zstd_lt_75pct`, and
`high_entropy_raw_le_102pct`; their arithmetic is integer cross-multiplication
over recorded byte counts.

The evidence checker is:

```console
python3 tools/check_rsmp_v1_compression_evidence.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/rsmp-v1
```

The reproduction command for regenerating the evidence remains separate from
readiness:

```console
python3 -m benchmarks.rstim_vs_stim_simulator.run_rsmp_compression \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/rsmp-v1
```

The benchmark row also carries Stim-format baselines for exactly `b8`, `r8`,
and `ptb64`. All three serialize the same canonical 1024-shot, seed-7
measurement batch; `r8` is produced with the pinned `stim convert` command and
`ptb64` with the pinned stim Python API (the Stim CLI streams single records
and cannot write `ptb64`). Each baseline records its raw byte count, its
direct level-3 Zstandard byte count under the same recorded contract, the
conversion argv, the artifact SHA-256, and a round-trip `b8` SHA-256 that must
equal the canonical measurement SHA-256. The summary and report state byte
counts and ratios relative to the RSMP archive only; no universal
cross-format compression superiority is claimed. The pinned Stim binary
identity, version, and every conversion command are recorded in
`environment.json`.

The recorded environment includes producer identity, Git state, Rust target,
Cargo.lock hash, zstd crate versions, native zstd version, full command argv,
and artifact SHA-256 values. These gates prove only the pinned `rsmp v1`
evidence cases under that recorded producer and zstd contract. No fixed
wall-clock performance gate, cross-version byte-for-byte writer determinism,
or broader compression claim is made. No fixed wall-clock performance gate is part of readiness.

## Known Byte Vectors

```text
GLOBAL_VECTOR = [
52 53 54 4d 53 4d 50 00 01 00 00 00 98 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 01 00 01 00
01 00 01 00 01 00 00 00 00 10 00 00 00 00 00 00
02 01 00 00 00 00 00 00 01 02 00 00 00 00 00 00
02 00 00 00 00 00 00 00 01 01 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 01 02 03 04 05 06 07
08 09 0a 0b 0c 0d 0e 0f 10 11 12 13 14 15 16 17
18 19 1a 1b 1c 1d 1e 1f a0 a1 a2 a3 a4 a5 a6 a7
a8 a9 aa ab ac ad ae af b0 b1 b2 b3 b4 b5 b6 b7
b8 b9 ba bb bc bd be bf
]

BLOCK_VECTOR = [
52 53 4d 50 42 4c 4b 00 01 00 00 00 08 07 06 05
04 03 02 01 18 17 16 15 14 13 12 11 21 00 00 00
00 00 00 00 01 00 03 00 00 00 00 00 05 00 00 00
00 00 00 00 0d 00 00 00 00 00 00 00 02 00 00 00
00 00 00 00 09 00 00 00 00 00 00 00 c0 c1 c2 c3
c4 c5 c6 c7 c8 c9 ca cb cc cd ce cf d0 d1 d2 d3
d4 d5 d6 d7 d8 d9 da db dc dd de df
]

TRAILER_VECTOR = [
52 53 4d 50 45 4e 44 00 01 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
80 81 82 83 84 85 86 87 88 89 8a 8b 8c 8d 8e 8f
90 91 92 93 94 95 96 97 98 99 9a 9b 9c 9d 9e 9f
]
```
