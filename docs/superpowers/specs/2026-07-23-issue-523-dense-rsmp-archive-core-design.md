# Issue 523 Dense rsmp Archive Core Design

Date: 2026-07-23
Status: auto-approved for non-interactive Agent Desk run

## Scope

Implement the smallest complete `rsmp` v1 archive library core on top of the
existing fixed-width envelope and reversible measurement transform. This issue
supports zero blocks for `total_shots = 0` and exactly one dense data block for
positive shot counts. CLI integration, sparse encoding, multi-block streaming,
compatibility specimens, and broad hostile-input precedence coverage remain
deferred to their sequenced issues.

## Architecture

`sample_archive::format` remains the normative fixed layout. New
`sample_archive` modules add safe limits, dense bit packing, SHA-256 integrity,
Zstandard frame handling, and the public streaming-shaped writer/reader API.
The writer accepts only raw measurement `BitTable` values, owns one compiled
`MeasurementTransform`, creates `EncodedMeasurementBlock` internally, and writes
header, optional block, and trailer through a non-seekable `Write`.

The reader takes a `Read`, parsed circuit instructions, and `ArchiveLimits`.
It reads the header, verifies the header digest and circuit binding, constructs
the transform with `ArchiveLimits.transform`, returns at most one decoded block
from `next_block()`, and verifies trailer digest, counts, EOF, and trailing-data
absence only in `finish()`.

## Dense Streams

Both logical streams are packed directly from `BitTable::get`:
syndrome bits in `(shot, selected_detector)` order and free bits in
`(shot, free_measurement)` order, LSB-first, without per-shot byte padding.
Zero-width or zero-shot streams use the existing canonical empty-stream
representation. Non-empty streams are independent Zstandard frames with content
size and checksum enabled. The reader validates declared lengths, frame content
size, checksum, window size, decoded byte count, final padding, and the block
logical-payload digest before reconstructing measurements.

## Limits

`ArchiveLimits` embeds exactly one `MeasurementTransformLimits` value and uses
that value for transform construction, writer actual-usage validation, block
shot limits, transform working bytes, and block working bytes. Archive-specific
fields cover total shots, rank/free-width shape, compressed and decompressed
stream/archive bytes, and Zstandard window/decoder bounds. All archive-controlled
dimensions and lengths are checked before allocation, vector reservation,
transform construction, or decoder initialization.

## Error Mapping

Reader and writer errors use the #520 public taxonomy. Header and whole-archive
digest mismatches map to `RSMP_CHECKSUM_MISMATCH`; malformed Zstandard frames
or checksum decode failures map to `RSMP_DECOMPRESSION_FAILED`; reconstructed
canonical logical payload mismatches map to
`RSMP_LOGICAL_DIGEST_MISMATCH`; semantic circuit changes map to
`RSMP_CIRCUIT_MISMATCH`; the recognized sparse syndrome codec maps to
`RSMP_UNSUPPORTED_FEATURE`; one-block contract violations map to
`RSMP_MALFORMED_ARCHIVE` or `RSMP_SHAPE_MISMATCH` according to the issue's
negative cases.

## Verification

Add `rstim/tests/rsmp_archive_dense.rs`. The test creates six positive archives
only through `write_measurements`, covers the dense stream edge cases, verifies
exact recovery against the transform and `m2d`, and exercises short reads and a
non-seekable writer. Fifteen deterministic negative cases mutate valid archives
and assert exact `SampleArchiveErrorCode` values. The required command is:

```console
cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture
```

The output must contain:

```text
PASS rsmp dense archive valid_cases=6 negative_cases=15
```
