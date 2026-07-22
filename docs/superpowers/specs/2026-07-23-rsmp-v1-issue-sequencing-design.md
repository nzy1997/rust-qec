# rsmp v1 Issue Sequencing Design

Date: 2026-07-23
Status: approved in conversation; pending written-spec review

## Purpose

This addendum turns the approved circuit-bound sample archive design into a
safe sequence of one-issue/one-PR deliverables for GitHub issues #520 through
#533. It resolves contradictions discovered during a cold-reader audit without
changing the product boundary of `rsmp v1`.

The original feature design remains authoritative for the archive's goals,
mathematics, and supported scope. This document is authoritative for ownership
between issues, dependency order, staged API behavior, and the acceptance
contracts that prevent later issues from rewriting earlier work.

## Batch-wide decisions

### One issue, one implementation PR

Each issue produces one independently reviewable implementation PR. A
dependent issue starts only after its prerequisite behavior is available on the
default branch. Implementation plans are temporary working notes outside the
repository and are not committed.

The existing approved design commit is included in the #520 branch so the
shared design and the first normative format implementation land together.
Every issue links to a fixed commit permalink once that branch is pushed.

### Public CLI spelling

The v1 commands are:

```text
rstim pack_samples
rstim unpack_samples
```

They follow the repository's underscore convention. The shared options are
spelled `--circuit`, `--in`, `--out`, `--shots`, and `--in_format`. Unpack uses
`--measurements_out`, `--measurements_out_format`, `--detectors_out`,
`--detectors_out_format`, `--obs_out`, `--obs_out_format`, and
`--verify_only`. `-` denotes stdin or stdout where that stream is supported.

At most one input may consume stdin and at most one output may target stdout.
The CLI validates stream conflicts, duplicate final paths, format compatibility,
and required arguments before opening or truncating any destination.

### Shared checked measurement semantics

Issue #522 owns a single checked circuit-measurement layout and record-target
resolver shared by statistics, measurement-to-detection conversion, and the
archive transform. It must:

- count every measurement-producing operation consistently, including `MPP`,
  `MPAD`, heralded measurements, and loss-visible measurement families;
- expand `REPEAT` counts with checked arithmetic;
- reject invalid or out-of-history record references instead of converting a
  negative index to `usize`;
- cancel repeated record references by XOR; and
- expose dimensions and detector/observable parity rows without duplicating
  measurement semantics in the archive module.

`MeasurementTransform::from_circuit` constructs the reference, parity rows,
elimination state, selected detector rows, pivots, and free columns once.
`encode_block` and `decode_block` reuse that compiled transform for every shot
block.

### Safe limits exist before public readers

Issue #520 owns structural error primitives and the stable public error-code
names. Issue #523 owns checked wire-size arithmetic, conservative default
limits, and bounded Zstandard decode before any archive reader is exposed.
Issue #529 completes configurable limits, aggregate accounting, hostile-input
coverage, validation precedence, and stable CLI rendering. It does not retrofit
safety into an allocation path that was previously unbounded.

The v1 public taxonomy is exactly:

```text
RSMP_BAD_MAGIC
RSMP_UNSUPPORTED_VERSION
RSMP_UNSUPPORTED_FEATURE
RSMP_UNSUPPORTED_SWEEP
RSMP_CIRCUIT_MISMATCH
RSMP_SHAPE_MISMATCH
RSMP_LIMIT_EXCEEDED
RSMP_TRUNCATED
RSMP_MALFORMED_ARCHIVE
RSMP_DECOMPRESSION_FAILED
RSMP_CHECKSUM_MISMATCH
RSMP_LOGICAL_DIGEST_MISMATCH
RSMP_TRAILING_DATA
RSMP_IO
```

Issues must not introduce a catch-all `RSMP_CORRUPT_BLOCK` alias. Unknown
required features use `RSMP_UNSUPPORTED_FEATURE`; invalid canonical structure,
ordering, IDs, varints, or padding use `RSMP_MALFORMED_ARCHIVE`; Zstandard
frame, decode, or frame-checksum failures use `RSMP_DECOMPRESSION_FAILED`; and
reconstructed logical content that disagrees with its stored digest uses
`RSMP_LOGICAL_DIGEST_MISMATCH`.

No archive-controlled count or length reaches `BitTable::new`, `Vec`
allocation, frame decode, or transform reconstruction before checked conversion
and limit validation.

### Streaming completion is two-phase

The reader API uses `open`, `next_block`, and `finish` from its first archive
implementation. `next_block` proves only that the returned block passed all
block-local checks. `finish` proves trailer integrity, final counts, the
whole-archive digest, EOF, and absence of trailing bytes.

A later failure cannot revoke an already-returned block. Such a prefix is not a
successful archive and is not salvage. File outputs stay in sibling temporary
files until `finish` succeeds. stdout is the documented non-transactional
exception.

### Canonical result-format streaming

Issue #527 introduces strict streaming result readers and writers; it does not
claim that the existing whole-buffer readers are already streaming. Strict
readers reject malformed `01`, nonzero `b8` padding, nonzero final `ptb64`
padding, short input, and extra input.

`ptb64` output carries an incomplete 64-shot group across archive-block
boundaries. Concatenating independently padded per-block `ptb64` output is not
canonical when a configured archive block size is not a multiple of 64.

### Codec and memory rules

Dense syndrome bytes use `(shot, selected_detector)` order, LSB-first, with no
per-shot padding and only zero final padding. Sparse syndrome bytes contain a
canonical ULEB128 hit count per shot followed by canonical ULEB128 detector
index deltas.

The encoder computes both candidate lengths with checked arithmetic but does
not materialize both full candidates. It materializes and compresses only the
selected representation; dense wins an exact raw-size tie.

Streaming memory evidence reports a checked byte count, not a count of Rust
objects called "blocks". The peak includes decoded tables, codec buffers,
compressed frames, transform scratch, and Zstandard state, while excluding the
immutable compiled circuit transform from per-block growth. Tests show that
peak live block memory does not grow with total archive block count.

### Evidence contract

Compression evidence pins the exact measurement producer, command, input hash,
Zstandard implementation and level, frame settings, and arithmetic. Thresholds
are checked with integer cross-multiplication:

- archive bytes are strictly less than 20% of original `b8` bytes;
- archive bytes are strictly less than 75% of the same-level direct-Zstandard
  baseline for the d11/r100 benchmark; and
- the no-detector high-entropy archive is at most 102% of original `b8` bytes.

The high-entropy control uses a fixed, documented generator and a byte-aligned
shape of at least 1 MiB. Direct Zstandard remains a reported diagnostic for
that control but is not its normative denominator.

### Compatibility and corruption

The shared verification catalog is
`rstim/tests/fixtures/rsmp/catalog.json`. Its seven required IDs are semantic
roles, not an exact total, and its corruption recipes use symbolic field paths
instead of hard-coded byte offsets.

Issue #532 moves before #530. It pins an immutable, additive v1 reader specimen
with at least two blocks, both sparse and dense syndrome codecs, nonzero
reference measurements, free coordinates, an observable, and non-byte-aligned
padding. The current writer is never invoked by the compatibility test.

Issue #530 uses that specimen as its known-good base. Named corruption recipes
use format-aware field locators and record which enclosing lengths or digests
are recomputed so that one intended validation boundary fails. The required
recipe count excludes exhaustive truncation points and generated bit flips;
all three counts are reported separately.

Issue #530 verifies the library reader and corpus contract. It does not depend
on the CLI publication and `--verify_only` behavior owned by #531. Issue #531
separately proves that the same reader errors are rendered safely through the
CLI and that file outputs remain unpublished on late archive failure.

### Output publication

Pack archives and unpack result files are written to sibling temporary files
and published only after successful completion. Atomicity is per file, not
across arbitrary paths. If a later rename fails, the command:

1. returns `RSMP_IO`;
2. removes only unpublished temporary files;
3. leaves already-published files in place; and
4. names every published path in the diagnostic.

It never deletes an already-published path to simulate rollback, because that
could destroy a pre-existing destination that was atomically replaced.

## Issue ownership

| Issue | Owned deliverable |
|---|---|
| #520 | Normative binary envelope, module skeleton, structural errors, version/flag policy, and zero-shot representation |
| #521 | Shared semantic fixture catalog and field-level corruption recipe catalog |
| #522 | Checked shared measurement semantics and reusable reversible transform |
| #523 | Safe single-block dense archive state machine with default limits and bounded Zstandard decode |
| #524 | Strict one-block `b8` CLI path with the final command and option spelling |
| #525 | Adaptive dense/sparse syndrome codec with bounded candidate selection |
| #526 | Multi-block streaming and byte-based memory evidence |
| #527 | Strict streaming result-format adapters and full CLI interoperability |
| #528 | Pinned, checked compression evidence |
| #529 | Configurable and aggregate limits, validation precedence, and stable error rendering |
| #532 | Immutable v1 reader compatibility specimen |
| #530 | Complete corruption corpus based on #532 |
| #531 | File publication hardening and `--verify_only` |
| #533 | Operational documentation and always-on readiness aggregation |

## Required implementation order

The batch is executed sequentially in this order:

```text
#520 -> #521 -> #522 -> #523 -> #524 -> #525 -> #526
     -> #527 -> #528 -> #529 -> #532 -> #530 -> #531 -> #533
```

Although some issues are technically parallelizable, sequential execution
keeps every PR based on behavior already present on the default branch and
preserves the one-issue/one-PR review model requested for this batch.

## Readiness output

The final readiness aggregator consumes structured results from the catalog,
format/CLI tests, compression checker, corruption corpus, and compatibility
fixture. It writes a machine-readable artifact with actual counts. Its last
non-empty stdout line is exactly:

```text
PASS rsmp v1 readiness valid_cases=7 corruption_cases>=12 compatibility=1 compression=pass
```

`valid_cases=7` means the seven required semantic roles are present; the
catalog may contain additional valid cases. `corruption_cases>=12` counts
distinct named recipes, not generated truncation or bit-flip instances.

## Out of scope

- Changing the approved rsmp v1 product boundary.
- Combining multiple issues into one implementation PR.
- Stacked PRs whose closing behavior depends on an unmerged non-default base.
- Direct pushes of implementation commits to the default branch.
- Crash-durable transactions across multiple arbitrary output paths.
