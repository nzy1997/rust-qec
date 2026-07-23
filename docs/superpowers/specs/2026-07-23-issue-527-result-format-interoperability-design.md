# Issue 527 Result-Format Interoperability Design

Date: 2026-07-23
Status: auto-approved for non-interactive Agent Desk run

## Scope

Issue #527 adds strict streaming adapters between Stim/rstim result formats and
the `rsmp` v1 archive stream. Packing accepts only measurement input in `01`,
`b8`, or `ptb64`. Unpacking writes measurement and observable streams in `01`,
`b8`, `r8`, `hits`, or `ptb64`; detector streams also support `dets`.

The archive reader and writer remain the streaming boundary from issue #526.
This issue does not add new archive error codes, path-publication hardening from
#531, new result formats, or `r8`/`hits` pack input.

## Chosen Approach

Add one `result_stream` module with `ResultBlockReader<R: Read>` and
`ResultBlockWriter<W: Write>`. The reader knows the bit width, exact total shot
count, input format, and chunk shot limit. It reads only the bytes needed for
the next chunk and returns `BitTable` values to `SampleArchiveWriter`, including
canonical `ptb64` 64-shot groups that may cross archive block boundaries.

The writer accepts complete `DecodedSampleBlock` values from
`SampleArchiveReader`, validates equal shot counts across measurements,
detections, and observables before writing any block bytes, then serializes only
the configured output kind. `ptb64` output keeps pending shots across decoded
archive-block boundaries and pads only on `finish`. Detector `dets` output uses
both detections and observable flips, preserving existing `D#`/`L#` semantics.

## Alternatives Rejected

1. Reuse whole-buffer readers in `cli.rs`. This would violate the bounded-read
   requirement and miss strict `b8`/`ptb64` padding validation.
2. Align archive blocks to result-format chunks. The #526 archive writer
   already coalesces caller chunks; forcing alignment would reintroduce caller
   partition dependence.
3. Add separate writer types for each result kind. A single kind+format writer
   keeps validation and `ptb64` carry behavior in one place.

## Error Handling

Result-format errors are usage or data-format errors rendered by the CLI as
plain command failures. Archive errors continue to use the stable `RSMP_*`
taxonomy. Format compatibility is validated before opening or truncating any
destination.

## Verification

Add `rstim/tests/rsmp_result_format_interop.rs`. The focused command is:

```console
cargo test --locked -p rstim --test rsmp_result_format_interop -- --nocapture
```

The test output must contain exactly:

```text
PASS rsmp result formats pack_formats=3 measurement_formats=5 detector_formats=6 observable_formats=5 ptb64_cross_block=1 guarded_read=1 negative_cases=14
```
