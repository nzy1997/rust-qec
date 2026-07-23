# Issue 525 Adaptive Syndrome Codec Design

Date: 2026-07-23
Status: approved by non-interactive Agent Desk standing policy

## Context

Issue #525 adds only the `rsmp v1` adaptive syndrome pre-codec. The existing
single-block archive writer and reader from #523 already own the public
state-machine API, block headers, frame limits, Zstandard frame validation, and
logical digest checks. The existing dense helper already packs `BitTable` bits
in the required shot-major order, so this change should extend that boundary
instead of moving raw codec logic into container parsing.

## Chosen Approach

Add a focused `sample_archive::syndrome` module with helpers for:

- checked dense byte-length calculation for the selected-detector table;
- checked sparse byte-length counting without allocating the sparse payload;
- raw dense/sparse materialization for the selected candidate only;
- canonical sparse ULEB128 encode/decode; and
- raw sparse-to-`BitTable` reconstruction with strict shot-record and index
  validation.

The writer computes dense and sparse raw lengths first, chooses the shorter
representation with dense winning ties, materializes only the selected raw
bytes, and compresses that single raw payload through the existing bounded
one-frame Zstandard helper. The block logical digest remains based on canonical
dense selected-detector bytes followed by dense free bytes, so sparse transport
does not change archive semantics.

The reader dispatches syndrome bytes by codec ID after frame validation. Dense
uses the existing dense unpacker. Sparse validates canonical ULEB records,
strictly increasing detector hits, exact `S` shot records, no trailing decoded
bytes, checked delta arithmetic, and indices in `0..R` before transform
reconstruction is called. Declared raw length mismatches and final dense padding
fail as malformed archives.

## Rejected Approaches

1. Keep sparse support in `reader.rs` and `writer.rs`. This would make known
   raw-byte tests depend on container plumbing and would grow files that already
   mix archive state and validation.
2. Materialize dense and sparse bytes then compare. This is simpler but violates
   the bounded candidate-selection rule and must be caught by telemetry.
3. Serialize `BitTable::row_words()` directly. That follows detector-major
   internal storage and violates the v1 shot-major wire order.

## Test Strategy

Add `rstim/tests/rsmp_adaptive_codec.rs` as the issue verification test. It
will check the three selection/known-byte cases, four ULEB boundary cases,
eleven malformed raw/declared cases, 4,096 deterministic round trips, and
test-only telemetry proving at most one complete codec candidate is
materialized.
