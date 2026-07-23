# Issue 526 Stream Multiple Bounded Shot Blocks Design

Date: 2026-07-23
Status: auto-approved for non-interactive Agent Desk run

## Scope

Issue #526 extends the `rsmp` v1 archive library from the #523 zero-or-one-block
state machine to canonical sequential multi-block writing and reading. The
public writer boundary remains `SampleArchiveWriter::write_measurements(&BitTable)`.
The CLI result-format streaming and transactional publication behavior remain
owned by later issues.

## Chosen Approach

Keep the fixed header, block, and trailer envelope from `sample_archive::format`
and generalize the writer and reader state counters around it. The writer owns
one compiled `MeasurementTransform`, one optional carry `BitTable`, and the
incremental archive hasher. Each incoming table is copied shot-by-shot into the
carry table until the configured `max_shots_per_block` is reached. Full blocks
are encoded and written immediately; `finish` emits at most one shorter final
block, then writes a trailer whose block and shot totals cover every emitted
block.

The reader keeps the existing `open`, `next_block`, `finish` lifecycle. It
tracks the next expected zero-based block number and the checked first-shot sum.
`next_block` reads and validates one block-local unit only: frame bounds, codec
rules, dimensions, reconstruction, and logical digest. It returns `None` only
after it sees and parses the trailer, storing that trailer for `finish`.
`finish` drains unread blocks if necessary, then validates trailer counts,
total shots, whole-archive digest, EOF, and trailing-byte absence.

## Alternatives Rejected

1. Expose an encoded-block writer API. This would make caller code responsible
   for transform ownership and contradicts the stable public boundary from the
   sequencing addendum.
2. Buffer all supplied measurement chunks and split them at `finish`. This
   would make caller partitions easier to implement but violates the bounded
   memory requirement and would let total archive size determine peak memory.
3. Add random access or a block index now. Sequential archives are sufficient
   for this issue, and indexes are explicitly out of scope.

## Memory Evidence

Add test-only byte accounting under `sample_archive::telemetry`. It reports:

- writer-owned buffered input bytes and max buffered shots;
- decoded blocks retained by the reader;
- transform encode/decode payload peaks;
- raw syndrome/free codec buffers;
- compressed syndrome/free frame buffers; and
- a conservative Zstandard working-state estimate based on the configured
  archive limits.

The immutable compiled `MeasurementTransform` retained bytes are reported
separately from per-block mutable state. Accounting helpers use checked
addition and multiplication and expose their formulas as test diagnostics.
The streaming test compares the same shape and block size for 3 blocks and 21
blocks and requires equal per-block mutable high-water marks.

## Error Mapping

Malformed block order, repeated or skipped block numbers, incorrect first-shot
values, zero-shot interior blocks, impossible dense lengths, and invalid
canonical stream structure map to `RSMP_MALFORMED_ARCHIVE`. Dimension or
trailer-total disagreements map to `RSMP_SHAPE_MISMATCH`. Checked first-shot
overflow and configured block-size violations map to `RSMP_LIMIT_EXCEEDED`.
EOF between blocks maps to `RSMP_TRUNCATED`. A trailer archive digest mutation
can still allow an earlier block from `next_block`, but `finish` must return
`RSMP_CHECKSUM_MISMATCH`.

## Verification

Add `rstim/tests/rsmp_archive_streaming.rs`. The required command is:

```console
cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture
```

Its output must contain exactly:

```text
PASS rsmp streaming boundary_cases=4 partition_invariant=1 malformed_cases=10 max_buffered_shots=4096 max_live_decoded_blocks=1 max_transform_payloads=2 total_block_growth_bytes=0
```
