# Issue 530 rsmp Corruption Corpus Design

Date: 2026-07-24
Status: approved by Standing Answer Policy for non-interactive Agent Desk run

## Need

The rsmp v1 reader now has stable public error codes, bounded archive limits,
and one immutable two-block compatibility fixture. Issue #530 must turn the
shared fixture catalog into an executable corruption corpus that proves those
reader contracts against the committed fixture bytes, including exact mapping
for named recipes, every proper prefix, deterministic bit flips, and streaming
terminal behavior.

## Approach

Use the #532 fixture as the only valid archive source. A small verifier module
in the `rstim` crate will parse the catalog and fixture manifest, derive a
format-aware layout from the archive bytes, create each mutation, run the public
`SampleArchiveReader`, and compare the stable `RSMP_*` code against the
catalogued expectation. The example binary `rsmp_corruption_corpus` will expose
the reviewer command and write `benchmarks/out/rsmp-v1/corruption-summary.json`.
The integration test will call the same verifier directly and exercise focused
streaming-reader contracts.

## Mutation Model

Named recipes are catalog entries with stable IDs, locators, expected public
error codes, mutation kind, validation boundary, and recomputation notes. Most
recipes mutate the pinned archive bytes; control recipes provide an alternate
valid circuit or custom `ArchiveLimits`. Mutations that need to reach an inner
boundary recompute the enclosing header digest and/or trailer archive digest.
Payload recipes recompress modified raw streams with the existing zstd frame
helper when structural lengths must stay coherent.

Exhaustive truncation is generated for archive lengths `0..archive_len`.
Deterministic bit flips are generated from catalogued semantic locators that
span the global header, both block headers, both stream kinds, logical digests,
the trailer prefix, and archive digest. These generated cases report their own
counts and exact expected codes but do not count toward the named invalid
recipe threshold.

## Reader Semantics

The corpus exercises `SampleArchiveReader::open`, `next_block`, and `finish`
directly. A corrupt current block is only reported as an error. A block already
returned before a later corruption remains visible to the caller, but `finish`
must still reject the archive. After any reader error, the reader latches into a
terminal state so later `next_block` and `finish` calls cannot return data or
success.

## Summary Output

The verifier returns one structured result per named recipe and generated
mutation, aggregate counts by stable error code, and failure buckets for
unexpected successes, wrong error codes, panics, and timeouts. The example
prints exactly:

```text
PASS rsmp corruption corpus valid=1 invalid>=12
```

only when the valid fixture decodes, every named recipe fails with its expected
code, all prefixes map to `RSMP_TRUNCATED`, all bit flips map as catalogued, and
no panic or timeout is recorded.

## Testing

The focused integration test covers:

- exact named recipe mapping and rejection of an intentionally wrong expected
  code;
- exhaustive truncation mapping;
- format-aware bit flips;
- corrupt-current-block rejection;
- already-returned prefix followed by terminal `finish` failure; and
- terminal behavior after any reader error.

Final verification runs the reviewer-facing example command, the focused
integration tests, the negative control, and `cargo test`.
