# RSMP v1 Binary Envelope

This document is normative for the RSMP v1 structural envelope. All integers
are unsigned little-endian. The structural layer does not compress streams,
derive transforms, calculate digests, or parse archive state.

## Fixed records

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

## Compatibility and canonical structure

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

## Digest coverage

`header_sha256` covers global-header bytes `[0, 120)`. The block logical digest
covers canonical uncompressed dense selected-detector bytes followed by dense
free-measurement bytes. `archive_sha256` covers the complete global header,
all complete block bytes, and the trailer prefix `[0, 32)`.

## Known byte vectors

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

## Error taxonomy

The public codes are `RSMP_BAD_MAGIC`, `RSMP_UNSUPPORTED_VERSION`,
`RSMP_UNSUPPORTED_FEATURE`, `RSMP_UNSUPPORTED_SWEEP`,
`RSMP_CIRCUIT_MISMATCH`, `RSMP_SHAPE_MISMATCH`, `RSMP_LIMIT_EXCEEDED`,
`RSMP_TRUNCATED`, `RSMP_MALFORMED_ARCHIVE`, `RSMP_DECOMPRESSION_FAILED`,
`RSMP_CHECKSUM_MISMATCH`, `RSMP_LOGICAL_DIGEST_MISMATCH`,
`RSMP_TRAILING_DATA`, and `RSMP_IO`.

Structural parsing maps a bad magic to `RSMP_BAD_MAGIC`, a non-v1.0 version to
`RSMP_UNSUPPORTED_VERSION`, unknown required features to
`RSMP_UNSUPPORTED_FEATURE`, unknown global v1 identifiers to
`RSMP_UNSUPPORTED_FEATURE`, a short record to `RSMP_TRUNCATED`, and reserved
fields, canonical stream encodings, header lengths, parsed shot ranges, or
parsed declared-length arithmetic to `RSMP_MALFORMED_ARCHIVE`.
Public checked size helpers map caller-supplied representability overflow to
`RSMP_LIMIT_EXCEEDED`. Digest, compression, and archive-state validation are
owned by later archive layers.
