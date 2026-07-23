# Issue 524 b8 Pack/Unpack MVP Design

Date: 2026-07-23
Status: auto-approved for non-interactive Agent Desk run

## Scope

Add the first user-visible `rsmp` CLI path: `rstim pack_samples` and
`rstim unpack_samples`. This issue supports only circuit-bound archives, only
Stim-compatible `b8` measurement input and `b8` interchange outputs, and only
the zero-or-one-block dense archive state machine from issue #523.

The commands keep the exact underscore spelling and option names from the issue
contract. Format flags default to `b8` and reject every other value. The unpack
command accepts any requested subset of measurement, detector, and observable
outputs, but requires at least one destination.

## Architecture

The CLI stays thin over `MeasurementTransform` and `sample_archive`. Packing
parses the circuit, builds a `MeasurementTransform`, checks the requested shot
count against `ArchiveLimits` before reading sample bytes, reads one strict
whole `b8` measurement block into `BitTable(M, shots)`, and writes the archive
through `SampleArchiveWriter::write_measurements` followed by `finish()`.

Unpacking parses the circuit, opens the archive with `SampleArchiveReader`,
consumes the optional decoded block with `next_block()`, retains it until
`finish()` succeeds, and then writes requested `b8` streams from the decoded
measurements, detections, and observable flips. Measurements are reconstructed
internally even when only detector or observable output is requested because
the archive reader's decoded block is the trusted completion boundary.

## Strict b8 Adapter

Packing does not use the existing permissive `read_shots_b8(data, bits)` path.
The dedicated adapter computes `ceil(M / 8)` and `N * bytes_per_shot` with
checked arithmetic, rejects short or extra input, rejects nonzero unused high
bits in every final per-shot byte, and constructs `BitTable(0, N)` from empty
input when `M = 0` and `N > 0`.

Shot counts are converted only after checking platform and archive limits. A
shot count above the one-block limit is rejected before consuming stdin.

## Streams and Publication

The path value `-` means stdin for supported inputs and stdout for supported
outputs. Argument validation checks required values, `b8` format compatibility,
at most one stdin input, at most one stdout output, and duplicate final file
outputs before opening or truncating destinations.

File outputs use collision-safe sibling temporary files. Pack writes and flushes
only the temporary archive and renames it after writer `finish()` succeeds.
Unpack opens and validates the archive first, keeps every requested file output
unpublished until reader `finish()` succeeds, writes each temporary result, then
renames each temporary to its final destination. Unpublished temporary files are
removed on failure; already-published paths are never deleted.

stdout is the documented non-transactional exception. The validation rules still
ensure that at most one output targets stdout.

## Error Handling

Library archive errors are rendered through their typed public code strings,
including `RSMP_UNSUPPORTED_SWEEP` and `RSMP_CIRCUIT_MISMATCH`. CLI validation
errors are ordinary nonzero command failures and do not freeze stable wording in
this issue. Stable CLI rendering and full precedence coverage remain assigned
to issue #529.

## Verification

Add `rstim/tests/cli_rsmp_b8.rs`. The integration test uses the real binary and
must cover seven positive semantic roles from the shared catalog, byte-for-byte
measurement round trips, detector and observable comparisons against
`measurements_to_detections`, measurements-only, detectors-only,
observables-only, all-three-output unpack, file IO, one stdin/stdout pipeline,
and the `M = 0` nonzero-shot case.

The negative controls prove the ten required validation and publication
contracts, including circuit mismatch code preservation, strict `b8` short,
extra, and padding checks, pre-open argument failures, over-limit rejection
before consuming stdin, and corrupt archive preservation of existing result
files.

The required focused verification command is:

```console
cargo test --locked -p rstim --test cli_rsmp_b8 -- --nocapture
```

The output must contain exactly:

```text
PASS rsmp b8 cli valid_cases=7 negative_cases=10
```
